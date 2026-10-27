use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;
use crate::audio_inspection::process::{GroupPolicy, open_regular, run_tool_isolated, verify_pin};
use crate::frame_import::{FrameImportReport, FRAME_IMPORT_MAXIMUM_REPORT_BYTES, FRAME_IMPORT_REPORT_SCHEMA_V1};
use crate::frame_io::{copy_pinned, ensure_parent, identity, inspect_png, inventory, require_absent};
use crate::provider::{canonical_json_bytes, canonical_sha256, require_sha256};
use crate::{ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationArtifactBinding, ProviderInvocationRequest, Result, StreamRole};
use super::*;

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }

fn binding<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut values = bindings.iter().filter(|binding| binding.port == port);
    let value = values.next().ok_or_else(|| refused("missing selective repair port"))?;
    if values.next().is_some() { return Err(refused("duplicate selective repair port")); }
    Ok(value)
}

fn operation(request: &ProviderInvocationRequest) -> Result<FrameRepairOperation> {
    request.validate()?;
    let operation: FrameRepairOperation = serde_json::from_value(serde_json::to_value(&request.configuration.values)
        .map_err(|_| refused("cannot read repair configuration"))?).map_err(|_| refused("invalid closed repair operation"))?;
    operation.validate()?;
    if operation.provider_configuration()? != request.configuration || request.inputs.len() != 3 || request.outputs.len() != 3 {
        return Err(refused("repair requires its exact effective operation and three input/output ports"));
    }
    for (bindings, port, kind, role, stream, artifact_type) in [
        (&request.inputs,"frames",ArtifactKind::Directory,ArtifactRole::Intermediate,StreamRole::Video,"application/vnd.aniflow.rife-frames"),
        (&request.inputs,"import",ArtifactKind::File,ArtifactRole::ValidationEvidence,StreamRole::TimedMetadata,"application/vnd.aniflow.frame-import-report+json"),
        (&request.inputs,"index",ArtifactKind::File,ArtifactRole::ValidationEvidence,StreamRole::TimedMetadata,"application/vnd.aniflow.rife-frame-index+json"),
        (&request.outputs,"frames",ArtifactKind::Directory,ArtifactRole::Intermediate,StreamRole::Video,"application/vnd.aniflow.rife-frames"),
        (&request.outputs,"index",ArtifactKind::File,ArtifactRole::ValidationEvidence,StreamRole::TimedMetadata,"application/vnd.aniflow.rife-frame-index+json"),
        (&request.outputs,"evidence",ArtifactKind::File,ArtifactRole::ValidationEvidence,StreamRole::TimedMetadata,"application/vnd.aniflow.frame-repair-report+json"),
    ] {
        let value = binding(bindings, port)?;
        if value.kind != kind || value.artifact_role != role || value.stream_role != Some(stream) || value.artifact_type != artifact_type {
            return Err(refused("selective repair artifact port differs from the fixed profile"));
        }
    }
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency,"selective repair requires Unix process-group cancellation")); }
    Ok(operation)
}

