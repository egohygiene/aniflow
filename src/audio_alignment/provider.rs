//! Native observed-alignment orchestration; no source rewriting or downloads.
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::*;
use crate::audio_analysis::*;
use crate::audio_inspection::process::{
    GroupPolicy, hash_regular, open_regular, run_tool_isolated, verify_pin,
};
use crate::audio_inspection::{
    AUDIO_INSPECTION_MAXIMUM_BYTES, AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2,
    AudioInspectionConfiguration, AudioInspectionDiagnostic, AudioInspectionProviderConfiguration,
    AudioTechnicalCommandEvidence, AudioTechnicalInspection, AudioToolPin, wav,
};
use crate::{
    ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, ProviderReference, Result,
    StreamRole,
};

const FEATURE_PARAMS: &str = "-lowerf 130\n-upperf 6800\n-nfilt 25\n-transform dct\n-lifter 22\n-feat 1s_c_d_dd\n-svspec 0-12/13-25/26-38\n-agc none\n-cmn batch\n-varnorm no\n-model ptm\n-remove_noise yes\n";
const NOISE_DICTIONARY: &str = "<s> SIL\n</s> SIL\n<sil> SIL\n[NOISE] +NSN+\n[SPEECH] +SPN+\n";
const MAXIMUM_TOOL_BYTES: u64 = 128 * 1024 * 1024;

