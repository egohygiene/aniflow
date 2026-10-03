//! Native MIDI candidate orchestration; no source rewriting or downloads.
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
    AudioTechnicalCommandEvidence, AudioTechnicalInspection, wav,
};
use crate::{
    ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, ProviderReference, Result,
    StreamRole,
};

const MAXIMUM_TOOL_BYTES: u64 = 128 * 1024 * 1024;

fn fail(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn dependency(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Dependency, message)
}
fn diagnostic(
    code: AudioMidiDiagnosticCode,
    component: &str,
    message: &str,
) -> AudioMidiDiagnostic {
    AudioMidiDiagnostic {
        code,
        component: component.to_owned(),
        message: message.to_owned(),
    }
}
fn tool_diagnostic(error: AudioInspectionDiagnostic) -> AudioMidiDiagnostic {
    use crate::audio_inspection::AudioInspectionDiagnosticCode as Old;
    use AudioMidiDiagnosticCode as New;
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
fn dependency_error(value: AudioMidiDiagnostic) -> Error {
    dependency(format!(
        "{:?}: {}: {}",
        value.code, value.component, value.message
    ))
}
fn limits(
    configuration: &MidiConfiguration,
    tools: &AudioInspectionConfiguration,
) -> AudioInspectionConfiguration {
    let mut limits = tools.clone();
    limits.tool_timeout_milliseconds = configuration.tool_timeout_milliseconds;
    limits.maximum_tool_output_bytes = configuration.maximum_tool_output_bytes;
    limits
}
fn verify_model(configuration: &MidiConfiguration) -> std::result::Result<(), AudioMidiDiagnostic> {
    use AudioMidiDiagnosticCode as Code;
    let pin = &configuration.model;
    let (digest, size) =
        hash_regular(&pin.path, AUDIO_MIDI_MAXIMUM_MODEL_BYTES).map_err(|error| {
            diagnostic(
                if error.kind() == std::io::ErrorKind::NotFound {
                    Code::MissingModel
                } else {
                    Code::InvalidModel
                },
                "basic-pitch-onnx-icassp-2022",
                "configured model must remain a bounded nonsymlink regular file",
            )
        })?;
    if pin.path.canonicalize().ok().as_deref() != Some(pin.path.as_path()) {
        return Err(diagnostic(
            Code::InvalidModel,
            "basic-pitch-onnx-icassp-2022",
            "model path must remain canonical and symlink-free",
        ));
    }
    if size != pin.byte_size {
        return Err(diagnostic(
            Code::ModelSizeMismatch,
            "basic-pitch-onnx-icassp-2022",
            "model byte count differs from its configured identity",
        ));
    }
    if digest != pin.sha256 {
        return Err(diagnostic(
            Code::ModelDigestMismatch,
            "basic-pitch-onnx-icassp-2022",
            "model SHA-256 differs from its configured identity",
        ));
    }
    Ok(())
}
fn verify_tool(configuration: &MidiConfiguration) -> std::result::Result<(), AudioMidiDiagnostic> {
    verify_pin(&configuration.python, "python").map_err(tool_diagnostic)?;
    let path = &configuration.python.executable;
    if path.canonicalize().ok().as_deref() != Some(path.as_path())
        || std::fs::symlink_metadata(path)
            .map_or(true, |metadata| metadata.len() > MAXIMUM_TOOL_BYTES)
    {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::InvalidTool,
            "python",
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
                "midi cancelled while staging input",
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
fn verify_adapter(
    configuration: &MidiConfiguration,
) -> std::result::Result<(), AudioMidiDiagnostic> {
    let pin = &configuration.adapter;
    let (digest, _) = hash_regular(&pin.path, 1024 * 1024).map_err(|_| {
        diagnostic(
            AudioMidiDiagnosticCode::InvalidAdapter,
            "adapter",
            "adapter must remain a bounded nonsymlink regular file",
        )
    })?;
    if pin.path.canonicalize().ok().as_deref() != Some(pin.path.as_path()) {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::InvalidAdapter,
            "adapter",
            "adapter path must remain canonical and symlink-free",
        ));
    }
    if digest != pin.sha256 {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::AdapterDigestMismatch,
            "adapter",
            "adapter SHA-256 differs from its configured identity",
        ));
    }
    Ok(())
}
fn stage_adapter(
    configuration: &MidiConfiguration,
    directory: &Path,
    cancellation: &CancellationToken,
) -> Result<std::path::PathBuf> {
    verify_adapter(configuration).map_err(dependency_error)?;
    let identity = hash_regular(&configuration.adapter.path, 1024 * 1024)
        .map_err(|_| dependency("MIDI adapter unavailable"))?;
    let target = directory.join("adapter.py");
    copy_private(
        &configuration.adapter.path,
        &target,
        &identity,
        1024 * 1024,
        cancellation,
    )?;
    Ok(target)
}
fn probe(
    configuration: &MidiConfiguration,
    tools: &AudioInspectionConfiguration,
    adapter: &Path,
    cancellation: &CancellationToken,
    policy: GroupPolicy,
    directory: &Path,
) -> std::result::Result<AudioMidiProbe, AudioMidiDiagnostic> {
    verify_tool(configuration)?;
    verify_model(configuration)?;
    verify_adapter(configuration)?;
    if hash_regular(adapter, 1024 * 1024)
        .map_err(|_| {
            diagnostic(
                AudioMidiDiagnosticCode::InvalidAdapter,
                "adapter",
                "private adapter is unavailable",
            )
        })?
        .0
        != configuration.adapter.sha256
    {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::AdapterDigestMismatch,
            "adapter",
            "private adapter differs from its configured identity",
        ));
    }
    let capture = run_tool_isolated(
        &configuration.python,
        "python",
        &arguments(adapter, None),
        &limits(configuration, tools),
        cancellation,
        policy,
        Some(directory),
    )
    .map_err(tool_diagnostic)?;
    let observed = AudioMidiProbe::from_json_slice(&capture.stdout).map_err(|_| {
        diagnostic(
            AudioMidiDiagnosticCode::InvalidProbe,
            "runtime",
            "runtime probe is not a valid bounded MIDI probe",
        )
    })?;
    if observed.python_version != configuration.python.version {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::ToolVersionMismatch,
            "python",
            "observed Python version differs from configured identity",
        ));
    }
    if observed.runtime_sha256 != configuration.runtime.sha256
        || observed.runtime_file_count != configuration.runtime.file_count
        || observed.runtime_byte_count != configuration.runtime.byte_count
    {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::RuntimeMismatch,
            "runtime",
            "runtime inventory differs from configured identity",
        ));
    }
    verify_tool(configuration)?;
    verify_model(configuration)?;
    verify_adapter(configuration)?;
    if hash_regular(adapter, 1024 * 1024)
        .map_err(|_| {
            diagnostic(
                AudioMidiDiagnosticCode::InvalidAdapter,
                "adapter",
                "private adapter became unavailable",
            )
        })?
        .0
        != configuration.adapter.sha256
    {
        return Err(diagnostic(
            AudioMidiDiagnosticCode::AdapterDigestMismatch,
            "adapter",
            "private adapter changed during runtime observation",
        ));
    }
    Ok(observed)
}
fn verify_private_inventory(directory: &Path, expected: &[&str]) -> Result<()> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|_| dependency("private midi directory unavailable"))?
    {
        if names.len() >= expected.len() {
            return Err(dependency("midi created too many private files"));
        }
        let entry = entry.map_err(|_| dependency("cannot inspect private midi directory"))?;
        if !entry
            .file_type()
            .map_err(|_| dependency("private midi entry unavailable"))?
            .is_file()
        {
            return Err(dependency(
                "midi created an undeclared private filesystem entry",
            ));
        }
        names.push(entry.file_name());
    }
    let mut expected = expected.iter().map(OsString::from).collect::<Vec<_>>();
    names.sort();
    expected.sort();
    if names != expected {
        return Err(dependency("midi created an undeclared private file"));
    }
    Ok(())
}
pub(super) fn preflight(
    configuration: &MidiConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> Result<AudioMidiPreflight> {
    configuration.validate()?;
    tools.validate()?;
    let mut diagnostics = Vec::new();
    for check in [
        verify_tool(configuration),
        verify_model(configuration),
        verify_adapter(configuration),
    ] {
        if let Err(error) = check {
            diagnostics.push(error);
        }
    }
    let mut observed = None;
    if diagnostics.is_empty() {
        let private = tempfile::Builder::new()
            .prefix("aniflow-midi-preflight-")
            .tempdir()
            .map_err(|_| dependency("cannot reserve private MIDI preflight directory"))?;
        let directory = private
            .path()
            .canonicalize()
            .map_err(|_| dependency("cannot resolve private MIDI preflight directory"))?;
        match stage_adapter(configuration, &directory, cancellation) {
            Ok(adapter) => {
                match probe(
                    configuration,
                    tools,
                    &adapter,
                    cancellation,
                    GroupPolicy::Own,
                    &directory,
                ) {
                    Ok(value) => observed = Some(value),
                    Err(error) => diagnostics.push(error),
                }
                if let Err(error) = verify_private_inventory(&directory, &["adapter.py"]) {
                    diagnostics.push(diagnostic(
                        AudioMidiDiagnosticCode::InvalidProbe,
                        "runtime",
                        error.message(),
                    ));
                }
            }
            Err(error) => diagnostics.push(diagnostic(
                if cancellation.is_cancelled() {
                    AudioMidiDiagnosticCode::Cancelled
                } else {
                    AudioMidiDiagnosticCode::InvalidAdapter
                },
                "adapter",
                error.message(),
            )),
        }
        private
            .close()
            .map_err(|_| dependency("cannot remove private MIDI preflight directory"))?;
    }
    if !diagnostics.is_empty() {
        observed = None;
    }
    let result = AudioMidiPreflight {
        schema: AUDIO_MIDI_PREFLIGHT_SCHEMA_V1.to_owned(),
        ready: diagnostics.is_empty(),
        diagnostics,
        probe: observed,
    };
    result.validate()?;
    Ok(result)
}
fn arguments(adapter: &Path, inputs: Option<(&Path, &Path)>) -> Vec<OsString> {
    let mut values = vec!["-I".into(), "-B".into(), adapter.as_os_str().to_owned()];
    if let Some((snapshot, model)) = inputs {
        values.extend([
            "--input".into(),
            snapshot.as_os_str().to_owned(),
            "--model".into(),
            model.as_os_str().to_owned(),
        ]);
    } else {
        values.push("--probe".into());
    }
    values
}
pub(super) fn command_evidence(inferred: bool) -> Vec<AudioTechnicalCommandEvidence> {
    let evidence = |inputs| AudioTechnicalCommandEvidence {
        tool: "python".to_owned(),
        arguments: arguments(Path::new("{staged_adapter}"), inputs)
            .into_iter()
            .map(|value| {
                value
                    .into_string()
                    .expect("fixed command evidence is UTF-8")
            })
            .collect(),
    };
    let mut commands = vec![evidence(None)];
    if inferred {
        commands.push(evidence(Some((
            Path::new("{snapshot}"),
            Path::new("{staged_model}"),
        ))));
    }
    commands.push(evidence(None));
    commands
}

fn input<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .inputs
        .iter()
        .find(|input| input.port == port)
        .expect("validated midi input")
}
fn output<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .outputs
        .iter()
        .find(|output| output.port == port)
        .expect("validated midi output")
}

