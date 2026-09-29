//! Closed evidence contracts for the pinned, English observed-transcript profile.
//! Raw deserialization is unvalidated; use the constructors or `validate`.
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio_analysis::{
    AudioArtifactReference, AudioConfidence, AudioProvenanceClass, AudioRationalTime, AudioScope,
    AudioSource,
};
use crate::audio_inspection::{
    AudioInspectionConfiguration, AudioTechnicalCommandEvidence, AudioToolPin,
};
use crate::timed_text::TimedTextDocument;
use crate::{Error, ErrorCategory, ProviderReference, Result};

pub const AUDIO_TRANSCRIPTION_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-transcription.configuration/v1";
pub const AUDIO_TRANSCRIPTION_PROVIDER_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-transcription.provider-configuration/v1";
pub const AUDIO_TRANSCRIPTION_REPORT_SCHEMA_V1: &str = "aniflow.audio-transcription/v1";
pub const AUDIO_TRANSCRIPTION_PREFLIGHT_SCHEMA_V1: &str =
    "aniflow.audio-transcription-preflight/v1";
pub const AUDIO_TRANSCRIPTION_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-transcription";
pub const AUDIO_TRANSCRIPTION_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_TRANSCRIPTION_CAPABILITY_ID: &str = "aniflow/audio-transcription";
pub const AUDIO_TRANSCRIPTION_WHISPER_VERSION: &str = "1.8.7";
pub const AUDIO_TRANSCRIPTION_WHISPER_REVISION: &str = "48f628a84833905ee4a0658ee6d4a5c915ce1997";
pub const AUDIO_TRANSCRIPTION_CONFIDENCE_REASON: &str =
    "Basic whisper.cpp JSON does not report calibrated segment confidence.";
pub const AUDIO_TRANSCRIPTION_WORD_TIMING_REASON: &str =
    "Basic whisper.cpp JSON reports segment timing, not word timing.";
pub const AUDIO_TRANSCRIPTION_MAXIMUM_MODEL_BYTES: u64 = 256 * 1024 * 1024;
pub const AUDIO_TRANSCRIPTION_MAXIMUM_RAW_BYTES: usize = 1024 * 1024;
pub const AUDIO_TRANSCRIPTION_MAXIMUM_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const AUDIO_TRANSCRIPTION_TOOL_LICENSE_URL: &str =
    "https://github.com/ggml-org/whisper.cpp/blob/48f628a84833905ee4a0658ee6d4a5c915ce1997/LICENSE";
