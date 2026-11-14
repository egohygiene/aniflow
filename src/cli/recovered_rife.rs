use super::*;
use aniflow::video_interpolation::recovered::{self, RecoveredRifeRequest};
#[derive(Debug, Subcommand)]
pub(super) enum RecoveredRifeCommands {
    Chunks {
        #[command(subcommand)]
        command: rife_chunks::ChunkCommands,
    },
    Plan {
        #[arg(long)]
        request: PathBuf,
    },
    Run {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        run_directory: PathBuf,
    },
    Resume {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        run_directory: PathBuf,
    },
    Status {
        #[arg(long)]
        run_directory: PathBuf,
    },
    BindAssembly {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        run_directory: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
impl RecoveredRifeCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Chunks { command } => command.name(),
            Self::Plan { .. } => CommandName::VideoRifeRecoveredPlan,
            Self::Run { .. } => CommandName::VideoRifeRecoveredRun,
            Self::Resume { .. } => CommandName::VideoRifeRecoveredResume,
            Self::Status { .. } => CommandName::VideoRifeRecoveredStatus,
            Self::BindAssembly { .. } => CommandName::VideoRifeRecoveredBindAssembly,
        }
    }
}
fn read(path: &Path) -> Result<RecoveredRifeRequest> {
    RecoveredRifeRequest::from_json_slice(&read_audio_configuration(
        path,
        "recovered RIFE request",
    )?)
}
pub(super) fn dispatch(
    command: RecoveredRifeCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    if let RecoveredRifeCommands::Chunks { command } = command {
        return rife_chunks::dispatch(command, presentation);
    }
    let name = command.name();
    let token = cli_cancellation_token()?;
    match command {
        RecoveredRifeCommands::Chunks { .. } => unreachable!(),
        RecoveredRifeCommands::Plan { request } => {
            let value = recovered::runtime::plan(&read(&request)?, &token)?;
            print_result(name, presentation, &value, || {
                print_pipeline_v3_plan(&value)
            })
        }
        RecoveredRifeCommands::Run {
            request,
            run_directory,
        }
        | RecoveredRifeCommands::Resume {
            request,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let request = read(&request)?;
            let mut recovery = Some(root.clone());
            let progress = |event: &PipelineV3RunProgress| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            };
            let result = if name == CommandName::VideoRifeRecoveredRun {
                recovered::runtime::run(request, &root, &token, progress)
            } else {
                recovered::runtime::resume(request, &root, &token, progress)
            }
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        RecoveredRifeCommands::Status { run_directory } => {
            let value = recovered::runtime::status(&absolute_delivery_path(run_directory)?)?;
            print_result(name, presentation, &value, || {
                print_pipeline_v3_status(&value)
            })
        }
        RecoveredRifeCommands::BindAssembly {
            request,
            run_directory,
            output,
        } => {
            let request = read(&request)?;
            let value = aniflow::video_assembly::rife::bind_recovered_run(
                &absolute_delivery_path(run_directory)?,
                request.configuration,
                &request.source,
                &absolute_delivery_path(output)?,
            )?;
            print_result(name, presentation, &value, || {
                println!("Current recovered RIFE assembly carrier; no creative approval inferred")
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovered_rife_requires_explicit_pinned_request_and_fresh_root_or_accepted_binding() {
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "rife",
                "recovered",
                "run",
                "--request",
                "rife.json"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "rife",
                "recovered",
                "bind-assembly",
                "--request",
                "rife.json",
                "--run-directory",
                "private/rife"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "rife",
                "recovered",
                "run",
                "--request",
                "rife.json",
                "--run-directory",
                "private/rife"
            ])
            .is_ok()
        );
    }
}
