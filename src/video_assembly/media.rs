//! Fixed FFmpeg 8.0.1 assembly and contemporary, bounded media observations.
//!
//! Interface evidence: FFmpeg tag n8.0.1, libavformat/movenc.c (FFV1 MP4
//! tag, sample-rate audio timebase and edit lists), libavcodec/ffv1enc.c
//! (version 3, RGB8), and fftools/ffprobe.c (packet/extradata hashes and skips).
//! These source-derived interfaces are not a native backend qualification.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::audio_inspection::AudioToolPin;
use crate::audio_inspection::process::{GroupPolicy, hash_regular, open_regular, run_tool_isolated_with_limits};
use crate::provider::require_sha256;
use crate::temporal::{RationalTime, StreamSelection, TemporalInspection};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use super::{AssemblyConfiguration, AssemblyStageKind};

const PROFILE: &str = "aniflow.ffmpeg-8.0.1-rgb8-cfr-aac-assembly/v1";
const METHOD: &str = "native_integer_timeline_rgb24_framehash_aac_packet_sha256_mp4_atoms";
const MAXIMUM_OBSERVATIONS: usize = 1_000_000;

pub(crate) struct FrameSet {
    pub directory: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub period: RationalTime,
    pub frame_count: usize,
    pub source_inspection: TemporalInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyMediaEvidence {
    pub profile: String,
    pub kind: AssemblyStageKind,
    pub method: String,
    pub temporal_inspection: TemporalInspection,
    pub source_audio_sha256: Option<String>,
    pub output_audio_sha256: Option<String>,
    /// Digest of the ordered, fully decoded RGB24 frame hashes, master only.
    pub decoded_rgb24_sha256: Option<String>,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub pixel_format: String,
    pub faststart: bool,
    pub native_qualified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AudioPacket {
    pts: RationalTime,
    dts: RationalTime,
    duration: RationalTime,
    size: u64,
    sha256: String,
    flags: String,
    skip: Option<SkipSamples>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SkipSamples {
    skip_samples: u64,
    discard_padding: u64,
    skip_reason: u64,
    discard_reason: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct AudioFacts {
    codec: String,
    profile: String,
    sample_rate: u32,
    channels: u64,
    channel_layout: String,
    extradata_size: u64,
    extradata_sha256: String,
    packets: Vec<AudioPacket>,
}

struct Observation {
    temporal: TemporalInspection,
    inventory: Value,
    audio: Option<AudioFacts>,
}

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }
fn media_error(error: anyhow::Error) -> Error { Error::from_anyhow(ErrorCategory::Media, error) }
fn strings(values: &[&str]) -> Vec<OsString> { values.iter().map(OsString::from).collect() }
fn digest<T: Serialize>(value: &T) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|_| refused("cannot serialize assembly evidence"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn run(tool: &AudioToolPin, id: &str, args: &[OsString], settings: &AssemblyConfiguration, scratch: &Path, cancellation: &CancellationToken) -> Result<Vec<u8>> {
    let capture = run_tool_isolated_with_limits(tool, id, args, &settings.tool_limits(), cancellation,
        GroupPolicy::Inherit, Some(scratch), settings.timeout_milliseconds, settings.maximum_capture_bytes as u64)
        .map_err(|error| refused(format!("assembly {id} failed ({:?}): {}", error.code, error.message)))?;
    if !capture.stderr.is_empty() { return Err(refused(format!("assembly {id} emitted an error diagnostic"))); }
    Ok(capture.stdout)
}

fn verify_versions(settings: &AssemblyConfiguration, scratch: &Path, cancellation: &CancellationToken) -> Result<()> {
    for (id, tool) in [("ffmpeg", &settings.ffmpeg), ("ffprobe", &settings.ffprobe)] {
        if tool.version != "8.0.1" { return Err(refused("assembly requires the exact FFmpeg/FFprobe 8.0.1 interface")); }
        let output = run(tool, id, &strings(&["-version"]), settings, scratch, cancellation)?;
        let text = std::str::from_utf8(&output).map_err(|_| refused("tool version banner is not UTF-8"))?;
        let first: Vec<_> = text.lines().next().unwrap_or_default().split_whitespace().take(3).collect();
        if first != [id, "version", "8.0.1"] { return Err(refused("tool banner differs from the exact configured 8.0.1 token")); }
    }
    Ok(())
}

fn probe(path: &Path, stream: Option<u32>, options: &[&str], settings: &AssemblyConfiguration, scratch: &Path, cancellation: &CancellationToken) -> Result<Value> {
    let mut args = strings(&["-v", "error", "-protocol_whitelist", "file,pipe"]);
    if let Some(index) = stream { args.extend(["-select_streams".into(), index.to_string().into()]); }
    args.extend(options.iter().map(OsString::from));
    args.extend(["-of".into(), "json".into(), "-i".into(), path.as_os_str().to_owned()]);
    let bytes = run(&settings.ffprobe, "ffprobe", &args, settings, scratch, cancellation)?;
    serde_json::from_slice(&bytes).map_err(|_| refused("bounded FFprobe observation is not complete JSON"))
}

fn integer(value: &Value, field: &str) -> Result<i64> {
    match value.get(field) {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse::<i64>().ok(),
        _ => None,
    }.ok_or_else(|| refused(format!("FFprobe {field} must be an observed integer")))
}
fn positive(value: &Value, field: &str) -> Result<u64> {
    let number = integer(value, field)?;
    u64::try_from(number).ok().filter(|n| *n > 0).ok_or_else(|| refused(format!("FFprobe {field} must be positive")))
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value.get(field).and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or_else(|| refused(format!("FFprobe {field} must be an observed bounded string")))
}
fn array<'a>(value: &'a Value, field: &str) -> Result<&'a [Value]> {
    let values = value.get(field).and_then(Value::as_array).ok_or_else(|| refused(format!("FFprobe {field} array is missing")))?;
    if values.len() > MAXIMUM_OBSERVATIONS { return Err(refused("FFprobe observation count exceeds its bound")); }
    Ok(values)
}
fn sha(value: &Value, field: &str) -> Result<String> {
    let hash = text(value, field)?.strip_prefix("SHA256:").ok_or_else(|| refused("FFprobe payload hash is not SHA256"))?;
    require_sha256(hash, "FFprobe SHA256")?;
    Ok(hash.to_owned())
}
fn stream(inventory: &Value, index: u32) -> Result<&Value> {
    let matches: Vec<_> = array(inventory, "streams")?.iter().filter(|s| integer(s, "index").ok() == Some(i64::from(index))).collect();
    if matches.len() != 1 { return Err(refused("FFprobe selected stream is missing or duplicated")); }
    Ok(matches[0])
}

fn skips(packet: &Value) -> Result<Option<SkipSamples>> {
    let Some(value) = packet.get("side_data_list") else { return Ok(None); };
    let entries = value.as_array().ok_or_else(|| refused("packet side data is malformed"))?;
    if entries.is_empty() { return Ok(None); }
    if entries.len() != 1 || text(&entries[0], "side_data_type")? != "Skip Samples" {
        return Err(refused("AAC packet contains unsupported side data; assembly will not discard it"));
    }
    let entry = &entries[0];
    let nonnegative = |field| -> Result<u64> {
        u64::try_from(integer(entry, field)?).map_err(|_| refused("AAC skip metadata cannot be negative"))
    };
    let skip = SkipSamples { skip_samples:nonnegative("skip_samples")?, discard_padding:nonnegative("discard_padding")?,
        skip_reason:nonnegative("skip_reason")?, discard_reason:nonnegative("discard_reason")? };
    if skip.skip_reason > 1 || skip.discard_reason > 1 { return Err(refused("AAC skip/discard reason is unsupported")); }
    Ok(Some(skip))
}

fn audio_facts(inventory: &Value, packets: &Value, index: u32) -> Result<AudioFacts> {
    let selected = stream(inventory, index)?;
    if text(selected, "codec_name")? != "aac" || text(selected, "profile")? != "LC" {
        return Err(refused("assembly audio passthrough currently supports AAC-LC with observed configuration"));
    }
    if selected.get("side_data_list").is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty())) {
        return Err(refused("AAC stream contains unsupported side data"));
    }
    let sample_rate = u32::try_from(positive(selected, "sample_rate")?).map_err(|_| refused("AAC sample rate is out of range"))?;
    if !(8000..=192000).contains(&sample_rate) { return Err(refused("AAC sample rate is outside this assembly profile")); }
    let time_base = RationalTime::parse(text(selected, "time_base")?).map_err(media_error)?;
    let entries = array(packets, "packets")?;
    if entries.is_empty() { return Err(refused("AAC passthrough requires observed packets")); }
    let mut observed = Vec::with_capacity(entries.len());
    for packet in entries {
        if integer(packet, "stream_index")? != i64::from(index) { return Err(refused("audio packet belongs to another stream")); }
        let pts = RationalTime::ticks(integer(packet, "pts")?, time_base).map_err(media_error)?;
        let dts = RationalTime::ticks(integer(packet, "dts")?, time_base).map_err(media_error)?;
        let duration = RationalTime::ticks(integer(packet, "duration")?, time_base).map_err(media_error)?;
        if pts != dts || duration <= RationalTime::ZERO { return Err(refused("AAC packets require equal PTS/DTS and positive durations")); }
        // MP4 uses a 1/sample_rate audio track timebase. Refuse rather than round.
        for time in [pts, dts, duration] {
            if time.checked_mul(u64::from(sample_rate)).map_err(media_error)?.denominator() != 1 {
                return Err(refused("AAC packet timing is not representable on its exact sample grid"));
            }
        }
        let flags = text(packet, "flags")?;
        if flags.len() > 16 || !flags.bytes().all(|b| b == b'_' || b.is_ascii_uppercase()) { return Err(refused("AAC packet flags are malformed")); }
        observed.push(AudioPacket { pts, dts, duration, size:positive(packet, "size")?, sha256:sha(packet, "data_hash")?,
            flags:flags.to_owned(), skip:skips(packet)? });
    }
    Ok(AudioFacts { codec:"aac".to_owned(), profile:"LC".to_owned(), sample_rate,
        channels:positive(selected, "channels")?, channel_layout:text(selected, "channel_layout")?.to_owned(),
        extradata_size:positive(selected, "extradata_size")?, extradata_sha256:sha(selected, "extradata_hash")?, packets:observed })
}

