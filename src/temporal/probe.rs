use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    Cadence, FrameTimestamp, MediaStreamKind, PacketTimestamp, RationalTime, SelectedStreams,
    StreamIdentity, StreamSelection, StreamTimeline, TEMPORAL_INSPECTION_SCHEMA_V1,
    TEMPORAL_POLICY_V1, TemporalCode, TemporalInspection, TemporalSupport, TimeRange,
    assess_processing, diagnostic,
};
use crate::command::{ProcessLimits, run_bounded};

const MAXIMUM_PROBE_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_OBSERVATIONS: usize = 1_000_000;

pub(super) fn inspect(input: &Path, selection: &StreamSelection) -> Result<TemporalInspection> {
    inspect_with_program(input, selection, "ffprobe")
}

pub(crate) fn inspect_with_program(input: &Path, selection: &StreamSelection, program: &str) -> Result<TemporalInspection> {
    let source = input
        .canonicalize()
        .context("failed to resolve temporal source")?;
    let metadata = fs::metadata(&source)?;
    if !metadata.is_file() {
        return Err(diagnostic(
            TemporalCode::MalformedProbe,
            None,
            "temporal source must be a regular file",
        )
        .into());
    }
    let before = sha256_file(&source)?;
    // The subprocess reads a stable private copy; the original is opened read-only.
    let snapshot = tempfile::Builder::new()
        .prefix("aniflow-temporal-")
        .tempfile()?;
    fs::copy(&source, snapshot.path()).context("failed to snapshot temporal source")?;
    if sha256_file(snapshot.path())? != before || sha256_file(&source)? != before {
        return Err(diagnostic(
            TemporalCode::SourceChanged,
            None,
            "source changed while creating the inspection snapshot",
        )
        .into());
    }
    let inventory = probe(program, snapshot.path(), None, &["-show_streams", "-show_format"])?;
    let streams = parse_inventory(&inventory)?;
    let selected = resolve_selection(&streams, selection);
    let mut observations = Vec::new();
    if let Ok(selected) = selected {
        for index in selected.video.into_iter().chain(selected.audio) {
            let frames = probe(
                program, snapshot.path(),
                Some(index),
                &[
                    "-show_frames",
                    "-show_entries",
                    "frame=stream_index,pts,pkt_dts,duration,pkt_duration,key_frame,nb_samples",
                ],
            )?;
            let packets = probe(
                program, snapshot.path(),
                Some(index),
                &[
                    "-show_packets",
                    "-show_entries",
                    "packet=stream_index,pts,dts,duration",
                ],
            )?;
            observations.push((index, frames, packets));
        }
    }
    if sha256_file(&source)? != before || fs::metadata(&source)?.len() != metadata.len() {
        return Err(diagnostic(
            TemporalCode::SourceChanged,
            None,
            "source changed during temporal inspection",
        )
        .into());
    }
    from_documents(
        &before,
        metadata.len(),
        selection,
        &inventory,
        &observations,
    )
}

