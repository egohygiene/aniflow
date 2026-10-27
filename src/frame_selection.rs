//! Read-only frame-range previews over declared, exact source-frame evidence.
//!
//! The existing RIFE frame index is an input adapter, not a requirement to use
//! RIFE. Planning checks the declared mapping and clock but never observes PNG
//! bytes, creates an import stage, or grants a repair provider authority.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::provider::canonical_sha256;
use crate::temporal::TimeRange;
use crate::video_interpolation::{RifeFrameIndex, RIFE_FRAME_INDEX_SCHEMA_V1};
use crate::{Error, ErrorCategory, Result};

pub const FRAME_SELECTION_PLAN_SCHEMA_V1: &str = "aniflow.frame-selection-plan/v1";
pub const MAXIMUM_FRAME_SELECTION_RANGES: usize = 1024;

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Input, message)
}

/// One-based, inclusive frame positions, independent of filename numbering.
/// For example, `1:299` selects zero-based ordinals `[0, 299)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InclusiveFrameRange {
    pub first: u64,
    pub last: u64,
}

impl InclusiveFrameRange {
    fn validate(self) -> Result<()> {
        if self.first == 0 || self.last < self.first {
            return Err(invalid("frame ranges require 1 <= FIRST <= LAST"));
        }
        Ok(())
    }
}

impl FromStr for InclusiveFrameRange {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let (first, last) = value.split_once(':').ok_or_else(|| {
            invalid("frame range must be FIRST:LAST using one-based inclusive integers")
        })?;
        if [first, last].iter().any(|part| {
            part.is_empty() || part.len() > 20 || !part.bytes().all(|byte| byte.is_ascii_digit())
        }) {
            return Err(invalid("frame range must contain only two unsigned decimal integers"));
        }
        let range = Self {
            first: first.parse().map_err(|_| invalid("FIRST is outside the supported integer range"))?,
            last: last.parse().map_err(|_| invalid("LAST is outside the supported integer range"))?,
        };
        range.validate()?;
        Ok(range)
    }
}

/// An ordered selection on the original frame grid. The time interval is
/// half-open, from the first selected frame's start to the last frame's end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedFrameRange {
    pub first_frame: u64,
    pub last_frame: u64,
    pub start_ordinal: u64,
    pub end_ordinal_exclusive: u64,
    pub frame_count: u64,
    pub first_filename: String,
    pub last_filename: String,
    pub source_time: TimeRange,
}

/// This preview checks index declarations; it does not observe media files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameSelectionEvidence {
    IndexDeclarationsOnly,
}

/// Frame content does not identify the tool that historically produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalFrameProcessing {
    Unknown,
}

/// A reproducible preview, not an accepted import, execution plan or checkpoint.
/// Counts describe selection intent; no output or untouched bytes are verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSelectionPlan {
    pub schema: String,
    pub input_schema: String,
    pub frame_index_sha256: String,
    pub source_sha256: String,
    pub source_frame_count: u64,
    pub selected_frame_count: u64,
    pub unselected_frame_count: u64,
    pub evidence_basis: FrameSelectionEvidence,
    pub historical_processing: HistoricalFrameProcessing,
    pub ranges: Vec<SelectedFrameRange>,
}

/// Preview one or more inclusive frame ranges against an existing exact index.
///
/// Ranges are sorted by source position; duplicates and overlap are refused.
/// Adjacent ranges remain separate. Gaps are unselected frames, not missing
/// mappings: the input index must still describe the complete source grid.
/// Only the index's existing CFR, zero-origin, RGB8/SDR profile is admitted.
/// This pure operation does not read source/frame paths or invoke any tool.
pub fn plan_from_rife_index(
    index: &RifeFrameIndex,
    ranges: &[InclusiveFrameRange],
) -> Result<FrameSelectionPlan> {
    if ranges.is_empty() || ranges.len() > MAXIMUM_FRAME_SELECTION_RANGES {
        return Err(invalid("select between 1 and 1024 explicit frame ranges"));
    }
    index.validate()?;
    let timeline = index.temporal_inspection.video_timeline()
        .map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
    let source_frame_count = u64::try_from(index.frames.len())
        .map_err(|_| invalid("frame count exceeds the supported integer range"))?;
    let mut ordered = ranges.to_vec();
    ordered.sort_by_key(|range| (range.first, range.last));
    let mut selected = Vec::with_capacity(ordered.len());
    let mut previous_last = 0;
    let mut selected_frame_count = 0_u64;
    for range in ordered {
        range.validate()?;
        if range.last > source_frame_count {
            return Err(invalid("frame range exceeds the complete declared source frame count"));
        }
        if range.first <= previous_last {
            return Err(invalid("frame ranges must not overlap or repeat selected frames"));
        }
        let start_ordinal = range.first - 1;
        // A one-based inclusive LAST is already the zero-based exclusive end.
        // Avoid LAST + 1, including for externally constructed u64::MAX ranges.
        let end_ordinal_exclusive = range.last;
        let start = usize::try_from(start_ordinal)
            .map_err(|_| invalid("start frame cannot be represented on this platform"))?;
        let last = usize::try_from(range.last - 1)
            .map_err(|_| invalid("last frame cannot be represented on this platform"))?;
        let first_frame = timeline.frames.get(start)
            .ok_or_else(|| invalid("selected first frame has no exact source-time mapping"))?;
        let last_frame = timeline.frames.get(last)
            .ok_or_else(|| invalid("selected last frame has no exact source-time mapping"))?;
        let first_file = index.frames.get(start)
            .ok_or_else(|| invalid("selected first frame has no filename mapping"))?;
        let last_file = index.frames.get(last)
            .ok_or_else(|| invalid("selected last frame has no filename mapping"))?;
        let frame_count = end_ordinal_exclusive - start_ordinal;
        selected_frame_count = selected_frame_count.checked_add(frame_count)
            .ok_or_else(|| invalid("selected frame count overflow"))?;
        selected.push(SelectedFrameRange {
            first_frame: range.first,
            last_frame: range.last,
            start_ordinal,
            end_ordinal_exclusive,
            frame_count,
            first_filename: first_file.filename.clone(),
            last_filename: last_file.filename.clone(),
            source_time: TimeRange::new(first_frame.source_time.start, last_frame.source_time.end)
                .map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?,
        });
        previous_last = range.last;
    }
    Ok(FrameSelectionPlan {
        schema: FRAME_SELECTION_PLAN_SCHEMA_V1.to_owned(),
        input_schema: RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(),
        frame_index_sha256: canonical_sha256(index)?,
        source_sha256: index.temporal_inspection.source_sha256.clone(),
        source_frame_count,
        selected_frame_count,
        unselected_frame_count: source_frame_count - selected_frame_count,
        evidence_basis: FrameSelectionEvidence::IndexDeclarationsOnly,
        historical_processing: HistoricalFrameProcessing::Unknown,
        ranges: selected,
    })
}