fn fail(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn dependency(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Dependency, message)
}
fn diagnostic(
    code: AudioAlignmentDiagnosticCode,
    component: &str,
    message: &str,
) -> AudioAlignmentDiagnostic {
    AudioAlignmentDiagnostic {
        code,
        component: component.to_owned(),
        message: message.to_owned(),
    }
}
fn tool_diagnostic(error: AudioInspectionDiagnostic) -> AudioAlignmentDiagnostic {
    use crate::audio_inspection::AudioInspectionDiagnosticCode as Old;
    use AudioAlignmentDiagnosticCode as New;
    let code = match error.code {
        Old::MissingTool => New::MissingTool,
        Old::InvalidTool => New::InvalidTool,
        Old::ToolDigestMismatch => New::ToolDigestMismatch,
        Old::ToolVersionMismatch => New::ToolVersionMismatch,
        Old::ToolTimeout => New::ToolTimeout,
        Old::ToolOutputLimit => New::ToolOutputLimit,
        Old::ToolFailed => New::ToolFailed,
        Old::Cancelled => New::Cancelled,
        Old::UnsupportedPlatform => New::UnsupportedPlatform,
    };
    diagnostic(code, &error.tool, &error.message)
}
fn dependency_error(value: AudioAlignmentDiagnostic) -> Error {
    dependency(format!(
        "{:?}: {}: {}",
        value.code, value.component, value.message
    ))
}
fn limits(
    configuration: &AlignmentConfiguration,
    tools: &AudioInspectionConfiguration,
) -> AudioInspectionConfiguration {
    let mut limits = tools.clone();
    limits.tool_timeout_milliseconds = configuration.tool_timeout_milliseconds;
    limits.maximum_tool_output_bytes = configuration.maximum_tool_output_bytes;
    limits
}
pub(super) fn read_reviewed_lyrics(path: &Path) -> Result<ReviewedLyrics> {
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(fail("reviewed lyrics must use a canonical nonsymlink path"));
    }
    let identity = hash_regular(path, AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES as u64)
        .map_err(|_| fail("reviewed lyrics must remain a bounded regular file"))?;
    let mut bytes = Vec::new();
    open_regular(path)
        .map_err(|_| fail("reviewed lyrics cannot be opened safely"))?
        .take(AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("reviewed lyrics cannot be read"))?;
    let reviewed = ReviewedLyrics::from_json_slice(&bytes)?;
    if (
        reviewed.artifact.sha256.clone(),
        reviewed.artifact.byte_size,
    ) != identity
    {
        return Err(fail(
            "reviewed_lyrics_changed: reviewed lyrics changed while reading",
        ));
    }
    Ok(reviewed)
}
fn verify_private_root(directory: &Path) -> Result<()> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|_| dependency("private alignment directory unavailable"))?
    {
        if names.len() >= 4 {
            return Err(dependency("alignment created too many private entries"));
        }
        let entry = entry.map_err(|_| dependency("private alignment entry unavailable"))?;
        let kind = entry
            .file_type()
            .map_err(|_| dependency("private alignment entry type unavailable"))?;
        if (entry.file_name() == "model" && !kind.is_dir())
            || (entry.file_name() != "model" && !kind.is_file())
        {
            return Err(dependency(
                "alignment created an undeclared private filesystem entry",
            ));
        }
        names.push(entry.file_name());
    }
    names.sort();
    let expected = ["dictionary.dict", "model", "pocketsphinx", "source.wav"].map(OsString::from);
    if names != expected {
        return Err(dependency("alignment created an undeclared private entry"));
    }
    Ok(())
}
fn verify_feature_configuration(directory: &Path) -> Result<()> {
    // Native feature parameters override command arguments after parsing, so a
    // digest of arbitrary content would not close the admitted resource profile.
    for (name, expected) in [
        ("feat.params", FEATURE_PARAMS),
        ("noisedict", NOISE_DICTIONARY),
    ] {
        let path = directory.join(name);
        let mut actual = Vec::new();
        open_regular(&path)
            .map_err(|_| dependency("model control resource cannot be opened safely"))?
            .take(expected.len() as u64 + 1)
            .read_to_end(&mut actual)
            .map_err(|_| dependency("model control resource cannot be read"))?;
        if actual != expected.as_bytes() {
            return Err(dependency(
                "model control resources must match the exact supported PocketSphinx en-us profile",
            ));
        }
    }
    Ok(())
}
fn verify_model(
    configuration: &AlignmentConfiguration,
) -> std::result::Result<(), AudioAlignmentDiagnostic> {
    use AudioAlignmentDiagnosticCode as Code;
    let pin = &configuration.model;
    if pin.directory.canonicalize().ok().as_deref() != Some(pin.directory.as_path())
        || std::fs::symlink_metadata(&pin.directory).map_or(true, |metadata| !metadata.is_dir())
    {
        return Err(diagnostic(
            if pin.directory.exists() {
                Code::InvalidModel
            } else {
                Code::MissingModel
            },
            "en-us",
            "model directory must remain canonical and symlink-free",
        ));
    }
    for file in &pin.files {
        let path = pin.directory.join(&file.name);
        verify_resource(
            &path,
            &file.sha256,
            file.byte_size,
            AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES,
            "en-us",
        )?;
    }
    verify_feature_configuration(&pin.directory)
        .map_err(|error| diagnostic(Code::InvalidModel, "en-us", error.message()))?;
    verify_resource(
        &configuration.dictionary.path,
        &configuration.dictionary.sha256,
        configuration.dictionary.byte_size,
        AUDIO_ALIGNMENT_MAXIMUM_DICTIONARY_BYTES,
        "cmudict-en-us",
    )?;
    Ok(())
}
fn verify_resource(
    path: &Path,
    sha256: &str,
    byte_size: u64,
    maximum: u64,
    component: &str,
) -> std::result::Result<(), AudioAlignmentDiagnostic> {
    use AudioAlignmentDiagnosticCode as Code;
    let dictionary = component == "cmudict-en-us";
    let missing = if dictionary {
        Code::MissingDictionary
    } else {
        Code::MissingModel
    };
    let invalid = if dictionary {
        Code::InvalidDictionary
    } else {
        Code::InvalidModel
    };
    let size_mismatch = if dictionary {
        Code::DictionarySizeMismatch
    } else {
        Code::ModelSizeMismatch
    };
    let digest_mismatch = if dictionary {
        Code::DictionaryDigestMismatch
    } else {
        Code::ModelDigestMismatch
    };
    let (digest, size) = hash_regular(path, maximum).map_err(|error| {
        diagnostic(
            if error.kind() == std::io::ErrorKind::NotFound {
                missing
            } else {
                invalid
            },
            component,
            "configured resource must remain a bounded nonsymlink regular file",
        )
    })?;
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(diagnostic(
            invalid,
            component,
            "resource path must remain canonical and symlink-free",
        ));
    }
    if size != byte_size {
        return Err(diagnostic(
            size_mismatch,
            component,
            "resource byte count differs from its configured identity",
        ));
    }
    if digest != sha256 {
        return Err(diagnostic(
            digest_mismatch,
            component,
            "resource SHA-256 differs from its configured identity",
        ));
    }
    Ok(())
}
fn verify_tool(
    configuration: &AlignmentConfiguration,
) -> std::result::Result<(), AudioAlignmentDiagnostic> {
    verify_pin(&configuration.pocketsphinx, "pocketsphinx").map_err(tool_diagnostic)?;
    let path = &configuration.pocketsphinx.executable;
    if path.canonicalize().ok().as_deref() != Some(path.as_path())
        || std::fs::symlink_metadata(path)
            .map_or(true, |metadata| metadata.len() > MAXIMUM_TOOL_BYTES)
    {
        return Err(diagnostic(
            AudioAlignmentDiagnosticCode::InvalidTool,
            "pocketsphinx",
            "executable must remain canonical, symlink-free and at most 128 MiB",
        ));
    }
    Ok(())
}
fn copy_private(
    source: &Path,
    target: &Path,
    expected: &(String, u64),
    maximum: u64,
    cancellation: &CancellationToken,
) -> Result<()> {
    if source.canonicalize().ok().as_deref() != Some(source)
        || hash_regular(source, maximum).map_err(|_| dependency("staged input is unavailable"))?
            != *expected
    {
        return Err(dependency("staged input differs from its pinned identity"));
    }
    let mut input = open_regular(source)
        .map_err(|_| dependency("staged input cannot be opened safely"))?
        .take(maximum + 1);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| dependency("cannot reserve private staged input"))?;
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::new(
                ErrorCategory::Execution,
                "alignment cancelled while staging input",
            ));
        }
        let amount = input
            .read(&mut buffer)
            .map_err(|_| dependency("cannot read staged input"))?;
        if amount == 0 {
            break;
        }
        copied += amount as u64;
        if copied > maximum {
            return Err(dependency("staged input exceeded its byte bound"));
        }
        output
            .write_all(&buffer[..amount])
            .map_err(|_| dependency("cannot write staged input"))?;
    }
    output
        .sync_all()
        .map_err(|_| dependency("cannot synchronize staged input"))?;
    if copied != expected.1
        || hash_regular(target, maximum)
            .map_err(|_| dependency("private staged input cannot be observed"))?
            != *expected
    {
        return Err(dependency(
            "private staged input differs from its pinned identity",
        ));
    }
    Ok(())
}
fn stage_tool(
    configuration: &AlignmentConfiguration,
    directory: &Path,
    cancellation: &CancellationToken,
) -> Result<AudioToolPin> {
    verify_tool(configuration).map_err(dependency_error)?;
    let mut pin = configuration.pocketsphinx.clone();
    let identity = hash_regular(&pin.executable, MAXIMUM_TOOL_BYTES)
        .map_err(|_| dependency("alignment executable is unavailable"))?;
    let target = directory.join("pocketsphinx");
    copy_private(
        &pin.executable,
        &target,
        &identity,
        MAXIMUM_TOOL_BYTES,
        cancellation,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| dependency("cannot make private alignment executable runnable"))?;
    }
    pin.executable = target;
    Ok(pin)
}
fn verify_private_inventory(directory: &Path, expected: &[&str]) -> Result<()> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|_| dependency("private alignment directory unavailable"))?
    {
        if names.len() >= expected.len() {
            return Err(dependency("alignment created too many private files"));
        }
        let entry = entry.map_err(|_| dependency("cannot inspect private alignment directory"))?;
        if !entry
            .file_type()
            .map_err(|_| dependency("private alignment entry unavailable"))?
            .is_file()
        {
            return Err(dependency(
                "alignment created an undeclared private filesystem entry",
            ));
        }
        names.push(entry.file_name());
    }
    let mut expected = expected.iter().map(OsString::from).collect::<Vec<_>>();
    names.sort();
    expected.sort();
    if names != expected {
        return Err(dependency("alignment created an undeclared private file"));
    }
    Ok(())
}
pub(super) fn preflight(
    configuration: &AlignmentConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> Result<AudioAlignmentPreflight> {
    configuration.validate()?;
    tools.validate()?;
    let mut diagnostics = Vec::new();
    if configuration.language != "en" {
        diagnostics.push(diagnostic(
            AudioAlignmentDiagnosticCode::UnsupportedLanguage,
            "language",
            "the en-us alignment profile supports explicit English only",
        ));
    }
    if let Err(error) = verify_tool(configuration) {
        diagnostics.push(error);
    }
    if let Err(error) = verify_model(configuration) {
        diagnostics.push(error);
    }
    if cancellation.is_cancelled() {
        diagnostics.push(diagnostic(
            AudioAlignmentDiagnosticCode::Cancelled,
            "pocketsphinx",
            "alignment preflight cancelled",
        ));
    }
    // PocketSphinx has no version command. Version is a caller declaration bound
    // to the executable digest; preflight never runs inference or claims a probe.
    let result = AudioAlignmentPreflight {
        schema: AUDIO_ALIGNMENT_PREFLIGHT_SCHEMA_V1.to_owned(),
        ready: diagnostics.is_empty(),
        diagnostics,
    };
    result.validate()?;
    Ok(result)
}
fn arguments(
    model: &Path,
    dictionary: &Path,
    disabled_lm: &Path,
    snapshot: &Path,
    phrase: &str,
) -> Vec<OsString> {
    vec![
        "-hmm".into(),
        model.as_os_str().to_owned(),
        "-dict".into(),
        dictionary.as_os_str().to_owned(),
        "-lm".into(),
        disabled_lm.as_os_str().to_owned(),
        "-samprate".into(),
        "16000".into(),
        "-frate".into(),
        "100".into(),
        "-phone_align".into(),
        "no".into(),
        "-state_align".into(),
        "no".into(),
        "-fsgusealtpron".into(),
        "no".into(),
        "-loglevel".into(),
        "ERROR".into(),
        "align".into(),
        snapshot.as_os_str().to_owned(),
        phrase.into(),
    ]
}
pub(super) fn command_evidence(observed: bool) -> Vec<AudioTechnicalCommandEvidence> {
    if !observed {
        return Vec::new();
    }
    vec![AudioTechnicalCommandEvidence {
        tool: "pocketsphinx".to_owned(),
        arguments: arguments(
            Path::new("{staged_model}"),
            Path::new("{staged_dictionary}"),
            Path::new("{staged_disabled_lm}"),
            Path::new("{snapshot}"),
            "{reviewed_phrase}",
        )
        .into_iter()
        .map(|value| value.into_string().expect("fixed evidence is UTF-8"))
        .collect(),
    }]
}

fn input<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .inputs
        .iter()
        .find(|input| input.port == port)
        .expect("validated alignment input")
}
fn output<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .outputs
        .iter()
        .find(|output| output.port == port)
        .expect("validated alignment output")
}

