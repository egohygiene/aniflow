//! Selected export reuses this owner's exact probes, packet facts and decoders.
use super::*;
use crate::destination::video::{VideoExportMedia, VideoExportOperation};

pub(super) fn source_matches(
    before: &Observation,
    expected: &crate::final_video::FinalVideoArtifact,
) -> Result<()> {
    let timeline = before.temporal.video_timeline().map_err(media_error)?;
    let v = stream(&before.inventory, 0)?;
    if before.temporal.source_sha256 != expected.sha256
        || before.temporal.source_size_bytes != expected.byte_count
        || timeline.frames.len() as u64 != expected.frame_count
        || timeline.source_time != Some(expected.video_interval)
        || timeline.frame_period != Some(expected.frame_rate.reciprocal().map_err(media_error)?)
        || positive(v, "width")? != u64::from(expected.width)
        || positive(v, "height")? != u64::from(expected.height)
        || text(v, "codec_name")? != expected.video_codec
        || text(v, "pix_fmt")? != expected.pixel_format
        || text(v, "color_primaries")? != expected.color_primaries
        || text(v, "color_transfer")? != expected.color_transfer
        || text(v, "color_space")? != expected.color_matrix
        || text(v, "color_range")? != expected.color_range
    {
        return Err(refused(
            "native export source differs from its exact accepted bytes/clock/playback facts",
        ));
    }
    if before.audio.is_some() != expected.audio.is_some() {
        return Err(refused("accepted source audio presence changed"));
    }
    if let (Some(actual), Some(expected)) = (&before.audio, &expected.audio) {
        let interval = before
            .temporal
            .timeline(1)
            .and_then(|t| t.source_time)
            .ok_or_else(|| refused("source audio interval missing"))?;
        if digest(actual)? != expected.packet_configuration_timing_sha256
            || actual.codec != expected.codec
            || actual.profile != expected.profile
            || actual.sample_rate != expected.sample_rate_hz
            || actual.channels != expected.channels
            || actual.channel_layout != expected.channel_layout
            || interval != expected.interval
        {
            return Err(refused(
                "native export source AAC bytes/configuration/clock differ from acceptance",
            ));
        }
    }
    Ok(())
}
fn arguments(
    operation: &VideoExportOperation,
    settings: &AssemblyConfiguration,
    source: &Path,
    output: &Path,
    before: &Observation,
) -> Result<Vec<OsString>> {
    let p = &operation.intent.payload;
    let period = before
        .temporal
        .video_timeline()
        .map_err(media_error)?
        .frame_period
        .ok_or_else(|| refused("destination source period unavailable"))?;
    let frames = FrameSet {
        directory: None,
        width: p.geometry.width,
        height: p.geometry.height,
        period,
        frame_count: p.artifact.frame_count as usize,
        source_inspection: before.temporal.clone(),
        derived_video_clock: None,
    };
    let audio = before.audio.as_ref().map(|_| 1);
    let scales = timescales(period, before.audio.as_ref())?;
    let mut args = encode_arguments(
        AssemblyStageKind::Delivery,
        source,
        &frames,
        Some(source),
        output,
        audio,
        scales,
        settings,
    )?;
    let filters = args
        .iter()
        .enumerate()
        .filter(|(_, v)| v.as_os_str() == std::ffi::OsStr::new("-vf"))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if filters.len() != 1 || filters[0] + 1 >= args.len() {
        return Err(refused(
            "native export requires one fixed video-filter slot",
        ));
    }
    args[filters[0] + 1] = p.geometry.filter(&p.artifact.color_range)?.into();
    args.pop()
        .ok_or_else(|| refused("destination output argument missing"))?;
    crate::delivery_metadata::append_arguments(
        &mut args,
        settings
            .delivery_metadata
            .as_ref()
            .ok_or_else(|| refused("explicit export metadata policy missing"))?,
        &crate::delivery_metadata::inventory(&before.inventory)?,
        1,
        0,
        audio,
    )?;
    args.extend(["-fs".into(), operation.output_limit().to_string().into()]);
    args.push(output.as_os_str().into());
    Ok(args)
}
pub(crate) fn execute(
    operation: &VideoExportOperation,
    settings: &AssemblyConfiguration,
    source: &Path,
    output: &Path,
    scratch: &Path,
    token: &CancellationToken,
) -> Result<VideoExportMedia> {
    operation.validate()?;
    verify_versions(settings, scratch, token, &GroupPolicy::Inherit)?;
    let selection = operation.selection();
    let before = observe(
        source,
        &selection,
        settings,
        scratch,
        token,
        settings.maximum_source_bytes,
        &GroupPolicy::Inherit,
    )?;
    admitted_source_video(&before)?;
    source_matches(&before, &operation.intent.payload.artifact)?;
    let before_tags = crate::delivery_metadata::inventory(&before.inventory)?;
    if array(&before.inventory, "streams")?.len() != 1 + usize::from(before.audio.is_some())
        || !before_tags.chapters.is_empty()
        || !before_tags.programs.is_empty()
    {
        return Err(refused(
            "initial destination export refuses unselected tracks/chapters/programs",
        ));
    }
    let args = arguments(operation, settings, source, output, &before)?;
    run(&settings.ffmpeg, "ffmpeg", &args, settings, scratch, token)?;
    let after = observe(
        output,
        &selection,
        settings,
        scratch,
        token,
        operation.output_limit(),
        &GroupPolicy::Inherit,
    )?;
    admitted_source_video(&after)?;
    crate::temporal::validate_reconstruction(&before.temporal, &after.temporal, false)
        .map_err(media_error)?;
    let g = &operation.intent.payload.geometry;
    let timeline = before.temporal.video_timeline().map_err(media_error)?;
    let frames = FrameSet {
        directory: None,
        width: g.width,
        height: g.height,
        period: timeline
            .frame_period
            .ok_or_else(|| refused("export source period missing"))?,
        frame_count: timeline.frames.len(),
        source_inspection: before.temporal.clone(),
        derived_video_clock: None,
    };
    validate_output(&after, &before, &frames, AssemblyStageKind::Delivery)?;
    if before.audio != after.audio {
        return Err(refused(
            "destination export changed AAC packet payload/configuration/timing",
        ));
    }
    let after_tags = crate::delivery_metadata::inventory(&after.inventory)?;
    let mut metadata = crate::delivery_metadata::verify(
        settings
            .delivery_metadata
            .as_ref()
            .ok_or_else(|| refused("export metadata policy missing"))?,
        before_tags,
        after_tags,
        &before.temporal.source_sha256,
        &after.temporal.source_sha256,
        0,
        before.audio.as_ref().map(|_| 1),
    )?;
    let container =
        crate::delivery_metadata::mp4::inspect(output, operation.output_limit(), token)?;
    crate::delivery_metadata::mp4::agrees(&container, &metadata.after)?;
    if metadata.public_summary.status
        != crate::delivery_metadata::MetadataInspectionStatus::Unsupported
    {
        return Err(refused(
            "destination export metadata dictionary policy has residual or failed fields",
        ));
    }
    metadata.public_summary.status =
        crate::delivery_metadata::MetadataInspectionStatus::CleanWithinScope;
    metadata.public_summary.inspected_scope="ffprobe dictionaries plus bounded MP4 global mdta; structural properties independently checked".into();
    metadata.public_summary.unsupported_scope =
        "signal marks, credential authenticity, native/platform/quality qualification".into();
    full_decode(output, after.audio.is_some(), settings, scratch, token)?;
    if !faststart(output, operation.output_limit())? {
        return Err(refused("destination MP4 lacks faststart layout"));
    }
    verify_versions(settings, scratch, token, &GroupPolicy::Inherit)?;
    if crate::frame_io::identity(source, settings.maximum_source_bytes)?
        != (
            before.temporal.source_sha256.clone(),
            before.temporal.source_size_bytes,
        )
        || crate::frame_io::identity(output, operation.output_limit())?
            != (
                after.temporal.source_sha256.clone(),
                after.temporal.source_size_bytes,
            )
    {
        return Err(refused(
            "destination source/output changed after validation",
        ));
    }
    let audio = if let Some(a) = &after.audio {
        Some(crate::final_video::FinalAudioProperties {
            codec: a.codec.clone(),
            profile: a.profile.clone(),
            sample_rate_hz: a.sample_rate,
            channels: a.channels,
            channel_layout: a.channel_layout.clone(),
            interval: after
                .temporal
                .timeline(1)
                .and_then(|t| t.source_time)
                .ok_or_else(|| refused("destination audio interval missing"))?,
            packet_configuration_timing_sha256: digest(a)?,
        })
    } else {
        None
    };
    let playback = MetadataRemuxPlayback {
        video_codec: "h264".into(),
        pixel_format: "yuv420p".into(),
        color_primaries: "bt709".into(),
        color_transfer: "bt709".into(),
        color_matrix: "bt709".into(),
        color_range: "tv".into(),
        sample_aspect_ratio: "1/1".into(),
        orientation_degrees: 0,
        faststart: true,
        audio,
    };
    Ok(VideoExportMedia {
        source: before.temporal,
        candidate: after.temporal,
        geometry: g.clone(),
        playback,
        metadata,
        container,
        audio_packets_preserved: true,
        complete_clock_preserved: true,
        full_decode_observed: true,
        native_qualified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn before() -> Observation {
        let selection = StreamSelection {
            video_stream: Some(0),
            no_audio: true,
            ..StreamSelection::default()
        };
        let inventory = json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264","time_base":"1/30","start_pts":0,"duration_ts":2,"r_frame_rate":"30/1","avg_frame_rate":"30/1","width":2,"height":2,"pix_fmt":"yuv420p"}],"format":{"tags":{}},"chapters":[],"programs":[]});
        let decoded = json!({"frames":[{"stream_index":0,"pts":0,"pkt_dts":0,"duration":1,"key_frame":1},{"stream_index":0,"pts":1,"pkt_dts":1,"duration":1,"key_frame":0}]});
        let packets = json!({"packets":[{"stream_index":0,"pts":0,"dts":0,"duration":1},{"stream_index":0,"pts":1,"dts":1,"duration":1}]});
        let temporal = crate::temporal::from_probe_documents(
            &"c".repeat(64),
            1000,
            &selection,
            &inventory,
            &[(0, decoded, packets)],
        )
        .unwrap();
        Observation {
            temporal,
            inventory,
            audio: None,
        }
    }
    #[test]
    fn generated_export_argv_keeps_exact_clock_and_literal_paths_without_trimming() {
        let op = crate::destination::video::tests::operation();
        let source = Path::new("/synthetic/source $ literal/video.mp4");
        let output = Path::new("/synthetic/candidate 音.mp4");
        let args = arguments(
            &op,
            &op.intent.payload.request.configuration.tools,
            source,
            output,
            &before(),
        )
        .unwrap();
        let text = args.iter().map(|a| a.to_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(args.last().unwrap().as_os_str(), output.as_os_str());
        assert_eq!(
            text.iter()
                .filter(|v| **v == source.to_str().unwrap())
                .count(),
            2
        );
        for denied in [
            "-shortest",
            "-t",
            "-ss",
            "-r",
            "-start_at_zero",
            "-af",
            "-ar",
            "-ac",
        ] {
            assert!(!text.contains(&denied));
        }
        assert!(text.windows(2).any(|w| w == ["-fps_mode:v", "passthrough"]));
        assert!(text.windows(2).any(|w| w == ["-c:v", "libx264"]));
        assert!(
            text.windows(2)
                .any(|w| w == ["-video_track_timescale", "30"])
        );
    }
    #[test]
    fn native_source_byte_or_playback_mismatch_refuses_before_encoding() {
        let observation = before();
        let mut a = crate::destination::video::tests::operation()
            .intent
            .payload
            .artifact;
        assert!(source_matches(&observation, &a).is_err());
        a.sha256 = "0".repeat(64);
        assert!(source_matches(&observation, &a).is_err());
    }
}
