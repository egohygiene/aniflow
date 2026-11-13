use super::*;
use aniflow::destination::video::{self, VideoExportRequest};
#[derive(Debug, Subcommand)]
pub(super) enum VideoExportCommands {
    Intent {
        #[arg(long)]
        request: PathBuf,
    },
    Plan {
        #[arg(long)]
        request: PathBuf,
    },
    Run {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
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
}
impl VideoExportCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Intent { .. } => CommandName::DeliveryVideoExportIntent,
            Self::Plan { .. } => CommandName::DeliveryVideoExportPlan,
            Self::Run { .. } => CommandName::DeliveryVideoExportRun,
            Self::Resume { .. } => CommandName::DeliveryVideoExportResume,
            Self::Status { .. } => CommandName::DeliveryVideoExportStatus,
        }
    }
}
pub(super) fn read(path: &Path) -> Result<VideoExportRequest> {
    VideoExportRequest::from_json_slice(&read_audio_configuration(
        path,
        "destination video request",
    )?)
}
pub(super) fn dispatch(
    command: VideoExportCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let token = cli_cancellation_token()?;
    match command {
        VideoExportCommands::Plan { request } => {
            let value = video::plan(&read(&request)?, &token)?;
            print_result(name, presentation, &value, || {
                println!("Native destination plan: {}", value.plan_sha256)
            })
        }
        VideoExportCommands::Run {
            request,
            output_directory,
        } => {
            let mut recovery = None;
            let value = video::run(
                read(&request)?,
                &absolute_delivery_path(output_directory)?,
                &token,
                |e| observe_pipeline_v3_progress(presentation, &mut recovery, e),
            )
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &value, || {
                println!(
                    "Native destination stages executed: {}",
                    value.executed_stages.len()
                )
            })
        }
        VideoExportCommands::Resume {
            request,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            let value = video::resume(read(&request)?, &root, &token, |e| {
                observe_pipeline_v3_progress(presentation, &mut recovery, e)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &value, || {
                println!(
                    "Native destination stages executed: {}",
                    value.executed_stages.len()
                )
            })
        }
        VideoExportCommands::Status { run_directory } => {
            let value = video::observe(&absolute_delivery_path(run_directory)?, &token)?;
            print_result(name, presentation, &value, || {
                println!(
                    "Accepted destination bytes: {}; final creative/platform review remains separate",
                    value.output_bytes
                )
            })
        }
        VideoExportCommands::Intent { request } => {
            let value = video::intent(&read(&request)?, &token)?;
            print_result(name, presentation, &value, || {
                let g = &value.payload.geometry;
                println!("Destination intent: {}", value.intent_sha256);
                println!(
                    "Scaled content: {} x {}; canvas: {} x {}",
                    g.scaled_width, g.scaled_height, g.width, g.height
                );
                println!(
                    "Source/clock/audio preserved; lossy quality and platform acceptance require separate checks"
                );
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_delivery_requires_explicit_output_or_exact_resume_request() {
        let run = [
            "aniflow",
            "delivery",
            "video-export",
            "run",
            "--request",
            "selected.json",
        ];
        assert!(Cli::try_parse_from(run).is_err());
        assert_eq!(
            Cli::try_parse_from(
                run.into_iter()
                    .chain(["--output-directory", "private candidates"])
            )
            .unwrap()
            .command
            .name(),
            CommandName::DeliveryVideoExportRun
        );
        let resume = [
            "aniflow",
            "delivery",
            "video-export",
            "resume",
            "--run-directory",
            "private run",
        ];
        assert!(Cli::try_parse_from(resume).is_err());
        assert_eq!(
            Cli::try_parse_from(resume.into_iter().chain(["--request", "exact.json"]))
                .unwrap()
                .command
                .name(),
            CommandName::DeliveryVideoExportResume
        );
    }
    #[test]
    fn intent_requires_an_explicit_request_without_automatic_transform_flags() {
        let args = [
            "aniflow",
            "delivery",
            "video-export",
            "intent",
            "--request",
            "private intent.json",
        ];
        assert_eq!(
            Cli::try_parse_from(args).unwrap().command.name(),
            CommandName::DeliveryVideoExportIntent
        );
        for flag in ["--crop", "--trim", "--normalize", "--post", "--download"] {
            assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());
        }
    }
}