fn verify_invocation(
    request: &ProviderInvocationRequest,
) -> Result<AudioAlignmentProviderConfiguration> {
    request.validate()?;
    let config: AudioAlignmentProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| fail("invalid alignment configuration values"))?,
    )
    .map_err(|_| fail("invalid closed alignment provider configuration"))?;
    config.validate()?;
    if request.configuration != config.provider_configuration()? {
        return Err(fail(
            "alignment invocation must select its exact provider configuration",
        ));
    }
    if request.inputs.len() != 4 || request.outputs.len() != 2 {
        return Err(fail(
            "alignment provider requires exactly four inputs and two outputs",
        ));
    }
    for (port, id, mime, role, stream) in [
        (
            "audio",
            "source_audio",
            "audio/wav",
            ArtifactRole::TemporalSource,
            StreamRole::Audio,
        ),
        (
            "technical",
            "technical",
            "application/vnd.aniflow.audio-technical-inspection+json",
            ArtifactRole::ValidationEvidence,
            StreamRole::TimedMetadata,
        ),
        (
            "reviewed_lyrics",
            "reviewed_lyrics",
            "application/vnd.aniflow.timed-text+json",
            ArtifactRole::TemporalSource,
            StreamRole::Transcript,
        ),
        (
            "upstream_analysis",
            config.upstream_analysis_artifact_id.as_str(),
            "application/vnd.aniflow.audio-analysis+json",
            ArtifactRole::ValidationEvidence,
            StreamRole::TimedMetadata,
        ),
    ] {
        let values = request
            .inputs
            .iter()
            .filter(|input| input.port == port)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            return Err(fail("missing or duplicate alignment input port"));
        }
        let input = values[0];
        if input.artifact_id != id
            || input.artifact_type != mime
            || input.artifact_role != role
            || input.stream_role != Some(stream)
            || input.kind != ArtifactKind::File
        {
            return Err(fail(
                "alignment input binding differs from the declared profile",
            ));
        }
    }
    for (port, id, mime) in [
        (
            "alignment",
            "alignment",
            "application/vnd.aniflow.audio-alignment+json",
        ),
        (
            "analysis",
            "alignment_analysis",
            "application/vnd.aniflow.audio-analysis+json",
        ),
    ] {
        let values = request
            .outputs
            .iter()
            .filter(|output| output.port == port)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            return Err(fail("missing or duplicate alignment output port"));
        }
        let output = values[0];
        if output.artifact_id != id
            || output.artifact_type != mime
            || output.artifact_role != ArtifactRole::ValidationEvidence
            || output.stream_role != Some(StreamRole::TimedMetadata)
            || output.kind != ArtifactKind::File
        {
            return Err(fail(
                "alignment output binding differs from the declared profile",
            ));
        }
    }
    Ok(config)
}

