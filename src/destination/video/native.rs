use super::*;
use crate::frame_io::{copy_pinned, identity, require_absent};
use crate::{
    ArtifactKind, ArtifactRole, ProviderInvocationArtifactBinding, ProviderInvocationRequest,
    StreamRole,
};
use std::path::{Path, PathBuf};
fn port<'a>(
    values: &'a [ProviderInvocationArtifactBinding],
    name: &str,
    kind: ArtifactKind,
    role: ArtifactRole,
    stream: StreamRole,
    media: &str,
) -> Result<&'a Path> {
    let selected = values.iter().filter(|v| v.port == name).collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(invalid("destination native port missing or ambiguous"));
    }
    let v = selected[0];
    if v.kind != kind
        || v.artifact_role != role
        || v.stream_role != Some(stream)
        || v.artifact_type != media
    {
        return Err(invalid(
            "destination native port differs from its fixed profile",
        ));
    }
    Ok(&v.path)
}
fn private_tool(
    source: &Path,
    target: &Path,
    pin: &(String, u64),
    token: &CancellationToken,
) -> Result<()> {
    copy_pinned(source, target, pin, 134_217_728, token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| invalid("cannot prepare private destination tool"))?;
    }
    Ok(())
}
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    request.validate()?;
    if !cfg!(unix) {
        return Err(invalid(
            "destination native export requires Unix process-group runtime",
        ));
    }
    let operation: VideoExportOperation = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| invalid("invalid destination native values"))?,
    )
    .map_err(|_| invalid("invalid closed destination native operation"))?;
    operation.validate()?;
    if operation.configuration()? != request.configuration
        || request.inputs.len() != 1
        || request.outputs.len() != 2
    {
        return Err(invalid(
            "destination native operation/port set differs from the lock",
        ));
    }
    let source = port(
        &request.inputs,
        "source",
        ArtifactKind::File,
        ArtifactRole::TemporalSource,
        StreamRole::Video,
        "video/mp4",
    )?;
    let output = port(
        &request.outputs,
        "media",
        ArtifactKind::File,
        ArtifactRole::CandidateMaster,
        StreamRole::Video,
        "video/mp4",
    )?;
    let evidence = port(
        &request.outputs,
        "evidence",
        ArtifactKind::File,
        ArtifactRole::ValidationEvidence,
        StreamRole::TimedMetadata,
        "application/vnd.aniflow.destination-video-report+json",
    )?;
    let parent = output
        .parent()
        .ok_or_else(|| invalid("destination output needs a parent"))?;
    if evidence.parent() != Some(parent) || output == evidence {
        return Err(invalid(
            "destination outputs must be distinct fixed siblings",
        ));
    }
    let settings = &operation.intent.payload.request.configuration.tools;
    let source_pin = (
        operation.intent.payload.artifact.sha256.clone(),
        operation.intent.payload.artifact.byte_count,
    );
    if identity(source, settings.maximum_source_bytes)? != source_pin {
        return Err(invalid(
            "destination source differs from the exact accepted input",
        ));
    }
    for out in [output, evidence] {
        require_absent(out)?;
        for input in [
            source,
            settings.ffmpeg.executable.as_path(),
            settings.ffprobe.executable.as_path(),
        ] {
            if out.starts_with(input) || input.starts_with(out) {
                return Err(invalid("destination candidate overlaps source or tools"));
            }
        }
    }
    crate::frame_io::ensure_parent(parent)?;
    crate::video_assembly::require_storage(
        parent,
        operation.working_bytes()?,
        settings.minimum_free_bytes,
    )?;
    let observed = crate::video_assembly::observe_components(settings)?;
    let token = CancellationToken::default();
    let scratch = tempfile::Builder::new()
        .prefix(".destination-video-")
        .tempdir_in(parent)
        .map_err(|_| invalid("cannot reserve destination scratch"))?;
    let root = scratch
        .path()
        .canonicalize()
        .map_err(|_| invalid("cannot resolve destination scratch"))?;
    let result = (|| -> Result<()> {
        let private_source = root.join("source.mp4");
        copy_pinned(
            source,
            &private_source,
            &source_pin,
            settings.maximum_source_bytes,
            &token,
        )?;
        let mut private_settings = settings.clone();
        private_settings.ffmpeg.executable = root.join("ffmpeg");
        private_settings.ffprobe.executable = root.join("ffprobe");
        private_tool(
            &settings.ffmpeg.executable,
            &private_settings.ffmpeg.executable,
            &observed[0],
            &token,
        )?;
        private_tool(
            &settings.ffprobe.executable,
            &private_settings.ffprobe.executable,
            &observed[1],
            &token,
        )?;
        let candidate = root.join("candidate.mp4");
        let media = crate::video_assembly::media::destination::execute(
            &operation,
            &private_settings,
            &private_source,
            &candidate,
            &root,
            &token,
        )?;
        let pin = identity(&candidate, operation.output_limit())?;
        if pin.1 == 0
            || pin
                != (
                    media.candidate.source_sha256.clone(),
                    media.candidate.source_size_bytes,
                )
        {
            return Err(invalid(
                "destination candidate differs from complete native observations",
            ));
        }
        copy_pinned(&candidate, output, &pin, operation.output_limit(), &token)?;
        if identity(source, settings.maximum_source_bytes)? != source_pin
            || crate::video_assembly::observe_components(settings)? != observed
            || identity(&private_source, settings.maximum_source_bytes)? != source_pin
        {
            return Err(invalid(
                "destination source or tools changed before native report",
            ));
        }
        let report = VideoExportReport {
            schema: "aniflow.destination-video-report/v1".into(),
            operation: operation.clone(),
            provider_lock_sha256: request.provider_lock_sha256.clone(),
            output_sha256: pin.0,
            output_bytes: pin.1,
            media,
            quality_approval: "not_performed; lossy export requires separate final creative review"
                .into(),
            publication_authority: "none".into(),
        };
        let bytes = canonical_json_bytes(&report)?;
        if bytes.len() as u64 > REPORT_BYTES {
            return Err(invalid("destination native report exceeds 64 MiB"));
        }
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(evidence)
            .map_err(|_| invalid("cannot reserve destination report"))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| invalid("cannot sync destination report"))?;
        Ok(())
    })();
    if result.is_err() {
        let root: PathBuf = scratch.keep();
        let _=local::write(&root.join("failure.json"),b"{\"accepted\":false,\"cause\":\"unproven\",\"schema\":\"aniflow.destination-video-failure/v1\"}");
    }
    result
}