fn verify_invocation(
    request: &ProviderInvocationRequest,
) -> Result<AudioMidiProviderConfiguration> {
    request.validate()?;
    let config: AudioMidiProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| fail("invalid midi configuration values"))?,
    )
    .map_err(|_| fail("invalid closed midi provider configuration"))?;
    config.validate()?;
    if request.configuration != config.provider_configuration()? {
        return Err(fail(
            "midi invocation must select its exact provider configuration",
        ));
    }
    if request.inputs.len() != 3 || request.outputs.len() != 2 {
        return Err(fail(
            "midi provider requires exactly three inputs and two outputs",
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
            return Err(fail("missing or duplicate midi input port"));
        }
        let input = values[0];
        if input.artifact_id != id
            || input.artifact_type != mime
            || input.artifact_role != role
            || input.stream_role != Some(stream)
            || input.kind != ArtifactKind::File
        {
            return Err(fail("midi input binding differs from the declared profile"));
        }
    }
    for (port, id, mime) in [
        ("midi", "midi", "application/vnd.aniflow.audio-midi+json"),
        (
            "analysis",
            "midi_analysis",
            "application/vnd.aniflow.audio-analysis+json",
        ),
    ] {
        let values = request
            .outputs
            .iter()
            .filter(|output| output.port == port)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            return Err(fail("missing or duplicate midi output port"));
        }
        let output = values[0];
        if output.artifact_id != id
            || output.artifact_type != mime
            || output.artifact_role != ArtifactRole::ValidationEvidence
            || output.stream_role != Some(StreamRole::TimedMetadata)
            || output.kind != ArtifactKind::File
        {
            return Err(fail(
                "midi output binding differs from the declared profile",
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
            "midi upstream evidence must use canonical nonsymlink paths",
        ));
    }
    let (sha256, byte_size) = hash_regular(&binding.path, maximum)
        .map_err(|_| fail("midi upstream evidence unavailable or oversized"))?;
    let mut bytes = Vec::new();
    open_regular(&binding.path)
        .map_err(|_| fail("midi upstream evidence cannot be opened safely"))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("midi upstream evidence cannot be read"))?;
    if bytes.len() as u64 != byte_size || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return Err(fail("midi upstream evidence changed while reading"));
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
    let mut file = open_regular(path).map_err(|_| fail("midi snapshot cannot be opened"))?;
    file.seek(SeekFrom::Start(wave.data_offset))
        .map_err(|_| fail("midi snapshot PCM cannot be located"))?;
    let mut buffer = [0_u8; 65536];
    let frame_bytes = usize::from(wave.channels) * 2;
    let mut remaining = wave.data_bytes;
    while remaining != 0 {
        let amount =
            usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded read size");
        file.read_exact(&mut buffer[..amount])
            .map_err(|_| fail("midi snapshot PCM is truncated"))?;
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
        return Err(fail("midi output directory must be absolute"));
    }
    let mut existing = parent;
    while !existing.exists() {
        if existing.is_symlink() {
            return Err(fail("midi output directory cannot contain symlinks"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| fail("midi output ancestor missing"))?;
    }
    if existing.canonicalize().ok().as_deref() != Some(existing) {
        return Err(fail(
            "midi output directory must be canonical and symlink-free",
        ));
    }
    std::fs::create_dir_all(parent).map_err(|_| fail("cannot create assigned midi workspace"))
}
fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| fail("cannot exclusively create midi evidence"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| fail("cannot publish complete midi evidence"))
}

