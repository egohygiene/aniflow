//! Independent bounded RIFF/PCM parser; never delegates source truth to FFmpeg.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::types::{AUDIO_INSPECTION_MAXIMUM_BYTES, invalid};
use crate::{Error, ErrorCategory, Result};

/// Exact native sample representation admitted by the bounded WAV profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeSampleFormat {
    Pcm16,
    Pcm24,
    Float32,
}

impl NativeSampleFormat {
    #[must_use]
    pub const fn bytes_per_sample(self) -> u16 {
        match self {
            Self::Pcm16 => 2,
            Self::Pcm24 => 3,
            Self::Float32 => 4,
        }
    }

    #[must_use]
    pub const fn bits_per_sample(self) -> u16 {
        self.bytes_per_sample() * 8
    }

    #[must_use]
    pub const fn codec(self) -> &'static str {
        match self {
            Self::Pcm16 => "pcm_s16le",
            Self::Pcm24 => "pcm_s24le",
            Self::Float32 => "pcm_f32le",
        }
    }

    /// FFmpeg's decoded representation; packed PCM24 occupies s32 samples there.
    #[must_use]
    pub const fn sample_format(self) -> &'static str {
        match self {
            Self::Pcm16 => "s16",
            Self::Pcm24 => "s32",
            Self::Float32 => "flt",
        }
    }

    pub(crate) fn from_wave_header(format_code: u16, bits: u16) -> Result<Self> {
        match (format_code, bits) {
            (1, 16) => Ok(Self::Pcm16),
            (1, 24) => Ok(Self::Pcm24),
            (3, 32) => Ok(Self::Float32),
            _ => Err(invalid(
                "unsupported_audio: expected native PCM16, packed PCM24 or IEEE float32 WAV",
            )),
        }
    }
}

pub(crate) struct PcmWave {
    pub sample_format: NativeSampleFormat,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub pcm_sha256: String,
    pub data_offset: u64,
    pub data_bytes: u64,
}