fn probe(program: &str, source: &Path, stream: Option<u32>, options: &[&str]) -> Result<Value> {
    let mut arguments: Vec<OsString> = ["-v", "error", "-protocol_whitelist", "file,pipe"]
        .into_iter()
        .map(OsString::from)
        .collect();
    if let Some(index) = stream {
        arguments.extend([
            OsString::from("-select_streams"),
            OsString::from(index.to_string()),
        ]);
    }
    arguments.extend(options.iter().map(OsString::from));
    arguments.extend([
        OsString::from("-of"),
        OsString::from("json"),
        OsString::from("-i"),
        source.as_os_str().to_owned(),
    ]);
    let output = run_bounded(
        program,
        arguments,
        ProcessLimits {
            timeout: Duration::from_secs(120),
            maximum_output_bytes: MAXIMUM_PROBE_BYTES,
        },
        None,
    )
    .context("bounded temporal ffprobe failed")?;
    if output.stdout_truncated || output.stderr_truncated {
        return Err(diagnostic(
            TemporalCode::ProbeLimit,
            stream,
            "FFprobe exceeded the 64 MiB observation bound; no truncated evidence is accepted",
        )
        .into());
    }
    if !output.status.success() {
        return Err(diagnostic(
            TemporalCode::MalformedProbe,
            stream,
            format!(
                "FFprobe failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        )
        .into());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| {
        diagnostic(
            TemporalCode::MalformedProbe,
            stream,
            format!("invalid FFprobe JSON: {e}"),
        )
        .into()
    })
}

pub(super) fn from_documents(
    source_sha256: &str,
    source_size_bytes: u64,
    selection: &StreamSelection,
    inventory: &Value,
    observations: &[(u32, Value, Value)],
) -> Result<TemporalInspection> {
    if source_sha256.len() != 64
        || !source_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || source_size_bytes == 0
    {
        return Err(diagnostic(
            TemporalCode::MalformedProbe,
            None,
            "source identity requires a lowercase SHA-256 and nonzero byte count",
        )
        .into());
    }
    let streams = parse_inventory(inventory)?;
    let mut diagnostics = Vec::new();
    let selected = match resolve_selection(&streams, selection) {
        Ok(value) => Some(value),
        Err(reason) => {
            diagnostics.push(reason);
            None
        }
    };
    let mut timelines = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, frames, packets) in observations {
        if !seen.insert(*index) {
            return Err(diagnostic(
                TemporalCode::MalformedProbe,
                Some(*index),
                "duplicate observation document",
            )
            .into());
        }
        let stream = streams.iter().find(|s| s.index == *index).ok_or_else(|| {
            diagnostic(
                TemporalCode::InvalidSelection,
                Some(*index),
                "observation references an unknown stream",
            )
        })?;
        if !selected
            .as_ref()
            .is_some_and(|s| s.video == Some(*index) || s.audio == Some(*index))
        {
            return Err(diagnostic(
                TemporalCode::InvalidSelection,
                Some(*index),
                "observation must belong to an explicitly resolved stream",
            )
            .into());
        }
        let frames = parse_frames(stream, frames)?;
        let packets = parse_packets(*index, packets)?;
        let period = frames
            .first()
            .map(|f| f.source_time.duration())
            .transpose()?;
        let constant = !frames.is_empty()
            && frames
                .iter()
                .all(|f| f.source_time.duration().ok() == period)
            && frames
                .windows(2)
                .all(|p| p[0].source_time.end == p[1].source_time.start);
        let cadence = if frames.is_empty() {
            Cadence::Unavailable
        } else if constant {
            Cadence::Constant
        } else {
            Cadence::Variable
        };
        let source_time = frames
            .first()
            .zip(frames.last())
            .map(|(a, b)| TimeRange::new(a.source_time.start, b.source_time.end))
            .transpose()?;
        timelines.push(StreamTimeline {
            stream_index: *index,
            cadence,
            frame_period: if constant { period } else { None },
            source_time,
            frames,
            packets,
        });
    }
    timelines.sort_by_key(|t| t.stream_index);
    let mut result = TemporalInspection {
        schema: TEMPORAL_INSPECTION_SCHEMA_V1.to_owned(),
        source_sha256: source_sha256.to_owned(),
        source_size_bytes,
        selection_intent: selection.clone(),
        streams,
        selected,
        timelines,
        diagnostics,
        processing: TemporalSupport {
            policy: TEMPORAL_POLICY_V1.to_owned(),
            supported: false,
            diagnostics: Vec::new(),
            frame_count: None,
            video_frame_tolerance: None,
            audio_boundary_tolerance: None,
        },
    };
    result.processing = assess_processing(&result)?;
    Ok(result)
}

fn integer(value: &Value, field: &str) -> Result<Option<i64>> {
    let Some(raw) = value
        .get(field)
        .filter(|v| !v.is_null() && v.as_str() != Some("N/A"))
    else {
        return Ok(None);
    };
    raw.as_i64()
        .or_else(|| raw.as_str().and_then(|s| s.parse::<i64>().ok()))
        .map(Some)
        .ok_or_else(|| {
            diagnostic(
                TemporalCode::MalformedProbe,
                None,
                format!("{field} must be an exact signed integer"),
            )
            .into()
        })
}

fn unsigned(value: &Value, field: &str) -> Result<Option<u64>> {
    integer(value, field)?
        .map(|v| {
            u64::try_from(v).map_err(|_| {
                diagnostic(
                    TemporalCode::MalformedProbe,
                    None,
                    format!("{field} must be nonnegative"),
                )
                .into()
            })
        })
        .transpose()
}

fn rate(value: &Value, field: &str) -> Result<Option<RationalTime>> {
    match value.get(field).and_then(Value::as_str) {
        None | Some("N/A" | "0/0" | "0/1") => Ok(None),
        Some(raw) => {
            let result = RationalTime::parse(raw)?;
            if result <= RationalTime::ZERO {
                return Err(diagnostic(
                    TemporalCode::InvalidRational,
                    None,
                    format!("{field} must be positive"),
                )
                .into());
            }
            Ok(Some(result))
        }
    }
}

fn parse_inventory(root: &Value) -> Result<Vec<StreamIdentity>> {
    let raw = root
        .get("streams")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            diagnostic(
                TemporalCode::MalformedProbe,
                None,
                "FFprobe inventory requires streams",
            )
        })?;
    if raw.is_empty() || raw.len() > 1024 {
        return Err(diagnostic(
            TemporalCode::ProbeLimit,
            None,
            "inventory requires 1..=1024 streams",
        )
        .into());
    }
    let mut seen = BTreeSet::new();
    let mut streams = Vec::new();
    for value in raw {
        let index = unsigned(value, "index")?
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| {
                diagnostic(
                    TemporalCode::MalformedProbe,
                    None,
                    "stream index requires u32",
                )
            })?;
        if !seen.insert(index) {
            return Err(diagnostic(
                TemporalCode::DuplicateStream,
                Some(index),
                "duplicate input-global stream index",
            )
            .into());
        }
        let kind = match value["codec_type"].as_str() {
            Some("video") if value["disposition"]["attached_pic"].as_u64() == Some(1) => {
                MediaStreamKind::CoverArt
            }
            Some("video") => MediaStreamKind::Video,
            Some("audio") => MediaStreamKind::Audio,
            Some("subtitle") => MediaStreamKind::Subtitle,
            Some("data") => MediaStreamKind::Data,
            Some("attachment") => MediaStreamKind::Attachment,
            _ => MediaStreamKind::Unknown,
        };
        streams.push(StreamIdentity {
            index,
            kind,
            codec: value["codec_name"].as_str().map(str::to_owned),
            time_base: rate(value, "time_base")?,
            start_pts: integer(value, "start_pts")?,
            duration_ticks: integer(value, "duration_ts")?,
            declared_frame_rate: rate(value, "r_frame_rate")?,
            average_frame_rate: rate(value, "avg_frame_rate")?,
            width: unsigned(value, "width")?,
            height: unsigned(value, "height")?,
            pixel_format: value["pix_fmt"].as_str().map(str::to_owned),
            sample_rate_hz: unsigned(value, "sample_rate")?
                .map(|v| {
                    u32::try_from(v).map_err(|_| {
                        diagnostic(
                            TemporalCode::MalformedProbe,
                            Some(index),
                            "sample rate overflow",
                        )
                    })
                })
                .transpose()?,
        });
    }
    streams.sort_by_key(|s| s.index);
    Ok(streams)
}