pub(super) fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    let config = verify_invocation(request)?;
    let cancellation = CancellationToken::default();
    let (technical_bytes, technical_artifact) =
        read_evidence(input(request, "technical"), 1_048_576)?;
    let technical = AudioTechnicalInspection::from_json_slice(&technical_bytes)?;
    let (analysis_bytes, upstream_analysis_artifact) =
        read_evidence(input(request, "upstream_analysis"), 8 * 1024 * 1024)?;
    let analysis = AudioAnalysis::from_json_slice(&analysis_bytes)?;
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
            "upstream_mismatch: midi source, technical evidence and normalized analysis disagree",
        ));
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    verify_pin(&config.tools.ffprobe, "ffprobe")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    verify_tool(&config.settings).map_err(dependency_error)?;
    verify_model(&config.settings).map_err(dependency_error)?;
    verify_adapter(&config.settings).map_err(dependency_error)?;
    let source_path = &input(request, "audio").path;
    let midi_path = &output(request, "midi").path;
    let analysis_path = &output(request, "analysis").path;
    let parent = midi_path
        .parent()
        .ok_or_else(|| fail("midi outputs require an assigned workspace"))?;
    if analysis_path.parent() != Some(parent) {
        return Err(fail("midi outputs must share their assigned parent"));
    }
    ensure_parent(parent)?;
    for path in [midi_path, analysis_path] {
        if path.symlink_metadata().is_ok() {
            return Err(fail("midi output already exists"));
        }
    }
    let private = tempfile::Builder::new()
        .prefix(".midi-")
        .tempdir_in(parent)
        .map_err(|_| fail("cannot reserve private midi workspace"))?;
    let directory = private.path();
    let adapter = stage_adapter(&config.settings, directory, &cancellation)?;
    let model_path = directory.join("model.onnx");
    let expected_model = (
        config.settings.model.sha256.clone(),
        config.settings.model.byte_size,
    );
    copy_private(
        &config.settings.model.path,
        &model_path,
        &expected_model,
        AUDIO_MIDI_MAXIMUM_MODEL_BYTES,
        &cancellation,
    )?;
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
            "upstream_mismatch: midi snapshot differs from independent technical evidence",
        ));
    }
    let observed = probe(
        &config.settings,
        &config.tools,
        &adapter,
        &cancellation,
        GroupPolicy::Inherit,
        directory,
    )
    .map_err(dependency_error)?;
    let unavailable = if wave.sample_rate_hz != 22050 {
        Some(MidiUnavailableReason::UnsupportedSampleRate)
    } else if wave.channels != 1 {
        Some(MidiUnavailableReason::UnsupportedChannels)
    } else if wave.frame_count > 120 * u64::from(wave.sample_rate_hz) {
        Some(MidiUnavailableReason::DurationLimit)
    } else if silent_input(&snapshot, &wave)? {
        Some(MidiUnavailableReason::SilentInput)
    } else {
        None
    };
    let (result, raw_observation) = if let Some(reason) = unavailable {
        (MidiResult::Unavailable { reason }, None)
    } else {
        if config.source.byte_size > 8 * 1024 * 1024 {
            return Err(fail(
                "MIDI candidate inference requires a source WAV no larger than 8 MiB",
            ));
        }
        let capture = run_tool_isolated(
            &config.settings.python,
            "python",
            &arguments(&adapter, Some((&snapshot, &model_path))),
            &limits(&config.settings, &config.tools),
            &cancellation,
            GroupPolicy::Inherit,
            Some(directory),
        )
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
        let observation = super::normalize(&capture.stdout, &analysis.source)?;
        let raw = AudioArtifactReference {
            id: "midi_observation".to_owned(),
            sha256: format!("{:x}", Sha256::digest(&capture.stdout)),
            byte_size: capture.stdout.len() as u64,
        };
        (MidiResult::Candidate { observation }, Some(raw))
    };
    let after = probe(
        &config.settings,
        &config.tools,
        &adapter,
        &cancellation,
        GroupPolicy::Inherit,
        directory,
    )
    .map_err(dependency_error)?;
    if observed != after {
        return Err(dependency("MIDI runtime identity changed during inference"));
    }
    verify_private_inventory(directory, &["adapter.py", "model.onnx", "source.wav"])?;
    verify_tool(&config.settings).map_err(dependency_error)?;
    verify_model(&config.settings).map_err(dependency_error)?;
    verify_adapter(&config.settings).map_err(dependency_error)?;
    for (path, identity, maximum) in [
        (
            source_path.as_path(),
            &expected_source,
            AUDIO_INSPECTION_MAXIMUM_BYTES,
        ),
        (
            snapshot.as_path(),
            &expected_source,
            AUDIO_INSPECTION_MAXIMUM_BYTES,
        ),
        (
            model_path.as_path(),
            &expected_model,
            AUDIO_MIDI_MAXIMUM_MODEL_BYTES,
        ),
    ] {
        if hash_regular(path, maximum).map_err(|_| fail("MIDI input became unavailable"))?
            != *identity
        {
            return Err(fail("source_changed: MIDI input changed during inference"));
        }
    }
    let implementation_sha256 = hash_regular(
        &std::env::current_exe().map_err(|_| fail("midi provider identity unavailable"))?,
        512 * 1024 * 1024,
    )
    .map_err(|_| fail("midi provider identity cannot be read"))?
    .0;
    if implementation_sha256 != technical.implementation_sha256 {
        return Err(fail(
            "upstream_mismatch: inspection and midi require the same native implementation",
        ));
    }
    let report = AudioMidiReport {
        schema: AUDIO_MIDI_REPORT_SCHEMA_V1.to_owned(),
        source: analysis.source.clone(),
        scope: AudioScope {
            channels: (0..analysis.source.channels).collect(),
            stem_id: analysis.source.stem.as_ref().map(|stem| stem.id.clone()),
        },
        technical_artifact,
        upstream_analysis_artifact,
        raw_observation,
        provider: ProviderReference {
            id: AUDIO_MIDI_PROVIDER_ID.to_owned(),
            version: AUDIO_MIDI_PROVIDER_VERSION.to_owned(),
        },
        implementation_sha256,
        configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(),
        settings: MidiSettingsEvidence::from_configuration(&config.settings),
        probe: observed,
        commands: command_evidence(!matches!(result, MidiResult::Unavailable { .. })),
        method: MidiMethod::default(),
        provenance: AudioProvenanceClass::Probabilistic,
        confidence: AudioConfidence::Unavailable {
            reason: AUDIO_MIDI_CONFIDENCE_REASON.to_owned(),
        },
        result,
    };
    let midi_bytes = report.canonical_json_bytes()?;
    let normalized = super::normalized_analysis(analysis, &report, &midi_bytes)?;
    let normalized_bytes = normalized.canonical_json_bytes()?;
    if normalized_bytes.len() > AUDIO_MIDI_MAXIMUM_REPORT_BYTES {
        return Err(fail(
            "normalized MIDI evidence exceeds its 8 MiB artifact bound",
        ));
    }
    for (port, reference) in [
        ("technical", &report.technical_artifact),
        ("upstream_analysis", &report.upstream_analysis_artifact),
    ] {
        if hash_regular(&input(request, port).path, 8 * 1024 * 1024)
            .map_err(|_| fail("midi upstream evidence became unavailable"))?
            != (reference.sha256.clone(), reference.byte_size)
        {
            return Err(fail(
                "upstream_mismatch: midi upstream evidence changed during inference",
            ));
        }
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    verify_pin(&config.tools.ffprobe, "ffprobe")
        .map_err(|error| dependency_error(tool_diagnostic(error)))?;
    private
        .close()
        .map_err(|_| fail("cannot remove private midi staging"))?;
    publish_new(midi_path, &midi_bytes)?;
    publish_new(analysis_path, &normalized_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_isolated_direct_argv_with_explicit_inputs() {
        let adapter = Path::new("/private café/adapter 雪.py");
        let model = Path::new("/private café/model 雪.onnx");
        let input = Path::new("/private café/source 雪.wav");
        let args = arguments(adapter, Some((input, model)));
        assert_eq!(
            args.iter()
                .map(|value| value.to_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "-I",
                "-B",
                "/private café/adapter 雪.py",
                "--input",
                "/private café/source 雪.wav",
                "--model",
                "/private café/model 雪.onnx"
            ]
        );
        assert_eq!(
            arguments(adapter, None),
            vec![
                OsString::from("-I"),
                OsString::from("-B"),
                adapter.as_os_str().to_owned(),
                OsString::from("--probe")
            ]
        );
        assert_eq!(command_evidence(false).len(), 2);
        assert_eq!(command_evidence(true).len(), 3);
    }

    #[test]
    fn private_model_staging_never_copies_adjacent_accelerator_assets() {
        let original = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let original_root = original.path().canonicalize().unwrap();
        let target_root = target.path().canonicalize().unwrap();
        let model = original_root.join("model.onnx");
        std::fs::write(&model, b"synthetic model bytes").unwrap();
        std::fs::write(
            original_root.join("external-weights.data"),
            b"must remain untouched",
        )
        .unwrap();
        std::fs::create_dir(original_root.join("unrelated-cache")).unwrap();
        let identity = hash_regular(&model, 1024).unwrap();
        let staged = target_root.join("model.onnx");
        copy_private(
            &model,
            &staged,
            &identity,
            1024,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(hash_regular(&staged, 1024).unwrap(), identity);
        verify_private_inventory(&target_root, &["model.onnx"]).unwrap();
        assert_eq!(
            std::fs::read(original_root.join("external-weights.data")).unwrap(),
            b"must remain untouched"
        );
        std::fs::write(target_root.join("undeclared.data"), b"unexpected").unwrap();
        assert!(verify_private_inventory(&target_root, &["model.onnx"]).is_err());
    }

    #[test]
    fn private_copy_refuses_stale_source_and_existing_target() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("source");
        let target = root.join("target");
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

    #[cfg(unix)]
    #[test]
    fn private_copy_refuses_symlinked_assets_and_preserves_their_target() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("model.onnx");
        let link = root.join("model-link.bin");
        let staged = root.join("staged.bin");
        std::fs::write(&source, b"synthetic pinned model").unwrap();
        let identity = hash_regular(&source, 1024).unwrap();
        symlink(&source, &link).unwrap();
        assert!(
            copy_private(
                &link,
                &staged,
                &identity,
                1024,
                &CancellationToken::default()
            )
            .is_err()
        );
        assert!(!staged.exists());
        assert_eq!(std::fs::read(&source).unwrap(), b"synthetic pinned model");
    }
}
