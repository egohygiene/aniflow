//! Typed native provider for one exact local technical inspection profile.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::process::{GroupPolicy, hash_regular, run_tool, verify_pin};
use super::types::*;
use super::wav::{self, NativeSampleFormat};
use crate::audio_analysis::*;
use crate::{
    ArtifactKind, ArtifactRole, CancellationToken, Error, ErrorCategory, ProviderInvocationRequest,
    ProviderReference, Result, StreamRole,
};

const PROBE_ARGUMENTS: &[&str] = &[
    "-v",
    "error",
    "-protocol_whitelist",
    "file,pipe",
    "-show_entries",
    "stream=index,codec_type,codec_name,sample_fmt,sample_rate,channels,bits_per_sample,time_base,duration_ts,start_pts,bit_rate:format=format_name,nb_streams",
    "-of",
    "json",
    "-i",
    "{snapshot}",
];
const NATIVE_PROBE_ARGUMENTS: &[&str] = &[
    "-v",
    "error",
    "-protocol_whitelist",
    "file,pipe",
    "-show_entries",
    "stream=index,codec_type,codec_name,sample_fmt,sample_rate,channels,bits_per_sample,bits_per_raw_sample,time_base,duration_ts,start_pts,bit_rate:format=format_name,nb_streams",
    "-of",
    "json",
    "-i",
    "{snapshot}",
];
const DECODE_ARGUMENTS: &[&str] = &[
    "-v",
    "error",
    "-nostdin",
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
    "-codec:a",
    "pcm_s16le",
    "-f",
    "hash",
    "-hash",
    "sha256",
    "pipe:1",
];

pub(super) fn command_evidence() -> Vec<AudioTechnicalCommandEvidence> {
    [("ffprobe", PROBE_ARGUMENTS), ("ffmpeg", DECODE_ARGUMENTS)]
        .into_iter()
        .map(|(tool, args)| AudioTechnicalCommandEvidence {
            tool: tool.to_owned(),
            arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
        })
        .collect()
}

fn native_decode_arguments(sample_format: NativeSampleFormat) -> Vec<&'static str> {
    DECODE_ARGUMENTS
        .iter()
        .map(|value| {
            if *value == "pcm_s16le" {
                sample_format.codec()
            } else {
                *value
            }
        })
        .collect()
}

pub(super) fn native_command_evidence(
    sample_format: NativeSampleFormat,
) -> Vec<AudioTechnicalCommandEvidence> {
    let decode = native_decode_arguments(sample_format);
    [
        ("ffprobe", NATIVE_PROBE_ARGUMENTS),
        ("ffmpeg", decode.as_slice()),
    ]
    .into_iter()
    .map(|(tool, args)| AudioTechnicalCommandEvidence {
        tool: tool.to_owned(),
        arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
    })
    .collect()
}

fn arguments(template: &[&str], snapshot: &Path) -> Vec<OsString> {
    template
        .iter()
        .map(|value| {
            if *value == "{snapshot}" {
                snapshot.as_os_str().to_owned()
            } else {
                OsString::from(value)
            }
        })
        .collect()
}

