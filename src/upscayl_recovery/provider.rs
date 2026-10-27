//! Native wrappers. Runtime dependencies and checkpoints own acceptance.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use crate::audio_inspection::process::{GroupPolicy, open_regular, run_tool_isolated, verify_pin};
use crate::frame_io::{copy_pinned, ensure_parent, identity, inspect_png, inventory, require_absent};
use crate::provider::{canonical_json_bytes, canonical_sha256, require_sha256};
use crate::video_interpolation::{RifeFrame, RIFE_MAXIMUM_INDEX_BYTES};
use crate::{ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationArtifactBinding, ProviderInvocationRequest, Result, StreamRole};
use super::*;

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }
fn io_failure(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Io, message) }

fn single<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut matches = bindings.iter().filter(|value| value.port == port);
    let value = matches.next().ok_or_else(|| refused(format!("missing upscayl {port} port")))?;
    if matches.next().is_some() { return Err(refused(format!("duplicate upscayl {port} port"))); }
    Ok(value)
}

fn named<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str, artifact_id: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut matches = bindings.iter().filter(|value| value.port == port && value.artifact_id == artifact_id);
    let value = matches.next().ok_or_else(|| refused("missing exact upscayl frame/evidence binding"))?;
    if matches.next().is_some() { return Err(refused("duplicate upscayl frame/evidence binding")); }
    Ok(value)
}

fn port(value: &ProviderInvocationArtifactBinding, kind: ArtifactKind, role: ArtifactRole, stream: StreamRole, artifact_type: &str) -> Result<()> {
    if value.kind != kind || value.artifact_role != role || value.stream_role != Some(stream) || value.artifact_type != artifact_type {
        return Err(refused("upscayl artifact port differs from the fixed profile"));
    }
    Ok(())
}

fn png_port(value: &ProviderInvocationArtifactBinding) -> Result<()> {
    port(value, ArtifactKind::File, ArtifactRole::Intermediate, StreamRole::Video, "image/png")
}

fn evidence_port(value: &ProviderInvocationArtifactBinding, artifact_type: &str) -> Result<()> {
    port(value, ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, artifact_type)
}

fn configuration<T: DeserializeOwned>(request: &ProviderInvocationRequest) -> Result<T> {
    request.validate()?;
    serde_json::from_value(serde_json::to_value(&request.configuration.values)
        .map_err(|_| refused("cannot read upscayl operation"))?)
        .map_err(|_| refused("invalid closed upscayl operation"))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)
        .map_err(|_| io_failure("cannot exclusively create upscayl evidence; inspect retained workspace and available storage"))?;
    file.write_all(bytes).and_then(|()| file.sync_all())
        .map_err(|_| io_failure("cannot synchronize upscayl evidence; inspect retained workspace and available storage"))
}

fn read_document<T: DeserializeOwned>(path: &Path, maximum: u64) -> Result<(T, (String, u64))> {
    let observed = identity(path, maximum)?;
    let mut bytes = Vec::new();
    open_regular(path).map_err(|_| refused("cannot safely open frame evidence"))?
        .take(maximum + 1).read_to_end(&mut bytes)
        .map_err(|_| io_failure("cannot read frame evidence"))?;
    if (format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64) != observed {
        return Err(refused("frame evidence changed while reading"));
    }
    let document = serde_json::from_slice(&bytes).map_err(|_| refused("invalid closed frame evidence"))?;
    Ok((document, observed))
}

fn storage(parent: &Path, config: &UpscaylRecoveryConfiguration, required: u64) -> Result<()> {
    let available = crate::cache_v3::available_storage(parent)?;
    if required.checked_add(config.minimum_free_bytes).is_none_or(|needed| needed > available) {
        return Err(Error::new(ErrorCategory::State, "insufficient available storage for the next bounded upscayl operation; free owned workspace space before explicitly resuming"));
    }
    Ok(())
}

/// Require distinct sibling outputs outside source artifacts and pinned tools.
fn output_parent<'a>(outputs: &[&'a Path], inputs: &[&Path]) -> Result<&'a Path> {
    let parent = outputs.first().and_then(|path| path.parent())
        .ok_or_else(|| refused("upscayl outputs require an assigned parent"))?;
    let mut unique = BTreeSet::new();
    for output in outputs {
        if output.parent() != Some(parent) || !unique.insert(*output) {
            return Err(refused("upscayl outputs must be distinct siblings"));
        }
        for input in inputs {
            if output.starts_with(input) || input.starts_with(output) {
                return Err(refused("upscayl outputs must remain separate from every input"));
            }
        }
        require_absent(output)?;
    }
    ensure_parent(parent)?;
    Ok(parent)
}

