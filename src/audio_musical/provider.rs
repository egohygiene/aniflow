//! Native musical adapter orchestration with bounded, immutable source evidence.
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::*;
use crate::audio_analysis::*;
use crate::audio_inspection::process::{GroupPolicy, hash_regular, run_tool, verify_pin};
use crate::audio_inspection::{
    AUDIO_INSPECTION_MAXIMUM_BYTES, AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V1,
    AudioInspectionConfiguration, AudioInspectionDiagnostic, AudioInspectionProviderConfiguration,
    AudioTechnicalCommandEvidence, AudioTechnicalInspection, wav,
};
use crate::{
    ArtifactKind, ArtifactRole, CancellationToken, CapabilityReference, Error, ErrorCategory,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, ProviderReference, Result,
    StreamRole,
};

fn fail(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn dependency(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Dependency, message)
}
fn tool_error(error: AudioInspectionDiagnostic) -> Error {
    dependency(format!(
        "{:?}: {}: {}",
        error.code, error.tool, error.message
    ))
}

fn limits(
    configuration: &MusicalAnalysisConfiguration,
    tools: &AudioInspectionConfiguration,
) -> AudioInspectionConfiguration {
    let mut limits = tools.clone();
    limits.tool_timeout_milliseconds = configuration.tool_timeout_milliseconds;
    limits.maximum_tool_output_bytes = configuration.maximum_tool_output_bytes;
    limits
}

fn verify_adapter(configuration: &MusicalAnalysisConfiguration) -> Result<()> {
    let path = &configuration.adapter.path;
    if path.canonicalize().ok().as_deref() != Some(path.as_path()) {
        return Err(dependency(
            "musical adapter path must remain canonical and symlink-free",
        ));
    }
    let (sha256, _) = hash_regular(path, 1_048_576)
        .map_err(|_| dependency("musical adapter must be a regular file no larger than 1 MiB"))?;
    if sha256 != configuration.adapter.sha256 {
        return Err(dependency(
            "musical adapter SHA-256 differs from its configured identity",
        ));
    }
    Ok(())
}

fn arguments(
    configuration: &MusicalAnalysisConfiguration,
    snapshot: Option<&Path>,
) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-I"),
        OsString::from("-B"),
        configuration.adapter.path.as_os_str().to_owned(),
    ];
    if let Some(snapshot) = snapshot {
        arguments.extend([OsString::from("--input"), snapshot.as_os_str().to_owned()]);
    } else {
        arguments.push(OsString::from("--probe"));
    }
    arguments
}

pub(super) fn command_evidence(estimated: bool) -> Vec<AudioTechnicalCommandEvidence> {
    let probe = AudioTechnicalCommandEvidence {
        tool: "python".to_owned(),
        arguments: ["-I", "-B", "{adapter}", "--probe"]
            .map(str::to_owned)
            .to_vec(),
    };
    let mut commands = vec![probe.clone()];
    if estimated {
        commands.push(AudioTechnicalCommandEvidence {
            tool: "python".to_owned(),
            arguments: ["-I", "-B", "{adapter}", "--input", "{snapshot}"]
                .map(str::to_owned)
                .to_vec(),
        });
    }
    commands.push(probe);
    commands
}

fn probe(
    configuration: &MusicalAnalysisConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
    policy: GroupPolicy,
    directory: Option<&Path>,
) -> Result<AudioMusicalProbe> {
    configuration.validate()?;
    tools.validate()?;
    verify_adapter(configuration)?;
    let capture = run_tool(
        &configuration.python,
        "python",
        &arguments(configuration, None),
        &limits(configuration, tools),
        cancellation,
        policy,
        directory,
    )
    .map_err(tool_error)?;
    verify_adapter(configuration)?;
    let observed = AudioMusicalProbe::from_json_slice(&capture.stdout)?;
    if observed.python_version != configuration.python.version
        || observed.runtime_sha256 != configuration.runtime.sha256
        || observed.runtime_file_count != configuration.runtime.file_count
        || observed.runtime_byte_count != configuration.runtime.byte_count
    {
        return Err(dependency(
            "musical analyzer probe differs from its configured Python or runtime identity",
        ));
    }
    Ok(observed)
}