pub(super) fn resolve_selection(
    streams: &[StreamIdentity],
    intent: &StreamSelection,
) -> Result<SelectedStreams, super::TemporalDiagnostic> {
    if intent.no_audio && intent.audio_stream.is_some() {
        return Err(diagnostic(
            TemporalCode::InvalidSelection,
            intent.audio_stream,
            "--no-audio conflicts with --audio-stream",
        ));
    }
    let choose = |kind: MediaStreamKind, explicit: Option<u32>, ambiguous: TemporalCode| {
        if let Some(index) = explicit {
            if !streams.iter().any(|s| s.index == index && s.kind == kind) {
                return Err(diagnostic(
                    TemporalCode::InvalidSelection,
                    Some(index),
                    "selected index is absent or has the wrong stream kind",
                ));
            }
            return Ok(Some(index));
        }
        let candidates: Vec<_> = streams
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.index)
            .collect();
        match candidates.as_slice() {
            [] => Ok(None),
            [index] => Ok(Some(*index)),
            _ => Err(diagnostic(
                ambiguous,
                None,
                format!(
                    "multiple {kind:?} streams {candidates:?}; choose an input-global index explicitly"
                ),
            )),
        }
    };
    let video = choose(
        MediaStreamKind::Video,
        intent.video_stream,
        TemporalCode::AmbiguousVideo,
    )?;
    let audio = if intent.no_audio {
        None
    } else {
        choose(
            MediaStreamKind::Audio,
            intent.audio_stream,
            TemporalCode::AmbiguousAudio,
        )?
    };
    let mut discarded = intent.discard_streams.clone();
    discarded.sort_unstable();
    discarded.dedup();
    if discarded.len() != intent.discard_streams.len() {
        return Err(diagnostic(
            TemporalCode::InvalidSelection,
            None,
            "discard indices must be unique",
        ));
    }
    for index in &discarded {
        if Some(*index) == video
            || Some(*index) == audio
            || !streams.iter().any(|s| s.index == *index)
        {
            return Err(diagnostic(
                TemporalCode::InvalidSelection,
                Some(*index),
                "cannot discard a selected or unknown stream",
            ));
        }
    }
    Ok(SelectedStreams { video, audio, explicitly_discarded: discarded, automatic_policy: "unique moving-video and unique optional audio; no incidental-order/default-disposition selection".to_owned() })
}

