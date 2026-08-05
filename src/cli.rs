use std::path::PathBuf;

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
