//! Strict native JSON admission and provider-neutral observed-text projection.
use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::audio_analysis::*;
use crate::timed_text::{
    CueTiming, OverlapPolicy, TIMED_TEXT_SCHEMA_V1, TextProvenance, TimedTextCue, TimedTextDocument,
};
use crate::{CapabilityReference, Result};

use super::types::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeObservation {
    systeminfo: String,
    model: WhisperModelParameters,
    params: NativeParameters,
    result: NativeResult,
    transcription: Vec<NativeSegment>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeParameters {
    model: String,
    language: String,
    translate: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeResult {
    language: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSegment {
    timestamps: NativeTimestamps,
    offsets: NativeOffsets,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeTimestamps {
    from: String,
    to: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeOffsets {
    from: u64,
    to: u64,
}

/// Admit the pinned basic whisper.cpp JSON dialect. The expected model path is
/// operational authority and is compared, then omitted from durable evidence.
/// This validates metadata only; the runtime separately verifies source bytes.
pub fn normalize(
    raw: &[u8],
    source: &AudioSource,
    expected_model_path: &str,
) -> Result<TranscriptionObservation> {
    bounded(
        raw,
        AUDIO_TRANSCRIPTION_MAXIMUM_RAW_BYTES,
        "native transcription JSON",
    )?;
    let native: NativeObservation =
        crate::provider::decode_json(raw, "native transcription observation")?;
    text(&native.systeminfo, 16384)?;
    text(&native.params.model, 4096)?;
    if !native
        .systeminfo
        .starts_with("WHISPER : COREML = 0 | OPENVINO = 0 | ")
    {
        return Err(invalid(
            "native transcript must explicitly report the admitted CPU-only CoreML/OpenVINO profile",
        ));
    }
    if native.params.model != expected_model_path
        || native.params.language != "en"
        || native.result.language != "en"
        || native.params.translate
        || native.transcription.len() > 10000
    {
        return Err(invalid(
            "native transcript has unexpected model authority, language, translation, or collection bounds",
        ));
    }
    let mut segments = Vec::with_capacity(native.transcription.len());
    for (index, segment) in native.transcription.into_iter().enumerate() {
        if segment.offsets.from > 600000
            || segment.offsets.to > 600000
            || segment.offsets.from % 10 != 0
            || segment.offsets.to % 10 != 0
            || segment.timestamps.from != timestamp(segment.offsets.from)
            || segment.timestamps.to != timestamp(segment.offsets.to)
        {
            return Err(invalid(
                "native display timestamps and exact centisecond offsets disagree",
            ));
        }
        segments.push(TranscriptionSegment {
            id: format!("transcription_segment_{:06}", index + 1),
            start: rational_milliseconds(segment.offsets.from),
            end: rational_milliseconds(segment.offsets.to),
            text: segment.text,
        });
    }
    let result = TranscriptionObservation {
        detected_language: native.result.language,
        model: native.model,
        segments,
    };
    result.validate(source)?;
    Ok(result)
}

/// Preserve nonempty observed segments in the existing timed-text contract.
/// No review authority is inferred, and an empty observation has no text export.
pub fn timed_text(
    observation: &TranscriptionObservation,
    source: &AudioSource,
    raw: &AudioArtifactReference,
) -> Result<Option<TimedTextDocument>> {
    observation.validate(source)?;
    reference(raw, AUDIO_TRANSCRIPTION_MAXIMUM_RAW_BYTES as u64)?;
    if raw.id != "transcription_observation" {
        return Err(invalid("unexpected captured transcript identity"));
    }
    if observation.segments.is_empty() {
        return Ok(None);
    }
    let document = TimedTextDocument {
        schema: TIMED_TEXT_SCHEMA_V1.into(),
        source: AudioArtifactReference {
            id: "source_text".into(),
            sha256: raw.sha256.clone(),
            byte_size: raw.byte_size,
        },
        provenance: TextProvenance::ObservedTranscript {
            producer: AUDIO_TRANSCRIPTION_PRODUCER.into(),
        },
        language: Some("en".into()),
        audio_source: Some(source.clone()),
        overlap_policy: OverlapPolicy::Reject,
        metadata: BTreeMap::new(),
        cues: observation
            .segments
            .iter()
            .map(|segment| TimedTextCue {
                id: segment.id.clone(),
                source_label: None,
                text: segment.text.clone(),
                timing: CueTiming::Interval {
                    start: segment.start,
                    end: segment.end,
                },
                speaker: None,
            })
            .collect(),
    };
    document.validate()?;
    Ok(Some(document))
}

/// Add transcript evidence to the already accepted inspection/lineage analysis.
/// The companion artifact is an observed transcript, never reviewed lyrics.
pub fn normalized_analysis(
    mut analysis: AudioAnalysis,
    report: &AudioTranscriptionReport,
    report_bytes: &[u8],
) -> Result<AudioAnalysis> {
    analysis.validate()?;
    report.validate()?;
    let upstream_bytes = analysis.canonical_json_bytes()?;
    if analysis.source != report.source
        || report_bytes != report.canonical_json_bytes()?
        || report.upstream_analysis_artifact.byte_size != upstream_bytes.len() as u64
        || report.upstream_analysis_artifact.sha256
            != format!("{:x}", Sha256::digest(&upstream_bytes))
        || !analysis
            .artifacts
            .iter()
            .any(|artifact| artifact == &report.technical_artifact)
    {
        return Err(invalid(
            "transcription source or companion bytes differ from their validated evidence",
        ));
    }
    if analysis
        .capabilities
        .iter()
        .any(|capability| capability.capability.id == AUDIO_TRANSCRIPTION_CAPABILITY_ID)
    {
        return Err(invalid("upstream analysis already declares transcription"));
    }
    let report_artifact = AudioArtifactReference {
        id: "transcription".into(),
        sha256: format!("{:x}", Sha256::digest(report_bytes)),
        byte_size: report_bytes.len() as u64,
    };
    for artifact in [report.upstream_analysis_artifact.clone(), report_artifact] {
        if analysis
            .artifacts
            .iter()
            .any(|existing| existing.id == artifact.id)
        {
            return Err(invalid(
                "transcription artifact identity collides with upstream evidence",
            ));
        }
        analysis.artifacts.push(artifact);
    }
    let provider_id = "audio-transcription-provider";
    let component_license=AudioLicenseEvidence::Recorded{statement:"The companion records upstream MIT declarations for whisper.cpp and Whisper model weights; digest pins do not authenticate their origin.".into(),evidence_artifact_id:"transcription".into()};
    analysis.providers.push(AudioProviderEvidence {
        id: provider_id.into(),
        provider: report.provider.clone(),
        implementation_sha256: report.implementation_sha256.clone(),
        configuration_sha256: report.configuration_sha256.clone(),
        tools: vec![AudioComponentEvidence {
            id: "whisper-cpp".into(),
            version: AUDIO_TRANSCRIPTION_WHISPER_VERSION.into(),
            revision: AUDIO_TRANSCRIPTION_WHISPER_REVISION.into(),
            sha256: report.settings.whisper_sha256.clone(),
            license: component_license.clone(),
        }],
        models: AudioModelEvidence::Available {
            components: vec![AudioComponentEvidence {
                id: "whisper-tiny-en".into(),
                version: report.settings.model.revision.clone(),
                revision: report.settings.model.revision.clone(),
                sha256: report.settings.model.sha256.clone(),
                license: component_license,
            }],
        },
        license: AudioLicenseEvidence::Unavailable {
            reason: "Native adapter license evidence is not collected by this observation.".into(),
        },
    });
    let evidence = vec![
        "source_audio".into(),
        "technical".into(),
        report.upstream_analysis_artifact.id.clone(),
        "transcription".into(),
    ];
    let provenance = AudioObservationProvenance {
        class: AudioProvenanceClass::Probabilistic,
        confidence: report.confidence.clone(),
        provider_evidence_id: provider_id.into(),
        evidence_artifact_ids: evidence.clone(),
    };
    let mut diagnostic_ids = Vec::new();
    let status = match &report.result {
        TranscriptionResult::Observed { observation } => {
            analysis.semantic_artifacts.push(AudioSemanticArtifact {
                artifact_id: "transcription".into(),
                capability_id: AUDIO_TRANSCRIPTION_CAPABILITY_ID.into(),
                kind: AudioSemanticArtifactKind::ObservedTranscript,
                provenance: provenance.clone(),
                authority: None,
            });
            let events = observation
                .segments
                .iter()
                .map(|segment| {
                    Ok(AudioEvent {
                        id: segment.id.clone(),
                        range: AudioFrameRange {
                            start: milliseconds(segment.start)? * 16,
                            end: milliseconds(segment.end)? * 16,
                        },
                        label: "observed transcript segment".into(),
                        provenance: provenance.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            analysis.timelines.push(AudioTimeline {
                id: "transcription-segments".into(),
                capability_id: AUDIO_TRANSCRIPTION_CAPABILITY_ID.into(),
                kind: AudioTimelineKind::DisjointRegions,
                scope: report.scope.clone(),
                events,
            });
            AudioAnalysisStatus::Complete
        }
        TranscriptionResult::Empty { .. } | TranscriptionResult::Unavailable { .. } => {
            let id = "transcription-unavailable".to_owned();
            let message = if matches!(report.result, TranscriptionResult::Empty { .. }) {
                "The analyzer returned no observed segments; this does not establish silence or absence of speech."
            } else {
                "The bounded transcription profile did not invoke the analyzer; inspect the companion's explicit reason."
            };
            analysis.diagnostics.push(AudioDiagnostic {
                id: id.clone(),
                code: id.clone(),
                severity: AudioDiagnosticSeverity::Warning,
                message: message.into(),
            });
            diagnostic_ids.push(id);
            analysis.status = AudioAnalysisStatus::Partial;
            AudioAnalysisStatus::Unavailable
        }
    };
    analysis.capabilities.push(AudioCapabilityOutcome {
        capability: CapabilityReference {
            id: AUDIO_TRANSCRIPTION_CAPABILITY_ID.into(),
            version: "1.0.0".into(),
        },
        status,
        provider_evidence_ids: vec![provider_id.into()],
        evidence_artifact_ids: evidence,
        diagnostic_ids,
    });
    analysis.validate()?;
    Ok(analysis)
}

fn timestamp(milliseconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02},{:03}",
        milliseconds / 3600000,
        milliseconds / 60000 % 60,
        milliseconds / 1000 % 60,
        milliseconds % 1000
    )
}
fn rational_milliseconds(milliseconds: u64) -> AudioRationalTime {
    let mut left = milliseconds;
    let mut right = 1000;
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    AudioRationalTime {
        numerator: (milliseconds / left) as i64,
        denominator: (1000 / left) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn source() -> AudioSource {
        AudioSource {
            artifact: AudioArtifactReference {
                id: "source_audio".into(),
                sha256: "a".repeat(64),
                byte_size: 64044,
            },
            stream_index: 0,
            sample_rate_hz: 16000,
            channels: 1,
            frame_count: 32000,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        }
    }
    fn raw() -> Value {
        json!({"systeminfo":"WHISPER : COREML = 0 | OPENVINO = 0 | CPU synthetic", "model":{"type":"tiny","multilingual":false,"vocab":51864,"audio":{"ctx":1500,"state":384,"head":6,"layer":4},"text":{"ctx":448,"state":384,"head":6,"layer":4},"mels":80,"ftype":1},"params":{"model":"/private/model.bin","language":"en","translate":false},"result":{"language":"en"},"transcription":[{"timestamps":{"from":"00:00:00,100","to":"00:00:01,250"},"offsets":{"from":100,"to":1250},"text":" Hello café 世界"}]})
    }
    fn parse(value: Value) -> Result<TranscriptionObservation> {
        normalize(
            &serde_json::to_vec(&value).unwrap(),
            &source(),
            "/private/model.bin",
        )
    }
    #[test]
    fn exact_segments_preserve_text_and_only_observed_authority() {
        let bytes = serde_json::to_vec(&raw()).unwrap();
        let observation = normalize(&bytes, &source(), "/private/model.bin").unwrap();
        assert_eq!(
            observation.segments[0].start,
            AudioRationalTime {
                numerator: 1,
                denominator: 10
            }
        );
        assert_eq!(
            observation.segments[0].end,
            AudioRationalTime {
                numerator: 5,
                denominator: 4
            }
        );
        let identity = AudioArtifactReference {
            id: "transcription_observation".into(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            byte_size: bytes.len() as u64,
        };
        let document = timed_text(&observation, &source(), &identity)
            .unwrap()
            .unwrap();
        assert!(matches!(
            document.provenance,
            TextProvenance::ObservedTranscript { .. }
        ));
        assert_eq!(document.cues[0].text, " Hello café 世界");
        assert_eq!(document.source.sha256, identity.sha256);
        assert_eq!(document.audio_source, Some(source()));
    }
    #[test]
    fn empty_observation_never_invents_a_timed_text_cue() {
        let mut value = raw();
        value["transcription"] = json!([]);
        let observation = parse(value).unwrap();
        let identity = AudioArtifactReference {
            id: "transcription_observation".into(),
            sha256: "b".repeat(64),
            byte_size: 12,
        };
        assert!(
            timed_text(&observation, &source(), &identity)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn rejects_malformed_native_timing_scope_language_and_models() {
        for pointer in [
            "/transcription/0/offsets/to",
            "/transcription/0/offsets/from",
        ] {
            let mut value = raw();
            value.pointer_mut(pointer).unwrap().clone_from(&json!(101));
            assert!(parse(value).is_err());
        }
        for (pointer, replacement) in [
            ("/params/translate", json!(true)),
            ("/result/language", json!("fr")),
            ("/model/multilingual", json!(true)),
            ("/model/vocab", json!(51865)),
            ("/model/ftype", json!(2)),
            ("/params/model", json!("/wrong/model")),
        ] {
            let mut value = raw();
            *value.pointer_mut(pointer).unwrap() = replacement;
            assert!(parse(value).is_err(), "{pointer}");
        }
        let mut value = raw();
        value["transcription"][0]["timestamps"]["to"] = json!("00:00:03,000");
        value["transcription"][0]["offsets"]["to"] = json!(3000);
        assert!(parse(value).is_err());
    }
    #[test]
    fn rejects_overlap_and_empty_interval_without_reordering_or_clipping() {
        let mut value = raw();
        let segment = value["transcription"][0].clone();
        value["transcription"].as_array_mut().unwrap().push(segment);
        assert!(parse(value).is_err());
        let mut value = raw();
        value["transcription"][0]["timestamps"]["to"] = json!("00:00:00,100");
        value["transcription"][0]["offsets"]["to"] = json!(100);
        assert!(parse(value).is_err());
    }
    #[test]
    fn closed_native_dialect_rejects_unknown_tokens_and_duplicate_fields() {
        let mut value = raw();
        value["transcription"][0]["tokens"] = json!([]);
        assert!(parse(value).is_err());
        let value = serde_json::to_string(&raw()).unwrap().replace(
            "\"translate\":false",
            "\"translate\":false,\"translate\":true",
        );
        assert!(normalize(value.as_bytes(), &source(), "/private/model.bin").is_err());
    }

    #[test]
    fn native_backend_evidence_requires_both_explicit_cpu_profile_flags() {
        for system in [
            "WHISPER : CPU synthetic",
            "WHISPER : COREML = 1 | OPENVINO = 0 | CPU synthetic",
            "WHISPER : COREML = 0 | OPENVINO = 1 | CPU synthetic",
        ] {
            let mut value = raw();
            value["systeminfo"] = json!(system);
            assert!(parse(value).is_err());
        }
    }

    #[test]
    fn normalized_projection_binds_the_exact_upstream_evidence_not_only_its_source() {
        let fixture =
            include_str!("../../docs/contracts/examples/audio-analysis-technical-v1.example.json")
                .replace("synthetic_source", "source_audio")
                .replace("synthetic_evidence", "technical");
        let upstream = AudioAnalysis::from_json_slice(fixture.as_bytes()).unwrap();
        let upstream_bytes = upstream.canonical_json_bytes().unwrap();
        let report = AudioTranscriptionReport {
            schema: AUDIO_TRANSCRIPTION_REPORT_SCHEMA_V1.into(),
            source: upstream.source.clone(),
            scope: AudioScope {
                channels: vec![0, 1],
                stem_id: None,
            },
            technical_artifact: upstream.artifacts[0].clone(),
            upstream_analysis_artifact: AudioArtifactReference {
                id: "analysis".into(),
                sha256: format!("{:x}", Sha256::digest(&upstream_bytes)),
                byte_size: upstream_bytes.len() as u64,
            },
            raw_observation: None,
            provider: crate::ProviderReference {
                id: AUDIO_TRANSCRIPTION_PROVIDER_ID.into(),
                version: AUDIO_TRANSCRIPTION_PROVIDER_VERSION.into(),
            },
            implementation_sha256: "a".repeat(64),
            configuration_sha256: "b".repeat(64),
            provider_lock_sha256: "c".repeat(64),
            settings: TranscriptionSettingsEvidence {
                whisper_version: AUDIO_TRANSCRIPTION_WHISPER_VERSION.into(),
                whisper_sha256: "d".repeat(64),
                model: TranscriptionModelEvidence {
                    sha256: "e".repeat(64),
                    byte_size: 100,
                    model_id: "tiny.en".into(),
                    revision: "synthetic".into(),
                },
                language: "en".into(),
                threads: 1,
                tool_timeout_milliseconds: 5000,
                maximum_tool_output_bytes: 1048576,
            },
            licenses: TranscriptionLicenseEvidence::default(),
            commands: super::super::provider::command_evidence(1, false),
            method: TranscriptionMethod::default(),
            provenance: AudioProvenanceClass::Probabilistic,
            confidence: AudioConfidence::Unavailable {
                reason: AUDIO_TRANSCRIPTION_CONFIDENCE_REASON.into(),
            },
            word_timing: TranscriptionWordTiming::Unavailable {
                reason: AUDIO_TRANSCRIPTION_WORD_TIMING_REASON.into(),
            },
            result: TranscriptionResult::Unavailable {
                reason: TranscriptionUnavailableReason::UnsupportedSampleRate,
            },
            timed_text: None,
        };
        let bytes = report.canonical_json_bytes().unwrap();
        assert!(normalized_analysis(upstream.clone(), &report, &bytes).is_ok());
        let mut changed = upstream.clone();
        changed.diagnostics.push(AudioDiagnostic {
            id: "additional-evidence".into(),
            code: "additional-evidence".into(),
            severity: AudioDiagnosticSeverity::Info,
            message: "This separately valid evidence was not bound by the report.".into(),
        });
        changed.validate().unwrap();
        assert_eq!(changed.source, upstream.source);
        assert!(normalized_analysis(changed, &report, &bytes).is_err());
        let mut changed_report = report;
        changed_report.technical_artifact.sha256 = "f".repeat(64);
        let bytes = changed_report.canonical_json_bytes().unwrap();
        assert!(normalized_analysis(upstream, &changed_report, &bytes).is_err());
    }
}
