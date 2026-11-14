use super::*;
use aniflow::deflicker_preview::{self as comparison, Request};
#[derive(Debug, Subcommand)]
pub(super) enum DeflickerCommands {
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
    RecordReview {
        #[arg(long)]
        run_directory: PathBuf,
        #[arg(long)]
        declaration: PathBuf,
    },
}
impl DeflickerCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Plan { .. } => CommandName::VideoDeflickerPlan,
            Self::Run { .. } => CommandName::VideoDeflickerRun,
            Self::Resume { .. } => CommandName::VideoDeflickerResume,
            Self::Status { .. } => CommandName::VideoDeflickerStatus,
            Self::RecordReview { .. } => CommandName::VideoDeflickerRecordReview,
        }
    }
}
pub(super) fn dispatch(
    command: DeflickerCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let token = cli_cancellation_token()?;
    let read = |p: &Path| {
        Request::from_json_slice(&read_audio_configuration(
            p,
            "explicit deflicker comparison request",
        )?)
    };
    let value = match command {
        DeflickerCommands::Plan { request } => {
            let p = comparison::plan(&read(&request)?, &token)?;
            return print_result(name, presentation, &p, || {
                println!(
                    "Comparison selected: {}; no rendering executed",
                    !p.bypassed
                )
            });
        }
        DeflickerCommands::Run {
            request,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            comparison::run(&read(&request)?, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?
        }
        DeflickerCommands::Resume { run_directory } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            comparison::resume(&root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?
        }
        DeflickerCommands::Status { run_directory } => {
            comparison::status(&absolute_delivery_path(run_directory)?, &token)?
        }
        DeflickerCommands::RecordReview {
            run_directory,
            declaration,
        } => comparison::record_review(
            &absolute_delivery_path(run_directory)?,
            &read_delivery_document(&declaration, "supplied deflicker excerpt review")?,
            &token,
        )?,
    };
    print_result(name, presentation, &value, || {
        println!(
            "Comparison complete: {}; bypassed: {}",
            value.complete, value.bypassed
        );
        if let Some(path) = &value.comparison_directory {
            println!(
                "Silent before/after preview: {}",
                path.join("preview.html").display()
            );
        }
        println!("Excerpt review and full-video approval remain separate");
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_request_and_separate_review_are_required() {
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "deflicker",
                "run",
                "--request",
                "compare.json"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "deflicker",
                "resume",
                "--run-directory",
                "private/compare",
                "--request",
                "changed.json"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "deflicker",
                "record-review",
                "--run-directory",
                "private/compare"
            ])
            .is_err()
        );
    }
}
