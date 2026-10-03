//! Native-sample measurements over the independently validated PCM payload.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::audio_analysis::AudioFrameRange;
use crate::audio_inspection::wav::PcmWave;
use crate::audio_inspection::{AUDIO_INSPECTION_MAXIMUM_BYTES, NativeSampleFormat};
use crate::{Error, ErrorCategory, Result};

pub(super) struct PcmMeasurements {
    pub channels: Vec<ChannelMeasurements>,
}

pub(super) struct ChannelMeasurements {
    pub peak: u32,
    pub sum_squares: u128,
    pub sample_peak_ratio: f64,
    pub rms_ratio: f64,
    pub silence: Vec<AudioFrameRange>,
    pub clipping: Vec<AudioFrameRange>,
}

pub(super) struct NativePcmMeasurements {
    pub channels: Vec<NativeChannelMeasurements>,
}

pub(super) struct NativeChannelMeasurements {
    pub sample_peak_ratio: f64,
    pub rms_ratio: f64,
    pub silence: Vec<AudioFrameRange>,
    pub clipping: Vec<AudioFrameRange>,
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}

struct RegionBuilder {
    start: Option<u64>,
    regions: Vec<AudioFrameRange>,
    minimum_frames: u64,
}

impl RegionBuilder {
    fn update(&mut self, frame: u64, active: bool) -> Result<()> {
        if active {
            self.start.get_or_insert(frame);
        } else if let Some(start) = self.start.take() {
            if frame - start >= self.minimum_frames {
                if self.regions.len() >= 10_000 {
                    return Err(invalid(
                        "signal_region_limit: more than 10000 regions in one channel",
                    ));
                }
                self.regions.push(AudioFrameRange { start, end: frame });
            }
        }
        Ok(())
    }
}

pub(super) fn measure(
    path: &Path,
    wave: &PcmWave,
    silence_threshold: u16,
    minimum_silence_milliseconds: u32,
    clipping_threshold: u16,
) -> Result<PcmMeasurements> {
    if wave.sample_format != NativeSampleFormat::Pcm16 {
        return Err(invalid(
            "unsupported_audio: legacy signal measurements require PCM16",
        ));
    }
    let minimum_silence_frames =
        (u64::from(minimum_silence_milliseconds) * u64::from(wave.sample_rate_hz)).div_ceil(1000);
    let mut peaks = vec![0_u32; usize::from(wave.channels)];
    let mut squares = vec![0_u128; usize::from(wave.channels)];
    let mut silence: Vec<_> = (0..wave.channels)
        .map(|_| RegionBuilder {
            start: None,
            regions: Vec::new(),
            minimum_frames: minimum_silence_frames,
        })
        .collect();
    let mut clipping: Vec<_> = (0..wave.channels)
        .map(|_| RegionBuilder {
            start: None,
            regions: Vec::new(),
            minimum_frames: 1,
        })
        .collect();
    let mut input = File::open(path).map_err(|_| invalid("signal source snapshot unavailable"))?;
    input
        .seek(SeekFrom::Start(wave.data_offset))
        .map_err(|_| invalid("cannot read validated PCM offset"))?;
    let frame_bytes = usize::from(wave.channels) * 2;
    let mut buffer = vec![0_u8; 65536 - 65536 % frame_bytes];
    let mut remaining = wave.data_bytes;
    let mut frame = 0_u64;
    while remaining > 0 {
        let amount =
            usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded input block");
        input
            .read_exact(&mut buffer[..amount])
            .map_err(|_| invalid("signal source snapshot was truncated"))?;
        for values in buffer[..amount].chunks_exact(frame_bytes) {
            for channel in 0..usize::from(wave.channels) {
                let sample = i16::from_le_bytes([values[channel * 2], values[channel * 2 + 1]]);
                let magnitude = u32::from(sample.unsigned_abs());
                peaks[channel] = peaks[channel].max(magnitude);
                squares[channel] += u128::from(magnitude) * u128::from(magnitude);
                silence[channel].update(frame, magnitude <= u32::from(silence_threshold))?;
                clipping[channel].update(frame, magnitude >= u32::from(clipping_threshold))?;
            }
            frame += 1;
        }
        remaining -= amount as u64;
    }
    if frame != wave.frame_count {
        return Err(invalid(
            "signal frame count differs from technical evidence",
        ));
    }
    let mut channels = Vec::new();
    for (channel, (mut silence, mut clipping)) in silence.into_iter().zip(clipping).enumerate() {
        silence.update(frame, false)?;
        clipping.update(frame, false)?;
        channels.push(ChannelMeasurements {
            peak: peaks[channel],
            sum_squares: squares[channel],
            sample_peak_ratio: f64::from(peaks[channel]) / 32768.0,
            rms_ratio: ((squares[channel] as f64 / wave.frame_count as f64).sqrt()) / 32768.0,
            silence: silence.regions,
            clipping: clipping.regions,
        });
    }
    Ok(PcmMeasurements { channels })
}