pub fn preflight(
    configuration: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> Result<AudioInspectionPreflight> {
    preflight_with_policy(configuration, cancellation, false)
}

fn preflight_with_policy(
    configuration: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
    inherited: bool,
) -> Result<AudioInspectionPreflight> {
    configuration.validate()?;
    let mut report = AudioInspectionPreflight {
        schema: AUDIO_INSPECTION_PREFLIGHT_SCHEMA_V1.to_owned(),
        ready: false,
        tools: Vec::new(),
        diagnostics: Vec::new(),
    };
    for (id, pin) in [
        ("ffmpeg", &configuration.ffmpeg),
        ("ffprobe", &configuration.ffprobe),
    ] {
        let policy = if inherited {
            GroupPolicy::Inherit
        } else {
            GroupPolicy::Own
        };
        match run_tool(
            pin,
            id,
            &[OsString::from("-version")],
            configuration,
            cancellation,
            policy,
            None,
        ) {
            Ok(capture) => {
                let text = String::from_utf8(capture.stdout).unwrap_or_default();
                let mut tokens = text.lines().next().unwrap_or_default().split_whitespace();
                if tokens.next() != Some(id)
                    || tokens.next() != Some("version")
                    || tokens.next() != Some(pin.version.as_str())
                {
                    report.diagnostics.push(super::process::failure(
                        AudioInspectionDiagnosticCode::ToolVersionMismatch,
                        id,
                        "tool version banner does not match the exact configured token",
                    ));
                } else {
                    report.tools.push(AudioToolObservation {
                        id: id.to_owned(),
                        version: pin.version.clone(),
                        sha256: pin.sha256.clone(),
                    });
                }
            }
            Err(diagnostic) => report.diagnostics.push(diagnostic),
        }
    }
    report.ready = report.diagnostics.is_empty() && report.tools.len() == 2;
    Ok(report)
}

fn tool_error(error: AudioInspectionDiagnostic) -> Error {
    Error::new(
        ErrorCategory::Dependency,
        format!("{:?}: {}: {}", error.code, error.tool, error.message),
    )
}

fn verify_invocation(
    request: &ProviderInvocationRequest,
) -> Result<AudioInspectionProviderConfiguration> {
    request.validate()?;
    let encoded = serde_json::to_vec(&request.configuration.values)
        .map_err(|_| invalid("invalid provider values"))?;
    let config: AudioInspectionProviderConfiguration = serde_json::from_slice(&encoded)
        .map_err(|_| invalid("invalid closed audio inspection provider configuration"))?;
    config.validate()?;
    if config.schema != AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2 {
        return Err(invalid(
            "native inspection execution requires the explicit v2 provider configuration",
        ));
    }
    if request.configuration != config.provider_configuration()? {
        return Err(invalid(
            "invocation does not select the exact audio inspection configuration",
        ));
    }
    if request.inputs.len() != 1 || request.outputs.len() != 2 {
        return Err(invalid(
            "inspection requires exactly one input and two output bindings",
        ));
    }
    let input = &request.inputs[0];
    if input.port != "audio"
        || input.artifact_id != "source_audio"
        || input.artifact_type != "audio/wav"
        || input.artifact_role != ArtifactRole::TemporalSource
        || input.stream_role != Some(StreamRole::Audio)
        || input.kind != ArtifactKind::File
    {
        return Err(invalid(
            "input binding does not match the audio inspection profile",
        ));
    }
    for (port, mime) in [
        (
            "technical",
            "application/vnd.aniflow.audio-technical-inspection+json",
        ),
        ("analysis", "application/vnd.aniflow.audio-analysis+json"),
    ] {
        let matches = request
            .outputs
            .iter()
            .filter(|output| output.port == port)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(invalid("missing or duplicate inspection output port"));
        }
        let output = matches[0];
        if output.artifact_id != port
            || output.artifact_type != mime
            || output.artifact_role != ArtifactRole::ValidationEvidence
            || output.stream_role != Some(StreamRole::TimedMetadata)
            || output.kind != ArtifactKind::File
            || output.path.exists()
            || output.path.is_symlink()
        {
            return Err(invalid(
                "inspection output binding must be a new declared evidence file",
            ));
        }
    }
    Ok(config)
}

