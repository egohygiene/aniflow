use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::command;
use crate::error::{Error, ErrorCategory, Result};
use crate::media::{self, MediaInspection};
use crate::pipeline::Pipeline;
use crate::pipeline_v3::PIPELINE_V3_SCHEMA;
use crate::segmentation::{
    self, CancellationToken, ReconstructionReport, SegmentOutcome, SegmentPlan, SegmentProgress,
    SegmentRequest,
};
use crate::{run as execution, state};

/// The result of checking one external runtime dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DependencyStatus {
    pub command: String,
    pub summary: Option<String>,
}

impl DependencyStatus {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.summary.is_some()
    }
}

/// Runtime dependency diagnostics for an aniflow installation or pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DoctorReport {
    pub dependencies: Vec<DependencyStatus>,
}

impl DoctorReport {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.dependencies.iter().all(DependencyStatus::is_available)
    }

    pub fn missing_commands(&self) -> impl Iterator<Item = &str> {
        self.dependencies
            .iter()
            .filter(|dependency| !dependency.is_available())
            .map(|dependency| dependency.command.as_str())
    }
}

/// One enabled frame processor in a resolved pipeline plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FrameProcessorPlan {
    pub id: String,
    pub command: String,
    pub execution: String,
}

/// A behavior-preserving view of the work aniflow will perform.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PipelinePlan {
    pub inspection: MediaInspection,
    pub pipeline_name: String,
    pub pipeline_version: u32,
    pub output_file: PathBuf,
    pub stages: Vec<String>,
    pub frame_processors: Vec<FrameProcessorPlan>,
    pub required_commands: Vec<String>,
}

/// Inputs for starting a new isolated run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunRequest {
    pub input: PathBuf,
    pub pipeline: PathBuf,
    pub output_directory: Option<PathBuf>,
}

impl RunRequest {
    #[must_use]
    pub fn new(input: impl Into<PathBuf>, pipeline: impl Into<PathBuf>) -> Self {
        Self {
            input: input.into(),
            pipeline: pipeline.into(),
            output_directory: None,
        }
    }

    #[must_use]
    pub fn with_output_directory(mut self, output_directory: impl Into<PathBuf>) -> Self {
        self.output_directory = Some(output_directory.into());
        self
    }
}

/// The durable paths produced or reused by a completed run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunOutcome {
    pub run_directory: PathBuf,
    pub output: PathBuf,
    pub delivery_manifest: PathBuf,
    pub run_manifest: PathBuf,
}

/// Whether progress belongs to a new run or a resumed run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunOperation {
    Run,
    Resume,
}

/// The current lifecycle observation for one pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProgressState {
    Waiting,
    Cached,
    Running,
    Complete,
    Failed,
}

/// A provisional progress observation emitted during execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunProgress {
    Started {
        operation: RunOperation,
        run_directory: PathBuf,
        pipeline_name: String,
    },
    Stage {
        name: String,
        state: ProgressState,
    },
}

/// A stage and its persisted status in a run manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StageStatus {
    pub name: String,
    pub state: ProgressState,
    pub message: Option<String>,
}

/// A named artifact recorded by a run manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ArtifactStatus {
    pub name: String,
    pub path: PathBuf,
    pub sha256: Option<String>,
}

/// A read-only application view of an existing run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunStatus {
    pub run_id: String,
    pub pipeline_name: String,
    pub source_file: PathBuf,
    pub stages: Vec<StageStatus>,
    pub artifacts: Vec<ArtifactStatus>,
}

/// Check aniflow's default dependencies or all dependencies for a pipeline.
pub fn doctor(pipeline_path: Option<&Path>) -> Result<DoctorReport> {
    let commands = if let Some(path) = pipeline_path {
        require_file(path, ErrorCategory::Configuration, "pipeline")?;
        Pipeline::load(path)
            .map_err(|error| Error::from_anyhow(ErrorCategory::Configuration, error))?
            .required_commands()
    } else {
        vec!["ffmpeg".to_owned(), "ffprobe".to_owned()]
    };

    let dependencies = commands
        .into_iter()
        .map(|executable| DependencyStatus {
            summary: command::executable_summary(&executable).ok(),
            command: executable,
        })
        .collect();

    Ok(DoctorReport { dependencies })
}

