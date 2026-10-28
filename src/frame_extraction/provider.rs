//! Fixed source extraction invocation. The existing provider process and v3
//! runtime own cancellation, candidate promotion and immutable acceptance.
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use sha2::{Digest, Sha256};

use crate::audio_inspection::AudioToolObservation;
use crate::frame_io::{copy_pinned, ensure_parent, identity, inspect_png, inventory, require_absent};
use crate::provider::{canonical_json_bytes, canonical_sha256};
use crate::{ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationArtifactBinding, ProviderInvocationRequest, Result, StreamRole};
use super::*;

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }
fn io_failure(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Io, message) }

fn binding<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut matches = bindings.iter().filter(|binding| binding.port == port);
    let value = matches.next().ok_or_else(|| refused(format!("missing extraction {port} port")))?;
    if matches.next().is_some() { return Err(refused(format!("duplicate extraction {port} port"))); }
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn port(bindings: &[ProviderInvocationArtifactBinding], name: &str, id: &str, kind: ArtifactKind,
    role: ArtifactRole, stream: StreamRole, artifact_type: &str) -> Result<()> {
    let value = binding(bindings, name)?;
    if value.artifact_id != id || value.kind != kind || value.artifact_role != role
        || value.stream_role != Some(stream) || value.artifact_type != artifact_type {
        return Err(refused("extraction artifact binding differs from its fixed stage profile"));
    }
    Ok(())
}

fn operation(request: &ProviderInvocationRequest) -> Result<FrameExtractionOperation> {
    request.validate()?;
    let value: FrameExtractionOperation = serde_json::from_value(serde_json::to_value(&request.configuration.values)
        .map_err(|_| refused("cannot read extraction operation"))?)
        .map_err(|_| refused("invalid closed extraction operation"))?;
    value.validate()?;
    if value.provider_configuration()? != request.configuration || request.stage_id != "extract_frames"
        || request.inputs.len() != 1 || request.outputs.len() != 3 {
        return Err(refused("extraction requires its exact locked operation and fixed artifact ports"));
    }
    port(&request.inputs, "source", "source_video", ArtifactKind::File, ArtifactRole::TemporalSource, StreamRole::Video, "video/*")?;
    port(&request.outputs, "frames", "extracted_frames", ArtifactKind::Directory, ArtifactRole::Intermediate, StreamRole::Video, "application/vnd.aniflow.rife-frames")?;
    port(&request.outputs, "index", "extracted_index", ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, "application/vnd.aniflow.rife-frame-index+json")?;
    port(&request.outputs, "evidence", "extraction_evidence", ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, "application/vnd.aniflow.frame-extraction-report+json")?;
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency, "native extraction requires Unix provider process-group cancellation")); }
    Ok(value)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)
        .map_err(|_| io_failure("cannot exclusively create extraction evidence"))?;
    file.write_all(bytes).and_then(|()| file.sync_all())
        .map_err(|_| io_failure("cannot synchronize extraction evidence"))
}

fn output_parent<'a>(request: &'a ProviderInvocationRequest, settings: &FrameExtractionConfiguration) -> Result<&'a Path> {
    let outputs = [binding(&request.outputs, "frames")?, binding(&request.outputs, "index")?, binding(&request.outputs, "evidence")?];
    let parent = outputs[0].path.parent().ok_or_else(|| refused("extraction output has no assigned parent"))?;
    for (position, output) in outputs.iter().enumerate() {
        if output.path.parent() != Some(parent) || outputs[..position].iter().any(|other| other.path == output.path) {
            return Err(refused("extraction outputs must be distinct siblings"));
        }
        for input in request.inputs.iter().map(|binding| &binding.path)
            .chain([&settings.ffmpeg.executable, &settings.ffprobe.executable]) {
            if output.path.starts_with(input) || input.starts_with(&output.path) {
                return Err(refused("extraction outputs must remain separate from source and tools"));
            }
        }
        require_absent(&output.path)?;
    }
    ensure_parent(parent)?;
    Ok(parent)
}

fn same_identity(path: &Path, maximum: u64, expected: &(String, u64)) -> Result<()> {
    if identity(path, maximum)? != *expected { return Err(refused("extraction source or tool changed during execution")); }
    Ok(())
}

