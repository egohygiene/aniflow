//! Provider-neutral audio evidence for the bounded, zero-origin, single-stream v1 profile.
//!
//! This module performs contract validation only. It does not read artifacts, run
//! analyzers, certify media, grant lyric authority, or authorize cached reuse.
//! Deserialize alone is unvalidated: use [`AudioAnalysis::from_json_slice`] or
//! call [`AudioAnalysis::validate`] before consuming a constructed value.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::provider::{canonical_json_bytes, require_sha256, validate_provider_reference};
use crate::{CapabilityReference, Error, ErrorCategory, ProviderReference, Result};

pub const AUDIO_ANALYSIS_SCHEMA_V1: &str = "aniflow.audio-analysis/v1";
/// Maximum serialized input accepted by `AudioAnalysis::from_json_slice`.
pub const AUDIO_MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
pub const AUDIO_MAX_COLLECTION_ITEMS: usize = 10_000;
pub const AUDIO_MAX_EXTENSION_DEPTH: usize = 32;
pub const AUDIO_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const AUDIO_MAX_SAMPLE_RATE_HZ: u32 = 768_000;
pub const AUDIO_MAX_CHANNELS: u16 = 64;
pub const AUDIO_CAPABILITY_VERSION_V1: &str = "1.0.0";
pub const AUDIO_CAPABILITY_IDS_V1: &[&str] = &[
    "aniflow/audio-technical-inspection",
    "aniflow/audio-signal-measurements",
    "aniflow/audio-stem-lineage",
    "aniflow/audio-musical-structure",
    "aniflow/audio-timed-text",
    "aniflow/audio-transcription",
    "aniflow/audio-lyrics-alignment",
    "aniflow/audio-midi-extraction",
];

/// Artifact identity only; resolving or verifying bytes belongs to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioArtifactReference {
    pub id: String,
    pub sha256: String,
    pub byte_size: u64,
}

/// Exact seconds. Valid values are reduced, with a positive denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioRationalTime {
    pub numerator: i64,
    pub denominator: u32,
}

/// Nonempty half-open source sample-frame range: start is included, end excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioFrameRange {
    pub start: u64,
    pub end: u64,
}

/// This proves a declared relationship, not alignment to the original mix clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStemIdentity {
    pub id: String,
    pub original_mix: AudioArtifactReference,
    pub relationship_evidence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSource {
    pub artifact: AudioArtifactReference,
    pub stream_index: u32,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub origin: AudioRationalTime,
    pub stem: Option<AudioStemIdentity>,
}

