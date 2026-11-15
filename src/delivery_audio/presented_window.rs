//! Explicit analysis view of the already-proved presented sample interval.
//! Complete decoder PCM and delivery media are never rewritten. The separate
//! WAV contains exactly selected source bytes, with no decode/gain/resampling.
use super::source_signal::VideoAudioSignalReceipt;
use crate::audio_inspection::{AudioInspectionConfiguration, NativeSampleFormat};
use crate::audio_signal::NativeLoudnessReceipt;
use crate::destination::local;
use crate::frame_io::{ensure_canonical_directory, identity};
use crate::temporal::{RationalTime, TimeRange};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const SCOPE: &str = "exact_presented_audio_window";
const METHOD: &str =
    "exact_float32_sample_bytes_from_complete_pcm; no_decode_gain_resample_or_delivery_edit";
const DIRECTORY: &str = "presented-window";
const WAV: &str = "selected-audio.wav";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentedWindowSelection {
    ExactPresentedAudio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentedWindowReceipt {
    pub schema: String,
    pub metric_scope: String,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub temporal_sha256: String,
    pub complete_pcm_sha256: String,
    pub complete_sample_frames: u64,
    pub source_audio_extent: TimeRange,
    pub start_sample_frame: u64,
    pub presented_sample_frames: u64,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub window_wav_sha256: String,
    pub window_wav_bytes: u64,
    pub window_pcm_sha256: String,
    pub method: String,
    pub loudness: NativeLoudnessReceipt,
    pub normalization: String,
    pub runtime_qualified: bool,
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn mapped(error: anyhow::Error) -> Error {
    Error::from_anyhow(ErrorCategory::Media, error)
}
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        return Err(invalid(
            "presented audio window cancelled; partial analysis is retained",
        ));
    }
    Ok(())
}

fn sample_count(range: TimeRange) -> Result<u64> {
    let count = range
        .duration()
        .map_err(mapped)?
        .checked_mul(48_000)
        .map_err(mapped)?;
    if range.start != RationalTime::ZERO || count.denominator() != 1 || count.numerator() <= 0 {
        return Err(invalid(
            "presented audio window requires the exact positive zero-origin sample interval",
        ));
    }
    Ok(count.numerator() as u64)
}

