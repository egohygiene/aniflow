use super::*;
use aniflow::music_video_recipe::recovered::with_rife::{self, WithRifeRequest};
#[derive(Debug, Subcommand)]
pub(super) enum RecoveredV2Commands {
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
        run_directory: PathBuf,
    },
    Status {
        #[arg(long)]
        run_directory: PathBuf,
    },
}
impl RecoveredV2Commands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Plan { .. } => CommandName::VideoRecoveredV2Plan,
            Self::Run { .. } => CommandName::VideoRecoveredV2Run,
            Self::Resume { .. } => CommandName::VideoRecoveredV2Resume,
            Self::Status { .. } => CommandName::VideoRecoveredV2Status,
        }
    }
}
fn read(path: &Path) -> Result<WithRifeRequest> {
    WithRifeRequest::from_json_slice(&read_bounded_configuration(
        path,
        "recovered recipe v2 request",
        4_194_304,
    )?)
}
pub(super) fn dispatch(
    command: RecoveredV2Commands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let token = cli_cancellation_token()?;
    let value = match command {
        RecoveredV2Commands::Plan { request } => {
            let value = with_rife::plan(&read(&request)?, &token)?;
            return print_result(name, presentation, &value, || {
                println!(
                    "Recovered v2 bounded intent: {}; interpolation selected: {}",
                    value.intent.intent_sha256,
                    value.intent.request.interpolation.is_some()
                )
            });
        }
        RecoveredV2Commands::Run {
            request,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            with_rife::run(read(&request)?, &root, &token, |_, e| {
                observe_pipeline_v3_progress(presentation, &mut recovery, e)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?
        }
        RecoveredV2Commands::Resume { run_directory } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            with_rife::resume(&root, &token, |_, e| {
                observe_pipeline_v3_progress(presentation, &mut recovery, e)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?
        }
        RecoveredV2Commands::Status { run_directory } => {
            with_rife::status(&absolute_delivery_path(run_directory)?, &token)?
        }
    };
    print_result(name, presentation, &value, || {
        println!(
            "Interpolation selected: {}; complete native assembly: {}; creative review remains separate",
            value.interpolation_selected, value.complete
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn v2_entry_is_explicit_and_resume_replays_only_its_immutable_intent() {
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "recovered",
                "with-rife",
                "run",
                "--request",
                "v2.json"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "recovered",
                "with-rife",
                "run",
                "--request",
                "v2.json",
                "--run-directory",
                "private/v2"
            ])
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "recovered",
                "with-rife",
                "resume",
                "--run-directory",
                "private/v2",
                "--request",
                "changed.json"
            ])
            .is_err()
        );
    }
}