/// Execute only the typed provider ABI. The outer Pipeline v3 runtime owns this
/// process group, overall deadline, cancellation, candidate validation and commit.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    if !cfg!(unix) {
        return Err(invalid(
            "unsupported_platform: audio inspection requires Unix process-group ownership",
        ));
    }
    let config = verify_invocation(request)?;
    let cancellation = CancellationToken::default();
    let preflight = preflight_with_policy(&config.settings, &cancellation, true)?;
    if !preflight.is_ready() {
        return Err(tool_error(preflight.diagnostics[0].clone()));
    }
    let source = &request.inputs[0].path;
    let technical_path = &request
        .outputs
        .iter()
        .find(|output| output.port == "technical")
        .expect("validated port")
        .path;
    let analysis_path = &request
        .outputs
        .iter()
        .find(|output| output.port == "analysis")
        .expect("validated port")
        .path;
    let parent = technical_path
        .parent()
        .ok_or_else(|| invalid("output needs a parent workspace"))?;
    if analysis_path.parent() != Some(parent) {
        return Err(invalid(
            "inspection evidence outputs must share their assigned parent workspace",
        ));
    }
    ensure_parent(parent)?;
    if source.canonicalize().ok().as_deref() != Some(source.as_path()) {
        return Err(invalid("source must use a canonical nonsymlink path"));
    }
    let observed = hash_regular(source, AUDIO_INSPECTION_MAXIMUM_BYTES)
        .map_err(|_| invalid("source_changed: source cannot be inspected safely"))?;
    if observed != (config.source.sha256.clone(), config.source.byte_size) {
        return Err(invalid(
            "source_changed: source does not match planned identity",
        ));
    }
    let mut snapshot = tempfile::Builder::new()
        .prefix(".audio-snapshot-")
        .suffix(".wav")
        .tempfile_in(parent)
        .map_err(|_| invalid("cannot reserve private source snapshot"))?;
    let copied = std::io::copy(
        &mut File::open(source)
            .map_err(|_| invalid("source unavailable"))?
            .take(AUDIO_INSPECTION_MAXIMUM_BYTES + 1),
        snapshot.as_file_mut(),
    )
    .map_err(|_| invalid("failed to snapshot source"))?;
    snapshot
        .as_file_mut()
        .sync_all()
        .map_err(|_| invalid("failed to sync source snapshot"))?;
    if copied != config.source.byte_size
        || hash_regular(snapshot.path(), AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| invalid("invalid snapshot"))?
            .0
            != config.source.sha256
    {
        return Err(invalid(
            "source_changed: private snapshot does not match planned identity",
        ));
    }
    let wave = wav::inspect_native(snapshot.path())?;
    let probe = run_tool(
        &config.settings.ffprobe,
        "ffprobe",
        &arguments(NATIVE_PROBE_ARGUMENTS, snapshot.path()),
        &config.settings,
        &cancellation,
        GroupPolicy::Inherit,
        Some(parent),
    )
    .map_err(tool_error)?;
    validate_probe(&probe.stdout, &wave)?;
    let decoded = run_tool(
        &config.settings.ffmpeg,
        "ffmpeg",
        &arguments(&native_decode_arguments(wave.sample_format), snapshot.path()),
        &config.settings,
        &cancellation,
        GroupPolicy::Inherit,
        Some(parent),
    )
    .map_err(tool_error)?;
    let decoded = std::str::from_utf8(&decoded.stdout)
        .map_err(|_| invalid("invalid decode evidence"))?
        .trim();
    let decoded = decoded
        .strip_prefix("SHA256=")
        .ok_or_else(|| invalid("invalid decode hash evidence"))?;
    if decoded != wave.pcm_sha256 {
        return Err(invalid(
            "decode_mismatch: decoded PCM differs from independently inspected PCM bytes",
        ));
    }
    if hash_regular(source, AUDIO_INSPECTION_MAXIMUM_BYTES)
        .map_err(|_| invalid("source_changed"))?
        != observed
        || hash_regular(snapshot.path(), AUDIO_INSPECTION_MAXIMUM_BYTES)
            .map_err(|_| invalid("snapshot_changed"))?
            != observed
    {
        return Err(invalid(
            "source_changed: source or private snapshot changed during inspection",
        ));
    }
    verify_pin(&config.settings.ffmpeg, "ffmpeg").map_err(tool_error)?;
    verify_pin(&config.settings.ffprobe, "ffprobe").map_err(tool_error)?;
    let implementation_sha256 = hash_regular(
        &std::env::current_exe()
            .map_err(|_| invalid("provider executable identity unavailable"))?,
        512 * 1024 * 1024,
    )
    .map_err(|_| invalid("provider executable identity unavailable"))?
    .0;
    let audio_source = AudioSource {
        artifact: config.source.clone(),
        stream_index: 0,
        sample_rate_hz: wave.sample_rate_hz,
        channels: wave.channels,
        frame_count: wave.frame_count,
        origin: AudioRationalTime {
            numerator: 0,
            denominator: 1,
        },
        stem: None,
    };
    let report = AudioTechnicalInspection {
        schema: AUDIO_TECHNICAL_INSPECTION_SCHEMA_V2.to_owned(),
        source: config.source,
        provider: ProviderReference {
            id: AUDIO_INSPECTION_PROVIDER_ID.to_owned(),
            version: AUDIO_INSPECTION_PROVIDER_VERSION.to_owned(),
        },
        implementation_sha256,
        configuration_sha256: request.configuration.effective_configuration_sha256.clone(),
        provider_lock_sha256: request.provider_lock_sha256.clone(),
        tools: preflight.tools,
        container: "wav".to_owned(),
        codec: wave.sample_format.codec().to_owned(),
        sample_format: wave.sample_format.sample_format().to_owned(),
        stream_index: 0,
        sample_rate_hz: wave.sample_rate_hz,
        channels: wave.channels,
        frame_count: wave.frame_count,
        duration: audio_source.time_for_frame(wave.frame_count)?,
        pcm_bitrate_bits_per_second: u64::from(wave.sample_rate_hz)
            * u64::from(wave.channels)
            * u64::from(wave.sample_format.bits_per_sample()),
        pcm_sha256: wave.pcm_sha256,
        decoded_pcm_sha256: decoded.to_owned(),
        decode_complete: true,
        source_unchanged: true,
        commands: native_command_evidence(wave.sample_format),
    };
    let technical_bytes = report.canonical_json_bytes()?;
    let analysis = normalized_analysis(&report, audio_source, &technical_bytes)?;
    let analysis_bytes = analysis.canonical_json_bytes()?;
    snapshot
        .close()
        .map_err(|_| invalid("cannot remove private source snapshot"))?;
    publish_new(technical_path, &technical_bytes)?;
    publish_new(analysis_path, &analysis_bytes)
}

