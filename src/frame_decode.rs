//! One bounded PNG-to-RGB24 observation shared by frame import and repair.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use crate::audio_inspection::{AudioInspectionConfiguration, AudioToolPin};
use crate::audio_inspection::process::{GroupPolicy, run_tool_isolated};
use crate::provider::require_sha256;
use crate::{CancellationToken, Error, ErrorCategory, Result};
fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }

fn decode_arguments(snapshot: &Path) -> Vec<OsString> {
    // No frame limit, stream copy, resizing, or error recovery. All input PNG
    // data must be decoded; the closed PNG profile rejects APNG/extra chunks.
    ["-hide_banner", "-nostdin", "-v", "error", "-xerror", "-err_detect", "explode",
     "-threads", "1", "-protocol_whitelist", "file,pipe", "-f", "image2",
     "-pattern_type", "none", "-c:v", "png", "-i", "{snapshot}", "-map", "0:v:0",
     "-an", "-sn", "-dn", "-c:v", "rawvideo", "-pix_fmt", "+rgb24", "-threads", "1",
     "-f", "framehash", "-format_version", "2", "-hash", "sha256", "pipe:1"]
        .into_iter().map(|value| if value == "{snapshot}" { snapshot.as_os_str().to_owned() } else { value.into() }).collect()
}

fn decoded_hash(bytes: &[u8], width: u32, height: u32, expected_bytes: u64) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|_| refused("decoder framehash is not UTF-8"))?;
    let required = ["#format: frame checksums".to_owned(), "#version: 2".to_owned(), "#hash: SHA256".to_owned(),
        "#media_type 0: video".to_owned(), "#codec_id 0: rawvideo".to_owned(), format!("#dimensions 0: {width}x{height}")];
    let mut headers = BTreeSet::new();
    let mut header_keys = BTreeSet::new();
    let mut hash = None;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with('#') {
            let key = if required.iter().any(|header| header == line) {
                line.split(':').next().expect("fixed header key")
            } else if line.starts_with("#software:") { "#software" }
            else if line.starts_with("#tb 0:") { "#tb 0" }
            else if matches!(line, "#sar 0: 0/1" | "#sar 0: 1/1") { "#sar 0" }
            else if line.strip_prefix('#').is_some_and(|value| value.split(',').map(str::trim).eq(["stream#", "dts", "pts", "duration", "size", "hash"])) { "#columns" }
            else { return Err(refused("decoder returned an unsupported framehash header")); };
            if hash.is_some() || !header_keys.insert(key.to_owned()) || !headers.insert(line.to_owned()) {
                return Err(refused("decoder returned duplicate or misplaced framehash headers"));
            }
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if hash.is_some() || fields.len() != 6 || fields[0] != "0"
            || fields[1].parse::<i64>().is_err() || fields[2].parse::<i64>().is_err()
            || fields[3].parse::<u64>().ok().is_none_or(|value| value == 0)
            || fields[4].parse::<u64>().ok() != Some(expected_bytes)
        { return Err(refused("decoder must return exactly one complete RGB24 frame of the declared size")); }
        require_sha256(fields[5], "decoded RGB24 SHA-256")?;
        hash = Some(fields[5].to_owned());
    }
    if required.iter().any(|header| !headers.contains(header)) {
        return Err(refused("decoder framehash profile or geometry differs"));
    }
    hash.ok_or_else(|| refused("decoder returned no complete frame"))
}

pub(crate) fn verify_version(tool: &AudioToolPin, limits: &AudioInspectionConfiguration, cancellation: &CancellationToken, directory: &Path) -> Result<()> {
    let capture = run_tool_isolated(tool, "ffmpeg", &["-version".into()], limits, cancellation, GroupPolicy::Inherit, Some(directory))
        .map_err(|diagnostic| Error::new(ErrorCategory::Dependency, format!("{:?}: {}", diagnostic.code, diagnostic.message)))?;
    let text = std::str::from_utf8(&capture.stdout).map_err(|_| refused("decoder version banner is not UTF-8"))?;
    let tokens: Vec<_> = text.lines().next().unwrap_or_default().split_whitespace().take(3).collect();
    if tokens != ["ffmpeg", "version", tool.version.as_str()] { return Err(refused("decoder version differs from its exact configured token")); }
    Ok(())
}

pub(crate) fn decode(tool: &AudioToolPin, limits: &AudioInspectionConfiguration, cancellation: &CancellationToken, directory: &Path, path: &Path, width: u32, height: u32) -> Result<String> {
    let expected = u64::from(width).checked_mul(u64::from(height)).and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| refused("decoded RGB24 byte count overflow"))?;
    let capture = run_tool_isolated(tool, "ffmpeg", &decode_arguments(path), limits, cancellation, GroupPolicy::Inherit, Some(directory))
        .map_err(|diagnostic| refused(format!("PNG decode failed ({:?}): {}", diagnostic.code, diagnostic.message)))?;
    if !capture.stderr.is_empty() { return Err(refused("PNG decoder emitted an error diagnostic")); }
    decoded_hash(&capture.stdout, width, height, expected)
}