/// Neumaier summation retains small squared samples alongside large peaks.
#[derive(Clone, Copy, Default)]
struct CompensatedSum {
    sum: f64,
    correction: f64,
}

impl CompensatedSum {
    fn add(&mut self, value: f64) {
        let total = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - total) + value
        } else {
            (value - total) + self.sum
        };
        self.sum = total;
    }

    fn total(self) -> f64 {
        self.sum + self.correction
    }
}

fn native_sample(bytes: &[u8], sample_format: NativeSampleFormat) -> Result<f64> {
    let sample = match sample_format {
        NativeSampleFormat::Pcm16 => {
            f64::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32768.0
        }
        NativeSampleFormat::Pcm24 => {
            let sign = if bytes[2] & 0x80 == 0 { 0 } else { 0xff };
            f64::from(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], sign])) / 8388608.0
        }
        NativeSampleFormat::Float32 => f64::from(f32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ])),
    };
    if !sample.is_finite() {
        return Err(invalid(
            "invalid_audio: float32 samples must be finite; NaN and infinity are refused",
        ));
    }
    Ok(sample)
}

/// Measure packed PCM24 and IEEE float32 without a PCM16 conversion or clipping.
pub(super) fn measure_native(
    path: &Path,
    wave: &PcmWave,
    silence_threshold_ratio: f64,
    minimum_silence_milliseconds: u32,
    clipping_threshold_ratio: f64,
) -> Result<NativePcmMeasurements> {
    if !silence_threshold_ratio.is_finite()
        || !(0.0..=1.0).contains(&silence_threshold_ratio)
        || !clipping_threshold_ratio.is_finite()
        || clipping_threshold_ratio <= 0.0
        || clipping_threshold_ratio > 1.0
    {
        return Err(invalid("invalid native signal threshold ratios"));
    }
    let bytes_per_sample = usize::from(wave.sample_format.bytes_per_sample());
    let frame_bytes = usize::from(wave.channels) * bytes_per_sample;
    if !matches!(wave.channels, 1 | 2)
        || !(8000..=192000).contains(&wave.sample_rate_hz)
        || wave.frame_count == 0
        || wave.frame_count > u64::from(wave.sample_rate_hz) * 600
        || wave.data_bytes > AUDIO_INSPECTION_MAXIMUM_BYTES
        || wave.frame_count.checked_mul(frame_bytes as u64) != Some(wave.data_bytes)
    {
        return Err(invalid("invalid bounded native PCM frame geometry"));
    }
    let end = wave
        .data_offset
        .checked_add(wave.data_bytes)
        .ok_or_else(|| invalid("invalid native PCM payload bounds"))?;
    let mut input = File::open(path).map_err(|_| invalid("signal source snapshot unavailable"))?;
    if input
        .metadata()
        .map_err(|_| invalid("cannot inspect signal source snapshot"))?
        .len()
        < end
    {
        return Err(invalid("signal source snapshot was truncated"));
    }
    input
        .seek(SeekFrom::Start(wave.data_offset))
        .map_err(|_| invalid("cannot read validated PCM offset"))?;
    let minimum_silence_frames =
        (u64::from(minimum_silence_milliseconds) * u64::from(wave.sample_rate_hz)).div_ceil(1000);
    let mut peaks = vec![0.0_f64; usize::from(wave.channels)];
    let mut squares = vec![CompensatedSum::default(); usize::from(wave.channels)];
    let mut silence: Vec<_> = (0..wave.channels)
        .map(|_| RegionBuilder {
            start: None,
            regions: Vec::new(),
            minimum_frames: minimum_silence_frames,
        })
        .collect();
    let mut clipping: Vec<_> = (0..wave.channels)
        .map(|_| RegionBuilder {
            start: None,
            regions: Vec::new(),
            minimum_frames: 1,
        })
        .collect();
    let mut buffer = vec![0_u8; 65536 - 65536 % frame_bytes];
    let mut remaining = wave.data_bytes;
    let mut frame = 0_u64;
    while remaining > 0 {
        let amount =
            usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded input block");
        input
            .read_exact(&mut buffer[..amount])
            .map_err(|_| invalid("signal source snapshot was truncated"))?;
        for values in buffer[..amount].chunks_exact(frame_bytes) {
            for (channel, bytes) in values.chunks_exact(bytes_per_sample).enumerate() {
                let sample = native_sample(bytes, wave.sample_format)?;
                let magnitude = sample.abs();
                peaks[channel] = peaks[channel].max(magnitude);
                // A finite f32 squared and summed over the bounded payload fits f64,
                // including subnormals, without quantizing the original samples.
                squares[channel].add(sample * sample);
                silence[channel].update(frame, magnitude <= silence_threshold_ratio)?;
                clipping[channel].update(frame, magnitude >= clipping_threshold_ratio)?;
            }
            frame += 1;
        }
        remaining -= amount as u64;
    }
    if frame != wave.frame_count {
        return Err(invalid("signal frame count differs from technical evidence"));
    }
    let mut channels = Vec::new();
    for (channel, (mut silence, mut clipping)) in silence.into_iter().zip(clipping).enumerate() {
        silence.update(frame, false)?;
        clipping.update(frame, false)?;
        channels.push(NativeChannelMeasurements {
            sample_peak_ratio: peaks[channel],
            rms_ratio: (squares[channel].total() / wave.frame_count as f64).sqrt(),
            silence: silence.regions,
            clipping: clipping.regions,
        });
    }
    Ok(NativePcmMeasurements { channels })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn run(samples: &[[i16; 2]], silence_ms: u32) -> PcmMeasurements {
        let mut source = tempfile::NamedTempFile::new().unwrap();
        for frame in samples {
            for sample in frame {
                source.write_all(&sample.to_le_bytes()).unwrap();
            }
        }
        let wave = PcmWave {
            sample_format: NativeSampleFormat::Pcm16,
            sample_rate_hz: 8000,
            channels: 2,
            frame_count: samples.len() as u64,
            pcm_sha256: "a".repeat(64),
            data_offset: 0,
            data_bytes: samples.len() as u64 * 4,
        };
        measure(source.path(), &wave, 32, silence_ms, 32767).unwrap()
    }

    #[test]
    fn exact_channel_values_and_half_open_threshold_regions() {
        let values = run(
            &[
                [0, 16384],
                [32, 16384],
                [33, 16384],
                [-32768, 16384],
                [32767, 16384],
                [0, 16384],
            ],
            0,
        );
        assert_eq!(values.channels[0].peak, 32768);
        assert_eq!(values.channels[0].sample_peak_ratio, 1.0);
        assert_eq!(values.channels[1].rms_ratio, 0.5);
        assert_eq!(values.channels[1].sample_peak_ratio, 0.5);
        assert_eq!(
            values.channels[0].silence,
            vec![
                AudioFrameRange { start: 0, end: 2 },
                AudioFrameRange { start: 5, end: 6 }
            ]
        );
        assert_eq!(
            values.channels[0].clipping,
            vec![AudioFrameRange { start: 3, end: 5 }]
        );
        assert!(values.channels[1].silence.is_empty());
    }

    #[test]
    fn silence_minimum_uses_ceiling_sample_frames() {
        let mut samples = vec![[0, 0]; 7];
        samples.push([100, 100]);
        samples.extend([[0, 0]; 8]);
        let values = run(&samples, 1);
        assert_eq!(
            values.channels[0].silence,
            vec![AudioFrameRange { start: 8, end: 16 }]
        );
        assert_eq!(values.channels[1].silence, values.channels[0].silence);
    }

    #[test]
    fn all_zero_pcm_keeps_exact_zero_energy_without_logarithms() {
        let values = run(&[[0, 0]; 16], 1);
        assert_eq!(values.channels[0].peak, 0);
        assert_eq!(values.channels[0].sum_squares, 0);
        assert_eq!(values.channels[0].rms_ratio, 0.0);
        assert_eq!(
            values.channels[0].silence,
            vec![AudioFrameRange { start: 0, end: 16 }]
        );
    }

    fn native_source(
        bytes: &[u8],
        sample_format: NativeSampleFormat,
        channels: u16,
    ) -> (tempfile::NamedTempFile, PcmWave) {
        let mut source = tempfile::NamedTempFile::new().unwrap();
        source.write_all(bytes).unwrap();
        let wave = PcmWave {
            sample_format,
            sample_rate_hz: 8000,
            channels,
            frame_count: bytes.len() as u64
                / (u64::from(channels) * u64::from(sample_format.bytes_per_sample())),
            pcm_sha256: "a".repeat(64),
            data_offset: 0,
            data_bytes: bytes.len() as u64,
        };
        (source, wave)
    }

    fn float_bytes(samples: &[f32]) -> Vec<u8> {
        samples.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    #[test]
    fn packed_pcm24_preserves_low_bits_and_signed_extremes_by_channel() {
        let samples = [[1_i32, -8_388_608_i32], [-1_i32, 8_388_607_i32]];
        let mut bytes = Vec::new();
        for frame in samples {
            for sample in frame {
                bytes.extend_from_slice(&sample.to_le_bytes()[..3]);
            }
        }
        let (source, wave) = native_source(&bytes, NativeSampleFormat::Pcm24, 2);
        let values = measure_native(source.path(), &wave, 0.0, 0, 1.0).unwrap();
        assert_eq!(values.channels[0].sample_peak_ratio, 1.0 / 8_388_608.0);
        assert_eq!(values.channels[0].rms_ratio, 1.0 / 8_388_608.0);
        assert!(values.channels[0].silence.is_empty());
        assert_eq!(values.channels[1].sample_peak_ratio, 1.0);
        let maximum_positive_ratio = 8_388_607.0_f64 / 8_388_608.0;
        assert_eq!(
            values.channels[1].rms_ratio,
            ((1.0 + maximum_positive_ratio * maximum_positive_ratio) / 2.0).sqrt()
        );
        assert_eq!(
            values.channels[1].clipping,
            vec![AudioFrameRange { start: 0, end: 1 }]
        );
        assert!(measure(source.path(), &wave, 32, 0, 32767).is_err());
    }

    #[test]
    fn native_pcm16_ratios_and_regions_agree_with_legacy_integer_measurement() {
        let samples = [0_i16, 32, -33, -32768, 32767, 0];
        let bytes: Vec<_> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let (source, wave) = native_source(&bytes, NativeSampleFormat::Pcm16, 1);
        let legacy = measure(source.path(), &wave, 32, 0, 32767).unwrap();
        let native = measure_native(
            source.path(),
            &wave,
            32.0 / 32768.0,
            0,
            32767.0 / 32768.0,
        )
        .unwrap();
        assert_eq!(
            native.channels[0].sample_peak_ratio,
            legacy.channels[0].sample_peak_ratio
        );
        assert_eq!(native.channels[0].rms_ratio, legacy.channels[0].rms_ratio);
        assert_eq!(native.channels[0].silence, legacy.channels[0].silence);
        assert_eq!(native.channels[0].clipping, legacy.channels[0].clipping);
    }

    #[test]
    fn float32_exceeding_full_scale_is_not_clamped_and_regions_are_half_open() {
        let samples = [0.0, -0.0, f32::from_bits(1), 0.5, -1.0, 1.0, 1.5, -2.0];
        let (source, wave) = native_source(&float_bytes(&samples), NativeSampleFormat::Float32, 1);
        let values = measure_native(source.path(), &wave, 0.0, 0, 1.0).unwrap();
        assert_eq!(values.channels[0].sample_peak_ratio, 2.0);
        assert_eq!(values.channels[0].rms_ratio, (8.5_f64 / 8.0).sqrt());
        assert_eq!(
            values.channels[0].silence,
            vec![AudioFrameRange { start: 0, end: 2 }]
        );
        assert_eq!(
            values.channels[0].clipping,
            vec![AudioFrameRange { start: 4, end: 8 }]
        );
        assert!(measure(source.path(), &wave, 32, 0, 32767).is_err());
    }

    #[test]
    fn float32_subnormal_and_largest_finite_values_keep_finite_nonzero_energy() {
        for sample in [f32::from_bits(1), 1.0e-9_f32, f32::MAX] {
            let (source, wave) = native_source(
                &float_bytes(&[sample, -sample]),
                NativeSampleFormat::Float32,
                1,
            );
            let values = measure_native(source.path(), &wave, 0.0, 0, 1.0).unwrap();
            assert_eq!(values.channels[0].sample_peak_ratio, f64::from(sample));
            assert_eq!(values.channels[0].rms_ratio, f64::from(sample));
            assert!(values.channels[0].silence.is_empty());
        }
    }

    #[test]
    fn native_float_refuses_every_nonfinite_sample_without_partial_measurements() {
        for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let (source, wave) = native_source(
                &float_bytes(&[0.5, sample]),
                NativeSampleFormat::Float32,
                1,
            );
            assert!(measure_native(source.path(), &wave, 0.0, 0, 1.0).is_err());
        }
    }

    #[test]
    fn compensated_energy_keeps_small_squares_after_a_large_sample() {
        let mut sum = CompensatedSum::default();
        sum.add(1.0);
        for _ in 0..100 {
            sum.add(1.0e-16);
        }
        assert_eq!(sum.total(), 1.0 + 1.0e-14);
    }

    #[test]
    fn native_pcm24_streams_complete_frames_across_blocks_and_honors_payload_bounds() {
        let mut bytes = vec![0xff; 7];
        bytes.extend(vec![0; 11_000 * 6]);
        let payload_end = bytes.len();
        bytes[payload_end - 6..payload_end - 3].copy_from_slice(&[0, 0, 0x80]);
        bytes[payload_end - 3..payload_end].copy_from_slice(&[0xff, 0xff, 0x7f]);
        bytes.extend_from_slice(&[0, 0, 0x80]);
        let (source, mut wave) = native_source(&bytes, NativeSampleFormat::Pcm24, 2);
        wave.data_offset = 7;
        wave.data_bytes = 11_000 * 6;
        wave.frame_count = 11_000;
        let values = measure_native(source.path(), &wave, 0.0, 1, 1.0).unwrap();
        assert_eq!(values.channels[0].sample_peak_ratio, 1.0);
        assert_eq!(values.channels[1].sample_peak_ratio, 8_388_607.0 / 8_388_608.0);
        assert_eq!(values.channels[0].rms_ratio, (1.0_f64 / 11_000.0).sqrt());
        assert_eq!(
            values.channels[0].silence,
            vec![AudioFrameRange { start: 0, end: 10_999 }]
        );
        assert_eq!(
            values.channels[0].clipping,
            vec![AudioFrameRange { start: 10_999, end: 11_000 }]
        );
        assert!(values.channels[1].clipping.is_empty());
    }

    #[test]
    fn native_measurement_refuses_invalid_geometry_truncation_and_thresholds() {
        let (source, mut wave) =
            native_source(&float_bytes(&[0.5]), NativeSampleFormat::Float32, 1);
        wave.frame_count = 0;
        assert!(measure_native(source.path(), &wave, 0.0, 0, 1.0).is_err());
        wave.frame_count = 2;
        assert!(measure_native(source.path(), &wave, 0.0, 0, 1.0).is_err());
        wave.data_bytes = 8;
        assert!(measure_native(source.path(), &wave, 0.0, 0, 1.0).is_err());
        wave.frame_count = 1;
        wave.data_bytes = 4;
        wave.data_offset = u64::MAX;
        assert!(measure_native(source.path(), &wave, 0.0, 0, 1.0).is_err());
        wave.data_offset = 0;
        for (silence, clipping) in [
            (f64::NAN, 1.0),
            (-1.0, 1.0),
            (1.1, 1.0),
            (0.0, f64::INFINITY),
            (0.0, 0.0),
            (0.0, 1.1),
        ] {
            assert!(measure_native(source.path(), &wave, silence, 0, clipping).is_err());
        }
    }

    #[test]
    fn too_many_regions_refuses_instead_of_truncating() {
        let mut regions = RegionBuilder {
            start: None,
            regions: Vec::new(),
            minimum_frames: 1,
        };
        for index in 0..10_000 {
            regions.update(index * 2, true).unwrap();
            regions.update(index * 2 + 1, false).unwrap();
        }
        regions.update(20_000, true).unwrap();
        assert!(regions.update(20_001, false).is_err());
    }
}
