//! Explicit selected-video decode -> accepted #13 PCM analysis -> meter view.
//! The private receipt composes evidence; v3 remains the child acceptance owner.
use crate::audio_inspection::process::{GroupPolicy, run_tool_isolated_with_limits};
use crate::audio_inspection::{AUDIO_INSPECTION_MAXIMUM_BYTES, AudioInspectionRequest};
use crate::audio_signal::{
    NativeLoudnessReceipt, NativeSignalAnalysisConfiguration, SignalAnalysisRequest,
};
use crate::frame_io::{copy_pinned, ensure_canonical_directory, identity, require_absent};
use crate::temporal::{StreamSelection, TemporalInspection, TimeRange};
use crate::video_assembly::{AssemblyConfiguration, require_storage};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoAudioSignalRequest {
    pub schema: String,
    pub source_video: PathBuf,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub selection: StreamSelection,
    pub configuration: AssemblyConfiguration,
    pub provider_executable: PathBuf,
    pub maximum_pcm_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoAudioSignalReceipt {
    pub schema: String,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub audio_stream: u32,
    pub temporal_sha256: String,
    pub source_audio_extent: TimeRange,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frames: u64,
    pub decoded_wav_sha256: String,
    pub decoded_wav_bytes: u64,
    pub decoded_pcm_sha256: String,
    pub ffmpeg_sha256: String,
    pub ffprobe_sha256: String,
    pub provider_sha256: String,
    pub decode_arguments: Vec<String>,
    pub signal_run: String,
    pub signal_plan_sha256: String,
    pub signal_report_sha256: String,
    pub loudness: NativeLoudnessReceipt,
    pub normalization: String,
    pub runtime_qualified: bool,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}
fn mapped(e: anyhow::Error) -> Error {
    Error::from_anyhow(ErrorCategory::Media, e)
}

pub(super) fn extent(inspection: &TemporalInspection) -> Result<(u32, TimeRange, u64)> {
    let index = inspection
        .selected
        .as_ref()
        .and_then(|s| s.audio)
        .ok_or_else(|| invalid("select one original audio stream"))?;
    let rate = inspection
        .stream(index)
        .and_then(|s| s.sample_rate_hz)
        .ok_or_else(|| invalid("selected audio rate unavailable"))?;
    if rate != 48_000 {
        return Err(invalid(
            "source-bound meter initially supports 48 kHz audio",
        ));
    }
    let timeline = inspection
        .timeline(index)
        .ok_or_else(|| invalid("audio decoded timeline unavailable"))?;
    let range = timeline
        .source_time
        .ok_or_else(|| invalid("audio extent unavailable"))?;
    let mut end = range.start;
    for frame in &timeline.frames {
        if frame.source_time.start != end {
            return Err(invalid(
                "audio meter refuses gaps/overlaps instead of inserting or dropping samples",
            ));
        }
        end = frame.source_time.end;
    }
    if timeline.frames.is_empty() || end != range.end {
        return Err(invalid("audio meter requires the complete decoded extent"));
    }
    let frames = range
        .duration()
        .map_err(mapped)?
        .checked_mul(u64::from(rate))
        .map_err(mapped)?;
    if frames.denominator() != 1 || frames.numerator() <= 0 {
        return Err(invalid(
            "audio extent is not an exact positive sample count",
        ));
    }
    Ok((index, range, frames.numerator() as u64))
}

pub(super) fn decode_arguments(
    source: &Path,
    output: &Path,
    index: u32,
    limit: u64,
) -> Vec<OsString> {
    [
        "-hide_banner",
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-err_detect",
        "explode",
        "-threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
        "-i",
        "{source}",
        "-map",
        "{stream}",
        "-vn",
        "-sn",
        "-dn",
        "-map_metadata",
        "-1",
        "-map_chapters",
        "-1",
        "-c:a",
        "pcm_f32le",
        "-flags:a",
        "+bitexact",
        "-fflags",
        "+bitexact",
        "-threads:a",
        "1",
        "-rf64",
        "never",
        "-fs",
        "{limit}",
        "-f",
        "wav",
        "-n",
        "{output}",
    ]
    .into_iter()
    .map(|v| match v {
        "{source}" => source.as_os_str().to_owned(),
        "{output}" => output.as_os_str().to_owned(),
        "{stream}" => format!("0:{index}").into(),
        "{limit}" => limit.to_string().into(),
        _ => v.into(),
    })
    .collect()
}

/// Create a fresh private observation directory. Failures preserve partial
/// evidence; reruns require a different directory and never overwrite sources.
pub fn measure(
    request: &VideoAudioSignalRequest,
    output: &Path,
    token: &CancellationToken,
) -> Result<VideoAudioSignalReceipt> {
    request.configuration.validate()?;
    crate::provider::require_sha256(&request.source_sha256, "video signal source")?;
    if request.schema != "aniflow.video-audio-signal-request/v1"
        || !cfg!(unix)
        || request.source_bytes == 0
        || request.source_bytes > request.configuration.maximum_source_bytes
        || !(1024..=AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&request.maximum_pcm_bytes)
        || request.selection.no_audio
        || request.selection.audio_stream.is_none()
        || request.selection.video_stream.is_none()
        || request.configuration.delivery_metadata.is_some()
    {
        return Err(invalid(
            "video signal request requires a selected original audio/video pair and finite source/PCM bounds without unrelated metadata policy",
        ));
    }
    if token.is_cancelled() {
        return Err(invalid("video signal cancelled before source capture"));
    }
    let parent = output
        .parent()
        .ok_or_else(|| invalid("video signal output needs an existing canonical parent"))?;
    ensure_canonical_directory(parent)?;
    require_absent(output)?;
    let inputs = [
        &request.source_video,
        &request.configuration.ffmpeg.executable,
        &request.configuration.ffprobe.executable,
        &request.provider_executable,
    ];
    for input in inputs {
        if input.to_str().is_none()
            || output.to_str().is_none()
            || input.starts_with(output)
            || output.starts_with(input)
        {
            return Err(invalid(
                "video signal paths require UTF-8 without output/input overlap",
            ));
        }
    }
    let source = (request.source_sha256.clone(), request.source_bytes);
    if identity(
        &request.source_video,
        request.configuration.maximum_source_bytes,
    )? != source
    {
        return Err(invalid(
            "video audio source differs from the requested exact identity",
        ));
    }
    let mut components = Vec::new();
    for input in [
        &request.configuration.ffmpeg.executable,
        &request.configuration.ffprobe.executable,
        &request.provider_executable,
    ] {
        components.push(identity(input, 512 * 1024 * 1024)?);
    }
    if components[0].0 != request.configuration.ffmpeg.sha256
        || components[1].0 != request.configuration.ffprobe.sha256
    {
        return Err(invalid("video signal tool pin changed"));
    }
    let initial = request
        .source_bytes
        .checked_add(components.iter().map(|v| v.1).sum::<u64>())
        .and_then(|v| v.checked_add(1_048_576))
        .ok_or_else(|| invalid("video signal storage overflow"))?;
    if initial > request.configuration.maximum_working_bytes {
        return Err(invalid(
            "video signal capture exceeds maximum_working_bytes",
        ));
    }
    require_storage(parent, initial, request.configuration.minimum_free_bytes)?;
    fs::create_dir(output).map_err(|_| invalid("cannot reserve fresh video signal workspace"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(output, fs::Permissions::from_mode(0o700))
            .map_err(|_| invalid("cannot make video signal workspace private"))?;
    }
    ensure_canonical_directory(output)?;
    let snapshot = output.join("source.media");
    copy_pinned(
        &request.source_video,
        &snapshot,
        &source,
        request.configuration.maximum_source_bytes,
        token,
    )?;
    let mut settings = request.configuration.clone();
    settings.ffmpeg.executable = output.join("ffmpeg");
    settings.ffprobe.executable = output.join("ffprobe");
    let provider = output.join("aniflow-provider");
    for ((original, destination), expected) in [
        (
            &request.configuration.ffmpeg.executable,
            &settings.ffmpeg.executable,
        ),
        (
            &request.configuration.ffprobe.executable,
            &settings.ffprobe.executable,
        ),
        (&request.provider_executable, &provider),
    ]
    .into_iter()
    .zip(&components)
    {
        copy_pinned(original, destination, expected, 512 * 1024 * 1024, token)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(destination, fs::Permissions::from_mode(0o700))
                .map_err(|_| invalid("cannot enable pinned private tool"))?;
        }
    }
    let (inspection, channels) = crate::video_assembly::media::observe_meter_source(
        &settings,
        &snapshot,
        &request.selection,
        output,
        token,
    )?;
    if inspection.source_sha256 != request.source_sha256
        || inspection.source_size_bytes != request.source_bytes
    {
        return Err(invalid("video meter observation changed source identity"));
    }
    let (stream, range, expected_frames) = extent(&inspection)?;
    let expected_wav_bytes = expected_frames
        .checked_mul(u64::from(channels) * 4)
        .and_then(|v| v.checked_add(4096))
        .ok_or_else(|| invalid("PCM byte bound overflow"))?;
    if expected_wav_bytes > request.maximum_pcm_bytes {
        return Err(invalid(
            "complete original audio exceeds requested PCM bound; no truncation is accepted",
        ));
    }
    let remaining = expected_wav_bytes
        .checked_mul(10)
        .and_then(|v| v.checked_add(32 * 1024 * 1024))
        .ok_or_else(|| invalid("signal analysis storage overflow"))?;
    if initial
        .checked_add(remaining)
        .is_none_or(|v| v > settings.maximum_working_bytes)
    {
        return Err(invalid(
            "source-bound signal analysis exceeds maximum_working_bytes",
        ));
    }
    require_storage(output, remaining, settings.minimum_free_bytes)?;
    let pcm = output.join("selected-audio.wav");
    let arguments = decode_arguments(&snapshot, &pcm, stream, request.maximum_pcm_bytes);
    let capture = run_tool_isolated_with_limits(
        &settings.ffmpeg,
        "ffmpeg",
        &arguments,
        &settings.tool_limits(),
        token,
        GroupPolicy::Own,
        Some(output),
        settings.timeout_milliseconds,
        settings.maximum_capture_bytes as u64,
    )
    .map_err(|_| {
        invalid("selected source audio decode failed/cancelled or exceeded capture bounds")
    })?;
    if !capture.stdout.is_empty() || !capture.stderr.is_empty() {
        return Err(invalid(
            "selected audio decoder returned unexpected diagnostics",
        ));
    }
    let pcm_identity = identity(&pcm, request.maximum_pcm_bytes)?;
    let wave = crate::audio_inspection::wav::inspect_native(&pcm)?;
    if wave.sample_format != crate::audio_inspection::NativeSampleFormat::Float32
        || wave.sample_rate_hz != 48_000
        || wave.channels != channels
        || wave.frame_count != expected_frames
    {
        return Err(invalid(
            "decoded PCM count, native format, rate or channels differ from complete source audio; no trim/pad/resample/downmix is accepted",
        ));
    }
    let configuration = settings.tool_limits();
    let analysis = crate::audio_signal::run(
        SignalAnalysisRequest::new_native(
            AudioInspectionRequest::new(&pcm, configuration.clone(), &provider),
            NativeSignalAnalysisConfiguration::default(),
        ),
        Some(output.join("signal-runs")),
        token,
        |_| {},
    )
    .map_err(|e| e.error)?;
    let accepted = crate::status_v3(&analysis.run_directory)?;
    let summary = super::loudness_summary(&analysis.run_directory)?;
    if summary.report_schema != crate::audio_signal::AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3
        || summary.source.artifact.sha256 != pcm_identity.0
        || summary.source.artifact.byte_size != pcm_identity.1
        || summary.source.frame_count != expected_frames
        || summary.source.sample_rate_hz != 48_000
        || summary.source.channels != channels
        || summary.source.stream_index != 0
        || summary.source.stem.is_some()
        || summary.source.origin.numerator != 0
        || summary.source.origin.denominator != 1
    {
        return Err(invalid(
            "accepted #13 signal report differs from the exact decoded source audio",
        ));
    }
    let loudness =
        crate::audio_signal::loudness::measure(&pcm, &pcm_identity, &configuration, output, token)?;
    if loudness.pcm_sha256 != wave.pcm_sha256
        || loudness.frames != expected_frames
        || loudness.channels != channels
    {
        return Err(invalid(
            "native meter differs from the accepted complete PCM subject",
        ));
    }
    if token.is_cancelled()
        || crate::status_v3(&analysis.run_directory)? != accepted
        || identity(&pcm, request.maximum_pcm_bytes)? != pcm_identity
        || identity(&request.source_video, settings.maximum_source_bytes)? != source
        || identity(&snapshot, settings.maximum_source_bytes)? != source
    {
        return Err(invalid(
            "source/PCM/acceptance changed during video audio composition",
        ));
    }
    for (input, expected) in [
        &request.configuration.ffmpeg.executable,
        &request.configuration.ffprobe.executable,
        &request.provider_executable,
    ]
    .into_iter()
    .zip(&components)
    {
        if identity(input, 512 * 1024 * 1024)? != *expected {
            return Err(invalid(
                "source-bound signal tools changed during observation",
            ));
        }
    }
    let receipt = VideoAudioSignalReceipt {
        schema: "aniflow.video-audio-signal/v1".into(),
        source_sha256: source.0,
        source_bytes: source.1,
        audio_stream: stream,
        temporal_sha256: inspection.sha256().map_err(mapped)?,
        source_audio_extent: range,
        sample_rate_hz: 48_000,
        channels,
        frames: expected_frames,
        decoded_wav_sha256: pcm_identity.0,
        decoded_wav_bytes: pcm_identity.1,
        decoded_pcm_sha256: wave.pcm_sha256,
        ffmpeg_sha256: settings.ffmpeg.sha256,
        ffprobe_sha256: settings.ffprobe.sha256,
        provider_sha256: components[2].0.clone(),
        decode_arguments: arguments
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect(),
        signal_run: analysis
            .run_directory
            .strip_prefix(output)
            .map_err(|_| invalid("signal child escaped owned workspace"))?
            .to_str()
            .ok_or_else(|| invalid("signal child path is not UTF-8"))?
            .into(),
        signal_plan_sha256: analysis.plan_sha256,
        signal_report_sha256: summary.report_sha256,
        loudness,
        normalization: "none".into(),
        runtime_qualified: false,
    };
    let bytes = crate::provider::canonical_json_bytes(&receipt)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("video-audio-signal.json"))
        .map_err(|_| invalid("cannot reserve private source-bound signal receipt"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| invalid("cannot commit private source-bound signal receipt"))?;
    Ok(receipt)
}

/// Read current, pinned composition evidence without invoking a media tool.
/// The native signal child owns acceptance; the separate meter remains an
/// unqualified observation bound to the same complete PCM subject.
pub fn observe(
    request: &VideoAudioSignalRequest,
    output: &Path,
    receipt_sha256: &str,
    token: &CancellationToken,
) -> Result<(VideoAudioSignalReceipt, super::DeliveryLoudnessSummary)> {
    use crate::destination::local;
    request.configuration.validate()?;
    crate::provider::require_sha256(receipt_sha256, "video signal receipt")?;
    ensure_canonical_directory(output)?;
    if token.is_cancelled()
        || request.schema != "aniflow.video-audio-signal-request/v1"
        || request.configuration.delivery_metadata.is_some()
        || request.selection.no_audio
        || request.selection.video_stream.is_none()
        || request.selection.audio_stream.is_none()
        || !(1024..=AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&request.maximum_pcm_bytes)
    {
        return Err(invalid("invalid or cancelled source-bound signal readback"));
    }
    let path = output.join("video-audio-signal.json");
    let pin = identity(&path, local::DOCUMENT_BYTES)?;
    if pin.0 != receipt_sha256 {
        return Err(invalid("video signal receipt differs from external pin"));
    }
    let receipt: VideoAudioSignalReceipt = local::read(&path)?;
    let summary = validate_retained(request, output, &receipt, token)?;
    if identity(&path, local::DOCUMENT_BYTES)? != pin {
        return Err(invalid("source signal receipt changed during observation"));
    }
    Ok((receipt, summary))
}

// Shared exact-byte/current-native checks, also used before publication of a
// recognized resumable receipt. No decoder/probe/meter is launched here.
pub(super) fn validate_retained(
    request: &VideoAudioSignalRequest,
    output: &Path,
    receipt: &VideoAudioSignalReceipt,
    token: &CancellationToken,
) -> Result<super::DeliveryLoudnessSummary> {
    request.configuration.validate()?;
    ensure_canonical_directory(output)?;
    let child_path = Path::new(&receipt.signal_run);
    if child_path.as_os_str().is_empty()
        || receipt.signal_run.contains('\\')
        || child_path.is_absolute()
        || child_path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(invalid(
            "signal child must remain in its declared relative workspace",
        ));
    }
    let child = output.join(child_path);
    ensure_canonical_directory(&child)?;
    let accepted = crate::status_v3(&child)?;
    let summary = super::loudness_summary(&child)?;
    let pcm = output.join("selected-audio.wav");
    let pcm_pin = (
        receipt.decoded_wav_sha256.clone(),
        receipt.decoded_wav_bytes,
    );
    let source = (request.source_sha256.clone(), request.source_bytes);
    if receipt.schema != "aniflow.video-audio-signal/v1"
        || receipt.normalization != "none"
        || receipt.runtime_qualified
        || receipt.source_sha256 != request.source_sha256
        || receipt.source_bytes != request.source_bytes
        || Some(receipt.audio_stream) != request.selection.audio_stream
        || receipt.sample_rate_hz != 48_000
        || !matches!(receipt.channels, 1 | 2)
        || receipt.frames == 0
        || receipt.decoded_wav_bytes > request.maximum_pcm_bytes
        || receipt.ffmpeg_sha256 != request.configuration.ffmpeg.sha256
        || receipt.ffprobe_sha256 != request.configuration.ffprobe.sha256
        || accepted.payload.state != crate::PipelineV3RunState::Complete
        || accepted.payload.plan_sha256 != receipt.signal_plan_sha256
        || summary.report_schema != crate::audio_signal::AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3
        || summary.report_sha256 != receipt.signal_report_sha256
        || summary.source.artifact.sha256 != pcm_pin.0
        || summary.source.artifact.byte_size != pcm_pin.1
        || summary.source.frame_count != receipt.frames
        || summary.source.sample_rate_hz != receipt.sample_rate_hz
        || summary.source.channels != receipt.channels
        || summary.source.stream_index != 0
        || summary.source.stem.is_some()
        || summary.source.origin.numerator != 0
        || summary.source.origin.denominator != 1
        || receipt.loudness.pcm_sha256 != receipt.decoded_pcm_sha256
        || receipt.loudness.frames != receipt.frames
        || receipt.loudness.channels != receipt.channels
        || receipt.loudness.sample_rate_hz != receipt.sample_rate_hz
    {
        return Err(invalid(
            "source-bound signal receipt differs from its current complete accepted PCM child",
        ));
    }
    crate::audio_signal::loudness::validate_receipt(&receipt.loudness)?;
    for (tool, expected) in [
        (
            &request.configuration.ffmpeg.executable,
            &receipt.ffmpeg_sha256,
        ),
        (
            &request.configuration.ffprobe.executable,
            &receipt.ffprobe_sha256,
        ),
        (&request.provider_executable, &receipt.provider_sha256),
        (&output.join("ffmpeg"), &receipt.ffmpeg_sha256),
        (&output.join("ffprobe"), &receipt.ffprobe_sha256),
        (&output.join("aniflow-provider"), &receipt.provider_sha256),
    ] {
        if identity(tool, 512 * 1024 * 1024)?.0 != *expected {
            return Err(invalid("source-bound signal tool identity changed"));
        }
    }
    if identity(
        &request.source_video,
        request.configuration.maximum_source_bytes,
    )? != source
        || identity(
            &output.join("source.media"),
            request.configuration.maximum_source_bytes,
        )? != source
        || identity(&pcm, request.maximum_pcm_bytes)? != pcm_pin
    {
        return Err(invalid(
            "source-bound signal video or retained complete PCM changed",
        ));
    }
    let wave = crate::audio_inspection::wav::inspect_native(&pcm)?;
    let frames = receipt
        .source_audio_extent
        .duration()
        .map_err(mapped)?
        .checked_mul(48_000)
        .map_err(mapped)?;
    if wave.sample_format != crate::audio_inspection::NativeSampleFormat::Float32
        || wave.pcm_sha256 != receipt.decoded_pcm_sha256
        || wave.sample_rate_hz != receipt.sample_rate_hz
        || wave.channels != receipt.channels
        || wave.frame_count != receipt.frames
        || frames.denominator() != 1
        || frames.numerator() <= 0
        || frames.numerator() as u64 != receipt.frames
        || receipt.decode_arguments
            != decode_arguments(
                &output.join("source.media"),
                &pcm,
                receipt.audio_stream,
                request.maximum_pcm_bytes,
            )
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
        || identity(&pcm, request.maximum_pcm_bytes)? != pcm_pin
        || crate::status_v3(&child)? != accepted
        || token.is_cancelled()
    {
        return Err(invalid(
            "complete source signal subject/command/acceptance changed during readback",
        ));
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoder_selects_one_stream_without_normalization_resampling_or_lossy_encoding() {
        let args = decode_arguments(
            Path::new("/private/a video.mp4"),
            Path::new("/private/out.wav"),
            3,
            1_048_576,
        );
        let args = args.iter().map(|a| a.to_str().unwrap()).collect::<Vec<_>>();
        assert!(args.windows(2).any(|v| v == ["-map", "0:3"]));
        assert!(args.contains(&"/private/a video.mp4"));
        assert!(args.windows(2).any(|v| v == ["-c:a", "pcm_f32le"]));
        assert!(!args.iter().any(|v| matches!(
            *v,
            "-shortest" | "-af" | "-ar" | "-ac" | "-ss" | "-t" | "-y"
        )));
    }
    #[test]
    fn full_decoded_extent_binds_the_original_stream_and_rejects_a_gap() {
        let doc: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../tests/fixtures/temporal/synchronized-audio-video.json"
        ))
        .unwrap();
        let selection = serde_json::from_value(doc["selection"].clone()).unwrap();
        let observations = doc["observations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                (
                    o["stream_index"].as_u64().unwrap() as u32,
                    o["frames"].clone(),
                    o["packets"].clone(),
                )
            })
            .collect::<Vec<_>>();
        let mut inspection = crate::temporal::from_probe_documents(
            &"a".repeat(64),
            1,
            &selection,
            &doc["inventory"],
            &observations,
        )
        .unwrap();
        let (stream, _, frames) = extent(&inspection).unwrap();
        assert_eq!(stream, 8);
        assert_eq!(frames, 8008);
        let audio = inspection
            .timelines
            .iter_mut()
            .find(|t| t.stream_index == 8)
            .unwrap();
        audio.frames[0].source_time.start = audio.frames[0]
            .source_time
            .start
            .checked_add(crate::temporal::RationalTime::new(1, 48_000).unwrap())
            .unwrap();
        assert!(extent(&inspection).is_err());
    }
}
