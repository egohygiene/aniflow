//! Selected gain audio in a fresh H.264/AAC candidate. Existing video observers,
//! native v3 state and final-file QC retain their separate authorities.
use super::{gain, policy_check};
use crate::destination::{MediaFacts, MediaInput, cancelled, invalid, local};
use crate::final_video::{FinalVideoArtifact, FinalVideoHandoff};
use crate::frame_io::identity;
use crate::provider::{canonical_json_bytes, canonical_sha256};
use crate::video_assembly::AssemblyConfiguration;
use crate::{CancellationToken, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
mod native;
mod runtime;
pub(crate) use native::execute_invocation;
pub use runtime::{completion, plan, resume, run};
pub const PROVIDER_ID: &str = "org.egohygiene.aniflow.gain-video";
pub const CAPABILITY_ID: &str = "aniflow/gain-video";
const SCHEMA: &str = "aniflow.gain-video-operation/v1";
const REPORT_BYTES: u64 = 67_108_864;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub gain_run_directory: PathBuf,
    pub gain_qc_directory: PathBuf,
    pub gain_qc_receipt_sha256: String,
    pub settings: AssemblyConfiguration,
    pub audio_bitrate_kbps: u16,
    pub provider_executable: PathBuf,
}
pub(super) fn selected_files(
    request: &Request,
    token: &CancellationToken,
) -> Result<Vec<crate::destination::MediaFile>> {
    let operation = current(request, token)?;
    Ok(vec![
        crate::destination::MediaFile {
            locator: operation.artifact.locator,
            sha256: operation.artifact.sha256,
            byte_count: operation.artifact.byte_count,
        },
        operation.gain.file,
    ])
}
impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("gain video request exceeds 64 KiB"));
        }
        let r: Self = crate::provider::decode_json(bytes, "gain video request")?;
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        crate::provider::require_sha256(&self.gain_qc_receipt_sha256, "gain final-file QC")?;
        if self.schema != "aniflow.gain-video-request/v1"
            || self.settings.ffmpeg.version != "8.0.1"
            || self.settings.ffprobe.version != "8.0.1"
            || self.settings.delivery_metadata.is_none()
            || !(64..=320).contains(&self.audio_bitrate_kbps)
            || self.audio_bitrate_kbps % 16 != 0
            || self.provider_executable.as_os_str().is_empty()
        {
            return Err(invalid(
                "gain video requires explicit current candidate QC, metadata policy, exact tools, bounded selected AAC bitrate and provider",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub schema: String,
    pub request: Request,
    pub source: FinalVideoHandoff,
    pub artifact: FinalVideoArtifact,
    pub gain: gain::Completion,
    pub gain_assessment: policy_check::Assessment,
    pub components: Vec<gain::Component>,
}
impl Operation {
    pub(crate) fn validate(&self) -> Result<()> {
        self.request.validate()?;
        policy_check::validate_snapshot(&self.gain_assessment)?;
        let g = &self.gain.report.operation;
        let expected = policy_check::Request {
            schema: "aniflow.audio-policy-check-request/v1".into(),
            input: MediaInput::AcceptedAudioWithQc {
                subject: super::final_lossless_qc::FinalLosslessSubject::AcceptedGainCandidate {
                    run_directory: self.request.gain_run_directory.clone(),
                },
                qc_directory: self.request.gain_qc_directory.clone(),
                qc_receipt_sha256: self.request.gain_qc_receipt_sha256.clone(),
            },
            policy: g.request.check.policy.clone(),
        };
        let original = match &g.source.assessment.subject.facts {
            MediaFacts::VideoWithQc {
                completion,
                artifact_id,
                ..
            } => (completion, artifact_id),
            _ => {
                return Err(invalid(
                    "gain video requires a gain candidate derived from this exact video's QC",
                ));
            }
        };
        if self.schema != SCHEMA
            || self.source != *original.0
            || &self.artifact.artifact_id != original.1
            || !self.source.artifacts.contains(&self.artifact)
            || self.artifact.variant != crate::final_video::FinalVideoVariant::DestinationCopy
            || self.artifact.audio.is_none()
            || self.artifact.video_codec != "h264"
            || self.artifact.pixel_format != "yuv420p"
            || self.artifact.color_primaries != "bt709"
            || self.artifact.color_transfer != "bt709"
            || self.artifact.color_matrix != "bt709"
            || self.artifact.color_range != "tv"
            || self.artifact.orientation_degrees != 0
            || !crate::final_video::square_pixels(&self.artifact.sample_aspect_ratio)
            || self.gain_assessment.request != expected
            || self.gain_assessment.subject.file != self.gain.file
            || !self.gain_assessment.evaluation.matches_selected_policy
            || self.gain.report.sample_rate_hz != 48_000
            || self.gain.report.channels != g.source.channels
            || self.gain.report.frame_count != g.source.frame_count
            || self.components.len() != 3
            || self
                .components
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
                != ["ffmpeg", "ffprobe", "provider"]
        {
            return Err(invalid(
                "gain video source, current candidate QC/policy or component selection differs",
            ));
        }
        let source_audio = self
            .artifact
            .audio
            .as_ref()
            .ok_or_else(|| invalid("gain video source audio missing"))?;
        let duration = source_audio
            .interval
            .duration()
            .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))?;
        let samples = duration
            .checked_mul(48_000)
            .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))?;
        if source_audio.sample_rate_hz != 48_000
            || source_audio.channels != u64::from(self.gain.report.channels)
            || source_audio.interval.start != g.source.audio_origin
            || samples.denominator() != 1
            || samples.numerator() < 1
            || samples.numerator() as u64 != self.gain.report.frame_count
        {
            return Err(invalid(
                "gain candidate must retain the full original sample count/channels and exact audio origin",
            ));
        }
        self.audio_offset_ticks()?;
        for (component, path) in self.components.iter().zip([
            &self.request.settings.ffmpeg.executable,
            &self.request.settings.ffprobe.executable,
            &self.request.provider_executable,
        ]) {
            crate::provider::require_sha256(&component.sha256, "gain video component")?;
            if &component.path != path
                || component.byte_count == 0
                || component.byte_count > 134_217_728
            {
                return Err(invalid("gain video component path/size invalid"));
            }
        }
        if self.components[0].sha256 != self.request.settings.ffmpeg.sha256
            || self.components[1].sha256 != self.request.settings.ffprobe.sha256
        {
            return Err(invalid("gain video tools differ from selected pins"));
        }
        if self.artifact.byte_count > self.request.settings.maximum_source_bytes
            || self.gain.file.byte_count > 268_435_456
        {
            return Err(invalid("gain video complete source exceeds bound"));
        }
        self.working_bytes()?;
        local::bytes(self)?;
        Ok(())
    }
    pub(crate) fn audio_offset_ticks(&self) -> Result<i64> {
        source_offset_ticks(self.gain.report.operation.source.audio_origin)
    }
    fn working_bytes(&self) -> Result<u64> {
        let tools = self.components.iter().try_fold(0u64, |sum, c| {
            sum.checked_add(c.byte_count)
                .ok_or_else(|| invalid("gain video component overflow"))
        })?;
        let bytes = self
            .artifact
            .byte_count
            .checked_mul(3)
            .and_then(|v| v.checked_add(self.gain.file.byte_count.checked_mul(3)?))
            .and_then(|v| v.checked_add(self.request.settings.maximum_output_bytes.checked_mul(3)?))
            .and_then(|v| v.checked_add(tools.checked_mul(4)?))
            .and_then(|v| v.checked_add(REPORT_BYTES * 4))
            .ok_or_else(|| invalid("gain video whole-work overflow"))?;
        if bytes > self.request.settings.maximum_working_bytes {
            return Err(invalid(
                "gain video snapshots/output/evidence exceed selected admission",
            ));
        }
        Ok(bytes)
    }
    fn reference() -> crate::ConfigurationSchemaReference {
        crate::ConfigurationSchemaReference {
            id: SCHEMA.into(),
            version: "1.0.0".into(),
            sha256: local::hash(include_bytes!(
                "../../../docs/contracts/gain-video-operation-v1.schema.json"
            )),
        }
    }
    fn configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value =
            serde_json::to_value(self).map_err(|_| invalid("gain video operation encoding"))?;
        crate::ProviderConfiguration::new(
            crate::ProviderReference {
                id: PROVIDER_ID.into(),
                version: "1.0.0".into(),
            },
            crate::CapabilityReference {
                id: CAPABILITY_ID.into(),
                version: "1.0.0".into(),
            },
            Self::reference(),
            value
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
    fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let port = |name: &str, kind: &str, role: &str, stream: &str| serde_json::json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value = serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow selected gain audio video copy"},"configuration_schemas":[Self::reference()],"capabilities":[{"id":CAPABILITY_ID,"version":"1.0.0","kind":"assembler_encoder","configuration_schema":Self::reference(),"inputs":[port("video","video/mp4","temporal_source","video"),port("audio","audio/wav","temporal_source","audio")],"outputs":[port("media","video/mp4","candidate_master","video"),port("evidence","application/vnd.aniflow.gain-video-report+json","validation_evidence","timed_metadata")],"batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":crate::video_assembly::component_inventory(&self.request.settings).tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":"=8.0.1","sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},"behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],"provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
}

