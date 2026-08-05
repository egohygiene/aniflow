mod cli;
mod command;
mod media;
mod pipeline;
mod run;
mod state;
mod workspace;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn execute() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor { pipeline } => {
            let commands = if let Some(path) = pipeline {
                pipeline::Pipeline::load(&path)?.required_commands()
            } else {
                vec!["ffmpeg".to_owned(), "ffprobe".to_owned()]
            };
            command::doctor(&commands)
        }
        Commands::Inspect { input, json } => {
            let inspection = media::inspect(&input)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&inspection)?);
            } else {
                media::print_inspection(&inspection);
            }
            Ok(())
        }
        Commands::Plan { input, pipeline } => run::print_plan(&input, &pipeline),
        Commands::Run {
            input,
            pipeline,
            output_dir,
        } => run::start(&input, &pipeline, output_dir.as_deref()),
        Commands::Resume { run_directory } => run::resume(&run_directory),
        Commands::Status { run_directory } => state::print_status(&run_directory),
    }
}
