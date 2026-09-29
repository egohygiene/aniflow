//! Closed contracts for proposed alignment of explicitly supplied reviewed lyrics.
//! Deserialization alone is unvalidated; use the parsers or `validate` methods.
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::audio_analysis::{
    AudioArtifactReference, AudioConfidence, AudioProvenanceClass, AudioRationalTime, AudioScope,
    AudioSource,
};
use crate::audio_inspection::{
    AudioInspectionConfiguration, AudioTechnicalCommandEvidence, AudioToolPin,
};
use crate::timed_text::{TextProvenance, TimedTextDocument};
use crate::{Error, ErrorCategory, ProviderReference, Result};

pub const AUDIO_ALIGNMENT_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-alignment.configuration/v1";
pub const AUDIO_ALIGNMENT_PROVIDER_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-alignment.provider-configuration/v1";
pub const AUDIO_ALIGNMENT_REPORT_SCHEMA_V1: &str = "aniflow.audio-alignment/v1";
pub const AUDIO_ALIGNMENT_PREFLIGHT_SCHEMA_V1: &str = "aniflow.audio-alignment-preflight/v1";
pub const AUDIO_ALIGNMENT_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-alignment";
pub const AUDIO_ALIGNMENT_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_ALIGNMENT_CAPABILITY_ID: &str = "aniflow/audio-lyrics-alignment";
pub const AUDIO_ALIGNMENT_POCKETSPHINX_VERSION: &str = "5.1.1";
pub const AUDIO_ALIGNMENT_POCKETSPHINX_REVISION: &str = "511126b492dcb267cf30d49d631946d7b61a9530";
pub const AUDIO_ALIGNMENT_CONFIDENCE_REASON: &str = "Forced alignment scores are not calibrated confidence and do not establish that the supplied words occur in the audio.";
pub const AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES: u64 = 64 * 1024 * 1024;
pub const AUDIO_ALIGNMENT_MAXIMUM_DICTIONARY_BYTES: u64 = 16 * 1024 * 1024;
pub const AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES: usize = 64 * 1024;
pub const AUDIO_ALIGNMENT_MAXIMUM_PHRASE_BYTES: usize = 16 * 1024;
pub const AUDIO_ALIGNMENT_MAXIMUM_TOKENS: usize = 512;
pub const AUDIO_ALIGNMENT_MAXIMUM_RAW_BYTES: usize = 1024 * 1024;
pub const AUDIO_ALIGNMENT_MAXIMUM_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const AUDIO_ALIGNMENT_MODEL_FILES: [&str; 7] = [
    "feat.params",
    "mdef",
    "means",
    "noisedict",
    "sendump",
    "transition_matrices",
    "variances",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentResourcePin {
    pub name: String,
    pub sha256: String,
    pub byte_size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentModelPin {
    pub directory: PathBuf,
    pub model_id: String,
    pub revision: String,
    pub files: Vec<AlignmentResourcePin>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentDictionaryPin {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentConfiguration {
    pub schema: String,
    pub pocketsphinx: AudioToolPin,
    pub model: AlignmentModelPin,
    pub dictionary: AlignmentDictionaryPin,
    pub language: String,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedLyrics {
    pub artifact: AudioArtifactReference,
    pub document: TimedTextDocument,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAlignmentProviderConfiguration {
    pub schema: String,
    pub settings: AlignmentConfiguration,
    pub tools: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
    pub reviewed_lyrics: AudioArtifactReference,
    pub upstream_analysis_artifact_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAlignmentDiagnosticCode {
    UnsupportedLanguage,
    MissingTool,
    InvalidTool,
    ToolDigestMismatch,
    ToolVersionMismatch,
    MissingModel,
    InvalidModel,
    ModelDigestMismatch,
    ModelSizeMismatch,
    MissingDictionary,
    InvalidDictionary,
    DictionaryDigestMismatch,
    DictionarySizeMismatch,
    UnsupportedLexeme,
    ToolTimeout,
    ToolOutputLimit,
    ToolFailed,
    Cancelled,
    UnsupportedPlatform,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAlignmentDiagnostic {
    pub code: AudioAlignmentDiagnosticCode,
    pub component: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAlignmentPreflight {
    pub schema: String,
    pub ready: bool,
    pub diagnostics: Vec<AudioAlignmentDiagnostic>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentModelEvidence {
    pub model_id: String,
    pub revision: String,
    pub files: Vec<AlignmentResourcePin>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentDictionaryEvidence {
    pub sha256: String,
    pub byte_size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentSettingsEvidence {
    pub pocketsphinx_version: String,
    pub pocketsphinx_sha256: String,
    pub model: AlignmentModelEvidence,
    pub dictionary: AlignmentDictionaryEvidence,
    pub language: String,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentLicenseEvidence {
    pub tool_expression: String,
    pub tool_source_url: String,
    pub model_expression: String,
    pub model_source_url: String,
    pub dictionary_statement: String,
    pub dictionary_source_url: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentMethod {
    pub sample_rate_hz: u32,
    pub downmix: String,
    pub timestamp_grid_milliseconds: u16,
    pub tokenization: String,
    pub alternate_pronunciations: bool,
    pub gpu: bool,
    pub review_attestation_independently_verified: bool,
    pub timing_reviewed: bool,
}
/// Byte offsets address the exact authored cue string, not the normalized phrase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentToken {
    pub cue_id: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub text: String,
    pub normalized: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AlignmentWordTiming {
    Candidate {
        start: AudioRationalTime,
        end: AudioRationalTime,
    },
    Unmatched {},
    Ambiguous {},
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignedWord {
    pub token: AlignmentToken,
    pub timing: AlignmentWordTiming,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AlignmentCueTiming {
    Candidate {
        start: AudioRationalTime,
        end: AudioRationalTime,
    },
    Partial {},
    Unmatched {},
    Ambiguous {},
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignedCue {
    pub cue_id: String,
    pub timing: AlignmentCueTiming,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentNativeWord {
    pub text: String,
    pub start: AudioRationalTime,
    pub end: AudioRationalTime,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentObservation {
    pub native_words: Vec<AlignmentNativeWord>,
    pub words: Vec<AlignedWord>,
    pub cues: Vec<AlignedCue>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentUnavailableReason {
    UnsupportedSampleRate,
    UnsupportedChannels,
    SilentInput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AlignmentResult {
    Candidate { observation: AlignmentObservation },
    Unavailable { reason: AlignmentUnavailableReason },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAlignmentReport {
    pub schema: String,
    pub source: AudioSource,
    pub scope: AudioScope,
    pub technical_artifact: AudioArtifactReference,
    pub upstream_analysis_artifact: AudioArtifactReference,
    pub reviewed_lyrics: ReviewedLyrics,
    pub raw_observation: Option<AudioArtifactReference>,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub settings: AlignmentSettingsEvidence,
    pub licenses: AlignmentLicenseEvidence,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
    pub method: AlignmentMethod,
    pub provenance: AudioProvenanceClass,
    pub confidence: AudioConfidence,
    pub result: AlignmentResult,
    pub timed_text: Option<TimedTextDocument>,
}

impl AlignmentConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "alignment configuration")?;
        let value: Self = crate::provider::decode_json(bytes, "alignment configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_ALIGNMENT_CONFIGURATION_SCHEMA_V1
            || self.pocketsphinx.version != AUDIO_ALIGNMENT_POCKETSPHINX_VERSION
        {
            return Err(invalid(
                "alignment requires its v1 configuration and pinned PocketSphinx 5.1.1 declaration",
            ));
        }
        normalized_path(&self.pocketsphinx.executable)?;
        normalized_path(&self.model.directory)?;
        normalized_path(&self.dictionary.path)?;
        crate::provider::require_sha256(&self.pocketsphinx.sha256, "alignment executable sha256")?;
        validate_model(&AlignmentModelEvidence::from(&self.model))?;
        validate_dictionary(&AlignmentDictionaryEvidence::from(&self.dictionary))?;
        text(&self.language, 16)?;
        if !self
            .language
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c == b'-')
        {
            return Err(invalid(
                "alignment language must be a bounded lowercase language token",
            ));
        }
        bounds(
            self.tool_timeout_milliseconds,
            self.maximum_tool_output_bytes,
        )
    }
}
impl ReviewedLyrics {
    /// Read an existing timed-text JSON document and bind its exact original bytes.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(
            bytes,
            AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES,
            "reviewed lyrics JSON",
        )?;
        let value = Self {
            artifact: AudioArtifactReference {
                id: "reviewed_lyrics".into(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                byte_size: bytes.len() as u64,
            },
            document: TimedTextDocument::from_json_slice(bytes)?,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.document.validate()?;
        reference(&self.artifact, AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES as u64)?;
        if self.artifact.id != "reviewed_lyrics"
            || !matches!(
                self.document.provenance,
                TextProvenance::ReviewedLyrics { .. }
            )
            || self.document.language.as_deref() != Some("en")
        {
            return Err(invalid(
                "alignment accepts only explicitly reviewed English timed-text documents",
            ));
        }
        // The original byte identity may include formatting and cannot be reconstructed
        // from this parsed document. The runtime verifies it against the input bytes.
        bounded(
            &self.document.canonical_json_bytes()?,
            AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES,
            "normalized reviewed lyrics",
        )?;
        super::normalize::tokenize_document(&self.document)?;
        Ok(())
    }
    pub fn validate_source(&self, source: &AudioSource) -> Result<()> {
        self.validate()?;
        source.validate()?;
        if self
            .document
            .audio_source
            .as_ref()
            .is_some_and(|bound| bound != source)
        {
            return Err(invalid(
                "reviewed lyrics are bound to a different audio source or stem",
            ));
        }
        Ok(())
    }
}
impl AudioAlignmentProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        self.tools.validate()?;
        reference(&self.source, 256 * 1024 * 1024)?;
        reference(
            &self.reviewed_lyrics,
            AUDIO_ALIGNMENT_MAXIMUM_LYRICS_BYTES as u64,
        )?;
        if self.schema != AUDIO_ALIGNMENT_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.source.id != "source_audio"
            || self.reviewed_lyrics.id != "reviewed_lyrics"
            || !matches!(
                self.upstream_analysis_artifact_id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid(
                "unsupported alignment provider configuration or source bindings",
            ));
        }
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value =
            serde_json::to_value(self).map_err(|_| invalid("cannot encode alignment settings"))?;
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_ALIGNMENT_PROVIDER_ID.into(),
                version: AUDIO_ALIGNMENT_PROVIDER_VERSION.into(),
            },
            crate::CapabilityReference {
                id: AUDIO_ALIGNMENT_CAPABILITY_ID.into(),
                version: "1.0.0".into(),
            },
            configuration_schema_reference(),
            value
                .as_object()
                .expect("configuration struct")
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )
    }
}
#[must_use]
pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference {
        id: AUDIO_ALIGNMENT_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
        version: "1.0.0".into(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-alignment/provider-configuration.schema.json"
            ))
        ),
    }
}
impl From<&AlignmentModelPin> for AlignmentModelEvidence {
    fn from(value: &AlignmentModelPin) -> Self {
        Self {
            model_id: value.model_id.clone(),
            revision: value.revision.clone(),
            files: value.files.clone(),
        }
    }
}
impl From<&AlignmentDictionaryPin> for AlignmentDictionaryEvidence {
    fn from(value: &AlignmentDictionaryPin) -> Self {
        Self {
            sha256: value.sha256.clone(),
            byte_size: value.byte_size,
        }
    }
}
impl AlignmentSettingsEvidence {
    #[must_use]
    pub fn from_configuration(value: &AlignmentConfiguration) -> Self {
        Self {
            pocketsphinx_version: value.pocketsphinx.version.clone(),
            pocketsphinx_sha256: value.pocketsphinx.sha256.clone(),
            model: AlignmentModelEvidence::from(&value.model),
            dictionary: AlignmentDictionaryEvidence::from(&value.dictionary),
            language: value.language.clone(),
            tool_timeout_milliseconds: value.tool_timeout_milliseconds,
            maximum_tool_output_bytes: value.maximum_tool_output_bytes,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.pocketsphinx_version != AUDIO_ALIGNMENT_POCKETSPHINX_VERSION
            || self.language != "en"
        {
            return Err(invalid(
                "alignment evidence requires the pinned English-only profile",
            ));
        }
        crate::provider::require_sha256(&self.pocketsphinx_sha256, "alignment executable sha256")?;
        validate_model(&self.model)?;
        validate_dictionary(&self.dictionary)?;
        bounds(
            self.tool_timeout_milliseconds,
            self.maximum_tool_output_bytes,
        )
    }
}
impl Default for AlignmentMethod {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16000,
            downmix: "none".into(),
            timestamp_grid_milliseconds: 10,
            tokenization: "ascii-english-byte-offsets/v1".into(),
            alternate_pronunciations: false,
            gpu: false,
            review_attestation_independently_verified: false,
            timing_reviewed: false,
        }
    }
}
impl Default for AlignmentLicenseEvidence {
    fn default() -> Self {
        Self {
            tool_expression: "BSD-2-Clause; bundled components retain their own notices".into(),
            tool_source_url: format!("https://github.com/cmusphinx/pocketsphinx/blob/{AUDIO_ALIGNMENT_POCKETSPHINX_REVISION}/LICENSE"),
            model_expression: "BSD-2-Clause (Alpha Cephei declaration)".into(),
            model_source_url: format!("https://github.com/cmusphinx/pocketsphinx/blob/{AUDIO_ALIGNMENT_POCKETSPHINX_REVISION}/model/en-us/en-us/README"),
            dictionary_statement: "CMU dictionary attribution and upstream terms are recorded, not independently verified.".into(),
            dictionary_source_url: format!("https://github.com/cmusphinx/pocketsphinx/blob/{AUDIO_ALIGNMENT_POCKETSPHINX_REVISION}/model/en-us/cmudict-en-us.dict"),
        }
    }
}
impl AudioAlignmentPreflight {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
            && self.diagnostics.is_empty()
            && self.schema == AUDIO_ALIGNMENT_PREFLIGHT_SCHEMA_V1
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_ALIGNMENT_PREFLIGHT_SCHEMA_V1
            || self.ready != self.diagnostics.is_empty()
            || self.diagnostics.len() > 32
        {
            return Err(invalid("inconsistent alignment preflight outcome"));
        }
        for diagnostic in &self.diagnostics {
            text(&diagnostic.component, 256)?;
            text(&diagnostic.message, 4096)?;
        }
        Ok(())
    }
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "alignment preflight")?;
        let value: Self = crate::provider::decode_json(bytes, "alignment preflight")?;
        value.validate()?;
        Ok(value)
    }
}
impl AlignmentObservation {
    /// Structural coverage only; this never certifies acoustic correctness.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.words.is_empty()
            && !self.cues.is_empty()
            && self
                .words
                .iter()
                .all(|word| matches!(word.timing, AlignmentWordTiming::Candidate { .. }))
            && self
                .cues
                .iter()
                .all(|cue| matches!(cue.timing, AlignmentCueTiming::Candidate { .. }))
    }
    pub fn validate(&self, source: &AudioSource, reviewed: &ReviewedLyrics) -> Result<()> {
        let expected = super::normalize::map_words(self.native_words.clone(), source, reviewed)?;
        if self != &expected {
            return Err(invalid(
                "alignment candidates do not exactly preserve the reviewed tokens and conservative native mapping",
            ));
        }
        Ok(())
    }
}
impl AudioAlignmentReport {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(
            bytes,
            AUDIO_ALIGNMENT_MAXIMUM_REPORT_BYTES,
            "alignment report",
        )?;
        let value: Self = crate::provider::decode_json(bytes, "alignment report")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        bounded(
            &bytes,
            AUDIO_ALIGNMENT_MAXIMUM_REPORT_BYTES,
            "alignment report",
        )?;
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.settings.validate()?;
        self.reviewed_lyrics.validate_source(&self.source)?;
        let scope = AudioScope {
            channels: (0..self.source.channels).collect(),
            stem_id: self.source.stem.as_ref().map(|stem| stem.id.clone()),
        };
        if self.schema != AUDIO_ALIGNMENT_REPORT_SCHEMA_V1
            || self.source.artifact.id != "source_audio"
            || self.scope != scope
            || self.provider
                != (ProviderReference {
                    id: AUDIO_ALIGNMENT_PROVIDER_ID.into(),
                    version: AUDIO_ALIGNMENT_PROVIDER_VERSION.into(),
                })
            || self.method != AlignmentMethod::default()
            || self.licenses != AlignmentLicenseEvidence::default()
            || self.provenance != AudioProvenanceClass::Probabilistic
            || self.confidence
                != (AudioConfidence::Unavailable {
                    reason: AUDIO_ALIGNMENT_CONFIDENCE_REASON.into(),
                })
        {
            return Err(invalid(
                "unsupported alignment report identity, method, scope, licenses, or uncertainty",
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
                "alignment source is outside the inspected PCM16 profile",
            ));
        }
        for hash in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
        ] {
            crate::provider::require_sha256(hash, "alignment provenance digest")?;
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
                "alignment upstream artifact identities differ from the pipeline",
            ));
        }
        let observed = match &self.result {
            AlignmentResult::Candidate { observation } => {
                observation.validate(&self.source, &self.reviewed_lyrics)?;
                let raw = self.raw_observation.as_ref().ok_or_else(|| {
                    invalid("alignment candidates require captured observation identity")
                })?;
                reference(raw, AUDIO_ALIGNMENT_MAXIMUM_RAW_BYTES as u64)?;
                if raw.id != "alignment_observation" {
                    return Err(invalid("unexpected captured alignment identity"));
                }
                let expected =
                    super::normalize::timed_text(observation, &self.source, &self.reviewed_lyrics)?;
                if self.timed_text.as_ref() != Some(&expected) {
                    return Err(invalid(
                        "alignment timed text must preserve reviewed text and authority exactly, with only proposed cue timing",
                    ));
                }
                true
            }
            AlignmentResult::Unavailable { reason } => {
                let valid = match reason {
                    AlignmentUnavailableReason::UnsupportedSampleRate => {
                        self.source.sample_rate_hz != 16000
                    }
                    AlignmentUnavailableReason::UnsupportedChannels => {
                        self.source.sample_rate_hz == 16000 && self.source.channels != 1
                    }
                    AlignmentUnavailableReason::SilentInput => {
                        self.source.sample_rate_hz == 16000 && self.source.channels == 1
                    }
                };
                if !valid || self.raw_observation.is_some() || self.timed_text.is_some() {
                    return Err(invalid(
                        "unavailable alignment has an inconsistent reason or fabricated candidate",
                    ));
                }
                false
            }
        };
        if self.commands != super::provider::command_evidence(observed) {
            return Err(invalid(
                "alignment command evidence differs from the pinned invocation",
            ));
        }
        Ok(())
    }
}
fn bounds(timeout: u64, output: u64) -> Result<()> {
    if !(1..=120000).contains(&timeout) || !(1024..=1048576).contains(&output) {
        return Err(invalid(
            "alignment execution bounds exceed the supported profile",
        ));
    }
    Ok(())
}
fn validate_model(model: &AlignmentModelEvidence) -> Result<()> {
    text(&model.revision, 256)?;
    if model.model_id != "en-us" || model.files.len() != AUDIO_ALIGNMENT_MODEL_FILES.len() {
        return Err(invalid(
            "alignment requires the closed en-us acoustic model inventory",
        ));
    }
    let mut total = 0u64;
    for (file, name) in model.files.iter().zip(AUDIO_ALIGNMENT_MODEL_FILES) {
        crate::provider::require_sha256(&file.sha256, "alignment model resource sha256")?;
        if file.name != name
            || file.byte_size == 0
            || file.byte_size > AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES
        {
            return Err(invalid(
                "alignment model files require exact sorted names and bounded sizes",
            ));
        }
        total = total
            .checked_add(file.byte_size)
            .ok_or_else(|| invalid("alignment model size overflow"))?;
    }
    if total > AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES {
        return Err(invalid("alignment acoustic model exceeds 64 MiB"));
    }
    Ok(())
}
fn validate_dictionary(value: &AlignmentDictionaryEvidence) -> Result<()> {
    crate::provider::require_sha256(&value.sha256, "alignment dictionary sha256")?;
    if value.byte_size == 0 || value.byte_size > AUDIO_ALIGNMENT_MAXIMUM_DICTIONARY_BYTES {
        return Err(invalid("alignment dictionary exceeds its size bound"));
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
            "alignment asset paths must be absolute normalized UTF-8 paths",
        ));
    }
    Ok(())
}
pub(super) fn reference(value: &AudioArtifactReference, maximum: u64) -> Result<()> {
    text(&value.id, 256)?;
    crate::provider::require_sha256(&value.sha256, "alignment artifact sha256")?;
    if value.byte_size == 0 || value.byte_size > maximum {
        return Err(invalid("alignment artifact size exceeds its bound"));
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
            "alignment metadata must be bounded nonempty printable text",
        ));
    }
    Ok(())
}
pub(super) fn milliseconds(time: AudioRationalTime) -> Result<u64> {
    time.validate()?;
    if time.numerator < 0 {
        return Err(invalid("alignment timestamps cannot be negative"));
    }
    let scaled = u128::from(time.numerator as u64) * 1000;
    if scaled % u128::from(time.denominator) != 0 {
        return Err(invalid("alignment timestamps must be exact milliseconds"));
    }
    let ms = scaled / u128::from(time.denominator);
    if ms > 600000 || ms % 10 != 0 {
        return Err(invalid(
            "alignment timing requires the bounded 10 ms native grid",
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

    fn example() -> AudioAlignmentReport {
        AudioAlignmentReport::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-alignment-v1.example.json"
        ))
        .unwrap()
    }
    #[test]
    fn published_report_preserves_text_review_but_refuses_timing_authority_or_confidence() {
        let report = example();
        assert_eq!(
            report.timed_text.as_ref().unwrap().provenance,
            report.reviewed_lyrics.document.provenance
        );
        let mut changed = report.clone();
        changed.method.timing_reviewed = true;
        assert!(changed.validate().is_err());
        let mut changed = report.clone();
        changed.method.review_attestation_independently_verified = true;
        assert!(changed.validate().is_err());
        let mut changed = report.clone();
        changed.confidence = AudioConfidence::Calibrated { score: 1.0 };
        assert!(changed.validate().is_err());
        let mut changed = report.clone();
        changed.provenance = AudioProvenanceClass::Deterministic;
        assert!(changed.validate().is_err());
        let mut changed = report.clone();
        changed.timed_text.as_mut().unwrap().cues[0].text.push('!');
        assert!(changed.validate().is_err());
        let mut changed = report;
        changed.timed_text.as_mut().unwrap().metadata.clear();
        assert!(changed.validate().is_err());
    }
    #[test]
    fn report_rederives_candidate_words_and_cues_instead_of_trusting_status() {
        let mut report = example();
        let AlignmentResult::Candidate { observation } = &mut report.result else {
            unreachable!()
        };
        observation.words[0].token.byte_end += 1;
        assert!(report.validate().is_err());
        let mut report = example();
        let AlignmentResult::Candidate { observation } = &mut report.result else {
            unreachable!()
        };
        observation.cues[0].timing = AlignmentCueTiming::Unmatched {};
        assert!(report.validate().is_err());
        let mut report = example();
        report.raw_observation = None;
        assert!(report.validate().is_err());
    }
    #[test]
    fn settings_require_the_closed_resource_inventory_and_execution_bounds() {
        let report = example();
        let mut settings = report.settings.clone();
        settings.model.files.swap(0, 1);
        assert!(settings.validate().is_err());
        let mut settings = report.settings.clone();
        settings.model.files[0].byte_size = AUDIO_ALIGNMENT_MAXIMUM_MODEL_BYTES;
        assert!(settings.validate().is_err());
        let mut settings = report.settings.clone();
        settings.dictionary.byte_size = AUDIO_ALIGNMENT_MAXIMUM_DICTIONARY_BYTES + 1;
        assert!(settings.validate().is_err());
        let mut settings = report.settings.clone();
        settings.maximum_tool_output_bytes = 1023;
        assert!(settings.validate().is_err());
        let mut settings = report.settings;
        settings.language = "fr".into();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn report_unavailability_is_explicit_and_cannot_keep_fabricated_candidates() {
        let mut report = example();
        report.result = AlignmentResult::Unavailable {
            reason: AlignmentUnavailableReason::UnsupportedSampleRate,
        };
        assert!(report.validate().is_err());
        report.result = AlignmentResult::Unavailable {
            reason: AlignmentUnavailableReason::SilentInput,
        };
        report.raw_observation = None;
        report.timed_text = None;
        report.commands = super::super::provider::command_evidence(false);
        report.validate().unwrap();
    }
}
