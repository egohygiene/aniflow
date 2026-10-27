//! Confined bounded frame-file observations shared by import and interpolation.
//! Structural PNG checks are distinct from a native full decode.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;
use sha2::{Digest, Sha256};
use crate::audio_inspection::process::{hash_regular, open_regular};
use crate::{CancellationToken, Error, ErrorCategory, Result};
const MAXIMUM_PNG_CHUNKS: u64 = 65_536;
fn fail(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }

pub(crate) fn ensure_canonical_directory(path: &Path) -> Result<()> {
    if path.canonicalize().ok().as_deref() != Some(path)
        || std::fs::symlink_metadata(path)
            .map_or(true, |metadata| !metadata.file_type().is_dir())
    {
        return Err(fail("frame requires a canonical nonsymlink directory"));
    }
    Ok(())
}

pub(crate) fn ensure_parent(parent: &Path) -> Result<()> {
    if !parent.is_absolute() {
        return Err(fail("frame output parent must be an absolute workspace path"));
    }
    let mut existing = parent;
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() {
                    return Err(fail("frame output ancestors must be nonsymlink directories"));
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing
                    .parent()
                    .ok_or_else(|| fail("frame output ancestor is unavailable"))?;
            }
            Err(_) => return Err(fail("cannot inspect assigned frame workspace")),
        }
    }
    ensure_canonical_directory(existing)?;
    std::fs::create_dir_all(parent)
        .map_err(|_| fail("cannot create assigned frame workspace"))?;
    ensure_canonical_directory(parent)
}

pub(crate) fn require_absent(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(fail("assigned frame output already exists or is unreadable")),
    }
}

pub(crate) fn inventory(directory: &Path, expected: &[OsString]) -> Result<()> {
    ensure_canonical_directory(directory)?;
    let mut names = BTreeSet::new();
    for item in std::fs::read_dir(directory)
        .map_err(|_| fail("cannot inspect frame directory inventory"))?
    {
        let item = item.map_err(|_| fail("cannot inspect frame directory entry"))?;
        if names.len() >= expected.len()
            || !item
                .file_type()
                .map_err(|_| fail("cannot inspect frame entry type"))?
                .is_file()
        {
            return Err(fail("frame directory contains an undeclared or special entry"));
        }
        names.insert(item.file_name());
    }
    if names != expected.iter().cloned().collect() {
        return Err(fail("frame directory inventory differs from its exact declared files"));
    }
    Ok(())
}

pub(crate) fn identity(path: &Path, maximum: u64) -> Result<(String, u64)> {
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(fail("frame inputs must remain canonical and symlink-free"));
    }
    hash_regular(path, maximum)
        .map_err(|_| fail("frame input must remain a bounded nonsymlink regular file"))
}

pub(crate) fn copy_pinned(
    source: &Path,
    destination: &Path,
    expected: &(String, u64),
    maximum: u64,
    cancellation: &CancellationToken,
) -> Result<()> {
    if identity(source, maximum)? != *expected {
        return Err(fail("frame input bytes differ from their pinned identity"));
    }
    let mut input = open_regular(source)
        .map_err(|_| fail("cannot safely open frame input"))?
        .take(maximum.saturating_add(1));
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| fail("cannot reserve private frame snapshot"))?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::new(ErrorCategory::Execution, "frame staging cancelled"));
        }
        let count = input
            .read(&mut buffer)
            .map_err(|_| fail("cannot read frame snapshot input"))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|value| *value <= maximum)
            .ok_or_else(|| fail("frame snapshot exceeds its configured byte bound"))?;
        digest.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|_| fail("cannot write frame snapshot"))?;
    }
    output
        .sync_all()
        .map_err(|_| fail("cannot synchronize frame snapshot"))?;
    if (format!("{:x}", digest.finalize()), total) != *expected
        || identity(destination, maximum)? != *expected
    {
        return Err(fail("frame snapshot identity changed during staging"));
    }
    Ok(())
}

const fn crc32_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(value & 1)));
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}
const CRC32_TABLE: [u32; 256] = crc32_table();
pub(crate) fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ u32::from(*byte)) & 255) as usize];
    }
    crc
}

