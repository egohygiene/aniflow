//! Thin selected historical-frame import delivery.
use super::*;
use aniflow::upscayl_recovery::historical::{self, ImportRequest};
#[derive(Debug, Subcommand)]
pub(super) enum HistoricalImportCommands {
    Prepare {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        output_request: Option<PathBuf>,
    },
    ReviewPlan {
        #[arg(long)]
        request: PathBuf,
    },
    Plan {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        declaration: PathBuf,
    },
    Run {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        declaration: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    Resume {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        declaration: PathBuf,
        #[arg(long)]
        run_directory: PathBuf,
    },
    Status {
        #[arg(long)]
        run_directory: PathBuf,
    },
}
impl HistoricalImportCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Prepare { .. } => CommandName::VideoHistoricalImportPrepare,
            Self::ReviewPlan { .. } => CommandName::VideoHistoricalImportReviewPlan,
            Self::Plan { .. } => CommandName::VideoHistoricalImportPlan,
            Self::Run { .. } => CommandName::VideoHistoricalImportRun,
            Self::Resume { .. } => CommandName::VideoHistoricalImportResume,
            Self::Status { .. } => CommandName::VideoHistoricalImportStatus,
        }
    }
}
fn read(path: &Path) -> Result<ImportRequest> {
    serde_json::from_slice(&read_bounded_configuration(
        path,
        "historical import request",
        262_144,
    )?)
    .map_err(|_| {
        Error::new(
            ErrorCategory::Configuration,
            "invalid closed historical import request",
        )
    })
}
fn decision(path: &Path) -> Result<aniflow::music_video_recipe::manual::ManualReview> {
    serde_json::from_slice(&read_audio_configuration(
        path,
        "historical import decision",
    )?)
    .map_err(|_| {
        Error::new(
            ErrorCategory::Configuration,
            "invalid supplied import decision",
        )
    })
}
pub(super) fn dispatch(
    command: HistoricalImportCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let token = cli_cancellation_token()?;
    match command {
        HistoricalImportCommands::Prepare {
            request,
            output_request,
        } => {
            let selected = historical::preparation::PreparationRequest::from_json_slice(
                &read_bounded_configuration(&request, "historical selection preparation", 262_144)?,
            )?;
            let value = historical::preparation::prepare(&selected, &token)?;
            if let Some(output) = output_request {
                historical::preparation::write_request(
                    &value,
                    &absolute_delivery_path(output)?,
                    &token,
                )?;
            }
            print_result(name, presentation, &value, || {
                println!(
                    "Prepared {} exact selected frames; current review subject {}; no import approval or native execution",
                    value.request.frames.len(),
                    value.review.subject_sha256
                )
            })
        }
        HistoricalImportCommands::ReviewPlan { request } => {
            let value = historical::review_plan(&read(&request)?, &token)?;
            print_result(name, presentation, &value, || {
                println!("Historical import context: {}", value.context_sha256);
                println!("Selected subject: {}", value.subject_sha256);
                println!(
                    "Selected frames: {}; prior history unknown; no import decision inferred",
                    value.intent.frames.len()
                );
            })
        }
        HistoricalImportCommands::Plan {
            request,
            declaration,
        } => {
            let value = historical::plan(&read(&request)?, &decision(&declaration)?, &token)?;
            print_result(name, presentation, &value, || {
                println!("Historical native import plan: {}", value.plan_sha256)
            })
        }
        HistoricalImportCommands::Run {
            request,
            declaration,
            output_directory,
        } => {
            let mut recovery = None;
            let value = historical::run(
                read(&request)?,
                decision(&declaration)?,
                &absolute_delivery_path(output_directory)?,
                &token,
                |e| observe_pipeline_v3_progress(presentation, &mut recovery, e),
            )
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &value, || {
                println!(
                    "Historical import native stages executed: {}",
                    value.executed_stages.len()
                )
            })
        }
        HistoricalImportCommands::Resume {
            request,
            declaration,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            let value = historical::resume(
                read(&request)?,
                decision(&declaration)?,
                &root,
                &token,
                |e| observe_pipeline_v3_progress(presentation, &mut recovery, e),
            )
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &value, || {
                println!(
                    "Historical import native stages executed: {}",
                    value.executed_stages.len()
                )
            })
        }
        HistoricalImportCommands::Status { run_directory } => {
            let value = historical::observe(&absolute_delivery_path(run_directory)?, &token)?;
            print_result(name, presentation, &value, || {
                println!(
                    "Accepted historical selection: {} frames; original history remains unknown",
                    value.subset.frames.len()
                )
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_requires_explicit_selection_and_has_no_approval_default() {
        let args = [
            "aniflow",
            "video",
            "historical-import",
            "prepare",
            "--request",
            "selected files.json",
            "--output-request",
            "separate import.json",
        ];
        assert_eq!(
            Cli::try_parse_from(args).unwrap().command.name(),
            CommandName::VideoHistoricalImportPrepare
        );
        for flag in ["--scan-folder", "--approve", "--adopt"] {
            assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());
        }
    }
    #[test]
    fn import_requires_supplied_decision_and_never_adopts_partial_outputs() {
        let args = [
            "aniflow",
            "video",
            "historical-import",
            "plan",
            "--request",
            "private frames.json",
        ];
        assert!(Cli::try_parse_from(args).is_err());
        assert_eq!(
            Cli::try_parse_from(args.into_iter().chain(["--declaration", "decision.json"]))
                .unwrap()
                .command
                .name(),
            CommandName::VideoHistoricalImportPlan
        );
        for flag in ["--adopt", "--approve", "--overwrite", "--guess-model"] {
            assert!(
                Cli::try_parse_from(args.into_iter().chain([
                    "--declaration",
                    "decision.json",
                    flag
                ]))
                .is_err()
            );
        }
    }
}