fn directory_identity(path: &Path) -> Result<crate::PipelineInputIdentity> {
    let declaration = crate::AuthoredPipelineInput { id:"extracted_frames".to_owned(),
        artifact_type:"application/vnd.aniflow.rife-frames".to_owned(), artifact_role:ArtifactRole::Intermediate,
        stream_role:Some(StreamRole::Video) };
    crate::pipeline_v3::hash_pipeline_input(&declaration, path)
        .map_err(|failure| Error::new(failure.category(), failure.message))
}

pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let operation = operation(request)?;
    let settings = &operation.settings;
    let source = &binding(&request.inputs, "source")?.path;
    let source_identity = identity(source, settings.maximum_source_bytes)?;
    if source_identity != (operation.source_sha256.clone(), operation.source_byte_count) {
        return Err(refused("extraction source differs from its frozen operation"));
    }
    let components = observe_components(settings)?;
    let tool_bytes = components[0].1.checked_add(components[1].1).ok_or_else(|| refused("extraction tool byte count overflow"))?;
    // Only source/tool/probe admission occurs before observing source geometry.
    // The native operation admits the actual finite PNG envelope before writing.
    let initial = checked_working_bytes(settings, source_identity.1, tool_bytes)?;
    let parent = output_parent(request, settings)?;
    require_storage(parent, initial, settings.minimum_free_bytes)?;
    let private = tempfile::Builder::new().prefix(".frame-extraction-").tempdir_in(parent)
        .map_err(|_| io_failure("cannot reserve private extraction work"))?;
    let root = private.path().canonicalize().map_err(|_| refused("cannot resolve private extraction work"))?;
    let result = execute_private(request, &operation, &source_identity, &components, &root, &CancellationToken::default());
    if let Err(error) = &result {
        let retained = private.keep();
        let failure = serde_json::json!({"schema":"aniflow.frame-extraction-private-failure/v1",
            "stage":"extract_frames","category":error.category(),"message":error.message(),"accepted":false,"cause":"unproven"});
        if let Ok(bytes) = canonical_json_bytes(&failure) { let _ = write_new(&retained.join("failure.json"), &bytes); }
    }
    result
}