fn video_frame_profile(inventory: &Value, document: &Value, index: u32) -> Result<()> {
    let video = stream(inventory, index)?;
    for frame in array(document, "frames")? {
        if integer(frame, "stream_index")? != i64::from(index) || integer(frame, "interlaced_frame")? != 0
            || positive(frame, "width")? != positive(video, "width")?
            || positive(frame, "height")? != positive(video, "height")? {
            return Err(refused("decoded video changes geometry or contains interlaced frames"));
        }
        for field in ["crop_top", "crop_bottom", "crop_left", "crop_right"] {
            if integer(frame, field)? != 0 { return Err(refused("decoded video has unsupported crop metadata")); }
        }
        for field in ["pix_fmt", "sample_aspect_ratio", "color_range", "color_space", "color_primaries", "color_transfer"] {
            if text(frame, field)? != text(video, field)? {
                return Err(refused("decoded video changes its declared pixel/color/aspect profile"));
            }
        }
        if let Some(entries) = frame.get("side_data_list") {
            for entry in entries.as_array().ok_or_else(|| refused("decoded frame side data is malformed"))? {
                if text(entry, "side_data_type")? != "H.26[45] User Data Unregistered SEI message" {
                    return Err(refused("decoded video carries unsupported dynamic color/transform side data"));
                }
            }
        }
    }
    Ok(())
}