/// Check bounded PNG structure without allocating a decompressed image.
/// This does not establish decoded image fidelity or the truth of an SDR declaration.
pub(crate) fn inspect_png(path: &Path, width: u32, height: u32, maximum: u64) -> Result<(String, u64)> {
    let observed = identity(path, maximum)?;
    let mut file = open_regular(path).map_err(|_| fail("cannot open bounded frame PNG"))?;
    let mut signature = [0_u8; 8];
    file.read_exact(&mut signature)
        .map_err(|_| fail("frame PNG is truncated"))?;
    if signature != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(fail("frame must be a PNG file"));
    }
    let mut consumed = 8_u64;
    let mut data_seen = false;
    let mut data_finished = false;
    let mut singleton_chunks = BTreeSet::new();
    for ordinal in 0..MAXIMUM_PNG_CHUNKS {
        let mut header = [0_u8; 8];
        file.read_exact(&mut header)
            .map_err(|_| fail("frame PNG chunk is truncated"))?;
        let length = u32::from_be_bytes(header[..4].try_into().expect("four-byte length"));
        let kind: [u8; 4] = header[4..].try_into().expect("four-byte chunk type");
        consumed = consumed
            .checked_add(u64::from(length) + 12)
            .filter(|value| *value <= observed.1)
            .ok_or_else(|| fail("frame PNG chunk exceeds its bounded file"))?;
        if (ordinal == 0) != (kind == *b"IHDR") {
            return Err(fail("frame PNG requires exactly one initial IHDR"));
        }
        match &kind {
            b"IHDR" if length == 13 => (),
            b"IDAT" if !data_finished => data_seen = true,
            b"IEND" if length == 0 && data_seen => (),
            b"sRGB" if length == 1 && !data_seen => (),
            b"gAMA" if length == 4 && !data_seen => (),
            b"cHRM" if length == 32 && !data_seen => (),
            b"pHYs" if length == 9 && !data_seen => (),
            _ => return Err(fail("frame PNG contains unsupported chunks or ordering")),
        }
        if kind != *b"IDAT" && !singleton_chunks.insert(kind) {
            return Err(fail("frame PNG contains duplicate singleton chunks"));
        }
        if data_seen && kind != *b"IDAT" {
            data_finished = true;
        }
        let mut crc = crc32_update(u32::MAX, &kind);
        let mut remaining = u64::from(length);
        let mut small = [0_u8; 32];
        let mut buffer = [0_u8; 65_536];
        while remaining > 0 {
            let count = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded PNG read count");
            file.read_exact(&mut buffer[..count])
                .map_err(|_| fail("frame PNG payload is truncated"))?;
            crc = crc32_update(crc, &buffer[..count]);
            if kind != *b"IDAT" {
                small[..count].copy_from_slice(&buffer[..count]);
            }
            remaining -= count as u64;
        }
        let mut checksum = [0_u8; 4];
        file.read_exact(&mut checksum)
            .map_err(|_| fail("frame PNG checksum is truncated"))?;
        if !crc != u32::from_be_bytes(checksum) {
            return Err(fail("frame PNG chunk checksum differs"));
        }
        match &kind {
            b"IHDR" => {
                let actual_width = u32::from_be_bytes(small[..4].try_into().expect("PNG width"));
                let actual_height = u32::from_be_bytes(small[4..8].try_into().expect("PNG height"));
                if actual_width != width
                    || actual_height != height
                    || small[8..13] != [8, 2, 0, 0, 0]
                {
                    return Err(fail("frame PNG must match declared unrotated RGB8 geometry"));
                }
            }
            b"sRGB" if small[0] > 3 => return Err(fail("frame PNG has invalid sRGB intent")),
            b"gAMA" if small[..4] != 45455_u32.to_be_bytes() => {
                return Err(fail("frame PNG gamma differs from the supported SDR profile"));
            }
            b"cHRM" => {
                let expected = [31270_u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000];
                if small
                    .chunks_exact(4)
                    .zip(expected)
                    .any(|(bytes, expected)| bytes != expected.to_be_bytes())
                {
                    return Err(fail("frame PNG chromaticities differ from the SDR profile"));
                }
            }
            b"pHYs" if small[..4] == [0, 0, 0, 0]
                || small[..4] != small[4..8] || small[8] > 1 => {
                return Err(fail("frame PNG requires square pixels"));
            }
            b"IEND" => {
                let mut tail = [0_u8; 1];
                if consumed != observed.1
                    || file.read(&mut tail).map_err(|_| fail("cannot inspect PNG tail"))? != 0
                    || identity(path, maximum)? != observed
                {
                    return Err(fail("frame PNG has trailing data or changed during inspection"));
                }
                return Ok(observed);
            }
            _ => (),
        }
    }
    Err(fail("frame PNG exceeds the bounded chunk count"))
}