/// Inspect a source video without involving the CLI parser.
pub fn inspect(input: impl AsRef<Path>) -> Result<MediaInspection> {
    let input = input.as_ref();
    require_file(input, ErrorCategory::Input, "input video")?;
    command::require_executable("ffprobe")
        .map_err(|error| Error::from_anyhow(ErrorCategory::Dependency, error))?;
    media::inspect(input).map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))
}

/// Resolve and inspect a pipeline without modifying media.
pub fn plan(input: impl AsRef<Path>, pipeline_path: impl AsRef<Path>) -> Result<PipelinePlan> {
    let input = input.as_ref();
    let pipeline_path = pipeline_path.as_ref();
    require_file(input, ErrorCategory::Input, "input video")?;
    require_file(pipeline_path, ErrorCategory::Configuration, "pipeline")?;
    command::require_executable("ffprobe")
        .map_err(|error| Error::from_anyhow(ErrorCategory::Dependency, error))?;
    let inspection =
        media::inspect(input).map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
    let pipeline = Pipeline::load(pipeline_path)
        .map_err(|error| Error::from_anyhow(ErrorCategory::Configuration, error))?;
    let frame_processors = pipeline
        .enabled_frame_processors()
        .map(|processor| FrameProcessorPlan {
            id: processor.id().to_owned(),
            command: processor.command().to_owned(),
            execution: processor
                .concurrency()
                .map(|value| format!("per-frame concurrency={value}"))
                .unwrap_or_else(|| "native batch".to_owned()),
        })
        .collect();

    Ok(PipelinePlan {
        stages: pipeline.stage_names(inspection.has_audio),
        required_commands: pipeline.required_commands(),
        pipeline_name: pipeline.name,
        pipeline_version: pipeline.version,
        output_file: pipeline.output.file,
        frame_processors,
        inspection,
    })
}

/// Start a run without progress presentation.
pub fn run(request: RunRequest) -> Result<RunOutcome> {
    run_with_progress(request, |_| {})
}

/// Start a run and receive provisional lifecycle observations.
pub fn run_with_progress<F>(request: RunRequest, mut progress: F) -> Result<RunOutcome>
where
    F: FnMut(&RunProgress),
{
    run_with_progress_and_cancellation(request, &CancellationToken::default(), &mut progress)
}

/// Start a run with lifecycle observations and cooperative processor cancellation.
pub fn run_with_progress_and_cancellation<F>(
    request: RunRequest,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<RunOutcome>
where
    F: FnMut(&RunProgress),
{
    require_file(&request.input, ErrorCategory::Input, "input video")?;
    require_file(&request.pipeline, ErrorCategory::Configuration, "pipeline")?;
    reject_pipeline_v3_execution(&request.pipeline)?;
    execution::start(
        &request.input,
        &request.pipeline,
        request.output_directory.as_deref(),
        cancellation,
        &mut progress,
    )
    .map_err(|error| Error::from_anyhow(ErrorCategory::Execution, error))
}

/// Resume an existing run without progress presentation.
pub fn resume(run_directory: impl AsRef<Path>) -> Result<RunOutcome> {
    resume_with_progress(run_directory, |_| {})
}

/// Resume an existing run and receive provisional lifecycle observations.
pub fn resume_with_progress<F>(
    run_directory: impl AsRef<Path>,
    mut progress: F,
) -> Result<RunOutcome>
where
    F: FnMut(&RunProgress),
{
    resume_with_progress_and_cancellation(
        run_directory,
        &CancellationToken::default(),
        &mut progress,
    )
}

/// Resume a run with lifecycle observations and cooperative processor cancellation.
pub fn resume_with_progress_and_cancellation<F>(
    run_directory: impl AsRef<Path>,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<RunOutcome>
where
    F: FnMut(&RunProgress),
{
    let run_directory = run_directory.as_ref();
    require_directory(run_directory, ErrorCategory::State, "run directory")?;
    reject_pipeline_v3_execution(&run_directory.join("config/pipeline.yml"))?;
    execution::resume(run_directory, cancellation, &mut progress)
        .map_err(|error| Error::from_anyhow(ErrorCategory::Execution, error))
}

/// Query the persisted status of an existing run.
pub fn status(run_directory: impl AsRef<Path>) -> Result<RunStatus> {
    let run_directory = run_directory.as_ref();
    require_directory(run_directory, ErrorCategory::State, "run directory")?;
    state::status(run_directory).map_err(|error| Error::from_anyhow(ErrorCategory::State, error))
}

/// Inspect and normalize a short-segment request without creating artifacts.
pub fn plan_segments(request: &SegmentRequest) -> Result<SegmentPlan> {
    validate_segment_request(request)?;
    segmentation::plan(request).map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))
}