fn parse_frames(stream: &StreamIdentity, root: &Value) -> Result<Vec<FrameTimestamp>> {
    let raw = root
        .get("frames")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            diagnostic(
                TemporalCode::MalformedProbe,
                Some(stream.index),
                "missing decoded frame array",
            )
        })?;
    if raw.len() > MAXIMUM_OBSERVATIONS {
        return Err(diagnostic(
            TemporalCode::ProbeLimit,
            Some(stream.index),
            "too many decoded frames",
        )
        .into());
    }
    let tb = stream.time_base.ok_or_else(|| {
        diagnostic(
            TemporalCode::MissingTimestamp,
            Some(stream.index),
            "selected stream requires an exact time base",
        )
    })?;
    let mut frames = Vec::new();
    for (ordinal, value) in raw.iter().enumerate() {
        if unsigned(value, "stream_index")? != Some(u64::from(stream.index)) {
            return Err(diagnostic(
                TemporalCode::InvalidSelection,
                Some(stream.index),
                "decoded frame belongs to another stream",
            )
            .into());
        }
        let pts = integer(value, "pts")?.ok_or_else(|| {
            diagnostic(
                TemporalCode::MissingTimestamp,
                Some(stream.index),
                "decoded frame requires PTS; best-effort/inferred timestamps are not substituted",
            )
        })?;
        let duration = integer(value, "duration")?
            .or(integer(value, "pkt_duration")?)
            .or_else(|| {
                if stream.kind == MediaStreamKind::Audio
                    && stream.sample_rate_hz.is_some_and(|r| {
                        tb == RationalTime::new(1, u64::from(r)).unwrap_or(RationalTime::ZERO)
                    })
                {
                    value["nb_samples"].as_i64()
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                diagnostic(
                    TemporalCode::MissingTimestamp,
                    Some(stream.index),
                    "decoded frame requires an observed duration",
                )
            })?;
        if duration <= 0 {
            return Err(diagnostic(
                TemporalCode::InvalidDuration,
                Some(stream.index),
                "decoded frame duration must be positive",
            )
            .into());
        }
        let start = RationalTime::ticks(pts, tb)?;
        let end = start.checked_add(RationalTime::ticks(duration, tb)?)?;
        frames.push(FrameTimestamp {
            ordinal: ordinal as u64,
            stream_index: stream.index,
            presentation_ticks: pts,
            decode_ticks: integer(value, "pkt_dts")?,
            duration_ticks: duration,
            keyframe: unsigned(value, "key_frame")? == Some(1),
            source_time: TimeRange::new(start, end)?,
        });
    }
    Ok(frames)
}

fn parse_packets(index: u32, root: &Value) -> Result<Vec<PacketTimestamp>> {
    let raw = root
        .get("packets")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            diagnostic(
                TemporalCode::MalformedProbe,
                Some(index),
                "missing packet array",
            )
        })?;
    if raw.len() > MAXIMUM_OBSERVATIONS {
        return Err(diagnostic(TemporalCode::ProbeLimit, Some(index), "too many packets").into());
    }
    raw.iter()
        .enumerate()
        .map(|(ordinal, value)| {
            if unsigned(value, "stream_index")? != Some(u64::from(index)) {
                return Err(diagnostic(
                    TemporalCode::InvalidSelection,
                    Some(index),
                    "packet belongs to another stream",
                )
                .into());
            }
            Ok(PacketTimestamp {
                ordinal: ordinal as u64,
                stream_index: index,
                presentation_ticks: integer(value, "pts")?,
                decode_ticks: integer(value, "dts")?,
                duration_ticks: integer(value, "duration")?,
            })
        })
        .collect()
}

pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