struct ComponentObservations {
    processor: (String, u64),
    decoder: (String, u64),
    param: (String, u64),
    weights: (String, u64),
}

impl ComponentObservations {
    fn observe(config: &UpscaylRecoveryConfiguration) -> Result<Self> {
        let observations = Self {
            processor: identity(&config.upscayl.executable, MAXIMUM_TOOL_BYTES)?,
            decoder: identity(&config.import.ffmpeg.executable, MAXIMUM_TOOL_BYTES)?,
            param: identity(&config.model.param.path, MAXIMUM_MODEL_BYTES)?,
            weights: identity(&config.model.weights.path, MAXIMUM_MODEL_BYTES)?,
        };
        if observations.processor.0 != config.upscayl.sha256
            || observations.decoder.0 != config.import.ffmpeg.sha256
            || observations.param.0 != config.model.param.sha256
            || observations.weights.0 != config.model.weights.sha256
            || observations.param.1 == 0 || observations.weights.1 == 0
        { return Err(Error::new(ErrorCategory::Dependency, "upscayl binary, decoder or model bytes differ from the configured pins")); }
        verify_pin(&config.upscayl, "upscayl").map_err(|_| Error::new(ErrorCategory::Dependency, "upscayl executable pin is unavailable or changed"))?;
        verify_pin(&config.import.ffmpeg, "ffmpeg").map_err(|_| Error::new(ErrorCategory::Dependency, "decoder executable pin is unavailable or changed"))?;
        Ok(observations)
    }

    fn bytes(&self) -> Result<u64> {
        [self.processor.1, self.decoder.1, self.param.1, self.weights.1].into_iter()
            .try_fold(0_u64, |total, bytes| total.checked_add(bytes).ok_or_else(|| refused("component byte count overflow")))
    }

    fn verify(&self, config: &UpscaylRecoveryConfiguration) -> Result<()> {
        for (path, maximum, expected) in [
            (&config.upscayl.executable, MAXIMUM_TOOL_BYTES, &self.processor),
            (&config.import.ffmpeg.executable, MAXIMUM_TOOL_BYTES, &self.decoder),
            (&config.model.param.path, MAXIMUM_MODEL_BYTES, &self.param),
            (&config.model.weights.path, MAXIMUM_MODEL_BYTES, &self.weights),
        ] {
            if identity(path, maximum)? != *expected { return Err(refused("upscayl component changed during execution")); }
        }
        Ok(())
    }
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| io_failure("cannot make the private tool executable"))?;
    }
    Ok(())
}

fn preserve_error(private: &Path, ordinal: u64, phase: &str, error: &Error) {
    let value = serde_json::json!({
        "schema":"aniflow.upscayl-private-failure/v1", "ordinal":ordinal,
        "phase":phase, "category":error.category(), "message":error.message(),
        "cause":"unproven", "accepted":false,
    });
    if let Ok(bytes) = canonical_json_bytes(&value) {
        let _ = write_new(&private.join("failure.json"), &bytes);
    }
}

fn verify_private_membership(root: &Path, param: &str, weights: &str) -> Result<()> {
    let expected: BTreeSet<OsString> = ["processor", "ffmpeg", "input.png", "output.png", "models"].into_iter().map(Into::into).collect();
    let mut observed = BTreeSet::new();
    for entry in std::fs::read_dir(root).map_err(|_| refused("cannot inspect private upscayl work"))? {
        let entry = entry.map_err(|_| refused("cannot inspect private upscayl entry"))?;
        let name = entry.file_name();
        let kind = entry.file_type().map_err(|_| refused("cannot inspect private upscayl entry type"))?;
        if !expected.contains(&name) || !observed.insert(name.clone())
            || (name == "models" && !kind.is_dir()) || (name != "models" && !kind.is_file())
        { return Err(refused("upscayl created an undeclared or special private artifact")); }
    }
    if observed != expected { return Err(refused("upscayl private output membership is incomplete")); }
    inventory(&root.join("models"), &[param.into(), weights.into()])
}

