//! Narrow complete-AAC decode evidence, distinct from presentation timestamps.
//! FFmpeg n8.0.1 fftools/ffmpeg_dec.c sets audio duration to nb_samples;
//! a short final MP4 packet without discard_padding still decodes a full block.
use super::{AudioFacts, Result, Value, array, integer, media_error, refused};
use crate::temporal::{RationalTime, TemporalInspection, TimeRange};
use serde::{Deserialize, Serialize};

const SCOPE: &str = "complete_decoded_pcm_including_nonpresented_tail";
/// Optional extension only for the observed 48 kHz AAC-LC terminal-block profile.
/// Existing exact-grid receipts omit it and retain their original meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioDecodeTail {
    pub schema: String,
    pub metric_scope: String,
    pub presented_sample_frames: u64,
    pub decoded_sample_frames: u64,
    pub nonpresented_tail_sample_frames: u32,
    pub decoded_block_count: u64,
    pub final_presented_sample_frames: u32,
    pub priming_sample_frames: u32,
    pub decoded_extent: TimeRange,
}
impl AudioDecodeTail {
    pub(crate) fn validate(&self, presented: TimeRange) -> Result<u64> {
        let total = self.decoded_block_count.checked_mul(1024);
        let head = self
            .decoded_block_count
            .checked_sub(1)
            .and_then(|n| n.checked_mul(1024));
        let expected =
            head.and_then(|n| n.checked_add(u64::from(self.final_presented_sample_frames)));
        let extent_samples = presented
            .duration()
            .map_err(media_error)?
            .checked_mul(48_000)
            .map_err(media_error)?;
        let decoded_end = i64::try_from(self.decoded_sample_frames)
            .ok()
            .and_then(|n| RationalTime::new(n, 48_000).ok());
        if self.schema != "aniflow.aac-decoded-tail/v1"
            || self.metric_scope != SCOPE
            || !(1..=1_000_000).contains(&self.decoded_block_count)
            || !(1..1024).contains(&self.final_presented_sample_frames)
            || self.priming_sample_frames != 1024
            || self.nonpresented_tail_sample_frames != 1024 - self.final_presented_sample_frames
            || total != Some(self.decoded_sample_frames)
            || expected != Some(self.presented_sample_frames)
            || presented.start != RationalTime::ZERO
            || extent_samples.denominator() != 1
            || u64::try_from(extent_samples.numerator()).ok() != Some(self.presented_sample_frames)
            || self.decoded_extent.start != RationalTime::ZERO
            || Some(self.decoded_extent.end) != decoded_end
        {
            return Err(refused(
                "AAC decoded-tail evidence does not exactly bind its complete PCM and presented extents",
            ));
        }
        Ok(self.decoded_sample_frames)
    }
}

