//! Explicit fixed-gain candidates. Native v3 owns execution/acceptance; current
//! final-file QC owns the input measurements. Fresh output QC is still required.
use super::policy_check;
use crate::audio_inspection::{AudioInspectionConfiguration, NativeSampleFormat};
use crate::audio_signal::AudioSignalValue;
use crate::destination::{MediaFacts, MediaFile, MediaInput, cancelled, invalid, local};
use crate::frame_io::identity;
use crate::provider::canonical_json_bytes;
use crate::{CancellationToken, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod native;
mod runtime;
pub(crate) use native::execute_invocation;
pub use runtime::{observe, plan, resume, run};

pub const PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-gain";
pub const CAPABILITY_ID: &str = "aniflow/fixed-audio-gain";
const SCHEMA: &str = "aniflow.audio-gain-operation/v1";
const MEDIA_BYTES: u64 = 268_435_456;
const TOOL_BYTES: u64 = 134_217_728;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema: String,
    pub tools: AudioInspectionConfiguration,
    pub maximum_absolute_gain_milli_db: u16,
    pub peak_guard_milli_db: u16,
    pub maximum_output_bytes: u64,
    pub maximum_working_bytes: u64,
    pub minimum_free_bytes: u64,
}
impl Configuration {
    pub fn validate(&self) -> Result<()> {
        self.tools.validate()?;
        if self.schema != "aniflow.audio-gain-configuration/v1"
            || self.tools.ffmpeg.version != "8.0.1"
            || self.tools.ffprobe.version != "8.0.1"
            || self.tools.ffmpeg.executable == self.tools.ffprobe.executable
            || !(1..=24_000).contains(&self.maximum_absolute_gain_milli_db)
            || self.peak_guard_milli_db > 3000
            || !(1024..=MEDIA_BYTES).contains(&self.maximum_output_bytes)
            || !(8_388_608..=1_099_511_627_776).contains(&self.maximum_working_bytes)
            || self.minimum_free_bytes > 1_099_511_627_776
        {
            return Err(invalid(
                "fixed gain requires explicit bounded gain/headroom, exact tools and finite output/storage settings",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub check: policy_check::Request,
    pub configuration: Configuration,
    pub provider_executable: PathBuf,
}
impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("gain request exceeds 64 KiB"));
        }
        let r: Self = crate::provider::decode_json(bytes, "fixed audio gain request")?;
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> Result<()> {
        self.check.validate()?;
        self.configuration.validate()?;
        if self.schema != "aniflow.audio-gain-request/v1"
            || self.provider_executable.as_os_str().is_empty()
        {
            return Err(invalid(
                "gain requires an explicit current-QC request and native provider",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionState {
    Ready,
    AlreadyWithinPolicy,
    MissingEvidence,
    GainExceedsBound,
    PeakCeilingWouldBeExceeded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub state: DecisionState,
    pub gain_milli_db: Option<i32>,
    pub predicted_true_peak_dbtp: Option<f64>,
}

/// Predict a bounded constant gain, never a measured output or implicit limiter.
pub fn decide(check: &policy_check::Evaluation, configuration: &Configuration) -> Result<Decision> {
    configuration.validate()?;
    check.policy.validate()?;
    // Recompute the states; a supplied `matches` flag has no authority.
    let channels = u16::try_from(check.integrated_loudness.observed.scope.channels.len())
        .map_err(|_| invalid("gain scope overflow"))?;
    let evaluation = policy_check::compare(
        &check.policy,
        channels,
        &check.integrated_loudness.observed,
        &check.true_peak.observed,
    )?;
    if &evaluation != check {
        return Err(invalid(
            "gain evaluation differs from its exact numeric observations/policy",
        ));
    }
    if check.matches_selected_policy {
        return Ok(Decision {
            state: DecisionState::AlreadyWithinPolicy,
            gain_milli_db: None,
            predicted_true_peak_dbtp: None,
        });
    }
    let (
        AudioSignalValue::Measured { value: loudness },
        AudioSignalValue::Measured { value: peak },
    ) = (
        &check.integrated_loudness.observed.value,
        &check.true_peak.observed.value,
    )
    else {
        return Ok(Decision {
            state: DecisionState::MissingEvidence,
            gain_milli_db: None,
            predicted_true_peak_dbtp: None,
        });
    };
    let gain = (f64::from(check.policy.target_milli_lufs) - loudness * 1000.0).round();
    if !gain.is_finite() || gain.abs() > f64::from(configuration.maximum_absolute_gain_milli_db) {
        return Ok(Decision {
            state: DecisionState::GainExceedsBound,
            gain_milli_db: None,
            predicted_true_peak_dbtp: None,
        });
    }
    let gain = gain as i32;
    let predicted = peak + f64::from(gain) / 1000.0;
    let ceiling = f64::from(check.policy.maximum_true_peak_milli_dbtp) / 1000.0
        - f64::from(configuration.peak_guard_milli_db) / 1000.0;
    let state = if predicted <= ceiling {
        DecisionState::Ready
    } else {
        DecisionState::PeakCeilingWouldBeExceeded
    };
    Ok(Decision {
        state,
        gain_milli_db: Some(gain),
        predicted_true_peak_dbtp: Some(predicted),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub assessment: policy_check::Assessment,
    pub pcm: MediaFile,
    pub pcm_sha256: String,
    pub encoding: NativeSampleFormat,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub audio_origin: crate::temporal::RationalTime,
}

fn source(request: &Request, token: &CancellationToken) -> Result<Source> {
    let assessment = policy_check::assess(&request.check, token)?;
    let (root, wav_name) = match &request.check.input {
        MediaInput::AcceptedVideoWithQc { qc_directory, .. } => {
            (qc_directory, "signal/selected-audio.wav")
        }
        MediaInput::AcceptedAudioWithQc { qc_directory, .. } => (qc_directory, "decoded.wav"),
        _ => {
            return Err(invalid(
                "gain requires a retained exact-final-file QC subject",
            ));
        }
    };
    let (pin, pcm_hash, rate, channels, frames, origin) = match &assessment.subject.facts {
        MediaFacts::VideoWithQc { qc, .. } => (
            &qc.signal.decoded_wav_sha256,
            &qc.signal.decoded_pcm_sha256,
            qc.signal.sample_rate_hz,
            qc.signal.channels,
            qc.signal.frames,
            qc.signal.source_audio_extent.start,
        ),
        MediaFacts::AudioWithQc { qc } => (
            &qc.decoded_wav_sha256,
            &qc.decoded_pcm_sha256,
            qc.accepted_pcm.source.sample_rate_hz,
            qc.accepted_pcm.source.channels,
            qc.accepted_pcm.source.frame_count,
            crate::temporal::RationalTime::ZERO,
        ),
        _ => return Err(invalid("gain source lacks current retained complete PCM")),
    };
    let path = root.join(wav_name);
    let observed = identity(&path, MEDIA_BYTES)?;
    let wave = crate::audio_inspection::wav::inspect_native(&path)?;
    if observed.0 != *pin
        || wave.pcm_sha256 != *pcm_hash
        || wave.sample_rate_hz != rate
        || wave.channels != channels
        || wave.frame_count != frames
        || !matches!(rate, 44_100 | 48_000)
        || !matches!(channels, 1 | 2)
        || frames == 0
        || frames > u64::from(rate) * 600
    {
        return Err(invalid(
            "gain source differs from exact complete native QC samples/clock or supported ten-minute profile",
        ));
    }
    let result = Source {
        assessment: assessment.clone(),
        pcm: MediaFile {
            locator: path,
            sha256: observed.0,
            byte_count: observed.1,
        },
        pcm_sha256: pcm_hash.clone(),
        encoding: wave.sample_format,
        sample_rate_hz: rate,
        channels,
        frame_count: frames,
        audio_origin: origin,
    };
    if policy_check::recheck(&assessment, token)? != assessment {
        return Err(invalid("gain source changed during preparation"));
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}
fn components(request: &Request) -> Result<Vec<Component>> {
    let mut values = Vec::new();
    for (id, pin) in [
        ("ffmpeg", &request.configuration.tools.ffmpeg),
        ("ffprobe", &request.configuration.tools.ffprobe),
    ] {
        crate::audio_inspection::process::verify_pin(pin, id)
            .map_err(|_| invalid("selected gain tool differs from its pin"))?;
        let observed = identity(&pin.executable, TOOL_BYTES)?;
        values.push(Component {
            id: id.into(),
            path: pin.executable.clone(),
            sha256: observed.0,
            byte_count: observed.1,
        });
    }
    let observed = identity(&request.provider_executable, TOOL_BYTES)?;
    values.push(Component {
        id: "provider".into(),
        path: request.provider_executable.clone(),
        sha256: observed.0,
        byte_count: observed.1,
    });
    Ok(values)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub schema: String,
    pub request: Request,
    pub source: Source,
    pub components: Vec<Component>,
    pub gain_milli_db: i32,
}
impl Operation {
    fn validate(&self) -> Result<()> {
        self.request.validate()?;
        policy_check::validate_snapshot(&self.source.assessment)?;
        let decision = decide(
            &self.source.assessment.evaluation,
            &self.request.configuration,
        )?;
        if self.schema != SCHEMA
            || self.source.assessment.request != self.request.check
            || decision.state != DecisionState::Ready
            || decision.gain_milli_db != Some(self.gain_milli_db)
            || !matches!(self.source.sample_rate_hz, 44_100 | 48_000)
            || !matches!(self.source.channels, 1 | 2)
            || self.source.frame_count == 0
            || self.source.frame_count > u64::from(self.source.sample_rate_hz) * 600
            || self.components.len() != 3
            || self
                .components
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
                != ["ffmpeg", "ffprobe", "provider"]
        {
            return Err(invalid(
                "gain operation differs from its explicit current policy/source/component selection",
            ));
        }
        for (component, path) in self.components.iter().zip([
            &self.request.configuration.tools.ffmpeg.executable,
            &self.request.configuration.tools.ffprobe.executable,
            &self.request.provider_executable,
        ]) {
            crate::provider::require_sha256(&component.sha256, "gain component")?;
            if &component.path != path
                || component.byte_count == 0
                || component.byte_count > TOOL_BYTES
            {
                return Err(invalid("gain component binding invalid"));
            }
        }
        if self.components[0].sha256 != self.request.configuration.tools.ffmpeg.sha256
            || self.components[1].sha256 != self.request.configuration.tools.ffprobe.sha256
        {
            return Err(invalid("gain tool content differs from requested pins"));
        }
        crate::provider::require_sha256(&self.source.pcm.sha256, "gain source WAV")?;
        crate::provider::require_sha256(&self.source.pcm_sha256, "gain source PCM")?;
        let expected = match (
            &self.request.check.input,
            &self.source.assessment.subject.facts,
        ) {
            (
                MediaInput::AcceptedVideoWithQc { qc_directory, .. },
                MediaFacts::VideoWithQc { qc, .. },
            ) => (
                qc_directory.join("signal/selected-audio.wav"),
                qc.signal.decoded_wav_sha256.as_str(),
                qc.signal.decoded_wav_bytes,
                qc.signal.decoded_pcm_sha256.as_str(),
                qc.signal.sample_rate_hz,
                qc.signal.channels,
                qc.signal.frames,
                qc.signal.source_audio_extent.start,
            ),
            (
                MediaInput::AcceptedAudioWithQc { qc_directory, .. },
                MediaFacts::AudioWithQc { qc },
            ) => (
                qc_directory.join("decoded.wav"),
                qc.decoded_wav_sha256.as_str(),
                qc.decoded_wav_bytes,
                qc.decoded_pcm_sha256.as_str(),
                qc.accepted_pcm.source.sample_rate_hz,
                qc.accepted_pcm.source.channels,
                qc.accepted_pcm.source.frame_count,
                crate::temporal::RationalTime::ZERO,
            ),
            _ => return Err(invalid("gain source does not match selected QC owner")),
        };
        if self.source.pcm.locator != expected.0
            || self.source.pcm.sha256 != expected.1
            || self.source.pcm.byte_count != expected.2
            || self.source.pcm_sha256 != expected.3
            || self.source.sample_rate_hz != expected.4
            || self.source.channels != expected.5
            || self.source.frame_count != expected.6
            || self.source.audio_origin != expected.7
        {
            return Err(invalid(
                "gain source fields differ from retained exact QC samples/clock/origin",
            ));
        }
        self.output_bytes()?;
        self.working_bytes()?;
        local::bytes(self)?;
        Ok(())
    }
    fn output_bytes(&self) -> Result<u64> {
        let bytes = self
            .source
            .frame_count
            .checked_mul(u64::from(self.source.channels) * 4)
            .and_then(|v| v.checked_add(64))
            .ok_or_else(|| invalid("gain output size overflow"))?;
        if bytes > self.request.configuration.maximum_output_bytes {
            return Err(invalid(
                "complete float gain candidate exceeds selected output bound",
            ));
        }
        Ok(bytes)
    }
    fn working_bytes(&self) -> Result<u64> {
        let tools = self.components.iter().try_fold(0u64, |sum, c| {
            sum.checked_add(c.byte_count)
                .ok_or_else(|| invalid("gain component size overflow"))
        })?;
        let bytes = self
            .source
            .pcm
            .byte_count
            .checked_mul(3)
            .and_then(|v| v.checked_add(self.output_bytes().ok()?.checked_mul(4)?))
            .and_then(|v| v.checked_add(tools.checked_mul(4)?))
            .and_then(|v| v.checked_add(8_388_608))
            .ok_or_else(|| invalid("gain whole-work admission overflow"))?;
        if bytes > self.request.configuration.maximum_working_bytes {
            return Err(invalid(
                "gain snapshots/outputs/evidence exceed selected whole-work admission",
            ));
        }
        Ok(bytes)
    }
    fn schema_reference() -> crate::ConfigurationSchemaReference {
        crate::ConfigurationSchemaReference {
            id: SCHEMA.into(),
            version: "1.0.0".into(),
            sha256: local::hash(include_bytes!(
                "../../../docs/contracts/audio-gain-operation-v1.schema.json"
            )),
        }
    }
    fn configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|_| invalid("gain operation encoding"))?;
        crate::ProviderConfiguration::new(
            crate::ProviderReference {
                id: PROVIDER_ID.into(),
                version: "1.0.0".into(),
            },
            crate::CapabilityReference {
                id: CAPABILITY_ID.into(),
                version: "1.0.0".into(),
            },
            Self::schema_reference(),
            value
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
    fn inventory(&self) -> crate::ComponentInventory {
        crate::ComponentInventory {
            tools: self.components[..2]
                .iter()
                .map(|c| crate::ComponentIdentity {
                    id: c.id.clone(),
                    version: "8.0.1".into(),
                    sha256: Some(c.sha256.clone()),
                })
                .collect(),
            codecs: vec![],
            models: vec![],
        }
    }
    fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let port = |name: &str, kind: &str, role: &str, stream: &str| serde_json::json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value = serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow explicit fixed audio gain"},"configuration_schemas":[Self::schema_reference()],"capabilities":[{"id":CAPABILITY_ID,"version":"1.0.0","kind":"assembler_encoder","configuration_schema":Self::schema_reference(),"inputs":[port("source","audio/wav","temporal_source","audio")],"outputs":[port("media","audio/wav","candidate_master","audio"),port("evidence","application/vnd.aniflow.audio-gain-report+json","validation_evidence","timed_metadata")],"batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":self.inventory().tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":"=8.0.1","sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},"behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],"provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub request: Request,
    pub source: Source,
    pub decision: Decision,
    pub native_plan: Option<crate::PipelineV3Plan>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub operation: Operation,
    pub provider_lock_sha256: String,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub output_pcm_sha256: String,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub output_policy_status: String,
    pub runtime_qualified: bool,
    pub creative_approval: String,
    pub publication_authority: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub schema: String,
    pub file: MediaFile,
    pub report: Report,
}

impl Completion {
    /// Shape/byte identity of the current candidate, with no invented meters.
    pub(crate) fn pcm_source(&self) -> Result<crate::audio_analysis::AudioSource> {
        let mut source = match &self.report.operation.source.assessment.subject.facts {
            MediaFacts::VideoWithQc { qc, .. } => qc.accepted_pcm.source.clone(),
            MediaFacts::AudioWithQc { qc } => qc.accepted_pcm.source.clone(),
            _ => return Err(invalid("gain completion lacks its source PCM descriptor")),
        };
        source.artifact = crate::audio_analysis::AudioArtifactReference {
            id: "gain_audio".into(),
            sha256: self.file.sha256.clone(),
            byte_size: self.file.byte_count,
        };
        source.stream_index = 0;
        source.sample_rate_hz = self.report.sample_rate_hz;
        source.channels = self.report.channels;
        source.frame_count = self.report.frame_count;
        source.origin = crate::audio_analysis::AudioRationalTime {
            numerator: 0,
            denominator: 1,
        };
        source.stem = None;
        source.validate()?;
        Ok(source)
    }
}

#[cfg(test)]
mod tests;
