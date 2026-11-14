use super::*;
use crate::frame_io::{copy_pinned, ensure_parent, require_absent};

pub(crate) fn execute_invocation(inv: &crate::ProviderInvocationRequest) -> Result<()> {
    inv.validate()?;
    let op: Operation = serde_json::from_value(
        serde_json::to_value(&inv.configuration.values)
            .map_err(|_| invalid("gain video operation encoding"))?,
    )
    .map_err(|_| invalid("closed gain video operation required"))?;
    op.validate()?;
    if !cfg!(unix)
        || inv.configuration != op.configuration()?
        || inv.stage_id != "gain_video"
        || inv.inputs.len() != 2
        || inv.outputs.len() != 2
    {
        return Err(invalid("outside fixed gain video native profile"));
    }
    let input = |port: &str,
                 id: &str,
                 kind: &str,
                 stream: crate::StreamRole|
     -> Result<&crate::invocation_v3::ProviderInvocationArtifactBinding> {
        let values = inv
            .inputs
            .iter()
            .filter(|i| i.port == port)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            return Err(invalid("gain video input missing/ambiguous"));
        }
        let i = values[0];
        if i.artifact_id != id
            || i.artifact_type != kind
            || i.kind != crate::ArtifactKind::File
            || i.artifact_role != crate::ArtifactRole::TemporalSource
            || i.stream_role != Some(stream)
        {
            return Err(invalid("gain video input contract differs"));
        }
        Ok(i)
    };
    let video = input(
        "video",
        "gain_video_source",
        "video/mp4",
        crate::StreamRole::Video,
    )?;
    let audio = input(
        "audio",
        "gain_video_audio",
        "audio/wav",
        crate::StreamRole::Audio,
    )?;
    let output = |port: &str,
                  id: &str,
                  kind: &str,
                  role: crate::ArtifactRole,
                  stream: crate::StreamRole|
     -> Result<&Path> {
        let values = inv
            .outputs
            .iter()
            .filter(|o| o.port == port)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            return Err(invalid("gain video output missing/ambiguous"));
        }
        let o = values[0];
        if o.artifact_id != id
            || o.artifact_type != kind
            || o.kind != crate::ArtifactKind::File
            || o.artifact_role != role
            || o.stream_role != Some(stream)
        {
            return Err(invalid("gain video output contract differs"));
        }
        Ok(&o.path)
    };
    let candidate = output(
        "media",
        "gain_video",
        "video/mp4",
        crate::ArtifactRole::CandidateMaster,
        crate::StreamRole::Video,
    )?;
    let evidence = output(
        "evidence",
        "gain_video_evidence",
        "application/vnd.aniflow.gain-video-report+json",
        crate::ArtifactRole::ValidationEvidence,
        crate::StreamRole::TimedMetadata,
    )?;
    let parent = candidate
        .parent()
        .ok_or_else(|| invalid("gain video parent missing"))?;
    if evidence.parent() != Some(parent) || candidate == evidence {
        return Err(invalid("gain video outputs must be distinct siblings"));
    }
    ensure_parent(parent)?;
    for output in [candidate, evidence] {
        require_absent(output)?;
        for source in [&video.path, &audio.path]
            .into_iter()
            .chain(op.components.iter().map(|c| &c.path))
        {
            if output.starts_with(source) || source.starts_with(output) {
                return Err(invalid("gain video output overlaps source/component"));
            }
        }
    }
    let token = CancellationToken::new();
    if current(&op.request, &token)? != op {
        return Err(invalid(
            "gain video source/QC/components changed before native execution",
        ));
    }
    let executable =
        std::env::current_exe().map_err(|_| invalid("gain video provider unavailable"))?;
    if identity(&executable, 134_217_728)?.0 != op.components[2].sha256 {
        return Err(invalid("gain video actual provider differs from pin"));
    }
    crate::video_assembly::require_storage(
        parent,
        op.working_bytes()?,
        op.request.settings.minimum_free_bytes,
    )?;
    let private = tempfile::Builder::new()
        .prefix(".gain-video-")
        .tempdir_in(parent)
        .map_err(|_| invalid("cannot reserve private gain video work"))?;
    let root = private
        .path()
        .canonicalize()
        .map_err(|_| invalid("cannot resolve private gain video work"))?;
    let result: Result<()> = (|| {
        let mut settings = op.request.settings.clone();
        settings.ffmpeg.executable = root.join("ffmpeg");
        settings.ffprobe.executable = root.join("ffprobe");
        for (component, path) in op.components[..2]
            .iter()
            .zip([&settings.ffmpeg.executable, &settings.ffprobe.executable])
        {
            copy_pinned(
                &component.path,
                path,
                &(component.sha256.clone(), component.byte_count),
                134_217_728,
                &token,
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| invalid("cannot enable private gain video component"))?;
            }
        }
        let vpin = (op.artifact.sha256.clone(), op.artifact.byte_count);
        let apin = (op.gain.file.sha256.clone(), op.gain.file.byte_count);
        let source = root.join("source.mp4");
        let gain = root.join("gain.wav");
        let out = root.join("video.mp4");
        copy_pinned(
            &video.path,
            &source,
            &vpin,
            settings.maximum_source_bytes,
            &token,
        )?;
        copy_pinned(&audio.path, &gain, &apin, 268_435_456, &token)?;
        let media = crate::video_assembly::media::gain_video::execute(
            &op, &settings, &source, &gain, &out, &root, &token,
        )?;
        let pin = identity(&out, settings.maximum_output_bytes)?;
        if pin
            != (
                media.candidate.source_sha256.clone(),
                media.candidate.source_size_bytes,
            )
            || identity(&video.path, settings.maximum_source_bytes)? != vpin
            || identity(&audio.path, 268_435_456)? != apin
            || current(&op.request, &token)? != op
        {
            return Err(invalid(
                "gain video source/QC/components changed during native work",
            ));
        }
        copy_pinned(&out, candidate, &pin, settings.maximum_output_bytes, &token)?;
        let report = Report {
            schema: "aniflow.gain-video-report/v1".into(),
            operation: op.clone(),
            provider_lock_sha256: inv.provider_lock_sha256.clone(),
            output_sha256: pin.0,
            output_bytes: pin.1,
            media,
            output_policy_status: "requires_fresh_encoded_file_qc".into(),
            creative_approval: "not_performed".into(),
            publication_authority: "none".into(),
        };
        let bytes = canonical_json_bytes(&report)?;
        if bytes.len() as u64 > REPORT_BYTES {
            return Err(invalid(
                "gain video evidence exceeds selected profile bound",
            ));
        }
        local::write(evidence, &bytes)?;
        local::sync(parent)
    })();
    if result.is_err() {
        let _retained = private.keep();
    }
    result
}