/// Execute one frame under the outer provider's process group. Success produces
/// candidates only; the existing v3 runtime validates and accepts the stage.
pub fn execute_frame_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation: UpscaylFrameOperation = configuration(request)?;
    operation.validate()?;
    if request.configuration != operation.provider_configuration()?
        || request.stage_id != format!("upscale_frame_{:06}", operation.frame.ordinal)
        || request.inputs.len() != 1 || request.outputs.len() != 2
    { return Err(refused("upscayl frame invocation differs from its exact operation")); }
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency, "upscayl recovery requires Unix process-group cancellation")); }
    let input = single(&request.inputs, "frame")?;
    let output = single(&request.outputs, "frame")?;
    let evidence = single(&request.outputs, "evidence")?;
    png_port(input)?;
    png_port(output)?;
    evidence_port(evidence, "application/vnd.aniflow.upscayl-frame-report+json")?;
    if output.artifact_id != format!("frame_{:06}", operation.frame.ordinal)
        || evidence.artifact_id != format!("evidence_{:06}", operation.frame.ordinal)
    { return Err(refused("upscayl output IDs differ from their exact frame ordinal")); }
    let config = &operation.settings;
    let frame = &operation.frame;
    let (width, height) = config.output_dimensions(frame.input_width, frame.input_height)?;
    let source = inspect_png(&input.path, frame.input_width, frame.input_height, config.import.maximum_frame_bytes)?;
    if source != (frame.input_sha256.clone(), frame.input_byte_count) {
        return Err(refused("source PNG differs from the planned frame identity"));
    }
    let components = ComponentObservations::observe(config)?;
    let parent = output_parent(&[&output.path, &evidence.path], &[
        &input.path, &config.upscayl.executable, &config.import.ffmpeg.executable,
        &config.model.param.path, &config.model.weights.path,
    ])?;
    let required = components.bytes()?.checked_add(source.1)
        .and_then(|value| value.checked_add(config.maximum_output_frame_bytes.checked_mul(2)?))
        .and_then(|value| value.checked_add(UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES))
        .ok_or_else(|| refused("frame storage estimate overflow"))?;
    storage(parent, config, required)?;
    let private = tempfile::Builder::new().prefix(".upscayl-frame-").tempdir_in(parent)
        .map_err(|_| io_failure("cannot reserve owned upscayl frame work"))?;
    let private_root = private.path().canonicalize().map_err(|_| refused("cannot resolve private frame work"))?;
    let _retained = private.keep();
    let cancellation = CancellationToken::default();
    let result = execute_private_frame(request, &operation, input, output, evidence, &private_root,
        &source, &components, (width, height), &cancellation);
    if let Err(error) = &result { preserve_error(&private_root, frame.ordinal, "frame", error); }
    result
}