fn header(frames: u64, channels: u16) -> Result<Vec<u8>> {
    if frames == 0 || !matches!(channels, 1 | 2) {
        return Err(invalid(
            "presented audio window requires positive mono/stereo samples",
        ));
    }
    let bytes = frames
        .checked_mul(u64::from(channels) * 4)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| invalid("presented audio window RIFF size overflow"))?;
    let size = bytes
        .checked_add(48)
        .ok_or_else(|| invalid("presented audio window RIFF envelope overflow"))?;
    let frames =
        u32::try_from(frames).map_err(|_| invalid("presented audio window fact count overflow"))?;
    let mut out = Vec::with_capacity(56);
    out.extend_from_slice(b"RIFF");
    out.extend(size.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(3u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(48_000u32.to_le_bytes());
    out.extend((48_000 * u32::from(channels) * 4).to_le_bytes());
    out.extend((channels * 4).to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend_from_slice(b"fact");
    out.extend(4u32.to_le_bytes());
    out.extend(frames.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend(bytes.to_le_bytes());
    Ok(out)
}

pub(super) fn projected_bytes(frames: u64, channels: u16) -> Result<u64> {
    header(frames, channels)?;
    frames
        .checked_mul(u64::from(channels) * 4)
        .and_then(|n| n.checked_add(56))
        .ok_or_else(|| invalid("presented audio window storage overflow"))
}

/// Structural scope binding, also required for saved policy snapshots. Current
/// QC readback additionally verifies the actual source/window bytes below.
pub(super) fn validate_binding(
    signal: &VideoAudioSignalReceipt,
    window: &PresentedWindowReceipt,
) -> Result<()> {
    let frames = sample_count(signal.source_audio_extent)?;
    crate::audio_signal::loudness::validate_receipt(&window.loudness)?;
    for value in [
        &window.source_sha256,
        &window.temporal_sha256,
        &window.complete_pcm_sha256,
        &signal.decoded_wav_sha256,
        &window.window_wav_sha256,
        &window.window_pcm_sha256,
    ] {
        crate::provider::require_sha256(value, "presented audio window digest")?;
    }
    if window.schema != "aniflow.presented-audio-window/v1"
        || window.metric_scope != SCOPE
        || signal.source_bytes == 0
        || window.source_sha256 != signal.source_sha256
        || window.source_bytes != signal.source_bytes
        || window.temporal_sha256 != signal.temporal_sha256
        || window.complete_pcm_sha256 != signal.decoded_pcm_sha256
        || window.complete_sample_frames != signal.frames
        || window.source_audio_extent != signal.source_audio_extent
        || window.start_sample_frame != 0
        || window.presented_sample_frames != frames
        || frames > signal.frames
        || super::source_signal::complete_frames(
            signal.source_audio_extent,
            signal.decoded_audio_tail.as_ref(),
        )? != signal.frames
        || window.sample_rate_hz != 48_000
        || signal.sample_rate_hz != 48_000
        || window.channels != signal.channels
        || window.window_wav_bytes != projected_bytes(frames, window.channels)?
        || window.method != METHOD
        || window.normalization != "none"
        || window.runtime_qualified
        || window.loudness.pcm_sha256 != window.window_pcm_sha256
        || window.loudness.frames != frames
        || window.loudness.sample_rate_hz != window.sample_rate_hz
        || window.loudness.channels != window.channels
    {
        return Err(invalid(
            "presented audio window differs from its exact source, complete PCM, interval or meter scope",
        ));
    }
    Ok(())
}

pub(crate) fn policy_meter(
    qc: &super::final_qc::FinalAudioQcReport,
) -> Result<&NativeLoudnessReceipt> {
    let signal = &qc.signal;
    if qc.intent.request.presented_window != qc.intent.signal_request.presented_window
        || qc.intent.signal_request.presented_window.is_some() != signal.presented_window.is_some()
    {
        return Err(crate::destination::invalid(
            "presented-window QC selection and exact signal evidence differ",
        ));
    }
    if let Some(window) = &signal.presented_window {
        validate_binding(signal, window)?;
        return Ok(&window.loudness);
    }
    if signal.decoded_audio_tail.is_some() {
        return Err(crate::destination::invalid(
            "presented delivery loudness is unverified: current meter covers complete decoded AAC including nonpresented tail",
        ));
    }
    Ok(&signal.loudness)
}

/// Rehash both files and compare every selected sample byte. This launches no
/// media tool and never treats a stored summary or metric alone as current proof.
pub(super) fn validate_current(
    output: &Path,
    signal: &VideoAudioSignalReceipt,
    window: &PresentedWindowReceipt,
    maximum: u64,
    token: &CancellationToken,
) -> Result<()> {
    validate_binding(signal, window)?;
    validate_bytes(
        output,
        signal,
        ViewIdentity {
            frames: window.presented_sample_frames,
            wav_sha256: &window.window_wav_sha256,
            wav_bytes: window.window_wav_bytes,
            pcm_sha256: &window.window_pcm_sha256,
        },
        maximum,
        token,
    )
}

struct ViewIdentity<'a> {
    frames: u64,
    wav_sha256: &'a str,
    wav_bytes: u64,
    pcm_sha256: &'a str,
}
fn validate_bytes(
    output: &Path,
    signal: &VideoAudioSignalReceipt,
    view_identity: ViewIdentity<'_>,
    maximum: u64,
    token: &CancellationToken,
) -> Result<()> {
    cancelled(token)?;
    if view_identity.frames != sample_count(signal.source_audio_extent)?
        || view_identity.frames > signal.frames
        || view_identity.wav_bytes != projected_bytes(view_identity.frames, signal.channels)?
    {
        return Err(invalid(
            "presented PCM byte proof has a different sample interval",
        ));
    }
    let directory = output.join(DIRECTORY);
    ensure_canonical_directory(&directory)?;
    local::entries(&directory, &[WAV], &[], false)?;
    let source = output.join("selected-audio.wav");
    let selected = directory.join(WAV);
    let source_pin = (signal.decoded_wav_sha256.clone(), signal.decoded_wav_bytes);
    let selected_pin = (view_identity.wav_sha256.to_owned(), view_identity.wav_bytes);
    if identity(&source, maximum)? != source_pin || identity(&selected, maximum)? != selected_pin {
        return Err(invalid(
            "presented audio window or complete PCM file identity changed",
        ));
    }
    let original = crate::audio_inspection::wav::inspect_native(&source)?;
    let view = crate::audio_inspection::wav::inspect_native(&selected)?;
    if original.sample_format != NativeSampleFormat::Float32
        || view.sample_format != NativeSampleFormat::Float32
        || original.frame_count != signal.frames
        || original.pcm_sha256 != signal.decoded_pcm_sha256
        || original.sample_rate_hz != 48_000
        || original.channels != signal.channels
        || view.frame_count != view_identity.frames
        || view.pcm_sha256 != view_identity.pcm_sha256
        || view.sample_rate_hz != 48_000
        || view.channels != signal.channels
        || view.data_offset != 56
    {
        return Err(invalid(
            "presented audio window sample representation or complete PCM binding changed",
        ));
    }
    let mut before =
        File::open(&source).map_err(|_| invalid("complete PCM analysis source unavailable"))?;
    let mut after =
        File::open(&selected).map_err(|_| invalid("presented PCM analysis view unavailable"))?;
    let expected_header = header(view_identity.frames, signal.channels)?;
    let mut observed_header = vec![0; expected_header.len()];
    after
        .read_exact(&mut observed_header)
        .map_err(|_| invalid("presented PCM header truncated"))?;
    if observed_header != expected_header {
        return Err(invalid(
            "presented PCM header differs from the canonical analysis view",
        ));
    }
    before
        .seek(SeekFrom::Start(original.data_offset))
        .map_err(|_| invalid("cannot seek complete PCM sample payload"))?;
    let mut remaining = view.data_bytes;
    let (mut a, mut b) = ([0u8; 65536], [0u8; 65536]);
    while remaining > 0 {
        cancelled(token)?;
        let n = remaining.min(a.len() as u64) as usize;
        before
            .read_exact(&mut a[..n])
            .map_err(|_| invalid("complete PCM samples truncated"))?;
        after
            .read_exact(&mut b[..n])
            .map_err(|_| invalid("presented PCM samples truncated"))?;
        if a[..n] != b[..n] {
            return Err(invalid(
                "presented PCM samples are not the exact selected complete-source bytes",
            ));
        }
        remaining -= n as u64;
    }
    if identity(&source, maximum)? != source_pin || identity(&selected, maximum)? != selected_pin {
        return Err(invalid(
            "presented or complete PCM changed during window readback",
        ));
    }
    cancelled(token)
}

pub(super) fn measure(
    output: &Path,
    signal: &VideoAudioSignalReceipt,
    configuration: &AudioInspectionConfiguration,
    maximum: u64,
    minimum_free_bytes: u64,
    token: &CancellationToken,
) -> Result<PresentedWindowReceipt> {
    cancelled(token)?;
    let frames = sample_count(signal.source_audio_extent)?;
    let bytes = projected_bytes(frames, signal.channels)?;
    if bytes > maximum || frames > signal.frames {
        return Err(invalid(
            "presented PCM view exceeds its finite complete-signal bound",
        ));
    }
    let source = output.join("selected-audio.wav");
    let source_pin = (signal.decoded_wav_sha256.clone(), signal.decoded_wav_bytes);
    if identity(&source, maximum)? != source_pin {
        return Err(invalid(
            "complete PCM changed before presented-window selection",
        ));
    }
    let wave = crate::audio_inspection::wav::inspect_native(&source)?;
    if wave.sample_format != NativeSampleFormat::Float32
        || wave.sample_rate_hz != 48_000
        || wave.channels != signal.channels
        || wave.frame_count != signal.frames
        || wave.pcm_sha256 != signal.decoded_pcm_sha256
    {
        return Err(invalid(
            "presented window requires the exact retained complete float32 PCM",
        ));
    }
    crate::video_assembly::require_storage(output, bytes, minimum_free_bytes)?;
    let directory = output.join(DIRECTORY);
    local::directory(&directory)?;
    let selected = directory.join(WAV);
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut destination = options
        .open(&selected)
        .map_err(|_| invalid("cannot reserve fresh presented PCM analysis view"))?;
    destination
        .write_all(&header(frames, signal.channels)?)
        .map_err(|_| invalid("cannot write presented PCM header"))?;
    let mut input = File::open(&source).map_err(|_| invalid("complete PCM source unavailable"))?;
    input
        .seek(SeekFrom::Start(wave.data_offset))
        .map_err(|_| invalid("cannot seek complete PCM sample payload"))?;
    let mut remaining = bytes - 56;
    let mut buffer = [0u8; 65536];
    let mut checksum = Sha256::new();
    while remaining > 0 {
        cancelled(token)?;
        let n = remaining.min(buffer.len() as u64) as usize;
        input
            .read_exact(&mut buffer[..n])
            .map_err(|_| invalid("complete PCM sample range truncated"))?;
        destination
            .write_all(&buffer[..n])
            .map_err(|_| invalid("cannot retain presented PCM sample bytes"))?;
        checksum.update(&buffer[..n]);
        remaining -= n as u64;
    }
    destination
        .sync_all()
        .map_err(|_| invalid("cannot commit presented PCM analysis view"))?;
    drop(destination);
    let selected_pin = identity(&selected, maximum)?;
    let pcm_sha256 = format!("{:x}", checksum.finalize());
    validate_bytes(
        output,
        signal,
        ViewIdentity {
            frames,
            wav_sha256: &selected_pin.0,
            wav_bytes: selected_pin.1,
            pcm_sha256: &pcm_sha256,
        },
        maximum,
        token,
    )?;
    let loudness = crate::audio_signal::loudness::measure(
        &selected,
        &selected_pin,
        configuration,
        &directory,
        token,
    )?;
    let receipt = PresentedWindowReceipt {
        schema: "aniflow.presented-audio-window/v1".into(),
        metric_scope: SCOPE.into(),
        source_sha256: signal.source_sha256.clone(),
        source_bytes: signal.source_bytes,
        temporal_sha256: signal.temporal_sha256.clone(),
        complete_pcm_sha256: signal.decoded_pcm_sha256.clone(),
        complete_sample_frames: signal.frames,
        source_audio_extent: signal.source_audio_extent,
        start_sample_frame: 0,
        presented_sample_frames: frames,
        sample_rate_hz: 48_000,
        channels: signal.channels,
        window_wav_sha256: selected_pin.0,
        window_wav_bytes: selected_pin.1,
        window_pcm_sha256: pcm_sha256,
        method: METHOD.into(),
        loudness,
        normalization: "none".into(),
        runtime_qualified: false,
    };
    validate_current(output, signal, &receipt, maximum, token)?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_float32_header_binds_the_exact_presented_count() {
        let bytes = header(96_000, 2).unwrap();
        assert_eq!(bytes.len(), 56);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 768_048);
        assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 3);
        assert_eq!(&bytes[36..40], b"fact");
        assert_eq!(
            u32::from_le_bytes(bytes[44..48].try_into().unwrap()),
            96_000
        );
        assert_eq!(
            u32::from_le_bytes(bytes[52..56].try_into().unwrap()),
            768_000
        );
        assert_eq!(projected_bytes(96_000, 2).unwrap(), 768_056);
    }
    #[test]
    fn invalid_or_overflowing_analysis_envelopes_refuse() {
        for (frames, channels) in [(0, 2), (1, 0), (1, 3), (u64::MAX, 2), (u32::MAX as u64, 2)] {
            assert!(header(frames, channels).is_err());
            assert!(projected_bytes(frames, channels).is_err());
        }
    }
    #[test]
    fn only_exact_zero_origin_sample_intervals_are_selected() {
        let range = |a, b, d| {
            TimeRange::new(
                RationalTime::new(a, d).unwrap(),
                RationalTime::new(b, d).unwrap(),
            )
            .unwrap()
        };
        assert_eq!(sample_count(range(0, 2, 1)).unwrap(), 96_000);
        assert!(sample_count(range(1, 96_001, 48_000)).is_err());
        assert!(sample_count(range(0, 1, 7)).is_err());
    }
    #[test]
    fn selection_has_one_explicit_closed_spelling() {
        assert_eq!(
            serde_json::to_value(PresentedWindowSelection::ExactPresentedAudio).unwrap(),
            serde_json::json!("exact_presented_audio")
        );
        for value in ["auto", "complete", "presented", "", "trim_tail"] {
            assert!(
                serde_json::from_value::<PresentedWindowSelection>(serde_json::json!(value))
                    .is_err()
            );
        }
    }
}