fn observe(path: &Path, selection: &StreamSelection, settings: &AssemblyConfiguration, scratch: &Path, cancellation: &CancellationToken, maximum: u64) -> Result<Observation> {
    let identity = hash_regular(path, maximum).map_err(|_| refused("media observation requires a bounded regular snapshot"))?;
    let inventory = probe(path, None, &["-show_streams", "-show_format", "-show_data_hash", "sha256"], settings, scratch, cancellation)?;
    let format = inventory.get("format").ok_or_else(|| refused("MP4 format observation is missing"))?;
    if text(format, "format_name")? != "mov,mp4,m4a,3gp,3g2,mj2" {
        return Err(refused("this assembly profile requires an MP4/MOV-family original and MP4 artifacts"));
    }
    let video = selection.video_stream.ok_or_else(|| refused("assembly requires an explicit selected video stream"))?;
    let audio = if selection.no_audio { None } else { selection.audio_stream };
    let mut observations = Vec::new();
    let mut audio_observation = None;
    for index in Some(video).into_iter().chain(audio) {
        let frames = probe(path, Some(index), &["-show_frames", "-show_entries", "frame=stream_index,pts,pkt_dts,duration,pkt_duration,key_frame,nb_samples,width,height,pix_fmt,sample_aspect_ratio,color_range,color_space,color_primaries,color_transfer,interlaced_frame,crop_top,crop_bottom,crop_left,crop_right:frame_side_data"], settings, scratch, cancellation)?;
        let packets = probe(path, Some(index), &["-show_packets", "-show_data_hash", "sha256", "-show_entries",
            "packet=stream_index,pts,dts,duration,size,flags,data_hash:packet_side_data"], settings, scratch, cancellation)?;
        if index == video { video_frame_profile(&inventory, &frames, index)?; }
        if Some(index) == audio { audio_observation = Some(audio_facts(&inventory, &packets, index)?); }
        observations.push((index, frames, packets));
    }
    let temporal = crate::temporal::from_probe_documents(&identity.0, identity.1, selection, &inventory, &observations)?;
    temporal.require_processing().map_err(media_error)?;
    if hash_regular(path, maximum).map_err(|_| refused("media snapshot changed during observation"))? != identity {
        return Err(refused("media snapshot changed during observation"));
    }
    Ok(Observation { temporal, inventory, audio:audio_observation })
}

fn admitted_source_video(source: &Observation) -> Result<()> {
    let index = source.temporal.selected.as_ref().and_then(|s| s.video).ok_or_else(|| refused("source video selection is unavailable"))?;
    let video = stream(&source.inventory, index)?;
    if !matches!(text(video, "pix_fmt")?, "rgb24" | "bgr24" | "bgr0" | "0rgb" | "gbrp" | "yuv420p" | "yuv422p" | "yuv444p") {
        return Err(refused("source pixel format is outside the closed opaque RGB8/8-bit SDR profile"));
    }
    if text(video, "sample_aspect_ratio")? != "1:1" || text(video, "color_primaries")? != "bt709"
        || text(video, "color_transfer")? != "bt709" || !matches!(text(video, "color_space")?, "bt709" | "gbr")
        || !matches!(text(video, "color_range")?, "tv" | "pc")
        || text(video, "field_order")? != "progressive" {
        return Err(refused("source must declare square, unrotated BT.709 SDR color; unsupported metadata is not normalized"));
    }
    if let Some(rotation) = video.get("tags").and_then(|tags| tags.get("rotate")) {
        if rotation.as_str() != Some("0") {
            return Err(refused("legacy rotation metadata is outside the unrotated source profile"));
        }
    }
    // Rotation, HDR, mastering metadata and unknown side data need distinct policies.
    if let Some(entries) = video.get("side_data_list") {
        for entry in entries.as_array().ok_or_else(|| refused("video side data is malformed"))? {
            if text(entry, "side_data_type")? != "CPB properties" {
                return Err(refused("source video contains unsupported rotation/color/other side data"));
            }
        }
    }
    Ok(())
}

