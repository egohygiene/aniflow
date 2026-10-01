//! Safe, resumable temporal segmentation and reconstruction.

use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::command::{self, ProcessLimits};
use crate::media::{self, MediaInspection};
use crate::temporal::{
    self, RationalTime, SourceStreamWindow, StreamSelection, TemporalCode, TemporalInspection,
    TimeRange, diagnostic,
};

pub const SEGMENT_PLAN_SCHEMA_V1: &str = "aniflow.segment-plan/v1";
pub const SEGMENT_MANIFEST_SCHEMA_V1: &str = "aniflow.segment-manifest/v1";
pub const RECONSTRUCTION_REPORT_SCHEMA_V1: &str = "aniflow.reconstruction-report/v1";
pub const SEGMENT_PLAN_SCHEMA_V2: &str = "aniflow.segment-plan/v2";
pub const SEGMENT_MANIFEST_SCHEMA_V2: &str = "aniflow.segment-manifest/v2";
pub const RECONSTRUCTION_REPORT_SCHEMA_V2: &str = "aniflow.reconstruction-report/v2";
pub const SEGMENT_CAPABILITY_ID_V1: &str = "media.video.segment/v1";
pub const RECONSTRUCT_CAPABILITY_ID_V1: &str = "media.video.reconstruct/v1";