/// Start a short-segment run without progress presentation or external cancellation.
pub fn segment(request: SegmentRequest) -> Result<SegmentOutcome> {
    segment_with_progress(request, &CancellationToken::default(), |_| {})
}

/// Start a short-segment run with progress and cooperative child-process cancellation.
pub fn segment_with_progress<F>(
    request: SegmentRequest,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<SegmentOutcome>
where
    F: FnMut(&SegmentProgress),
{
    validate_segment_request(&request)?;
    segmentation::execute(&request, cancellation, &mut progress)
        .map_err(|error| Error::from_anyhow(ErrorCategory::Execution, error))
}

/// Resume a compatible interrupted short-segment run.
pub fn resume_segments(run_directory: impl AsRef<Path>) -> Result<SegmentOutcome> {
    resume_segments_with_progress(run_directory, &CancellationToken::default(), |_| {})
}

/// Resume segmentation with progress and cooperative cancellation.
pub fn resume_segments_with_progress<F>(
    run_directory: impl AsRef<Path>,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<SegmentOutcome>
where
    F: FnMut(&SegmentProgress),
{
    let run_directory = run_directory.as_ref();
    require_directory(run_directory, ErrorCategory::State, "segment run directory")?;
    require_media_dependencies()?;
    segmentation::resume(run_directory, cancellation, &mut progress)
        .map_err(|error| Error::from_anyhow(ErrorCategory::Execution, error))
}

/// Reconstruct and validate a complete segment manifest.
pub fn reconstruct_segments(
    run_directory: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<ReconstructionReport> {
    reconstruct_segments_with_progress(run_directory, output, &CancellationToken::default(), |_| {})
}

/// Reconstruct segments with progress and cooperative cancellation.
pub fn reconstruct_segments_with_progress<F>(
    run_directory: impl AsRef<Path>,
    output: impl AsRef<Path>,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<ReconstructionReport>
where
    F: FnMut(&SegmentProgress),
{
    let run_directory = run_directory.as_ref();
    require_directory(run_directory, ErrorCategory::State, "segment run directory")?;
    require_media_dependencies()?;
    segmentation::reconstruct(run_directory, output.as_ref(), cancellation, &mut progress)
        .map_err(|error| Error::from_anyhow(ErrorCategory::Execution, error))
}

fn validate_segment_request(request: &SegmentRequest) -> Result<()> {
    require_file(&request.input, ErrorCategory::Input, "input video")?;
    segmentation::validate_request(request)
        .map_err(|error| Error::from_anyhow(ErrorCategory::Configuration, error))?;
    require_media_dependencies()
}

fn require_media_dependencies() -> Result<()> {
    for executable in ["ffmpeg", "ffprobe"] {
        command::require_executable(executable)
            .map_err(|error| Error::from_anyhow(ErrorCategory::Dependency, error))?;
    }
    Ok(())
}

fn reject_pipeline_v3_execution(pipeline_path: &Path) -> Result<()> {
    let Ok(contents) = fs::read(pipeline_path) else {
        return Ok(());
    };
    let Ok(document) = serde_yaml::from_slice::<serde_yaml::Value>(&contents) else {
        return Ok(());
    };
    let schema = document
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("schema".to_owned())))
        .and_then(serde_yaml::Value::as_str);
    if schema == Some(PIPELINE_V3_SCHEMA) {
        return Err(Error::new(
            ErrorCategory::Configuration,
            "Pipeline v3 execution and resume are not available yet; use plan-v3 for read-only resolution",
        ));
    }
    Ok(())
}

fn require_file(path: &Path, category: ErrorCategory, name: &str) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(Error::new(
            category,
            format!("{name} does not exist: {}", path.display()),
        ))
    }
}

fn require_directory(path: &Path, category: ErrorCategory, name: &str) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(Error::new(
            category,
            format!("{name} does not exist: {}", path.display()),
        ))
    }
}