fn ensure_parent(parent: &Path) -> Result<()> {
    let mut existing = parent;
    while !existing.exists() {
        if existing.is_symlink() {
            return Err(invalid("output workspace cannot contain symlinks"));
        }
        existing = existing
            .parent()
            .ok_or_else(|| invalid("missing output workspace ancestor"))?;
    }
    if existing.canonicalize().ok().as_deref() != Some(existing) {
        return Err(invalid(
            "output workspace must be canonical and symlink-free",
        ));
    }
    std::fs::create_dir_all(parent).map_err(|_| invalid("cannot create assigned output workspace"))
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| invalid("cannot exclusively create assigned evidence artifact"))?;
    output
        .write_all(bytes)
        .and_then(|()| output.sync_all())
        .map_err(|_| invalid("cannot publish complete evidence bytes"))
}

fn validate_probe(bytes: &[u8], wave: &wav::PcmWave) -> Result<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("probe_mismatch: ffprobe did not return JSON"))?;
    let streams = value["streams"]
        .as_array()
        .ok_or_else(|| invalid("probe_mismatch: missing audio stream evidence"))?;
    if streams.len() != 1
        || value["format"]["format_name"].as_str() != Some("wav")
        || value["format"]["nb_streams"].as_u64() != Some(1)
    {
        return Err(invalid(
            "unsupported_audio: ffprobe did not observe exactly one WAV audio stream",
        ));
    }
    let stream = &streams[0];
    let rate = wave.sample_rate_hz.to_string();
    let time_base = format!("1/{}", wave.sample_rate_hz);
    if stream["index"].as_u64() != Some(0)
        || stream["codec_type"].as_str() != Some("audio")
        || stream["codec_name"].as_str() != Some(wave.sample_format.codec())
        || stream["sample_fmt"].as_str() != Some(wave.sample_format.sample_format())
        || stream["sample_rate"].as_str() != Some(rate.as_str())
        || stream["channels"].as_u64() != Some(u64::from(wave.channels))
        || stream["bits_per_sample"].as_u64()
            != Some(u64::from(wave.sample_format.bits_per_sample()))
        || stream["time_base"].as_str() != Some(time_base.as_str())
        || stream["duration_ts"].as_u64() != Some(wave.frame_count)
        || stream
            .get("start_pts")
            .is_some_and(|start| start.as_i64() != Some(0))
    {
        return Err(invalid(
            "probe_mismatch: ffprobe identity or exact timing differs from independently parsed PCM WAV",
        ));
    }
    let raw_bits = stream.get("bits_per_raw_sample");
    let expected_bits = wave.sample_format.bits_per_sample().to_string();
    if (wave.sample_format == NativeSampleFormat::Pcm24
        && raw_bits.and_then(serde_json::Value::as_str) != Some("24"))
        || raw_bits.is_some_and(|bits| {
            bits.as_str() != Some(expected_bits.as_str()) && bits.as_str() != Some("0")
        })
    {
        return Err(invalid(
            "probe_mismatch: native sample precision differs from independently parsed WAV",
        ));
    }
    if let Some(bitrate) = stream.get("bit_rate") {
        let expected = (u64::from(wave.sample_rate_hz)
            * u64::from(wave.channels)
            * u64::from(wave.sample_format.bits_per_sample()))
        .to_string();
        if bitrate.as_str() != Some(expected.as_str()) {
            return Err(invalid(
                "probe_mismatch: PCM bitrate differs from source truth",
            ));
        }
    }
    Ok(())
}

