use anyhow::Result;

use super::{
    Cadence, MediaStreamKind, RationalTime, TEMPORAL_INSPECTION_SCHEMA_V1, TEMPORAL_POLICY_V1,
    TemporalCode, TemporalInspection, TemporalSupport, diagnostic,
};

/// Bound processing to contiguous CFR presentation, zero origin and acknowledged discards.
/// Packet PTS may be reordered; DTS must remain monotonic in demux order.
pub fn assess_processing(report: &TemporalInspection) -> Result<TemporalSupport> {
    let mut reasons = report.diagnostics.clone();
    let mut frame_count = None;
    let mut video_tolerance = None;
    let mut audio_tolerance = None;
    if report.schema != TEMPORAL_INSPECTION_SCHEMA_V1 {
        reasons.push(diagnostic(
            TemporalCode::MissingTemporalEvidence,
            None,
            "unsupported temporal inspection schema",
        ));
    }
    let Some(selected) = report.selected.as_ref() else {
        if reasons.is_empty() {
            reasons.push(diagnostic(
                TemporalCode::InvalidSelection,
                None,
                "no resolved stream selection",
            ));
        }
        return Ok(TemporalSupport {
            policy: TEMPORAL_POLICY_V1.to_owned(),
            supported: false,
            diagnostics: reasons,
            frame_count,
            video_frame_tolerance: video_tolerance,
            audio_boundary_tolerance: audio_tolerance,
        });
    };
    match super::probe::resolve_selection(&report.streams, &report.selection_intent) {
        Ok(resolved) if resolved == *selected => {}
        _ => reasons.push(diagnostic(
            TemporalCode::InvalidSelection,
            None,
            "resolved streams disagree with the frozen selection intent",
        )),
    }
    let mut stream_indices = std::collections::BTreeSet::new();
    let mut timeline_indices = std::collections::BTreeSet::new();
    if report
        .streams
        .iter()
        .any(|s| !stream_indices.insert(s.index))
        || report
            .timelines
            .iter()
            .any(|t| !timeline_indices.insert(t.stream_index))
    {
        reasons.push(diagnostic(
            TemporalCode::DuplicateStream,
            None,
            "inspection contains duplicate stream or timeline identities",
        ));
    }
    for stream in &report.streams {
        if Some(stream.index) != selected.video
            && Some(stream.index) != selected.audio
            && !selected.explicitly_discarded.contains(&stream.index)
        {
            reasons.push(diagnostic(
                TemporalCode::UnacknowledgedStream,
                Some(stream.index),
                "unselected stream would be lost; explicitly acknowledge it with --discard-stream",
            ));
        }
    }
    if selected.video.is_none() {
        reasons.push(diagnostic(
            TemporalCode::MissingVideo,
            None,
            "CFR processing requires one moving-video stream",
        ));
    }
    for (index, kind) in selected
        .video
        .map(|i| (i, MediaStreamKind::Video))
        .into_iter()
        .chain(selected.audio.map(|i| (i, MediaStreamKind::Audio)))
    {
        let Some(stream) = report.stream(index) else {
            reasons.push(diagnostic(
                TemporalCode::InvalidSelection,
                Some(index),
                "resolved stream does not exist",
            ));
            continue;
        };
        if stream.kind != kind {
            reasons.push(diagnostic(
                TemporalCode::InvalidSelection,
                Some(index),
                "resolved stream has incompatible kind",
            ));
            continue;
        }
        let Some(timeline) = report.timeline(index) else {
            reasons.push(diagnostic(
                TemporalCode::MissingTemporalEvidence,
                Some(index),
                "decoded timeline was not observed",
            ));
            continue;
        };
        let Some(range) = timeline.source_time else {
            reasons.push(diagnostic(
                TemporalCode::MissingTemporalEvidence,
                Some(index),
                "empty decoded timeline",
            ));
            continue;
        };
        if timeline.frames.is_empty() || timeline.packets.is_empty() {
            reasons.push(diagnostic(
                TemporalCode::MissingTemporalEvidence,
                Some(index),
                "decoded frames and demuxed packets are both required",
            ));
        }
        for (position, frame) in timeline.frames.iter().enumerate() {
            if frame.ordinal != position as u64
                || frame.stream_index != index
                || frame.duration_ticks <= 0
            {
                reasons.push(diagnostic(
                    TemporalCode::TimelineMismatch,
                    Some(index),
                    "invalid frame ordinal, stream identity or duration",
                ));
                break;
            }
            let tb = stream.time_base.ok_or_else(|| {
                diagnostic(
                    TemporalCode::MissingTimestamp,
                    Some(index),
                    "missing stream time base",
                )
            })?;
            let start = RationalTime::ticks(frame.presentation_ticks, tb)?;
            let end = start.checked_add(RationalTime::ticks(frame.duration_ticks, tb)?)?;
            if frame.source_time.start != start || frame.source_time.end != end {
                reasons.push(diagnostic(
                    TemporalCode::TimelineMismatch,
                    Some(index),
                    "frame range disagrees with PTS/duration/time base",
                ));
                break;
            }
        }
        if timeline.frames.first().map(|f| f.source_time.start) != Some(range.start)
            || timeline.frames.last().map(|f| f.source_time.end) != Some(range.end)
        {
            reasons.push(diagnostic(
                TemporalCode::TimelineMismatch,
                Some(index),
                "timeline range disagrees with decoded frames",
            ));
        }
        if timeline
            .frames
            .windows(2)
            .any(|p| p[1].presentation_ticks <= p[0].presentation_ticks)
        {
            reasons.push(diagnostic(
                TemporalCode::PresentationOrder,
                Some(index),
                "decoded presentation timestamps must strictly increase",
            ));
        }
        if timeline
            .frames
            .windows(2)
            .any(|p| p[0].source_time.end != p[1].source_time.start)
        {
            reasons.push(diagnostic(
                TemporalCode::TimelineMismatch,
                Some(index),
                "decoded frames contain a gap or overlap",
            ));
        }
        if range.start != RationalTime::ZERO {
            reasons.push(diagnostic(
                if range.start < RationalTime::ZERO {
                    TemporalCode::NegativePresentationOffset
                } else {
                    TemporalCode::NonzeroPresentationOffset
                },
                Some(index),
                "offset is preserved in inspection; this processing profile cannot preserve it",
            ));
        }
        if let Some(declared_start) = stream.start_pts {
            if RationalTime::ticks(
                declared_start,
                stream.time_base.ok_or_else(|| {
                    diagnostic(
                        TemporalCode::MissingTimestamp,
                        Some(index),
                        "missing time base",
                    )
                })?,
            )? != range.start
            {
                reasons.push(diagnostic(
                    TemporalCode::TimelineMismatch,
                    Some(index),
                    "declared start differs from decoded presentation origin",
                ));
            }
        }
        if timeline.packets.iter().enumerate().any(|(position, p)| {
            p.ordinal != position as u64
                || p.stream_index != index
                || p.presentation_ticks.is_none()
                || p.decode_ticks.is_none()
        }) {
            reasons.push(diagnostic(
                TemporalCode::MissingTimestamp,
                Some(index),
                "packet PTS/DTS and contiguous ordinals are required",
            ));
        }
        if timeline.packets.windows(2).any(|p| {
            p[0].decode_ticks
                .zip(p[1].decode_ticks)
                .is_some_and(|(a, b)| b < a)
        }) {
            reasons.push(diagnostic(
                TemporalCode::DecodeOrder,
                Some(index),
                "demuxed DTS must be monotonic; reordered packet PTS is retained separately",
            ));
        }
        if let Some(duration) = stream.duration_ticks {
            if duration <= 0 {
                reasons.push(diagnostic(
                    TemporalCode::InvalidDuration,
                    Some(index),
                    "declared duration must be positive",
                ));
            }
            let tb = stream.time_base.ok_or_else(|| {
                diagnostic(
                    TemporalCode::MissingTimestamp,
                    Some(index),
                    "missing time base",
                )
            })?;
            let duration_tolerance = if kind == MediaStreamKind::Audio {
                timeline
                    .frames
                    .iter()
                    .map(|f| f.source_time.duration())
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .max()
                    .unwrap_or(tb)
            } else {
                tb
            };
            if range
                .duration()?
                .checked_sub(RationalTime::ticks(duration, tb)?)?
                .abs()?
                > duration_tolerance
            {
                reasons.push(diagnostic(TemporalCode::StreamDurationMismatch, Some(index), "decoded duration exceeds the declared stream duration tolerance (one video tick or one audio decode block)"));
            }
        }
        if kind == MediaStreamKind::Video {
            frame_count = Some(timeline.frames.len() as u64);
            let actual_period = timeline
                .frames
                .first()
                .map(|f| f.source_time.duration())
                .transpose()?;
            if timeline.cadence != Cadence::Constant
                || actual_period.is_none()
                || timeline.frame_period != actual_period
                || timeline
                    .frames
                    .iter()
                    .any(|f| f.source_time.duration().ok() != actual_period)
            {
                reasons.push(diagnostic(TemporalCode::VariableFrameRate, Some(index), "VFR is inspectable; fixed-rate frame reconstruction and segmentation refuse it"));
            } else {
                video_tolerance = actual_period;
            }
        } else {
            if stream.sample_rate_hz.is_none_or(|rate| rate == 0) {
                reasons.push(diagnostic(
                    TemporalCode::MissingTemporalEvidence,
                    Some(index),
                    "selected audio requires a positive sample clock",
                ));
            }
            audio_tolerance = timeline
                .frames
                .iter()
                .map(|f| f.source_time.duration())
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .max();
        }
    }
    if let (Some(video), Some(audio)) = (
        selected
            .video
            .and_then(|i| report.timeline(i))
            .and_then(|t| t.source_time),
        selected
            .audio
            .and_then(|i| report.timeline(i))
            .and_then(|t| t.source_time),
    ) {
        // Origin must be exact. End padding is bounded by one observed video frame
        // plus one observed audio decode block, never an implicit arbitrary epsilon.
        let tolerance = video_tolerance
            .unwrap_or(RationalTime::ZERO)
            .checked_add(audio_tolerance.unwrap_or(RationalTime::ZERO))?;
        if video.start != audio.start || video.end.checked_sub(audio.end)?.abs()? > tolerance {
            reasons.push(diagnostic(
                TemporalCode::SynchronizationMismatch,
                selected.audio,
                "selected A/V origins or ends exceed the declared exact boundary tolerance",
            ));
        }
    }
    Ok(TemporalSupport {
        policy: TEMPORAL_POLICY_V1.to_owned(),
        supported: reasons.is_empty(),
        diagnostics: reasons,
        frame_count,
        video_frame_tolerance: video_tolerance,
        audio_boundary_tolerance: audio_tolerance,
    })
}

