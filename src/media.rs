use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::temporal::{
    self, MediaStreamKind, RationalTime, StreamSelection, TemporalCode, TemporalInspection,
    diagnostic,
};

/// Compatibility summary plus the independently versioned exact temporal record.
/// Floating-point fields are presentation only; processing uses `temporal`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInspection {
    pub source: String,
    pub duration_seconds: f64,
    pub width: u64,
    pub height: u64,
    pub average_frame_rate: String,
    pub frames_per_second: f64,
    pub estimated_frame_count: u64,
    pub video_codec: String,
    pub pixel_format: Option<String>,
    pub has_audio: bool,
    pub audio_codec: Option<String>,
    pub has_subtitles: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<TemporalInspection>,
}

pub fn inspect(input: &Path) -> Result<MediaInspection> {
    inspect_with_selection(input, &StreamSelection::default())
}

pub fn inspect_with_selection(
    input: &Path,
    selection: &StreamSelection,
) -> Result<MediaInspection> {
    let canonical = input
        .canonicalize()
        .context("failed to resolve media input")?;
    let temporal = temporal::inspect(&canonical, selection).map_err(|e| {
        if let Some(d) = e.temporal_diagnostic() {
            anyhow::Error::new(d.clone())
        } else {
            anyhow::Error::new(e)
        }
    })?;
    if let Some(reason) = temporal.diagnostics.first() {
        return Err(reason.clone().into());
    }
    let selected = temporal
        .selected
        .as_ref()
        .context("temporal inspection has no selection")?;
    let index = selected.video.ok_or_else(|| {
        diagnostic(
            TemporalCode::MissingVideo,
            None,
            "source has no selected moving-video stream",
        )
    })?;
    let video = temporal
        .stream(index)
        .context("selected video identity is missing")?;
    let timeline = temporal.video_timeline()?;
    let duration = timeline
        .source_time
        .context("source video has no decoded interval")?
        .duration()?;
    // For VFR this is an informational average, never a reconstruction clock.
    let rate = timeline
        .frame_period
        .map(RationalTime::reciprocal)
        .transpose()?
        .or(video.average_frame_rate)
        .context("source frame rate is unavailable")?;
    let audio = selected.audio.and_then(|i| temporal.stream(i));
    Ok(MediaInspection {
        source: canonical.display().to_string(),
        duration_seconds: duration.as_seconds_f64(),
        width: video.width.unwrap_or_default(),
        height: video.height.unwrap_or_default(),
        average_frame_rate: video.average_frame_rate.unwrap_or(rate).to_string(),
        frames_per_second: rate.as_seconds_f64(),
        // The legacy field now contains the observed decoded count, not a rate estimate.
        estimated_frame_count: timeline.frames.len() as u64,
        video_codec: video.codec.clone().unwrap_or_else(|| "unknown".to_owned()),
        pixel_format: video.pixel_format.clone(),
        has_audio: audio.is_some(),
        audio_codec: audio.and_then(|s| s.codec.clone()),
        has_subtitles: temporal
            .streams
            .iter()
            .any(|s| s.kind == MediaStreamKind::Subtitle),
        temporal: Some(temporal),
    })
}

impl MediaInspection {
    pub(crate) fn exact(&self) -> Result<&TemporalInspection> {
        self.temporal.as_ref().ok_or_else(|| diagnostic(TemporalCode::LegacyTemporalEvidence, None, "legacy inspection has no exact temporal evidence; start a new run rather than upgrading old completion markers").into())
    }
    pub(crate) fn require_processing(&self) -> Result<()> {
        self.exact()?.require_processing()
    }
    pub(crate) fn exact_frame_rate(&self) -> Result<String> {
        self.require_processing()?;
        Ok(self
            .exact()?
            .video_timeline()?
            .frame_period
            .context("CFR period is missing")?
            .reciprocal()?
            .to_string())
    }
}

/// Legacy presentation helper; its result is never used for timeline decisions.
#[cfg(test)]
pub fn parse_frame_rate(value: &str) -> Result<f64> {
    let rate = RationalTime::parse(value)?;
    if rate <= RationalTime::ZERO {
        return Err(diagnostic(
            TemporalCode::InvalidRational,
            None,
            "frame rate must be positive",
        )
        .into());
    }
    Ok(rate.as_seconds_f64())
}

#[cfg(test)]
mod tests {
    use super::parse_frame_rate;
    #[test]
    fn parses_ntsc_frame_rate() {
        let actual = parse_frame_rate("30000/1001").expect("frame rate should parse");
        assert!((actual - 29.970_029_97).abs() < 0.000_001);
    }
}
