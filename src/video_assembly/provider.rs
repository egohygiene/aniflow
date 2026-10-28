//! Fixed native assembly operations. Pipeline v3 owns promotion, cancellation,
//! checkpoints and acceptance; private candidates never grant those authorities.
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::audio_inspection::AudioToolObservation;
use crate::frame_io::{copy_pinned, ensure_parent, identity, inspect_png, inventory, require_absent};
use crate::provider::canonical_json_bytes;
use crate::{ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationArtifactBinding, ProviderInvocationRequest, Result, StreamRole};
use super::*;

const MAXIMUM_TOOL_BYTES: u64 = 134_217_728;

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }
fn io_failure(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Io, message) }

fn binding<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut matches = bindings.iter().filter(|binding| binding.port == port);
    let value = matches.next().ok_or_else(|| refused(format!("missing assembly {port} port")))?;
    if matches.next().is_some() { return Err(refused(format!("duplicate assembly {port} port"))); }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn port(bindings: &[ProviderInvocationArtifactBinding], name: &str, artifact_id: &str, kind: ArtifactKind,
    role: ArtifactRole, stream: StreamRole, artifact_type: &str) -> Result<()> {
    let value = binding(bindings, name)?;
    if value.artifact_id != artifact_id || value.kind != kind || value.artifact_role != role
        || value.stream_role != Some(stream) || value.artifact_type != artifact_type {
        return Err(refused("assembly artifact binding differs from its fixed stage profile"));
    }
    Ok(())
}

fn operation(request: &ProviderInvocationRequest) -> Result<AssemblyOperation> {
    request.validate()?;
    let value: AssemblyOperation = serde_json::from_value(serde_json::to_value(&request.configuration.values)
        .map_err(|_| refused("cannot read assembly operation"))?)
        .map_err(|_| refused("invalid closed assembly operation"))?;
    value.validate()?;
    if value.provider_configuration()? != request.configuration || request.stage_id != value.stage.stage_id()
        || request.inputs.len() != 3 || request.outputs.len() != 2 {
        return Err(refused("assembly requires its exact locked operation and fixed artifact ports"));
    }
    port(&request.inputs, "source", "source_video", ArtifactKind::File, ArtifactRole::TemporalSource, StreamRole::Video, "video/*")?;
    port(&request.inputs, "index", "source_index", ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, "application/vnd.aniflow.assembly-frame-index+json")?;
    match value.stage {
        AssemblyStageKind::Master => port(&request.inputs, "frames", "source_frames", ArtifactKind::Directory, ArtifactRole::Intermediate, StreamRole::Video, "application/vnd.aniflow.assembly-frames")?,
        AssemblyStageKind::Delivery => port(&request.inputs, "master", "lossless_master", ArtifactKind::File, ArtifactRole::CandidateMaster, StreamRole::Video, "video/mp4")?,
    }
    port(&request.outputs, "media", value.stage.artifact_id(), ArtifactKind::File, ArtifactRole::CandidateMaster, StreamRole::Video, "video/mp4")?;
    let report_id = match value.stage { AssemblyStageKind::Master => "master_evidence", AssemblyStageKind::Delivery => "delivery_evidence" };
    port(&request.outputs, "evidence", report_id, ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, "application/vnd.aniflow.video-assembly-report+json")?;
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency, "native assembly requires Unix provider process-group cancellation")); }
    Ok(value)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)
        .map_err(|_| io_failure("cannot exclusively create assembly evidence"))?;
    file.write_all(bytes).and_then(|()| file.sync_all())
        .map_err(|_| io_failure("cannot synchronize assembly evidence"))
}

fn private_executable(path: &Path) -> Result<()> {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| io_failure("cannot make the private assembly tool executable"))?;
    }
    Ok(())
}

