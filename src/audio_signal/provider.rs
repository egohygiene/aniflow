//! Signal provider using immutable technical evidence and the existing v3 lifecycle.
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::*;
use super::{ebur128, pcm};
use crate::audio_analysis::*;
use crate::audio_inspection::process::{GroupPolicy, hash_regular, run_tool, verify_pin};
use crate::audio_inspection::wav;
use crate::audio_inspection::{
    AUDIO_INSPECTION_MAXIMUM_BYTES, AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V1,
    AudioInspectionDiagnostic, AudioInspectionProviderConfiguration, AudioTechnicalCommandEvidence,
    AudioTechnicalInspection,
};
use crate::{
    ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, ProviderReference, Result,
    StreamRole,
};

const MAIN_FILTER: &str =
    "ebur128=metadata=1:peak=true:framelog=verbose,ametadata=print:key=lavfi.r128.S:file=-";

fn meter_arguments(filter: &str) -> Vec<String> {
    [
        "-hide_banner",
        "-nostdin",
        "-nostats",
        "-loglevel",
        "info",
        "-xerror",
        "-err_detect",
        "explode",
        "-threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
        "-i",
        "{snapshot}",
        "-map",
        "0:a:0",
        "-vn",
        "-sn",
        "-dn",
        "-af",
        filter,
        "-f",
        "null",
        "-",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub(super) fn command_evidence(sample_rate_hz: u32) -> Vec<AudioTechnicalCommandEvidence> {
    let mut commands = vec![AudioTechnicalCommandEvidence {
        tool: "ffmpeg".to_owned(),
        arguments: meter_arguments(MAIN_FILTER),
    }];
    if sample_rate_hz <= 48000 {
        commands.push(AudioTechnicalCommandEvidence {
            tool: "ffmpeg".to_owned(),
            arguments: meter_arguments(&format!(
                "apad=pad_len={},ebur128=peak=true:framelog=verbose",
                sample_rate_hz / 10
            )),
        });
    }
    commands
}

fn tool_error(error: AudioInspectionDiagnostic) -> Error {
    Error::new(
        ErrorCategory::Dependency,
        format!("{:?}: {}: {}", error.code, error.tool, error.message),
    )
}

fn input<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .inputs
        .iter()
        .find(|value| value.port == port)
        .expect("validated input port")
}
fn output<'a>(
    request: &'a ProviderInvocationRequest,
    port: &str,
) -> &'a ProviderInvocationArtifactBinding {
    request
        .outputs
        .iter()
        .find(|value| value.port == port)
        .expect("validated output port")
}

fn verify_invocation(
    request: &ProviderInvocationRequest,
) -> Result<AudioSignalProviderConfiguration> {
    request.validate()?;
    let config: AudioSignalProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&request.configuration.values)
            .map_err(|_| invalid("invalid signal values"))?,
    )
    .map_err(|_| invalid("invalid closed signal provider configuration"))?;
    config.validate()?;
    if request.configuration != config.provider_configuration()? {
        return Err(invalid(
            "invocation must select the exact signal configuration",
        ));
    }
    if request.inputs.len() != 3 || request.outputs.len() != 2 {
        return Err(invalid(
            "signal provider requires exactly three inputs and two outputs",
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
            "inspection_analysis",
            "analysis",
            "application/vnd.aniflow.audio-analysis+json",
            ArtifactRole::ValidationEvidence,
            StreamRole::TimedMetadata,
        ),
    ] {
        let matches: Vec<_> = request
            .inputs
            .iter()
            .filter(|value| value.port == port)
            .collect();
        if matches.len() != 1 {
            return Err(invalid("missing or duplicate signal input port"));
        }
        let binding = matches[0];
        if binding.artifact_id != id
            || binding.artifact_type != mime
            || binding.artifact_role != role
            || binding.stream_role != Some(stream)
            || binding.kind != ArtifactKind::File
        {
            return Err(invalid(
                "signal input binding differs from the declared profile",
            ));
        }
    }
    for (port, mime) in [
        (
            "signal",
            "application/vnd.aniflow.audio-signal-measurements+json",
        ),
        (
            "signal_analysis",
            "application/vnd.aniflow.audio-analysis+json",
        ),
    ] {
        let matches: Vec<_> = request
            .outputs
            .iter()
            .filter(|value| value.port == port)
            .collect();
        if matches.len() != 1 {
            return Err(invalid("missing or duplicate signal output port"));
        }
        let binding = matches[0];
        if binding.artifact_id != port
            || binding.artifact_type != mime
            || binding.artifact_role != ArtifactRole::ValidationEvidence
            || binding.stream_role != Some(StreamRole::TimedMetadata)
            || binding.kind != ArtifactKind::File
            || binding.path.exists()
            || binding.path.is_symlink()
        {
            return Err(invalid(
                "signal output must be a new declared evidence file",
            ));
        }
    }
    Ok(config)
}

