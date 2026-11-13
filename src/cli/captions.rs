//! Explicit selections over the public caption facade.
use super::*;
use aniflow::subtitle_delivery::{
    self as captions, CaptionDeliveryRequest, CaptionRequest, CaptionRunRequest,
};
#[derive(Debug, Clone, Args)]
pub(super) struct CaptionArguments {
    #[arg(long)]
    request: PathBuf,
}
impl CaptionArguments {
    fn run(self) -> Result<CaptionRunRequest> {
        let mut request = CaptionRequest::from_json_slice(&read_audio_configuration(
            &self.request,
            "caption request",
        )?)?;
        request.video_run_directory = absolute_delivery_path(request.video_run_directory)?;
        request.lyrics_run_directory = absolute_delivery_path(request.lyrics_run_directory)?;
        request.lyrics_delivery_directory =
            absolute_delivery_path(request.lyrics_delivery_directory)?;
        request.configuration.font.file = absolute_delivery_path(request.configuration.font.file)?;
        request.configuration.settings.ffmpeg.executable =
            absolute_delivery_path(request.configuration.settings.ffmpeg.executable)?;
        request.configuration.settings.ffprobe.executable =
            absolute_delivery_path(request.configuration.settings.ffprobe.executable)?;
        Ok(CaptionRunRequest {
            request,
            provider_executable: std::env::current_exe().map_err(|_| {
                Error::new(
                    ErrorCategory::Dependency,
                    "cannot identify caption provider executable",
                )
            })?,
        })
    }
}
#[derive(Debug, Clone, Args)]
pub(super) struct DeliveryArguments {
    #[command(flatten)]
    arguments: CaptionArguments,
    #[arg(long)]
    preview_run_directory: PathBuf,
    /// Supplied declaration approving the exact preview context/subject.
    #[arg(long)]
    declaration: PathBuf,
}
impl DeliveryArguments {
    fn request(self) -> Result<CaptionDeliveryRequest> {
        let declaration = serde_json::from_slice(&read_audio_configuration(
            &self.declaration,
            "caption preview declaration",
        )?)
        .map_err(|_| {
            Error::new(
                ErrorCategory::Configuration,
                "invalid caption preview declaration",
            )
        })?;
        Ok(CaptionDeliveryRequest {
            run: self.arguments.run()?,
            preview_run_directory: absolute_delivery_path(self.preview_run_directory)?,
            declaration,
        })
    }
}
#[derive(Debug, Subcommand)]
pub(super) enum CaptionCommands {
    PreviewPlan {
        #[command(flatten)]
        arguments: CaptionArguments,
    },
    PreviewRun {
        #[command(flatten)]
        arguments: CaptionArguments,
        #[arg(long)]
        output_directory: PathBuf,
    },
    PreviewResume {
        #[command(flatten)]
        arguments: CaptionArguments,
        #[arg(long)]
        run_directory: PathBuf,
    },
    PreviewStatus {
        #[arg(long)]
        run_directory: PathBuf,
    },
    DeliveryPlan {
        #[command(flatten)]
        arguments: DeliveryArguments,
    },
    DeliveryRun {
        #[command(flatten)]
        arguments: DeliveryArguments,
        #[arg(long)]
        output_directory: PathBuf,
    },
    DeliveryResume {
        #[command(flatten)]
        arguments: DeliveryArguments,
        #[arg(long)]
        run_directory: PathBuf,
    },
    DeliveryStatus {
        #[arg(long)]
        run_directory: PathBuf,
    },
}
impl CaptionCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::PreviewPlan { .. } => CommandName::VideoCaptionPreviewPlan,
            Self::PreviewRun { .. } => CommandName::VideoCaptionPreviewRun,
            Self::PreviewResume { .. } => CommandName::VideoCaptionPreviewResume,
            Self::PreviewStatus { .. } => CommandName::VideoCaptionPreviewStatus,
            Self::DeliveryPlan { .. } => CommandName::VideoCaptionDeliveryPlan,
            Self::DeliveryRun { .. } => CommandName::VideoCaptionDeliveryRun,
            Self::DeliveryResume { .. } => CommandName::VideoCaptionDeliveryResume,
            Self::DeliveryStatus { .. } => CommandName::VideoCaptionDeliveryStatus,
        }
    }
}
pub(super) fn dispatch(command: CaptionCommands, presentation: Presentation) -> CommandResult<()> {
    let name = command.name();
    match command {
        CaptionCommands::PreviewPlan { arguments } => {
            let value = captions::preview_plan(&arguments.run()?, &cli_cancellation_token()?)?;
            print_result(name, presentation, &value, || {
                print_pipeline_v3_plan(&value)
            })
        }
        CaptionCommands::DeliveryPlan { arguments } => {
            let value = captions::delivery_plan(&arguments.request()?, &cli_cancellation_token()?)?;
            print_result(name, presentation, &value, || {
                print_pipeline_v3_plan(&value)
            })
        }
        CaptionCommands::PreviewStatus { run_directory } => {
            let value = captions::preview_status(
                &absolute_delivery_path(run_directory)?,
                &cli_cancellation_token()?,
            )?;
            print_result(name, presentation, &value, || {
                println!(
                    "{} selected rendered frames; context {}; subject {}; full delivery review remains separate",
                    value.report.frames.len(),
                    value.binding.context_sha256,
                    value.binding.subject_sha256
                )
            })
        }
        CaptionCommands::DeliveryStatus { run_directory } => {
            let value = captions::delivery_report(
                &absolute_delivery_path(run_directory)?,
                &cli_cancellation_token()?,
            )?;
            print_result(name, presentation, &value, || {
                println!(
                    "{:?} candidate: {} bytes; final creative review remains separate",
                    value.operation.request.configuration.mode, value.media.output_byte_count
                )
            })
        }
        CaptionCommands::PreviewRun {
            arguments,
            output_directory,
        } => {
            let request = arguments.run()?;
            let root = absolute_delivery_path(output_directory)?;
            let token = cli_cancellation_token()?;
            let mut recovery = None;
            let result = captions::preview_run(request, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|error| CommandFailure::pipeline_v3(error, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        CaptionCommands::DeliveryRun {
            arguments,
            output_directory,
        } => {
            let request = arguments.request()?;
            let root = absolute_delivery_path(output_directory)?;
            let token = cli_cancellation_token()?;
            let mut recovery = None;
            let result = captions::delivery_run(request, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|error| CommandFailure::pipeline_v3(error, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        CaptionCommands::PreviewResume {
            arguments,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let request = arguments
                .run()
                .map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let token = cli_cancellation_token()
                .map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let mut recovery = Some(root.clone());
            let result = captions::preview_resume(request, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        CaptionCommands::DeliveryResume {
            arguments,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let request = arguments
                .request()
                .map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let token = cli_cancellation_token()
                .map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let mut recovery = Some(root.clone());
            let result = captions::delivery_resume(request, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_always_requires_supplied_preview_and_declaration() {
        let base = [
            "aniflow",
            "video",
            "captions",
            "delivery-plan",
            "--request",
            "caption.json",
        ];
        assert!(Cli::try_parse_from(base).is_err());
        assert_eq!(
            Cli::try_parse_from(base.into_iter().chain([
                "--preview-run-directory",
                "preview",
                "--declaration",
                "my-review.json"
            ]))
            .unwrap()
            .command
            .name(),
            CommandName::VideoCaptionDeliveryPlan
        );
        assert!(Cli::try_parse_from(base.into_iter().chain(["--approve"])).is_err());
    }
    #[test]
    fn preview_status_is_read_only_without_invented_acceptance() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        std::fs::write(root.join("private.json"), b"loose report").unwrap();
        assert!(
            dispatch(
                CaptionCommands::PreviewStatus {
                    run_directory: root.clone()
                },
                Presentation::Machine
            )
            .is_err()
        );
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
    }
}
