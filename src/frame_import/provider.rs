use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use crate::audio_inspection::process::{GroupPolicy, run_tool_isolated, verify_pin};
use crate::audio_inspection::AudioToolObservation;
use crate::frame_io::{copy_pinned, ensure_parent, identity, inspect_png, inventory, require_absent};
use crate::provider::{canonical_json_bytes, canonical_sha256, require_sha256};
use crate::{ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationArtifactBinding, ProviderInvocationRequest, Result, StreamRole};
use super::*;

const MAXIMUM_TOOL_BYTES: u64 = 134_217_728;

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }

fn binding<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut selected = bindings.iter().filter(|binding| binding.port == port);
    let binding = selected.next().ok_or_else(|| refused("missing frame import port"))?;
    if selected.next().is_some() { return Err(refused("duplicate frame import port")); }
    Ok(binding)
}

fn configuration(request: &ProviderInvocationRequest) -> Result<FrameImportConfiguration> {
    request.validate()?;
    let config: FrameImportConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values).map_err(|_| refused("invalid frame import provider values"))?,
    ).map_err(|_| refused("invalid closed frame import configuration"))?;
    config.validate()?;
    if config.provider_configuration()? != request.configuration || request.inputs.len() != 3 || request.outputs.len() != 2 {
        return Err(refused("frame import requires its exact configuration and artifact ports"));
    }
    for (bindings, port, kind, role, stream, artifact_type) in [
        (&request.inputs, "source", ArtifactKind::File, ArtifactRole::TemporalSource, StreamRole::Video, "video/*"),
        (&request.inputs, "frames", ArtifactKind::Directory, ArtifactRole::Intermediate, StreamRole::Video, "application/vnd.aniflow.rife-frames"),
        (&request.inputs, "index", ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, "application/vnd.aniflow.rife-frame-index+json"),
        (&request.outputs, "frames", ArtifactKind::Directory, ArtifactRole::Intermediate, StreamRole::Video, "application/vnd.aniflow.rife-frames"),
        (&request.outputs, "evidence", ArtifactKind::File, ArtifactRole::ValidationEvidence, StreamRole::TimedMetadata, "application/vnd.aniflow.frame-import-report+json"),
    ] {
        let value = binding(bindings, port)?;
        if value.kind != kind || value.artifact_role != role || value.stream_role != Some(stream) || value.artifact_type != artifact_type {
            return Err(refused("frame import binding differs from its fixed artifact profile"));
        }
    }
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency, "frame import requires Unix process-group cancellation")); }
    Ok(config)
}

fn decode_arguments(snapshot: &Path) -> Vec<OsString> {
    // No frame limit, stream copy, resizing, or error recovery. All input PNG
    // data must be decoded; the closed PNG profile rejects APNG/extra chunks.
    ["-hide_banner", "-nostdin", "-v", "error", "-xerror", "-err_detect", "explode",
     "-threads", "1", "-protocol_whitelist", "file,pipe", "-f", "image2",
     "-pattern_type", "none", "-c:v", "png", "-i", "{snapshot}", "-map", "0:v:0",
     "-an", "-sn", "-dn", "-c:v", "rawvideo", "-pix_fmt", "+rgb24", "-threads", "1",
     "-f", "framehash", "-format_version", "2", "-hash", "sha256", "pipe:1"]
        .into_iter().map(|value| if value == "{snapshot}" { snapshot.as_os_str().to_owned() } else { value.into() }).collect()
}