fn read_evidence(
    binding: &ProviderInvocationArtifactBinding,
    maximum: u64,
) -> Result<(Vec<u8>, AudioArtifactReference)> {
    let (sha256, byte_size) = hash_regular(&binding.path, maximum)
        .map_err(|_| invalid("upstream evidence must be a bounded nonsymlink regular file"))?;
    let mut bytes = Vec::new();
    File::open(&binding.path)
        .map_err(|_| invalid("upstream evidence unavailable"))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read upstream evidence"))?;
    if bytes.len() as u64 != byte_size || format!("{:x}", Sha256::digest(&bytes)) != sha256 {
        return Err(invalid("upstream evidence changed while reading"));
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

/// Execute a typed invocation within the existing Pipeline v3 process group.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    if !cfg!(unix) {
        return Err(invalid(
            "unsupported_platform: signal measurements require Unix process-group ownership",
        ));
    }
    let config = verify_invocation(request)?;
    let (technical_bytes, technical_artifact) =
        read_evidence(input(request, "technical"), 1_048_576)?;
    let technical = AudioTechnicalInspection::from_json_slice(&technical_bytes)?;
    let (analysis_bytes, inspection_analysis_artifact) =
        read_evidence(input(request, "inspection_analysis"), 8 * 1024 * 1024)?;
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
        || analysis.status != AudioAnalysisStatus::Complete
        || !analysis.artifacts.contains(&technical_artifact)
        || technical.tools[0].sha256 != config.tools.ffmpeg.sha256
        || technical.tools[0].version != config.tools.ffmpeg.version
        || technical.tools[1].sha256 != config.tools.ffprobe.sha256
        || technical.tools[1].version != config.tools.ffprobe.version
    {
        return Err(invalid(
            "upstream_mismatch: technical and normalized evidence disagree with the pinned source or tools",
        ));
    }
    let rate = technical.sample_rate_hz;
    if rate % 10 != 0 {
        return Err(invalid(
            "unsupported_signal_rate: this signal profile requires an exact 100ms sample grid",
        ));
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg").map_err(tool_error)?;
    verify_pin(&config.tools.ffprobe, "ffprobe").map_err(tool_error)?;
    let source_path = &input(request, "audio").path;
    let signal_path = &output(request, "signal").path;
    let analysis_path = &output(request, "signal_analysis").path;
    let parent = signal_path
        .parent()
        .ok_or_else(|| invalid("signal output requires assigned workspace"))?;
    if analysis_path.parent() != Some(parent) {
        return Err(invalid(
            "signal outputs must share an assigned parent workspace",
        ));
    }
    ensure_parent(parent)?;
    if source_path.canonicalize().ok().as_deref() != Some(source_path.as_path()) {
        return Err(invalid(
            "signal source must use a canonical nonsymlink path",
        ));
    }
    let expected_source = (config.source.sha256.clone(), config.source.byte_size);
    if hash_regular(source_path, AUDIO_INSPECTION_MAXIMUM_BYTES)
        .map_err(|_| invalid("signal source unavailable"))?
        != expected_source
    {
        return Err(invalid(
            "source_changed: signal source differs from planned identity",
        ));
    }
    let mut snapshot = tempfile::Builder::new()
        .prefix(".signal-snapshot-")
        .suffix(".wav")
        .tempfile_in(parent)
        .map_err(|_| invalid("cannot reserve signal source snapshot"))?;
    let copied = std::io::copy(
        &mut File::open(source_path)
            .map_err(|_| invalid("signal source unavailable"))?
            .take(AUDIO_INSPECTION_MAXIMUM_BYTES + 1),
        snapshot.as_file_mut(),
    )
    .map_err(|_| invalid("cannot snapshot signal source"))?;
    snapshot
        .as_file_mut()
        .sync_all()
        .map_err(|_| invalid("cannot sync signal snapshot"))?;
    if copied != config.source.byte_size
        || hash_regular(snapshot.path(), AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| invalid("signal snapshot invalid"))?
            != expected_source
    {
        return Err(invalid(
            "source_changed: signal snapshot differs from planned identity",
        ));
    }
    let wave = wav::inspect(snapshot.path())?;
    if wave.sample_rate_hz != rate
        || wave.channels != technical.channels
        || wave.frame_count != technical.frame_count
        || wave.pcm_sha256 != technical.pcm_sha256
    {
        return Err(invalid(
            "upstream_mismatch: signal snapshot PCM differs from decode evidence",
        ));
    }
    let native = pcm::measure(
        snapshot.path(),
        &wave,
        config.settings.silence_threshold_pcm,
        config.settings.minimum_silence_milliseconds,
        config.settings.clipping_threshold_pcm,
    )?;
    let cancellation = CancellationToken::default();
    let commands = command_evidence(rate);
    let run = |command: &AudioTechnicalCommandEvidence| {
        let arguments: Vec<OsString> = command
            .arguments
            .iter()
            .map(|value| {
                if value == "{snapshot}" {
                    snapshot.path().as_os_str().to_owned()
                } else {
                    OsString::from(value)
                }
            })
            .collect();
        run_tool(
            &config.tools.ffmpeg,
            "ffmpeg",
            &arguments,
            &config.tools,
            &cancellation,
            GroupPolicy::Inherit,
            Some(parent),
        )
        .map_err(tool_error)
    };
    let main_capture = run(&commands[0])?;
    let summary = ebur128::summary(&main_capture.stderr)?;
    let short_term = ebur128::short_term(&main_capture.stdout, rate, wave.frame_count)?;
    let true_peak = if let Some(command) = commands.get(1) {
        let capture = run(command)?;
        if !capture.stdout.is_empty() {
            return Err(invalid(
                "signal_tool_output: true-peak pass must not emit stdout",
            ));
        }
        ebur128::summary(&capture.stderr)?.true_peak
    } else {
        None
    };
    let implementation_sha256 = hash_regular(
        &std::env::current_exe().map_err(|_| invalid("signal provider identity unavailable"))?,
        512 * 1024 * 1024,
    )
    .map_err(|_| invalid("signal provider identity unavailable"))?
    .0;
    if implementation_sha256 != technical.implementation_sha256 {
        return Err(invalid(
            "upstream_mismatch: inspection and signal providers must use the same pinned native implementation",
        ));
    }
    let report = build_report(
        &config,
        request,
        analysis.source.clone(),
        technical_artifact,
        inspection_analysis_artifact,
        technical.tools.clone(),
        implementation_sha256,
        native,
        summary,
        short_term,
        true_peak,
    )?;
    let signal_bytes = report.canonical_json_bytes()?;
    let normalized = normalized_analysis(analysis, &report, &signal_bytes)?;
    let normalized_bytes = normalized.canonical_json_bytes()?;
    if signal_bytes.len() > 8 * 1024 * 1024 || normalized_bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid("signal evidence exceeds 8 MiB contract bound"));
    }
    for path in [source_path.as_path(), snapshot.path()] {
        if hash_regular(path, AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| invalid("source_changed"))?
            != expected_source
        {
            return Err(invalid(
                "source_changed: immutable source or snapshot changed during signal measurement",
            ));
        }
    }
    for (port, reference) in [
        ("technical", &report.technical_artifact),
        ("inspection_analysis", &report.inspection_analysis_artifact),
    ] {
        if hash_regular(&input(request, port).path, 8 * 1024 * 1024)
            .map_err(|_| invalid("upstream_changed"))?
            != (reference.sha256.clone(), reference.byte_size)
        {
            return Err(invalid(
                "upstream_changed: inspection evidence changed during signal measurement",
            ));
        }
    }
    verify_pin(&config.tools.ffmpeg, "ffmpeg").map_err(tool_error)?;
    verify_pin(&config.tools.ffprobe, "ffprobe").map_err(tool_error)?;
    snapshot
        .close()
        .map_err(|_| invalid("cannot remove private signal snapshot"))?;
    publish_new(signal_path, &signal_bytes)?;
    publish_new(analysis_path, &normalized_bytes)
}