fn output_parent<'a>(request: &'a ProviderInvocationRequest, config: &AssemblyConfiguration) -> Result<&'a Path> {
    let media = &binding(&request.outputs, "media")?.path;
    let report = &binding(&request.outputs, "evidence")?.path;
    let parent = media.parent().ok_or_else(|| refused("assembly output has no assigned parent"))?;
    if report.parent() != Some(parent) || media == report {
        return Err(refused("assembly outputs must be distinct siblings"));
    }
    for output in [media, report] {
        for input in request.inputs.iter().map(|binding| &binding.path)
            .chain([&config.ffmpeg.executable, &config.ffprobe.executable]) {
            if output.starts_with(input) || input.starts_with(output) {
                return Err(refused("assembly outputs must remain separate from every source and tool"));
            }
        }
        require_absent(output)?;
    }
    ensure_parent(parent)?;
    Ok(parent)
}

fn same_identity(path: &Path, maximum: u64, expected: &(String, u64)) -> Result<()> {
    if identity(path, maximum)? != *expected { return Err(refused("assembly source or component changed during execution")); }
    Ok(())
}

fn preserve_failure(root: &Path, stage: AssemblyStageKind, error: &Error) {
    let detail = serde_json::json!({
        "schema":"aniflow.video-assembly-private-failure/v1", "stage":stage,
        "category":error.category(), "message":error.message(), "cause":"unproven", "accepted":false,
    });
    if let Ok(bytes) = canonical_json_bytes(&detail) { let _ = write_new(&root.join("failure.json"), &bytes); }
}

