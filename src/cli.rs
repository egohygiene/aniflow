use std::path::PathBuf;
use std::process::ExitCode;

use aniflow::{
    CommandName, DoctorReport, Error, ErrorCategory, MachineEnvelope, MediaInspection,
    PipelinePlan, ProgressState, Result, RunOperation, RunOutcome, RunProgress, RunRequest,
    RunStatus,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(
    name = "aniflow",
    version,
    about = "Define the pipeline once. Transform every frame. Rebuild the experience."
)]
pub struct Cli {
    /// Select human presentation or the versioned JSON contract.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    output: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    Human,
    Machine,
    LegacyInspectJson,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Verify required runtime dependencies.
    Doctor {
        /// Also verify every enabled tool required by this pipeline.
        #[arg(long)]
        pipeline: Option<PathBuf>,
    },
    /// Inspect a source video with ffprobe.
    Inspect {
        /// Source video to inspect.
        input: PathBuf,
        /// Compatibility alias for `--output json`.
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Print the stages that would execute without modifying media.
    Plan {
        /// Source video to process.
        #[arg(long)]
        input: PathBuf,
        /// Pipeline YAML file.
        #[arg(long)]
        pipeline: PathBuf,
    },
    /// Start a new isolated pipeline run.
    Run {
        /// Source video to process.
        #[arg(long)]
        input: PathBuf,
        /// Pipeline YAML file.
        #[arg(long)]
        pipeline: PathBuf,
        /// Parent directory for the new run.
        #[arg(long = "output-directory", visible_alias = "output-dir")]
        output_directory: Option<PathBuf>,
    },
    /// Continue an interrupted or failed run.
    Resume {
        /// Existing aniflow run directory.
        run_directory: PathBuf,
    },
    /// Display stage and artifact status for a run.
    Status {
        /// Existing aniflow run directory.
        run_directory: PathBuf,
    },
}

impl Commands {
    const fn name(&self) -> CommandName {
        match self {
            Self::Doctor { .. } => CommandName::Doctor,
            Self::Inspect { .. } => CommandName::Inspect,
            Self::Plan { .. } => CommandName::Plan,
            Self::Run { .. } => CommandName::Run,
            Self::Resume { .. } => CommandName::Resume,
            Self::Status { .. } => CommandName::Status,
        }
    }

    const fn requests_legacy_json(&self) -> bool {
        matches!(self, Self::Inspect { json: true, .. })
    }
}

pub fn execute() -> ExitCode {
    let cli = Cli::parse();
    let command = cli.command.name();
    let presentation = match (cli.output, cli.command.requests_legacy_json()) {
        (OutputFormat::Json, _) => Presentation::Machine,
        (OutputFormat::Human, true) => Presentation::LegacyInspectJson,
        (OutputFormat::Human, false) => Presentation::Human,
    };

    match dispatch(cli.command, presentation) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Err(render_error) = print_error(command, presentation, &error) {
                eprintln!("error: {render_error}");
                return ExitCode::from(ErrorCategory::Internal.exit_code());
            }
            ExitCode::from(error.category().exit_code())
        }
    }
}