fn read_evidence(
    binding: &ProviderInvocationArtifactBinding,
    maximum: u64,
) -> Result<(Vec<u8>, AudioArtifactReference)> {
    if binding.path.canonicalize().ok().as_deref() != Some(binding.path.as_path()) {
        return Err(fail(
            "alignment upstream evidence must use canonical nonsymlink paths",
        ));
    }
    let (sha256, byte_size) = hash_regular(&binding.path, maximum)
        .map_err(|_| fail("alignment upstream evidence unavailable or oversized"))?;
    let mut bytes = Vec::new();
    open_regular(&binding.path)
        .map_err(|_| fail("alignment upstream evidence cannot be opened safely"))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("alignment upstream evidence cannot be read"))?;
    if bytes.len() as u64 != byte_size || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return Err(fail("alignment upstream evidence changed while reading"));
    }
    Ok((
        bytes,
        AudioArtifactReference {
            id: binding.artifact_id.clone(),
            sha256,
            byte_size,
        },
    ))
}

fn silent_input(path: &Path, wave: &wav::PcmWave) -> Result<bool> {
    let mut file = open_regular(path).map_err(|_| fail("alignment snapshot cannot be opened"))?;
    file.seek(SeekFrom::Start(wave.data_offset))
        .map_err(|_| fail("alignment snapshot PCM cannot be located"))?;
    let mut buffer = [0_u8; 65536];
    let frame_bytes = usize::from(wave.channels) * 2;
    let mut remaining = wave.data_bytes;
    while remaining != 0 {
        let amount =
            usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded read size");
        file.read_exact(&mut buffer[..amount])
            .map_err(|_| fail("alignment snapshot PCM is truncated"))?;
        for frame in buffer[..amount].chunks_exact(frame_bytes) {
            let sum = frame
                .chunks_exact(2)
                .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
                .sum::<i32>();
            if sum != 0 {
                return Ok(false);
            }
        }
        remaining -= amount as u64;
    }
    Ok(true)
}