/// Compare decoded presentation grids, frame cardinality and selected A/V ranges.
/// Re-encoding may change packet DTS; it must not change presentation time.
pub fn validate_reconstruction(
    source: &TemporalInspection,
    derived: &TemporalInspection,
    allow_authored_subtitles: bool,
) -> Result<()> {
    source.require_processing()?;
    let selection = source.selected.as_ref().ok_or_else(|| {
        diagnostic(
            TemporalCode::InvalidSelection,
            None,
            "source has no selection",
        )
    })?;
    for stream in &derived.streams {
        let accepted = matches!(stream.kind, MediaStreamKind::Video | MediaStreamKind::Audio)
            || (allow_authored_subtitles && stream.kind == MediaStreamKind::Subtitle);
        if !accepted {
            return Err(diagnostic(
                TemporalCode::UnsupportedStream,
                Some(stream.index),
                "derived artifact has an unexpected stream kind",
            )
            .into());
        }
    }
    let mut assessed = derived.clone();
    if allow_authored_subtitles {
        assessed.selection_intent.discard_streams.extend(
            derived
                .streams
                .iter()
                .filter(|s| s.kind == MediaStreamKind::Subtitle)
                .map(|s| s.index),
        );
        if let Some(selected) = assessed.selected.as_mut() {
            selected.explicitly_discarded.extend(
                derived
                    .streams
                    .iter()
                    .filter(|s| s.kind == MediaStreamKind::Subtitle)
                    .map(|s| s.index),
            );
        }
    }
    assessed.require_processing()?;
    let videos = derived
        .streams
        .iter()
        .filter(|s| s.kind == MediaStreamKind::Video)
        .count();
    let audios = derived
        .streams
        .iter()
        .filter(|s| s.kind == MediaStreamKind::Audio)
        .count();
    if videos != 1 || audios != usize::from(selection.audio.is_some()) {
        return Err(diagnostic(
            TemporalCode::InvalidSelection,
            None,
            "derived artifact changed the selected video/audio cardinality",
        )
        .into());
    }
    let original = source.video_timeline()?;
    let output = derived.video_timeline()?;
    if original.frames.len() != output.frames.len() {
        return Err(diagnostic(
            TemporalCode::FrameCountMismatch,
            None,
            "derived video frame count differs from source",
        )
        .into());
    }
    for (a, b) in original.frames.iter().zip(&output.frames) {
        if a.source_time != b.source_time {
            return Err(diagnostic(
                TemporalCode::TimelineMismatch,
                Some(b.stream_index),
                "derived video changed an exact source presentation interval",
            )
            .into());
        }
    }
    if let Some(source_audio) = selection
        .audio
        .and_then(|i| source.timeline(i))
        .and_then(|t| t.source_time)
    {
        let output_audio = derived
            .selected
            .as_ref()
            .and_then(|s| s.audio)
            .and_then(|i| derived.timeline(i))
            .and_then(|t| t.source_time)
            .ok_or_else(|| {
                diagnostic(
                    TemporalCode::MissingTemporalEvidence,
                    None,
                    "derived audio timeline is missing",
                )
            })?;
        let bounds = assess_processing(source)?;
        let tolerance = bounds
            .video_frame_tolerance
            .unwrap_or(RationalTime::ZERO)
            .checked_add(
                bounds
                    .audio_boundary_tolerance
                    .unwrap_or(RationalTime::ZERO),
            )?;
        if output_audio.start != source_audio.start
            || output_audio.end.checked_sub(source_audio.end)?.abs()? > tolerance
        {
            return Err(diagnostic(
                TemporalCode::SynchronizationMismatch,
                None,
                "derived audio changed the source origin or exceeded exact end tolerance",
            )
            .into());
        }
    }
    Ok(())
}