fn read(input: &mut File, bytes: &mut [u8]) -> Result<()> {
    input
        .read_exact(bytes)
        .map_err(|_| invalid("invalid_audio: WAV contains truncated bytes"))
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

pub(crate) fn inspect(path: &Path) -> Result<PcmWave> {
    let wave = inspect_native(path)?;
    if wave.sample_format != NativeSampleFormat::Pcm16 {
        return Err(invalid(
            "unsupported_audio: this consumer requires the legacy PCM16 profile",
        ));
    }
    Ok(wave)
}

/// Parse native samples without quantizing, clipping, resampling or changing bytes.
pub(crate) fn inspect_native(path: &Path) -> Result<PcmWave> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| invalid("invalid_audio: source is unavailable"))?;
    let size = metadata.len();
    if !metadata.file_type().is_file() || !(44..=AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&size) {
        return Err(invalid(
            "invalid_audio: expected a nonempty nonsymlink WAV file up to 256 MiB",
        ));
    }
    let mut input = File::open(path)
        .map_err(|_| Error::new(ErrorCategory::Io, "could not open private audio snapshot"))?;
    let mut header = [0_u8; 12];
    read(&mut input, &mut header)?;
    if &header[..4] != b"RIFF"
        || &header[8..] != b"WAVE"
        || u64::from(u32_at(&header, 4)) + 8 != size
    {
        return Err(invalid(
            "unsupported_audio: expected an exact RIFF32/WAVE envelope",
        ));
    }
    let mut format = None;
    let mut payload = None;
    let mut float_fact_frames = None;
    let mut fact_before_format = false;
    let mut position = 12_u64;
    let mut chunks = 0;
    while position < size {
        chunks += 1;
        if chunks > 4096 {
            return Err(invalid("unsupported_audio: WAV exceeds 4096 chunks"));
        }
        let mut chunk = [0_u8; 8];
        read(&mut input, &mut chunk)?;
        position += 8;
        let count = u64::from(u32_at(&chunk, 4));
        let padded = count + count % 2;
        if position + padded > size {
            return Err(invalid("invalid_audio: truncated RIFF chunk"));
        }
        if &chunk[..4] == b"fmt " {
            if format.is_some() || !matches!(count, 16 | 18) {
                return Err(invalid(
                    "unsupported_audio: duplicate or extensible WAV format chunk",
                ));
            }
            let mut values = vec![0_u8; count as usize];
            read(&mut input, &mut values)?;
            if count == 18 && u16_at(&values, 16) != 0 {
                return Err(invalid("unsupported_audio: nonempty WAV format extension"));
            }
            let sample_format =
                NativeSampleFormat::from_wave_header(u16_at(&values, 0), u16_at(&values, 14))?;
            if sample_format == NativeSampleFormat::Float32 && fact_before_format {
                return Err(invalid("unsupported_audio: float fact chunk must follow fmt"));
            }
            format = Some(values);
        } else if &chunk[..4] == b"data" {
            if payload.is_some() || format.is_none() || count == 0 {
                return Err(invalid(
                    "invalid_audio: WAV requires one nonempty data chunk after fmt",
                ));
            }
            let mut remaining = count;
            let values = format.as_ref().expect("format precedes data");
            let sample_format =
                NativeSampleFormat::from_wave_header(u16_at(values, 0), u16_at(values, 14))?;
            if count % u64::from(sample_format.bytes_per_sample()) != 0 {
                return Err(invalid("invalid_audio: truncated native sample"));
            }
            let mut digest = Sha256::new();
            let mut buffer = [0_u8; 65536];
            while remaining > 0 {
                let amount = usize::try_from(remaining.min(buffer.len() as u64))
                    .expect("bounded buffer length");
                read(&mut input, &mut buffer[..amount])?;
                if sample_format == NativeSampleFormat::Float32 {
                    for sample in buffer[..amount].chunks_exact(4) {
                        let value = f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
                        if !value.is_finite() {
                            return Err(invalid(
                                "invalid_audio: float32 samples must be finite; NaN and infinity are refused",
                            ));
                        }
                    }
                }
                digest.update(&buffer[..amount]);
                remaining -= amount as u64;
            }
            payload = Some((count, format!("{:x}", digest.finalize()), position));
        } else if &chunk[..4] == b"fact"
            && format.as_ref().is_some_and(|values| u16_at(values, 0) == 3)
        {
            if count != 4 || float_fact_frames.is_some() {
                return Err(invalid("invalid_audio: unsupported or duplicate float fact chunk"));
            }
            let mut frames = [0_u8; 4];
            read(&mut input, &mut frames)?;
            float_fact_frames = Some(u64::from(u32::from_le_bytes(frames)));
        } else {
            if &chunk[..4] == b"fact" && format.is_none() {
                fact_before_format = true;
            }
            input
                .seek(SeekFrom::Current(
                    i64::try_from(count).expect("u32 chunk size"),
                ))
                .map_err(|_| invalid("invalid_audio: cannot traverse WAV chunk"))?;
        }
        if count % 2 != 0 {
            let mut padding = [0_u8; 1];
            read(&mut input, &mut padding)?;
        }
        position += padded;
    }
    if input
        .metadata()
        .map_err(|_| invalid("cannot inspect audio snapshot"))?
        .len()
        != size
    {
        return Err(invalid("invalid_audio: private snapshot changed"));
    }
    let values = format.ok_or_else(|| invalid("invalid_audio: missing fmt chunk"))?;
    let (data_bytes, pcm_sha256, data_offset) =
        payload.ok_or_else(|| invalid("invalid_audio: missing data chunk"))?;
    let channels = u16_at(&values, 2);
    let sample_rate_hz = u32_at(&values, 4);
    let block_align = u16_at(&values, 12);
    let sample_format =
        NativeSampleFormat::from_wave_header(u16_at(&values, 0), u16_at(&values, 14))?;
    if !matches!(channels, 1 | 2)
        || !(8000..=192000).contains(&sample_rate_hz)
    {
        return Err(invalid(
            "unsupported_audio: native PCM WAV requires mono/stereo at 8000–192000 Hz",
        ));
    }
    if block_align != channels * sample_format.bytes_per_sample()
        || u32_at(&values, 8) != sample_rate_hz * u32::from(block_align)
        || data_bytes % u64::from(block_align) != 0
    {
        return Err(invalid(
            "invalid_audio: inconsistent PCM block alignment or byte rate",
        ));
    }
    let frame_count = data_bytes / u64::from(block_align);
    if float_fact_frames.is_some_and(|frames| frames != frame_count) {
        return Err(invalid("invalid_audio: float fact sample count differs from data"));
    }
    if frame_count > u64::from(sample_rate_hz) * 600 {
        return Err(invalid("unsupported_audio: duration exceeds 600 seconds"));
    }
    Ok(PcmWave {
        sample_format,
        sample_rate_hz,
        channels,
        frame_count,
        pcm_sha256,
        data_offset,
        data_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn wave(format: NativeSampleFormat, channels: u16, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let block = channels * format.bytes_per_sample();
        let padded = data.len() + data.len() % 2;
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + padded as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        let tag = if format == NativeSampleFormat::Float32 {
            3_u16
        } else {
            1_u16
        };
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&48_000_u32.to_le_bytes());
        bytes.extend_from_slice(&(48_000 * u32::from(block)).to_le_bytes());
        bytes.extend_from_slice(&block.to_le_bytes());
        bytes.extend_from_slice(&format.bits_per_sample().to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(data);
        if data.len() % 2 != 0 {
            bytes.push(0);
        }
        bytes
    }

    fn inspect_bytes(bytes: &[u8]) -> Result<PcmWave> {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        inspect_native(file.path())
    }

    #[test]
    fn packed_pcm24_retains_signed_extremes_and_odd_chunk_padding() {
        let data: Vec<u8> = [-8_388_608_i32, -1, 0, 1, 8_388_607]
            .iter()
            .flat_map(|value| value.to_le_bytes()[..3].to_vec())
            .collect();
        let bytes = wave(NativeSampleFormat::Pcm24, 1, &data);
        let inspected = inspect_bytes(&bytes).unwrap();
        assert_eq!(inspected.sample_format, NativeSampleFormat::Pcm24);
        assert_eq!(inspected.frame_count, 5);
        assert_eq!(inspected.data_bytes, 15);
        assert_eq!(inspected.pcm_sha256, format!("{:x}", Sha256::digest(&data)));
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        assert!(
            inspect(file.path()).is_err(),
            "legacy consumers must not reinterpret PCM24"
        );
    }

    #[test]
    fn finite_float_preserves_headroom_negative_zero_and_subnormals() {
        let data: Vec<u8> = [-1.5_f32, 1.25, -0.0, f32::from_bits(1), f32::MAX, 0.0]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let bytes = wave(NativeSampleFormat::Float32, 2, &data);
        let inspected = inspect_bytes(&bytes).unwrap();
        assert_eq!(inspected.sample_format, NativeSampleFormat::Float32);
        assert_eq!(inspected.frame_count, 3);
        assert_eq!(inspected.pcm_sha256, format!("{:x}", Sha256::digest(&data)));
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        assert!(
            inspect(file.path()).is_err(),
            "legacy consumers must not reinterpret floats"
        );
    }

    #[test]
    fn rejects_nonfinite_float_at_and_beyond_the_hash_block_boundary() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for prefix in [0, 65_536] {
                let mut data = vec![0_u8; prefix];
                data.extend_from_slice(&value.to_le_bytes());
                assert!(inspect_bytes(&wave(NativeSampleFormat::Float32, 1, &data)).is_err());
            }
        }
    }

    #[test]
    fn rejects_wrong_native_width_alignment_and_truncated_samples() {
        let mut wrong_width = wave(NativeSampleFormat::Pcm24, 1, &[0, 0, 0]);
        wrong_width[34..36].copy_from_slice(&32_u16.to_le_bytes());
        assert!(inspect_bytes(&wrong_width).is_err());
        let mut wrong_alignment = wave(NativeSampleFormat::Float32, 2, &[0; 8]);
        wrong_alignment[32..34].copy_from_slice(&4_u16.to_le_bytes());
        assert!(inspect_bytes(&wrong_alignment).is_err());
        assert!(inspect_bytes(&wave(NativeSampleFormat::Pcm24, 1, &[0, 0])).is_err());
        assert!(inspect_bytes(&wave(NativeSampleFormat::Float32, 1, &[0, 0, 0])).is_err());
    }

    #[test]
    fn rejects_extensible_and_rf64_without_silently_selecting_a_native_profile() {
        let mut bytes = wave(NativeSampleFormat::Pcm16, 1, &[0, 0]);
        bytes[20..22].copy_from_slice(&0xfffe_u16.to_le_bytes());
        assert!(inspect_bytes(&bytes).is_err());
        bytes[20..22].copy_from_slice(&1_u16.to_le_bytes());
        bytes[..4].copy_from_slice(b"RF64");
        assert!(inspect_bytes(&bytes).is_err());
    }

    #[test]
    fn optional_float_fact_must_match_exact_sample_frames() {
        let mut bytes = wave(NativeSampleFormat::Float32, 2, &[0; 16]);
        bytes.extend_from_slice(b"fact");
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        let riff_size = bytes.len() as u32 - 8;
        bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
        assert_eq!(inspect_bytes(&bytes).unwrap().frame_count, 2);
        let end = bytes.len();
        bytes[end - 4..].copy_from_slice(&4_u32.to_le_bytes());
        assert!(inspect_bytes(&bytes).is_err());
    }

    #[test]
    fn float_fact_cannot_precede_format_or_appear_twice() {
        let mut fact = b"fact".to_vec();
        fact.extend_from_slice(&4_u32.to_le_bytes());
        fact.extend_from_slice(&1_u32.to_le_bytes());
        let original = wave(NativeSampleFormat::Float32, 1, &[0; 4]);
        let mut before = original[..12].to_vec();
        before.extend_from_slice(&fact);
        before.extend_from_slice(&original[12..]);
        let size = before.len() as u32 - 8;
        before[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(inspect_bytes(&before).is_err());
        let mut duplicate = original;
        duplicate.extend_from_slice(&fact);
        duplicate.extend_from_slice(&fact);
        let size = duplicate.len() as u32 - 8;
        duplicate[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(inspect_bytes(&duplicate).is_err());
    }
}