pub const AUDIO_TRANSCRIPTION_MODEL_LICENSE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/blob/5359861c739e955e79d9a303bcbc70fb988958b1/README.md";
pub const AUDIO_TRANSCRIPTION_PRODUCER: &str = "org.egohygiene.aniflow.audio-transcription@1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionModelPin {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_size: u64,
    pub model_id: String,
    pub revision: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionConfiguration {
    pub schema: String,
    pub whisper: AudioToolPin,
    pub model: TranscriptionModelPin,
    pub language: String,
    pub threads: u16,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTranscriptionProviderConfiguration {
    pub schema: String,
    pub settings: TranscriptionConfiguration,
    pub tools: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
    pub upstream_analysis_artifact_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTranscriptionDiagnosticCode {
    UnsupportedLanguage,
    MissingTool,
    InvalidTool,
    ToolDigestMismatch,
    ToolVersionMismatch,
    MissingModel,
    InvalidModel,
    ModelDigestMismatch,
    ModelSizeMismatch,
    ToolTimeout,
    ToolOutputLimit,
    ToolFailed,
    Cancelled,
    UnsupportedPlatform,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTranscriptionDiagnostic {
    pub code: AudioTranscriptionDiagnosticCode,
    pub component: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTranscriptionPreflight {
    pub schema: String,
    pub ready: bool,
    pub diagnostics: Vec<AudioTranscriptionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionModelEvidence {
    pub sha256: String,
    pub byte_size: u64,
    pub model_id: String,
    pub revision: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionSettingsEvidence {
    pub whisper_version: String,
    pub whisper_sha256: String,
    pub model: TranscriptionModelEvidence,
    pub language: String,
    pub threads: u16,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionLicenseEvidence {
    pub tool_expression: String,
    pub tool_source_url: String,
    pub model_expression: String,
    pub model_source_url: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhisperNetworkDimensions {
    pub ctx: u32,
    pub state: u32,
    pub head: u32,
    pub layer: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhisperModelParameters {
    #[serde(rename = "type")]
    pub model_type: String,
    pub multilingual: bool,
    pub vocab: u32,
    pub audio: WhisperNetworkDimensions,
    pub text: WhisperNetworkDimensions,
    pub mels: u32,
    pub ftype: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionSegment {
    pub id: String,
    pub start: AudioRationalTime,
    pub end: AudioRationalTime,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionObservation {
    pub detected_language: String,
    pub model: WhisperModelParameters,
    pub segments: Vec<TranscriptionSegment>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionUnavailableReason {
    UnsupportedSampleRate,
    UnsupportedChannels,
    SilentInput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum TranscriptionResult {
    Observed {
        observation: TranscriptionObservation,
    },
    Empty {
        observation: TranscriptionObservation,
    },
    Unavailable {
        reason: TranscriptionUnavailableReason,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum TranscriptionWordTiming {
    Unavailable { reason: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionMethod {
    pub sample_rate_hz: u32,
    pub downmix: String,
    pub timestamp_grid_milliseconds: u16,
    pub translation: bool,
    pub vad: bool,
    pub word_timestamps: bool,
    pub gpu: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTranscriptionReport {
    pub schema: String,
    pub source: AudioSource,
    pub scope: AudioScope,
    pub technical_artifact: AudioArtifactReference,
    pub upstream_analysis_artifact: AudioArtifactReference,
    /// Identity of captured analyzer bytes; those bytes are not a persisted output.
    pub raw_observation: Option<AudioArtifactReference>,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub settings: TranscriptionSettingsEvidence,
    pub licenses: TranscriptionLicenseEvidence,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
    pub method: TranscriptionMethod,
    pub provenance: AudioProvenanceClass,
    pub confidence: AudioConfidence,
    pub word_timing: TranscriptionWordTiming,
    pub result: TranscriptionResult,
    pub timed_text: Option<TimedTextDocument>,
}

impl TranscriptionConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "transcription configuration")?;
        let value: Self = crate::provider::decode_json(bytes, "transcription configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_TRANSCRIPTION_CONFIGURATION_SCHEMA_V1 {
            return Err(invalid("unsupported transcription configuration schema"));
        }
        normalized_path(&self.whisper.executable)?;
        normalized_path(&self.model.path)?;
        if self.whisper.version != AUDIO_TRANSCRIPTION_WHISPER_VERSION {
            return Err(invalid(
                "transcription requires exact whisper.cpp version 1.8.7",
            ));
        }
        crate::provider::require_sha256(&self.whisper.sha256, "whisper executable sha256")?;
        validate_model(&TranscriptionModelEvidence::from(&self.model))?;
        text(&self.language, 16)?;
        if !self
            .language
            .bytes()
            .all(|value| value.is_ascii_lowercase() || value == b'-')
        {
            return Err(invalid(
                "language must be a bounded lowercase language token",
            ));
        }
        bounds(
            self.threads,
            self.tool_timeout_milliseconds,
            self.maximum_tool_output_bytes,
        )
    }
}
impl AudioTranscriptionProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        self.tools.validate()?;
        if self.schema != AUDIO_TRANSCRIPTION_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.source.id != "source_audio"
            || !matches!(
                self.upstream_analysis_artifact_id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid(
                "unsupported transcription provider configuration or source binding",
            ));
        }
        reference(&self.source, 256 * 1024 * 1024)
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let serialized = serde_json::to_value(self)
            .map_err(|_| invalid("cannot encode transcription settings"))?;
        let values = serialized
            .as_object()
            .expect("configuration struct")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_TRANSCRIPTION_PROVIDER_ID.into(),
                version: AUDIO_TRANSCRIPTION_PROVIDER_VERSION.into(),
            },
            crate::CapabilityReference {
                id: AUDIO_TRANSCRIPTION_CAPABILITY_ID.into(),
                version: "1.0.0".into(),
            },
            configuration_schema_reference(),
            values,
        )
    }
}
#[must_use]
pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    use sha2::{Digest, Sha256};
    crate::ConfigurationSchemaReference {
        id: AUDIO_TRANSCRIPTION_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
        version: "1.0.0".into(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-transcription/provider-configuration.schema.json"
            ))
        ),
    }
}
impl From<&TranscriptionModelPin> for TranscriptionModelEvidence {
    fn from(value: &TranscriptionModelPin) -> Self {
        Self {
            sha256: value.sha256.clone(),
            byte_size: value.byte_size,
            model_id: value.model_id.clone(),
            revision: value.revision.clone(),
        }
    }
}
impl TranscriptionSettingsEvidence {
    #[must_use]
    pub fn from_configuration(value: &TranscriptionConfiguration) -> Self {
        Self {
            whisper_version: value.whisper.version.clone(),
            whisper_sha256: value.whisper.sha256.clone(),
            model: TranscriptionModelEvidence::from(&value.model),
            language: value.language.clone(),
            threads: value.threads,
            tool_timeout_milliseconds: value.tool_timeout_milliseconds,
            maximum_tool_output_bytes: value.maximum_tool_output_bytes,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.whisper_version != AUDIO_TRANSCRIPTION_WHISPER_VERSION || self.language != "en" {
            return Err(invalid(
                "transcription report requires the pinned English-only engine profile",
            ));
        }
        crate::provider::require_sha256(&self.whisper_sha256, "transcription engine sha256")?;
        validate_model(&self.model)?;
        bounds(
            self.threads,
            self.tool_timeout_milliseconds,
            self.maximum_tool_output_bytes,
        )
    }
}
impl Default for TranscriptionMethod {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16000,
            downmix: "none".into(),
            timestamp_grid_milliseconds: 10,
            translation: false,
            vad: false,
            word_timestamps: false,
            gpu: false,
        }
    }
}
impl Default for TranscriptionLicenseEvidence {
    fn default() -> Self {
        Self {
            tool_expression: "MIT".into(),
            tool_source_url: AUDIO_TRANSCRIPTION_TOOL_LICENSE_URL.into(),
            model_expression: "MIT".into(),
            model_source_url: AUDIO_TRANSCRIPTION_MODEL_LICENSE_URL.into(),
        }
    }
}
impl AudioTranscriptionPreflight {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
            && self.diagnostics.is_empty()
            && self.schema == AUDIO_TRANSCRIPTION_PREFLIGHT_SCHEMA_V1
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_TRANSCRIPTION_PREFLIGHT_SCHEMA_V1
            || self.ready != self.diagnostics.is_empty()
            || self.diagnostics.len() > 32
        {
            return Err(invalid("inconsistent transcription preflight outcome"));
        }
        for diagnostic in &self.diagnostics {
            text(&diagnostic.component, 256)?;
            text(&diagnostic.message, 4096)?;
        }
        Ok(())
    }
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "transcription preflight")?;
        let value: Self = crate::provider::decode_json(bytes, "transcription preflight")?;
        value.validate()?;
        Ok(value)
    }
}
impl WhisperModelParameters {
    pub fn validate(&self) -> Result<()> {
        if self.model_type != "tiny"
            || self.multilingual
            || self.vocab != 51864
            || self.audio
                != (WhisperNetworkDimensions {
                    ctx: 1500,
                    state: 384,
                    head: 6,
                    layer: 4,
                })
            || self.text
                != (WhisperNetworkDimensions {
                    ctx: 448,
                    state: 384,
                    head: 6,
                    layer: 4,
                })
            || self.mels != 80
            || self.ftype != 1
        {
            return Err(invalid(
                "analyzer model metadata does not match the admitted tiny.en f16 profile",
            ));
        }
        Ok(())
    }
}
impl TranscriptionObservation {
    pub fn validate(&self, source: &AudioSource) -> Result<()> {
        source.validate()?;
        self.model.validate()?;
        if self.detected_language != "en"
            || source.sample_rate_hz != 16000
            || source.channels != 1
            || self.segments.len() > 10000
        {
            return Err(invalid(
                "observation is outside the mono 16 kHz English transcript profile",
            ));
        }
        let mut previous_end = 0u64;
        let mut total_text = 0usize;
        for (index, segment) in self.segments.iter().enumerate() {
            let start = milliseconds(segment.start)?;
            let end = milliseconds(segment.end)?;
            if segment.id != format!("transcription_segment_{:06}", index + 1)
                || start >= end
                || start < previous_end
                || u128::from(end) * u128::from(source.sample_rate_hz)
                    > u128::from(source.frame_count) * 1000
            {
                return Err(invalid(
                    "transcription segments require stable ordered identities and nonempty in-range timing",
                ));
            }
            if segment.text.is_empty()
                || segment.text.len() > 65536
                || segment
                    .text
                    .chars()
                    .any(|value| value.is_control() && !matches!(value, '\n' | '\r' | '\t'))
            {
                return Err(invalid(
                    "transcript segment text exceeds its bound or contains unsupported controls",
                ));
            }
            total_text = total_text
                .checked_add(segment.text.len())
                .ok_or_else(|| invalid("transcript text length overflow"))?;
            if total_text > AUDIO_TRANSCRIPTION_MAXIMUM_RAW_BYTES {
                return Err(invalid("transcript text exceeds 1 MiB"));
            }
            previous_end = end;
        }
        Ok(())
    }
}
impl AudioTranscriptionReport {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(
            bytes,
            AUDIO_TRANSCRIPTION_MAXIMUM_REPORT_BYTES,
            "transcription report",
        )?;
        let value: Self = crate::provider::decode_json(bytes, "transcription report")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        bounded(
            &bytes,
            AUDIO_TRANSCRIPTION_MAXIMUM_REPORT_BYTES,
            "transcription report",
        )?;
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.settings.validate()?;
        let expected_scope = AudioScope {
            channels: (0..self.source.channels).collect(),
            stem_id: self.source.stem.as_ref().map(|stem| stem.id.clone()),
        };
        if self.schema != AUDIO_TRANSCRIPTION_REPORT_SCHEMA_V1
            || self.provider
                != (ProviderReference {
                    id: AUDIO_TRANSCRIPTION_PROVIDER_ID.into(),
                    version: AUDIO_TRANSCRIPTION_PROVIDER_VERSION.into(),
                })
            || self.source.artifact.id != "source_audio"
            || self.scope != expected_scope
            || self.method != TranscriptionMethod::default()
            || self.licenses != TranscriptionLicenseEvidence::default()
            || self.provenance != AudioProvenanceClass::Probabilistic
            || self.confidence
                != (AudioConfidence::Unavailable {
                    reason: AUDIO_TRANSCRIPTION_CONFIDENCE_REASON.into(),
                })
            || self.word_timing
                != (TranscriptionWordTiming::Unavailable {
                    reason: AUDIO_TRANSCRIPTION_WORD_TIMING_REASON.into(),
                })
        {
            return Err(invalid(
                "unsupported transcription report identity, method, scope, license declarations, or uncertainty",
            ));
        }
        if !(8000..=192000).contains(&self.source.sample_rate_hz)
            || !(1..=2).contains(&self.source.channels)
            || self.source.frame_count > u64::from(self.source.sample_rate_hz) * 600
            || self.source.artifact.byte_size > 256 * 1024 * 1024
            || self.source.artifact.byte_size
                < 44 + self.source.frame_count * u64::from(self.source.channels) * 2
        {
            return Err(invalid(
                "transcription source is outside the inspected PCM16 profile",
            ));
        }
        for hash in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
        ] {
            crate::provider::require_sha256(hash, "transcription provenance digest")?;
        }
        reference(&self.technical_artifact, 8 * 1024 * 1024)?;
        reference(&self.upstream_analysis_artifact, 8 * 1024 * 1024)?;
        if self.technical_artifact.id != "technical"
            || !matches!(
                self.upstream_analysis_artifact.id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid(
                "transcription upstream artifact identities differ from the pipeline",
            ));
        }
        let observation = match &self.result {
            TranscriptionResult::Observed { observation } => {
                if observation.segments.is_empty() {
                    return Err(invalid("observed transcript requires nonempty segments"));
                }
                Some(observation)
            }
            TranscriptionResult::Empty { observation } => {
                if !observation.segments.is_empty() {
                    return Err(invalid("empty transcript result cannot contain segments"));
                }
                Some(observation)
            }
            TranscriptionResult::Unavailable { reason } => {
                let valid = match reason {
                    TranscriptionUnavailableReason::UnsupportedSampleRate => {
                        self.source.sample_rate_hz != 16000
                    }
                    TranscriptionUnavailableReason::UnsupportedChannels => {
                        self.source.sample_rate_hz == 16000 && self.source.channels != 1
                    }
                    TranscriptionUnavailableReason::SilentInput => {
                        self.source.sample_rate_hz == 16000 && self.source.channels == 1
                    }
                };
                if !valid || self.raw_observation.is_some() || self.timed_text.is_some() {
                    return Err(invalid(
                        "unavailable transcript has an inconsistent reason or fabricated observation",
                    ));
                }
                None
            }
        };
        if self.commands
            != super::provider::command_evidence(self.settings.threads, observation.is_some())
        {
            return Err(invalid(
                "transcription command evidence differs from the pinned invocation",
            ));
        }
        if let Some(observation) = observation {
            observation.validate(&self.source)?;
            let raw = self.raw_observation.as_ref().ok_or_else(|| {
                invalid("captured transcript bytes require an observation identity")
            })?;
            reference(raw, AUDIO_TRANSCRIPTION_MAXIMUM_RAW_BYTES as u64)?;
            if raw.id != "transcription_observation" {
                return Err(invalid("unexpected raw transcript observation identity"));
            }
            let expected = super::normalize::timed_text(observation, &self.source, raw)?;
            if self.timed_text != expected {
                return Err(invalid(
                    "timed-text export must exactly preserve observed transcript segments and provenance",
                ));
            }
        }
        Ok(())
    }
}