#[allow(clippy::too_many_arguments)]
fn execute_private_frame(
    request: &ProviderInvocationRequest, operation: &UpscaylFrameOperation,
    input: &ProviderInvocationArtifactBinding, output: &ProviderInvocationArtifactBinding,
    evidence: &ProviderInvocationArtifactBinding, root: &Path, source: &(String, u64),
    components: &ComponentObservations, dimensions: (u32, u32), cancellation: &CancellationToken,
) -> Result<()> {
    let config = &operation.settings;
    let frame = &operation.frame;
    let (width, height) = dimensions;
    let (param_name, weights_name) = config.model_filenames()?;
    let models = root.join("models");
    std::fs::create_dir(&models).map_err(|_| io_failure("cannot reserve private model directory"))?;
    let snapshot = root.join("input.png");
    let candidate = root.join("output.png");
    let param = models.join(&param_name);
    let weights = models.join(&weights_name);
    let mut processor = config.upscayl.clone();
    let mut decoder = config.import.ffmpeg.clone();
    processor.executable = root.join("processor");
    decoder.executable = root.join("ffmpeg");
    for (original, copy, observed, maximum) in [
        (&config.upscayl.executable, &processor.executable, &components.processor, MAXIMUM_TOOL_BYTES),
        (&config.import.ffmpeg.executable, &decoder.executable, &components.decoder, MAXIMUM_TOOL_BYTES),
        (&config.model.param.path, &param, &components.param, MAXIMUM_MODEL_BYTES),
        (&config.model.weights.path, &weights, &components.weights, MAXIMUM_MODEL_BYTES),
    ] { copy_pinned(original, copy, observed, maximum, cancellation)?; }
    make_executable(&processor.executable)?;
    make_executable(&decoder.executable)?;
    copy_pinned(&input.path, &snapshot, source, config.import.maximum_frame_bytes, cancellation)?;
    let limits = config.import.tool_limits();
    crate::frame_decode::verify_version(&decoder, &limits, cancellation, root)?;
    let input_decoded = crate::frame_decode::decode(&decoder, &limits, cancellation, root, &snapshot, frame.input_width, frame.input_height)
        .map_err(|error| { preserve_error(root, frame.ordinal, "input_decode", &error); error })?;
    let mut arguments: Vec<OsString> = [
        "-i".to_owned(), "input.png".to_owned(), "-o".to_owned(), "output.png".to_owned(),
        "-m".to_owned(), "models".to_owned(), "-n".to_owned(), config.model.name.clone(),
        "-z".to_owned(), config.model.native_scale.to_string(), "-s".to_owned(), config.output_scale.to_string(),
        "-t".to_owned(), config.tile_size.to_string(), "-g".to_owned(), config.gpu_id.to_string(),
        "-j".to_owned(), config.threads.clone(), "-f".to_owned(), "png".to_owned(),
    ].into_iter().map(Into::into).collect();
    if config.tta { arguments.push("-x".into()); }
    let started = Instant::now();
    run_tool_isolated(&processor, "upscayl", &arguments, &limits, cancellation, GroupPolicy::Inherit, Some(root))
        .map_err(|diagnostic| {
            let detail = serde_json::json!({"schema":"aniflow.upscayl-private-failure/v1", "ordinal":frame.ordinal,
                "phase":"processor", "diagnostic":diagnostic, "cause":"unproven", "accepted":false});
            if let Ok(bytes) = canonical_json_bytes(&detail) { let _ = write_new(&root.join("failure.json"), &bytes); }
            Error::new(ErrorCategory::Execution, format!("upscayl frame {} process failed ({:?}): {}; inspect retained evidence before explicitly resuming", frame.ordinal, diagnostic.code, diagnostic.message))
        })?;
    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    verify_private_membership(root, &param_name, &weights_name)?;
    if identity(&snapshot, config.import.maximum_frame_bytes)? != *source {
        return Err(refused("upscayl changed the immutable input snapshot"));
    }
    let observed = inspect_png(&candidate, width, height, config.maximum_output_frame_bytes)?;
    let output_decoded = crate::frame_decode::decode(&decoder, &limits, cancellation, root, &candidate, width, height)
        .map_err(|error| { preserve_error(root, frame.ordinal, "output_decode", &error); error })?;
    if inspect_png(&candidate, width, height, config.maximum_output_frame_bytes)? != observed {
        return Err(refused("upscayl output changed during full decode"));
    }
    verify_private_membership(root, &param_name, &weights_name)?;
    components.verify(config)?;
    for (path, expected, maximum) in [
        (&processor.executable, &components.processor, MAXIMUM_TOOL_BYTES),
        (&decoder.executable, &components.decoder, MAXIMUM_TOOL_BYTES),
        (&param, &components.param, MAXIMUM_MODEL_BYTES),
        (&weights, &components.weights, MAXIMUM_MODEL_BYTES),
        (&snapshot, source, config.import.maximum_frame_bytes),
        (&input.path, source, config.import.maximum_frame_bytes),
    ] { if identity(path, maximum)? != *expected { return Err(refused("upscayl input/component identity changed during execution")); } }
    copy_pinned(&candidate, &output.path, &observed, config.maximum_output_frame_bytes, cancellation)?;
    let report = UpscaylFrameReport {
        schema:UPSCAYL_FRAME_REPORT_SCHEMA_V1.to_owned(), stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(),
        configuration_sha256:request.configuration.effective_configuration_sha256.clone(), frame:frame.clone(),
        upscayl_sha256:processor.sha256, decoder_sha256:decoder.sha256,
        model_param_sha256:components.param.0.clone(), model_weights_sha256:components.weights.0.clone(),
        native_scale:config.model.native_scale, output_scale:config.output_scale, output_width:width, output_height:height,
        output_sha256:observed.0, output_byte_count:observed.1,
        input_decoded_rgb24_sha256:input_decoded, output_decoded_rgb24_sha256:output_decoded,
        processor_elapsed_milliseconds:elapsed, backend_qualification:"unqualified".to_owned(),
    };
    let bytes = canonical_json_bytes(&report)?;
    if bytes.len() as u64 > UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES { return Err(refused("upscayl frame report exceeds its byte bound")); }
    write_new(&evidence.path, &bytes)?;
    std::fs::remove_dir_all(root).map_err(|_| io_failure("cannot remove successful owned upscayl frame scratch"))
}

