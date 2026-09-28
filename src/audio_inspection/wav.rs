//! Independent bounded RIFF/PCM parser; never delegates source truth to FFmpeg.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::{AUDIO_INSPECTION_MAXIMUM_BYTES, invalid};
use crate::{Error, ErrorCategory, Result};

pub(crate) struct PcmWave {
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
            format = Some(values);
        } else if &chunk[..4] == b"data" {
            if payload.is_some() || format.is_none() || count == 0 {
                return Err(invalid(
                    "invalid_audio: WAV requires one nonempty data chunk after fmt",
                ));
            }
            let mut remaining = count;
            let mut digest = Sha256::new();
            let mut buffer = [0_u8; 65536];
            while remaining > 0 {
                let amount = usize::try_from(remaining.min(buffer.len() as u64))
                    .expect("bounded buffer length");
                read(&mut input, &mut buffer[..amount])?;
                digest.update(&buffer[..amount]);
                remaining -= amount as u64;
            }
            payload = Some((count, format!("{:x}", digest.finalize()), position));
        } else {
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
    if u16_at(&values, 0) != 1
        || u16_at(&values, 14) != 16
        || !matches!(channels, 1 | 2)
        || !(8000..=192000).contains(&sample_rate_hz)
    {
        return Err(invalid(
            "unsupported_audio: only PCM16 mono/stereo WAV at 8000–192000 Hz is supported",
        ));
    }
    if block_align != channels * 2
        || u32_at(&values, 8) != sample_rate_hz * u32::from(block_align)
        || data_bytes % u64::from(block_align) != 0
    {
        return Err(invalid(
            "invalid_audio: inconsistent PCM block alignment or byte rate",
        ));
    }
    let frame_count = data_bytes / u64::from(block_align);
    if frame_count > u64::from(sample_rate_hz) * 600 {
        return Err(invalid("unsupported_audio: duration exceeds 600 seconds"));
    }
    Ok(PcmWave {
        sample_rate_hz,
        channels,
        frame_count,
        pcm_sha256,
        data_offset,
        data_bytes,
    })
}