fn ensure_parent(parent: &Path) -> Result<()> {
    let mut existing = parent;
    while !existing.exists() {
        if existing.is_symlink() {
            return Err(invalid("signal workspace cannot contain symlinks"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| invalid("signal workspace ancestor missing"))?;
    }
    if existing.canonicalize().ok().as_deref() != Some(existing) {
        return Err(invalid(
            "signal workspace must be canonical and symlink-free",
        ));
    }
    std::fs::create_dir_all(parent).map_err(|_| invalid("cannot create assigned signal workspace"))
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| invalid("cannot exclusively create signal evidence artifact"))?;
    output
        .write_all(bytes)
        .and_then(|()| output.sync_all())
        .map_err(|_| invalid("cannot publish complete signal evidence"))
}

fn measured(value: f64) -> AudioSignalValue {
    AudioSignalValue::Measured { value }
}
fn unavailable(reason: AudioSignalUnavailableReason) -> AudioSignalValue {
    AudioSignalValue::Unavailable { reason }
}
fn measurement(
    unit: AudioSignalUnit,
    scope: &AudioScope,
    value: AudioSignalValue,
) -> AudioSignalMeasurement {
    AudioSignalMeasurement {
        unit,
        scope: scope.clone(),
        value,
    }
}
fn logarithm(ratio: f64) -> AudioSignalValue {
    if ratio == 0.0 {
        unavailable(AudioSignalUnavailableReason::SilentInput)
    } else {
        measured(20.0 * ratio.log10())
    }
}

#[allow(clippy::too_many_arguments)]
fn build_report(
    config: &AudioSignalProviderConfiguration,
    request: &ProviderInvocationRequest,
    source: AudioSource,
    technical_artifact: AudioArtifactReference,
    inspection_analysis_artifact: AudioArtifactReference,
    tools: Vec<crate::audio_inspection::AudioToolObservation>,
    implementation_sha256: String,
    native: pcm::PcmMeasurements,
    summary: ebur128::EburSummary,
    short_values: Vec<ebur128::ShortTermValue>,
    true_peak: Option<f64>,
) -> Result<AudioSignalMeasurements> {
    use AudioSignalUnavailableReason as Reason;
    use AudioSignalUnit as Unit;
    let scope = AudioScope {
        channels: (0..source.channels).collect(),
        stem_id: None,
    };
    let method = AudioSignalMethod::for_sample_rate(source.sample_rate_hz);
    let silent = native.channels.iter().all(|channel| channel.peak == 0);
    let mut silence_regions = Vec::new();
    let mut clipping_regions = Vec::new();
    let channels = native
        .channels
        .into_iter()
        .enumerate()
        .map(|(index, channel)| {
            let channel_index = index as u16;
            let scope = AudioScope {
                channels: vec![channel_index],
                stem_id: None,
            };
            silence_regions.extend(channel.silence.into_iter().map(|range| AudioSignalRegion {
                channel: channel_index,
                range,
            }));
            clipping_regions.extend(channel.clipping.into_iter().map(|range| AudioSignalRegion {
                channel: channel_index,
                range,
            }));
            let crest = if channel.sum_squares == 0 {
                unavailable(Reason::SilentInput)
            } else {
                measured((20.0 * (channel.sample_peak_ratio / channel.rms_ratio).log10()).max(0.0))
            };
            AudioChannelSignal {
                channel: channel_index,
                sample_peak_ratio: channel.sample_peak_ratio,
                rms_ratio: channel.rms_ratio,
                sample_peak: measurement(
                    Unit::DecibelsFullScale,
                    &scope,
                    logarithm(channel.sample_peak_ratio),
                ),
                rms: measurement(
                    Unit::DecibelsFullScale,
                    &scope,
                    logarithm(channel.rms_ratio),
                ),
                crest_factor: measurement(Unit::Decibels, &scope, crest),
            }
        })
        .collect::<Vec<_>>();
    if silence_regions.len() > 10_000 || clipping_regions.len() > 10_000 {
        return Err(invalid(
            "signal_region_limit: report exceeds 10000 regions of one kind",
        ));
    }
    let integrated = if silent {
        unavailable(Reason::SilentInput)
    } else if source.frame_count < method.integrated_minimum_frames {
        unavailable(Reason::InsufficientDuration)
    } else if summary.integrated <= -70.0 {
        unavailable(Reason::BelowAbsoluteGate)
    } else {
        measured(summary.integrated)
    };
    let short_term = short_values
        .into_iter()
        .map(|value| {
            let loudness = if silent {
                unavailable(Reason::SilentInput)
            } else if value.loudness < -70.0 {
                unavailable(Reason::BelowMeasurementFloor)
            } else {
                measured(value.loudness)
            };
            AudioShortTermLoudness {
                range: value.range,
                measurement: measurement(Unit::LoudnessUnitsFullScale, &scope, loudness),
            }
        })
        .collect::<Vec<_>>();
    let lra_qualifying_windows = short_term.iter().filter(|entry| matches!(entry.measurement.value, AudioSignalValue::Measured { value } if value >= (-69.999_f64).max(summary.lra_threshold + 0.1))).count() as u32;
    let lra = if silent {
        unavailable(Reason::SilentInput)
    } else if source.frame_count < method.loudness_range_minimum_frames {
        unavailable(Reason::InsufficientDuration)
    } else if lra_qualifying_windows < method.loudness_range_minimum_gated_windows {
        unavailable(Reason::InsufficientGatedWindows)
    } else {
        measured(summary.loudness_range)
    };
    let peak = if source.sample_rate_hz > method.true_peak_max_sample_rate_hz {
        unavailable(Reason::UnsupportedTruePeakRate)
    } else if silent {
        if true_peak.is_some() {
            return Err(invalid(
                "signal_tool_output: silent PCM produced a finite true-peak measurement",
            ));
        }
        unavailable(Reason::SilentInput)
    } else {
        let value = true_peak.ok_or_else(|| {
            invalid("signal_tool_output: nonzero PCM produced no finite true-peak evidence")
        })?;
        let sample_peak = channels
            .iter()
            .map(|channel| channel.sample_peak_ratio)
            .fold(0.0_f64, f64::max);
        if value + f64::from(method.true_peak_sample_peak_tolerance_millidecibels) / 1000.0
            < 20.0 * sample_peak.log10()
        {
            return Err(invalid(
                "signal_tool_output: true-peak export is below independently measured sample peak",
            ));
        }
        measured(value)
    };
    let report = AudioSignalMeasurements {
        schema: AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V1.to_owned(),
        source: source.clone(),
        technical_artifact,
        inspection_analysis_artifact,
        provider: ProviderReference {
            id: AUDIO_SIGNAL_PROVIDER_ID.to_owned(),
            version: AUDIO_SIGNAL_PROVIDER_VERSION.to_owned(),
        },
        implementation_sha256,
        configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(),
        tools,
        settings: config.settings.clone(),
        method,
        commands: command_evidence(source.sample_rate_hz),
        channels,
        integrated_loudness: measurement(Unit::LoudnessUnitsFullScale, &scope, integrated),
        loudness_range: measurement(Unit::LoudnessUnits, &scope, lra),
        lra_threshold_lufs: summary.lra_threshold,
        lra_qualifying_windows,
        true_peak: measurement(Unit::DecibelsTruePeak, &scope, peak),
        short_term_status: if source.frame_count < u64::from(source.sample_rate_hz) * 3 {
            AudioSignalSeriesStatus::Unavailable {
                reason: Reason::InsufficientDuration,
            }
        } else {
            AudioSignalSeriesStatus::Measured {}
        },
        short_term,
        silence_regions,
        clipping_regions,
    };
    report.validate()?;
    Ok(report)
}

fn normalized_analysis(
    mut analysis: AudioAnalysis,
    report: &AudioSignalMeasurements,
    signal_bytes: &[u8],
) -> Result<AudioAnalysis> {
    let signal = AudioArtifactReference {
        id: "signal".to_owned(),
        sha256: format!("{:x}", Sha256::digest(signal_bytes)),
        byte_size: signal_bytes.len() as u64,
    };
    analysis
        .artifacts
        .push(report.inspection_analysis_artifact.clone());
    analysis.artifacts.push(signal);
    let provider_id = "audio-signal-provider";
    let evidence = vec![
        "source_audio".to_owned(),
        "technical".to_owned(),
        "analysis".to_owned(),
        "signal".to_owned(),
    ];
    let license = || AudioLicenseEvidence::Unavailable {
        reason: "license evidence is not collected by this signal measurement".to_owned(),
    };
    analysis.providers.push(AudioProviderEvidence {
        id: provider_id.to_owned(),
        provider: report.provider.clone(),
        implementation_sha256: report.implementation_sha256.clone(),
        configuration_sha256: report.configuration_sha256.clone(),
        tools: report
            .tools
            .iter()
            .map(|tool| AudioComponentEvidence {
                id: tool.id.clone(),
                version: tool.version.clone(),
                revision: tool.version.clone(),
                sha256: tool.sha256.clone(),
                license: license(),
            })
            .collect(),
        models: AudioModelEvidence::NoneRequired {},
        license: license(),
    });
    let partial = [
        &report.integrated_loudness.value,
        &report.loudness_range.value,
        &report.true_peak.value,
    ]
    .iter()
    .any(|value| matches!(value, AudioSignalValue::Unavailable { .. }))
        || matches!(
            report.short_term_status,
            AudioSignalSeriesStatus::Unavailable { .. }
        )
        || report.short_term.iter().any(|value| {
            matches!(
                value.measurement.value,
                AudioSignalValue::Unavailable { .. }
            )
        })
        || report
            .channels
            .iter()
            .any(|channel| channel.sample_peak_ratio == 0.0);
    let diagnostics = if partial {
        analysis.status = AudioAnalysisStatus::Partial;
        analysis.diagnostics.push(AudioDiagnostic { id: "signal-measurements-unavailable".to_owned(), code: "signal-measurements-unavailable".to_owned(), severity: AudioDiagnosticSeverity::Warning,
            message: "One or more signal quantities are explicitly unavailable; inspect the signal artifact for per-measurement reasons.".to_owned() });
        vec!["signal-measurements-unavailable".to_owned()]
    } else {
        Vec::new()
    };
    analysis.capabilities.push(AudioCapabilityOutcome {
        capability: crate::CapabilityReference {
            id: AUDIO_SIGNAL_CAPABILITY_ID.to_owned(),
            version: "1.0.0".to_owned(),
        },
        status: if partial {
            AudioAnalysisStatus::Partial
        } else {
            AudioAnalysisStatus::Complete
        },
        provider_evidence_ids: vec![provider_id.to_owned()],
        evidence_artifact_ids: evidence.clone(),
        diagnostic_ids: diagnostics,
    });
    for (kind, regions) in [
        ("silence", &report.silence_regions),
        ("clipping-threshold", &report.clipping_regions),
    ] {
        for channel in 0..report.source.channels {
            let events = regions.iter().filter(|region| region.channel == channel).enumerate().map(|(index, region)| AudioEvent {
                id: format!("signal-{kind}-{channel}-{index}"), range: region.range,
                label: if kind == "silence" { "PCM magnitude at or below configured silence threshold" } else { "PCM magnitude at or above configured clipping threshold; not proof of distortion" }.to_owned(),
                provenance: AudioObservationProvenance { class: AudioProvenanceClass::Deterministic, confidence: AudioConfidence::NotApplicable {}, provider_evidence_id: provider_id.to_owned(), evidence_artifact_ids: evidence.clone() },
            }).collect::<Vec<_>>();
            if !events.is_empty() {
                analysis.timelines.push(AudioTimeline {
                    id: format!("signal-{kind}-{channel}"),
                    capability_id: AUDIO_SIGNAL_CAPABILITY_ID.to_owned(),
                    kind: AudioTimelineKind::DisjointRegions,
                    scope: AudioScope {
                        channels: vec![channel],
                        stem_id: None,
                    },
                    events,
                });
            }
        }
    }
    analysis.validate()?;
    Ok(analysis)
}