fn validate_grid(observation: &Observation, frames: &FrameSet) -> Result<()> {
    let timeline = observation.temporal.video_timeline().map_err(media_error)?;
    if timeline.frames.len() != frames.frame_count || timeline.frame_period != Some(frames.period) {
        return Err(refused("assembled video count or exact CFR period differs from the frame index"));
    }
    for (ordinal, frame) in timeline.frames.iter().enumerate() {
        let start = frames.period.checked_mul(ordinal as u64).map_err(media_error)?;
        let end = frames.period.checked_mul(ordinal as u64 + 1).map_err(media_error)?;
        if frame.source_time.start != start || frame.source_time.end != end {
            return Err(refused("assembled video changed a frame's exact presentation interval"));
        }
    }
    Ok(())
}

fn validate_output(observation: &Observation, source: &Observation, frames: &FrameSet, kind: AssemblyStageKind) -> Result<()> {
    crate::temporal::validate_reconstruction(&source.temporal, &observation.temporal, false).map_err(media_error)?;
    validate_grid(observation, frames)?;
    if observation.audio != source.audio { return Err(refused("AAC passthrough changed packet payload, exact timing, configuration or skip/discard semantics")); }
    let video = stream(&observation.inventory, 0)?;
    let (codec, pixel_format, color_space, color_range) = match kind {
        AssemblyStageKind::Master => ("ffv1", "bgr0", "gbr", "pc"),
        AssemblyStageKind::Delivery => ("h264", "yuv420p", "bt709", "tv"),
    };
    if text(video, "codec_name")? != codec || text(video, "pix_fmt")? != pixel_format
        || positive(video, "width")? != u64::from(frames.width) || positive(video, "height")? != u64::from(frames.height)
        || text(video, "sample_aspect_ratio")? != "1:1" || text(video, "color_space")? != color_space
        || text(video, "color_range")? != color_range || text(video, "color_primaries")? != "bt709"
        || text(video, "color_transfer")? != "bt709" {
        return Err(refused("assembled codec, geometry, pixel format or explicit color metadata differs from its profile"));
    }
    if video.get("side_data_list").is_some_and(|entries| entries.as_array().is_none_or(|items| items.iter().any(|item|
        item.get("side_data_type").and_then(Value::as_str) != Some("CPB properties")))) {
        return Err(refused("assembled video contains unexpected side data"));
    }
    Ok(())
}

fn gcd(mut left: u64, mut right: u64) -> u64 { while right != 0 { (left, right) = (right, left % right); } left }
fn timescales(period: RationalTime, audio: Option<&AudioFacts>) -> Result<(u64, u64)> {
    let video = period.denominator();
    let sample_rate = audio.map_or(1, |facts| u64::from(facts.sample_rate));
    let movie = video.checked_div(gcd(video, sample_rate)).and_then(|value| value.checked_mul(sample_rate))
        .ok_or_else(|| refused("MP4 exact movie timescale overflow"))?;
    if video == 0 || video > i32::MAX as u64 || movie == 0 || movie > i32::MAX as u64 {
        return Err(refused("exact video/audio clock cannot fit MP4 timescale bounds"));
    }
    Ok((video, movie))
}