fn bounds(threads: u16, timeout: u64, output: u64) -> Result<()> {
    if !(1..=16).contains(&threads)
        || !(1..=120000).contains(&timeout)
        || !(1024..=1048576).contains(&output)
    {
        return Err(invalid(
            "transcription execution bounds are outside the supported profile",
        ));
    }
    Ok(())
}
fn validate_model(model: &TranscriptionModelEvidence) -> Result<()> {
    crate::provider::require_sha256(&model.sha256, "transcription model sha256")?;
    text(&model.revision, 256)?;
    if model.model_id != "tiny.en"
        || model.byte_size == 0
        || model.byte_size > AUDIO_TRANSCRIPTION_MAXIMUM_MODEL_BYTES
    {
        return Err(invalid(
            "transcription model must be a bounded pinned tiny.en artifact",
        ));
    }
    Ok(())
}
fn normalized_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        || path.to_str().is_none_or(|value| {
            value.len() > 4096
                || value.chars().any(char::is_control)
                || value.split('/').any(|part| matches!(part, "." | ".."))
        })
    {
        return Err(invalid(
            "transcription asset paths must be absolute normalized UTF-8 paths",
        ));
    }
    Ok(())
}
pub(super) fn reference(value: &AudioArtifactReference, maximum: u64) -> Result<()> {
    text(&value.id, 256)?;
    crate::provider::require_sha256(&value.sha256, "transcription artifact sha256")?;
    if value.byte_size == 0 || value.byte_size > maximum {
        return Err(invalid("transcription artifact size exceeds its bound"));
    }
    Ok(())
}
pub(super) fn bounded(bytes: &[u8], maximum: usize, label: &str) -> Result<()> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!(
            "{label} is empty or exceeds its byte bound"
        )));
    }
    Ok(())
}
pub(super) fn text(value: &str, maximum: usize) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(invalid(
            "transcription metadata must be bounded nonempty printable text",
        ));
    }
    Ok(())
}
pub(super) fn milliseconds(time: AudioRationalTime) -> Result<u64> {
    time.validate()?;
    if time.numerator < 0 {
        return Err(invalid("transcript timestamps cannot be negative"));
    }
    let scaled = u128::from(time.numerator as u64) * 1000;
    if scaled % u128::from(time.denominator) != 0 {
        return Err(invalid("transcript timestamps must be exact milliseconds"));
    }
    let ms = scaled / u128::from(time.denominator);
    if ms > 600000 || ms % 10 != 0 {
        return Err(invalid(
            "transcript timing requires the bounded 10 ms native grid",
        ));
    }
    Ok(ms as u64)
}
pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_analysis::AudioReviewedAuthority;
    use crate::timed_text::TextProvenance;

    #[test]
    fn published_observation_validates_and_cannot_promote_lyrics_or_confidence() {
        let report = AudioTranscriptionReport::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-transcription-v1.example.json"
        ))
        .unwrap();
        assert!(matches!(
            report.result,
            TranscriptionResult::Observed { .. }
        ));
        let mut elevated = report.clone();
        let document = elevated.timed_text.as_mut().unwrap();
        document.provenance = TextProvenance::ReviewedLyrics {
            authority: AudioReviewedAuthority {
                supplied_by: "synthetic reviewer".into(),
                provenance_artifact_id: "review_evidence".into(),
            },
            evidence: AudioArtifactReference {
                id: "review_evidence".into(),
                sha256: "a".repeat(64),
                byte_size: 1,
            },
            source_sha256: document.source.sha256.clone(),
        };
        document.validate().unwrap();
        assert!(elevated.validate().is_err());
        let mut calibrated = report;
        calibrated.confidence = AudioConfidence::Calibrated { score: 0.9 };
        assert!(calibrated.validate().is_err());
    }

    #[test]
    fn unavailable_transcription_does_not_claim_a_captured_result_or_text_export() {
        let mut report = AudioTranscriptionReport::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-transcription-v1.example.json"
        ))
        .unwrap();
        report.result = TranscriptionResult::Unavailable {
            reason: TranscriptionUnavailableReason::SilentInput,
        };
        report.commands = super::super::provider::command_evidence(report.settings.threads, false);
        assert!(report.validate().is_err());
        report.raw_observation = None;
        report.timed_text = None;
        assert!(report.validate().is_ok());
    }
}