/// Every native frame and packet is compared before this compact summary is
/// authored. This does not change common temporal bytes or packet identity.
pub(super) fn observe(
    inspection: &TemporalInspection,
    frames: &Value,
    facts: &AudioFacts,
    index: u32,
) -> Result<Option<AudioDecodeTail>> {
    let timeline = inspection
        .timeline(index)
        .ok_or_else(|| refused("AAC decoded timeline missing"))?;
    let range = timeline
        .source_time
        .ok_or_else(|| refused("AAC presented extent missing"))?;
    let native = array(frames, "frames")?;
    if native.is_empty() || native.len() != timeline.frames.len() {
        return Err(refused("AAC native/sample timeline count differs"));
    }
    let mut decoded = 0u64;
    let mut presented = 0u64;
    let mut samples = Vec::with_capacity(native.len());
    for (frame, time) in native.iter().zip(&timeline.frames) {
        let count = u32::try_from(integer(frame, "nb_samples")?)
            .map_err(|_| refused("AAC nb_samples exceeds bounded positive count"))?;
        let duration = time
            .source_time
            .duration()
            .map_err(media_error)?
            .checked_mul(u64::from(facts.sample_rate))
            .map_err(media_error)?;
        if count == 0 || count > 1024 || duration.denominator() != 1 || duration.numerator() <= 0 {
            return Err(refused(
                "AAC decoded frame has unsupported sample count or sample clock",
            ));
        }
        let shown = duration.numerator() as u64;
        decoded = decoded
            .checked_add(u64::from(count))
            .ok_or_else(|| refused("AAC decoded sample count overflow"))?;
        presented = presented
            .checked_add(shown)
            .ok_or_else(|| refused("AAC presented sample count overflow"))?;
        samples.push((count, shown));
    }
    if samples
        .iter()
        .all(|(count, shown)| u64::from(*count) == *shown)
    {
        return Ok(None);
    }
    if facts.sample_rate != 48_000
        || range.start != RationalTime::ZERO
        || native.len() > 1_000_000
        || facts.packets.len() != native.len() + 1
        || samples.iter().any(|(n, _)| *n != 1024)
        || samples[..samples.len() - 1].iter().any(|(_, n)| *n != 1024)
        || !(1..1024).contains(&samples.last().unwrap().1)
    {
        return Err(refused(
            "only complete contiguous AAC-LC blocks with one observed short final presentation are admitted",
        ));
    }
    let selected = inspection
        .stream(index)
        .ok_or_else(|| refused("AAC selected stream declaration missing"))?;
    let time_base = selected
        .time_base
        .ok_or_else(|| refused("AAC selected stream time base missing"))?;
    let start = selected
        .start_pts
        .ok_or_else(|| refused("AAC declared presentation origin missing"))?;
    let duration = selected
        .duration_ticks
        .ok_or_else(|| refused("AAC declared presentation duration missing"))?;
    if RationalTime::ticks(start, time_base).map_err(media_error)? != range.start
        || RationalTime::ticks(duration, time_base).map_err(media_error)?
            != range.duration().map_err(media_error)?
    {
        return Err(refused(
            "AAC tail presentation must exactly equal the declared stream origin and duration",
        ));
    }
    let priming = &facts.packets[0];
    let block = RationalTime::new(1024, 48_000).map_err(media_error)?;
    let negative = RationalTime::new(-1024, 48_000).map_err(media_error)?;
    if priming.pts != negative
        || priming.dts != negative
        || priming.duration != block
        || !priming.skip.as_ref().is_some_and(|s| {
            s.skip_samples == 1024
                && s.discard_padding == 0
                && s.skip_reason == 0
                && s.discard_reason == 0
        })
    {
        return Err(refused(
            "AAC tail profile requires exactly observed one-block priming without discard padding",
        ));
    }
    let mut end = range.start;
    for ((time, packet), (_, shown)) in timeline
        .frames
        .iter()
        .zip(&facts.packets[1..])
        .zip(&samples)
    {
        let duration = time.source_time.duration().map_err(media_error)?;
        if time.source_time.start != end
            || packet.pts != end
            || packet.dts != end
            || packet.duration != duration
            || packet.skip.is_some()
            || *shown == 0
        {
            return Err(refused(
                "AAC tail profile refuses packet/frame gaps, edits, side data or clock differences",
            ));
        }
        end = time.source_time.end;
    }
    if end != range.end {
        return Err(refused(
            "AAC tail profile does not cover complete presented extent",
        ));
    }
    let value = AudioDecodeTail {
        schema: "aniflow.aac-decoded-tail/v1".into(),
        metric_scope: SCOPE.into(),
        presented_sample_frames: presented,
        decoded_sample_frames: decoded,
        nonpresented_tail_sample_frames: 1024 - samples.last().unwrap().1 as u32,
        decoded_block_count: native.len() as u64,
        final_presented_sample_frames: samples.last().unwrap().1 as u32,
        priming_sample_frames: 1024,
        decoded_extent: TimeRange::new(
            RationalTime::ZERO,
            RationalTime::new(
                i64::try_from(decoded)
                    .map_err(|_| refused("AAC decoded count overflows rational clock"))?,
                48_000,
            )
            .map_err(media_error)?,
        )
        .map_err(media_error)?,
    };
    value.validate(range)?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (AudioDecodeTail, TimeRange) {
        let presented =
            TimeRange::new(RationalTime::ZERO, RationalTime::new(2, 1).unwrap()).unwrap();
        (
            AudioDecodeTail {
                schema: "aniflow.aac-decoded-tail/v1".into(),
                metric_scope: SCOPE.into(),
                presented_sample_frames: 96000,
                decoded_sample_frames: 96256,
                nonpresented_tail_sample_frames: 256,
                decoded_block_count: 94,
                final_presented_sample_frames: 768,
                priming_sample_frames: 1024,
                decoded_extent: TimeRange::new(
                    RationalTime::ZERO,
                    RationalTime::new(96256, 48000).unwrap(),
                )
                .unwrap(),
            },
            presented,
        )
    }
    #[test]
    fn full_decoder_extent_is_distinct_from_presentation() {
        let (value, presented) = fixture();
        assert_eq!(value.validate(presented).unwrap(), 96256);
        assert_ne!(value.decoded_extent, presented);
    }
    #[test]
    fn malformed_or_overflowing_tail_summary_refuses() {
        let (value, range) = fixture();
        for field in [
            "presented_sample_frames",
            "decoded_sample_frames",
            "nonpresented_tail_sample_frames",
            "decoded_block_count",
            "final_presented_sample_frames",
            "priming_sample_frames",
        ] {
            let mut json = serde_json::to_value(&value).unwrap();
            json[field] = serde_json::json!(0);
            let changed: AudioDecodeTail = serde_json::from_value(json).unwrap();
            assert!(changed.validate(range).is_err(), "{field}");
        }
        let mut changed = value;
        changed.decoded_block_count = u64::MAX;
        assert!(changed.validate(range).is_err());
    }
    #[test]
    fn presented_scope_and_shifted_or_shortened_extents_refuse() {
        let (mut value, range) = fixture();
        value.metric_scope = "presented_audio".into();
        assert!(value.validate(range).is_err());
        let (mut value, range) = fixture();
        value.decoded_extent = range;
        assert!(value.validate(range).is_err());
        let (value, _) = fixture();
        let shifted = TimeRange::new(
            RationalTime::new(1, 48000).unwrap(),
            RationalTime::new(96001, 48000).unwrap(),
        )
        .unwrap();
        assert!(value.validate(shifted).is_err());
    }
}