fn execute_private(request: &ProviderInvocationRequest, operation: &FrameExtractionOperation,
    source_identity: &(String,u64), components: &[(String,u64);2], root: &Path,
    cancellation: &CancellationToken) -> Result<()> {
    let settings = &operation.settings;
    let source = &binding(&request.inputs, "source")?.path;
    let source_copy = root.join("source.media");
    let private_frames = root.join("frames");
    let mut private_settings = settings.clone();
    private_settings.ffmpeg.executable = root.join("ffmpeg");
    private_settings.ffprobe.executable = root.join("ffprobe");
    for (original, copy, observed) in [
        (&settings.ffmpeg.executable, &private_settings.ffmpeg.executable, &components[0]),
        (&settings.ffprobe.executable, &private_settings.ffprobe.executable, &components[1]),
    ] {
        copy_pinned(original, copy, observed, FRAME_EXTRACTION_MAXIMUM_TOOL_BYTES, cancellation)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(copy, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| io_failure("cannot make a private extraction tool executable"))?;
        }
    }
    copy_pinned(source, &source_copy, source_identity, settings.maximum_source_bytes, cancellation)?;
    let (index, media) = media::execute(&private_settings, &source_copy, &private_frames, root, cancellation)?;
    if index.temporal_inspection.source_sha256 != operation.source_sha256
        || index.temporal_inspection.source_size_bytes != operation.source_byte_count
        || index.temporal_inspection.selection_intent != settings.source_selection {
        return Err(refused("native extraction source evidence differs from the frozen source or stream selection"));
    }
    let names: Vec<OsString> = index.frames.iter().map(|frame| frame.filename.clone().into()).collect();
    inventory(&private_frames, &names)?;
    let mut entries = Vec::with_capacity(index.frames.len());
    let mut observations = Vec::with_capacity(index.frames.len());
    let mut frame_bytes = 0_u64;
    for frame in &index.frames {
        let observed = inspect_png(&private_frames.join(&frame.filename), index.width, index.height, settings.maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(refused("extracted frame changed after source verification")); }
        frame_bytes = frame_bytes.checked_add(observed.1).filter(|bytes| *bytes <= settings.maximum_total_frame_bytes)
            .ok_or_else(|| refused("extracted frames exceed the aggregate byte bound"))?;
        entries.push(crate::pipeline_v3::DirectoryContentEntry { relative_path:frame.filename.clone(),
            kind:crate::PipelineInputKind::File,byte_count:observed.1,content_sha256:Some(observed.0.clone()) });
        observations.push(observed);
    }
    entries.sort_by(|left,right| left.relative_path.cmp(&right.relative_path));
    let frames_sha256 = canonical_sha256(&entries)?;
    let directory = directory_identity(&private_frames)?;
    if directory.content_sha256 != frames_sha256 || directory.byte_count != frame_bytes || directory.file_count != index.frames.len() as u64 {
        return Err(refused("extracted directory changed during complete identity observation"));
    }
    let index_bytes = canonical_json_bytes(&index)?;
    if index_bytes.len() as u64 > FRAME_EXTRACTION_MAXIMUM_INDEX_BYTES {
        return Err(refused("extracted original frame index exceeds its consumer's bounded contract"));
    }
    let frame_index_sha256 = format!("{:x}",Sha256::digest(&index_bytes));
    for (path, expected, maximum) in [
        (source.as_path(), source_identity, settings.maximum_source_bytes),
        (source_copy.as_path(), source_identity, settings.maximum_source_bytes),
        (settings.ffmpeg.executable.as_path(), &components[0], FRAME_EXTRACTION_MAXIMUM_TOOL_BYTES),
        (settings.ffprobe.executable.as_path(), &components[1], FRAME_EXTRACTION_MAXIMUM_TOOL_BYTES),
        (private_settings.ffmpeg.executable.as_path(), &components[0], FRAME_EXTRACTION_MAXIMUM_TOOL_BYTES),
        (private_settings.ffprobe.executable.as_path(), &components[1], FRAME_EXTRACTION_MAXIMUM_TOOL_BYTES),
    ] { same_identity(path, maximum, expected)?; }
    let report = FrameExtractionReport {
        schema:FRAME_EXTRACTION_REPORT_SCHEMA_V1.to_owned(), stage_id:request.stage_id.clone(),
        provider_lock_sha256:request.provider_lock_sha256.clone(), configuration_sha256:request.configuration.effective_configuration_sha256.clone(),
        source_sha256:source_identity.0.clone(), source_byte_count:source_identity.1, frame_index_sha256,
        frames_sha256, frames_byte_count:frame_bytes, frame_count:index.frames.len() as u64, width:index.width, height:index.height,
        ffmpeg:AudioToolObservation { id:"ffmpeg".to_owned(),version:settings.ffmpeg.version.clone(),sha256:settings.ffmpeg.sha256.clone() },
        ffprobe:AudioToolObservation { id:"ffprobe".to_owned(),version:settings.ffprobe.version.clone(),sha256:settings.ffprobe.sha256.clone() },
        source_relationship:"observed_decoded_source_frames".to_owned(), historical_processing:"unknown".to_owned(),
        backend_qualification:"invocation_observed_not_backend_qualified".to_owned(), media,
    };
    let report_bytes = canonical_json_bytes(&report)?;
    if report_bytes.len() as u64 > FRAME_EXTRACTION_MAXIMUM_REPORT_BYTES { return Err(refused("extraction evidence exceeds its bounded contract")); }
    let output_frames = &binding(&request.outputs, "frames")?.path;
    std::fs::create_dir(output_frames).map_err(|_| io_failure("cannot exclusively create extracted frame output"))?;
    for (frame, observed) in index.frames.iter().zip(&observations) {
        copy_pinned(&private_frames.join(&frame.filename), &output_frames.join(&frame.filename), observed,
            settings.maximum_frame_bytes, cancellation)?;
    }
    inventory(output_frames, &names)?;
    let copied = directory_identity(output_frames)?;
    if copied.content_sha256 != directory.content_sha256 || copied.byte_count != directory.byte_count || copied.file_count != directory.file_count {
        return Err(refused("extracted output differs from its verified private candidate"));
    }
    same_identity(source, settings.maximum_source_bytes, source_identity)?;
    same_identity(&source_copy, settings.maximum_source_bytes, source_identity)?;
    write_new(&binding(&request.outputs, "index")?.path, &index_bytes)?;
    write_new(&binding(&request.outputs, "evidence")?.path, &report_bytes)
}