const MAXIMUM_SEGMENT_DURATION_MS: u64 = 29_999;
const DEFAULT_PROCESS_TIMEOUT_SECONDS: u64 = 3_600;
const MAXIMUM_PROCESS_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SegmentMode {
    StreamCopy,
    TranscodeH264Aac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BoundaryAccuracy {
    KeyframeAligned,
    FrameAccurate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SegmentRequest {
    pub input: PathBuf,
    pub output_directory: PathBuf,
    pub segment_duration_ms: u64,
    pub mode: SegmentMode,
    pub process_timeout_seconds: u64,
    #[serde(default)]
    pub stream_selection: StreamSelection,
}

impl SegmentRequest {
    #[must_use]
    pub fn new(
        input: impl Into<PathBuf>,
        output_directory: impl Into<PathBuf>,
        segment_duration_ms: u64,
        mode: SegmentMode,
    ) -> Self {
        Self {
            input: input.into(),
            output_directory: output_directory.into(),
            segment_duration_ms,
            mode,
            process_timeout_seconds: DEFAULT_PROCESS_TIMEOUT_SECONDS,
            stream_selection: StreamSelection::default(),
        }
    }

    #[must_use]
    pub const fn with_process_timeout_seconds(mut self, seconds: u64) -> Self {
        self.process_timeout_seconds = seconds;
        self
    }

    #[must_use]
    pub fn with_stream_selection(mut self, selection: StreamSelection) -> Self {
        self.stream_selection = selection;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SegmentPlan {
    pub schema: String,
    pub capability_id: String,
    pub source: PathBuf,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub source_media: MediaInspection,
    pub output_directory: PathBuf,
    pub segment_duration_ms: u64,
    pub expected_segments: u64,
    pub segments: Vec<PlannedSegment>,
    pub mode: SegmentMode,
    pub boundary_accuracy: BoundaryAccuracy,
    pub stream_policy: String,
    pub estimated_required_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PlannedSegment {
    pub index: u64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub source_time: TimeRange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SegmentRecord {
    pub index: u64,
    pub path: PathBuf,
    pub requested_start_ms: u64,
    pub requested_end_ms: u64,
    pub actual_duration_ms: u64,
    pub boundary_accuracy: BoundaryAccuracy,
    pub sha256: String,
    pub size_bytes: u64,
    pub source_streams: Vec<SourceStreamWindow>,
    pub observed_frame_count: u64,
    pub output_temporal: TemporalInspection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SegmentManifest {
    pub schema: String,
    pub capability_id: String,
    pub aniflow_version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub complete: bool,
    pub plan_sha256: String,
    pub source: PathBuf,
    pub source_sha256: String,
    pub source_duration_ms: u64,
    pub segment_duration_ms: u64,
    pub mode: SegmentMode,
    pub boundary_accuracy: BoundaryAccuracy,
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
    pub segments: Vec<SegmentRecord>,
    pub source_temporal: TemporalInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SegmentOutcome {
    pub run_directory: PathBuf,
    pub manifest: PathBuf,
    pub segment_count: u64,
    pub resumed_segments: u64,
    pub generated_segments: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReconstructionReport {
    pub schema: String,
    pub capability_id: String,
    pub aniflow_version: String,
    pub manifest: PathBuf,
    pub manifest_sha256: String,
    pub output: PathBuf,
    pub output_sha256: String,
    pub output_duration_ms: u64,
    pub source_duration_ms: u64,
    pub duration_delta_ms: u64,
    pub tolerance_ms: u64,
    pub validated: bool,
    pub source_temporal_sha256: String,
    pub source_streams: Vec<SourceStreamWindow>,
    pub strategy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SegmentProgress {
    Planned { segments: u64 },
    SegmentStarted { index: u64, total: u64 },
    SegmentReused { index: u64, total: u64 },
    SegmentComplete { index: u64, total: u64 },
    ReconstructionStarted,
    ReconstructionComplete,
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn shared(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}

pub fn plan(request: &SegmentRequest) -> Result<SegmentPlan> {
    validate_request(request)?;
    command::require_executable("ffmpeg")?;
    command::require_executable("ffprobe")?;
    let source = request
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", request.input.display()))?;
    let source_media = media::inspect_with_selection(&source, &request.stream_selection)?;
    source_media.require_processing()?;
    let source_size_bytes = fs::metadata(&source)?.len();
    let source_duration_ms = source_media
        .exact()?
        .video_timeline()?
        .source_time
        .context("source time range is missing")?
        .duration()?
        .milliseconds_ceil()?;
    if source_duration_ms == 0 {
        bail!("source duration must be greater than zero");
    }
    let (boundary_accuracy, stream_policy, multiplier, segments) = match request.mode {
        SegmentMode::StreamCopy => (
            BoundaryAccuracy::KeyframeAligned,
            "copy only explicitly selected streams; exact keyframe source intervals; acknowledge every discard".to_owned(),
            1.10,
            plan_temporal_segments(source_media.exact()?, request.segment_duration_ms, true)?,
        ),
        SegmentMode::TranscodeH264Aac => (
            BoundaryAccuracy::FrameAccurate,
            "encode explicitly selected video/audio; trim exact frame ordinals and ceil audio sample boundaries; reconstruct restores original selected audio"
                .to_owned(),
            2.0,
            plan_temporal_segments(source_media.exact()?, request.segment_duration_ms, false)?,
        ),
    };
    let expected_segments = segments.len() as u64;
    Ok(SegmentPlan {
        schema: SEGMENT_PLAN_SCHEMA_V2.to_owned(),
        capability_id: SEGMENT_CAPABILITY_ID_V1.to_owned(),
        source_sha256: source_media.exact()?.source_sha256.clone(),
        source_size_bytes,
        source: source.clone(),
        source_media,
        output_directory: absolute_output_directory(&request.output_directory)?,
        segment_duration_ms: request.segment_duration_ms,
        expected_segments,
        segments,
        mode: request.mode,
        boundary_accuracy,
        stream_policy,
        estimated_required_bytes: (source_size_bytes as f64 * multiplier).ceil() as u64,
    })
}

pub fn execute(
    request: &SegmentRequest,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(&SegmentProgress),
) -> Result<SegmentOutcome> {
    let plan = plan(request)?;
    prepare_new_run(&plan.output_directory)?;
    fs::create_dir_all(plan.output_directory.join("segments"))?;
    fs::create_dir_all(plan.output_directory.join("state"))?;
    fs::create_dir_all(plan.output_directory.join("logs"))?;
    fs::create_dir_all(plan.output_directory.join("delivery"))?;
    write_json_atomic(&plan.output_directory.join("state/request.json"), request)?;
    write_json_atomic(&plan.output_directory.join("state/plan.json"), &plan)?;
    let now = Utc::now();
    let mut manifest = SegmentManifest {
        schema: SEGMENT_MANIFEST_SCHEMA_V2.to_owned(),
        capability_id: SEGMENT_CAPABILITY_ID_V1.to_owned(),
        aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: now,
        updated_at: now,
        complete: false,
        plan_sha256: digest_json(&plan)?,
        source: plan.source.clone(),
        source_sha256: plan.source_sha256.clone(),
        source_duration_ms: plan
            .source_media
            .exact()?
            .video_timeline()?
            .source_time
            .context("source range is missing")?
            .duration()?
            .milliseconds_ceil()?,
        segment_duration_ms: plan.segment_duration_ms,
        mode: plan.mode,
        boundary_accuracy: plan.boundary_accuracy,
        ffmpeg_version: command::executable_summary("ffmpeg")?,
        ffprobe_version: command::executable_summary("ffprobe")?,
        segments: Vec::new(),
        source_temporal: plan.source_media.exact()?.clone(),
    };
    write_checkpoint(&plan.output_directory, &mut manifest)?;
    progress(&SegmentProgress::Planned {
        segments: plan.expected_segments,
    });
    execute_plan(&plan, request, &mut manifest, cancellation, progress, 0)
}

pub fn resume(
    run_directory: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(&SegmentProgress),
) -> Result<SegmentOutcome> {
    require_current_segment_schema(&run_directory.join("state/checkpoint.json"))?;
    let request: SegmentRequest = read_json(&run_directory.join("state/request.json"))?;
    let plan: SegmentPlan = read_json(&run_directory.join("state/plan.json"))?;
    let mut manifest: SegmentManifest = read_json(&run_directory.join("state/checkpoint.json"))?;
    let canonical_run = run_directory
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", run_directory.display()))?;
    if canonical_run != plan.output_directory {
        bail!("segment run directory moved; refusing to resume ambiguous path state");
    }
    if sha256_file(&plan.source)? != plan.source_sha256
        || manifest.source_sha256 != plan.source_sha256
        || digest_json(&plan)? != manifest.plan_sha256
        || manifest.source_temporal != *plan.source_media.exact()?
    {
        return Err(diagnostic(
            TemporalCode::SourceChanged,
            None,
            "segment source, plan or exact temporal evidence changed",
        )
        .into());
    }
    let current =
        media::inspect_with_selection(&plan.source, &manifest.source_temporal.selection_intent)?;
    current.require_processing()?;
    if current.exact()? != &manifest.source_temporal {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "segment source timeline changed",
        )
        .into());
    }
    if manifest.complete {
        verify_manifest_segments(&plan.output_directory, &manifest)?;
        let delivery_manifest = plan.output_directory.join("delivery/segment-manifest.json");
        if !delivery_manifest.is_file() {
            write_json_atomic(&delivery_manifest, &manifest)?;
        }
        return outcome(
            &plan.output_directory,
            &manifest,
            manifest.segments.len() as u64,
            0,
        );
    }
    if sha256_file(&plan.source)? != plan.source_sha256 {
        bail!("source checksum changed; refusing to resume with different media");
    }
    if digest_json(&plan)? != manifest.plan_sha256 {
        bail!("segment plan digest changed; refusing incompatible resume");
    }
    let reusable = retain_valid_prefix(&plan.output_directory, &mut manifest)?;
    progress(&SegmentProgress::Planned {
        segments: plan.expected_segments,
    });
    execute_plan(
        &plan,
        &request,
        &mut manifest,
        cancellation,
        progress,
        reusable,
    )
}

pub fn reconstruct(
    run_directory: &Path,
    output: &Path,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(&SegmentProgress),
) -> Result<ReconstructionReport> {
    let manifest_path = run_directory.join("delivery/segment-manifest.json");
    require_current_segment_schema(&manifest_path)?;
    let manifest: SegmentManifest = read_json(&manifest_path)?;
    let plan: SegmentPlan = read_json(&run_directory.join("state/plan.json"))?;
    if digest_json(&plan)? != manifest.plan_sha256
        || plan.source_media.exact()? != &manifest.source_temporal
        || plan.source_sha256 != manifest.source_sha256
        || plan.source != manifest.source
    {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "reconstruction manifest disagrees with the frozen source plan",
        )
        .into());
    }
    if !manifest.complete {
        bail!("only a complete segment manifest can be reconstructed");
    }
    verify_manifest_segments(run_directory, &manifest)?;
    if sha256_file(&manifest.source)? != manifest.source_sha256 {
        return Err(diagnostic(
            TemporalCode::SourceChanged,
            None,
            "original source changed before reconstruction",
        )
        .into());
    }
    let source = &manifest.source_temporal;
    source.require_processing()?;
    let selected = source
        .selected
        .as_ref()
        .context("source selection is missing")?;
    let period = source
        .video_timeline()?
        .frame_period
        .context("source period is missing")?;
    progress(&SegmentProgress::ReconstructionStarted);
    let concat_path = run_directory.join("concat.txt");
    let concat = manifest
        .segments
        .iter()
        .map(|segment| {
            let name = segment
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .context("segment filename is not UTF-8")?;
            Ok(format!("file 'segments/{name}'\n"))
        })
        .collect::<Result<String>>()?;
    fs::write(&concat_path, concat)?;
    let output = absolute_output_path(output)?;
    if output.exists() {
        bail!("reconstruction output already exists: {}", output.display());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = temporary_media_path(&output);
    remove_file_if_present(&temporary)?;
    let mut arguments: Vec<OsString> = [
        "-hide_banner",
        "-nostdin",
        "-y",
        "-f",
        "concat",
        "-safe",
        "1",
        "-i",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    arguments.push(concat_path.as_os_str().to_owned());
    if selected.audio.is_some() {
        arguments.extend([OsString::from("-i"), manifest.source.as_os_str().to_owned()]);
    }
    arguments.extend([OsString::from("-map"), OsString::from("0:v:0")]);
    if let Some(audio) = selected.audio {
        arguments.extend([
            OsString::from("-map"),
            OsString::from(format!("1:{audio}")),
            OsString::from("-c:a"),
            OsString::from("aac"),
            OsString::from("-b:a"),
            OsString::from("192k"),
        ]);
    }
    // The manifest proves a CFR source grid. Reassign that exact declared grid
    // instead of trusting per-segment codec padding as the concatenation clock.
    arguments.extend([
        OsString::from("-vf"),
        OsString::from(format!(
            "setpts=N*{}/({}*TB)",
            period.numerator(),
            period.denominator()
        )),
        OsString::from("-r"),
        OsString::from(period.reciprocal()?.to_string()),
        OsString::from("-fps_mode"),
        OsString::from("passthrough"),
        OsString::from("-c:v"),
        OsString::from("libx264"),
        OsString::from("-bf"),
        OsString::from("0"),
        OsString::from("-crf"),
        OsString::from("18"),
        OsString::from("-preset"),
        OsString::from("medium"),
        OsString::from("-movflags"),
        OsString::from("+faststart"),
        temporary.as_os_str().to_owned(),
    ]);
    let result = (|| -> Result<MediaInspection> {
        let bounded = command::run_bounded(
            "ffmpeg",
            arguments,
            ProcessLimits {
                timeout: Duration::from_secs(DEFAULT_PROCESS_TIMEOUT_SECONDS),
                maximum_output_bytes: MAXIMUM_PROCESS_OUTPUT_BYTES,
            },
            Some(cancellation.shared()),
        )?;
        write_process_log(&run_directory.join("logs/reconstruct.log"), &bounded)?;
        if !bounded.status.success() || bounded.stdout_truncated || bounded.stderr_truncated {
            bail!("bounded FFmpeg reconstruction failed; see logs/reconstruct.log");
        }
        let inspected = media::inspect(&temporary)?;
        temporal::validate_reconstruction(source, inspected.exact()?, false)?;
        if sha256_file(&manifest.source)? != manifest.source_sha256 {
            return Err(diagnostic(
                TemporalCode::SourceChanged,
                None,
                "source changed during reconstruction",
            )
            .into());
        }
        Ok(inspected)
    })();
    remove_file_if_present(&concat_path)?;
    let inspection = match result {
        Ok(value) => value,
        Err(error) => {
            remove_file_if_present(&temporary)?;
            return Err(error);
        }
    };
    // Exclusive publication does not replace a path created after preflight.
    fs::hard_link(&temporary, &output)
        .context("failed to publish reconstruction without replacing an existing file")?;
    remove_file_if_present(&temporary)?;
    let output_duration_ms = inspection
        .exact()?
        .video_timeline()?
        .source_time
        .context("output time range is missing")?
        .duration()?
        .milliseconds_ceil()?;
    let support = temporal::assess_processing(source)?;
    let tolerance_ms = support
        .video_frame_tolerance
        .context("video tolerance is missing")?
        .checked_add(
            support
                .audio_boundary_tolerance
                .unwrap_or(RationalTime::ZERO),
        )?
        .milliseconds_ceil()?;
    let report = ReconstructionReport {
        schema: RECONSTRUCTION_REPORT_SCHEMA_V2.to_owned(), capability_id: RECONSTRUCT_CAPABILITY_ID_V1.to_owned(), aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        manifest_sha256: sha256_file(&manifest_path)?, manifest: manifest_path, output_sha256: sha256_file(&output)?, output,
        output_duration_ms, source_duration_ms: manifest.source_duration_ms, duration_delta_ms: output_duration_ms.abs_diff(manifest.source_duration_ms), tolerance_ms, validated: true,
        source_temporal_sha256: source.sha256()?, source_streams: selected.video.into_iter().chain(selected.audio).map(|i| Ok(SourceStreamWindow { stream_index: i, source_time: source.timeline(i).and_then(|t| t.source_time).context("selected source interval is missing")?, frame_ordinal: None })).collect::<Result<Vec<_>>>()?,
        strategy: "concatenate verified video frame ordinals on the declared original CFR grid; restore original selected source audio once as AAC; segmented audio is not the reconstruction clock".to_owned(),
    };
    write_json_atomic(
        &run_directory.join("delivery/reconstruction-report.json"),
        &report,
    )?;
    progress(&SegmentProgress::ReconstructionComplete);
    Ok(report)
}

fn execute_plan(
    plan: &SegmentPlan,
    request: &SegmentRequest,
    manifest: &mut SegmentManifest,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(&SegmentProgress),
    reusable: u64,
) -> Result<SegmentOutcome> {
    if plan.mode == SegmentMode::StreamCopy {
        return execute_stream_copy_plan(plan, request, manifest, cancellation, progress, reusable);
    }
    for index in 0..plan.expected_segments {
        if cancellation.is_cancelled() {
            bail!("segmentation was cancelled before segment {}", index + 1);
        }
        if index < reusable {
            progress(&SegmentProgress::SegmentReused {
                index: index + 1,
                total: plan.expected_segments,
            });
            continue;
        }
        progress(&SegmentProgress::SegmentStarted {
            index: index + 1,
            total: plan.expected_segments,
        });
        let planned = plan
            .segments
            .get(index as usize)
            .context("segment plan is missing a declared boundary")?;
        let requested_start_ms = planned.start_ms;
        let requested_end_ms = planned.end_ms;
        let filename = format!("segment-{index:06}.mp4");
        let destination = plan.output_directory.join("segments").join(&filename);
        let temporary = plan
            .output_directory
            .join("segments")
            .join(format!(".{filename}.part.mp4"));
        remove_file_if_present(&temporary)?;
        let arguments = segment_arguments(plan, planned, &temporary)?;
        let bounded = command::run_bounded(
            "ffmpeg",
            arguments,
            ProcessLimits {
                timeout: Duration::from_secs(request.process_timeout_seconds),
                maximum_output_bytes: MAXIMUM_PROCESS_OUTPUT_BYTES,
            },
            Some(cancellation.shared()),
        );
        let bounded = match bounded {
            Ok(output) => output,
            Err(error) => {
                remove_file_if_present(&temporary)?;
                return Err(error);
            }
        };
        write_process_log(
            &plan
                .output_directory
                .join("logs")
                .join(format!("segment-{index:06}.log")),
            &bounded,
        )?;
        if !bounded.status.success() {
            remove_file_if_present(&temporary)?;
            bail!("ffmpeg failed while generating segment {}", index + 1);
        }
        if bounded.stdout_truncated || bounded.stderr_truncated {
            remove_file_if_present(&temporary)?;
            bail!("ffmpeg diagnostics exceeded the 4 MiB limit");
        }
        if !temporary.is_file() {
            bail!(
                "ffmpeg reported success without creating segment {}",
                index + 1
            );
        }
        let inspection = media::inspect(&temporary)?;
        validate_segment_output(
            plan.source_media.exact()?,
            planned.source_time,
            inspection.exact()?,
        )?;
        fs::rename(&temporary, &destination)?;
        manifest.segments.push(SegmentRecord {
            index,
            path: PathBuf::from("segments").join(filename),
            requested_start_ms,
            requested_end_ms,
            actual_duration_ms: inspection
                .exact()?
                .video_timeline()?
                .source_time
                .context("segment time range is missing")?
                .duration()?
                .milliseconds_ceil()?,
            boundary_accuracy: plan.boundary_accuracy,
            sha256: sha256_file(&destination)?,
            size_bytes: fs::metadata(&destination)?.len(),
            source_streams: segment_source_windows(
                plan.source_media.exact()?,
                planned.source_time,
            )?,
            observed_frame_count: inspection.exact()?.video_timeline()?.frames.len() as u64,
            output_temporal: inspection.exact()?.clone(),
        });
        write_checkpoint(&plan.output_directory, manifest)?;
        progress(&SegmentProgress::SegmentComplete {
            index: index + 1,
            total: plan.expected_segments,
        });
    }
    finalize_manifest(plan, manifest, reusable)
}

fn execute_stream_copy_plan(
    plan: &SegmentPlan,
    request: &SegmentRequest,
    manifest: &mut SegmentManifest,
    cancellation: &CancellationToken,
    progress: &mut dyn FnMut(&SegmentProgress),
    reusable: u64,
) -> Result<SegmentOutcome> {
    let staging = plan.output_directory.join("state/stream-copy-staging");
    remove_directory_if_present(&staging)?;
    fs::create_dir_all(&staging)?;
    let split_times = plan
        .segments
        .iter()
        .take(plan.segments.len().saturating_sub(1))
        .map(|segment| segment.source_time.end.ffmpeg_microseconds())
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let pattern = staging.join("segment-%06d.mp4");
    let selected = plan
        .source_media
        .exact()?
        .selected
        .as_ref()
        .context("stream selection is missing")?;
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-nostdin"),
        OsString::from("-y"),
        OsString::from("-i"),
        plan.source.as_os_str().to_owned(),
        OsString::from("-map"),
        OsString::from(format!(
            "0:{}",
            selected.video.context("video stream is missing")?
        )),
        OsString::from("-c"),
        OsString::from("copy"),
        OsString::from("-f"),
        OsString::from("segment"),
        OsString::from("-segment_times"),
        OsString::from(split_times),
        OsString::from("-segment_time_delta"),
        OsString::from("0.000001"),
        OsString::from("-reset_timestamps"),
        OsString::from("1"),
        OsString::from("-segment_start_number"),
        OsString::from("0"),
        pattern.as_os_str().to_owned(),
    ];
    if let Some(audio) = selected.audio {
        arguments.splice(
            7..7,
            [OsString::from("-map"), OsString::from(format!("0:{audio}"))],
        );
    }
    let bounded = command::run_bounded(
        "ffmpeg",
        arguments,
        ProcessLimits {
            timeout: Duration::from_secs(request.process_timeout_seconds),
            maximum_output_bytes: MAXIMUM_PROCESS_OUTPUT_BYTES,
        },
        Some(cancellation.shared()),
    );
    let bounded = match bounded {
        Ok(output) => output,
        Err(error) => {
            remove_directory_if_present(&staging)?;
            return Err(error);
        }
    };
    write_process_log(
        &plan.output_directory.join("logs/stream-copy.log"),
        &bounded,
    )?;
    if !bounded.status.success() {
        remove_directory_if_present(&staging)?;
        bail!("ffmpeg stream-copy segmentation failed; see logs/stream-copy.log");
    }
    if bounded.stdout_truncated || bounded.stderr_truncated {
        remove_directory_if_present(&staging)?;
        bail!("ffmpeg diagnostics exceeded the 4 MiB limit");
    }
    let produced = fs::read_dir(&staging)?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().is_file())
        .count() as u64;
    if produced != plan.expected_segments {
        remove_directory_if_present(&staging)?;
        bail!(
            "ffmpeg produced {produced} stream-copy segments; the keyframe plan requires {}",
            plan.expected_segments
        );
    }
    for index in reusable..plan.expected_segments {
        if cancellation.is_cancelled() {
            remove_directory_if_present(&staging)?;
            bail!(
                "segmentation was cancelled before publishing segment {}",
                index + 1
            );
        }
        progress(&SegmentProgress::SegmentStarted {
            index: index + 1,
            total: plan.expected_segments,
        });
        let planned = plan
            .segments
            .get(index as usize)
            .context("segment plan is missing a declared boundary")?;
        let filename = format!("segment-{index:06}.mp4");
        let staged = staging.join(&filename);
        let destination = plan.output_directory.join("segments").join(&filename);
        let inspection = media::inspect(&staged)?;
        validate_segment_output(
            plan.source_media.exact()?,
            planned.source_time,
            inspection.exact()?,
        )?;
        fs::rename(&staged, &destination)?;
        manifest.segments.push(SegmentRecord {
            index,
            path: PathBuf::from("segments").join(filename),
            requested_start_ms: planned.start_ms,
            requested_end_ms: planned.end_ms,
            actual_duration_ms: inspection
                .exact()?
                .video_timeline()?
                .source_time
                .context("segment time range is missing")?
                .duration()?
                .milliseconds_ceil()?,
            boundary_accuracy: BoundaryAccuracy::KeyframeAligned,
            sha256: sha256_file(&destination)?,
            size_bytes: fs::metadata(&destination)?.len(),
            source_streams: segment_source_windows(
                plan.source_media.exact()?,
                planned.source_time,
            )?,
            observed_frame_count: inspection.exact()?.video_timeline()?.frames.len() as u64,
            output_temporal: inspection.exact()?.clone(),
        });
        write_checkpoint(&plan.output_directory, manifest)?;
        progress(&SegmentProgress::SegmentComplete {
            index: index + 1,
            total: plan.expected_segments,
        });
    }
    for index in 0..reusable {
        progress(&SegmentProgress::SegmentReused {
            index: index + 1,
            total: plan.expected_segments,
        });
    }
    remove_directory_if_present(&staging)?;
    finalize_manifest(plan, manifest, reusable)
}

fn finalize_manifest(
    plan: &SegmentPlan,
    manifest: &mut SegmentManifest,
    reusable: u64,
) -> Result<SegmentOutcome> {
    if sha256_file(&plan.source)? != plan.source_sha256 {
        return Err(diagnostic(
            TemporalCode::SourceChanged,
            None,
            "source changed during segmentation",
        )
        .into());
    }
    if manifest.segments.len() as u64 != plan.expected_segments {
        return Err(diagnostic(
            TemporalCode::FrameCountMismatch,
            None,
            "segment count differs from its exact plan",
        )
        .into());
    }
    manifest.complete = true;
    verify_manifest_segments(&plan.output_directory, manifest)?;
    manifest.updated_at = Utc::now();
    let manifest_path = plan.output_directory.join("delivery/segment-manifest.json");
    write_json_atomic(&manifest_path, manifest)?;
    write_json_atomic(
        &plan.output_directory.join("state/checkpoint.json"),
        manifest,
    )?;
    outcome(
        &plan.output_directory,
        manifest,
        reusable,
        plan.expected_segments - reusable,
    )
}

fn segment_arguments(
    plan: &SegmentPlan,
    planned: &PlannedSegment,
    output: &Path,
) -> Result<Vec<OsString>> {
    let source = plan.source_media.exact()?;
    let selected = source
        .selected
        .as_ref()
        .context("stream selection is missing")?;
    let video = selected.video.context("video stream is missing")?;
    let frames = source.video_timeline()?;
    let start = frames
        .frames
        .iter()
        .position(|f| f.source_time.start == planned.source_time.start)
        .context("segment start must be an exact source frame boundary")?;
    let end = frames
        .frames
        .iter()
        .position(|f| f.source_time.start == planned.source_time.end)
        .unwrap_or(frames.frames.len());
    let mut filters =
        format!("[0:{video}]trim=start_frame={start}:end_frame={end},setpts=PTS-STARTPTS[v]");
    if let Some(audio) = selected.audio {
        let rate = source
            .stream(audio)
            .and_then(|s| s.sample_rate_hz)
            .context("audio sample clock is missing")?;
        let first = planned.source_time.start.sample_boundary_ceil(rate)?;
        let last = planned.source_time.end.sample_boundary_ceil(rate)?;
        filters.push_str(&format!(
            ";[0:{audio}]atrim=start_sample={first}:end_sample={last},asetpts=PTS-STARTPTS[a]"
        ));
    }
    let mut arguments: Vec<OsString> = ["-hide_banner", "-nostdin", "-y", "-i"]
        .into_iter()
        .map(OsString::from)
        .collect();
    arguments.push(plan.source.as_os_str().to_owned());
    arguments.extend([
        OsString::from("-filter_complex"),
        OsString::from(filters),
        OsString::from("-map"),
        OsString::from("[v]"),
    ]);
    if selected.audio.is_some() {
        arguments.extend([
            OsString::from("-map"),
            OsString::from("[a]"),
            OsString::from("-c:a"),
            OsString::from("aac"),
            OsString::from("-b:a"),
            OsString::from("192k"),
        ]);
    }
    arguments.extend([
        OsString::from("-c:v"),
        OsString::from("libx264"),
        OsString::from("-bf"),
        OsString::from("0"),
        OsString::from("-preset"),
        OsString::from("medium"),
        OsString::from("-crf"),
        OsString::from("18"),
        OsString::from("-r"),
        OsString::from(plan.source_media.exact_frame_rate()?),
        OsString::from("-fps_mode"),
        OsString::from("passthrough"),
        OsString::from("-movflags"),
        OsString::from("+faststart"),
        output.as_os_str().to_owned(),
    ]);
    Ok(arguments)
}

pub(crate) fn validate_request(request: &SegmentRequest) -> Result<()> {
    if !request.input.is_file() {
        bail!("input video does not exist: {}", request.input.display());
    }
    if request.segment_duration_ms == 0 || request.segment_duration_ms > MAXIMUM_SEGMENT_DURATION_MS
    {
        bail!("segment duration must be between 1 and {MAXIMUM_SEGMENT_DURATION_MS} milliseconds");
    }
    if request.process_timeout_seconds == 0 {
        bail!("process timeout must be greater than zero");
    }
    Ok(())
}

fn plan_temporal_segments(
    source: &TemporalInspection,
    target_ms: u64,
    keyframes_only: bool,
) -> Result<Vec<PlannedSegment>> {
    source.require_processing()?;
    let timeline = source.video_timeline()?;
    if keyframes_only {
        for index in source
            .selected
            .as_ref()
            .context("stream selection is missing")?
            .video
            .into_iter()
            .chain(source.selected.as_ref().and_then(|s| s.audio))
        {
            let packets = &source
                .timeline(index)
                .context("selected packet clock is missing")?
                .packets;
            if packets.iter().any(|p| {
                p.presentation_ticks != p.decode_ticks
                    || p.presentation_ticks.is_some_and(|v| v < 0)
            }) {
                return Err(diagnostic(TemporalCode::UnsupportedStream, Some(index), "stream-copy reset cannot prove reordered/negative packet timing; use transcode-h264-aac").into());
            }
        }
    }
    let range = timeline.source_time.context("source range is missing")?;
    let target = RationalTime::new(i64::try_from(target_ms)?, 1000)?;
    let maximum = RationalTime::new(30, 1)?;
    let mut cursor = range.start;
    let mut segments = Vec::new();
    while cursor < range.end {
        let desired = cursor.checked_add(target)?;
        let end = if desired >= range.end {
            range.end
        } else if keyframes_only {
            timeline
                .frames
                .iter()
                .find(|f| {
                    f.keyframe && f.source_time.start >= desired && f.source_time.start > cursor
                })
                .map_or(range.end, |f| f.source_time.start)
        } else {
            timeline
                .frames
                .iter()
                .filter(|f| f.source_time.start > cursor && f.source_time.start <= desired)
                .next_back()
                .map(|f| f.source_time.start)
                .ok_or_else(|| {
                    diagnostic(
                        TemporalCode::InvalidDuration,
                        None,
                        "segment duration is smaller than one exact source frame",
                    )
                })?
        };
        let source_time = TimeRange::new(cursor, end)?;
        if source_time.duration()? >= maximum {
            return Err(diagnostic(
                TemporalCode::InvalidDuration,
                None,
                "exact frame/keyframe boundary would create a segment of 30 seconds or more",
            )
            .into());
        }
        let windows = segment_source_windows(source, source_time)?;
        if keyframes_only {
            if let Some(audio) = source.selected.as_ref().and_then(|s| s.audio) {
                let observed = source
                    .timeline(audio)
                    .context("audio timeline is missing")?;
                let window = windows
                    .last()
                    .context("audio source window is missing")?
                    .source_time;
                let boundaries = |time| {
                    observed
                        .frames
                        .iter()
                        .any(|f| f.source_time.start == time || f.source_time.end == time)
                };
                if !boundaries(window.start) || !boundaries(window.end) {
                    return Err(diagnostic(TemporalCode::UnsupportedStream, Some(audio), "stream-copy audio packet/frame boundaries do not align with selected video keyframes; use transcode-h264-aac").into());
                }
            }
        }
        segments.push(PlannedSegment {
            index: segments.len() as u64,
            start_ms: cursor.milliseconds_ceil()?,
            end_ms: end.milliseconds_ceil()?,
            source_time,
        });
        cursor = end;
    }
    Ok(segments)
}

fn prepare_new_run(root: &Path) -> Result<()> {
    if root.exists() {
        if fs::symlink_metadata(root)?.file_type().is_symlink() {
            bail!("segment output directory cannot be a symbolic link");
        }
        if !root.is_dir() {
            bail!("segment output path is not a directory: {}", root.display());
        }
        if fs::read_dir(root)?.next().is_some() {
            bail!(
                "segment output directory must be empty for a new run: {}",
                root.display()
            );
        }
    } else {
        fs::create_dir_all(root)?;
    }
    Ok(())
}

fn retain_valid_prefix(root: &Path, manifest: &mut SegmentManifest) -> Result<u64> {
    let mut valid = 0_usize;
    let mut cursor = RationalTime::ZERO;
    for (position, segment) in manifest.segments.iter().enumerate() {
        if validate_segment_record(root, manifest, segment, position, cursor).is_err() {
            break;
        }
        cursor = segment.source_streams[0].source_time.end;
        valid += 1;
    }
    manifest.segments.truncate(valid);
    write_checkpoint(root, manifest)?;
    Ok(valid as u64)
}

fn validate_segment_record(
    root: &Path,
    manifest: &SegmentManifest,
    segment: &SegmentRecord,
    position: usize,
    cursor: RationalTime,
) -> Result<()> {
    let expected_path = PathBuf::from("segments").join(format!("segment-{position:06}.mp4"));
    if segment.index != position as u64 || segment.path != expected_path {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "segment order/path changed",
        )
        .into());
    }
    let path = root.join(&segment.path);
    if fs::symlink_metadata(&path)?.file_type().is_symlink()
        || !path.canonicalize()?.starts_with(root.canonicalize()?)
        || !path.is_file()
        || sha256_file(&path)? != segment.sha256
    {
        return Err(diagnostic(
            TemporalCode::SourceChanged,
            None,
            "segment escaped its workspace or its digest changed",
        )
        .into());
    }
    let range = segment
        .source_streams
        .first()
        .context("segment source-stream binding is missing")?
        .source_time;
    if range.start != cursor
        || segment.source_streams != segment_source_windows(&manifest.source_temporal, range)?
        || segment.output_temporal.source_sha256 != segment.sha256
        || segment.observed_frame_count
            != segment.output_temporal.video_timeline()?.frames.len() as u64
        || segment.actual_duration_ms
            != segment
                .output_temporal
                .video_timeline()?
                .source_time
                .context("segment output interval is missing")?
                .duration()?
                .milliseconds_ceil()?
        || segment.requested_start_ms != range.start.milliseconds_ceil()?
        || segment.requested_end_ms != range.end.milliseconds_ceil()?
    {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "segment timing/evidence binding changed",
        )
        .into());
    }
    validate_segment_output(&manifest.source_temporal, range, &segment.output_temporal)
}

fn verify_manifest_segments(root: &Path, manifest: &SegmentManifest) -> Result<()> {
    if manifest.schema != SEGMENT_MANIFEST_SCHEMA_V2
        || manifest.source_sha256 != manifest.source_temporal.source_sha256
        || manifest.segments.is_empty()
    {
        return Err(diagnostic(
            TemporalCode::LegacyTemporalEvidence,
            None,
            "manifest requires v2 exact source evidence and segments",
        )
        .into());
    }
    manifest.source_temporal.require_processing()?;
    let mut cursor = RationalTime::ZERO;
    for (position, segment) in manifest.segments.iter().enumerate() {
        validate_segment_record(root, manifest, segment, position, cursor)?;
        cursor = segment.source_streams[0].source_time.end;
    }
    if manifest.complete
        && cursor
            != manifest
                .source_temporal
                .video_timeline()?
                .source_time
                .context("source range is missing")?
                .end
    {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "complete manifest does not cover the complete selected source video interval",
        )
        .into());
    }
    Ok(())
}

fn outcome(
    root: &Path,
    manifest: &SegmentManifest,
    resumed_segments: u64,
    generated_segments: u64,
) -> Result<SegmentOutcome> {
    Ok(SegmentOutcome {
        run_directory: root.to_owned(),
        manifest: root.join("delivery/segment-manifest.json"),
        segment_count: manifest.segments.len() as u64,
        resumed_segments,
        generated_segments,
    })
}

fn write_checkpoint(root: &Path, manifest: &mut SegmentManifest) -> Result<()> {
    manifest.updated_at = Utc::now();
    write_json_atomic(&root.join("state/checkpoint.json"), manifest)
}

fn write_process_log(path: &Path, output: &command::BoundedOutput) -> Result<()> {
    let rendered = format!(
        "status: {}\nduration_ms: {}\nstdout_truncated: {}\nstderr_truncated: {}\n\n--- stdout ---\n{}\n\n--- stderr ---\n{}",
        output.status,
        output.duration.as_millis(),
        output.stdout_truncated,
        output.stderr_truncated,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(path, rendered).with_context(|| format!("failed to write {}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("invalid JSON in {}", path.display()))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&temporary, path).with_context(|| format!("failed to publish {}", path.display()))
}

fn digest_json(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn absolute_output_directory(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn absolute_output_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn temporary_media_path(output: &Path) -> PathBuf {
    let filename = output
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("reconstructed.mp4");
    output.with_file_name(format!(".{filename}.part.mp4"))
}

fn remove_file_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_directory_if_present(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn require_current_segment_schema(path: &Path) -> Result<()> {
    let document: serde_json::Value = read_json(path)?;
    if document["schema"].as_str() != Some(SEGMENT_MANIFEST_SCHEMA_V2) {
        return Err(diagnostic(
            TemporalCode::LegacyTemporalEvidence,
            None,
            "v1 segment manifests lack exact source-stream clocks; start a v2 run",
        )
        .into());
    }
    Ok(())
}

fn segment_source_windows(
    source: &TemporalInspection,
    video_range: TimeRange,
) -> Result<Vec<SourceStreamWindow>> {
    let selection = source
        .selected
        .as_ref()
        .context("source stream selection is missing")?;
    let mut windows = vec![SourceStreamWindow {
        stream_index: selection.video.context("source video is missing")?,
        source_time: video_range,
        frame_ordinal: None,
    }];
    if let Some(audio) = selection.audio {
        let rate = source
            .stream(audio)
            .and_then(|s| s.sample_rate_hz)
            .context("audio sample clock is missing")?;
        let original = source
            .timeline(audio)
            .and_then(|t| t.source_time)
            .context("audio source range is missing")?;
        let start = RationalTime::new(
            i64::try_from(video_range.start.sample_boundary_ceil(rate)?)?,
            u64::from(rate),
        )?;
        let end = RationalTime::new(
            i64::try_from(video_range.end.sample_boundary_ceil(rate)?)?,
            u64::from(rate),
        )?
        .min(original.end);
        windows.push(SourceStreamWindow {
            stream_index: audio,
            source_time: TimeRange::new(start, end)?,
            frame_ordinal: None,
        });
    }
    Ok(windows)
}

fn validate_segment_output(
    source: &TemporalInspection,
    range: TimeRange,
    output: &TemporalInspection,
) -> Result<()> {
    source.require_processing()?;
    output.require_processing()?;
    let expected: Vec<_> = source
        .video_timeline()?
        .frames
        .iter()
        .filter(|f| f.source_time.start >= range.start && f.source_time.end <= range.end)
        .collect();
    let actual = output.video_timeline()?;
    if expected.is_empty() || expected.len() != actual.frames.len() {
        return Err(diagnostic(
            TemporalCode::FrameCountMismatch,
            None,
            "segment did not preserve its exact source frame cardinality",
        )
        .into());
    }
    if expected[0].source_time.start != range.start
        || expected
            .last()
            .context("source frame selection is empty")?
            .source_time
            .end
            != range.end
    {
        return Err(diagnostic(
            TemporalCode::TimelineMismatch,
            None,
            "segment range must follow exact source frame boundaries",
        )
        .into());
    }
    for (a, b) in expected.iter().zip(&actual.frames) {
        if a.source_time.start.checked_sub(range.start)? != b.source_time.start
            || a.source_time.end.checked_sub(range.start)? != b.source_time.end
        {
            return Err(diagnostic(
                TemporalCode::TimelineMismatch,
                Some(b.stream_index),
                "segment changed a source frame interval after explicit origin rebasing",
            )
            .into());
        }
    }
    let has_audio = source.selected.as_ref().and_then(|s| s.audio).is_some();
    if output
        .streams
        .iter()
        .filter(|s| s.kind == temporal::MediaStreamKind::Video)
        .count()
        != 1
        || output
            .streams
            .iter()
            .filter(|s| s.kind == temporal::MediaStreamKind::Audio)
            .count()
            != usize::from(has_audio)
        || output.streams.iter().any(|s| {
            !matches!(
                s.kind,
                temporal::MediaStreamKind::Video | temporal::MediaStreamKind::Audio
            )
        })
    {
        return Err(diagnostic(
            TemporalCode::InvalidSelection,
            None,
            "segment changed the selected stream cardinality/kinds",
        )
        .into());
    }
    if has_audio {
        let windows = segment_source_windows(source, range)?;
        let expected_audio = windows
            .last()
            .context("audio source window is missing")?
            .source_time
            .duration()?;
        let observed = output
            .selected
            .as_ref()
            .and_then(|s| s.audio)
            .and_then(|i| output.timeline(i))
            .and_then(|t| t.source_time)
            .context("segment audio clock is missing")?;
        let support = temporal::assess_processing(output)?;
        let tolerance = support
            .video_frame_tolerance
            .context("video tolerance is missing")?
            .checked_add(
                support
                    .audio_boundary_tolerance
                    .context("audio tolerance is missing")?,
            )?;
        if observed.start != RationalTime::ZERO
            || observed.duration()?.checked_sub(expected_audio)?.abs()? > tolerance
        {
            return Err(diagnostic(
                TemporalCode::SynchronizationMismatch,
                None,
                "segment audio exceeds its declared source/sample boundary tolerance",
            )
            .into());
        }
    }
    Ok(())
}