fn ensure_parent(parent: &Path) -> Result<()> {
    if !parent.is_absolute() {
        return Err(fail("alignment output directory must be absolute"));
    }
    let mut existing = parent;
    while !existing.exists() {
        if existing.is_symlink() {
            return Err(fail("alignment output directory cannot contain symlinks"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| fail("alignment output ancestor missing"))?;
    }
    if existing.canonicalize().ok().as_deref() != Some(existing) {
        return Err(fail(
            "alignment output directory must be canonical and symlink-free",
        ));
    }
    std::fs::create_dir_all(parent).map_err(|_| fail("cannot create assigned alignment workspace"))
}
fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| fail("cannot exclusively create alignment evidence"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| fail("cannot publish complete alignment evidence"))
}

pub(super) fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let config = verify_invocation(request)?;
    if config.settings.language != "en" {
        return Err(dependency(
            "unsupported_language: alignment requires explicit English",
        ));
    }
    let cancellation = CancellationToken::default();
    let (technical_bytes, technical_artifact) =
        read_evidence(input(request, "technical"), 1_048_576)?;
    let technical = AudioTechnicalInspection::from_json_slice(&technical_bytes)?;
    let (analysis_bytes, upstream_analysis_artifact) =
        read_evidence(input(request, "upstream_analysis"), 8 * 1024 * 1024)?;
    let analysis = AudioAnalysis::from_json_slice(&analysis_bytes)?;
    let reviewed = read_reviewed_lyrics(&input(request, "reviewed_lyrics").path)?;
    reviewed.validate_source(&analysis.source)?;
    if reviewed.artifact != config.reviewed_lyrics {
        return Err(fail(
            "reviewed_lyrics_changed: reviewed text differs from its planned identity",
        ));
    }
    let technical_config = AudioInspectionProviderConfiguration {
        schema: AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2.to_owned(),
        settings: config.tools.clone(),
        source: config.source.clone(),
    }
    .provider_configuration()?;
    if technical.source != config.source
        || technical.configuration_sha256 != technical_config.effective_configuration_sha256
        || analysis.source.artifact != config.source
        || analysis.source.sample_rate_hz != technical.sample_rate_hz
        || analysis.source.channels != technical.channels
        || analysis.source.frame_count != technical.frame_count
        || !matches!(
            analysis.status,
            AudioAnalysisStatus::Complete | AudioAnalysisStatus::Partial
        )
        || !analysis.artifacts.contains(&technical_artifact)
        || technical.tools[0].sha256 != config.tools.ffmpeg.sha256
        || technical.tools[0].version != config.tools.ffmpeg.version
        || technical.tools[1].sha256 != config.tools.ffprobe.sha256
        || technical.tools[1].version != config.tools.ffprobe.version
    {
        return Err(fail(
            "upstream_mismatch: alignment source, technical evidence and normalized analysis disagree",
        ));
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    verify_pin(&config.tools.ffprobe, "ffprobe")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    verify_tool(&config.settings).map_err(dependency_error)?;
    verify_model(&config.settings).map_err(dependency_error)?;
    let source_path = &input(request, "audio").path;
    let alignment_path = &output(request, "alignment").path;
    let analysis_path = &output(request, "analysis").path;
    let parent = alignment_path
        .parent()
        .ok_or_else(|| fail("alignment outputs require an assigned workspace"))?;
    if analysis_path.parent() != Some(parent) {
        return Err(fail("alignment outputs must share their assigned parent"));
    }
    ensure_parent(parent)?;
    for path in [alignment_path, analysis_path] {
        if path.symlink_metadata().is_ok() {
            return Err(fail("alignment output already exists"));
        }
    }
    let private = tempfile::Builder::new()
        .prefix(".alignment-")
        .tempdir_in(parent)
        .map_err(|_| fail("cannot reserve private alignment workspace"))?;
    let directory = private.path();
    let pin = stage_tool(&config.settings, directory, &cancellation)?;
    let model_path = directory.join("model");
    std::fs::create_dir(&model_path)
        .map_err(|_| dependency("cannot reserve private model directory"))?;
    for file in &config.settings.model.files {
        copy_private(
            &config.settings.model.directory.join(&file.name),
            &model_path.join(&file.name),
            &(file.sha256.clone(), file.byte_size),
            AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES,
            &cancellation,
        )?;
    }
    let dictionary_path = directory.join("dictionary.dict");
    let dictionary_identity = (
        config.settings.dictionary.sha256.clone(),
        config.settings.dictionary.byte_size,
    );
    copy_private(
        &config.settings.dictionary.path,
        &dictionary_path,
        &dictionary_identity,
        AUDIO_ALIGNMENT_MAXIMUM_DICTIONARY_BYTES,
        &cancellation,
    )?;
    verify_feature_configuration(&model_path)?;
    let snapshot = directory.join("source.wav");
    let expected_source = (config.source.sha256.clone(), config.source.byte_size);
    copy_private(
        source_path,
        &snapshot,
        &expected_source,
        AUDIO_INSPECTION_MAXIMUM_BYTES,
        &cancellation,
    )?;
    let wave = wav::inspect(&snapshot)?;
    if wave.sample_rate_hz != technical.sample_rate_hz
        || wave.channels != technical.channels
        || wave.frame_count != technical.frame_count
        || wave.pcm_sha256 != technical.pcm_sha256
    {
        return Err(fail(
            "upstream_mismatch: alignment snapshot differs from independent technical evidence",
        ));
    }
    let unavailable = if wave.sample_rate_hz != 16000 {
        Some(AlignmentUnavailableReason::UnsupportedSampleRate)
    } else if wave.channels != 1 {
        Some(AlignmentUnavailableReason::UnsupportedChannels)
    } else if silent_input(&snapshot, &wave)? {
        Some(AlignmentUnavailableReason::SilentInput)
    } else {
        None
    };
    let (result, raw_observation, timed_text) = if let Some(reason) = unavailable {
        (AlignmentResult::Unavailable { reason }, None, None)
    } else {
        let phrase = super::alignment_phrase(&reviewed)?;
        let capture = run_tool_isolated(
            &pin,
            "pocketsphinx",
            &arguments(
                &model_path,
                &dictionary_path,
                &directory.join("disabled.lm"),
                &snapshot,
                &phrase,
            ),
            &limits(&config.settings, &config.tools),
            &cancellation,
            GroupPolicy::Inherit,
            Some(directory),
        )
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
        let observation = super::normalize(&capture.stdout, &analysis.source, &reviewed)?;
        let raw = AudioArtifactReference {
            id: "alignment_observation".to_owned(),
            sha256: format!("{:x}", Sha256::digest(&capture.stdout)),
            byte_size: capture.stdout.len() as u64,
        };
        let timed_text = Some(super::timed_text(
            &observation,
            &analysis.source,
            &reviewed,
        )?);
        (
            AlignmentResult::Candidate { observation },
            Some(raw),
            timed_text,
        )
    };
    verify_private_root(directory)?;
    verify_private_inventory(
        &model_path,
        &[
            "feat.params",
            "mdef",
            "means",
            "noisedict",
            "sendump",
            "transition_matrices",
            "variances",
        ],
    )?;
    verify_tool(&config.settings).map_err(dependency_error)?;
    verify_model(&config.settings).map_err(dependency_error)?;
    verify_pin(&pin, "pocketsphinx").map_err(|error| dependency_error(tool_diagnostic(error)))?;
    for file in &config.settings.model.files {
        verify_resource(
            &model_path.join(&file.name),
            &file.sha256,
            file.byte_size,
            AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES,
            "en-us",
        )
        .map_err(dependency_error)?;
    }
    verify_resource(
        &dictionary_path,
        &dictionary_identity.0,
        dictionary_identity.1,
        AUDIO_ALIGNMENT_MAXIMUM_DICTIONARY_BYTES,
        "cmudict-en-us",
    )
    .map_err(dependency_error)?;
    for path in [source_path.as_path(), snapshot.as_path()] {
        if hash_regular(path, AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| fail("alignment input became unavailable"))?
            != expected_source
        {
            return Err(fail(
                "source_changed: alignment input changed during inference",
            ));
        }
    }
    if read_reviewed_lyrics(&input(request, "reviewed_lyrics").path)? != reviewed {
        return Err(fail(
            "reviewed_lyrics_changed: reviewed lyrics changed during inference",
        ));
    }
    let implementation_sha256 = hash_regular(
        &std::env::current_exe().map_err(|_| fail("alignment provider identity unavailable"))?,
        512 * 1024 * 1024,
    )
    .map_err(|_| fail("alignment provider identity cannot be read"))?
    .0;
    if implementation_sha256 != technical.implementation_sha256 {
        return Err(fail(
            "upstream_mismatch: inspection and alignment require the same native implementation",
        ));
    }
    let report = AudioAlignmentReport {
        schema: AUDIO_ALIGNMENT_REPORT_SCHEMA_V1.to_owned(),
        source: analysis.source.clone(),
        reviewed_lyrics: reviewed,
        scope: AudioScope {
            channels: (0..analysis.source.channels).collect(),
            stem_id: analysis.source.stem.as_ref().map(|stem| stem.id.clone()),
        },
        technical_artifact,
        upstream_analysis_artifact,
        raw_observation,
        provider: ProviderReference {
            id: AUDIO_ALIGNMENT_PROVIDER_ID.to_owned(),
            version: AUDIO_ALIGNMENT_PROVIDER_VERSION.to_owned(),
        },
        implementation_sha256,
        configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(),
        settings: AlignmentSettingsEvidence::from_configuration(&config.settings),
        licenses: AlignmentLicenseEvidence::default(),
        commands: command_evidence(!matches!(result, AlignmentResult::Unavailable { .. })),
        method: AlignmentMethod::default(),
        provenance: AudioProvenanceClass::Probabilistic,
        confidence: AudioConfidence::Unavailable {
            reason: AUDIO_ALIGNMENT_CONFIDENCE_REASON.to_owned(),
        },
        result,
        timed_text,
    };
    let alignment_bytes = report.canonical_json_bytes()?;
    let normalized = super::normalized_analysis(analysis, &report, &alignment_bytes)?;
    let normalized_bytes = normalized.canonical_json_bytes()?;
    for (port, reference) in [
        ("technical", &report.technical_artifact),
        ("upstream_analysis", &report.upstream_analysis_artifact),
    ] {
        if hash_regular(&input(request, port).path, 8 * 1024 * 1024)
            .map_err(|_| fail("alignment upstream evidence became unavailable"))?
            != (reference.sha256.clone(), reference.byte_size)
        {
            return Err(fail(
                "upstream_mismatch: alignment upstream evidence changed during inference",
            ));
        }
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    verify_pin(&config.tools.ffprobe, "ffprobe")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    private
        .close()
        .map_err(|_| fail("cannot remove private alignment staging"))?;
    publish_new(alignment_path, &alignment_bytes)?;
    publish_new(analysis_path, &normalized_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_arguments_use_one_literal_phrase_and_closed_cpu_resources() {
        let model = Path::new("/private café/model 雪");
        let dictionary = Path::new("/private café/dictionary 雪");
        let audio = Path::new("/private café/source 雪.wav");
        let phrase = "hello synthetic world";
        let args = arguments(
            model,
            dictionary,
            Path::new("/private café/disabled.lm"),
            audio,
            phrase,
        );
        assert_eq!(args.len(), 21);
        assert_eq!(args[1], model.as_os_str());
        assert_eq!(args[3], dictionary.as_os_str());
        assert_eq!(args[19], audio.as_os_str());
        assert_eq!(args[20], phrase);
        assert_eq!(args[16], "-loglevel");
        assert_eq!(args[18], "align");
        assert!(command_evidence(false).is_empty());
        assert_eq!(command_evidence(true).len(), 1);
    }

    #[test]
    fn arbitrary_pinned_feature_parameters_cannot_override_resource_or_frame_clock() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("feat.params"), FEATURE_PARAMS).unwrap();
        std::fs::write(root.path().join("noisedict"), NOISE_DICTIONARY).unwrap();
        verify_feature_configuration(root.path()).unwrap();
        for text in [
            format!("{FEATURE_PARAMS}-frate 10\n"),
            format!("{FEATURE_PARAMS}-dict /external/dictionary\n"),
            FEATURE_PARAMS.trim_end().to_owned(),
        ] {
            std::fs::write(root.path().join("feat.params"), text).unwrap();
            assert!(verify_feature_configuration(root.path()).is_err());
        }
        std::fs::write(root.path().join("feat.params"), FEATURE_PARAMS).unwrap();
        std::fs::write(root.path().join("noisedict"), "hello SIL\n").unwrap();
        assert!(verify_feature_configuration(root.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_copy_refuses_stale_identity_existing_target_and_symlink() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap();
        let source = directory.join("source");
        let target = directory.join("target");
        std::fs::write(&source, b"original").unwrap();
        let identity = hash_regular(&source, 1024).unwrap();
        std::fs::write(&target, b"existing").unwrap();
        assert!(
            copy_private(
                &source,
                &target,
                &identity,
                1024,
                &CancellationToken::default()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"existing");
        std::fs::remove_file(&target).unwrap();
        let link = directory.join("link");
        symlink(&source, &link).unwrap();
        assert!(
            copy_private(
                &link,
                &target,
                &identity,
                1024,
                &CancellationToken::default()
            )
            .is_err()
        );
        std::fs::write(&source, b"changed").unwrap();
        assert!(
            copy_private(
                &source,
                &target,
                &identity,
                1024,
                &CancellationToken::default()
            )
            .is_err()
        );
        assert!(!target.exists());
    }
}
