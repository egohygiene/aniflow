//! Fixed native encoder worker. The runner owns process groups and recovery.
use super::*;
use crate::frame_io::{copy_pinned, ensure_parent, inspect_png, inventory, require_absent};

fn observed_inputs(op: &MovieOperation, frames: &Path, report_path: &Path, index_path: &Path, token: &CancellationToken) -> Result<PreviewReport> {
    cancelled(token)?;
    let s = &op.source;
    if identity(report_path, MAXIMUM_PREVIEW_REPORT_BYTES)? != (s.report_sha256.clone(), s.report_byte_count) {
        return Err(invalid("movie preview report snapshot differs from its selected acceptance"));
    }
    let (index, index_identity) = crate::video_interpolation::provider::read_index(index_path)?;
    if index_identity != (s.source_index_sha256.clone(), s.source_index_byte_count) { return Err(invalid("movie original frame index snapshot changed")); }
    let report: PreviewReport = local::read_bounded(report_path, MAXIMUM_PREVIEW_REPORT_BYTES)?;
    report.validate_against(&index)?;
    if report.source_index_sha256 != s.source_index_sha256 || report.source_index_byte_count != s.source_index_byte_count
        || index.width != s.width || index.height != s.height || report.frames.len() as u64 != s.frame_count
        || report.full_source_plan.output_frame_rate != s.frame_rate || report.window != s.original_window
        || report.frames.first().map(|f| f.frame.ordinal) != Some(s.first_original_output_ordinal) {
        return Err(invalid("movie source snapshot differs from the exact selected original mapping"));
    }
    let names = report.frames.iter().map(|f| std::ffi::OsString::from(&f.frame.filename)).collect::<Vec<_>>();
    inventory(frames, &names)?;
    for frame in &report.frames {
        cancelled(token)?;
        if inspect_png(&frames.join(&frame.frame.filename), s.width, s.height, op.request.settings.maximum_frame_bytes)?.0 != frame.frame.sha256 {
            return Err(invalid("movie source PNG differs from its exact accepted preview mapping"));
        }
    }
    let declaration = crate::AuthoredPipelineInput { id: "preview_frames".into(), artifact_type: "application/vnd.aniflow.rife-preview-frames".into(),
        artifact_role: crate::ArtifactRole::Intermediate, stream_role: Some(crate::StreamRole::Video) };
    let directory = crate::pipeline_v3::hash_pipeline_input(&declaration, frames).map_err(|e| invalid(e.message))?;
    if directory.content_sha256 != s.frames_sha256 || directory.byte_count != s.frames_byte_count || directory.file_count != s.frame_count
        || identity(report_path, MAXIMUM_PREVIEW_REPORT_BYTES)? != (s.report_sha256.clone(), s.report_byte_count)
        || identity(index_path, super::super::super::RIFE_MAXIMUM_INDEX_BYTES)? != index_identity {
        return Err(invalid("movie source snapshot changed during observation"));
    }
    Ok(report)
}
pub(crate) fn execute_invocation(invocation: &crate::ProviderInvocationRequest) -> Result<()> {
    invocation.validate()?;
    let operation: MovieOperation = serde_json::from_value(serde_json::to_value(&invocation.configuration.values)
        .map_err(|_| invalid("invalid preview movie values"))?).map_err(|_| invalid("invalid closed preview movie operation"))?;
    operation.validate()?;
    if !cfg!(unix) || invocation.stage_id != STAGE || invocation.configuration != operation.configuration()?
        || invocation.inputs.len() != 3 || invocation.outputs.len() != 2 { return Err(invalid("invocation is outside the fixed Unix preview movie profile")); }
    let select = |entries: &[crate::ProviderInvocationArtifactBinding], port: &str, id: &str, kind: &str,
        role: crate::ArtifactRole, stream: crate::StreamRole, directory: bool| -> Result<PathBuf> {
        let selected = entries.iter().filter(|a| a.port == port).collect::<Vec<_>>();
        if selected.len() != 1 { return Err(invalid("preview movie port is absent or ambiguous")); }
        let a = selected[0];
        if a.artifact_id != id || a.artifact_type != kind || a.artifact_role != role || a.stream_role != Some(stream)
            || a.kind != if directory { crate::ArtifactKind::Directory } else { crate::ArtifactKind::File } {
            return Err(invalid("preview movie port contract differs"));
        }
        Ok(a.path.clone())
    };
    let frames = select(&invocation.inputs, "frames", "preview_frames", "application/vnd.aniflow.rife-preview-frames", crate::ArtifactRole::Intermediate, crate::StreamRole::Video, true)?;
    let preview = select(&invocation.inputs, "preview", "source_preview", "application/vnd.aniflow.rife-preview-report+json", crate::ArtifactRole::ValidationEvidence, crate::StreamRole::TimedMetadata, false)?;
    let index = select(&invocation.inputs, "index", "source_index", "application/vnd.aniflow.rife-frame-index+json", crate::ArtifactRole::ValidationEvidence, crate::StreamRole::TimedMetadata, false)?;
    let media = select(&invocation.outputs, "media", "preview_movie", "video/mp4", crate::ArtifactRole::Intermediate, crate::StreamRole::Video, false)?;
    let evidence = select(&invocation.outputs, "evidence", "movie_report", "application/vnd.aniflow.rife-preview-movie-report+json", crate::ArtifactRole::ValidationEvidence, crate::StreamRole::TimedMetadata, false)?;
    let parent = media.parent().ok_or_else(|| invalid("preview movie output parent missing"))?;
    if evidence.parent() != Some(parent) || media == evidence { return Err(invalid("movie outputs must be distinct siblings")); }
    let settings = &operation.request.settings;
    let renderer = std::env::current_exe().map_err(|_| invalid("cannot identify preview movie renderer"))?;
    if identity(&renderer, crate::video_assembly::MAXIMUM_TOOL_BYTES)? != (operation.renderer_sha256.clone(), operation.renderer_byte_count) {
        return Err(invalid("preview movie renderer implementation changed"));
    }
    for output in [&media, &evidence] {
        require_absent(output)?;
        for input in [&frames, &preview, &index, &settings.ffmpeg.executable, &settings.ffprobe.executable, &renderer] {
            if output.starts_with(input) || input.starts_with(output) { return Err(invalid("preview movie output overlaps source or executable")); }
        }
    }
    let token = CancellationToken::default();
    let original = observed_inputs(&operation, &frames, &preview, &index, &token)?;
    let tools = crate::video_assembly::observe_components(settings)?;
    if [tools[0].1, tools[1].1] != operation.tool_byte_counts { return Err(invalid("preview movie tool byte admission changed")); }
    ensure_parent(parent)?;
    crate::video_assembly::require_storage(parent, operation.working_bytes, settings.minimum_free_bytes)?;
    let private = tempfile::Builder::new().prefix(".rife-preview-movie-").tempdir_in(parent).map_err(|_| invalid("cannot reserve private preview movie work"))?;
    let root = private.path().canonicalize().map_err(|_| invalid("cannot resolve private preview movie work"))?;
    let result: Result<()> = (|| {
        let mut native = settings.clone();
        native.ffmpeg.executable = root.join("ffmpeg");
        native.ffprobe.executable = root.join("ffprobe");
        for (pin, destination, expected) in [(&settings.ffmpeg, &native.ffmpeg.executable, &tools[0]), (&settings.ffprobe, &native.ffprobe.executable, &tools[1])] {
            copy_pinned(&pin.executable, destination, expected, crate::video_assembly::MAXIMUM_TOOL_BYTES, &token)?;
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700)).map_err(|_| invalid("cannot make private preview tool executable"))?; }
        }
        let private_frames = root.join("frames");
        local::directory(&private_frames)?;
        let mut copied = Vec::with_capacity(original.frames.len());
        for (i, frame) in original.frames.iter().enumerate() {
            let source = frames.join(&frame.frame.filename);
            let expected = identity(&source, settings.maximum_frame_bytes)?;
            if expected.0 != frame.frame.sha256 { return Err(invalid("movie PNG changed before private snapshot")); }
            // image2 starts at local ordinal zero; the report retains global ordinals.
            let destination = private_frames.join(format!("frame-{i:08}.png"));
            copy_pinned(&source, &destination, &expected, settings.maximum_frame_bytes, &token)?;
            copied.push((destination, expected));
        }
        let candidate = root.join("preview.mp4");
        let observed = crate::video_assembly::media::rife_preview::encode(&operation, &native, &private_frames, &candidate, &root, &token)?;
        let report = MovieReport { schema: REPORT.into(), operation: operation.clone(), provider_lock_sha256: invocation.provider_lock_sha256.clone(),
            frames: mapping(&original)?, media: observed, clock_policy: CLOCK.into(), color_policy: COLOR.into(), audio_scope: AUDIO.into(),
            review_scope: SCOPE.into(), native_qualified: false, creative_approval: "not_performed".into(), publication_authority: "none".into() };
        report.validate(&original)?;
        if observed_inputs(&operation, &frames, &preview, &index, &token)? != original { return Err(invalid("movie source changed during encoding")); }
        for (path, expected) in &copied {
            if identity(path, settings.maximum_frame_bytes)? != *expected { return Err(invalid("private preview PNG changed during encoding")); }
        }
        inventory(&private_frames, &copied.iter().map(|(p,_)| p.file_name().unwrap().to_owned()).collect::<Vec<_>>())?;
        if crate::video_assembly::observe_components(settings)? != tools || crate::video_assembly::observe_components(&native)? != tools
            || identity(&renderer, crate::video_assembly::MAXIMUM_TOOL_BYTES)? != (operation.renderer_sha256.clone(), operation.renderer_byte_count) {
            return Err(invalid("preview movie tools/implementation changed during rendering"));
        }
        let bytes = local::bytes(&report)?;
        copy_pinned(&candidate, &media, &(report.media.sha256.clone(), report.media.byte_count), settings.maximum_output_bytes, &token)?;
        local::write(&evidence, &bytes)?;
        local::sync(parent)?;
        Ok(())
    })();
    match result { Ok(()) => Ok(()), Err(error) => { let _retained = private.keep(); Err(error) } }
}
