//! Fixed source-video conversion and contemporary lineage observations.
//!
//! Interface source: FFmpeg n8.0.1 doc/ffmpeg.texi (passthrough timestamps,
//! explicit encoder timebase), doc/muxers.texi (image2 sequence numbers),
//! doc/filters.texi (scale range/matrix) and libavcodec/pngenc.c (BT.709 cICP).
//! These source-derived interfaces are not a native backend qualification.
use std::ffi::OsString;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::audio_inspection::process::{GroupPolicy, run_tool_isolated_with_limits};
use crate::frame_io::{identity, inspect_png, inventory};
use crate::temporal::TemporalInspection;
use crate::video_assembly::media::{FrameSet, SourceExtractionObservation, observe_extraction_source, parse_framehash};
use crate::video_interpolation::{RifeFrame, RifeFrameIndex, RIFE_FRAME_INDEX_SCHEMA_V1};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use super::FrameExtractionConfiguration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameExtractionMediaEvidence {
    pub profile: String,
    pub method: String,
    pub temporal_inspection: TemporalInspection,
    pub decoded_rgb24_sha256: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub source_pixel_format: String,
    pub source_color_range: String,
    pub conversion: String,
    pub native_qualified: bool,
}

fn refused(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Media, message) }
fn media_error(error: anyhow::Error) -> Error { Error::from_anyhow(ErrorCategory::Media, error) }
fn strings(values: &[&str]) -> Vec<OsString> { values.iter().map(OsString::from).collect() }

fn conversion(source: &SourceExtractionObservation) -> Result<String> {
    let range = match source.color_range.as_str() {
        "tv" => "limited", "pc" => "full", _ => return Err(refused("source range is outside the extraction profile")),
    };
    Ok(format!("scale=in_color_matrix=bt709:out_color_matrix=bt709:in_range={range}:out_range=full:flags=accurate_rnd+bitexact,format=pix_fmts=rgb24,setsar=1"))
}

fn source_arguments(source: &Path, observation: &SourceExtractionObservation,
    settings: &FrameExtractionConfiguration) -> Result<Vec<OsString>> {
    let selected = observation.temporal.selected.as_ref().and_then(|value| value.video)
        .ok_or_else(|| refused("extraction source video selection is missing"))?;
    let period = observation.temporal.video_timeline().map_err(media_error)?.frame_period
        .ok_or_else(|| refused("extraction source has no exact CFR period"))?;
    let mut args = strings(&["-hide_banner", "-nostdin", "-v", "error", "-xerror", "-n", "-copyts",
        "-filter_threads", "1", "-threads", "1", "-err_detect", "explode", "-protocol_whitelist", "file,pipe",
        "-noautorotate", "-apply_cropping", "none", "-i"]);
    args.push(source.as_os_str().to_owned());
    args.extend(["-map".into(), format!("0:{selected}").into()]);
    args.extend(strings(&["-an", "-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1", "-noautoscale", "-vf"]));
    args.push(conversion(observation)?.into());
    args.extend(strings(&["-pix_fmt", "+rgb24", "-threads:v", "1", "-fps_mode:v", "passthrough", "-enc_time_base:v"]));
    args.push(period.to_string().into());
    // One extra frame beyond the entire admitted bound is a refusal sentinel,
    // never an accepted truncation. Exact source/framehash counts must agree.
    args.extend(["-frames:v".into(), (settings.maximum_frames + 1).to_string().into()]);
    Ok(args)
}

fn extraction_arguments(source: &Path, observation: &SourceExtractionObservation,
    directory: &Path, settings: &FrameExtractionConfiguration) -> Result<Vec<OsString>> {
    let mut args = source_arguments(source, observation, settings)?;
    args.extend(strings(&["-c:v", "png", "-compression_level", "6", "-pred", "mixed",
        "-colorspace", "rgb", "-color_range", "pc", "-color_primaries", "bt709", "-color_trc", "bt709",
        "-f", "image2", "-start_number", "0", "-atomic_writing", "1"]));
    let directory = directory.to_str().ok_or_else(|| refused("extraction output path is not UTF-8"))?.replace('%', "%%");
    args.push(format!("{directory}/frame-%08d.png").into());
    Ok(args)
}

fn hash_arguments(source: &Path, observation: &SourceExtractionObservation,
    settings: &FrameExtractionConfiguration) -> Result<Vec<OsString>> {
    let mut args = source_arguments(source, observation, settings)?;
    args.extend(strings(&["-c:v", "rawvideo", "-f", "framehash", "-format_version", "2", "-hash", "sha256", "pipe:1"]));
    Ok(args)
}

fn run(args: &[OsString], settings: &FrameExtractionConfiguration, scratch: &Path,
    cancellation: &CancellationToken) -> Result<Vec<u8>> {
    let capture = run_tool_isolated_with_limits(&settings.ffmpeg, "ffmpeg", args, &settings.tool_limits(),
        cancellation, GroupPolicy::Inherit, Some(scratch), settings.timeout_milliseconds,
        settings.maximum_capture_bytes as u64)
        .map_err(|error| refused(format!("source extraction failed ({:?}): {}", error.code, error.message)))?;
    if !capture.stderr.is_empty() { return Err(refused("source extraction emitted an error diagnostic")); }
    Ok(capture.stdout)
}

