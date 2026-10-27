//! Serial, bounded frame-pair execution using the existing provider process runner.
//! The admission estimate is not an OS-enforced RSS/VRAM quota or a sandbox.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::audio_inspection::process::{
    GroupPolicy, open_regular, run_tool_isolated, verify_pin,
};
use crate::audio_inspection::{
    AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1, AudioInspectionConfiguration, AudioToolPin,
};
use crate::{
    ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, Result, StreamRole,
};

use super::types::*;
use crate::frame_io::{copy_pinned, ensure_canonical_directory, ensure_parent, identity, inspect_png, inventory, require_absent};

const MAXIMUM_TOOL_BYTES: u64 = 128 * 1024 * 1024;
const MAXIMUM_MODEL_PARAM_BYTES: u64 = 16 * 1024 * 1024;
const MAXIMUM_MODEL_BIN_BYTES: u64 = 256 * 1024 * 1024;

fn fail(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn dependency(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Dependency, message)
}

fn private_workspace_inventory(directory: &Path) -> Result<()> {
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|_| fail("cannot inspect private RIFE workspace"))?
    {
        let entry = entry.map_err(|_| fail("cannot inspect private RIFE workspace entry"))?;
        if names.len() >= 3
            || !entry.file_type().map_err(|_| fail("cannot inspect RIFE workspace type"))?.is_dir()
        {
            return Err(fail("RIFE created an undeclared private workspace entry"));
        }
        names.insert(entry.file_name());
    }
    let expected = [OsString::from("tool"), OsString::from("rife-v4.6"), OsString::from("frames")]
        .into_iter().collect::<BTreeSet<_>>();
    if names != expected {
        return Err(fail("private RIFE workspace inventory differs"));
    }
    Ok(())
}

fn pair_arguments(left: &Path, right: &Path, middle: &Path, model: &Path, device: i32) -> Vec<OsString> {
    // The pinned upstream exposes only these short spellings for these options.
    vec![
        "-0".into(), left.as_os_str().to_owned(),
        "-1".into(), right.as_os_str().to_owned(),
        "-o".into(), middle.as_os_str().to_owned(),
        "-m".into(), model.as_os_str().to_owned(),
        "-s".into(), "0.5".into(),
        "-g".into(), device.to_string().into(),
        "-j".into(), "1:1:1".into(),
    ]
}

fn publish_json(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| fail("cannot exclusively publish RIFE evidence"))?;
    output.write_all(bytes)
        .and_then(|()| output.sync_all())
        .map_err(|_| fail("cannot synchronize complete RIFE evidence"))
}

fn selected<'a>(bindings: &'a [ProviderInvocationArtifactBinding], port: &str) -> Result<&'a ProviderInvocationArtifactBinding> {
    let mut matches = bindings.iter().filter(|binding| binding.port == port);
    let selected = matches.next().ok_or_else(|| fail("missing RIFE artifact port"))?;
    if matches.next().is_some() {
        return Err(fail("duplicate RIFE artifact port"));
    }
    Ok(selected)
}

fn verify_invocation(request: &ProviderInvocationRequest) -> Result<RifeProviderConfiguration> {
    request.validate()?;
    let configuration: RifeProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| fail("cannot read RIFE provider configuration values"))?,
    ).map_err(|_| fail("invalid closed RIFE provider configuration"))?;
    configuration.validate()?;
    if configuration.provider_configuration()? != request.configuration {
        return Err(fail("RIFE invocation must bind its exact effective configuration"));
    }
    if request.inputs.len() != 2 || request.outputs.len() != 2 {
        return Err(fail("RIFE requires exactly two inputs and two outputs"));
    }
    for (bindings, index_type) in [
        (&request.inputs, "application/vnd.aniflow.rife-frame-index+json"),
        (&request.outputs, "application/vnd.aniflow.rife-output-index+json"),
    ] {
        let frames = selected(bindings, "frames")?;
        let index = selected(bindings, "index")?;
        if frames.artifact_type != "application/vnd.aniflow.rife-frames"
            || frames.artifact_role != ArtifactRole::Intermediate
            || frames.stream_role != Some(StreamRole::Video)
            || frames.kind != ArtifactKind::Directory
            || index.artifact_type != index_type
            || index.artifact_role != ArtifactRole::ValidationEvidence
            || index.stream_role != Some(StreamRole::TimedMetadata)
            || index.kind != ArtifactKind::File
        {
            return Err(fail("RIFE artifact binding differs from the declared frame profile"));
        }
    }
    if !cfg!(unix)
        || configuration.configuration.host.operating_system != std::env::consts::OS
        || configuration.configuration.host.architecture != std::env::consts::ARCH
    {
        return Err(dependency("RIFE execution must match its declared Unix host OS and architecture"));
    }
    if !configuration.configuration.enabled {
        return Err(fail("disabled RIFE stages must be skipped before provider launch"));
    }
    Ok(configuration)
}