fn read_import(path: &Path) -> Result<(FrameImportReport, (String,u64))> {
    let observed = identity(path, FRAME_IMPORT_MAXIMUM_REPORT_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(path).map_err(|_| refused("cannot open imported-frame evidence"))?
        .take(FRAME_IMPORT_MAXIMUM_REPORT_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_| refused("cannot read imported-frame evidence"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(refused("import evidence changed while reading")); }
    let report = serde_json::from_slice(&bytes).map_err(|_| refused("invalid closed import evidence"))?;
    Ok((report,observed))
}

fn verify_import(report: &FrameImportReport, index: &crate::video_interpolation::RifeFrameIndex, config: &FrameRepairConfiguration, decoded_bytes: u64) -> Result<()> {
    require_sha256(&report.provider_lock_sha256,"import provider lock")?;
    if report.schema != FRAME_IMPORT_REPORT_SCHEMA_V1 || report.stage_id != "import_frames"
        || report.index != *index || report.frame_index_sha256 != canonical_sha256(index)?
        || report.source_sha256 != index.temporal_inspection.source_sha256
        || report.source_byte_count == 0 || report.source_byte_count != index.temporal_inspection.source_size_bytes
        || report.configuration_sha256 != config.import.provider_configuration()?.effective_configuration_sha256
        || report.historical_processing != "unknown" || report.source_relationship != "declared_not_verified"
        || report.decode_method != "ffmpeg_png_rgb24_framehash_sha256"
        || report.decoder.id != "ffmpeg" || report.decoder.sha256 != config.import.ffmpeg.sha256
        || report.decoder.version != config.import.ffmpeg.version || report.frames.len() != index.frames.len()
    { return Err(refused("import evidence differs from the bound source/index/decoder profile")); }
    for (observed, declared) in report.frames.iter().zip(&index.frames) {
        require_sha256(&observed.decoded_rgb24_sha256,"import decoded RGB identity")?;
        if observed.ordinal != declared.ordinal || observed.filename != declared.filename || observed.sha256 != declared.sha256
            || observed.byte_count == 0 || observed.byte_count > config.import.maximum_frame_bytes || observed.decoded_byte_count != decoded_bytes
        { return Err(refused("import observations do not completely map to source frames")); }
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path).map_err(|_| refused("cannot exclusively create repair evidence"))?;
    file.write_all(bytes).and_then(|()| file.sync_all()).map_err(|_| refused("cannot synchronize repair evidence"))
}

/// Native ABI only. The existing v3 run validates the predecessor checkpoint,
/// supplies its accepted artifacts and owns current stage acceptance. Calling
/// this function directly never establishes an accepted prior or current stage.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation = operation(request)?;
    let config = &operation.settings;
    let input_frames = &binding(&request.inputs,"frames")?.path;
    let import_path = &binding(&request.inputs,"import")?.path;
    let index_path = &binding(&request.inputs,"index")?.path;
    let output_frames = &binding(&request.outputs,"frames")?.path;
    let output_index = &binding(&request.outputs,"index")?.path;
    let output_evidence = &binding(&request.outputs,"evidence")?.path;
    let (index,index_identity) = frame_import::read_index(index_path)?;
    let decoded_bytes = frame_import::admit_geometry(&config.import,&index)?;
    let selection = plan_from_rife_index(&index,&operation.ranges)?;
    let (import,import_identity) = read_import(import_path)?;
    verify_import(&import,&index,config,decoded_bytes)?;
    let names: Vec<OsString> = index.frames.iter().map(|frame| frame.filename.clone().into()).collect();
    inventory(input_frames,&names)?;
    let mut total = 0_u64;
    let mut unselected_bytes = 0_u64;
    let mut input_identities = Vec::with_capacity(index.frames.len());
    for (frame,imported) in index.frames.iter().zip(&import.frames) {
        let observed = inspect_png(&input_frames.join(&frame.filename),index.width,index.height,config.import.maximum_frame_bytes)?;
        if observed != (frame.sha256.clone(),imported.byte_count) { return Err(refused("imported frame bytes differ from accepted import observations")); }
        total = total.checked_add(observed.1).filter(|value| *value <= config.import.maximum_total_frame_bytes)
            .ok_or_else(|| refused("imported frames exceed the aggregate bound"))?;
        if !selected(&selection,frame.ordinal) { unselected_bytes += observed.1; }
        input_identities.push(observed);
    }
    let processor_identity = identity(&config.processor.tool.executable,MAXIMUM_TOOL_BYTES)?;
    let decoder_identity = identity(&config.import.ffmpeg.executable,MAXIMUM_TOOL_BYTES)?;
    for (pin,id) in [(&config.processor.tool,"direct-image-processor"),(&config.import.ffmpeg,"ffmpeg")] {
        verify_pin(pin,id).map_err(|_| Error::new(ErrorCategory::Dependency,"repair tool pin changed"))?;
    }
    admit_working_bytes(config,import.source_byte_count,total,unselected_bytes,&selection,processor_identity.1+decoder_identity.1)?;
    let parent = output_frames.parent().ok_or_else(|| refused("repair outputs require an assigned parent"))?;
    if output_index.parent()!=Some(parent) || output_evidence.parent()!=Some(parent)
        || output_index==output_evidence || output_frames==output_index || output_frames==output_evidence
    { return Err(refused("repair outputs must be three distinct siblings")); }
    for output in [output_frames,output_index,output_evidence] {
        for input in [input_frames,import_path,index_path,&config.processor.tool.executable,&config.import.ffmpeg.executable] {
            if output.starts_with(input) || input.starts_with(output) { return Err(refused("repair outputs must be separate from every input")); }
        }
        require_absent(output)?;
    }
    ensure_parent(parent)?;
    std::fs::create_dir(output_frames).map_err(|_| refused("cannot exclusively create repaired-frame view"))?;
    // Retain failed candidate work, including an invalid selected result, for the
    // outer runtime's failure record. Only successful owned scratch is removed.
    let private = tempfile::Builder::new().prefix(".frame-repair-").tempdir_in(parent)
        .map_err(|_| refused("cannot reserve private repair work"))?;
    let private_root = private.path().canonicalize().map_err(|_| refused("cannot resolve private repair work"))?;
    let _retained = private.keep();
    let cancellation = CancellationToken::default();
    let mut processor = config.processor.tool.clone();
    let mut decoder = config.import.ffmpeg.clone();
    processor.executable = private_root.join("processor");
    decoder.executable = private_root.join("ffmpeg");
    for (original,copy,observed) in [(&config.processor.tool,&processor,&processor_identity),(&config.import.ffmpeg,&decoder,&decoder_identity)] {
        copy_pinned(&original.executable,&copy.executable,observed,MAXIMUM_TOOL_BYTES,&cancellation)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&copy.executable,std::fs::Permissions::from_mode(0o700))
                .map_err(|_| refused("cannot make a private repair tool executable"))?;
        }
    }
    let limits = config.import.tool_limits();
    crate::frame_decode::verify_version(&decoder,&limits,&cancellation,&private_root)?;
    let timeline = index.temporal_inspection.video_timeline().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let mut report = FrameRepairReport {
        schema:FRAME_REPAIR_REPORT_SCHEMA_V1.to_owned(),stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(),configuration_sha256:request.configuration.effective_configuration_sha256.clone(),
        import_report_sha256:import_identity.0.clone(),source_relationship:"declared_not_verified".to_owned(),historical_processing:"unknown".to_owned(),
        processor_profile:config.processor.profile.clone(),processor_sha256:processor.sha256.clone(),processor_declared_version:processor.version.clone(),
        model_environment_evidence:"unobserved".to_owned(),decoder_sha256:decoder.sha256.clone(),selection:selection.clone(),changed_frame_count:0,
        output_index:index.clone(),frames:Vec::with_capacity(index.frames.len()),
    };
    let mut output_bytes = 0_u64;
    for ((frame,imported),observed) in index.frames.iter().zip(&import.frames).zip(&input_identities) {
        let chosen = selected(&selection,frame.ordinal);
        let destination = output_frames.join(&frame.filename);
        let (output_identity,decoded) = if chosen {
            let pair = private_root.join("pair");
            std::fs::create_dir(&pair).map_err(|_| refused("cannot exclusively create one-frame repair work"))?;
            let snapshot = pair.join("input.png");
            let edited = pair.join("output.png");
            copy_pinned(&input_frames.join(&frame.filename),&snapshot,observed,config.import.maximum_frame_bytes,&cancellation)?;
            let arguments: Vec<OsString> = config.processor.arguments.iter().map(|arg| match arg.as_str() {
                "{input}" => snapshot.as_os_str().to_owned(), "{output}" => edited.as_os_str().to_owned(), _ => arg.into(),
            }).collect();
            run_tool_isolated(&processor,"direct-image-processor",&arguments,&limits,&cancellation,GroupPolicy::Inherit,Some(&pair))
                .map_err(|diagnostic| refused(format!("selected frame {} process failed ({:?}): {}",frame.ordinal,diagnostic.code,diagnostic.message)))?;
            inventory(&pair,&["input.png".into(),"output.png".into()])?;
            if identity(&snapshot,config.import.maximum_frame_bytes)? != *observed { return Err(refused("processor changed an immutable input snapshot")); }
            let result = inspect_png(&edited,index.width,index.height,config.import.maximum_frame_bytes)?;
            let decoded = crate::frame_decode::decode(&decoder,&limits,&cancellation,&private_root,&edited,index.width,index.height)
                .map_err(|error| refused(format!("selected frame {}: {error}",frame.ordinal)))?;
            if inspect_png(&edited,index.width,index.height,config.import.maximum_frame_bytes)? != result {
                return Err(refused("selected output changed during decode"));
            }
            inventory(&pair,&["input.png".into(),"output.png".into()])?;
            copy_pinned(&edited,&destination,&result,config.import.maximum_frame_bytes,&cancellation)?;
            std::fs::remove_dir_all(&pair).map_err(|_| refused("cannot remove successfully observed owned repair scratch"))?;
            (result,decoded)
        } else {
            copy_pinned(&input_frames.join(&frame.filename),&destination,observed,config.import.maximum_frame_bytes,&cancellation)?;
            (observed.clone(),imported.decoded_rgb24_sha256.clone())
        };
        output_bytes = output_bytes.checked_add(output_identity.1).filter(|value| *value <= config.import.maximum_total_frame_bytes)
            .ok_or_else(|| refused("repaired output exceeds the configured aggregate bound"))?;
        let changed = output_identity.0 != observed.0;
        if !chosen && changed { return Err(refused("an unselected frame changed")); }
        if changed { report.changed_frame_count += 1; }
        let ordinal = usize::try_from(frame.ordinal).map_err(|_| refused("frame ordinal exceeds host addressability"))?;
        report.output_index.frames[ordinal].sha256 = output_identity.0.clone();
        report.frames.push(FrameRepairFrameObservation {
            ordinal:frame.ordinal,filename:frame.filename.clone(),source_time:timeline.frames[ordinal].source_time,
            selected:chosen,bytes_changed:changed,input_sha256:observed.0.clone(),output_sha256:output_identity.0,
            output_byte_count:output_identity.1,decoded_rgb24_sha256:decoded,
        });
    }
    inventory(input_frames,&names)?;
    inventory(output_frames,&names)?;
    inventory(&private_root,&["processor".into(),"ffmpeg".into()])?;
    for ((frame,original),output) in index.frames.iter().zip(&input_identities).zip(&report.frames) {
        if identity(&input_frames.join(&frame.filename),config.import.maximum_frame_bytes)? != *original
            || identity(&output_frames.join(&frame.filename),config.import.maximum_frame_bytes)? != (output.output_sha256.clone(),output.output_byte_count)
        { return Err(refused("source or repaired frame changed before completion")); }
    }
    if identity(index_path,crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES)? != index_identity
        || identity(import_path,FRAME_IMPORT_MAXIMUM_REPORT_BYTES)? != import_identity
        || identity(&config.processor.tool.executable,MAXIMUM_TOOL_BYTES)? != processor_identity
        || identity(&processor.executable,MAXIMUM_TOOL_BYTES)? != processor_identity
        || identity(&config.import.ffmpeg.executable,MAXIMUM_TOOL_BYTES)? != decoder_identity
        || identity(&decoder.executable,MAXIMUM_TOOL_BYTES)? != decoder_identity
    { return Err(refused("import/index/processor/decoder identity changed during repair")); }
    report.output_index.validate()?;
    let index_bytes = canonical_json_bytes(&report.output_index)?;
    let evidence = canonical_json_bytes(&report)?;
    if index_bytes.len() as u64 > crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES || evidence.len() as u64 > FRAME_REPAIR_MAXIMUM_REPORT_BYTES {
        return Err(refused("repair evidence or output index exceeds its bounded size"));
    }
    std::fs::remove_dir_all(&private_root).map_err(|_| refused("cannot remove successful private repair tool copies"))?;
    write_new(output_index,&index_bytes)?;
    write_new(output_evidence,&evidence)
}
