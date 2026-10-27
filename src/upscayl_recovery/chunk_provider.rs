//! Bounded directory invocations; ordinary v3 stages own complete-chunk acceptance.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::audio_inspection::process::{GroupPolicy, run_tool_isolated_with_limits};
use crate::frame_io::{copy_pinned, identity, inspect_png, inventory};
use crate::provider::{canonical_json_bytes, canonical_sha256, require_sha256};
use crate::video_interpolation::{RifeFrame, RIFE_MAXIMUM_INDEX_BYTES};
use crate::{ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationRequest, Result, StreamRole};
use super::provider::{ComponentObservations, configuration, evidence_port, io_failure, make_executable, named, output_parent, png_port, port, read_document, refused, single, storage, write_new};
use super::*;

const CHUNK_FRAMES_TYPE: &str = "application/vnd.aniflow.upscayl-chunk-frames";
const CHUNK_REPORT_TYPE: &str = "application/vnd.aniflow.upscayl-chunk-report+json";

fn private_filename(ordinal: u64) -> String { format!("{ordinal:06}.png") }

fn preserve_failure(root: &Path, chunk: u64, phase: &str, error: &Error) {
    let document = serde_json::json!({"schema":"aniflow.upscayl-private-chunk-failure/v1",
        "chunk_ordinal":chunk,"phase":phase,"category":error.category(),"message":error.message(),
        "cause":"unproven","accepted":false});
    if let Ok(bytes) = canonical_json_bytes(&document) { let _ = write_new(&root.join("failure.json"), &bytes); }
}

fn private_membership(root: &Path, names: &[OsString], param: &str, weights: &str) -> Result<()> {
    let expected: BTreeSet<OsString> = ["processor","ffmpeg","input","output","models"].into_iter().map(Into::into).collect();
    let mut observed = BTreeSet::new();
    for entry in std::fs::read_dir(root).map_err(|_| refused("cannot inspect private upscayl chunk"))? {
        let entry = entry.map_err(|_| refused("cannot inspect private chunk entry"))?;
        let name = entry.file_name();
        let kind = entry.file_type().map_err(|_| refused("cannot inspect private chunk entry type"))?;
        let directory = matches!(name.to_str(),Some("input" | "output" | "models"));
        if !expected.contains(&name) || !observed.insert(name)
            || (directory && !kind.is_dir()) || (!directory && !kind.is_file()) {
            return Err(refused("upscayl chunk created an undeclared or special private entry"));
        }
    }
    if observed != expected { return Err(refused("private chunk membership is incomplete")); }
    inventory(&root.join("input"),names)?;
    inventory(&root.join("output"),names)?;
    inventory(&root.join("models"),&[param.into(),weights.into()])
}