fn decoded_hash(bytes: &[u8], width: u32, height: u32, expected_bytes: u64) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| refused("decoder framehash is not UTF-8"))?;
    let required = ["#format: frame checksums".to_owned(), "#version: 2".to_owned(), "#hash: SHA256".to_owned(),
        "#media_type 0: video".to_owned(), "#codec_id 0: rawvideo".to_owned(), format!("#dimensions 0: {width}x{height}")];
    let mut headers = BTreeSet::new();
    let mut header_keys = BTreeSet::new();
    let mut hash = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with('#') {
            let key = if required.iter().any(|header| header == line) {
                line.split(':').next().expect("fixed header key")
            } else if line.starts_with("#software:") { "#software" }
            else if line.starts_with("#tb 0:") { "#tb 0" }
            else if matches!(line, "#sar 0: 0/1" | "#sar 0: 1/1") { "#sar 0" }
            else if line.strip_prefix('#').is_some_and(|value| value.split(',').map(str::trim).eq(["stream#", "dts", "pts", "duration", "size", "hash"])) { "#columns" }
            else { return Err(refused("decoder returned an unsupported framehash header")); };
            if hash.is_some() || !header_keys.insert(key.to_owned()) || !headers.insert(line.to_owned()) {
                return Err(refused("decoder returned duplicate or misplaced framehash headers"));
            }
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if hash.is_some() || fields.len() != 6 || fields[0] != "0"
            || fields[1].parse::<i64>().is_err() || fields[2].parse::<i64>().is_err()
            || fields[3].parse::<u64>().ok().is_none_or(|value| value == 0)
            || fields[4].parse::<u64>().ok() != Some(expected_bytes)
        { return Err(refused("decoder must return exactly one complete RGB24 frame of the declared size")); }
        require_sha256(fields[5], "decoded RGB24 SHA-256")?;
        hash = Some(fields[5].to_owned());
    }
    if required.iter().any(|header| !headers.contains(header)) {
        return Err(refused("decoder framehash profile or geometry differs"));
    }
    hash.ok_or_else(|| refused("decoder returned no complete frame"))
}