pub(super) fn preflight(
    configuration: &MusicalAnalysisConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> Result<AudioMusicalProbe> {
    probe(configuration, tools, cancellation, GroupPolicy::Own, None)
}

fn input<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .inputs
        .iter()
        .find(|input| input.port == port)
        .expect("validated musical input")
}
fn output<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .outputs
        .iter()
        .find(|output| output.port == port)
        .expect("validated musical output")
}

fn verify_invocation(
    request: &ProviderInvocationRequest,
) -> Result<AudioMusicalProviderConfiguration> {
    request.validate()?;
    let config: AudioMusicalProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| fail("invalid musical configuration values"))?,
    )
    .map_err(|_| fail("invalid closed musical provider configuration"))?;
    config.validate()?;
    if request.configuration != config.provider_configuration()? {
        return Err(fail(
            "musical invocation must select its exact provider configuration",
        ));
    }
    if request.inputs.len() != 3 || request.outputs.len() != 2 {
        return Err(fail(
            "musical provider requires exactly three inputs and two outputs",
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
            return Err(fail("missing or duplicate musical input port"));
        }
        let input = values[0];
        if input.artifact_id != id
            || input.artifact_type != mime
            || input.artifact_role != role
            || input.stream_role != Some(stream)
            || input.kind != ArtifactKind::File
        {
            return Err(fail(
                "musical input binding differs from the declared profile",
            ));
        }
    }
    for (port, id, mime) in [
        (
            "musical",
            "musical",
            "application/vnd.aniflow.audio-musical-analysis+json",
        ),
        (
            "analysis",
            "musical_analysis",
            "application/vnd.aniflow.audio-analysis+json",
        ),
    ] {
        let values = request
            .outputs
            .iter()
            .filter(|output| output.port == port)
            .collect::<Vec<_>>();
        if values.len() != 1 {
            return Err(fail("missing or duplicate musical output port"));
        }
        let output = values[0];
        if output.artifact_id != id
            || output.artifact_type != mime
            || output.artifact_role != ArtifactRole::ValidationEvidence
            || output.stream_role != Some(StreamRole::TimedMetadata)
            || output.kind != ArtifactKind::File
        {
            return Err(fail(
                "musical output binding differs from the declared profile",
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
            "musical upstream evidence must use canonical nonsymlink paths",
        ));
    }
    let (sha256, byte_size) = hash_regular(&binding.path, maximum)
        .map_err(|_| fail("musical upstream evidence unavailable or oversized"))?;
    let mut bytes = Vec::new();
    File::open(&binding.path)
        .map_err(|_| fail("musical upstream evidence cannot be opened"))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("musical upstream evidence cannot be read"))?;
    if bytes.len() as u64 != byte_size || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return Err(fail("musical upstream evidence changed while reading"));
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

fn silent_downmix(path: &Path, wave: &wav::PcmWave) -> Result<bool> {
    let mut file = File::open(path).map_err(|_| fail("musical snapshot cannot be opened"))?;
    file.seek(SeekFrom::Start(wave.data_offset))
        .map_err(|_| fail("musical snapshot PCM cannot be located"))?;
    let mut buffer = [0_u8; 65536];
    let frame_bytes = usize::from(wave.channels) * 2;
    let mut remaining = wave.data_bytes;
    while remaining != 0 {
        let amount =
            usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded read size");
        file.read_exact(&mut buffer[..amount])
            .map_err(|_| fail("musical snapshot PCM is truncated"))?;
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
        return Err(fail("musical output directory must be absolute"));
    }
    let mut existing = parent;
    while !existing.exists() {
        if existing.is_symlink() {
            return Err(fail("musical output directory cannot contain symlinks"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| fail("musical output ancestor missing"))?;
    }
    if existing.canonicalize().ok().as_deref() != Some(existing) {
        return Err(fail(
            "musical output directory must be canonical and symlink-free",
        ));
    }
    std::fs::create_dir_all(parent).map_err(|_| fail("cannot create assigned musical workspace"))
}
fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| fail("cannot exclusively create musical evidence"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| fail("cannot publish complete musical evidence"))
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
        schema: AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
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
            "upstream_mismatch: musical source, technical evidence and normalized analysis disagree",
        ));
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg").map_err(tool_error)?;
    verify_pin(&config.tools.ffprobe, "ffprobe").map_err(tool_error)?;
    let source_path = &input(request, "audio").path;
    let musical_path = &output(request, "musical").path;
    let analysis_path = &output(request, "analysis").path;
    let parent = musical_path
        .parent()
        .ok_or_else(|| fail("musical outputs require an assigned workspace"))?;
    if analysis_path.parent() != Some(parent) {
        return Err(fail("musical outputs must share their assigned parent"));
    }
    ensure_parent(parent)?;
    if source_path.canonicalize().ok().as_deref() != Some(source_path.as_path()) {
        return Err(fail("musical source must use a canonical nonsymlink path"));
    }
    let expected_source = (config.source.sha256.clone(), config.source.byte_size);
    if hash_regular(source_path, AUDIO_INSPECTION_MAXIMUM_BYTES)
        .map_err(|_| fail("musical source is unavailable"))?
        != expected_source
    {
        return Err(fail(
            "source_changed: musical source differs from its planned identity",
        ));
    }
    let mut snapshot = tempfile::Builder::new()
        .prefix(".musical-snapshot-")
        .suffix(".wav")
        .tempfile_in(parent)
        .map_err(|_| fail("cannot reserve musical snapshot"))?;
    let copied = std::io::copy(
        &mut File::open(source_path)
            .map_err(|_| fail("musical source cannot be opened"))?
            .take(AUDIO_INSPECTION_MAXIMUM_BYTES + 1),
        snapshot.as_file_mut(),
    )
    .map_err(|_| fail("cannot snapshot musical source"))?;
    snapshot
        .as_file_mut()
        .sync_all()
        .map_err(|_| fail("cannot sync musical snapshot"))?;
    if copied != config.source.byte_size
        || hash_regular(snapshot.path(), AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| fail("musical snapshot is unavailable"))?
            != expected_source
    {
        return Err(fail(
            "source_changed: musical snapshot differs from its planned identity",
        ));
    }
    let wave = wav::inspect(snapshot.path())?;
    if wave.sample_rate_hz != technical.sample_rate_hz
        || wave.channels != technical.channels
        || wave.frame_count != technical.frame_count
        || wave.pcm_sha256 != technical.pcm_sha256
    {
        return Err(fail(
            "upstream_mismatch: musical snapshot differs from independently decoded technical evidence",
        ));
    }
    let observed = probe(
        &config.settings,
        &config.tools,
        &cancellation,
        GroupPolicy::Inherit,
        Some(parent),
    )?;
    let unavailable = if wave.sample_rate_hz != 44100 {
        Some(AudioMusicalUnavailableReason::UnsupportedSampleRate)
    } else if wave.frame_count < 352800 {
        Some(AudioMusicalUnavailableReason::InsufficientDuration)
    } else if silent_downmix(snapshot.path(), &wave)? {
        Some(AudioMusicalUnavailableReason::SilentDownmix)
    } else {
        None
    };
    let result = if let Some(reason) = unavailable {
        AudioMusicalResult::Unavailable { reason }
    } else {
        verify_adapter(&config.settings)?;
        let capture = run_tool(
            &config.settings.python,
            "python",
            &arguments(&config.settings, Some(snapshot.path())),
            &limits(&config.settings, &config.tools),
            &cancellation,
            GroupPolicy::Inherit,
            Some(parent),
        )
        .map_err(tool_error)?;
        verify_adapter(&config.settings)?;
        let observation = AudioMusicalObservation::from_json_slice(&capture.stdout)?;
        AudioMusicalResult::from_observation(observation, &analysis.source)?
    };
    let after = probe(
        &config.settings,
        &config.tools,
        &cancellation,
        GroupPolicy::Inherit,
        Some(parent),
    )?;
    if after != observed {
        return Err(dependency(
            "musical analyzer identity changed during estimation",
        ));
    }
    let implementation_sha256 = hash_regular(
        &std::env::current_exe().map_err(|_| fail("musical provider identity unavailable"))?,
        512 * 1024 * 1024,
    )
    .map_err(|_| fail("musical provider identity cannot be read"))?
    .0;
    if implementation_sha256 != technical.implementation_sha256 {
        return Err(fail(
            "upstream_mismatch: inspection and musical stages require the same pinned native implementation",
        ));
    }
    let report = AudioMusicalAnalysis {
        schema: AUDIO_MUSICAL_ANALYSIS_SCHEMA_V1.to_owned(),
        source: analysis.source.clone(),
        scope: AudioScope {
            channels: (0..analysis.source.channels).collect(),
            stem_id: analysis.source.stem.as_ref().map(|stem| stem.id.clone()),
        },
        technical_artifact,
        upstream_analysis_artifact,
        provider: ProviderReference {
            id: AUDIO_MUSICAL_PROVIDER_ID.to_owned(),
            version: AUDIO_MUSICAL_PROVIDER_VERSION.to_owned(),
        },
        implementation_sha256,
        configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(),
        probe: observed,
        settings: AudioMusicalSettingsEvidence::from_configuration(&config.settings),
        commands: command_evidence(matches!(&result, AudioMusicalResult::Estimated { .. })),
        method: AudioMusicalMethod::default(),
        provenance: AudioProvenanceClass::Heuristic,
        confidence: AudioConfidence::Unavailable {
            reason: AUDIO_MUSICAL_CONFIDENCE_REASON.to_owned(),
        },
        result,
        unsupported_families: AudioMusicalUnsupportedFamily::all(),
    };
    let musical_bytes = report.canonical_json_bytes()?;
    let normalized = normalized_analysis(analysis, &report, &musical_bytes)?;
    let normalized_bytes = normalized.canonical_json_bytes()?;
    if musical_bytes.len() > 8 * 1024 * 1024 || normalized_bytes.len() > 8 * 1024 * 1024 {
        return Err(fail("musical evidence exceeds its 8 MiB artifact bound"));
    }
    for path in [source_path.as_path(), snapshot.path()] {
        if hash_regular(path, AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| fail("musical source or snapshot is unavailable"))?
            != expected_source
        {
            return Err(fail(
                "source_changed: musical source or snapshot changed during estimation",
            ));
        }
    }
    for (port, reference) in [
        ("technical", &report.technical_artifact),
        ("upstream_analysis", &report.upstream_analysis_artifact),
    ] {
        if hash_regular(&input(request, port).path, 8 * 1024 * 1024)
            .map_err(|_| fail("musical upstream evidence became unavailable"))?
            != (reference.sha256.clone(), reference.byte_size)
        {
            return Err(fail(
                "upstream_mismatch: musical upstream evidence changed during estimation",
            ));
        }
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg").map_err(tool_error)?;
    verify_pin(&config.tools.ffprobe, "ffprobe").map_err(tool_error)?;
    verify_pin(&config.settings.python, "python").map_err(tool_error)?;
    verify_adapter(&config.settings)?;
    snapshot
        .close()
        .map_err(|_| fail("cannot remove private musical snapshot"))?;
    publish_new(musical_path, &musical_bytes)?;
    publish_new(analysis_path, &normalized_bytes)
}

fn normalized_analysis(
    mut analysis: AudioAnalysis,
    report: &AudioMusicalAnalysis,
    bytes: &[u8],
) -> Result<AudioAnalysis> {
    let provider_id = "audio-musical-provider";
    if analysis
        .capabilities
        .iter()
        .any(|capability| capability.capability.id == AUDIO_MUSICAL_CAPABILITY_ID)
    {
        return Err(fail(
            "upstream audio analysis already declares musical estimates",
        ));
    }
    let musical_artifact = AudioArtifactReference {
        id: "musical".to_owned(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        byte_size: bytes.len() as u64,
    };
    for artifact in [report.upstream_analysis_artifact.clone(), musical_artifact] {
        if analysis
            .artifacts
            .iter()
            .any(|existing| existing.id == artifact.id)
        {
            return Err(fail(
                "musical evidence artifact collides with upstream identity",
            ));
        }
        analysis.artifacts.push(artifact);
    }
    analysis.providers.push(AudioProviderEvidence {
        id: provider_id.to_owned(), provider: report.provider.clone(), implementation_sha256: report.implementation_sha256.clone(), configuration_sha256: report.configuration_sha256.clone(),
        tools: vec![
            AudioComponentEvidence { id: "python".to_owned(), version: report.probe.python_version.clone(), revision: report.probe.python_version.clone(), sha256: report.settings.python_sha256.clone(), license: AudioLicenseEvidence::Unavailable { reason: "Python license evidence is not collected by this adapter".to_owned() } },
            AudioComponentEvidence { id: "musical-adapter".to_owned(), version: "1.0.0".to_owned(), revision: "1.0.0".to_owned(), sha256: report.settings.adapter_sha256.clone(), license: AudioLicenseEvidence::Unavailable { reason: "Adapter license evidence is not collected by this observation".to_owned() } },
            AudioComponentEvidence { id: "essentia-python-runtime".to_owned(), version: report.probe.essentia_version.clone(), revision: report.probe.essentia_git_sha.clone(), sha256: report.probe.runtime_sha256.clone(), license: AudioLicenseEvidence::Recorded { statement: "Essentia declares AGPL-3.0-only; the musical companion identifies dependency license metadata and the combined runtime inventory.".to_owned(), evidence_artifact_id: "musical".to_owned() } },
        ],
        models: AudioModelEvidence::NoneRequired {},
        license: AudioLicenseEvidence::Unavailable { reason: "Native provider license evidence is not collected by this observation".to_owned() },
    });
    let evidence = vec![
        "source_audio".to_owned(),
        "technical".to_owned(),
        report.upstream_analysis_artifact.id.clone(),
        "musical".to_owned(),
    ];
    let provenance = AudioObservationProvenance {
        class: AudioProvenanceClass::Heuristic,
        confidence: report.confidence.clone(),
        provider_evidence_id: provider_id.to_owned(),
        evidence_artifact_ids: evidence.clone(),
    };
    let mut status = AudioAnalysisStatus::Unavailable;
    let mut diagnostic_ids = Vec::new();
    if let AudioMusicalResult::Estimated {
        beats,
        key_disagreement,
        tempo_status,
        beats_status,
        key_status,
        tempo_candidates,
        key_candidates,
        ..
    } = &report.result
    {
        let estimated = [tempo_status, beats_status, key_status]
            .into_iter()
            .filter(|status| matches!(status, AudioMusicalFamilyStatus::Estimated {}))
            .count();
        status = match estimated {
            3 => AudioAnalysisStatus::Complete,
            0 => AudioAnalysisStatus::Unavailable,
            _ => AudioAnalysisStatus::Partial,
        };
        for candidate in tempo_candidates {
            analysis.observations.push(AudioObservation {
                id: format!("musical-{}", candidate.id),
                capability_id: AUDIO_MUSICAL_CAPABILITY_ID.to_owned(),
                kind: AudioObservationKind::Tempo,
                value: AudioObservationValue::Quantity {
                    value: candidate.value,
                    unit: AudioUnit::BeatsPerMinute,
                },
                scope: report.scope.clone(),
                provenance: provenance.clone(),
            });
        }
        for candidate in key_candidates {
            let pitch = [
                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
            ][usize::from(candidate.pitch_class)];
            analysis.observations.push(AudioObservation {
                id: format!("musical-{}", candidate.id),
                capability_id: AUDIO_MUSICAL_CAPABILITY_ID.to_owned(),
                kind: AudioObservationKind::Key,
                value: AudioObservationValue::Label {
                    value: format!("{pitch} {}", candidate.mode),
                },
                scope: report.scope.clone(),
                provenance: provenance.clone(),
            });
        }
        if !beats.is_empty() {
            analysis.timelines.push(AudioTimeline {
                id: "musical-beats".to_owned(),
                capability_id: AUDIO_MUSICAL_CAPABILITY_ID.to_owned(),
                kind: AudioTimelineKind::Markers,
                scope: report.scope.clone(),
                events: beats
                    .iter()
                    .map(|beat| AudioEvent {
                        id: format!("musical-{}", beat.id),
                        range: AudioFrameRange {
                            start: beat.source_frame,
                            end: beat.source_frame + 1,
                        },
                        label: "beat estimate".to_owned(),
                        provenance: provenance.clone(),
                    })
                    .collect(),
            });
        }
        if *key_disagreement {
            let id = "musical-key-disagreement".to_owned();
            analysis.diagnostics.push(AudioDiagnostic { id: id.clone(), code: id.clone(), severity: AudioDiagnosticSeverity::Warning, message: "Key profile estimates disagree; both candidates are preserved without selecting a consensus.".to_owned() });
            diagnostic_ids.push(id);
        }
    }
    if status != AudioAnalysisStatus::Complete {
        let id = "musical-estimates-unavailable".to_owned();
        analysis.diagnostics.push(AudioDiagnostic { id: id.clone(), code: id.clone(), severity: AudioDiagnosticSeverity::Warning, message: "One or more requested musical estimates are unavailable; the musical companion preserves per-family outcomes.".to_owned() });
        diagnostic_ids.push(id);
        analysis.status = AudioAnalysisStatus::Partial;
    }
    analysis.capabilities.push(AudioCapabilityOutcome {
        capability: CapabilityReference {
            id: AUDIO_MUSICAL_CAPABILITY_ID.to_owned(),
            version: "1.0.0".to_owned(),
        },
        status,
        provider_evidence_ids: vec![provider_id.to_owned()],
        evidence_artifact_ids: evidence,
        diagnostic_ids,
    });
    analysis.validate()?;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::silent_downmix;
    use crate::audio_inspection::wav::PcmWave;
    use std::io::Write;

    fn silent(samples: &[i16], channels: u16) -> bool {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        for sample in samples {
            file.write_all(&sample.to_le_bytes()).unwrap();
        }
        file.flush().unwrap();
        let wave = PcmWave {
            sample_rate_hz: 44100,
            channels,
            frame_count: samples.len() as u64 / u64::from(channels),
            pcm_sha256: String::new(),
            data_offset: 0,
            data_bytes: (samples.len() * 2) as u64,
        };
        silent_downmix(file.path(), &wave).unwrap()
    }

    #[test]
    fn arithmetic_average_silence_is_distinct_from_channel_silence() {
        assert!(silent(&[0, 0, 0], 1));
        assert!(silent(&[123, -123, 32767, -32767], 2));
        assert!(!silent(&[-32768, 32767], 2));
        assert!(!silent(&[0, 1], 2));
        assert!(!silent(&[-1, 0], 1));
    }

    #[test]
    fn downmix_scan_checks_samples_beyond_the_first_read_buffer() {
        let mut samples = vec![0; 32770];
        samples[32769] = 1;
        assert!(!silent(&samples, 2));
        samples[32768] = -1;
        assert!(silent(&samples, 2));
    }
}