/// A directory invocation produces one candidate chunk, never partial checkpoints.
pub fn execute_chunk_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation: UpscaylChunkOperation = configuration(request)?;
    operation.validate()?;
    if request.configuration != operation.provider_configuration()?
        || request.stage_id != format!("upscale_chunk_{:06}",operation.chunk_ordinal)
        || request.inputs.len() != operation.frames.len() || request.outputs.len() != 2 {
        return Err(refused("upscayl chunk invocation differs from its exact operation"));
    }
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency,"upscayl chunk recovery requires Unix process-group cancellation")); }
    let output = single(&request.outputs,"frames")?;
    let evidence = single(&request.outputs,"evidence")?;
    port(output,ArtifactKind::Directory,ArtifactRole::Intermediate,StreamRole::Video,CHUNK_FRAMES_TYPE)?;
    evidence_port(evidence,CHUNK_REPORT_TYPE)?;
    if output.artifact_id != format!("chunk_frames_{:06}",operation.chunk_ordinal)
        || evidence.artifact_id != format!("chunk_evidence_{:06}",operation.chunk_ordinal) {
        return Err(refused("chunk output IDs differ from their ordinal"));
    }
    let config = &operation.settings;
    let first = &operation.frames[0];
    let dimensions = config.output_dimensions(first.input_width,first.input_height)?;
    let components = ComponentObservations::observe(config)?;
    let mut paths: Vec<&Path> = request.inputs.iter().map(|input|input.path.as_path()).collect();
    paths.extend([config.upscayl.executable.as_path(),config.import.ffmpeg.executable.as_path(),config.model.param.path.as_path(),config.model.weights.path.as_path()]);
    let parent = output_parent(&[&output.path,&evidence.path],&paths)?;
    let source_bytes = operation.frames.iter().try_fold(0_u64,|total,frame| total.checked_add(frame.input_byte_count).ok_or_else(|| refused("chunk source size overflow")))?;
    let component_bytes = components.bytes()?;
    let required = (operation.frames.len() as u64).checked_mul(config.maximum_output_frame_bytes)
        .and_then(|bytes| bytes.checked_mul(2)).and_then(|bytes| bytes.checked_add(source_bytes))
        .and_then(|bytes| bytes.checked_add(component_bytes))
        .and_then(|bytes| bytes.checked_add(UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES))
        .ok_or_else(|| refused("chunk storage estimate overflow"))?;
    storage(parent,config,required)?;
    let private = tempfile::Builder::new().prefix(".upscayl-chunk-").tempdir_in(parent)
        .map_err(|_| io_failure("cannot reserve owned upscayl chunk work"))?;
    let root = private.path().canonicalize().map_err(|_| refused("cannot resolve private chunk work"))?;
    let _retained = private.keep();
    let result = execute_private_chunk(request,&operation,&root,&components,dimensions);
    if let Err(error) = &result { preserve_failure(&root,operation.chunk_ordinal,"chunk",error); }
    result
}