fn read_index(path: &Path) -> Result<(RifeFrameIndex, (String, u64))> {
    let expected = identity(path, RIFE_MAXIMUM_INDEX_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(path)
        .map_err(|_| fail("cannot safely open RIFE frame index"))?
        .take(RIFE_MAXIMUM_INDEX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("cannot read RIFE frame index"))?;
    if bytes.len() as u64 > RIFE_MAXIMUM_INDEX_BYTES
        || (format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64) != expected
    {
        return Err(fail("RIFE frame index changed or exceeds its byte limit"));
    }
    let index: RifeFrameIndex = serde_json::from_slice(&bytes)
        .map_err(|_| fail("invalid closed RIFE frame index"))?;
    Ok((index, expected))
}

fn output_frame_template(plan: &RifePlan) -> Result<Vec<RifeOutputFrame>> {
    super::validate_mapping_plan(plan)?;
    let capacity = usize::try_from(plan.output_frame_count)
        .map_err(|_| fail("RIFE output count exceeds host addressable size"))?;
    let mut frames = Vec::with_capacity(capacity);
    for ordinal in 0..plan.output_frame_count {
        let (left, right, origin) = super::mapped_output_frame(plan, ordinal)?;
        frames.push(RifeOutputFrame {
            ordinal,
            filename: format!("frame_{ordinal:08}.png"),
            sha256: "0".repeat(64),
            source_left_ordinal: left,
            source_right_ordinal: right,
            origin,
        });
    }
    Ok(frames)
}

fn checked_sum(values: &[u64]) -> Result<u64> {
    values.iter().try_fold(0_u64, |sum, value| sum.checked_add(*value)
        .ok_or_else(|| fail("RIFE storage arithmetic overflow")))
}

fn frame_byte_bound(index: &RifeFrameIndex, settings: &RifeSettings) -> Result<u64> {
    // RGB scanlines plus conservative encoder overhead; no all-frame decode buffer.
    let raw = u64::from(index.width)
        .checked_mul(3)
        .and_then(|row| row.checked_add(1))
        .and_then(|row| row.checked_mul(u64::from(index.height)))
        .and_then(|raw| raw.checked_mul(2))
        .and_then(|raw| raw.checked_add(65_536))
        .ok_or_else(|| fail("RIFE PNG storage arithmetic overflow"))?;
    Ok(raw.min(settings.maximum_frame_bytes))
}

fn model_identities(settings: &RifeSettings, directory: &Path) -> Result<[(PathBuf, (String, u64), u64); 2]> {
    ensure_canonical_directory(directory)?;
    inventory(directory, &["flownet.param".into(), "flownet.bin".into()])?;
    let pins = [
        (directory.join("flownet.param"), (settings.model.param_sha256.clone(), settings.model.param_bytes), MAXIMUM_MODEL_PARAM_BYTES),
        (directory.join("flownet.bin"), (settings.model.bin_sha256.clone(), settings.model.bin_bytes), MAXIMUM_MODEL_BIN_BYTES),
    ];
    for (path, expected, maximum) in &pins {
        if identity(path, *maximum)? != *expected {
            return Err(dependency("RIFE model bytes differ from their explicit pins"));
        }
    }
    Ok(pins)
}

fn tool_limits(settings: &RifeSettings, pin: &AudioToolPin) -> AudioInspectionConfiguration {
    // The existing runner only reads these timeout/capture settings and selected pin.
    AudioInspectionConfiguration {
        schema: AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.to_owned(),
        ffmpeg: pin.clone(),
        ffprobe: pin.clone(),
        tool_timeout_milliseconds: settings.tool_timeout_milliseconds,
        maximum_tool_output_bytes: settings.maximum_tool_output_bytes,
    }
}

/// Execute an explicitly bound frame-directory provider invocation. This is the
/// serial frame subset; no source-video extraction, encoding or audio mux occurs.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let wrapper = verify_invocation(request)?;
    let configuration = &wrapper.configuration;
    let settings = &configuration.settings;
    let frames_input = &selected(&request.inputs, "frames")?.path;
    let index_input = &selected(&request.inputs, "index")?.path;
    let frames_output = &selected(&request.outputs, "frames")?.path;
    let index_output = &selected(&request.outputs, "index")?.path;
    ensure_canonical_directory(frames_input)?;
    let (index, index_identity) = read_index(index_input)?;
    let plan = super::plan(configuration, &index)?;
    let mut report = RifeOutputIndex {
        schema: RIFE_OUTPUT_INDEX_SCHEMA_V1.to_owned(),
        frames: output_frame_template(&plan)?,
        plan,
    };
    let template_bytes = crate::provider::canonical_json_bytes(&report)?;
    if template_bytes.len() as u64 > RIFE_MAXIMUM_INDEX_BYTES {
        return Err(fail("RIFE output evidence exceeds its bounded index size"));
    }
    let index_output_bytes = template_bytes.len() as u64;
    drop(template_bytes);
    let maximum_frame_bytes = frame_byte_bound(&index, settings)?;
    let source_names = index.frames.iter().map(|frame| OsString::from(&frame.filename))
        .collect::<Vec<_>>();
    inventory(frames_input, &source_names)?;
    let mut sources = Vec::with_capacity(index.frames.len());
    let mut source_bytes = 0_u64;
    for frame in &index.frames {
        let path = frames_input.join(&frame.filename);
        let observed = inspect_png(&path, index.width, index.height, maximum_frame_bytes)?;
        if observed.0 != frame.sha256 {
            return Err(fail("RIFE source PNG differs from its frame index digest"));
        }
        source_bytes = checked_sum(&[source_bytes, observed.1])?;
        if source_bytes > settings.maximum_artifact_bytes {
            return Err(fail("RIFE input frames exceed the aggregate storage bound"));
        }
        sources.push((path, observed));
    }
    let original_models = model_identities(settings, &settings.model.directory)?;
    verify_pin(&settings.tool, "rife-ncnn-vulkan")
        .map_err(|error| dependency(format!("{:?}: {}", error.code, error.message)))?;
    let original_tool = identity(&settings.tool.executable, MAXIMUM_TOOL_BYTES)?;
    let output_reservation = maximum_frame_bytes.checked_mul(report.plan.output_frame_count)
        .ok_or_else(|| fail("RIFE output storage reservation overflow"))?;
    let pair_reservation = maximum_frame_bytes.checked_mul(3)
        .ok_or_else(|| fail("RIFE pair storage reservation overflow"))?;
    let reserved = checked_sum(&[
        source_bytes,
        output_reservation,
        pair_reservation,
        original_tool.1,
        settings.model.param_bytes,
        settings.model.bin_bytes,
        index_identity.1,
        index_output_bytes,
    ])?;
    if reserved > settings.maximum_artifact_bytes {
        return Err(fail("RIFE bounded input/output/model/pair storage reservation exceeds maximum_artifact_bytes"));
    }
    let parent = frames_output.parent()
        .ok_or_else(|| fail("RIFE outputs require an assigned parent workspace"))?;
    if index_output.parent() != Some(parent) {
        return Err(fail("RIFE frame and index outputs must share their assigned parent"));
    }
    if frames_output.starts_with(frames_input)
        || frames_input.starts_with(frames_output)
        || index_input.starts_with(frames_output)
        || frames_output.starts_with(&settings.model.directory)
        || index_output.starts_with(&settings.model.directory)
        || frames_output == settings.tool.executable
        || index_output == settings.tool.executable
        || index_output == index_input
        || frames_output == index_output
    {
        return Err(fail("RIFE outputs must be distinct from source inputs"));
    }
    ensure_parent(parent)?;
    require_absent(frames_output)?;
    require_absent(index_output)?;
    let workspace = tempfile::Builder::new().prefix(".rife-").tempdir_in(parent)
        .map_err(|_| fail("cannot reserve private RIFE workspace"))?;
    let root = workspace.path().canonicalize()
        .map_err(|_| fail("cannot resolve private RIFE workspace"))?;
    let tool_directory = root.join("tool");
    let model_directory = root.join("rife-v4.6");
    let output_directory = root.join("frames");
    for directory in [&tool_directory, &model_directory, &output_directory] {
        std::fs::create_dir(directory).map_err(|_| fail("cannot create private RIFE directory"))?;
    }
    let cancellation = CancellationToken::default();
    let mut tool = settings.tool.clone();
    tool.executable = tool_directory.join("rife-ncnn-vulkan");
    copy_pinned(&settings.tool.executable, &tool.executable, &original_tool,
        MAXIMUM_TOOL_BYTES, &cancellation)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&tool.executable, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| dependency("cannot make private RIFE executable runnable"))?;
    }
    for (path, expected, maximum) in &original_models {
        let name = path.file_name().expect("fixed RIFE model filename");
        copy_pinned(path, &model_directory.join(name), expected, *maximum, &cancellation)?;
    }
    for frame in &mut report.frames {
        let left = usize::try_from(frame.source_left_ordinal)
            .map_err(|_| fail("RIFE source ordinal exceeds host addressability"))?;
        let right = usize::try_from(frame.source_right_ordinal)
            .map_err(|_| fail("RIFE source ordinal exceeds host addressability"))?;
        let (source_path, source_identity) = sources.get(left)
            .ok_or_else(|| fail("RIFE mapped source frame is missing"))?;
        let destination = output_directory.join(&frame.filename);
        let observed = if frame.origin == RifeFrameOrigin::InterpolatedMidpoint {
            let (right_path, right_identity) = sources.get(right)
                .ok_or_else(|| fail("RIFE mapped right source frame is missing"))?;
            let pair = tempfile::Builder::new().prefix("pair-").tempdir_in(&root)
                .map_err(|_| fail("cannot reserve bounded RIFE pair workspace"))?;
            let pair_root = pair.path().canonicalize()
                .map_err(|_| fail("cannot resolve RIFE pair workspace"))?;
            let left_snapshot = pair_root.join("left.png");
            let right_snapshot = pair_root.join("right.png");
            let middle = pair_root.join("middle.png");
            copy_pinned(source_path, &left_snapshot, source_identity, maximum_frame_bytes, &cancellation)?;
            copy_pinned(right_path, &right_snapshot, right_identity, maximum_frame_bytes, &cancellation)?;
            model_identities(settings, &model_directory)?;
            run_tool_isolated(
                &tool,
                "rife-ncnn-vulkan",
                &pair_arguments(&left_snapshot, &right_snapshot, &middle, &model_directory, settings.device_id),
                &tool_limits(settings, &tool),
                &cancellation,
                GroupPolicy::Inherit,
                Some(&pair_root),
            ).map_err(|error| dependency(format!("{:?}: {}", error.code, error.message)))?;
            inventory(&pair_root, &["left.png".into(), "right.png".into(), "middle.png".into()])?;
            if identity(&left_snapshot, maximum_frame_bytes)? != *source_identity
                || identity(&right_snapshot, maximum_frame_bytes)? != *right_identity
            {
                return Err(fail("RIFE modified an immutable pair snapshot"));
            }
            model_identities(settings, &model_directory)?;
            let observed = inspect_png(&middle, index.width, index.height, maximum_frame_bytes)?;
            copy_pinned(&middle, &destination, &observed, maximum_frame_bytes, &cancellation)?;
            observed
        } else {
            copy_pinned(source_path, &destination, source_identity, maximum_frame_bytes, &cancellation)?;
            source_identity.clone()
        };
        frame.sha256 = observed.0;
    }
    inventory(&tool_directory, &["rife-ncnn-vulkan".into()])?;
    private_workspace_inventory(&root)?;
    if identity(&tool.executable, MAXIMUM_TOOL_BYTES)? != original_tool
        || identity(&settings.tool.executable, MAXIMUM_TOOL_BYTES)? != original_tool
        || identity(index_input, RIFE_MAXIMUM_INDEX_BYTES)? != index_identity
    {
        return Err(fail("RIFE pinned tool or source index changed during execution"));
    }
    model_identities(settings, &settings.model.directory)?;
    model_identities(settings, &model_directory)?;
    inventory(frames_input, &source_names)?;
    for (path, expected) in &sources {
        if identity(path, maximum_frame_bytes)? != *expected {
            return Err(fail("RIFE source frame changed during execution"));
        }
    }
    let output_names = report.frames.iter().map(|frame| OsString::from(&frame.filename))
        .collect::<Vec<_>>();
    inventory(&output_directory, &output_names)?;
    for frame in &report.frames {
        if inspect_png(&output_directory.join(&frame.filename), index.width, index.height, maximum_frame_bytes)?.0 != frame.sha256 {
            return Err(fail("RIFE output frame changed before publication"));
        }
    }
    report.validate_against(configuration, &index)?;
    let bytes = crate::provider::canonical_json_bytes(&report)?;
    if bytes.len() as u64 != index_output_bytes {
        return Err(fail("RIFE evidence exceeded its exact reserved size"));
    }
    // Exclusive evidence creation is separate from directory publication; the
    // outer provider runtime accepts the pair only after both outputs validate.
    publish_json(index_output, &bytes)?;
    require_absent(frames_output)?;
    std::fs::rename(&output_directory, frames_output)
        .map_err(|_| fail("cannot publish complete RIFE frame directory"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(bytes: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(payload);
        let crc = crate::frame_io::crc32_update(crate::frame_io::crc32_update(u32::MAX, kind), payload);
        bytes.extend_from_slice(&(!crc).to_be_bytes());
    }

    /// Synthetic one-pixel RGB8 PNG with a valid uncompressed DEFLATE stream.
    fn png(color: [u8; 3]) -> Vec<u8> {
        let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut header = Vec::new();
        header.extend_from_slice(&1_u32.to_be_bytes());
        header.extend_from_slice(&1_u32.to_be_bytes());
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut bytes, b"IHDR", &header);
        let scanline = [0, color[0], color[1], color[2]];
        let mut deflate = vec![0x78, 0x01, 0x01, 4, 0, 0xfb, 0xff];
        deflate.extend_from_slice(&scanline);
        let mut sum_a = 1_u32;
        let mut sum_b = 0_u32;
        for byte in scanline {
            sum_a = (sum_a + u32::from(byte)) % 65_521;
            sum_b = (sum_b + sum_a) % 65_521;
        }
        deflate.extend_from_slice(&((sum_b << 16) | sum_a).to_be_bytes());
        chunk(&mut bytes, b"IDAT", &deflate);
        chunk(&mut bytes, b"IEND", &[]);
        bytes
    }

    #[test]
    fn serial_pair_arguments_select_one_device_and_no_directory_tta_or_uhd_mode() {
        let values = pair_arguments(Path::new("/left.png"), Path::new("/right.png"),
            Path::new("/middle.png"), Path::new("/rife-v4.6"), -1)
            .into_iter().map(|value| value.into_string().unwrap()).collect::<Vec<_>>();
        assert_eq!(values, ["-0", "/left.png", "-1", "/right.png", "-o", "/middle.png",
            "-m", "/rife-v4.6", "-s", "0.5", "-g", "-1", "-j", "1:1:1"]);
    }

    #[test]
    fn png_structure_requires_bounded_exact_rgb8_geometry_checksums_and_no_trailing_bytes() {
        assert_eq!(!crate::frame_io::crc32_update(u32::MAX, b"123456789"), 0xcbf4_3926);
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("frame.png");
        let valid = png([1, 2, 3]);
        std::fs::write(&path, &valid).unwrap();
        assert_eq!(inspect_png(&path, 1, 1, 1024).unwrap().1, valid.len() as u64);
        assert!(inspect_png(&path, 2, 1, 1024).is_err());
        assert!(inspect_png(&path, 1, 1, valid.len() as u64 - 1).is_err());
        let mut changed = valid.clone();
        changed[29] ^= 1;
        std::fs::write(&path, &changed).unwrap();
        assert!(inspect_png(&path, 1, 1, 1024).is_err());
        let mut changed = valid;
        changed.push(0);
        std::fs::write(&path, &changed).unwrap();
        assert!(inspect_png(&path, 1, 1, 1024).is_err());
    }

    #[test]
    fn structurally_valid_profile_or_animation_chunks_are_explicitly_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap().join("frame.png");
        for kind in [b"iCCP", b"acTL", b"tEXt"] {
            let mut valid = png([1, 2, 3]);
            let mut extra = Vec::new();
            chunk(&mut extra, kind, b"synthetic");
            valid.splice(33..33, extra);
            std::fs::write(&path, valid).unwrap();
            assert!(inspect_png(&path, 1, 1, 1024).is_err());
        }
    }

    #[test]
    fn valid_header_checksums_do_not_admit_alpha_high_depth_or_interlaced_frames() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap().join("frame.png");
        for (offset, value) in [(24, 16_u8), (25, 6_u8), (28, 1_u8)] {
            let mut bytes = png([1, 2, 3]);
            bytes[offset] = value;
            let crc = !crate::frame_io::crc32_update(u32::MAX, &bytes[12..29]);
            bytes[29..33].copy_from_slice(&crc.to_be_bytes());
            std::fs::write(&path, bytes).unwrap();
            assert!(inspect_png(&path, 1, 1, 1024).unwrap_err().message().contains("RGB8"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn special_symlink_and_parent_escape_inputs_are_rejected_without_opening_them() {
        use std::os::unix::fs::symlink;
        use nix::sys::stat::Mode;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("real.png");
        std::fs::write(&source, png([1, 2, 3])).unwrap();
        let linked = root.join("linked.png");
        symlink(&source, &linked).unwrap();
        assert!(inspect_png(&linked, 1, 1, 1024).is_err());
        let fifo = root.join("fifo.png");
        nix::unistd::mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
        assert!(inspect_png(&fifo, 1, 1, 1024).is_err());
        assert!(identity(&root.join("child/../real.png"), 1024).is_err());
        assert!(inventory(&root, &["real.png".into()]).is_err());
    }

    #[test]
    fn staging_refuses_changed_pins_and_does_not_replace_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("source");
        let output = root.join("output");
        std::fs::write(&source, b"synthetic source").unwrap();
        let expected = identity(&source, 1024).unwrap();
        std::fs::write(&source, b"changed source").unwrap();
        assert!(copy_pinned(&source, &output, &expected, 1024, &CancellationToken::default()).is_err());
        assert!(!output.exists());
        let expected = identity(&source, 1024).unwrap();
        std::fs::write(&output, b"existing").unwrap();
        assert!(copy_pinned(&source, &output, &expected, 1024, &CancellationToken::default()).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"existing");
    }

    #[cfg(unix)]
    struct Fixture {
        _directory: tempfile::TempDir,
        request: ProviderInvocationRequest,
        index: RifeFrameIndex,
    }

    #[cfg(unix)]
    fn make_fixture(script: &str, cuts: Vec<u64>) -> Fixture {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source_frames = root.join("source_frames");
        let models = root.join("rife-v4.6");
        std::fs::create_dir(&source_frames).unwrap();
        std::fs::create_dir(&models).unwrap();
        let param = models.join("flownet.param");
        let bin = models.join("flownet.bin");
        std::fs::write(&param, b"synthetic model param bytes").unwrap();
        std::fs::write(&bin, b"synthetic model bin bytes").unwrap();
        let tool = root.join("synthetic-rife");
        std::fs::write(&tool, script).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut frames = Vec::new();
        for ordinal in 0..2_u64 {
            let filename = format!("source_{ordinal:08}.png");
            let bytes = png([ordinal as u8, 2, 3]);
            std::fs::write(source_frames.join(&filename), &bytes).unwrap();
            frames.push(RifeFrame { ordinal, filename, sha256: format!("{:x}", Sha256::digest(bytes)) });
        }
        let temporal = crate::temporal::from_probe_documents(&"a".repeat(64), 100,
            &crate::temporal::StreamSelection::default(),
            &serde_json::json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
                "time_base":"1/24","start_pts":0,"duration_ts":2,"r_frame_rate":"24/1",
                "avg_frame_rate":"24/1","width":1,"height":1,"pix_fmt":"yuv420p",
                "disposition":{"attached_pic":0}}]}),
            &[(0, serde_json::json!({"frames":[
                {"stream_index":0,"pts":0,"pkt_dts":0,"pkt_duration":1,"key_frame":1},
                {"stream_index":0,"pts":1,"pkt_dts":1,"pkt_duration":1,"key_frame":0}]}),
                serde_json::json!({"packets":[
                {"stream_index":0,"pts":0,"dts":0,"duration":1},
                {"stream_index":0,"pts":1,"dts":1,"duration":1}]}))]).unwrap();
        let index = RifeFrameIndex {
            schema: RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection: temporal,
            width:1, height:1, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(),
            frames, cut_before_ordinals:cuts,
        };
        let index_input = root.join("source-index.json");
        std::fs::write(&index_input, serde_json::to_vec(&index).unwrap()).unwrap();
        let (param_sha256, param_bytes) = identity(&param, 1024).unwrap();
        let (bin_sha256, bin_bytes) = identity(&bin, 1024).unwrap();
        let wrapper = RifeProviderConfiguration {
            schema:RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
            configuration:RifeConfiguration {
                schema:RIFE_CONFIGURATION_SCHEMA_V1.to_owned(), enabled:true,
                host:RifeHost {
                    operating_system:std::env::consts::OS.to_owned(),
                    architecture:std::env::consts::ARCH.to_owned(),
                    total_memory_mib:16_384, available_memory_mib:12_288,
                },
                settings:RifeSettings {
                    tool:AudioToolPin { executable:tool.clone(), version:"1.0.0".to_owned(), sha256:identity(&tool, MAXIMUM_TOOL_BYTES).unwrap().0 },
                    model:RifeModelPin { directory:models, param_sha256, param_bytes, bin_sha256, bin_bytes },
                    preset:RifePreset::Conservative, device_id:-1,
                    tool_timeout_milliseconds:120_000, maximum_tool_output_bytes:1024,
                    maximum_frame_bytes:1024, maximum_artifact_bytes:1_048_576,
                },
            },
        };
        let request = ProviderInvocationRequest::new_artifact_set("rife", "b".repeat(64),
            wrapper.provider_configuration().unwrap(),
            vec![
                ProviderInvocationArtifactBinding::new("frames", "source_frames", "application/vnd.aniflow.rife-frames",
                    ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::Directory, source_frames).unwrap(),
                ProviderInvocationArtifactBinding::new("index", "source_index", "application/vnd.aniflow.rife-frame-index+json",
                    ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, index_input).unwrap(),
            ],
            vec![
                ProviderInvocationArtifactBinding::new("frames", "derived_frames", "application/vnd.aniflow.rife-frames",
                    ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::Directory, root.join("output/frames")).unwrap(),
                ProviderInvocationArtifactBinding::new("index", "derived_index", "application/vnd.aniflow.rife-output-index+json",
                    ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, root.join("output/index.json")).unwrap(),
            ]).unwrap();
        Fixture { _directory:directory, request, index }
    }

    #[cfg(unix)]
    const COPY_PAIR_SCRIPT: &str = "#!/bin/sh\nleft=\"\"\noutput=\"\"\nwhile [ \"$#\" -gt 0 ]; do\ncase \"$1\" in\n-0) left=\"$2\"; shift 2 ;;\n-o) output=\"$2\"; shift 2 ;;\n*) shift ;;\nesac\ndone\nexec /bin/cp \"$left\" \"$output\"\n";

    #[cfg(unix)]
    #[test]
    fn synthetic_serial_provider_publishes_exact_two_x_grid_and_retains_original_frames() {
        let fixture = make_fixture(COPY_PAIR_SCRIPT, vec![]);
        let inputs_before = fixture.index.frames.iter().map(|frame|
            std::fs::read(selected(&fixture.request.inputs,"frames").unwrap().path.join(&frame.filename)).unwrap())
            .collect::<Vec<_>>();
        execute_invocation(&fixture.request).unwrap();
        let report: RifeOutputIndex = serde_json::from_slice(&std::fs::read(
            &selected(&fixture.request.outputs,"index").unwrap().path).unwrap()).unwrap();
        assert_eq!(report.frames.len(),4);
        assert_eq!(report.frames.iter().map(|frame|frame.origin).collect::<Vec<_>>(),
            [RifeFrameOrigin::Original,RifeFrameOrigin::InterpolatedMidpoint,RifeFrameOrigin::Original,RifeFrameOrigin::EndpointHold]);
        assert_eq!(report.plan.duration, super::super::plan(
            &serde_json::from_value::<RifeProviderConfiguration>(serde_json::to_value(&fixture.request.configuration.values).unwrap()).unwrap().configuration,
            &fixture.index).unwrap().duration);
        assert_eq!(report.frames[0].sha256,fixture.index.frames[0].sha256);
        assert_eq!(report.frames[2].sha256,fixture.index.frames[1].sha256);
        assert_eq!(report.frames[3].sha256,fixture.index.frames[1].sha256);
        assert!(!report.plan.native_qualified);
        for (frame, before) in fixture.index.frames.iter().zip(inputs_before) {
            assert_eq!(std::fs::read(selected(&fixture.request.inputs,"frames").unwrap().path.join(&frame.filename)).unwrap(),before);
        }
        let frames_root = &selected(&fixture.request.outputs,"frames").unwrap().path;
        assert_eq!(std::fs::read_dir(frames_root).unwrap().count(),4);
        for frame in report.frames {
            assert_eq!(identity(&frames_root.join(frame.filename),1024).unwrap().0,frame.sha256);
        }
    }

    #[cfg(unix)]
    #[test]
    fn declared_cut_holds_previous_frame_without_launching_tool() {
        let fixture = make_fixture("#!/bin/sh\nexit 99\n", vec![1]);
        execute_invocation(&fixture.request).unwrap();
        let report: RifeOutputIndex = serde_json::from_slice(&std::fs::read(
            &selected(&fixture.request.outputs,"index").unwrap().path).unwrap()).unwrap();
        assert_eq!(report.frames[1].origin,RifeFrameOrigin::CutHold);
        assert_eq!(report.frames[1].sha256,fixture.index.frames[0].sha256);
        assert_eq!(report.frames[1].source_right_ordinal,0);
    }

    #[cfg(unix)]
    #[test]
    fn missing_pair_output_and_modified_pins_refuse_publication() {
        let fixture = make_fixture("#!/bin/sh\nexit 0\n", vec![]);
        assert!(execute_invocation(&fixture.request).is_err());
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
        let fixture = make_fixture(COPY_PAIR_SCRIPT, vec![]);
        let wrapper: RifeProviderConfiguration = serde_json::from_value(
            serde_json::to_value(&fixture.request.configuration.values).unwrap()).unwrap();
        std::fs::write(&wrapper.configuration.settings.model.directory.join("flownet.bin"),b"changed model").unwrap();
        assert!(execute_invocation(&fixture.request).is_err());
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
        let fixture = make_fixture(COPY_PAIR_SCRIPT, vec![]);
        let mut wrapper: RifeProviderConfiguration = serde_json::from_value(
            serde_json::to_value(&fixture.request.configuration.values).unwrap()).unwrap();
        wrapper.configuration.settings.tool.sha256 = "0".repeat(64);
        let mut request = fixture.request.clone();
        request.configuration = wrapper.provider_configuration().unwrap();
        assert!(execute_invocation(&request).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_refuses_complete_output_using_existing_runner() {
        let mut fixture = make_fixture("#!/bin/sh\nexec /bin/sleep 2\n",vec![]);
        let mut wrapper: RifeProviderConfiguration = serde_json::from_value(
            serde_json::to_value(&fixture.request.configuration.values).unwrap()).unwrap();
        wrapper.configuration.settings.tool_timeout_milliseconds = 10;
        fixture.request.configuration = wrapper.provider_configuration().unwrap();
        let error = execute_invocation(&fixture.request).unwrap_err();
        assert!(error.message().contains("ToolTimeout"));
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
    }

    #[cfg(unix)]
    #[test]
    fn host_mismatch_and_insufficient_storage_are_rejected_before_launch() {
        let mut fixture = make_fixture(COPY_PAIR_SCRIPT,vec![]);
        let mut wrapper: RifeProviderConfiguration = serde_json::from_value(
            serde_json::to_value(&fixture.request.configuration.values).unwrap()).unwrap();
        wrapper.configuration.host.operating_system = if std::env::consts::OS == "macos" {"linux"} else {"macos"}.to_owned();
        fixture.request.configuration = wrapper.provider_configuration().unwrap();
        assert!(execute_invocation(&fixture.request).unwrap_err().message().contains("host OS"));
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
        let mut fixture = make_fixture(COPY_PAIR_SCRIPT,vec![]);
        let mut wrapper: RifeProviderConfiguration = serde_json::from_value(
            serde_json::to_value(&fixture.request.configuration.values).unwrap()).unwrap();
        let bytes = vec![0_u8;1_048_576];
        std::fs::write(wrapper.configuration.settings.model.directory.join("flownet.bin"),&bytes).unwrap();
        wrapper.configuration.settings.model.bin_sha256 = format!("{:x}",Sha256::digest(&bytes));
        wrapper.configuration.settings.model.bin_bytes = bytes.len() as u64;
        fixture.request.configuration = wrapper.provider_configuration().unwrap();
        assert!(execute_invocation(&fixture.request).unwrap_err().message().contains("storage reservation"));
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
    }

    #[cfg(unix)]
    #[test]
    fn undeclared_private_entries_and_output_capture_overflow_refuse_publication() {
        let mut script = COPY_PAIR_SCRIPT.replace("exec /bin/cp", "/bin/cp");
        script.push_str("printf \"undeclared\" > undeclared.txt\n");
        let fixture = make_fixture(&script,vec![]);
        assert!(execute_invocation(&fixture.request).unwrap_err().message().contains("undeclared"));
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
        let fixture = make_fixture("#!/bin/sh\nwhile :; do printf \"synthetic bounded output capture test\\n\"; done\n",vec![]);
        let error = execute_invocation(&fixture.request).unwrap_err();
        assert!(error.message().contains("ToolOutputLimit") || error.message().contains("ToolFailed"));
        for output in &fixture.request.outputs { assert!(!output.path.exists()); }
    }
}
