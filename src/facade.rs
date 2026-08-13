use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::command;
use crate::media::{self, MediaInspection};
use crate::pipeline::Pipeline;
use crate::{run as execution, state};

/// The result of checking one external runtime dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Runtime dependency diagnostics for an Aniflow installation or pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FrameProcessorPlan {
    pub id: String,
    pub command: String,
    pub execution: String,
}

/// A behavior-preserving view of the work Aniflow will perform.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RunOutcome {
    pub run_directory: PathBuf,
    pub output: PathBuf,
    pub delivery_manifest: PathBuf,
    pub run_manifest: PathBuf,
}

/// Whether progress belongs to a new run or a resumed run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunOperation {
    Run,
    Resume,
}

/// The current lifecycle observation for one pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProgressState {
    Waiting,
    Cached,
    Running,
    Complete,
    Failed,
}

/// A provisional progress observation emitted during execution.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StageStatus {
    pub name: String,
    pub state: ProgressState,
    pub message: Option<String>,
}

/// A named artifact recorded by a run manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArtifactStatus {
    pub name: String,
    pub path: PathBuf,
    pub sha256: Option<String>,
}

/// A read-only application view of an existing run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RunStatus {
    pub run_id: String,
    pub pipeline_name: String,
    pub source_file: PathBuf,
    pub stages: Vec<StageStatus>,
    pub artifacts: Vec<ArtifactStatus>,
}

/// Check Aniflow's default dependencies or all dependencies for a pipeline.
pub fn doctor(pipeline_path: Option<&Path>) -> Result<DoctorReport> {
    let commands = if let Some(path) = pipeline_path {
        Pipeline::load(path)?.required_commands()
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
    media::inspect(input.as_ref())
}

/// Resolve and inspect a pipeline without modifying media.
pub fn plan(input: impl AsRef<Path>, pipeline_path: impl AsRef<Path>) -> Result<PipelinePlan> {
    command::require_executable("ffprobe")?;
    let inspection = media::inspect(input.as_ref())?;
    let pipeline = Pipeline::load(pipeline_path.as_ref())?;
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
    execution::start(
        &request.input,
        &request.pipeline,
        request.output_directory.as_deref(),
        &mut progress,
    )
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
    execution::resume(run_directory.as_ref(), &mut progress)
}

/// Query the persisted status of an existing run.
pub fn status(run_directory: impl AsRef<Path>) -> Result<RunStatus> {
    state::status(run_directory.as_ref())
}