/// Explicit zero-based channels in ascending order; no implicit all-channel scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioScope {
    pub channels: Vec<u16>,
    pub stem_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioLicenseEvidence {
    Recorded {
        statement: String,
        evidence_artifact_id: String,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioComponentEvidence {
    pub id: String,
    pub version: String,
    pub revision: String,
    pub sha256: String,
    pub license: AudioLicenseEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioModelEvidence {
    NoneRequired {},
    Available {
        components: Vec<AudioComponentEvidence>,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioProviderEvidence {
    pub id: String,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub tools: Vec<AudioComponentEvidence>,
    pub models: AudioModelEvidence,
    pub license: AudioLicenseEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioProvenanceClass {
    Deterministic,
    Heuristic,
    Probabilistic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioConfidence {
    Calibrated { score: f64 },
    Uncalibrated { score: f64 },
    Unavailable { reason: String },
    NotApplicable {},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioObservationProvenance {
    pub class: AudioProvenanceClass,
    pub confidence: AudioConfidence,
    pub provider_evidence_id: String,
    pub evidence_artifact_ids: Vec<String>,
}

/// Intentionally small: later checkpoints add family-specific payload contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioObservationKind {
    SampleRate,
    ChannelCount,
    Tempo,
    Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioUnit {
    Hertz,
    Channels,
    BeatsPerMinute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioObservationValue {
    Quantity { value: f64, unit: AudioUnit },
    Count { value: u64, unit: AudioUnit },
    Label { value: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioObservation {
    pub id: String,
    pub capability_id: String,
    pub kind: AudioObservationKind,
    pub value: AudioObservationValue,
    pub scope: AudioScope,
    pub provenance: AudioObservationProvenance,
}

/// Markers occupy exactly one sample frame. Regions may overlap only when the
/// timeline explicitly declares OverlappingRegions (e.g. polyphonic candidates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTimelineKind {
    Markers,
    DisjointRegions,
    OverlappingRegions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioEvent {
    pub id: String,
    pub range: AudioFrameRange,
    pub label: String,
    pub provenance: AudioObservationProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTimeline {
    pub id: String,
    pub capability_id: String,
    pub kind: AudioTimelineKind,
    pub scope: AudioScope,
    pub events: Vec<AudioEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSemanticArtifactKind {
    ObservedTranscript,
    ReviewedLyrics,
    CandidateMidi,
}

/// Explicit supplied review provenance, never inferred from contract validity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioReviewedAuthority {
    pub supplied_by: String,
    pub provenance_artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSemanticArtifact {
    pub artifact_id: String,
    pub capability_id: String,
    pub kind: AudioSemanticArtifactKind,
    pub provenance: AudioObservationProvenance,
    pub authority: Option<AudioReviewedAuthority>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioExcerptReference {
    pub id: String,
    pub artifact_id: String,
    pub range: AudioFrameRange,
    pub scope: AudioScope,
    pub evidence_artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAnalysisStatus {
    Complete,
    Partial,
    Unavailable,
    Unsupported,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioDiagnostic {
    pub id: String,
    pub code: String,
    pub severity: AudioDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioCapabilityOutcome {
    pub capability: CapabilityReference,
    pub status: AudioAnalysisStatus,
    pub provider_evidence_ids: Vec<String>,
    pub evidence_artifact_ids: Vec<String>,
    pub diagnostic_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioAnalysis {
    pub schema: String,
    pub status: AudioAnalysisStatus,
    pub source: AudioSource,
    pub artifacts: Vec<AudioArtifactReference>,
    pub providers: Vec<AudioProviderEvidence>,
    pub capabilities: Vec<AudioCapabilityOutcome>,
    pub observations: Vec<AudioObservation>,
    pub timelines: Vec<AudioTimeline>,
    pub semantic_artifacts: Vec<AudioSemanticArtifact>,
    pub excerpts: Vec<AudioExcerptReference>,
    pub diagnostics: Vec<AudioDiagnostic>,
    /// Keys are lowercase reverse-domain namespaces. Opaque data never supplies
    /// required core evidence or changes contract interpretation.
    pub extensions: BTreeMap<String, Value>,
}

impl AudioRationalTime {
    pub fn validate(&self) -> Result<()> {
        if self.denominator == 0
            || self.numerator.unsigned_abs() > AUDIO_MAX_SAFE_INTEGER
            || gcd(self.numerator.unsigned_abs(), u64::from(self.denominator)) != 1
        {
            return Err(invalid(
                "time must be a reduced, bounded rational with positive denominator",
            ));
        }
        Ok(())
    }
}

impl AudioSource {
    pub fn validate(&self) -> Result<()> {
        validate_artifact(&self.artifact)?;
        if self.artifact.byte_size == 0 {
            return Err(invalid("audio source bytes cannot be empty"));
        }
        if self.stream_index != 0
            || self.origin
                != (AudioRationalTime {
                    numerator: 0,
                    denominator: 1,
                })
        {
            return Err(invalid(
                "audio v1 requires a single stream at index 0 with origin 0/1; stream mappings are unsupported",
            ));
        }
        if !(1..=AUDIO_MAX_SAMPLE_RATE_HZ).contains(&self.sample_rate_hz)
            || !(1..=AUDIO_MAX_CHANNELS).contains(&self.channels)
            || !(1..=AUDIO_MAX_SAFE_INTEGER).contains(&self.frame_count)
        {
            return Err(invalid(
                "source sample rate, channels, or frame count is outside the audio v1 profile",
            ));
        }
        if let Some(stem) = &self.stem {
            token(&stem.id, "stem id")?;
            token(
                &stem.relationship_evidence_id,
                "stem relationship evidence id",
            )?;
            validate_artifact(&stem.original_mix)?;
            if stem.original_mix.byte_size == 0 {
                return Err(invalid("original audio mix bytes cannot be empty"));
            }
            if stem.original_mix.id == self.artifact.id {
                return Err(invalid(
                    "stem and original mix must have distinct artifact identities",
                ));
            }
        }
        Ok(())
    }

    /// Convert a boundary (including the exclusive final boundary) to exact seconds.
    pub fn time_for_frame(&self, frame: u64) -> Result<AudioRationalTime> {
        self.validate()?;
        if frame > self.frame_count {
            return Err(invalid("frame boundary exceeds source"));
        }
        let divisor = gcd(frame, u64::from(self.sample_rate_hz));
        Ok(AudioRationalTime {
            numerator: i64::try_from(frame / divisor)
                .map_err(|_| invalid("time numerator overflow"))?,
            denominator: u32::try_from(u64::from(self.sample_rate_hz) / divisor)
                .map_err(|_| invalid("time denominator overflow"))?,
        })
    }

    /// Convert exact seconds to a frame boundary; fractional frames are refused.
    pub fn frame_for_time(&self, time: AudioRationalTime) -> Result<u64> {
        self.validate()?;
        time.validate()?;
        let numerator =
            u128::try_from(time.numerator).map_err(|_| invalid("negative source-relative time"))?;
        let scaled = numerator
            .checked_mul(u128::from(self.sample_rate_hz))
            .ok_or_else(|| invalid("sample conversion overflow"))?;
        let divisor = u128::from(time.denominator);
        if scaled % divisor != 0 {
            return Err(invalid("time is not an exact sample-frame boundary"));
        }
        let frame =
            u64::try_from(scaled / divisor).map_err(|_| invalid("sample conversion overflow"))?;
        if frame > self.frame_count {
            return Err(invalid("time exceeds source duration"));
        }
        Ok(frame)
    }
}

impl AudioFrameRange {
    pub fn validate(&self, source: &AudioSource) -> Result<()> {
        source.validate()?;
        if self.start >= self.end || self.end > source.frame_count {
            return Err(invalid(
                "range must be a nonempty half-open range within source sample frames",
            ));
        }
        Ok(())
    }
}

impl AudioAnalysis {
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        if input.len() > AUDIO_MAX_DOCUMENT_BYTES {
            return Err(invalid("audio analysis JSON exceeds 8 MiB"));
        }
        let analysis: Self = crate::provider::decode_json(input, "audio analysis")?;
        analysis.validate()?;
        Ok(analysis)
    }

    /// Validate structure and declared evidence. This never verifies artifact bytes
    /// or replaces Pipeline v3 artifact integrity and execution status checks.
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_ANALYSIS_SCHEMA_V1 {
            return Err(invalid("unsupported audio analysis schema"));
        }
        self.source.validate()?;
        for size in [
            self.artifacts.len(),
            self.providers.len(),
            self.capabilities.len(),
            self.observations.len(),
            self.timelines.len(),
            self.semantic_artifacts.len(),
            self.excerpts.len(),
            self.diagnostics.len(),
        ] {
            bounded_items(size)?;
        }
        let mut artifacts = BTreeSet::from([self.source.artifact.id.as_str()]);
        if let Some(stem) = &self.source.stem {
            artifacts.insert(stem.original_mix.id.as_str());
        }
        for artifact in &self.artifacts {
            validate_artifact(artifact)?;
            if !artifacts.insert(artifact.id.as_str()) {
                return Err(invalid("duplicate artifact id"));
            }
        }
        if let Some(stem) = &self.source.stem {
            require_reference(
                &stem.relationship_evidence_id,
                &artifacts,
                "stem relationship evidence",
            )?;
            if stem.relationship_evidence_id == self.source.artifact.id
                || stem.relationship_evidence_id == stem.original_mix.id
            {
                return Err(invalid(
                    "stem relationship requires a separate evidence artifact",
                ));
            }
        }
        let mut providers = BTreeMap::new();
        for provider in &self.providers {
            token(&provider.id, "provider evidence id")?;
            if providers.insert(provider.id.as_str(), provider).is_some() {
                return Err(invalid("duplicate provider evidence id"));
            }
            validate_provider_reference(&provider.provider)?;
            require_sha256(&provider.implementation_sha256, "implementation sha256")?;
            require_sha256(&provider.configuration_sha256, "configuration sha256")?;
            validate_components(&provider.tools, &artifacts)?;
            validate_license(&provider.license, &artifacts)?;
            match &provider.models {
                AudioModelEvidence::Available { components } => {
                    if components.is_empty() {
                        return Err(invalid("available model inventory cannot be empty"));
                    }
                    validate_components(components, &artifacts)?;
                }
                AudioModelEvidence::Unavailable { reason } => {
                    text(reason, "model unavailability reason")?
                }
                AudioModelEvidence::NoneRequired {} => (),
            }
        }
        let mut diagnostics = BTreeMap::new();
        for diagnostic in &self.diagnostics {
            token(&diagnostic.id, "diagnostic id")?;
            token(&diagnostic.code, "diagnostic code")?;
            text(&diagnostic.message, "diagnostic message")?;
            if diagnostics
                .insert(diagnostic.id.as_str(), diagnostic)
                .is_some()
            {
                return Err(invalid("duplicate diagnostic id"));
            }
        }
        let capabilities = self.validate_capabilities(&providers, &artifacts, &diagnostics)?;
        let mut ids = BTreeSet::new();
        for observation in &self.observations {
            unique_id(&observation.id, &mut ids)?;
            let capability = output_capability(&observation.capability_id, &capabilities)?;
            self.validate_scope(&observation.scope)?;
            validate_provenance(&observation.provenance, capability, &providers, &artifacts)?;
            self.validate_observation_value(observation)?;
        }
        for timeline in &self.timelines {
            unique_id(&timeline.id, &mut ids)?;
            let capability = output_capability(&timeline.capability_id, &capabilities)?;
            self.validate_scope(&timeline.scope)?;
            if timeline.events.is_empty() {
                return Err(invalid("timeline must contain at least one event"));
            }
            bounded_items(timeline.events.len())?;
            let mut previous: Option<&AudioEvent> = None;
            for event in &timeline.events {
                unique_id(&event.id, &mut ids)?;
                event.range.validate(&self.source)?;
                text(&event.label, "event label")?;
                validate_provenance(&event.provenance, capability, &providers, &artifacts)?;
                if timeline.kind == AudioTimelineKind::Markers
                    && event.range.end - event.range.start != 1
                {
                    return Err(invalid(
                        "marker ranges must occupy exactly one sample frame",
                    ));
                }
                if let Some(prior) = previous {
                    if (event.range.start, event.range.end, event.id.as_str())
                        <= (prior.range.start, prior.range.end, prior.id.as_str())
                    {
                        return Err(invalid(
                            "timeline events must be ordered by start, end, then id",
                        ));
                    }
                    if timeline.kind != AudioTimelineKind::OverlappingRegions
                        && event.range.start < prior.range.end
                    {
                        return Err(invalid(
                            "overlap is allowed only for overlapping_regions timelines",
                        ));
                    }
                }
                previous = Some(event);
            }
        }
        let mut semantic_ids = BTreeSet::new();
        for artifact in &self.semantic_artifacts {
            require_reference(&artifact.artifact_id, &artifacts, "semantic artifact")?;
            if !semantic_ids.insert(&artifact.artifact_id) {
                return Err(invalid(
                    "one artifact cannot have multiple semantic identities",
                ));
            }
            let capability = output_capability(&artifact.capability_id, &capabilities)?;
            validate_provenance(&artifact.provenance, capability, &providers, &artifacts)?;
            let correct_family = match artifact.kind {
                AudioSemanticArtifactKind::ObservedTranscript => {
                    artifact.capability_id == AUDIO_CAPABILITY_IDS_V1[5]
                }
                AudioSemanticArtifactKind::ReviewedLyrics => {
                    [AUDIO_CAPABILITY_IDS_V1[4], AUDIO_CAPABILITY_IDS_V1[6]]
                        .contains(&artifact.capability_id.as_str())
                }
                AudioSemanticArtifactKind::CandidateMidi => {
                    artifact.capability_id == AUDIO_CAPABILITY_IDS_V1[7]
                }
            };
            if !correct_family {
                return Err(invalid(
                    "semantic artifact is attached to the wrong capability family",
                ));
            }
            match (&artifact.kind, &artifact.authority) {
                (AudioSemanticArtifactKind::ReviewedLyrics, Some(authority)) => {
                    text(&authority.supplied_by, "review authority supplier")?;
                    require_reference(
                        &authority.provenance_artifact_id,
                        &artifacts,
                        "review authority provenance",
                    )?;
                    if authority.provenance_artifact_id == artifact.artifact_id {
                        return Err(invalid(
                            "review authority must refer to separate supplied provenance",
                        ));
                    }
                }
                (AudioSemanticArtifactKind::ReviewedLyrics, None) => {
                    return Err(invalid(
                        "reviewed lyrics require explicit supplied authority",
                    ));
                }
                (_, Some(_)) => {
                    return Err(invalid(
                        "review authority is only valid for reviewed lyrics",
                    ));
                }
                (_, None) => (),
            }
            if artifact.kind == AudioSemanticArtifactKind::CandidateMidi
                && artifact.provenance.class != AudioProvenanceClass::Probabilistic
            {
                return Err(invalid(
                    "candidate MIDI extraction must remain probabilistic",
                ));
            }
        }
        for excerpt in &self.excerpts {
            unique_id(&excerpt.id, &mut ids)?;
            require_reference(&excerpt.artifact_id, &artifacts, "excerpt artifact")?;
            excerpt.range.validate(&self.source)?;
            self.validate_scope(&excerpt.scope)?;
            references(
                &excerpt.evidence_artifact_ids,
                &artifacts,
                "excerpt evidence",
                true,
            )?;
        }
        validate_extensions(&self.extensions)
    }

    /// Encode validated evidence using the same canonical JSON rules as Pipeline v3.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        canonical_json_bytes(self)
    }

    fn validate_scope(&self, scope: &AudioScope) -> Result<()> {
        if scope.channels.is_empty()
            || scope
                .channels
                .iter()
                .any(|channel| *channel >= self.source.channels)
            || scope.channels.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "scope channels must be nonempty, unique, ascending, and within the source",
            ));
        }
        if scope.stem_id.as_deref() != self.source.stem.as_ref().map(|stem| stem.id.as_str()) {
            return Err(invalid(
                "scope stem identity must match the selected source",
            ));
        }
        Ok(())
    }

    fn validate_observation_value(&self, observation: &AudioObservation) -> Result<()> {
        let valid = match (&observation.kind, &observation.value) {
            (
                AudioObservationKind::SampleRate,
                AudioObservationValue::Quantity {
                    value,
                    unit: AudioUnit::Hertz,
                },
            ) => value.is_finite() && *value == f64::from(self.source.sample_rate_hz),
            (
                AudioObservationKind::ChannelCount,
                AudioObservationValue::Count {
                    value,
                    unit: AudioUnit::Channels,
                },
            ) => *value == u64::from(self.source.channels),
            (
                AudioObservationKind::Tempo,
                AudioObservationValue::Quantity {
                    value,
                    unit: AudioUnit::BeatsPerMinute,
                },
            ) => value.is_finite() && *value > 0.0 && *value <= 1000.0,
            (AudioObservationKind::Key, AudioObservationValue::Label { value }) => {
                text(value, "key label").is_ok()
            }
            _ => false,
        };
        if !valid {
            return Err(invalid(
                "observation kind, value, unit, or source identity is inconsistent",
            ));
        }
        let technical = matches!(
            observation.kind,
            AudioObservationKind::SampleRate | AudioObservationKind::ChannelCount
        );
        let expected = if technical {
            AUDIO_CAPABILITY_IDS_V1[0]
        } else {
            AUDIO_CAPABILITY_IDS_V1[3]
        };
        if observation.capability_id != expected {
            return Err(invalid(
                "observation is attached to the wrong capability family",
            ));
        }
        if technical != (observation.provenance.class == AudioProvenanceClass::Deterministic) {
            return Err(invalid(
                "technical values must be deterministic and musical estimates must be heuristic or probabilistic",
            ));
        }
        Ok(())
    }

    fn validate_capabilities<'a>(
        &'a self,
        providers: &BTreeMap<&str, &AudioProviderEvidence>,
        artifacts: &BTreeSet<&str>,
        diagnostics: &BTreeMap<&str, &AudioDiagnostic>,
    ) -> Result<BTreeMap<&'a str, &'a AudioCapabilityOutcome>> {
        if self.capabilities.is_empty() {
            return Err(invalid(
                "at least one explicit capability outcome is required",
            ));
        }
        let provider_ids = providers.keys().copied().collect();
        let diagnostic_ids = diagnostics.keys().copied().collect();
        let mut capabilities = BTreeMap::new();
        for capability in &self.capabilities {
            if !AUDIO_CAPABILITY_IDS_V1.contains(&capability.capability.id.as_str())
                || capability.capability.version != AUDIO_CAPABILITY_VERSION_V1
            {
                return Err(invalid("unknown audio capability id or version"));
            }
            if capabilities
                .insert(capability.capability.id.as_str(), capability)
                .is_some()
            {
                return Err(invalid("duplicate capability id"));
            }
            let complete = capability.status == AudioAnalysisStatus::Complete;
            references(
                &capability.provider_evidence_ids,
                &provider_ids,
                "capability provider",
                complete,
            )?;
            references(
                &capability.evidence_artifact_ids,
                artifacts,
                "capability evidence",
                complete,
            )?;
            references(
                &capability.diagnostic_ids,
                &diagnostic_ids,
                "capability diagnostic",
                !complete,
            )?;
            if complete {
                if capability
                    .diagnostic_ids
                    .iter()
                    .any(|id| diagnostics[id.as_str()].severity == AudioDiagnosticSeverity::Error)
                {
                    return Err(invalid("complete capability cannot have error diagnostics"));
                }
                if capability.provider_evidence_ids.iter().any(|id| {
                    matches!(
                        providers[id.as_str()].models,
                        AudioModelEvidence::Unavailable { .. }
                    )
                }) {
                    return Err(invalid(
                        "complete capability cannot require unavailable models",
                    ));
                }
            }
        }
        let all_complete = self
            .capabilities
            .iter()
            .all(|capability| capability.status == AudioAnalysisStatus::Complete);
        if (self.status == AudioAnalysisStatus::Complete) != all_complete {
            return Err(invalid(
                "complete analysis requires exactly all-complete capability outcomes",
            ));
        }
        if self.status == AudioAnalysisStatus::Complete
            && self
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == AudioDiagnosticSeverity::Error)
        {
            return Err(invalid(
                "complete analysis cannot contain error diagnostics",
            ));
        }
        if matches!(
            self.status,
            AudioAnalysisStatus::Unavailable | AudioAnalysisStatus::Unsupported
        ) && self
            .capabilities
            .iter()
            .any(|capability| capability.status != self.status)
        {
            return Err(invalid(
                "unavailable or unsupported analysis must have matching capability outcomes",
            ));
        }
        if matches!(
            self.status,
            AudioAnalysisStatus::Cancelled | AudioAnalysisStatus::Failed
        ) && !self
            .capabilities
            .iter()
            .any(|capability| capability.status == self.status)
        {
            return Err(invalid(
                "cancelled or failed analysis requires a matching capability outcome",
            ));
        }
        Ok(capabilities)
    }
}

fn validate_provenance(
    provenance: &AudioObservationProvenance,
    capability: &AudioCapabilityOutcome,
    providers: &BTreeMap<&str, &AudioProviderEvidence>,
    artifacts: &BTreeSet<&str>,
) -> Result<()> {
    if !capability
        .provider_evidence_ids
        .contains(&provenance.provider_evidence_id)
    {
        return Err(invalid(
            "observation provider must be declared by its capability",
        ));
    }
    let provider = providers
        .get(provenance.provider_evidence_id.as_str())
        .ok_or_else(|| invalid("unknown observation provider evidence"))?;
    if matches!(provider.models, AudioModelEvidence::Unavailable { .. }) {
        return Err(invalid(
            "observation cannot claim results from an unavailable model",
        ));
    }
    references(
        &provenance.evidence_artifact_ids,
        artifacts,
        "observation evidence",
        true,
    )?;
    match &provenance.confidence {
        AudioConfidence::Calibrated { score } | AudioConfidence::Uncalibrated { score } => {
            if !score.is_finite() || !(0.0..=1.0).contains(score) {
                return Err(invalid(
                    "confidence score must be finite and between zero and one",
                ));
            }
        }
        AudioConfidence::Unavailable { reason } => {
            text(reason, "confidence unavailability reason")?
        }
        AudioConfidence::NotApplicable {} => {
            if provenance.class != AudioProvenanceClass::Deterministic {
                return Err(invalid(
                    "only deterministic observations may declare confidence not applicable",
                ));
            }
        }
    }
    Ok(())
}

fn output_capability<'a>(
    id: &str,
    capabilities: &BTreeMap<&str, &'a AudioCapabilityOutcome>,
) -> Result<&'a AudioCapabilityOutcome> {
    let capability = capabilities
        .get(id)
        .copied()
        .ok_or_else(|| invalid("output references an unknown capability"))?;
    if matches!(
        capability.status,
        AudioAnalysisStatus::Unavailable | AudioAnalysisStatus::Unsupported
    ) {
        return Err(invalid(
            "unavailable or unsupported capabilities cannot declare observations or output artifacts",
        ));
    }
    Ok(capability)
}

fn validate_artifact(artifact: &AudioArtifactReference) -> Result<()> {
    token(&artifact.id, "artifact id")?;
    require_sha256(&artifact.sha256, "artifact sha256")?;
    if artifact.byte_size > AUDIO_MAX_SAFE_INTEGER {
        return Err(invalid(
            "artifact byte size exceeds interoperable integer range",
        ));
    }
    Ok(())
}

fn validate_components(
    components: &[AudioComponentEvidence],
    artifacts: &BTreeSet<&str>,
) -> Result<()> {
    bounded_items(components.len())?;
    let mut ids = BTreeSet::new();
    for component in components {
        unique_id(&component.id, &mut ids)?;
        text(&component.version, "component version")?;
        text(&component.revision, "component revision")?;
        require_sha256(&component.sha256, "component sha256")?;
        validate_license(&component.license, artifacts)?;
    }
    Ok(())
}

fn validate_license(license: &AudioLicenseEvidence, artifacts: &BTreeSet<&str>) -> Result<()> {
    match license {
        AudioLicenseEvidence::Recorded {
            statement,
            evidence_artifact_id,
        } => {
            text(statement, "license statement")?;
            require_reference(evidence_artifact_id, artifacts, "license evidence")
        }
        AudioLicenseEvidence::Unavailable { reason } => {
            text(reason, "license unavailability reason")
        }
    }
}

fn references(values: &[String], known: &BTreeSet<&str>, name: &str, required: bool) -> Result<()> {
    bounded_items(values.len())?;
    if required && values.is_empty() {
        return Err(invalid(format!("{name} cannot be empty")));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        require_reference(value, known, name)?;
        if !seen.insert(value) {
            return Err(invalid(format!("duplicate {name} reference")));
        }
    }
    Ok(())
}

fn require_reference(value: &str, known: &BTreeSet<&str>, name: &str) -> Result<()> {
    if !known.contains(value) {
        return Err(invalid(format!("unknown {name} reference: {value}")));
    }
    Ok(())
}

fn unique_id<'a>(value: &'a str, known: &mut BTreeSet<&'a str>) -> Result<()> {
    token(value, "local id")?;
    if !known.insert(value) {
        return Err(invalid("duplicate local id"));
    }
    Ok(())
}

fn token(value: &str, name: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        return Err(invalid(format!(
            "{name} must be a lowercase ASCII identifier of at most 256 bytes"
        )));
    }
    Ok(())
}

fn text(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{name} must be nonblank, control-free text of at most 4096 bytes"
        )));
    }
    Ok(())
}

fn bounded_items(size: usize) -> Result<()> {
    if size > AUDIO_MAX_COLLECTION_ITEMS {
        return Err(invalid("audio contract collection exceeds 10000 entries"));
    }
    Ok(())
}

fn validate_extensions(extensions: &BTreeMap<String, Value>) -> Result<()> {
    let mut stack = Vec::new();
    for (namespace, value) in extensions {
        if !namespace.contains('.')
            || namespace.len() > 256
            || namespace.split('.').any(|part| {
                part.is_empty()
                    || !part
                        .bytes()
                        .next()
                        .is_some_and(|byte| byte.is_ascii_lowercase())
                    || !part.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
        {
            return Err(invalid(
                "extension keys must be lowercase reverse-domain namespaces",
            ));
        }
        stack.push((value, 0_usize));
    }
    let mut nodes = 0;
    while let Some((value, depth)) = stack.pop() {
        nodes += 1;
        if nodes > AUDIO_MAX_COLLECTION_ITEMS || depth > AUDIO_MAX_EXTENSION_DEPTH {
            return Err(invalid(
                "extension size or nesting exceeds bounded v1 profile",
            ));
        }
        match value {
            Value::Array(values) => stack.extend(values.iter().map(|value| (value, depth + 1))),
            Value::Object(values) => stack.extend(values.values().map(|value| (value, depth + 1))),
            _ => (),
        }
    }
    Ok(())
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}
