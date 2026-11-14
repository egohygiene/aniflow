//! Thin delivery for the existing fixed recipe review authority.
use super::*;
use aniflow::music_video_recipe::{manual::ManualReview, prepared_review};

#[derive(Debug, Subcommand)]
pub(super) enum FrameReviewCommands {
    /// Read exact frame locators, context/subject fingerprints and current decision.
    Status {
        #[arg(long)]
        run_directory: PathBuf,
    },
    /// Retain an explicit supplied declaration once; never synthesize approval.
    Record {
        #[arg(long)]
        run_directory: PathBuf,
        /// Existing aniflow.manual-review/v1 JSON, at most 64 KiB.
        #[arg(long)]
        declaration: PathBuf,
    },
}
impl FrameReviewCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Status { .. } => CommandName::VideoFrameReviewStatus,
            Self::Record { .. } => CommandName::VideoFrameReviewRecord,
        }
    }
}
pub(super) fn dispatch(
    command: FrameReviewCommands,
    presentation: Presentation,
) -> CommandResult<()> {
    let name = command.name();
    let result = match command {
        FrameReviewCommands::Status { run_directory } => {
            prepared_review::status(absolute_delivery_path(run_directory)?)?
        }
        FrameReviewCommands::Record {
            run_directory,
            declaration,
        } => {
            let declaration: ManualReview =
                read_delivery_document(&declaration, "prepared-frame review declaration")?;
            prepared_review::record(absolute_delivery_path(run_directory)?, &declaration)?
        }
    };
    print_result(name, presentation, &result, || {
        println!(
            "Prepared frames: {:?} ({} images)",
            result.review.state, result.frame_count
        );
        println!(
            "  directory: {}",
            escape_terminal_controls(&result.frames_directory.display().to_string())
        );
        println!(
            "  index: {}",
            escape_terminal_controls(&result.frame_index.display().to_string())
        );
        println!("  context_sha256: {}", result.review.context_sha256);
        println!(
            "  subject_sha256: {}",
            result
                .review
                .subject_sha256
                .as_deref()
                .unwrap_or("awaiting input")
        );
        println!("Decision is an operator declaration; assembly resumes separately");
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_delivery_requires_a_concrete_declaration_without_default_approval() {
        let base = [
            "aniflow",
            "--output",
            "json",
            "video",
            "frame-review",
            "record",
            "--run-directory",
            "recipe 音 with spaces",
        ];
        assert!(Cli::try_parse_from(base).is_err());
        let parsed = Cli::try_parse_from(
            base.into_iter()
                .chain(["--declaration", "operator decision.json"]),
        )
        .unwrap();
        assert_eq!(parsed.command.name(), CommandName::VideoFrameReviewRecord);
        for flag in ["--approve", "--reject", "--resume", "--force", "--execute"] {
            assert!(
                Cli::try_parse_from(base.into_iter().chain([
                    "--declaration",
                    "decision.json",
                    flag
                ]))
                .is_err()
            );
        }
    }
    #[test]
    fn readonly_review_status_does_not_select_a_write_or_native_mode() {
        let base = [
            "aniflow",
            "video",
            "frame-review",
            "status",
            "--run-directory",
            "recipe",
        ];
        assert_eq!(
            Cli::try_parse_from(base).unwrap().command.name(),
            CommandName::VideoFrameReviewStatus
        );
        assert!(
            Cli::try_parse_from(base.into_iter().chain(["--declaration", "decision.json"]))
                .is_err()
        );
    }
}