fn dispatch(command: Commands, presentation: Presentation) -> Result<()> {
    match command {
        Commands::Doctor { pipeline } => {
            let report = aniflow::doctor(pipeline.as_deref())?;
            if !report.is_ready() {
                if presentation == Presentation::Human {
                    print_doctor(&report);
                }
                return Err(Error::new(
                    ErrorCategory::Dependency,
                    format!(
                        "install the missing runtime dependencies: {}",
                        report.missing_commands().collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
            print_result(CommandName::Doctor, presentation, &report, || {
                print_doctor(&report);
            })
        }
        Commands::Inspect { input, .. } => {
            let inspection = aniflow::inspect(input)?;
            print_result(CommandName::Inspect, presentation, &inspection, || {
                print_inspection(&inspection);
            })
        }
        Commands::Plan { input, pipeline } => {
            let plan = aniflow::plan(input, pipeline)?;
            print_result(CommandName::Plan, presentation, &plan, || print_plan(&plan))
        }
        Commands::Run {
            input,
            pipeline,
            output_directory,
        } => {
            let mut request = RunRequest::new(input, pipeline);
            if let Some(output_directory) = output_directory {
                request = request.with_output_directory(output_directory);
            }
            let outcome = if presentation == Presentation::Human {
                aniflow::run_with_progress(request, print_progress)?
            } else {
                aniflow::run(request)?
            };
            print_result(CommandName::Run, presentation, &outcome, || {
                print_outcome(&outcome);
            })
        }
        Commands::Resume { run_directory } => {
            let outcome = if presentation == Presentation::Human {
                aniflow::resume_with_progress(run_directory, print_progress)?
            } else {
                aniflow::resume(run_directory)?
            };
            print_result(CommandName::Resume, presentation, &outcome, || {
                print_outcome(&outcome);
            })
        }
        Commands::Status { run_directory } => {
            let status = aniflow::status(run_directory)?;
            print_result(CommandName::Status, presentation, &status, || {
                print_status(&status);
            })
        }
    }
}

fn print_result<T, F>(
    command: CommandName,
    presentation: Presentation,
    result: &T,
    print_human: F,
) -> Result<()>
where
    T: Clone + Serialize,
    F: FnOnce(),
{
    match presentation {
        Presentation::Human => {
            print_human();
            Ok(())
        }
        Presentation::Machine => print_json(&MachineEnvelope::success(command, result.clone())),
        Presentation::LegacyInspectJson => print_json(result),
    }
}

fn print_error(command: CommandName, presentation: Presentation, error: &Error) -> Result<()> {
    match presentation {
        Presentation::Human | Presentation::LegacyInspectJson => {
            eprintln!("error: {error}");
            Ok(())
        }
        Presentation::Machine => {
            let envelope = MachineEnvelope::<Value>::failure(command, error, None);
            let rendered = render_json(&envelope)?;
            eprintln!("{rendered}");
            Ok(())
        }
    }
}

fn print_json<T>(value: &T) -> Result<()>
where
    T: Serialize,
{
    println!("{}", render_json(value)?);
    Ok(())
}

fn render_json<T>(value: &T) -> Result<String>
where
    T: Serialize,
{
    serde_json::to_string_pretty(value).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("failed to serialize machine output: {error}"),
        )
    })
}

fn print_doctor(report: &DoctorReport) {
    println!("aniflow doctor");
    println!();
    for dependency in &report.dependencies {
        if let Some(summary) = &dependency.summary {
            println!("  {:<24} ok  {summary}", dependency.command);
        } else {
            println!("  {:<24} missing", dependency.command);
        }
    }
    if report.is_ready() {
        println!();
        println!("ready");
    }
}

fn print_inspection(inspection: &MediaInspection) {
    println!("aniflow inspect");
    println!();
    println!("  source       {}", inspection.source);
    println!("  duration     {:.3} seconds", inspection.duration_seconds);
    println!("  dimensions   {}x{}", inspection.width, inspection.height);
    println!(
        "  frame rate   {} ({:.6} fps)",
        inspection.average_frame_rate, inspection.frames_per_second
    );
    println!("  frames       ~{}", inspection.estimated_frame_count);
    println!("  video        {}", inspection.video_codec);
    println!(
        "  audio        {}",
        inspection.audio_codec.as_deref().unwrap_or("none")
    );
    println!(
        "  subtitles    {}",
        if inspection.has_subtitles {
            "yes"
        } else {
            "no"
        }
    );
}

fn print_plan(plan: &PipelinePlan) {
    println!("aniflow plan");
    println!();
    println!("source");
    println!("  path         {}", plan.inspection.source);
    println!(
        "  media        {}x{} @ {:.6} fps",
        plan.inspection.width, plan.inspection.height, plan.inspection.frames_per_second
    );
    println!("  frames       ~{}", plan.inspection.estimated_frame_count);
    println!();
    println!("pipeline");
    println!("  name         {}", plan.pipeline_name);
    println!("  version      {}", plan.pipeline_version);
    println!("  output       {}", plan.output_file.display());
    println!();
    println!("stages");
    for (index, stage) in plan.stages.iter().enumerate() {
        println!("  {:>2}. {stage}", index + 1);
    }
    if !plan.frame_processors.is_empty() {
        println!();
        println!("frame processors");
        for processor in &plan.frame_processors {
            println!(
                "  {:<20} {:<16} {}",
                processor.id, processor.command, processor.execution
            );
        }
    }
    println!();
    println!("required commands");
    for command in &plan.required_commands {
        println!("  {command}");
    }
}

fn print_progress(progress: &RunProgress) {
    match progress {
        RunProgress::Started {
            operation,
            run_directory,
            pipeline_name,
        } => {
            let command = match operation {
                RunOperation::Run => "run",
                RunOperation::Resume => "resume",
                _ => "run",
            };
            println!("aniflow {command}");
            println!();
            println!("  workspace    {}", run_directory.display());
            println!("  pipeline     {pipeline_name}");
            println!();
        }
        RunProgress::Stage { name, state } => {
            println!("  {name:<24} {}", progress_state(*state));
        }
        _ => {}
    }
}

fn print_outcome(outcome: &RunOutcome) {
    println!();
    println!("complete");
    println!("  output       {}", outcome.output.display());
    println!("  delivery     {}", outcome.delivery_manifest.display());
    println!("  manifest     {}", outcome.run_manifest.display());
}

fn print_status(status: &RunStatus) {
    println!("aniflow status");
    println!();
    println!("  run        {}", status.run_id);
    println!("  pipeline   {}", status.pipeline_name);
    println!("  source     {}", status.source_file.display());
    println!();
    println!("stages");
    for stage in &status.stages {
        println!("  {:<24} {}", stage.name, progress_state(stage.state));
        if let Some(message) = &stage.message {
            println!("    {message}");
        }
    }
    println!();
    println!("artifacts");
    for artifact in &status.artifacts {
        println!("  {:<24} {}", artifact.name, artifact.path.display());
    }
}

const fn progress_state(state: ProgressState) -> &'static str {
    match state {
        ProgressState::Waiting => "waiting",
        ProgressState::Cached => "cached",
        ProgressState::Running => "running",
        ProgressState::Complete => "complete",
        ProgressState::Failed => "failed",
        _ => "unknown",
    }
}