/// Execute one fixed provider operation. Successful native observations are
/// still candidates until the outer runtime's validation gates accept them.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation = operation(request)?;
    let config = &operation.settings;
    let source_path = &binding(&request.inputs, "source")?.path;
    let index_path = &binding(&request.inputs, "index")?.path;
    let (index, index_identity) = read_index(index_path)?;
    admit_index(config, &index)?;
    let source_identity = identity(source_path, config.maximum_source_bytes)?;
    if source_identity != (operation.source_sha256.clone(), operation.source_byte_count)
        || source_identity.0 != index.temporal().source_sha256
        || source_identity.1 != index.temporal().source_size_bytes
        || index_identity.0 != operation.frame_index_sha256
        || index.schema() != operation.index_schema
        || index.frames().len() as u64 != operation.frame_count
        || index.dimensions() != (operation.width, operation.height)
        || index.temporal().selection_intent != operation.source_selection {
        return Err(refused("assembly source or index differs from the frozen operation"));
    }
    let mut frame_observations = Vec::new();
    let mut frame_bytes = 0_u64;
    let mut master_identity = None;
    match operation.stage {
        AssemblyStageKind::Master => {
            let directory = &binding(&request.inputs, "frames")?.path;
            let names: Vec<OsString> = index.frames().iter().map(|frame| frame.filename.clone().into()).collect();
            inventory(directory, &names)?;
            frame_observations.reserve(index.frames().len());
            for frame in index.frames() {
                let observed = inspect_png(&directory.join(&frame.filename), operation.width, operation.height, config.maximum_frame_bytes)?;
                if observed.0 != frame.sha256 { return Err(refused("assembly PNG differs from the indexed digest")); }
                frame_bytes = frame_bytes.checked_add(observed.1).filter(|bytes| *bytes <= config.maximum_total_frame_bytes)
                    .ok_or_else(|| refused("assembly frames exceed the aggregate byte bound"))?;
                frame_observations.push(observed);
            }
        }
        AssemblyStageKind::Delivery => {
            let observed = identity(&binding(&request.inputs, "master")?.path, config.maximum_output_bytes)?;
            if observed.1 == 0 { return Err(refused("accepted master is empty")); }
            master_identity = Some(observed);
        }
    }
    let components = observe_components(config)?;
    let tool_bytes = components[0].1.checked_add(components[1].1).ok_or_else(|| refused("assembly tool byte count overflow"))?;
    // Admit the conservative complete-run ceiling, then require only this
    // operation's new allocation so resume does not reserve accepted work twice.
    checked_working_bytes(config, source_identity.1, frame_bytes, tool_bytes)?;
    let needed = source_identity.1.checked_mul(3)
        .and_then(|bytes| bytes.checked_add(frame_bytes))
        .and_then(|bytes| bytes.checked_add(tool_bytes))
        .and_then(|bytes| bytes.checked_add(config.maximum_output_bytes.checked_mul(3)?))
        .and_then(|bytes| bytes.checked_add(ASSEMBLY_MAXIMUM_REPORT_BYTES.checked_mul(4)?))
        .and_then(|bytes| bytes.checked_add((config.maximum_capture_bytes as u64).checked_mul(12)?))
        .ok_or_else(|| refused("assembly immediate storage estimate overflow"))?;
    let parent = output_parent(request, config)?;
    require_storage(parent, needed, config.minimum_free_bytes)?;
    // An accepted checkpoint skips this invocation entirely. Recheck the
    // existing native validator's separate temporary volume only when this
    // stage will produce a new candidate that still needs validation.
    require_validator_storage(config)?;
    let private = tempfile::Builder::new().prefix(".video-assembly-").tempdir_in(parent)
        .map_err(|_| io_failure("cannot reserve private assembly work"))?;
    let root = private.path().canonicalize().map_err(|_| refused("cannot resolve private assembly work"))?;
    let cancellation = CancellationToken::default();
    let result = execute_private(request, &operation, &index, &index_identity, &source_identity,
        &frame_observations, master_identity.as_ref(), &components, &root, &cancellation);
    if let Err(error) = &result {
        let retained = private.keep();
        preserve_failure(&retained, operation.stage, error);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn execute_private(request: &ProviderInvocationRequest, operation: &AssemblyOperation, index: &AssemblyFrameIndex,
    index_identity: &(String, u64), source_identity: &(String, u64), frame_observations: &[(String, u64)],
    master_identity: Option<&(String, u64)>, components: &[(String, u64); 2], root: &Path,
    cancellation: &CancellationToken) -> Result<()> {
    let config = &operation.settings;
    let source_path = &binding(&request.inputs, "source")?.path;
    let index_path = &binding(&request.inputs, "index")?.path;
    let source_copy = root.join("source.media");
    let index_copy = root.join("index.json");
    let candidate = root.join("candidate.mp4");
    let mut private_config = config.clone();
    private_config.ffmpeg.executable = root.join("ffmpeg");
    private_config.ffprobe.executable = root.join("ffprobe");
    for (original, copy, observed) in [
        (&config.ffmpeg.executable, &private_config.ffmpeg.executable, &components[0]),
        (&config.ffprobe.executable, &private_config.ffprobe.executable, &components[1]),
    ] {
        copy_pinned(original, copy, observed, MAXIMUM_TOOL_BYTES, cancellation)?;
        private_executable(copy)?;
    }
    copy_pinned(source_path, &source_copy, source_identity, config.maximum_source_bytes, cancellation)?;
    copy_pinned(index_path, &index_copy, index_identity, ASSEMBLY_MAXIMUM_INDEX_BYTES, cancellation)?;
    let mut frames_directory = None;
    let mut master_copy = None;
    match operation.stage {
        AssemblyStageKind::Master => {
            let input_frames = &binding(&request.inputs, "frames")?.path;
            let directory = root.join("frames");
            std::fs::create_dir(&directory).map_err(|_| io_failure("cannot reserve private assembly frame directory"))?;
            for (position, (frame, observed)) in index.frames().iter().zip(frame_observations).enumerate() {
                copy_pinned(&input_frames.join(&frame.filename), &directory.join(format!("frame-{position:08}.png")), observed, config.maximum_frame_bytes, cancellation)?;
            }
            frames_directory = Some(directory);
        }
        AssemblyStageKind::Delivery => {
            let observed = master_identity.ok_or_else(|| refused("missing pinned master identity"))?;
            let copy = root.join("master.mp4");
            copy_pinned(&binding(&request.inputs, "master")?.path, &copy, observed, config.maximum_output_bytes, cancellation)?;
            master_copy = Some(copy);
        }
    }
    let timeline = index.temporal().video_timeline().map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
    let period = timeline.frames.first().ok_or_else(|| refused("assembly has no source frame interval"))?.source_time.duration()
        .map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
    let frame_set = media::FrameSet {
        directory:frames_directory.clone(), width:operation.width, height:operation.height,
        period, frame_count:index.frames().len(), source_inspection:index.temporal().clone(),
    };
    // Native source/profile admission happens inside media::execute before
    // complete PNG decode or encoding. Private structural copies confer no proof.
    let media = media::execute(operation.stage, &private_config, &source_copy, &frame_set,
        master_copy.as_deref(), &candidate, root, cancellation)?;
    let observed_output = identity(&candidate, config.maximum_output_bytes)?;
    if observed_output.1 == 0 { return Err(refused("assembly output is empty")); }
    for (path, expected, maximum) in [
        (source_path.as_path(), source_identity, config.maximum_source_bytes),
        (source_copy.as_path(), source_identity, config.maximum_source_bytes),
        (index_path.as_path(), index_identity, ASSEMBLY_MAXIMUM_INDEX_BYTES),
        (index_copy.as_path(), index_identity, ASSEMBLY_MAXIMUM_INDEX_BYTES),
        (config.ffmpeg.executable.as_path(), &components[0], MAXIMUM_TOOL_BYTES),
        (config.ffprobe.executable.as_path(), &components[1], MAXIMUM_TOOL_BYTES),
        (private_config.ffmpeg.executable.as_path(), &components[0], MAXIMUM_TOOL_BYTES),
        (private_config.ffprobe.executable.as_path(), &components[1], MAXIMUM_TOOL_BYTES),
    ] { same_identity(path, maximum, expected)?; }
    if let Some(directory) = &frames_directory {
        let input_frames = &binding(&request.inputs, "frames")?.path;
        let original_names: Vec<OsString> = index.frames().iter().map(|frame| frame.filename.clone().into()).collect();
        let private_names: Vec<OsString> = (0..index.frames().len()).map(|position| format!("frame-{position:08}.png").into()).collect();
        inventory(input_frames, &original_names)?;
        inventory(directory, &private_names)?;
        for (position, (frame, expected)) in index.frames().iter().zip(frame_observations).enumerate() {
            same_identity(&input_frames.join(&frame.filename), config.maximum_frame_bytes, expected)?;
            same_identity(&directory.join(format!("frame-{position:08}.png")), config.maximum_frame_bytes, expected)?;
        }
    }
    if let (Some(copy), Some(expected)) = (&master_copy, master_identity) {
        same_identity(&binding(&request.inputs, "master")?.path, config.maximum_output_bytes, expected)?;
        same_identity(copy, config.maximum_output_bytes, expected)?;
    }
    let report = AssemblyReport {
        schema:ASSEMBLY_REPORT_SCHEMA_V1.to_owned(), stage:operation.stage, stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(), configuration_sha256:request.configuration.effective_configuration_sha256.clone(),
        source_sha256:source_identity.0.clone(), source_byte_count:source_identity.1,
        frame_index_sha256:index_identity.0.clone(), index_schema:index.schema().to_owned(), frame_count:operation.frame_count,
        width:operation.width, height:operation.height, output_sha256:observed_output.0.clone(), output_byte_count:observed_output.1,
        ffmpeg:AudioToolObservation { id:"ffmpeg".to_owned(), version:config.ffmpeg.version.clone(), sha256:config.ffmpeg.sha256.clone() },
        ffprobe:AudioToolObservation { id:"ffprobe".to_owned(), version:config.ffprobe.version.clone(), sha256:config.ffprobe.sha256.clone() },
        source_relationship:"declared_not_verified".to_owned(), historical_processing:"unknown".to_owned(),
        backend_qualification:"invocation_observed_not_backend_qualified".to_owned(), media,
    };
    let bytes = canonical_json_bytes(&report)?;
    if bytes.len() as u64 > ASSEMBLY_MAXIMUM_REPORT_BYTES { return Err(refused("assembly evidence exceeds its 64 MiB bound")); }
    copy_pinned(&candidate, &binding(&request.outputs, "media")?.path, &observed_output, config.maximum_output_bytes, cancellation)?;
    write_new(&binding(&request.outputs, "evidence")?.path, &bytes)
}