fn execute_private_chunk(request: &ProviderInvocationRequest, operation: &UpscaylChunkOperation,
    root: &Path, components: &ComponentObservations, dimensions: (u32,u32)) -> Result<()> {
    let config = &operation.settings;
    let (width,height) = dimensions;
    let output = single(&request.outputs,"frames")?;
    let evidence = single(&request.outputs,"evidence")?;
    let cancellation = CancellationToken::default();
    let models = root.join("models");
    let input_directory = root.join("input");
    let output_directory = root.join("output");
    for directory in [&models,&input_directory,&output_directory] {
        std::fs::create_dir(directory).map_err(|_| io_failure("cannot create a private chunk directory"))?;
    }
    let (param_name,weights_name) = config.model_filenames()?;
    let param = models.join(&param_name);
    let weights = models.join(&weights_name);
    let mut processor = config.upscayl.clone();
    let mut decoder = config.import.ffmpeg.clone();
    processor.executable = root.join("processor");
    decoder.executable = root.join("ffmpeg");
    for (original,copy,observed,maximum) in [
        (&config.upscayl.executable,&processor.executable,&components.processor,MAXIMUM_TOOL_BYTES),
        (&config.import.ffmpeg.executable,&decoder.executable,&components.decoder,MAXIMUM_TOOL_BYTES),
        (&config.model.param.path,&param,&components.param,MAXIMUM_MODEL_BYTES),
        (&config.model.weights.path,&weights,&components.weights,MAXIMUM_MODEL_BYTES),
    ] { copy_pinned(original,copy,observed,maximum,&cancellation)?; }
    make_executable(&processor.executable)?;
    make_executable(&decoder.executable)?;
    let limits = config.import.tool_limits();
    crate::frame_decode::verify_version(&decoder,&limits,&cancellation,root)?;
    let names: Vec<OsString> = operation.frames.iter().map(|frame| private_filename(frame.ordinal).into()).collect();
    let mut decoded_inputs = Vec::with_capacity(operation.frames.len());
    for frame in &operation.frames {
        let input = named(&request.inputs,"frames",&format!("source_frame_{:06}",frame.ordinal))?;
        png_port(input)?;
        let observed = inspect_png(&input.path,frame.input_width,frame.input_height,config.import.maximum_frame_bytes)?;
        if observed != (frame.input_sha256.clone(),frame.input_byte_count) {
            return Err(refused("chunk source frame differs from its exact operation"));
        }
        let snapshot = input_directory.join(private_filename(frame.ordinal));
        copy_pinned(&input.path,&snapshot,&observed,config.import.maximum_frame_bytes,&cancellation)?;
        decoded_inputs.push(crate::frame_decode::decode(&decoder,&limits,&cancellation,root,&snapshot,frame.input_width,frame.input_height)?);
    }
    inventory(&input_directory,&names)?;
    inventory(&output_directory,&[])?;
    let mut arguments: Vec<OsString> = [
        "-i".to_owned(),"input".to_owned(),"-o".to_owned(),"output".to_owned(),
        "-m".to_owned(),"models".to_owned(),"-n".to_owned(),config.model.name.clone(),
        "-z".to_owned(),config.model.native_scale.to_string(),"-s".to_owned(),config.output_scale.to_string(),
        "-t".to_owned(),config.tile_size.to_string(),"-g".to_owned(),config.gpu_id.to_string(),
        "-j".to_owned(),config.threads.clone(),"-f".to_owned(),"png".to_owned(),
    ].into_iter().map(Into::into).collect();
    if config.tta { arguments.push("-x".into()); }
    let native_timeout = config.execution.chunk_timeout_milliseconds()
        .ok_or_else(|| refused("native chunk timeout requires bounded-chunk policy"))?;
    let native_capture_bytes = config.execution.chunk_maximum_tool_output_bytes()
        .ok_or_else(|| refused("native chunk capture bound requires bounded-chunk policy"))?;
    let started = Instant::now();
    run_tool_isolated_with_limits(&processor,"upscayl",&arguments,&limits,&cancellation,GroupPolicy::Inherit,Some(root),native_timeout,native_capture_bytes)
        .map_err(|diagnostic| {
            let detail = serde_json::json!({"schema":"aniflow.upscayl-private-chunk-failure/v1",
                "chunk_ordinal":operation.chunk_ordinal,"phase":"processor","diagnostic":diagnostic,
                "cause":"unproven","accepted":false});
            if let Ok(bytes) = canonical_json_bytes(&detail) { let _ = write_new(&root.join("failure.json"),&bytes); }
            Error::new(ErrorCategory::Execution,format!("upscayl chunk {} process failed ({:?}): {}; this whole chunk requires an explicit retry",operation.chunk_ordinal,diagnostic.code,diagnostic.message))
        })?;
    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    private_membership(root,&names,&param_name,&weights_name)?;
    let mut frames = Vec::with_capacity(operation.frames.len());
    for (frame,input_decoded) in operation.frames.iter().zip(decoded_inputs) {
        let snapshot = input_directory.join(private_filename(frame.ordinal));
        if identity(&snapshot,config.import.maximum_frame_bytes)? != (frame.input_sha256.clone(),frame.input_byte_count) {
            return Err(refused("upscayl changed a private chunk input"));
        }
        let candidate = output_directory.join(private_filename(frame.ordinal));
        let observed = inspect_png(&candidate,width,height,config.maximum_output_frame_bytes)?;
        let output_decoded = crate::frame_decode::decode(&decoder,&limits,&cancellation,root,&candidate,width,height)?;
        if inspect_png(&candidate,width,height,config.maximum_output_frame_bytes)? != observed {
            return Err(refused("chunk output changed during full decode"));
        }
        frames.push(UpscaylChunkFrameReport { frame:frame.clone(),output_sha256:observed.0,output_byte_count:observed.1,
            input_decoded_rgb24_sha256:input_decoded,output_decoded_rgb24_sha256:output_decoded });
    }
    private_membership(root,&names,&param_name,&weights_name)?;
    components.verify(config)?;
    for (path,expected,maximum) in [
        (&processor.executable,&components.processor,MAXIMUM_TOOL_BYTES),
        (&decoder.executable,&components.decoder,MAXIMUM_TOOL_BYTES),
        (&param,&components.param,MAXIMUM_MODEL_BYTES),(&weights,&components.weights,MAXIMUM_MODEL_BYTES),
    ] { if identity(path,maximum)? != *expected { return Err(refused("private chunk tool or model changed")); } }
    for frame in &operation.frames {
        let input = named(&request.inputs,"frames",&format!("source_frame_{:06}",frame.ordinal))?;
        let expected = (frame.input_sha256.clone(),frame.input_byte_count);
        if identity(&input.path,config.import.maximum_frame_bytes)? != expected
            || identity(&input_directory.join(private_filename(frame.ordinal)),config.import.maximum_frame_bytes)? != expected {
            return Err(refused("chunk source changed before candidate publication"));
        }
    }
    // Only a complete observed chunk is copied into the assigned candidate.
    std::fs::create_dir(&output.path).map_err(|_| io_failure("cannot exclusively create chunk frame candidate"))?;
    for report in &frames {
        copy_pinned(&output_directory.join(private_filename(report.frame.ordinal)),&output.path.join(&report.frame.filename),
            &(report.output_sha256.clone(),report.output_byte_count),config.maximum_output_frame_bytes,&cancellation)?;
    }
    let published_names: Vec<OsString> = frames.iter().map(|report|report.frame.filename.clone().into()).collect();
    inventory(&output.path,&published_names)?;
    let report = UpscaylChunkReport { schema:UPSCAYL_CHUNK_REPORT_SCHEMA_V1.to_owned(),stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(),configuration_sha256:request.configuration.effective_configuration_sha256.clone(),
        chunk_ordinal:operation.chunk_ordinal,upscayl_sha256:config.upscayl.sha256.clone(),decoder_sha256:config.import.ffmpeg.sha256.clone(),
        model_param_sha256:config.model.param.sha256.clone(),model_weights_sha256:config.model.weights.sha256.clone(),
        native_scale:config.model.native_scale,output_scale:config.output_scale,output_width:width,output_height:height,
        processor_elapsed_milliseconds:elapsed,backend_qualification:"unqualified".to_owned(),frames };
    let bytes = canonical_json_bytes(&report)?;
    if bytes.len() as u64 > UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES { return Err(refused("chunk report exceeds its bounded size")); }
    write_new(&evidence.path,&bytes)?;
    std::fs::remove_dir_all(root).map_err(|_| io_failure("cannot remove successful owned chunk scratch"))
}

