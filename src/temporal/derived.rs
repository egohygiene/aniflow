//! Explicit RIFE 2x clock validation. Original inspection is never rewritten.
use super::{MediaStreamKind, RationalTime, TemporalInspection, TimeRange};
use crate::{Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedVideoClock {
    pub schema: String,
    pub source_sha256: String,
    pub rife_plan_sha256: String,
    pub source_frames: u64,
    pub source_period: RationalTime,
    pub output_frames: u64,
    pub output_period: RationalTime,
    pub duration: RationalTime,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn mapped(error: anyhow::Error) -> Error {
    Error::from_anyhow(ErrorCategory::Media, error)
}
impl DerivedVideoClock {
    pub fn validate(&self) -> Result<()> {
        crate::provider::require_sha256(&self.source_sha256, "derived clock source")?;
        crate::provider::require_sha256(&self.rife_plan_sha256, "derived clock RIFE plan")?;
        for value in [self.source_period, self.output_period, self.duration] {
            value.validate().map_err(mapped)?;
        }
        if self.schema != "aniflow.rife-derived-clock/v1"
            || !(1..=50_000).contains(&self.source_frames)
            || self.source_period <= RationalTime::ZERO
            || self.output_frames != self.source_frames * 2
            || self.output_period.checked_mul(2).map_err(mapped)? != self.source_period
            || self
                .source_period
                .checked_mul(self.source_frames)
                .map_err(mapped)?
                != self.duration
            || self
                .output_period
                .checked_mul(self.output_frames)
                .map_err(mapped)?
                != self.duration
        {
            return Err(invalid(
                "derived clock must declare exact duration-preserving 2x RIFE cadence within assembly bounds",
            ));
        }
        Ok(())
    }
    pub fn validate_source(&self, source: &TemporalInspection) -> Result<()> {
        self.validate()?;
        source.require_processing().map_err(mapped)?;
        if source.source_sha256 != self.source_sha256 {
            return Err(invalid("derived clock names different original bytes"));
        }
        grid(
            source,
            self.source_frames,
            self.source_period,
            self.duration,
        )
    }
}
fn grid(
    inspection: &TemporalInspection,
    count: u64,
    period: RationalTime,
    duration: RationalTime,
) -> Result<()> {
    let timeline = inspection.video_timeline().map_err(mapped)?;
    if timeline.frames.len() as u64 != count
        || timeline.frame_period != Some(period)
        || timeline.source_time
            != Some(TimeRange::new(RationalTime::ZERO, duration).map_err(mapped)?)
    {
        return Err(invalid(
            "derived clock count, cadence, origin or full duration differs",
        ));
    }
    for (ordinal, frame) in timeline.frames.iter().enumerate() {
        if frame.source_time
            != TimeRange::new(
                period.checked_mul(ordinal as u64).map_err(mapped)?,
                period.checked_mul(ordinal as u64 + 1).map_err(mapped)?,
            )
            .map_err(mapped)?
        {
            return Err(invalid(
                "derived clock has a missing, shifted or overlapping frame interval",
            ));
        }
    }
    Ok(())
}
pub fn validate_derived_reconstruction(
    source: &TemporalInspection,
    output: &TemporalInspection,
    clock: &DerivedVideoClock,
) -> Result<()> {
    clock.validate_source(source)?;
    output.require_processing().map_err(mapped)?;
    let selected = source
        .selected
        .as_ref()
        .ok_or_else(|| invalid("derived clock source selection unavailable"))?;
    if output
        .streams
        .iter()
        .any(|s| !matches!(s.kind, MediaStreamKind::Video | MediaStreamKind::Audio))
        || output
            .streams
            .iter()
            .filter(|s| s.kind == MediaStreamKind::Video)
            .count()
            != 1
        || output
            .streams
            .iter()
            .filter(|s| s.kind == MediaStreamKind::Audio)
            .count()
            != usize::from(selected.audio.is_some())
    {
        return Err(invalid(
            "derived assembly changed selected audio/video cardinality",
        ));
    }
    grid(
        output,
        clock.output_frames,
        clock.output_period,
        clock.duration,
    )?;
    let audio = |inspection: &TemporalInspection| {
        inspection
            .selected
            .as_ref()
            .and_then(|s| s.audio)
            .and_then(|i| inspection.timeline(i))
            .and_then(|t| t.source_time)
    };
    if audio(source) != audio(output) {
        return Err(invalid(
            "derived cadence cannot shift, trim or pad original audio extent",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observations() -> (TemporalInspection, TemporalInspection, DerivedVideoClock) {
        let doc: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../tests/fixtures/temporal/synchronized-audio-video.json"
        ))
        .unwrap();
        let selection = serde_json::from_value(doc["selection"].clone()).unwrap();
        let observations = doc["observations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                (
                    o["stream_index"].as_u64().unwrap() as u32,
                    o["frames"].clone(),
                    o["packets"].clone(),
                )
            })
            .collect::<Vec<_>>();
        let source = super::super::from_probe_documents(
            &"a".repeat(64),
            128,
            &selection,
            &doc["inventory"],
            &observations,
        )
        .unwrap();
        let mut inventory = doc["inventory"].clone();
        inventory["streams"][0]["time_base"] = serde_json::json!("1/48000");
        inventory["streams"][0]["duration_ts"] = serde_json::json!(8008);
        inventory["streams"][0]["r_frame_rate"] = serde_json::json!("48000/1001");
        inventory["streams"][0]["avg_frame_rate"] = serde_json::json!("48000/1001");
        let frames=(0..8).map(|i|serde_json::json!({"stream_index":7,"pts":i*1001,"pkt_dts":i*1001,"pkt_duration":1001,"key_frame":1})).collect::<Vec<_>>();
        let packets = (0..8)
            .map(
                |i| serde_json::json!({"stream_index":7,"pts":i*1001,"dts":i*1001,"duration":1001}),
            )
            .collect::<Vec<_>>();
        let mut derived = observations;
        derived[0] = (
            7,
            serde_json::json!({"frames":frames}),
            serde_json::json!({"packets":packets}),
        );
        let output = super::super::from_probe_documents(
            &"b".repeat(64),
            256,
            &selection,
            &inventory,
            &derived,
        )
        .unwrap();
        let clock = DerivedVideoClock {
            schema: "aniflow.rife-derived-clock/v1".into(),
            source_sha256: source.source_sha256.clone(),
            rife_plan_sha256: "c".repeat(64),
            source_frames: 4,
            source_period: RationalTime::new(1001, 24000).unwrap(),
            output_frames: 8,
            output_period: RationalTime::new(1001, 48000).unwrap(),
            duration: RationalTime::new(1001, 6000).unwrap(),
        };
        (source, output, clock)
    }
    #[test]
    fn two_x_output_uses_separate_exact_grid_without_rewriting_source_or_audio() {
        let (source, output, clock) = observations();
        let original = source.clone();
        assert!(super::super::validate_reconstruction(&source, &output, false).is_err());
        validate_derived_reconstruction(&source, &output, &clock).unwrap();
        assert_eq!(source, original);
        assert_eq!(output.timeline(8).unwrap(), source.timeline(8).unwrap());
    }
    #[test]
    fn missing_frames_shifted_audio_and_wrong_duration_cannot_claim_the_same_clock() {
        let (source, mut output, clock) = observations();
        output
            .timelines
            .iter_mut()
            .find(|t| t.stream_index == 7)
            .unwrap()
            .frames
            .pop();
        assert!(validate_derived_reconstruction(&source, &output, &clock).is_err());
        let (_, mut output, _) = observations();
        output
            .timelines
            .iter_mut()
            .find(|t| t.stream_index == 8)
            .unwrap()
            .source_time
            .as_mut()
            .unwrap()
            .end = RationalTime::new(1, 5).unwrap();
        assert!(validate_derived_reconstruction(&source, &output, &clock).is_err());
        let mut wrong = clock;
        wrong.output_period = wrong.source_period;
        assert!(wrong.validate().is_err());
    }
}