fn verify_frame_report(report: &UpscaylFrameReport, index: &crate::video_interpolation::RifeFrameIndex,
    index_sha256: &str, ordinal: usize, settings: &UpscaylRecoveryConfiguration, dimensions: (u32, u32)) -> Result<()> {
    let frame = &index.frames[ordinal];
    let timeline = index.temporal_inspection.video_timeline().map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
    let expected = UpscaylFrameOperation {
        schema:UPSCAYL_FRAME_OPERATION_SCHEMA_V1.to_owned(), settings:settings.clone(),
        frame:UpscaylFrameSpec {
            source_sha256:index.temporal_inspection.source_sha256.clone(),
            source_byte_count:index.temporal_inspection.source_size_bytes,
            frame_index_sha256:index_sha256.to_owned(), ordinal:frame.ordinal, filename:frame.filename.clone(),
            source_time:timeline.frames[ordinal].source_time, input_sha256:frame.sha256.clone(),
            input_byte_count:report.frame.input_byte_count, input_width:index.width, input_height:index.height,
        },
    };
    expected.validate()?;
    for digest in [&report.provider_lock_sha256, &report.output_sha256,
        &report.input_decoded_rgb24_sha256, &report.output_decoded_rgb24_sha256] {
        require_sha256(digest, "upscayl frame evidence SHA-256")?;
    }
    if report.schema != UPSCAYL_FRAME_REPORT_SCHEMA_V1
        || report.stage_id != format!("upscale_frame_{:06}", frame.ordinal)
        || report.frame != expected.frame
        || report.configuration_sha256 != expected.provider_configuration()?.effective_configuration_sha256
        || report.upscayl_sha256 != settings.upscayl.sha256 || report.decoder_sha256 != settings.import.ffmpeg.sha256
        || report.model_param_sha256 != settings.model.param.sha256 || report.model_weights_sha256 != settings.model.weights.sha256
        || report.native_scale != settings.model.native_scale || report.output_scale != settings.output_scale
        || (report.output_width, report.output_height) != dimensions
        || report.output_byte_count == 0 || report.output_byte_count > settings.maximum_output_frame_bytes
        || report.backend_qualification != "unqualified"
    { return Err(refused("frame evidence differs from the exact source/model/scale/settings operation")); }
    Ok(())
}