fn encode_arguments(kind: AssemblyStageKind, source: &Path, frames: &FrameSet, master: Option<&Path>, output: &Path, audio: Option<u32>, scales: (u64, u64), settings: &AssemblyConfiguration) -> Result<Vec<OsString>> {
    let mut args = strings(&["-hide_banner", "-nostdin", "-v", "error", "-xerror", "-n", "-copyts", "-copytb", "1", "-filter_threads", "1"]);
    match kind {
        AssemblyStageKind::Master => {
            let directory = frames.directory.as_ref().ok_or_else(|| refused("master assembly has no private frame sequence"))?;
            let parent = directory.to_str().ok_or_else(|| refused("frame sequence path is not UTF-8"))?.replace('%', "%%");
            args.extend(strings(&["-threads", "1", "-err_detect", "explode", "-protocol_whitelist", "file,pipe", "-f", "image2", "-pattern_type", "sequence", "-start_number", "0", "-start_number_range", "1", "-framerate"]));
            args.push(frames.period.reciprocal().map_err(media_error)?.to_string().into());
            args.extend(["-c:v".into(), "png".into(), "-i".into(), format!("{parent}/frame-%08d.png").into()]);
        }
        AssemblyStageKind::Delivery => {
            let input = master.ok_or_else(|| refused("delivery requires an accepted master snapshot"))?;
            args.extend(strings(&["-threads", "1", "-err_detect", "explode", "-protocol_whitelist", "file,pipe", "-noautorotate", "-i"]));
            args.push(input.as_os_str().to_owned());
        }
    }
    if audio.is_some() {
        args.extend(strings(&["-protocol_whitelist", "file,pipe", "-i"]));
        args.push(source.as_os_str().to_owned());
    }
    args.extend(strings(&["-map", "0:v:0"]));
    if let Some(index) = audio {
        args.extend(["-map".into(), format!("1:{index}").into(), "-c:a".into(), "copy".into(), "-copypriorss:a".into(), "1".into()]);
    } else { args.push("-an".into()); }
    args.extend(strings(&["-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1", "-fps_mode:v", "passthrough", "-enc_time_base:v"]));
    args.push(frames.period.to_string().into());
    match kind {
        AssemblyStageKind::Master => args.extend(strings(&["-vf", "scale=in_range=full:out_range=full,format=pix_fmts=bgr0,setsar=1", "-c:v", "ffv1", "-level:v", "3", "-slicecrc", "1", "-pix_fmt", "+bgr0", "-colorspace", "rgb", "-color_range", "pc"])),
        AssemblyStageKind::Delivery => {
            args.extend(strings(&["-vf", "scale=in_range=full:out_range=limited:out_color_matrix=bt709,format=pix_fmts=yuv420p,setsar=1", "-c:v", "libx264", "-pix_fmt", "+yuv420p", "-colorspace", "bt709", "-color_range", "tv", "-crf"]));
            args.push(settings.delivery_crf.to_string().into());
            args.extend(["-preset".into(), settings.preset.clone().into(), "-bf".into(), "0".into()]);
        }
    }
    args.extend(strings(&["-threads:v", "1", "-color_primaries", "bt709", "-color_trc", "bt709", "-avoid_negative_ts", "disabled", "-use_editlist", "1", "-video_track_timescale"]));
    args.push(scales.0.to_string().into());
    args.extend(["-movie_timescale".into(), scales.1.to_string().into()]);
    args.extend(strings(&["-movflags", "+faststart", "-f", "mp4"]));
    args.push(output.as_os_str().to_owned());
    // Deliberately no -shortest, -t, -ss, -start_at_zero, padding or audio filter.
    // The confined, complete image sequence ends at its actual EOF.
    Ok(args)
}

fn full_decode(path: &Path, has_audio: bool, settings: &AssemblyConfiguration, scratch: &Path, cancellation: &CancellationToken) -> Result<()> {
    let mut args = strings(&["-hide_banner", "-nostdin", "-v", "error", "-xerror", "-err_detect", "explode", "-threads", "1", "-protocol_whitelist", "file,pipe", "-noautorotate", "-i"]);
    args.push(path.as_os_str().to_owned());
    args.extend(strings(&["-map", "0:v:0"]));
    if has_audio { args.extend(strings(&["-map", "0:a:0"])); }
    args.extend(strings(&["-sn", "-dn", "-f", "null", "-"]));
    run(&settings.ffmpeg, "ffmpeg", &args, settings, scratch, cancellation)?;
    Ok(())
}

fn parse_framehash(bytes: &[u8], frames: &FrameSet) -> Result<Vec<String>> {
    let input = std::str::from_utf8(bytes).map_err(|_| refused("RGB framehash is not UTF-8"))?;
    let required = ["#format: frame checksums".to_owned(), "#version: 2".to_owned(), "#hash: SHA256".to_owned(),
        "#media_type 0: video".to_owned(), "#codec_id 0: rawvideo".to_owned(), format!("#dimensions 0: {}x{}", frames.width, frames.height)];
    let mut headers = BTreeSet::new();
    let mut time_base = None;
    let mut hashes = Vec::new();
    let frame_bytes = u64::from(frames.width).checked_mul(u64::from(frames.height)).and_then(|n| n.checked_mul(3)).ok_or_else(|| refused("RGB frame size overflow"))?;
    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with('#') {
            if !hashes.is_empty() { return Err(refused("RGB framehash header appeared after frame data")); }
            let key = line.split(':').next().unwrap_or(line);
            if !headers.insert(key.to_owned()) { return Err(refused("duplicate RGB framehash header")); }
            if let Some(value) = line.strip_prefix("#tb 0:") {
                time_base = Some(RationalTime::parse(value.trim()).map_err(media_error)?);
            } else if !(required.iter().any(|header| header == line) || line.starts_with("#software:")
                || line == "#sar 0: 1/1" || line.strip_prefix('#').is_some_and(|value| value.split(',').map(str::trim).eq(["stream#", "dts", "pts", "duration", "size", "hash"]))) {
                return Err(refused("unsupported RGB framehash header"));
            }
            continue;
        }
        let fields: Vec<_> = line.split(',').map(str::trim).collect();
        if fields.len() != 6 || fields[0] != "0" || hashes.len() >= frames.frame_count {
            return Err(refused("RGB framehash contains unexpected or extra frames"));
        }
        let parse = |field: &str| field.parse::<i64>().map_err(|_| refused("RGB framehash timing is not integer"));
        let tb = time_base.ok_or_else(|| refused("RGB framehash has no timebase"))?;
        let start = frames.period.checked_mul(hashes.len() as u64).map_err(media_error)?;
        if RationalTime::ticks(parse(fields[1])?, tb).map_err(media_error)? != start
            || RationalTime::ticks(parse(fields[2])?, tb).map_err(media_error)? != start
            || RationalTime::ticks(parse(fields[3])?, tb).map_err(media_error)? != frames.period
            || fields[4].parse::<u64>().ok() != Some(frame_bytes) {
            return Err(refused("RGB framehash frame clock or complete decoded size differs"));
        }
        require_sha256(fields[5], "decoded master RGB24 SHA256")?;
        hashes.push(fields[5].to_owned());
    }
    if hashes.len() != frames.frame_count || required.iter().any(|header| !input.lines().map(str::trim).any(|line| line == header)) {
        return Err(refused("RGB framehash profile or full frame count differs"));
    }
    Ok(hashes)
}