fn normalized_analysis(
    report: &AudioTechnicalInspection,
    source: AudioSource,
    technical_bytes: &[u8],
) -> Result<AudioAnalysis> {
    let technical = AudioArtifactReference {
        id: "technical".to_owned(),
        sha256: format!("{:x}", Sha256::digest(technical_bytes)),
        byte_size: technical_bytes.len() as u64,
    };
    let evidence = vec!["source_audio".to_owned(), "technical".to_owned()];
    let provenance = AudioObservationProvenance {
        class: AudioProvenanceClass::Deterministic,
        confidence: AudioConfidence::NotApplicable {},
        provider_evidence_id: "audio-inspection-provider".to_owned(),
        evidence_artifact_ids: evidence.clone(),
    };
    let scope = AudioScope {
        channels: (0..source.channels).collect(),
        stem_id: None,
    };
    let license = || AudioLicenseEvidence::Unavailable {
        reason: "license evidence is not collected by this technical inspection".to_owned(),
    };
    let analysis = AudioAnalysis {
        schema: AUDIO_ANALYSIS_SCHEMA_V1.to_owned(),
        status: AudioAnalysisStatus::Complete,
        source: source.clone(),
        artifacts: vec![technical],
        providers: vec![AudioProviderEvidence {
            id: "audio-inspection-provider".to_owned(),
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
        }],
        capabilities: vec![AudioCapabilityOutcome {
            capability: crate::CapabilityReference {
                id: AUDIO_INSPECTION_CAPABILITY_ID.to_owned(),
                version: AUDIO_INSPECTION_CAPABILITY_VERSION.to_owned(),
            },
            status: AudioAnalysisStatus::Complete,
            provider_evidence_ids: vec!["audio-inspection-provider".to_owned()],
            evidence_artifact_ids: evidence.clone(),
            diagnostic_ids: Vec::new(),
        }],
        observations: vec![
            AudioObservation {
                id: "sample-rate".to_owned(),
                capability_id: AUDIO_INSPECTION_CAPABILITY_ID.to_owned(),
                kind: AudioObservationKind::SampleRate,
                value: AudioObservationValue::Quantity {
                    value: f64::from(source.sample_rate_hz),
                    unit: AudioUnit::Hertz,
                },
                scope: scope.clone(),
                provenance: provenance.clone(),
            },
            AudioObservation {
                id: "channel-count".to_owned(),
                capability_id: AUDIO_INSPECTION_CAPABILITY_ID.to_owned(),
                kind: AudioObservationKind::ChannelCount,
                value: AudioObservationValue::Count {
                    value: u64::from(source.channels),
                    unit: AudioUnit::Channels,
                },
                scope: scope.clone(),
                provenance,
            },
        ],
        timelines: Vec::new(),
        semantic_artifacts: Vec::new(),
        excerpts: vec![AudioExcerptReference {
            id: "source-span".to_owned(),
            artifact_id: "source_audio".to_owned(),
            range: AudioFrameRange {
                start: 0,
                end: source.frame_count,
            },
            scope,
            evidence_artifact_ids: evidence,
        }],
        diagnostics: Vec::new(),
        extensions: BTreeMap::new(),
    };
    analysis.validate()?;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_probe_must_agree_with_the_exact_source_precision_and_clock() {
        for format in [
            NativeSampleFormat::Pcm16,
            NativeSampleFormat::Pcm24,
            NativeSampleFormat::Float32,
        ] {
            let wave = wav::PcmWave {
                sample_format: format,
                sample_rate_hz: 48_000,
                channels: 2,
                frame_count: 480,
                pcm_sha256: "a".repeat(64),
                data_offset: 44,
                data_bytes: 480 * 2 * u64::from(format.bytes_per_sample()),
            };
            let probe = serde_json::json!({
                "format": {"format_name": "wav", "nb_streams": 1},
                "streams": [{
                    "index": 0,
                    "codec_type": "audio",
                    "codec_name": format.codec(),
                    "sample_fmt": format.sample_format(),
                    "sample_rate": "48000",
                    "channels": 2,
                    "bits_per_sample": format.bits_per_sample(),
                    "bits_per_raw_sample": format.bits_per_sample().to_string(),
                    "time_base": "1/48000",
                    "duration_ts": 480,
                    "start_pts": 0,
                    "bit_rate": (48_000 * 2 * u64::from(format.bits_per_sample())).to_string()
                }]
            });
            validate_probe(&serde_json::to_vec(&probe).unwrap(), &wave).unwrap();
            for (key, value) in [
                ("bits_per_sample", serde_json::json!(8)),
                ("bits_per_raw_sample", serde_json::json!("8")),
                ("codec_name", serde_json::json!("pcm_s32le")),
                ("sample_fmt", serde_json::json!("dbl")),
                ("duration_ts", serde_json::json!(481)),
            ] {
                let mut changed = probe.clone();
                changed["streams"][0][key] = value;
                assert!(validate_probe(&serde_json::to_vec(&changed).unwrap(), &wave).is_err());
            }
            if format == NativeSampleFormat::Pcm24 {
                let mut missing = probe;
                missing["streams"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("bits_per_raw_sample");
                assert!(validate_probe(&serde_json::to_vec(&missing).unwrap(), &wave).is_err());
            }
        }
    }
}
