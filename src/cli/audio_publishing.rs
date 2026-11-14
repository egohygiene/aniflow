//! Thin delivery of fixed audio publishing and supplied exact-file decisions.
use super::*;
use aniflow::music_video_recipe::publishing::audio::{
    self, AudioPublishingRequest, AudioPublishingStatus,
};
#[derive(Debug, Subcommand)]
pub(super) enum AudioPublishingCommands {
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
    NextAction {
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
impl AudioPublishingCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Plan { .. } => CommandName::DeliveryAudioPublishingPlan,
            Self::Run { .. } => CommandName::DeliveryAudioPublishingRun,
            Self::Resume { .. } => CommandName::DeliveryAudioPublishingResume,
            Self::Status { .. } => CommandName::DeliveryAudioPublishingStatus,
            Self::NextAction { .. } => CommandName::DeliveryAudioPublishingNextAction,
            Self::RecordReview { .. } => CommandName::DeliveryAudioPublishingRecordReview,
        }
    }
}
fn read(path: &Path) -> Result<AudioPublishingRequest> {
    AudioPublishingRequest::from_json_slice(&read_audio_configuration(
        path,
        "audio publishing request",
    )?)
}
fn human(value: &AudioPublishingStatus) {
    for step in &value.steps {
        println!("{:?}: {:?}", step.phase, step.state);
    }
    println!("Next: {}", value.next_action.action);
    println!("{}", escape_terminal_controls(&value.next_action.reason));
    if let (Some(context), Some(subject)) = (
        &value.next_action.review_context_sha256,
        &value.next_action.review_subject_sha256,
    ) {
        println!("Review context: {context}");
        println!("Final-file subject: {subject}");
    }
}
pub(super) fn dispatch(
    command: AudioPublishingCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let token = cli_cancellation_token()?;
    let value = match command {
        AudioPublishingCommands::Plan { request } => {
            let value = audio::plan(&read(&request)?, &token)?;
            return print_result(name, presentation, &value, || {
                println!(
                    "Fixed audio publishing intent: {}",
                    value.intent.intent_sha256
                );
                println!(
                    "Later bindings await actual accepted predecessors and supplied whole-file/public-metadata review"
                );
            });
        }
        AudioPublishingCommands::Run {
            request,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            audio::run(read(&request)?, &root, &token, |_, event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?
        }
        AudioPublishingCommands::Resume { run_directory } => {
            let root = absolute_delivery_path(run_directory)?;
            let mut recovery = Some(root.clone());
            audio::resume(&root, &token, |_, event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, Some(root)))?
        }
        AudioPublishingCommands::Status { run_directory } => {
            audio::status(&absolute_delivery_path(run_directory)?, &token)?
        }
        AudioPublishingCommands::NextAction { run_directory } => {
            let value = audio::next_action(&absolute_delivery_path(run_directory)?, &token)?;
            return print_result(name, presentation, &value, || {
                println!("Next: {}", value.action);
                println!("{}", escape_terminal_controls(&value.reason));
            });
        }
        AudioPublishingCommands::RecordReview {
            run_directory,
            declaration,
        } => {
            let review = read_delivery_document(
                &declaration,
                "supplied exact-final-audio/public-metadata decision",
            )?;
            audio::record_review(&absolute_delivery_path(run_directory)?, &review, &token)?
        }
    };
    print_result(name, presentation, &value, || human(&value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_publishing_requires_explicit_request_root_and_supplied_review() {
        let args = [
            "aniflow",
            "delivery",
            "audio-publishing",
            "run",
            "--request",
            "audio.json",
        ];
        assert!(Cli::try_parse_from(args).is_err());
        assert!(
            Cli::try_parse_from(
                args.into_iter()
                    .chain(["--run-directory", "private/audio-publish"])
            )
            .is_ok()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "delivery",
                "audio-publishing",
                "resume",
                "--run-directory",
                "private/audio-publish",
                "--request",
                "changed.json"
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "delivery",
                "audio-publishing",
                "record-review",
                "--run-directory",
                "private/audio-publish"
            ])
            .is_err()
        );
    }
    #[test]
    fn status_and_advice_have_no_review_execution_upload_or_sealing_switches() {
        for verb in ["status", "next-action"] {
            let args = [
                "aniflow",
                "delivery",
                "audio-publishing",
                verb,
                "--run-directory",
                "private/audio-publish",
            ];
            assert!(Cli::try_parse_from(args).is_ok());
            for flag in [
                "--approve",
                "--execute",
                "--upload",
                "--seal",
                "--sign",
                "--install",
            ] {
                assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());
            }
        }
    }
}