/// Native ABI implementation; the outer runtime owns cancellation, immutable
/// promotion, checkpoint authority and failed candidate retention.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let config = configuration(request)?;
    let source_path = &binding(&request.inputs, "source")?.path;
    let input_frames = &binding(&request.inputs, "frames")?.path;
    let index_path = &binding(&request.inputs, "index")?.path;
    let output_frames = &binding(&request.outputs, "frames")?.path;
    let output_evidence = &binding(&request.outputs, "evidence")?.path;
    let (index, index_identity) = read_index(index_path)?;
    let decoded_bytes = admit_geometry(&config, &index)?;
    let source_identity = identity(source_path, config.maximum_source_bytes)?;
    if source_identity.0 != index.temporal_inspection.source_sha256
        || source_identity.1 == 0 || source_identity.1 != index.temporal_inspection.source_size_bytes
    { return Err(refused("source video identity differs from the declared temporal inspection")); }
    let names: Vec<OsString> = index.frames.iter().map(|frame| frame.filename.clone().into()).collect();
    inventory(input_frames, &names)?;
    let mut observations = Vec::with_capacity(index.frames.len());
    let mut total = 0_u64;
    for frame in &index.frames {
        let observed = inspect_png(&input_frames.join(&frame.filename), index.width, index.height, config.maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(refused("PNG bytes differ from the declared frame digest")); }
        total = total.checked_add(observed.1).filter(|value| *value <= config.maximum_total_frame_bytes)
            .ok_or_else(|| refused("frame workspace exceeds the aggregate byte bound"))?;
        observations.push(observed);
    }
    verify_pin(&config.ffmpeg, "ffmpeg").map_err(|_| Error::new(ErrorCategory::Dependency, "frame import decoder pin changed"))?;
    let tool_identity = identity(&config.ffmpeg.executable, MAXIMUM_TOOL_BYTES)?;
    let parent = output_frames.parent().ok_or_else(|| refused("frame import outputs need a workspace parent"))?;
    if output_evidence.parent() != Some(parent) || output_frames == output_evidence {
        return Err(refused("frame import outputs must be distinct siblings"));
    }
    for input in [source_path, input_frames, index_path, &config.ffmpeg.executable] {
        if output_frames.starts_with(input) || input.starts_with(output_frames)
            || output_evidence.starts_with(input) || input.starts_with(output_evidence)
        { return Err(refused("frame import outputs must not overlap any source")); }
    }
    ensure_parent(parent)?;
    require_absent(output_frames)?;
    require_absent(output_evidence)?;
    // Candidate outputs are exclusively created here. A partial candidate is
    // deliberately retained on error; only the outer runtime can accept it.
    std::fs::create_dir(output_frames).map_err(|_| refused("cannot exclusively reserve frame import output"))?;
    let private = tempfile::Builder::new().prefix(".frame-import-decoder-").tempdir_in(parent)
        .map_err(|_| refused("cannot reserve private frame decoder directory"))?;
    let private_root = private.path().canonicalize().map_err(|_| refused("cannot resolve private decoder directory"))?;
    let cancellation = CancellationToken::default();
    let mut decoder = config.ffmpeg.clone();
    decoder.executable = private_root.join("ffmpeg");
    copy_pinned(&config.ffmpeg.executable, &decoder.executable, &tool_identity, MAXIMUM_TOOL_BYTES, &cancellation)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&decoder.executable, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| refused("cannot make the private decoder executable"))?;
    }
    let limits = config.tool_limits();
    let version = run_tool_isolated(&decoder, "ffmpeg", &["-version".into()], &limits, &cancellation, GroupPolicy::Inherit, Some(&private_root))
        .map_err(|diagnostic| Error::new(ErrorCategory::Dependency, format!("{:?}: {}", diagnostic.code, diagnostic.message)))?;
    let version_text = std::str::from_utf8(&version.stdout).map_err(|_| refused("decoder version banner is not UTF-8"))?;
    let tokens: Vec<_> = version_text.lines().next().unwrap_or_default().split_whitespace().take(3).collect();
    if tokens != ["ffmpeg", "version", config.ffmpeg.version.as_str()] { return Err(refused("decoder version differs from its exact configured token")); }
    let mut report = FrameImportReport {
        schema: FRAME_IMPORT_REPORT_SCHEMA_V1.to_owned(), stage_id: request.stage_id.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(), configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        source_sha256: source_identity.0.clone(), source_byte_count: source_identity.1,
        frame_index_sha256: canonical_sha256(&index)?, index: index.clone(),
        historical_processing: "unknown".to_owned(), source_relationship: "declared_not_verified".to_owned(),
        decode_method: "ffmpeg_png_rgb24_framehash_sha256".to_owned(),
        decoder: AudioToolObservation { id:"ffmpeg".to_owned(), version:config.ffmpeg.version.clone(), sha256:config.ffmpeg.sha256.clone() },
        frames: Vec::with_capacity(index.frames.len()),
    };
    for (frame, observed) in index.frames.iter().zip(&observations) {
        let snapshot = output_frames.join(&frame.filename);
        copy_pinned(&input_frames.join(&frame.filename), &snapshot, observed, config.maximum_frame_bytes, &cancellation)?;
        let capture = run_tool_isolated(&decoder, "ffmpeg", &decode_arguments(&snapshot), &limits, &cancellation, GroupPolicy::Inherit, Some(&private_root))
            .map_err(|diagnostic| refused(format!("frame {} decode failed ({:?}): {}", frame.ordinal, diagnostic.code, diagnostic.message)))?;
        if !capture.stderr.is_empty() { return Err(refused("PNG decoder emitted an error diagnostic")); }
        let decoded_rgb24_sha256 = decoded_hash(&capture.stdout, index.width, index.height, decoded_bytes)?;
        if inspect_png(&snapshot, index.width, index.height, config.maximum_frame_bytes)? != *observed {
            return Err(refused("private PNG snapshot changed during decode"));
        }
        report.frames.push(ImportedFrameObservation { ordinal:frame.ordinal, filename:frame.filename.clone(), sha256:observed.0.clone(), byte_count:observed.1, decoded_rgb24_sha256, decoded_byte_count:decoded_bytes });
    }
    inventory(input_frames, &names)?;
    inventory(output_frames, &names)?;
    inventory(&private_root, &["ffmpeg".into()])?;
    for (frame, observed) in index.frames.iter().zip(&observations) {
        if identity(&input_frames.join(&frame.filename), config.maximum_frame_bytes)? != *observed
            || identity(&output_frames.join(&frame.filename), config.maximum_frame_bytes)? != *observed
        { return Err(refused("source or imported frame changed before acceptance")); }
    }
    if identity(source_path, config.maximum_source_bytes)? != source_identity
        || identity(index_path, RIFE_MAXIMUM_INDEX_BYTES)? != index_identity
        || identity(&config.ffmpeg.executable, MAXIMUM_TOOL_BYTES)? != tool_identity
        || identity(&decoder.executable, MAXIMUM_TOOL_BYTES)? != tool_identity
    { return Err(refused("source, index or decoder changed before import acceptance")); }
    let bytes = canonical_json_bytes(&report)?;
    if bytes.len() as u64 > FRAME_IMPORT_MAXIMUM_REPORT_BYTES { return Err(refused("frame import evidence exceeds 64 MiB")); }
    let mut output = OpenOptions::new().create_new(true).write(true).open(output_evidence)
        .map_err(|_| refused("cannot exclusively publish frame import evidence"))?;
    output.write_all(&bytes).and_then(|()| output.sync_all()).map_err(|_| refused("cannot synchronize frame import evidence"))?;
    Ok(())
}
