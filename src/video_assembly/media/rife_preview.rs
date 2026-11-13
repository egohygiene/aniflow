//! Preview-only PNG encoding through the existing pinned native media boundary.
use super::*;
use crate::video_interpolation::preview::movie::{MovieMedia, MovieOperation};

pub(crate) fn arguments(
    operation: &MovieOperation,
    settings: &AssemblyConfiguration,
    directory: &Path,
    output: &Path,
) -> Result<Vec<OsString>> {
    let parent = directory
        .to_str()
        .ok_or_else(|| refused("preview PNG sequence path must be UTF-8"))?
        .replace('%', "%%");
    let period = operation
        .source
        .frame_rate
        .reciprocal()
        .map_err(media_error)?;
    let scales = timescales(period, None)?;
    let mut args = strings(&[
        "-hide_banner",
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-n",
        "-filter_threads",
        "1",
        "-threads",
        "1",
        "-err_detect",
        "explode",
        "-protocol_whitelist",
        "file,pipe",
        "-f",
        "image2",
        "-pattern_type",
        "sequence",
        "-start_number",
        "0",
        "-start_number_range",
        "1",
        "-framerate",
    ]);
    args.push(operation.source.frame_rate.to_string().into());
    args.extend([
        "-c:v".into(),
        "png".into(),
        "-i".into(),
        format!("{parent}/frame-%08d.png").into(),
    ]);
    args.extend(strings(&[
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
        "-fps_mode:v",
        "passthrough",
        "-enc_time_base:v",
    ]));
    args.push(period.to_string().into());
    args.extend(strings(&["-vf", "scale=in_range=full:out_range=limited:out_color_matrix=bt709,format=pix_fmts=yuv420p,setsar=1",
        "-c:v", "libx264", "-pix_fmt", "+yuv420p", "-colorspace", "bt709", "-color_range", "tv", "-crf"]));
    args.push(settings.delivery_crf.to_string().into());
    args.extend(["-preset".into(), settings.preset.clone().into()]);
    args.extend(strings(&[
        "-bf",
        "0",
        "-threads:v",
        "1",
        "-color_primaries",
        "bt709",
        "-color_trc",
        "bt709",
        "-avoid_negative_ts",
        "disabled",
        "-use_editlist",
        "1",
        "-video_track_timescale",
    ]));
    args.push(scales.0.to_string().into());
    args.extend(["-movie_timescale".into(), scales.1.to_string().into()]);
    args.extend(strings(&["-movflags", "+faststart", "-f", "mp4"]));
    args.push(output.as_os_str().to_owned());
    // EOF of the exact private sequence determines duration. No fps rounding,
    // truncation, frame padding, resizing or original-audio processing.
    Ok(args)
}

pub(crate) fn encode(
    operation: &MovieOperation,
    settings: &AssemblyConfiguration,
    directory: &Path,
    output: &Path,
    root: &Path,
    token: &CancellationToken,
) -> Result<MovieMedia> {
    verify_versions(settings, root, token, &GroupPolicy::Inherit)?;
    let args = arguments(operation, settings, directory, output)?;
    run(&settings.ffmpeg, "ffmpeg", &args, settings, root, token)?;
    let selection = StreamSelection {
        video_stream: Some(0),
        audio_stream: None,
        no_audio: true,
        discard_streams: Vec::new(),
        audio_extent: Some(crate::temporal::AudioExtentPolicy::PreserveExact),
    };
    let observed = observe(
        output,
        &selection,
        settings,
        root,
        token,
        settings.maximum_output_bytes,
        &GroupPolicy::Inherit,
    )?;
    admitted_source_video(&observed)?;
    let timeline = observed.temporal.video_timeline().map_err(media_error)?;
    let source = &operation.source;
    let period = source.frame_rate.reciprocal().map_err(media_error)?;
    if array(&observed.inventory, "streams")?.len() != 1
        || observed.audio.is_some()
        || timeline.frames.len() as u64 != source.frame_count
        || timeline.frame_period != Some(period)
    {
        return Err(refused(
            "preview MP4 changed exact picture count, period or silent track membership",
        ));
    }
    for (ordinal, frame) in timeline.frames.iter().enumerate() {
        let expected = crate::temporal::TimeRange::new(
            period.checked_mul(ordinal as u64).map_err(media_error)?,
            period
                .checked_mul(ordinal as u64 + 1)
                .map_err(media_error)?,
        )
        .map_err(media_error)?;
        if frame.source_time != expected {
            return Err(refused(
                "preview MP4 changed an exact local-zero picture interval",
            ));
        }
    }
    let video = stream(&observed.inventory, 0)?;
    if positive(video, "width")? != u64::from(source.width)
        || positive(video, "height")? != u64::from(source.height)
        || text(video, "codec_name")? != "h264"
        || text(video, "pix_fmt")? != "yuv420p"
        || text(video, "color_space")? != "bt709"
        || text(video, "color_range")? != "tv"
    {
        return Err(refused(
            "preview MP4 differs from explicit geometry/H.264/BT.709 limited viewing policy",
        ));
    }
    full_decode(output, false, settings, root, token)?;
    let fast = faststart(output, settings.maximum_output_bytes)?;
    if !fast {
        return Err(refused("preview MP4 is not faststart"));
    }
    let interval = timeline
        .source_time
        .ok_or_else(|| refused("preview MP4 has no exact picture interval"))?;
    let expected = crate::temporal::TimeRange::new(
        RationalTime::ZERO,
        period
            .checked_mul(source.frame_count)
            .map_err(media_error)?,
    )
    .map_err(media_error)?;
    if interval != expected
        || crate::frame_io::identity(output, settings.maximum_output_bytes)?
            != (
                observed.temporal.source_sha256.clone(),
                observed.temporal.source_size_bytes,
            )
    {
        return Err(refused(
            "preview MP4 exact interval or bytes changed during final decode",
        ));
    }
    Ok(MovieMedia {
        sha256: observed.temporal.source_sha256.clone(),
        byte_count: observed.temporal.source_size_bytes,
        width: source.width,
        height: source.height,
        frame_count: source.frame_count,
        frame_rate: source.frame_rate,
        local_interval: interval,
        video_codec: text(video, "codec_name")?.into(),
        pixel_format: text(video, "pix_fmt")?.into(),
        color_primaries: text(video, "color_primaries")?.into(),
        color_transfer: text(video, "color_transfer")?.into(),
        color_matrix: text(video, "color_space")?.into(),
        color_range: text(video, "color_range")?.into(),
        sample_aspect_ratio: text(video, "sample_aspect_ratio")?.into(),
        faststart: fast,
        full_decode_observed: true,
        audio_stream_count: 0,
    })
}