/// Collect exact same-run chunk dependencies; standalone files cannot grant reuse.
pub fn execute_chunk_collect_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation: UpscaylChunkCollectOperation = configuration(request)?;
    operation.validate()?;
    let chunk_size = u64::from(operation.settings.execution.frames_per_chunk()
        .ok_or_else(|| refused("chunk collection requires bounded-chunk policy"))?);
    let chunk_count = operation.frame_count.div_ceil(chunk_size);
    if request.configuration != operation.provider_configuration()? || request.stage_id != "collect_upscaled_chunks"
        || request.outputs.len() != 3 || request.inputs.len() as u64 != chunk_count * 2 + 1 {
        return Err(refused("chunk collection differs from its complete operation"));
    }
    let index_input = single(&request.inputs,"index")?;
    evidence_port(index_input,"application/vnd.aniflow.rife-frame-index+json")?;
    let output = single(&request.outputs,"frames")?;
    let index_output = single(&request.outputs,"index")?;
    let evidence_output = single(&request.outputs,"evidence")?;
    port(output,ArtifactKind::Directory,ArtifactRole::Intermediate,StreamRole::Video,"application/vnd.aniflow.upscayl-frames")?;
    evidence_port(index_output,"application/vnd.aniflow.upscayl-output-index+json")?;
    evidence_port(evidence_output,"application/vnd.aniflow.upscayl-chunk-recovery-report+json")?;
    let (source_index,index_identity) = crate::frame_import::read_index(&index_input.path)?;
    if source_index.frames.len() as u64 != operation.frame_count || canonical_sha256(&source_index)? != operation.frame_index_sha256 {
        return Err(refused("chunk collection source index differs from its complete planned mapping"));
    }
    let config = &operation.settings;
    let dimensions = config.output_dimensions(source_index.width,source_index.height)?;
    let timeline = source_index.temporal_inspection.video_timeline().map_err(|error|Error::from_anyhow(ErrorCategory::Media,error))?;
    let components = ComponentObservations::observe(config)?;
    let mut paths: Vec<&Path> = request.inputs.iter().map(|binding|binding.path.as_path()).collect();
    paths.extend([config.upscayl.executable.as_path(),config.import.ffmpeg.executable.as_path(),config.model.param.path.as_path(),config.model.weights.path.as_path()]);
    let parent = output_parent(&[&output.path,&index_output.path,&evidence_output.path],&paths)?;
    let required = operation.frame_count.checked_mul(config.maximum_output_frame_bytes).and_then(|bytes|bytes.checked_mul(2))
        .and_then(|bytes|bytes.checked_add(2 * UPSCAYL_MAXIMUM_REPORT_BYTES)).ok_or_else(||refused("chunk collection storage estimate overflow"))?;
    storage(parent,config,required)?;
    std::fs::create_dir(&output.path).map_err(|_|io_failure("cannot exclusively create collected chunk frame view"))?;
    let cancellation = CancellationToken::default();
    let mut output_index = UpscaylOutputIndex { schema:UPSCAYL_OUTPUT_INDEX_SCHEMA_V1.to_owned(),source_index:source_index.clone(),
        output_width:dimensions.0,output_height:dimensions.1,output_profile:source_index.frame_profile.clone(),frames:Vec::with_capacity(source_index.frames.len()) };
    let mut chunks = Vec::with_capacity(chunk_count as usize);
    let mut dependencies: Vec<(PathBuf,(String,u64),u64)> = Vec::with_capacity(source_index.frames.len()+chunk_count as usize);
    let mut directories: Vec<(PathBuf,Vec<OsString>)> = Vec::with_capacity(chunk_count as usize);
    for chunk_ordinal in 0..chunk_count {
        let images = named(&request.inputs,"frames",&format!("chunk_frames_{chunk_ordinal:06}"))?;
        let evidence = named(&request.inputs,"evidence",&format!("chunk_evidence_{chunk_ordinal:06}"))?;
        port(images,ArtifactKind::Directory,ArtifactRole::Intermediate,StreamRole::Video,CHUNK_FRAMES_TYPE)?;
        evidence_port(evidence,CHUNK_REPORT_TYPE)?;
        let (report,report_identity): (UpscaylChunkReport,_) = read_document(&evidence.path,UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES)?;
        let start = (chunk_ordinal * chunk_size) as usize;
        let end = ((chunk_ordinal + 1) * chunk_size).min(operation.frame_count) as usize;
        if report.frames.len() != end-start { return Err(refused("chunk evidence must contain its complete exact frame range")); }
        let mut expected_frames = Vec::with_capacity(end-start);
        for (ordinal,observation) in (start..end).zip(&report.frames) {
            let frame = &source_index.frames[ordinal];
            let expected = UpscaylFrameSpec { source_sha256:source_index.temporal_inspection.source_sha256.clone(),
                source_byte_count:source_index.temporal_inspection.source_size_bytes,frame_index_sha256:operation.frame_index_sha256.clone(),
                ordinal:frame.ordinal,filename:frame.filename.clone(),source_time:timeline.frames[ordinal].source_time,
                input_sha256:frame.sha256.clone(),input_byte_count:observation.frame.input_byte_count,input_width:source_index.width,input_height:source_index.height };
            if observation.frame != expected || observation.output_byte_count == 0 || observation.output_byte_count > config.maximum_output_frame_bytes {
                return Err(refused("chunk frame evidence differs from exact source timing or identity"));
            }
            for digest in [&observation.output_sha256,&observation.input_decoded_rgb24_sha256,&observation.output_decoded_rgb24_sha256] {
                require_sha256(digest,"chunk frame observation SHA-256")?;
            }
            expected_frames.push(expected);
        }
        let expected = UpscaylChunkOperation { schema:UPSCAYL_CHUNK_OPERATION_SCHEMA_V1.to_owned(),settings:config.clone(),chunk_ordinal,frames:expected_frames };
        expected.validate()?;
        require_sha256(&report.provider_lock_sha256,"chunk provider lock SHA-256")?;
        if report.schema != UPSCAYL_CHUNK_REPORT_SCHEMA_V1 || report.stage_id != format!("upscale_chunk_{chunk_ordinal:06}")
            || report.chunk_ordinal != chunk_ordinal || report.configuration_sha256 != expected.provider_configuration()?.effective_configuration_sha256
            || report.upscayl_sha256 != config.upscayl.sha256 || report.decoder_sha256 != config.import.ffmpeg.sha256
            || report.model_param_sha256 != config.model.param.sha256 || report.model_weights_sha256 != config.model.weights.sha256
            || report.native_scale != config.model.native_scale || report.output_scale != config.output_scale
            || (report.output_width,report.output_height) != dimensions || report.backend_qualification != "unqualified" {
            return Err(refused("chunk evidence differs from its exact locked settings and provider operation"));
        }
        let names: Vec<OsString> = report.frames.iter().map(|frame|frame.frame.filename.clone().into()).collect();
        inventory(&images.path,&names)?;
        for observation in &report.frames {
            let image = images.path.join(&observation.frame.filename);
            let observed = inspect_png(&image,dimensions.0,dimensions.1,config.maximum_output_frame_bytes)?;
            if observed != (observation.output_sha256.clone(),observation.output_byte_count) {
                return Err(refused("accepted chunk frame differs from its full-decode observation"));
            }
            copy_pinned(&image,&output.path.join(&observation.frame.filename),&observed,config.maximum_output_frame_bytes,&cancellation)?;
            dependencies.push((image,observed,config.maximum_output_frame_bytes));
            output_index.frames.push(RifeFrame { ordinal:observation.frame.ordinal,filename:observation.frame.filename.clone(),sha256:observation.output_sha256.clone() });
        }
        dependencies.push((evidence.path.clone(),report_identity,UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES));
        directories.push((images.path.clone(),names));
        chunks.push(report);
    }
    let names: Vec<OsString> = source_index.frames.iter().map(|frame|frame.filename.clone().into()).collect();
    inventory(&output.path,&names)?;
    for chunk in &chunks {
        for frame in &chunk.frames {
            if identity(&output.path.join(&frame.frame.filename),config.maximum_output_frame_bytes)? != (frame.output_sha256.clone(),frame.output_byte_count) {
                return Err(refused("collected chunk output changed before completion"));
            }
        }
    }
    for (path,observed,maximum) in dependencies {
        if identity(&path,maximum)? != observed { return Err(refused("chunk dependency changed during collection")); }
    }
    for (directory,names) in directories { inventory(&directory,&names)?; }
    if identity(&index_input.path,RIFE_MAXIMUM_INDEX_BYTES)? != index_identity { return Err(refused("source index changed during chunk collection")); }
    components.verify(config)?;
    output_index.validate()?;
    let report = UpscaylChunkRecoveryReport { schema:UPSCAYL_CHUNK_RECOVERY_REPORT_SCHEMA_V1.to_owned(),stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(),configuration_sha256:request.configuration.effective_configuration_sha256.clone(),
        source_relationship:"declared_not_verified".to_owned(),historical_processing:"unknown".to_owned(),backend_qualification:"unqualified".to_owned(),output_index,chunks };
    let index_bytes = canonical_json_bytes(&report.output_index)?;
    let report_bytes = canonical_json_bytes(&report)?;
    if index_bytes.len() as u64 > UPSCAYL_MAXIMUM_REPORT_BYTES || report_bytes.len() as u64 > UPSCAYL_MAXIMUM_REPORT_BYTES {
        return Err(refused("complete chunk index/report exceeds its bounded size"));
    }
    write_new(&index_output.path,&index_bytes)?;
    write_new(&evidence_output.path,&report_bytes)
}