/// Collect only declared same-run dependencies. A standalone invocation or
/// deserialized report cannot grant historical checkpoint acceptance.
pub fn execute_collect_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation: UpscaylCollectOperation = configuration(request)?;
    operation.validate()?;
    if request.configuration != operation.provider_configuration()?
        || request.stage_id != "collect_upscaled_frames" || request.outputs.len() != 3
        || request.inputs.len() as u64 != operation.frame_count * 2 + 1
    { return Err(refused("upscayl collection differs from its exact complete operation")); }
    let index_input = single(&request.inputs, "index")?;
    evidence_port(index_input, "application/vnd.aniflow.rife-frame-index+json")?;
    let frames_output = single(&request.outputs, "frames")?;
    let index_output = single(&request.outputs, "index")?;
    let evidence_output = single(&request.outputs, "evidence")?;
    port(frames_output, ArtifactKind::Directory, ArtifactRole::Intermediate, StreamRole::Video, "application/vnd.aniflow.upscayl-frames")?;
    evidence_port(index_output, "application/vnd.aniflow.upscayl-output-index+json")?;
    evidence_port(evidence_output, "application/vnd.aniflow.upscayl-recovery-report+json")?;
    let (source_index, index_identity) = crate::frame_import::read_index(&index_input.path)?;
    if source_index.frames.len() as u64 != operation.frame_count
        || canonical_sha256(&source_index)? != operation.frame_index_sha256
    { return Err(refused("collection source index differs from the planned complete frame set")); }
    let config = &operation.settings;
    let dimensions = config.output_dimensions(source_index.width, source_index.height)?;
    let components = ComponentObservations::observe(config)?;
    let mut inputs: Vec<&Path> = request.inputs.iter().map(|binding| binding.path.as_path()).collect();
    inputs.extend([config.upscayl.executable.as_path(), config.import.ffmpeg.executable.as_path(), config.model.param.path.as_path(), config.model.weights.path.as_path()]);
    let parent = output_parent(&[&frames_output.path, &index_output.path, &evidence_output.path], &inputs)?;
    let required = operation.frame_count.checked_mul(config.maximum_output_frame_bytes)
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(UPSCAYL_MAXIMUM_REPORT_BYTES * 2))
        .ok_or_else(|| refused("collection storage estimate overflow"))?;
    storage(parent, config, required)?;
    std::fs::create_dir(&frames_output.path).map_err(|_| io_failure("cannot exclusively create complete upscayl frame view"))?;
    let cancellation = CancellationToken::default();
    let mut output_index = UpscaylOutputIndex {
        schema:UPSCAYL_OUTPUT_INDEX_SCHEMA_V1.to_owned(), source_index:source_index.clone(),
        output_width:dimensions.0, output_height:dimensions.1,
        output_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(),
        frames:Vec::with_capacity(source_index.frames.len()),
    };
    let mut reports = Vec::with_capacity(source_index.frames.len());
    let mut dependencies: Vec<(PathBuf, (String, u64), u64)> = Vec::with_capacity(source_index.frames.len() * 2);
    for (ordinal, frame) in source_index.frames.iter().enumerate() {
        let image = named(&request.inputs, "frames", &format!("frame_{:06}", frame.ordinal))?;
        let evidence = named(&request.inputs, "evidence", &format!("evidence_{:06}", frame.ordinal))?;
        png_port(image)?;
        evidence_port(evidence, "application/vnd.aniflow.upscayl-frame-report+json")?;
        let (report, report_identity): (UpscaylFrameReport, _) = read_document(&evidence.path, UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES)?;
        verify_frame_report(&report, &source_index, &operation.frame_index_sha256, ordinal, config, dimensions)?;
        let observed = inspect_png(&image.path, dimensions.0, dimensions.1, config.maximum_output_frame_bytes)?;
        if observed != (report.output_sha256.clone(), report.output_byte_count) {
            return Err(refused("accepted frame bytes differ from their decoded frame observations"));
        }
        copy_pinned(&image.path, &frames_output.path.join(&frame.filename), &observed, config.maximum_output_frame_bytes, &cancellation)?;
        dependencies.push((image.path.clone(), observed, config.maximum_output_frame_bytes));
        dependencies.push((evidence.path.clone(), report_identity, UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES));
        output_index.frames.push(RifeFrame { ordinal:frame.ordinal, filename:frame.filename.clone(), sha256:report.output_sha256.clone() });
        reports.push(report);
    }
    let names: Vec<OsString> = source_index.frames.iter().map(|frame| frame.filename.clone().into()).collect();
    inventory(&frames_output.path, &names)?;
    for (frame, report) in output_index.frames.iter().zip(&reports) {
        if identity(&frames_output.path.join(&frame.filename), config.maximum_output_frame_bytes)? != (report.output_sha256.clone(), report.output_byte_count) {
            return Err(refused("collected output changed before completion"));
        }
    }
    for (path, observed, maximum) in dependencies {
        if identity(&path, maximum)? != observed { return Err(refused("upscayl dependency changed during collection")); }
    }
    if identity(&index_input.path, RIFE_MAXIMUM_INDEX_BYTES)? != index_identity { return Err(refused("source index changed during collection")); }
    components.verify(config)?;
    output_index.validate()?;
    let report = UpscaylRecoveryReport {
        schema:UPSCAYL_RECOVERY_REPORT_SCHEMA_V1.to_owned(), stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(),
        configuration_sha256:request.configuration.effective_configuration_sha256.clone(),
        source_relationship:"declared_not_verified".to_owned(), historical_processing:"unknown".to_owned(),
        backend_qualification:"unqualified".to_owned(), output_index, frames:reports,
    };
    let index_bytes = canonical_json_bytes(&report.output_index)?;
    let report_bytes = canonical_json_bytes(&report)?;
    if index_bytes.len() as u64 > UPSCAYL_MAXIMUM_REPORT_BYTES || report_bytes.len() as u64 > UPSCAYL_MAXIMUM_REPORT_BYTES {
        return Err(refused("complete upscayl index/report exceeds its bounded size"));
    }
    write_new(&index_output.path, &index_bytes)?;
    write_new(&evidence_output.path, &report_bytes)
}
