//! Exact source-stream clocks and explicit temporal support boundaries.
//! Inspection is an observation; supported processing is a separate decision.

mod policy;
mod probe;
mod provenance;
mod rational;

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use policy::{assess_processing, validate_reconstruction};
pub use provenance::{
    ArtifactStreamRelation, ArtifactTimeBinding, SourceStreamWindow, TemporalArtifactIndex,
    bind_artifact,
};
pub use rational::RationalTime;

pub const TEMPORAL_INSPECTION_SCHEMA_V1: &str = "aniflow.temporal-inspection/v1";
pub const TEMPORAL_ARTIFACT_INDEX_SCHEMA_V1: &str = "aniflow.temporal-artifact-index/v1";
pub const TEMPORAL_POLICY_V1: &str = "aniflow.temporal-policy/cfr-zero-origin/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TemporalCode {
    InvalidRational,
    ArithmeticOverflow,
    MalformedProbe,
    DuplicateStream,
    MissingVideo,
    AmbiguousVideo,
    AmbiguousAudio,
    InvalidSelection,
    UnacknowledgedStream,
    MissingTimestamp,
    InvalidDuration,
    PresentationOrder,
    DecodeOrder,
    MissingTemporalEvidence,
    VariableFrameRate,
    NonzeroPresentationOffset,
    NegativePresentationOffset,
    StreamDurationMismatch,
    SynchronizationMismatch,
    FrameCountMismatch,
    TimelineMismatch,
    UnsupportedStream,
    ProbeLimit,
    SourceChanged,
    LegacyTemporalEvidence,
}

/// Stable refusal code retained inside public media errors and machine reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalDiagnostic {
    pub code: TemporalCode,
    pub stream_index: Option<u32>,
    pub message: String,
}

impl fmt::Display for TemporalDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for TemporalDiagnostic {}

pub(crate) fn diagnostic(
    code: TemporalCode,
    stream_index: Option<u32>,
    message: impl Into<String>,
) -> TemporalDiagnostic {
    TemporalDiagnostic {
        code,
        stream_index,
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaStreamKind {
    Video,
    Audio,
    Subtitle,
    Data,
    Attachment,
    CoverArt,
    Unknown,
}

/// Input-global indices, not relative video/audio ordinals. Discards are opt-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamSelection {
    pub video_stream: Option<u32>,
    pub audio_stream: Option<u32>,
    pub no_audio: bool,
    pub discard_streams: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamIdentity {
    pub index: u32,
    pub kind: MediaStreamKind,
    pub codec: Option<String>,
    pub time_base: Option<RationalTime>,
    pub start_pts: Option<i64>,
    pub duration_ticks: Option<i64>,
    pub declared_frame_rate: Option<RationalTime>,
    pub average_frame_rate: Option<RationalTime>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub pixel_format: Option<String>,
    pub sample_rate_hz: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedStreams {
    pub video: Option<u32>,
    pub audio: Option<u32>,
    pub explicitly_discarded: Vec<u32>,
    pub automatic_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeRange {
    pub start: RationalTime,
    pub end: RationalTime,
}

impl TimeRange {
    pub fn new(start: RationalTime, end: RationalTime) -> anyhow::Result<Self> {
        start.validate()?;
        end.validate()?;
        if end <= start {
            return Err(diagnostic(
                TemporalCode::InvalidDuration,
                None,
                "time range must have positive duration",
            )
            .into());
        }
        Ok(Self { start, end })
    }
    pub fn duration(self) -> anyhow::Result<RationalTime> {
        Self::new(self.start, self.end)?;
        self.end.checked_sub(self.start)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameTimestamp {
    pub ordinal: u64,
    pub stream_index: u32,
    pub presentation_ticks: i64,
    pub decode_ticks: Option<i64>,
    pub duration_ticks: i64,
    pub keyframe: bool,
    pub source_time: TimeRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketTimestamp {
    pub ordinal: u64,
    pub stream_index: u32,
    pub presentation_ticks: Option<i64>,
    pub decode_ticks: Option<i64>,
    pub duration_ticks: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cadence {
    Constant,
    Variable,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamTimeline {
    pub stream_index: u32,
    pub cadence: Cadence,
    pub frame_period: Option<RationalTime>,
    pub source_time: Option<TimeRange>,
    pub frames: Vec<FrameTimestamp>,
    pub packets: Vec<PacketTimestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalSupport {
    pub policy: String,
    pub supported: bool,
    pub diagnostics: Vec<TemporalDiagnostic>,
    pub frame_count: Option<u64>,
    pub video_frame_tolerance: Option<RationalTime>,
    pub audio_boundary_tolerance: Option<RationalTime>,
}

/// Self-contained inspection bound to original source bytes, not format duration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalInspection {
    pub schema: String,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub selection_intent: StreamSelection,
    pub streams: Vec<StreamIdentity>,
    pub selected: Option<SelectedStreams>,
    pub timelines: Vec<StreamTimeline>,
    pub diagnostics: Vec<TemporalDiagnostic>,
    pub processing: TemporalSupport,
}

impl TemporalInspection {
    #[must_use]
    pub fn stream(&self, index: u32) -> Option<&StreamIdentity> {
        self.streams.iter().find(|s| s.index == index)
    }
    #[must_use]
    pub fn timeline(&self, index: u32) -> Option<&StreamTimeline> {
        self.timelines.iter().find(|s| s.stream_index == index)
    }
    pub fn video_timeline(&self) -> anyhow::Result<&StreamTimeline> {
        let index = self
            .selected
            .as_ref()
            .and_then(|s| s.video)
            .ok_or_else(|| {
                diagnostic(
                    TemporalCode::MissingVideo,
                    None,
                    "select one moving-video stream",
                )
            })?;
        self.timeline(index).ok_or_else(|| {
            diagnostic(
                TemporalCode::MissingTemporalEvidence,
                Some(index),
                "selected video has no decoded temporal evidence",
            )
            .into()
        })
    }
    pub fn require_processing(&self) -> anyhow::Result<()> {
        let assessment = assess_processing(self)?;
        if let Some(reason) = assessment.diagnostics.first() {
            return Err(reason.clone().into());
        }
        Ok(())
    }
    pub fn sha256(&self) -> anyhow::Result<String> {
        use sha2::{Digest, Sha256};
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}

/// Bounded local inspection. Ambiguous selections remain inspectable refusals.
pub fn inspect(
    input: impl AsRef<Path>,
    selection: &StreamSelection,
) -> crate::Result<TemporalInspection> {
    probe::inspect(input.as_ref(), selection)
        .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))
}

/// Deterministic parser seam for offline, generator-first FFprobe fixtures.
/// `streams_json` is inventory; frame/packet JSON is per selected input-global stream.
/// This validates observations, not arbitrary media or claimed probe authenticity.
pub fn from_probe_documents(
    source_sha256: &str,
    source_size_bytes: u64,
    selection: &StreamSelection,
    streams_json: &serde_json::Value,
    observations: &[(u32, serde_json::Value, serde_json::Value)],
) -> crate::Result<TemporalInspection> {
    probe::from_documents(
        source_sha256,
        source_size_bytes,
        selection,
        streams_json,
        observations,
    )
    .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))
}

pub(crate) use probe::sha256_file;
