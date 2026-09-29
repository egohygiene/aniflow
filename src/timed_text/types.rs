//! Closed, loss-aware text interchange contracts.
use crate::audio_analysis::{
    AudioArtifactReference, AudioRationalTime, AudioReviewedAuthority, AudioSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const TIMED_TEXT_SCHEMA_V1: &str = "aniflow.timed-text/v1";
pub const TIMED_TEXT_CONTEXT_SCHEMA_V1: &str = "aniflow.timed-text-context/v1";
pub const TIMED_TEXT_REGISTRY_SCHEMA_V1: &str = "aniflow.timed-text-registry/v1";
pub const TIMED_TEXT_CONVERSION_SCHEMA_V1: &str = "aniflow.timed-text-conversion/v1";
pub const MAX_TEXT_BYTES: usize = 1_048_576;
pub const MAX_CUES: usize = 10_000;
pub const MAX_CUE_BYTES: usize = 65_536;
pub const MAX_EVIDENCE_BYTES: u64 = 8_388_608;
pub const MAX_TIME_MILLISECONDS: u64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimedTextFormat {
    Plain,
    Lrc,
    Srt,
    Webvtt,
    Ttml,
    Json,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlapPolicy {
    Reject,
    Allow,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CueTiming {
    Untimed {},
    Point {
        at: AudioRationalTime,
    },
    Interval {
        start: AudioRationalTime,
        end: AudioRationalTime,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextProvenance {
    Unreviewed {},
    ObservedTranscript {
        producer: String,
    },
    ReviewedLyrics {
        authority: AudioReviewedAuthority,
        evidence: AudioArtifactReference,
        source_sha256: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextCue {
    pub id: String,
    pub source_label: Option<String>,
    pub text: String,
    pub timing: CueTiming,
    pub speaker: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextDocument {
    pub schema: String,
    pub source: AudioArtifactReference,
    pub provenance: TextProvenance,
    pub language: Option<String>,
    pub audio_source: Option<AudioSource>,
    pub overlap_policy: OverlapPolicy,
    #[serde(deserialize_with = "unique_metadata")]
    pub metadata: BTreeMap<String, String>,
    pub cues: Vec<TimedTextCue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextImportContext {
    pub schema: String,
    pub provenance: TextProvenance,
    pub language: Option<String>,
    pub audio_source: Option<AudioSource>,
    pub overlap_policy: OverlapPolicy,
}
impl Default for TimedTextImportContext {
    fn default() -> Self {
        Self {
            schema: TIMED_TEXT_CONTEXT_SCHEMA_V1.into(),
            provenance: TextProvenance::Unreviewed {},
            language: None,
            audio_source: None,
            overlap_policy: OverlapPolicy::Reject,
        }
    }
}
pub type ImportContext = TimedTextImportContext;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionLossKind {
    Timing,
    EndTimes,
    Precision,
    CueIdentifiers,
    Speaker,
    Metadata,
    Language,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionLoss {
    pub kind: ConversionLossKind,
    pub cue_id: Option<String>,
    pub field: String,
    pub detail: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionOptions {
    #[serde(default)]
    pub allow_losses: Vec<ConversionLossKind>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexicalFacts {
    pub utf8_bom_removed: bool,
    pub crlf_pairs: u64,
    pub lone_cr: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierOmission {
    Provenance,
    AudioBinding,
    InternalCueIdentities,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextConversionReport {
    pub schema: String,
    pub input: AudioArtifactReference,
    pub output: AudioArtifactReference,
    pub from: TimedTextFormat,
    pub to: TimedTextFormat,
    pub input_document_sha256: String,
    pub output_document_sha256: String,
    pub allowed_losses: Vec<ConversionLossKind>,
    pub losses: Vec<ConversionLoss>,
    pub lexical: LexicalFacts,
    pub carrier_omissions: Vec<CarrierOmission>,
    pub review_attestation_independently_verified: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextFormatProfile {
    pub format: TimedTextFormat,
    pub extension: String,
    pub media_type: String,
    pub subset: String,
    pub transport_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimedTextRegistry {
    pub schema: String,
    pub formats: Vec<TimedTextFormatProfile>,
}

#[derive(Debug, Clone)]
pub struct DecodedText {
    pub document: TimedTextDocument,
    pub lexical: LexicalFacts,
}
#[derive(Debug, Clone)]
pub struct EncodedText {
    pub document: TimedTextDocument,
    pub bytes: Vec<u8>,
    pub losses: Vec<ConversionLoss>,
    pub carrier_omissions: Vec<CarrierOmission>,
}
#[derive(Debug, Clone)]
pub struct TimedTextConversion {
    pub input_document: TimedTextDocument,
    pub output_document: TimedTextDocument,
    pub bytes: Vec<u8>,
    pub report: TimedTextConversionReport,
}
#[derive(Debug)]
pub struct ConversionFailure {
    pub error: crate::Error,
    pub losses: Vec<ConversionLoss>,
}
impl std::fmt::Display for ConversionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ConversionFailure {}
impl From<crate::Error> for ConversionFailure {
    fn from(error: crate::Error) -> Self {
        Self {
            error,
            losses: Vec::new(),
        }
    }
}

pub(super) struct ParsedCue {
    pub source_label: Option<String>,
    pub text: String,
    pub timing: CueTiming,
    pub speaker: Option<String>,
}
pub(super) struct ParsedText {
    pub cues: Vec<ParsedCue>,
    pub language: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl TimedTextFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Lrc => "lrc",
            Self::Srt => "srt",
            Self::Webvtt => "webvtt",
            Self::Ttml => "ttml",
            Self::Json => "json",
        }
    }
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Plain => "txt",
            Self::Webvtt => "vtt",
            other => other.as_str(),
        }
    }
}
impl std::str::FromStr for TimedTextFormat {
    type Err = crate::Error;
    fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "plain" => Ok(Self::Plain),
            "lrc" => Ok(Self::Lrc),
            "srt" => Ok(Self::Srt),
            "webvtt" => Ok(Self::Webvtt),
            "ttml" => Ok(Self::Ttml),
            "json" => Ok(Self::Json),
            _ => Err(invalid(
                "unsupported timed-text format; consult the registered format profiles",
            )),
        }
    }
}
impl std::str::FromStr for ConversionLossKind {
    type Err = crate::Error;
    fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "timing" => Ok(Self::Timing),
            "end_times" => Ok(Self::EndTimes),
            "precision" => Ok(Self::Precision),
            "cue_identifiers" => Ok(Self::CueIdentifiers),
            "speaker" => Ok(Self::Speaker),
            "metadata" => Ok(Self::Metadata),
            "language" => Ok(Self::Language),
            _ => Err(invalid("unsupported timed-text loss permission")),
        }
    }
}
impl TimedTextImportContext {
    pub fn from_json_slice(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() > MAX_TEXT_BYTES {
            return Err(invalid("timed-text context exceeds 1 MiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "timed-text context")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != TIMED_TEXT_CONTEXT_SCHEMA_V1 {
            return Err(invalid("unsupported timed-text context schema"));
        }
        validate_provenance(&self.provenance, None)?;
        validate_language(self.language.as_deref())?;
        if let Some(source) = &self.audio_source {
            source.validate()?;
        }
        Ok(())
    }
}
impl TimedTextDocument {
    pub fn from_json_slice(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() > MAX_EVIDENCE_BYTES as usize {
            return Err(invalid("timed-text document exceeds 8 MiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "timed-text document")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> crate::Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAX_EVIDENCE_BYTES as usize {
            return Err(invalid("normalized timed-text evidence exceeds 8 MiB"));
        }
        Ok(bytes)
    }
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != TIMED_TEXT_SCHEMA_V1
            || self.source.id != "source_text"
            || self.source.byte_size == 0
            || self.source.byte_size > MAX_TEXT_BYTES as u64
        {
            return Err(invalid("unsupported timed-text schema or source identity"));
        }
        crate::provider::require_sha256(&self.source.sha256, "text source sha256")?;
        validate_provenance(&self.provenance, Some(&self.source.sha256))?;
        validate_language(self.language.as_deref())?;
        if let Some(source) = &self.audio_source {
            source.validate()?;
        }
        if self.metadata.len() > 32 {
            return Err(invalid("timed-text metadata exceeds 32 fields"));
        }
        for (key, value) in &self.metadata {
            identifier(key, 64, true)?;
            printable(value, 4096, true)?;
        }
        if self.cues.is_empty() || self.cues.len() > MAX_CUES {
            return Err(invalid("timed-text document requires one to 10000 cues"));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut text_bytes = 0usize;
        let mut previous_start = None;
        let mut active_end = 0u64;
        let mut previous_point = None;
        for cue in &self.cues {
            identifier(&cue.id, 128, false)?;
            if !ids.insert(cue.id.as_str()) {
                return Err(invalid("duplicate internal cue identity"));
            }
            if let Some(label) = &cue.source_label {
                printable(label, 256, false)?;
            }
            if let Some(speaker) = &cue.speaker {
                printable(speaker, 256, false)?;
            }
            if cue.text.is_empty()
                || cue.text.len() > MAX_CUE_BYTES
                || cue
                    .text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            {
                return Err(invalid(
                    "cue text must be nonempty UTF-8 with only supported text controls and at most 64 KiB",
                ));
            }
            text_bytes = text_bytes
                .checked_add(cue.text.len())
                .ok_or_else(|| invalid("cue text byte count overflow"))?;
            if text_bytes > MAX_TEXT_BYTES {
                return Err(invalid("aggregate cue text exceeds 1 MiB"));
            }
            let (start, end, point) = match cue.timing {
                CueTiming::Untimed {} => continue,
                CueTiming::Point { at } => {
                    let at = super::milliseconds(at)?;
                    (at, at, true)
                }
                CueTiming::Interval { start, end } => {
                    let start = super::milliseconds(start)?;
                    let end = super::milliseconds(end)?;
                    if start >= end {
                        return Err(invalid("cue intervals must be nonempty and forward"));
                    }
                    (start, end, false)
                }
            };
            if previous_start.is_some_and(|previous| start < previous) {
                return Err(invalid(
                    "cue timing must preserve nondecreasing authored order",
                ));
            }
            if self.overlap_policy == OverlapPolicy::Reject
                && (start < active_end || previous_point == Some(start))
            {
                return Err(invalid(
                    "overlapping cues require an explicit allow overlap policy",
                ));
            }
            if let Some(source) = &self.audio_source {
                let time = if point { start } else { end };
                if u128::from(time) * u128::from(source.sample_rate_hz)
                    > u128::from(source.frame_count) * 1000
                {
                    return Err(invalid("cue timing exceeds the supplied audio duration"));
                }
            }
            previous_start = Some(start);
            active_end = active_end.max(end);
            previous_point = point.then_some(start);
        }
        Ok(())
    }
}
impl ConversionOptions {
    pub fn validate(&self) -> crate::Result<()> {
        let mut found = std::collections::BTreeSet::new();
        if self.allow_losses.iter().any(|kind| !found.insert(*kind)) {
            return Err(invalid("duplicate conversion loss permission"));
        }
        Ok(())
    }
}
impl TimedTextConversionReport {
    pub fn from_json_slice(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() > MAX_EVIDENCE_BYTES as usize {
            return Err(invalid("timed-text report exceeds 8 MiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "timed-text conversion report")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> crate::Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAX_EVIDENCE_BYTES as usize {
            return Err(invalid("timed-text report exceeds 8 MiB"));
        }
        Ok(bytes)
    }
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != TIMED_TEXT_CONVERSION_SCHEMA_V1
            || self.review_attestation_independently_verified
        {
            return Err(invalid(
                "unsupported conversion report or invented review verification",
            ));
        }
        for (reference, id) in [
            (&self.input, "conversion_input"),
            (&self.output, "conversion_output"),
        ] {
            if reference.id != id
                || reference.byte_size == 0
                || reference.byte_size > MAX_TEXT_BYTES as u64
            {
                return Err(invalid("conversion byte identity is outside its bounds"));
            }
            crate::provider::require_sha256(&reference.sha256, "conversion artifact sha256")?;
        }
        for digest in [&self.input_document_sha256, &self.output_document_sha256] {
            crate::provider::require_sha256(digest, "normalized text sha256")?;
        }
        ConversionOptions {
            allow_losses: self.allowed_losses.clone(),
        }
        .validate()?;
        if self.losses.len() > 100000
            || self.lexical.crlf_pairs > MAX_TEXT_BYTES as u64
            || self.lexical.lone_cr > MAX_TEXT_BYTES as u64
        {
            return Err(invalid("conversion report collections exceed bounds"));
        }
        for loss in &self.losses {
            if !self.allowed_losses.contains(&loss.kind) {
                return Err(invalid(
                    "conversion report contains a loss without explicit permission",
                ));
            }
            if let Some(id) = &loss.cue_id {
                identifier(id, 128, false)?;
            }
            printable(&loss.field, 128, false)?;
            printable(&loss.detail, 1024, false)?;
        }
        let expected = if self.to == TimedTextFormat::Json {
            Vec::new()
        } else {
            vec![
                CarrierOmission::Provenance,
                CarrierOmission::InternalCueIdentities,
            ]
        };
        if self.to == TimedTextFormat::Json {
            if !self.carrier_omissions.is_empty() || !self.losses.is_empty() {
                return Err(invalid(
                    "JSON transport preserves all document and carrier semantics",
                ));
            }
        } else if self.carrier_omissions != expected
            && self.carrier_omissions
                != vec![
                    CarrierOmission::Provenance,
                    CarrierOmission::AudioBinding,
                    CarrierOmission::InternalCueIdentities,
                ]
        {
            return Err(invalid(
                "conversion carrier omissions differ from the registered representation",
            ));
        }
        Ok(())
    }
}
fn validate_provenance(value: &TextProvenance, source_hash: Option<&str>) -> crate::Result<()> {
    match value {
        TextProvenance::Unreviewed {} => Ok(()),
        TextProvenance::ObservedTranscript { producer } => printable(producer, 256, false),
        TextProvenance::ReviewedLyrics {
            authority,
            evidence,
            source_sha256,
        } => {
            printable(&authority.supplied_by, 256, false)?;
            identifier(&evidence.id, 128, false)?;
            crate::provider::require_sha256(&evidence.sha256, "supplied review evidence sha256")?;
            crate::provider::require_sha256(source_sha256, "reviewed original source sha256")?;
            if authority.provenance_artifact_id != evidence.id
                || evidence.id == "source_text"
                || evidence.byte_size == 0
                || evidence.byte_size > MAX_EVIDENCE_BYTES
                || source_hash.is_some_and(|source| source != source_sha256)
            {
                return Err(invalid(
                    "reviewed lyrics require separate supplied evidence bound to the original imported bytes",
                ));
            }
            Ok(())
        }
    }
}
fn validate_language(value: Option<&str>) -> crate::Result<()> {
    if let Some(value) = value {
        let mut parts = value.split('-');
        let first = parts.next().unwrap_or("");
        if value.len() > 63
            || !(2..=8).contains(&first.len())
            || !first.bytes().all(|c| c.is_ascii_alphabetic())
            || parts.any(|part| {
                part.is_empty()
                    || part.len() > 8
                    || !part.bytes().all(|c| c.is_ascii_alphanumeric())
            })
        {
            return Err(invalid(
                "language must use the registered bounded BCP47-style tag subset",
            ));
        }
    }
    Ok(())
}
fn printable(value: &str, maximum: usize, allow_empty: bool) -> crate::Result<()> {
    if (!allow_empty && value.is_empty())
        || value.len() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(invalid("text metadata must be bounded and printable"));
    }
    Ok(())
}
fn identifier(value: &str, maximum: usize, dots: bool) -> crate::Result<()> {
    if value.is_empty()
        || value.len() > maximum
        || !value.as_bytes()[0].is_ascii_lowercase()
        || !value.bytes().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, b'_' | b'-')
                || (dots && c == b'.')
        })
    {
        return Err(invalid("invalid bounded lowercase text identifier"));
    }
    Ok(())
}
pub(super) fn invalid(message: impl Into<String>) -> crate::Error {
    crate::Error::new(crate::ErrorCategory::Configuration, message)
}

fn unique_metadata<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct UniqueMetadata;
    impl<'de> serde::de::Visitor<'de> for UniqueMetadata {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most 32 unique text metadata keys")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut access: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, String>()? {
                if values.len() == 32 {
                    return Err(serde::de::Error::custom("text metadata exceeds 32 fields"));
                }
                if values.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate text metadata key"));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(UniqueMetadata)
}