fn master_hashes(path: &Path, frames: &FrameSet, settings: &AssemblyConfiguration, scratch: &Path, cancellation: &CancellationToken) -> Result<Vec<String>> {
    let mut args = strings(&["-hide_banner", "-nostdin", "-v", "error", "-xerror", "-err_detect", "explode", "-threads", "1", "-copyts", "-protocol_whitelist", "file,pipe", "-noautorotate", "-i"]);
    args.push(path.as_os_str().to_owned());
    args.extend(strings(&["-map", "0:v:0", "-an", "-sn", "-dn", "-vf", "scale=in_range=full:out_range=full,format=pix_fmts=rgb24", "-c:v", "rawvideo", "-pix_fmt", "+rgb24", "-threads", "1", "-fps_mode:v", "passthrough", "-enc_time_base:v"]));
    args.push(frames.period.to_string().into());
    args.extend(strings(&["-f", "framehash", "-format_version", "2", "-hash", "sha256", "pipe:1"]));
    parse_framehash(&run(&settings.ffmpeg, "ffmpeg", &args, settings, scratch, cancellation)?, frames)
}

/// Read only atom headers and seek over payloads; no whole-media allocation.
fn faststart(path: &Path, maximum: u64) -> Result<bool> {
    let mut file = open_regular(path).map_err(|_| refused("MP4 atom scan requires a regular snapshot"))?;
    let length = file.metadata().map_err(|_| refused("cannot inspect MP4 size"))?.len();
    if length == 0 || length > maximum { return Err(refused("MP4 size is outside its bound")); }
    let mut offset = 0_u64;
    let mut moov = None;
    let mut mdat = None;
    let mut boxes = 0_u32;
    while offset < length {
        boxes = boxes.checked_add(1).ok_or_else(|| refused("MP4 atom count overflow"))?;
        if boxes > 1_000_000 || length - offset < 8 { return Err(refused("MP4 atom inventory is malformed or exceeds its bound")); }
        file.seek(SeekFrom::Start(offset)).map_err(|_| refused("cannot seek MP4 atom"))?;
        let mut header = [0_u8; 8];
        file.read_exact(&mut header).map_err(|_| refused("truncated MP4 atom header"))?;
        let short = u32::from_be_bytes(header[..4].try_into().expect("four bytes"));
        let (size, header_size) = match short {
            0 => (length - offset, 8),
            1 => {
                let mut extended = [0_u8; 8];
                file.read_exact(&mut extended).map_err(|_| refused("truncated extended MP4 atom header"))?;
                (u64::from_be_bytes(extended), 16)
            }
            size => (u64::from(size), 8),
        };
        if size < header_size || size > length - offset { return Err(refused("MP4 atom extends outside its complete snapshot")); }
        match &header[4..8] {
            b"moov" => { if moov.replace(offset).is_some() { return Err(refused("MP4 contains multiple movie headers")); } }
            b"mdat" => { mdat.get_or_insert(offset); }
            b"moof" => return Err(refused("fragmented MP4 is outside the fixed faststart output profile")),
            _ => (),
        }
        offset += size;
    }
    Ok(match (moov, mdat) { (Some(movie), Some(data)) => movie < data, _ => return Err(refused("MP4 movie or media atom is missing")) })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute(kind: AssemblyStageKind, settings: &AssemblyConfiguration, source: &Path, frames: &FrameSet, master: Option<&Path>, output: &Path, scratch: &Path, cancellation: &CancellationToken) -> Result<AssemblyMediaEvidence> {
    verify_versions(settings, scratch, cancellation)?;
    let expected = &frames.source_inspection;
    expected.require_processing().map_err(media_error)?;
    let selected = expected.selected.as_ref().ok_or_else(|| refused("bound source selection is missing"))?;
    let selection = StreamSelection { video_stream:selected.video, audio_stream:selected.audio,
        no_audio:selected.audio.is_none(), discard_streams:selected.explicitly_discarded.clone() };
    let mut source_observation = observe(source, &selection, settings, scratch, cancellation, settings.maximum_source_bytes)?;
    // Selection is made explicit for execution; compare evidence under the original
    // recorded intent so that an automatic but unambiguous import remains usable.
    source_observation.temporal.selection_intent = expected.selection_intent.clone();
    source_observation.temporal.selected = expected.selected.clone();
    source_observation.temporal.processing = crate::temporal::assess_processing(&source_observation.temporal).map_err(media_error)?;
    if &source_observation.temporal != expected { return Err(refused("fresh source media evidence differs from the bound frame index")); }
    admitted_source_video(&source_observation)?;
    validate_grid(&source_observation, frames)?;
    if frames.width == 0 || frames.height == 0 || frames.width % 2 != 0 || frames.height % 2 != 0 || frames.frame_count == 0 {
        return Err(refused("assembly requires nonempty frames with even output dimensions for yuv420p delivery"));
    }
    let scales = timescales(frames.period, source_observation.audio.as_ref())?;
    let output_selection = StreamSelection { video_stream:Some(0), audio_stream:selected.audio.map(|_| 1), no_audio:selected.audio.is_none(), discard_streams:Vec::new() };
    let mut expected_hashes = None;
    match kind {
        AssemblyStageKind::Master => {
            let directory = frames.directory.as_ref().ok_or_else(|| refused("master frame sequence is missing"))?;
            let mut hashes = Vec::with_capacity(frames.frame_count);
            for ordinal in 0..frames.frame_count {
                if cancellation.is_cancelled() { return Err(refused("assembly cancelled during complete PNG decoding")); }
                hashes.push(crate::frame_decode::decode(&settings.ffmpeg, &settings.tool_limits(), cancellation, scratch,
                    &directory.join(format!("frame-{ordinal:08}.png")), frames.width, frames.height)?);
            }
            expected_hashes = Some(hashes);
        }
        AssemblyStageKind::Delivery => {
            let master = master.ok_or_else(|| refused("delivery master is missing"))?;
            let observation = observe(master, &output_selection, settings, scratch, cancellation, settings.maximum_output_bytes)?;
            validate_output(&observation, &source_observation, frames, AssemblyStageKind::Master)?;
        }
    }
    let args = encode_arguments(kind, source, frames, master, output, selected.audio, scales, settings)?;
    run(&settings.ffmpeg, "ffmpeg", &args, settings, scratch, cancellation)?;
    let output_observation = observe(output, &output_selection, settings, scratch, cancellation, settings.maximum_output_bytes)?;
    validate_output(&output_observation, &source_observation, frames, kind)?;
    full_decode(output, selected.audio.is_some(), settings, scratch, cancellation)?;
    let decoded_rgb24_sha256 = if let Some(expected_hashes) = expected_hashes {
        let actual = master_hashes(output, frames, settings, scratch, cancellation)?;
        if actual != expected_hashes { return Err(refused("lossless master decoded pixels differ from the complete input PNG sequence")); }
        Some(digest(&actual)?)
    } else { None };
    let faststart = faststart(output, settings.maximum_output_bytes)?;
    if !faststart { return Err(refused("MP4 faststart movie header does not precede media data")); }
    verify_versions(settings, scratch, cancellation)?;
    let source_audio_sha256 = source_observation.audio.as_ref().map(digest).transpose()?;
    let output_audio_sha256 = output_observation.audio.as_ref().map(digest).transpose()?;
    let (video_codec, pixel_format) = match kind { AssemblyStageKind::Master => ("ffv1", "bgr0"), AssemblyStageKind::Delivery => ("h264", "yuv420p") };
    Ok(AssemblyMediaEvidence { profile:PROFILE.to_owned(), kind, method:METHOD.to_owned(), temporal_inspection:output_observation.temporal,
        source_audio_sha256, output_audio_sha256, decoded_rgb24_sha256, frame_count:frames.frame_count as u64,
        width:frames.width, height:frames.height, video_codec:video_codec.to_owned(), pixel_format:pixel_format.to_owned(), faststart, native_qualified:false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frames() -> FrameSet {
        let selection = StreamSelection { video_stream:Some(0), no_audio:true, ..StreamSelection::default() };
        let inventory = json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264","time_base":"1/30","start_pts":0,"duration_ts":2,"r_frame_rate":"30/1","avg_frame_rate":"30/1","width":2,"height":2,"pix_fmt":"yuv420p"}]});
        let decoded = json!({"frames":[{"stream_index":0,"pts":0,"pkt_dts":0,"duration":1,"key_frame":1},{"stream_index":0,"pts":1,"pkt_dts":1,"duration":1,"key_frame":0}]});
        let packets = json!({"packets":[{"stream_index":0,"pts":0,"dts":0,"duration":1},{"stream_index":0,"pts":1,"dts":1,"duration":1}]});
        let temporal = crate::temporal::from_probe_documents(&"a".repeat(64), 20, &selection, &inventory, &[(0,decoded,packets)]).expect("synthetic temporal observation");
        FrameSet { directory:Some(PathBuf::from("/private/frames")), width:2, height:2, period:RationalTime::new(1,30).expect("period"), frame_count:2, source_inspection:temporal }
    }

    fn configuration() -> AssemblyConfiguration {
        serde_json::from_value(json!({"schema":super::super::ASSEMBLY_CONFIGURATION_SCHEMA_V1,
            "ffmpeg":{"executable":"/private/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},
            "ffprobe":{"executable":"/private/ffprobe","version":"8.0.1","sha256":"b".repeat(64)}})).expect("configuration")
    }

    fn audio_documents() -> (Value, Value) {
        let inventory = json!({"streams":[{"index":1,"codec_name":"aac","profile":"LC","sample_rate":"48000","time_base":"1/48000","channels":2,"channel_layout":"stereo","extradata_size":2,"extradata_hash":format!("SHA256:{}","c".repeat(64))}]});
        let packets = json!({"packets":[
            {"stream_index":1,"pts":-1024,"dts":-1024,"duration":1024,"size":"20","flags":"KD","data_hash":format!("SHA256:{}","a".repeat(64)),"side_data_list":[{"side_data_type":"Skip Samples","skip_samples":1024,"discard_padding":0,"skip_reason":0,"discard_reason":0}]},
            {"stream_index":1,"pts":0,"dts":0,"duration":1024,"size":"21","flags":"K_","data_hash":format!("SHA256:{}","b".repeat(64))}
        ]});
        (inventory,packets)
    }

    #[test]
    fn rgb_hash_observes_every_frame_clock_size_and_payload() {
        let hashes = format!("#format: frame checksums\n#version: 2\n#hash: SHA256\n#tb 0: 1/30\n#media_type 0: video\n#codec_id 0: rawvideo\n#dimensions 0: 2x2\n#sar 0: 1/1\n#stream#, dts, pts, duration, size, hash\n0, 0, 0, 1, 12, {}\n0, 1, 1, 1, 12, {}\n", "a".repeat(64), "b".repeat(64));
        assert_eq!(parse_framehash(hashes.as_bytes(), &frames()).expect("complete hashes"), vec!["a".repeat(64), "b".repeat(64)]);
        for invalid in [hashes.replace("0, 1, 1, 1, 12", "0, 1, 0, 1, 12"), hashes.replace("1, 12,", "1, 11,"), hashes.replace("#dimensions 0: 2x2", "#dimensions 0: 4x2"), format!("{hashes}0, 2, 2, 1, 12, {}\n", "c".repeat(64))] {
            assert!(parse_framehash(invalid.as_bytes(), &frames()).is_err());
        }
    }

    #[test]
    fn aac_identity_preserves_negative_priming_and_exact_rational_clock() {
        let (inventory, packets) = audio_documents();
        let expected = audio_facts(&inventory, &packets, 1).expect("AAC priming");
        assert_eq!(expected.packets[0].pts, RationalTime::new(-8,375).expect("negative priming"));
        let mut rescaled_inventory = inventory.clone();
        rescaled_inventory["streams"][0]["time_base"] = json!("1/96000");
        let mut rescaled_packets = packets.clone();
        for packet in rescaled_packets["packets"].as_array_mut().expect("packets") {
            for field in ["pts","dts","duration"] { packet[field] = json!(packet[field].as_i64().expect("integer") * 2); }
        }
        assert_eq!(audio_facts(&rescaled_inventory, &rescaled_packets, 1).expect("equivalent exact grid"), expected);
        let mut changed = packets.clone();
        changed["packets"][0]["side_data_list"][0]["skip_samples"] = json!(0);
        assert_ne!(audio_facts(&inventory, &changed, 1).expect("different skips"), expected);
        changed = packets.clone();
        changed["packets"][1]["data_hash"] = json!(format!("SHA256:{}","d".repeat(64)));
        assert_ne!(audio_facts(&inventory, &changed, 1).expect("different payload"), expected);
        changed = packets;
        changed["packets"][1]["pts"] = json!(0.25);
        assert!(audio_facts(&inventory, &changed, 1).is_err());
    }

    #[test]
    fn fixed_argv_copies_original_audio_without_shortening_or_retiming() {
        let settings = configuration();
        for kind in [AssemblyStageKind::Master, AssemblyStageKind::Delivery] {
            let args = encode_arguments(kind, Path::new("/private/original.mp4"), &frames(), Some(Path::new("/private/master.mp4")), Path::new("/private/output.mp4"), Some(7), (30,48000), &settings).expect("fixed argv");
            let args: Vec<_> = args.iter().map(|arg| arg.to_str().expect("UTF-8 fixture")).collect();
            for pair in [["-map","1:7"],["-c:a","copy"],["-copypriorss:a","1"],["-avoid_negative_ts","disabled"],["-movie_timescale","48000"],["-enc_time_base:v","1/30"]] {
                assert!(args.windows(2).any(|window| window == pair));
            }
            for forbidden in ["-shortest","-t","-ss","-start_at_zero","-frames:v","-af","-r"] { assert!(!args.contains(&forbidden)); }
            assert!(args.contains(&"/private/original.mp4"));
            if kind == AssemblyStageKind::Delivery { assert!(args.contains(&"/private/master.mp4")); }
        }
    }

    #[test]
    fn mp4_timescales_preserve_fractional_video_and_audio_sample_grid() {
        let (inventory,packets) = audio_documents();
        let audio = audio_facts(&inventory,&packets,1).expect("audio");
        assert_eq!(timescales(RationalTime::new(1001,30000).expect("NTSC period"),Some(&audio)).expect("exact scales"),(30000,240000));
        assert!(timescales(RationalTime::new(1,2_147_483_647).expect("large denominator"),Some(&audio)).is_err());
    }
}
