//! Exact integer-domain measurements over the independently validated PCM payload.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::audio_analysis::AudioFrameRange;
use crate::audio_inspection::wav::PcmWave;
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