fn current(request: &Request, token: &CancellationToken) -> Result<Operation> {
    request.validate()?;
    cancelled(token)?;
    let gain = gain::observe(&request.gain_run_directory, token)?;
    let (source, artifact) = match &gain.report.operation.source.assessment.subject.facts {
        MediaFacts::VideoWithQc {
            completion,
            artifact_id,
            ..
        } => {
            let a = completion
                .artifacts
                .iter()
                .find(|a| &a.artifact_id == artifact_id)
                .ok_or_else(|| invalid("gain source video artifact missing"))?;
            (completion.clone(), a.clone())
        }
        _ => {
            return Err(invalid(
                "initial gain-video profile requires a video-derived gain candidate",
            ));
        }
    };
    let check = policy_check::Request {
        schema: "aniflow.audio-policy-check-request/v1".into(),
        input: MediaInput::AcceptedAudioWithQc {
            subject: super::final_lossless_qc::FinalLosslessSubject::AcceptedGainCandidate {
                run_directory: request.gain_run_directory.clone(),
            },
            qc_directory: request.gain_qc_directory.clone(),
            qc_receipt_sha256: request.gain_qc_receipt_sha256.clone(),
        },
        policy: gain.report.operation.request.check.policy.clone(),
    };
    let gain_assessment = policy_check::assess(&check, token)?;
    let mut components = Vec::new();
    for (id, path, hash) in [
        (
            "ffmpeg",
            &request.settings.ffmpeg.executable,
            Some(&request.settings.ffmpeg.sha256),
        ),
        (
            "ffprobe",
            &request.settings.ffprobe.executable,
            Some(&request.settings.ffprobe.sha256),
        ),
        ("provider", &request.provider_executable, None),
    ] {
        let pin = identity(path, 134_217_728)?;
        if hash.is_some_and(|h| h != &pin.0) {
            return Err(invalid("gain video current component differs"));
        }
        components.push(gain::Component {
            id: id.into(),
            path: path.clone(),
            sha256: pin.0,
            byte_count: pin.1,
        });
    }
    let operation = Operation {
        schema: SCHEMA.into(),
        request: request.clone(),
        source,
        artifact,
        gain,
        gain_assessment,
        components,
    };
    operation.validate()?;
    cancelled(token)?;
    Ok(operation)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Media {
    pub source: crate::temporal::TemporalInspection,
    pub candidate: crate::temporal::TemporalInspection,
    pub video_packets_sha256: String,
    pub playback: crate::delivery_metadata::remux::MetadataRemuxPlayback,
    pub metadata: crate::delivery_metadata::MetadataPrivateReceipt,
    pub container: crate::delivery_metadata::mp4::Mp4MetadataObservation,
    pub full_decode_observed: bool,
    pub native_qualified: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub operation: Operation,
    pub provider_lock_sha256: String,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub media: Media,
    pub output_policy_status: String,
    pub creative_approval: String,
    pub publication_authority: String,
}

fn source_offset_ticks(origin: crate::temporal::RationalTime) -> Result<i64> {
    let ticks = origin
        .checked_mul(48_000)
        .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))?;
    if ticks.denominator() != 1 || ticks.numerator().unsigned_abs() > 4_147_200_000 {
        return Err(invalid(
            "gain audio origin must be an exact bounded original-rate sample tick",
        ));
    }
    Ok(ticks.numerator())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_audio_offset_must_be_exactly_representable_on_the_sample_grid() {
        use crate::temporal::RationalTime;
        assert_eq!(
            source_offset_ticks(RationalTime::new(1, 48_000).unwrap()).unwrap(),
            1
        );
        assert_eq!(
            source_offset_ticks(RationalTime::new(-1024, 48_000).unwrap()).unwrap(),
            -1024
        );
        assert!(source_offset_ticks(RationalTime::new(1, 96_000).unwrap()).is_err());
        assert!(source_offset_ticks(RationalTime::new(86_401, 1).unwrap()).is_err());
    }
}
