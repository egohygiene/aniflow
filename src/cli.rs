use std::path::PathBuf;

use aniflow::{
    DoctorReport, MediaInspection, PipelinePlan, ProgressState, RunOperation, RunOutcome,
    RunProgress, RunRequest, RunStatus,
};
use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "aniflow",
    version,
    about = "Define the pipeline once. Transform every frame. Rebuild the experience."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
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
        /// Print machine-readable JSON.
        #[arg(long)]
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
        #[arg(long)]
        output_dir: Option<PathBuf>,
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

pub fn execute() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor { pipeline } => {
            let report = aniflow::doctor(pipeline.as_deref())?;
            print_doctor(&report);
            if report.is_ready() {
                Ok(())
            } else {
                bail!(
                    "install the missing runtime dependencies: {}",
                    report.missing_commands().collect::<Vec<_>>().join(", ")
                )
            }
        }
        Commands::Inspect { input, json } => {
            let inspection = aniflow::inspect(input)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                print_inspection(&inspection);
            }
            Ok(())
        }
        Commands::Plan { input, pipeline } => {
            let plan = aniflow::plan(input, pipeline)?;
            print_plan(&plan);
            Ok(())
        }
        Commands::Run {
            input,
            pipeline,
            output_dir,
        } => {
            let mut request = RunRequest::new(input, pipeline);
            if let Some(output_directory) = output_dir {
                request = request.with_output_directory(output_directory);
            }
            let outcome = aniflow::run_with_progress(request, print_progress)?;
            print_outcome(&outcome);
            Ok(())
        }
        Commands::Resume { run_directory } => {
            let outcome = aniflow::resume_with_progress(run_directory, print_progress)?;
            print_outcome(&outcome);
            Ok(())
        }
        Commands::Status { run_directory } => {
            let status = aniflow::status(run_directory)?;
            print_status(&status);
            Ok(())
        }
    }
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
            let state = match state {
                ProgressState::Waiting => "waiting",
                ProgressState::Cached => "cached",
                ProgressState::Running => "running",
                ProgressState::Complete => "complete",
                ProgressState::Failed => "failed",
                _ => "unknown",
            };
            println!("  {name:<24} {state}");
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
        let state = match stage.state {
            ProgressState::Waiting => "waiting",
            ProgressState::Cached => "cached",
            ProgressState::Running => "running",
            ProgressState::Complete => "complete",
            ProgressState::Failed => "failed",
            _ => "unknown",
        };
        println!("  {:<24} {state}", stage.name);
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
