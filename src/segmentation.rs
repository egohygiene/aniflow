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

pub const SEGMENT_PLAN_SCHEMA_V1: &str = "aniflow.segment-plan/v1";
pub const SEGMENT_MANIFEST_SCHEMA_V1: &str = "aniflow.segment-manifest/v1";
pub const RECONSTRUCTION_REPORT_SCHEMA_V1: &str = "aniflow.reconstruction-report/v1";
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
        }
    }

    #[must_use]
    pub const fn with_process_timeout_seconds(mut self, seconds: u64) -> Self {
        self.process_timeout_seconds = seconds;
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
    let source_media = media::inspect(&source)?;
    if request.mode == SegmentMode::TranscodeH264Aac && source_media.has_subtitles {
        bail!(
            "the h264/aac transcode preset does not silently discard subtitle streams; use stream-copy or provide a subtitle-free source"
        );
    }
    let source_size_bytes = fs::metadata(&source)?.len();
    let source_duration_ms = seconds_to_milliseconds(source_media.duration_seconds);
    if source_duration_ms == 0 {
        bail!("source duration must be greater than zero");
    }
    let (boundary_accuracy, stream_policy, multiplier, segments) = match request.mode {
        SegmentMode::StreamCopy => (
            BoundaryAccuracy::KeyframeAligned,
            "copy all streams; segment boundaries follow source keyframes".to_owned(),
            1.10,
            plan_keyframe_segments(&source, source_duration_ms, request.segment_duration_ms)?,
        ),
        SegmentMode::TranscodeH264Aac => (
            BoundaryAccuracy::FrameAccurate,
            "select the first video stream and optional first audio stream; encode H.264/AAC"
                .to_owned(),
            2.0,
            plan_exact_segments(source_duration_ms, request.segment_duration_ms),
        ),
    };
    let expected_segments = segments.len() as u64;
    Ok(SegmentPlan {
        schema: SEGMENT_PLAN_SCHEMA_V1.to_owned(),
        capability_id: SEGMENT_CAPABILITY_ID_V1.to_owned(),
        source_sha256: sha256_file(&source)?,
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
        schema: SEGMENT_MANIFEST_SCHEMA_V1.to_owned(),
        capability_id: SEGMENT_CAPABILITY_ID_V1.to_owned(),
        aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: now,
        updated_at: now,
        complete: false,
        plan_sha256: digest_json(&plan)?,
        source: plan.source.clone(),
        source_sha256: plan.source_sha256.clone(),
        source_duration_ms: seconds_to_milliseconds(plan.source_media.duration_seconds),
        segment_duration_ms: plan.segment_duration_ms,
        mode: plan.mode,
        boundary_accuracy: plan.boundary_accuracy,
        ffmpeg_version: command::executable_summary("ffmpeg")?,
        ffprobe_version: command::executable_summary("ffprobe")?,
        segments: Vec::new(),
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
    let request: SegmentRequest = read_json(&run_directory.join("state/request.json"))?;
    let plan: SegmentPlan = read_json(&run_directory.join("state/plan.json"))?;
    let mut manifest: SegmentManifest = read_json(&run_directory.join("state/checkpoint.json"))?;
    let canonical_run = run_directory
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", run_directory.display()))?;
    if canonical_run != plan.output_directory {
        bail!("segment run directory moved; refusing to resume ambiguous path state");
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
    let manifest: SegmentManifest = read_json(&manifest_path)?;
    if !manifest.complete {
        bail!("only a complete segment manifest can be reconstructed");
    }
    verify_manifest_segments(run_directory, &manifest)?;
    progress(&SegmentProgress::ReconstructionStarted);
    let concat_path = run_directory.join("concat.txt");
    let concat = manifest
        .segments
        .iter()
        .map(|segment| {
            let name = segment
                .path
                .file_name()
                .and_then(|value| value.to_str())
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
    let arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-nostdin"),
        OsString::from("-y"),
        OsString::from("-f"),
        OsString::from("concat"),
        OsString::from("-safe"),
        OsString::from("1"),
        OsString::from("-i"),
        concat_path.as_os_str().to_owned(),
        OsString::from("-map"),
        OsString::from("0"),
        OsString::from("-c"),
        OsString::from("copy"),
        OsString::from("-movflags"),
        OsString::from("+faststart"),
        temporary.as_os_str().to_owned(),
    ];
    let bounded = command::run_bounded(
        "ffmpeg",
        arguments,
        ProcessLimits {
            timeout: Duration::from_secs(DEFAULT_PROCESS_TIMEOUT_SECONDS),
            maximum_output_bytes: MAXIMUM_PROCESS_OUTPUT_BYTES,
        },
        Some(cancellation.shared()),
    );
    let bounded = match bounded {
        Ok(output) => output,
        Err(error) => {
            remove_file_if_present(&concat_path)?;
            remove_file_if_present(&temporary)?;
            return Err(error);
        }
    };
    write_process_log(&run_directory.join("logs/reconstruct.log"), &bounded)?;
    remove_file_if_present(&concat_path)?;
    if !bounded.status.success() {
        remove_file_if_present(&temporary)?;
        bail!("ffmpeg reconstruction failed; see logs/reconstruct.log");
    }
    if bounded.stdout_truncated || bounded.stderr_truncated {
        remove_file_if_present(&temporary)?;
        bail!("ffmpeg reconstruction diagnostics exceeded the 4 MiB limit");
    }
    fs::rename(&temporary, &output)?;
    let inspection = media::inspect(&output)?;
    let output_duration_ms = seconds_to_milliseconds(inspection.duration_seconds);
    let duration_delta_ms = output_duration_ms.abs_diff(manifest.source_duration_ms);
    let frame_tolerance_ms = (1_000.0 / inspection.frames_per_second).ceil() as u64;
    let tolerance_ms = match manifest.boundary_accuracy {
        BoundaryAccuracy::FrameAccurate => frame_tolerance_ms.max(50),
        BoundaryAccuracy::KeyframeAligned => manifest.segment_duration_ms + frame_tolerance_ms,
    };
    let report = ReconstructionReport {
        schema: RECONSTRUCTION_REPORT_SCHEMA_V1.to_owned(),
        capability_id: RECONSTRUCT_CAPABILITY_ID_V1.to_owned(),
        aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        manifest_sha256: sha256_file(&manifest_path)?,
        manifest: manifest_path,
        output_sha256: sha256_file(&output)?,
        output,
        output_duration_ms,
        source_duration_ms: manifest.source_duration_ms,
        duration_delta_ms,
        tolerance_ms,
        validated: duration_delta_ms <= tolerance_ms,
    };
    write_json_atomic(
        &run_directory.join("delivery/reconstruction-report.json"),
        &report,
    )?;
    if !report.validated {
        bail!(
            "reconstruction duration differs from the source by {} ms (tolerance {} ms)",
            report.duration_delta_ms,
            report.tolerance_ms
        );
    }
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
        let arguments = segment_arguments(
            plan,
            requested_start_ms,
            requested_end_ms - requested_start_ms,
            &temporary,
        );
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
        fs::rename(&temporary, &destination)?;
        let inspection = media::inspect(&destination)?;
        manifest.segments.push(SegmentRecord {
            index,
            path: PathBuf::from("segments").join(filename),
            requested_start_ms,
            requested_end_ms,
            actual_duration_ms: seconds_to_milliseconds(inspection.duration_seconds),
            boundary_accuracy: plan.boundary_accuracy,
            sha256: sha256_file(&destination)?,
            size_bytes: fs::metadata(&destination)?.len(),
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
        .map(|segment| format_seconds(segment.end_ms))
        .collect::<Vec<_>>()
        .join(",");
    let pattern = staging.join("segment-%06d.mp4");
    let arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-nostdin"),
        OsString::from("-y"),
        OsString::from("-i"),
        plan.source.as_os_str().to_owned(),
        OsString::from("-map"),
        OsString::from("0"),
        OsString::from("-c"),
        OsString::from("copy"),
        OsString::from("-f"),
        OsString::from("segment"),
        OsString::from("-segment_times"),
        OsString::from(split_times),
        OsString::from("-reset_timestamps"),
        OsString::from("1"),
        OsString::from("-segment_start_number"),
        OsString::from("0"),
        pattern.as_os_str().to_owned(),
    ];
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
        fs::rename(&staged, &destination)?;
        let inspection = media::inspect(&destination)?;
        manifest.segments.push(SegmentRecord {
            index,
            path: PathBuf::from("segments").join(filename),
            requested_start_ms: planned.start_ms,
            requested_end_ms: planned.end_ms,
            actual_duration_ms: seconds_to_milliseconds(inspection.duration_seconds),
            boundary_accuracy: BoundaryAccuracy::KeyframeAligned,
            sha256: sha256_file(&destination)?,
            size_bytes: fs::metadata(&destination)?.len(),
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
    verify_manifest_segments(&plan.output_directory, manifest)?;
    manifest.complete = true;
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
    start_ms: u64,
    duration_ms: u64,
    output: &Path,
) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-nostdin"),
        OsString::from("-y"),
        OsString::from("-ss"),
        OsString::from(format_seconds(start_ms)),
        OsString::from("-i"),
        plan.source.as_os_str().to_owned(),
        OsString::from("-t"),
        OsString::from(format_seconds(duration_ms)),
    ];
    match plan.mode {
        SegmentMode::StreamCopy => arguments.extend([
            OsString::from("-map"),
            OsString::from("0"),
            OsString::from("-c"),
            OsString::from("copy"),
            OsString::from("-avoid_negative_ts"),
            OsString::from("make_zero"),
        ]),
        SegmentMode::TranscodeH264Aac => arguments.extend([
            OsString::from("-map"),
            OsString::from("0:v:0"),
            OsString::from("-map"),
            OsString::from("0:a:0?"),
            OsString::from("-c:v"),
            OsString::from("libx264"),
            OsString::from("-preset"),
            OsString::from("medium"),
            OsString::from("-crf"),
            OsString::from("18"),
            OsString::from("-c:a"),
            OsString::from("aac"),
            OsString::from("-b:a"),
            OsString::from("192k"),
            OsString::from("-force_key_frames"),
            OsString::from("expr:gte(t,0)"),
        ]),
    }
    arguments.extend([
        OsString::from("-movflags"),
        OsString::from("+faststart"),
        output.as_os_str().to_owned(),
    ]);
    arguments
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

fn plan_exact_segments(source_duration_ms: u64, target_ms: u64) -> Vec<PlannedSegment> {
    let count = source_duration_ms.div_ceil(target_ms);
    (0..count)
        .map(|index| PlannedSegment {
            index,
            start_ms: index * target_ms,
            end_ms: ((index + 1) * target_ms).min(source_duration_ms),
        })
        .collect()
}

fn plan_keyframe_segments(
    source: &Path,
    source_duration_ms: u64,
    target_ms: u64,
) -> Result<Vec<PlannedSegment>> {
    let arguments = vec![
        OsString::from("-v"),
        OsString::from("error"),
        OsString::from("-select_streams"),
        OsString::from("v:0"),
        OsString::from("-skip_frame"),
        OsString::from("nokey"),
        OsString::from("-show_entries"),
        OsString::from("frame=best_effort_timestamp_time"),
        OsString::from("-of"),
        OsString::from("csv=p=0"),
        source.as_os_str().to_owned(),
    ];
    let output = command::run_bounded(
        "ffprobe",
        arguments,
        ProcessLimits {
            timeout: Duration::from_secs(30),
            maximum_output_bytes: MAXIMUM_PROCESS_OUTPUT_BYTES,
        },
        None,
    )?;
    if !output.status.success() {
        bail!("ffprobe failed while inspecting source keyframes");
    }
    if output.stdout_truncated || output.stderr_truncated {
        bail!("ffprobe keyframe evidence exceeded the 4 MiB limit");
    }
    let mut keyframes = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().trim_end_matches(',').parse::<f64>().ok())
        .map(seconds_to_milliseconds)
        .filter(|timestamp| *timestamp < source_duration_ms)
        .collect::<Vec<_>>();
    keyframes.push(0);
    keyframes.sort_unstable();
    keyframes.dedup();

    let mut cuts = vec![0_u64];
    let mut desired = target_ms;
    while desired < source_duration_ms {
        let last = *cuts.last().unwrap_or(&0);
        let Some(next) = keyframes
            .iter()
            .copied()
            .find(|timestamp| *timestamp >= desired && *timestamp > last)
        else {
            break;
        };
        cuts.push(next);
        desired = next.saturating_add(target_ms);
    }
    cuts.push(source_duration_ms);
    cuts.dedup();
    let segments = cuts
        .windows(2)
        .enumerate()
        .map(|(index, range)| PlannedSegment {
            index: index as u64,
            start_ms: range[0],
            end_ms: range[1],
        })
        .collect::<Vec<_>>();
    if let Some(segment) = segments
        .iter()
        .find(|segment| segment.end_ms - segment.start_ms >= 30_000)
    {
        bail!(
            "stream-copy keyframes would create a {} ms segment at index {}; use the frame-accurate transcode mode",
            segment.end_ms - segment.start_ms,
            segment.index
        );
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
    for (position, segment) in manifest.segments.iter().enumerate() {
        let path = root.join(&segment.path);
        if segment.index != position as u64
            || !path.is_file()
            || sha256_file(&path)? != segment.sha256
        {
            break;
        }
        valid += 1;
    }
    manifest.segments.truncate(valid);
    write_checkpoint(root, manifest)?;
    Ok(valid as u64)
}

fn verify_manifest_segments(root: &Path, manifest: &SegmentManifest) -> Result<()> {
    if manifest.segments.is_empty() {
        bail!("segment manifest contains no segments");
    }
    for (position, segment) in manifest.segments.iter().enumerate() {
        let expected_path = PathBuf::from("segments").join(format!("segment-{position:06}.mp4"));
        if segment.index != position as u64 {
            bail!("segment manifest ordering is not contiguous at index {position}");
        }
        if segment.path != expected_path {
            bail!("segment manifest contains an unsafe or unexpected path at index {position}");
        }
        let path = root.join(&segment.path);
        if !path.is_file() || sha256_file(&path)? != segment.sha256 {
            bail!(
                "segment {} is missing or has a checksum mismatch",
                segment.index
            );
        }
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

fn seconds_to_milliseconds(seconds: f64) -> u64 {
    (seconds * 1_000.0).round() as u64
}

fn format_seconds(milliseconds: u64) -> String {
    format!("{}.{:03}", milliseconds / 1_000, milliseconds % 1_000)
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