/// Generate the complete selected source sequence, then fully decode every PNG
/// and compare its RGB24 bytes to the same conversion at the original clock.
/// The runtime still owns output promotion, validation and accepted checkpoints.
pub(crate) fn execute(settings: &FrameExtractionConfiguration, source: &Path, directory: &Path,
    scratch: &Path, cancellation: &CancellationToken) -> Result<(RifeFrameIndex, FrameExtractionMediaEvidence)> {
    let assembly = settings.assembly_settings();
    let observation = observe_extraction_source(&assembly, source, &settings.source_selection, scratch, cancellation)?;
    let timeline = observation.temporal.video_timeline().map_err(media_error)?;
    let count = timeline.frames.len();
    let pixels = u64::from(observation.width).checked_mul(u64::from(observation.height))
        .ok_or_else(|| refused("source pixel count overflow"))?;
    if count == 0 || count as u64 > settings.maximum_frames || pixels > settings.maximum_pixels {
        return Err(refused("observed source exceeds the configured frame count or pixel bound"));
    }
    let frames = FrameSet { directory:None, width:observation.width, height:observation.height,
        period:timeline.frame_period.ok_or_else(|| refused("source frame period is missing"))?,
        frame_count:count, source_inspection:observation.temporal.clone() };
    // PNG rows include filter bytes and DEFLATE/chunk overhead. This deliberately
    // generous bound is a policy admission envelope, not a compression promise.
    // It can refuse an input that might compress below the configured ceiling.
    let png_ceiling = pixels.checked_mul(3).and_then(|bytes| bytes.checked_add(u64::from(observation.height)))
        .and_then(|bytes| bytes.checked_mul(2)).and_then(|bytes| bytes.checked_add(65_536))
        .ok_or_else(|| refused("source extraction PNG envelope overflow"))?;
    let sequence_ceiling = png_ceiling.checked_mul(count as u64)
        .ok_or_else(|| refused("source extraction sequence envelope overflow"))?;
    if png_ceiling > settings.maximum_frame_bytes || sequence_ceiling > settings.maximum_total_frame_bytes {
        return Err(refused("observed source RGB/PNG envelope exceeds configured per-frame or total byte bounds; raise explicit supported bounds or use a smaller source"));
    }
    let source_bytes = identity(source, settings.maximum_source_bytes)?.1;
    let components = super::observe_components(settings)?;
    let tool_bytes = components[0].1.checked_add(components[1].1).ok_or_else(|| refused("tool byte count overflow"))?;
    let required = super::checked_working_bytes(settings, source_bytes, tool_bytes)?
        .checked_add(sequence_ceiling.checked_mul(2).ok_or_else(|| refused("extraction frame copy bound overflow"))?)
        .filter(|bytes| *bytes <= settings.maximum_working_bytes)
        .ok_or_else(|| refused("observed source extraction copies exceed maximum_working_bytes"))?;
    super::require_storage(scratch, required, settings.minimum_free_bytes)?;
    // Full-source decode also precedes candidate creation. Its exact clock,
    // cardinality and decoded sizes are independently compared with the probe.
    let expected = parse_framehash(&run(&hash_arguments(source, &observation, settings)?, settings, scratch, cancellation)?, &frames)?;
    std::fs::create_dir(directory).map_err(|_| Error::new(ErrorCategory::Io, "cannot exclusively create private extracted frames"))?;
    run(&extraction_arguments(source, &observation, directory, settings)?, settings, scratch, cancellation)?;
    let names: Vec<OsString> = (0..count).map(|position| format!("frame-{position:08}.png").into()).collect();
    inventory(directory, &names)?;
    let mut index_frames = Vec::with_capacity(count);
    let mut bytes = 0_u64;
    let mut identities = Vec::with_capacity(count);
    for (ordinal, expected_hash) in expected.iter().enumerate() {
        if cancellation.is_cancelled() { return Err(refused("source extraction cancelled during complete PNG verification")); }
        let filename = format!("frame-{ordinal:08}.png");
        let path = directory.join(&filename);
        let observed = inspect_png(&path, observation.width, observation.height, settings.maximum_frame_bytes)?;
        bytes = bytes.checked_add(observed.1).filter(|value| *value <= settings.maximum_total_frame_bytes)
            .ok_or_else(|| refused("extracted frames exceed the aggregate configured byte bound"))?;
        let decoded = crate::frame_decode::decode(&settings.ffmpeg, &settings.tool_limits(), cancellation,
            scratch, &path, observation.width, observation.height)?;
        if decoded != *expected_hash || identity(&path, settings.maximum_frame_bytes)? != observed {
            return Err(refused("extracted PNG differs from the complete selected source RGB24 conversion"));
        }
        index_frames.push(RifeFrame { ordinal:ordinal as u64, filename, sha256:observed.0.clone() });
        identities.push(observed);
    }
    let fresh = observe_extraction_source(&assembly, source, &settings.source_selection, scratch, cancellation)?;
    if fresh.temporal != observation.temporal || fresh.width != observation.width || fresh.height != observation.height
        || fresh.pixel_format != observation.pixel_format || fresh.color_range != observation.color_range {
        return Err(refused("source media observations changed during extraction"));
    }
    inventory(directory, &names)?;
    for (frame, observed) in index_frames.iter().zip(&identities) {
        if identity(&directory.join(&frame.filename), settings.maximum_frame_bytes)? != *observed {
            return Err(refused("verified extracted frame changed before indexing"));
        }
    }
    let decoded_rgb24_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&expected)
        .map_err(|_| refused("cannot serialize ordered decoded source hashes"))?));
    let index = RifeFrameIndex { schema:RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection:observation.temporal.clone(),
        width:observation.width, height:observation.height, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(),
        frames:index_frames, cut_before_ordinals:Vec::new() };
    index.validate()?;
    let evidence = FrameExtractionMediaEvidence {
        profile:"aniflow.ffmpeg-8.0.1-rgb8-cfr-source-extraction/v1".to_owned(),
        method:"native_integer_timeline_rgb24_source_and_png_framehash".to_owned(),
        temporal_inspection:observation.temporal, decoded_rgb24_sha256, width:observation.width, height:observation.height,
        frame_count:count as u64, source_pixel_format:observation.pixel_format, source_color_range:observation.color_range,
        conversion:"explicit_bt709_source_to_full_range_rgb24".to_owned(), native_qualified:false,
    };
    Ok((index, evidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn configuration() -> FrameExtractionConfiguration {
        serde_json::from_value(json!({"schema":super::super::FRAME_EXTRACTION_CONFIGURATION_SCHEMA_V1,
            "ffmpeg":{"executable":"/private/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},
            "ffprobe":{"executable":"/private/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},
            "source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]}})).expect("configuration")
    }

    fn observation() -> SourceExtractionObservation {
        let selection = crate::temporal::StreamSelection { video_stream:Some(0), no_audio:true, ..Default::default() };
        let inventory = json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264","time_base":"1/30","start_pts":0,"duration_ts":2,"r_frame_rate":"30/1","avg_frame_rate":"30/1","width":2,"height":2,"pix_fmt":"yuv420p"}]});
        let frames = json!({"frames":[{"stream_index":0,"pts":0,"pkt_dts":0,"duration":1,"key_frame":1},{"stream_index":0,"pts":1,"pkt_dts":1,"duration":1,"key_frame":0}]});
        let packets = json!({"packets":[{"stream_index":0,"pts":0,"dts":0,"duration":1},{"stream_index":0,"pts":1,"dts":1,"duration":1}]});
        let temporal = crate::temporal::from_probe_documents(&"c".repeat(64), 20, &selection, &inventory, &[(0,frames,packets)]).expect("synthetic temporal");
        SourceExtractionObservation { temporal, width:2, height:2, pixel_format:"yuv420p".to_owned(), color_range:"tv".to_owned() }
    }

    #[test]
    fn extraction_and_source_hash_share_conversion_stream_and_clock() {
        let settings = configuration();
        let source = observation();
        let png = extraction_arguments(Path::new("/private/source.mp4"), &source, Path::new("/private/percent%/frames"), &settings).expect("PNG arguments");
        let hashes = hash_arguments(Path::new("/private/source.mp4"), &source, &settings).expect("hash arguments");
        let common = source_arguments(Path::new("/private/source.mp4"), &source, &settings).expect("common arguments");
        assert!(png.starts_with(&common));
        assert!(hashes.starts_with(&common));
        let args: Vec<_> = png.iter().map(|value| value.to_str().expect("fixture argument")).collect();
        for pair in [["-map","0:0"],["-fps_mode:v","passthrough"],["-enc_time_base:v","1/30"],["-frames:v","100001"],["-start_number","0"],["-pix_fmt","+rgb24"]] {
            assert!(args.windows(2).any(|window| window == pair));
        }
        assert_eq!(args.last().copied(),Some("/private/percent%%/frames/frame-%08d.png"));
        for forbidden in ["-r","-ss","-t","-shortest","-start_at_zero"] { assert!(!args.contains(&forbidden)); }
    }

    #[test]
    fn source_range_conversion_is_explicit_without_resizing_or_retiming() {
        let mut source = observation();
        let limited = conversion(&source).expect("limited");
        assert!(limited.contains("in_range=limited:out_range=full"));
        source.color_range = "pc".to_owned();
        assert!(conversion(&source).expect("full").contains("in_range=full:out_range=full"));
        source.color_range = "unknown".to_owned();
        assert!(conversion(&source).is_err());
    }
}
