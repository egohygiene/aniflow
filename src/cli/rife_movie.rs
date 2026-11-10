//! CLI delivery of the public accepted-preview movie lifecycle.
use super::*;
use aniflow::video_interpolation::preview::movie::{self, MovieRequest};

#[derive(Debug, Subcommand)]
pub(super) enum MovieCommands {
    Plan { #[arg(long)] request: PathBuf },
    Run { #[arg(long)] request: PathBuf, #[arg(long)] output_directory: PathBuf },
    Resume { #[arg(long)] request: PathBuf, #[arg(long)] run_directory: PathBuf },
    /// Read an accepted viewing copy. Use status-v3 for incomplete native work.
    Status { #[arg(long)] run_directory: PathBuf },
}
impl MovieCommands {
    pub(super) fn name(&self) -> CommandName { match self {
        Self::Plan { .. } => CommandName::VideoRifePreviewMoviePlan,
        Self::Run { .. } => CommandName::VideoRifePreviewMovieRun,
        Self::Resume { .. } => CommandName::VideoRifePreviewMovieResume,
        Self::Status { .. } => CommandName::VideoRifePreviewMovieStatus,
    } }
}
fn read(path: &Path) -> Result<MovieRequest> { MovieRequest::from_json_slice(&read_audio_configuration(path, "RIFE preview movie request")?) }
pub(super) fn dispatch(command: MovieCommands, presentation: Presentation) -> CommandResult<()> {
    let name = command.name();
    match command {
        MovieCommands::Plan { request } => {
            let result = movie::plan(&read(&request)?, &cli_cancellation_token()?)?;
            print_result(name, presentation, &result, || print_pipeline_v3_plan(&result))
        }
        MovieCommands::Run { request, output_directory } => {
            let request = read(&request)?;
            let parent = absolute_delivery_path(output_directory)?;
            let token = cli_cancellation_token()?;
            let mut recovery = None;
            let result = movie::run(request, &parent, &token, |event| observe_pipeline_v3_progress(presentation, &mut recovery, event))
                .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &result, || print_pipeline_v3_outcome(&result))
        }
        MovieCommands::Resume { request, run_directory } => {
            let root = absolute_delivery_path(run_directory)?;
            let request = read(&request).map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let token = cli_cancellation_token().map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let mut recovery = Some(root.clone());
            let result = movie::resume(request, &root, &token, |event| observe_pipeline_v3_progress(presentation, &mut recovery, event))
                .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &result, || print_pipeline_v3_outcome(&result))
        }
        MovieCommands::Status { run_directory } => {
            let result = movie::observe(&absolute_delivery_path(run_directory)?, &cli_cancellation_token()?)?;
            print_result(name, presentation, &result, || println!("{} silent preview pictures: {}", result.report.media.frame_count, result.movie_path.display()))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn movie_commands_require_pinned_intent_and_explicit_lifecycle_locations() {
        let base = ["aniflow", "video", "rife", "preview-movie"];
        for (command, tail, name) in [
            ("plan", vec!["--request", "preview.json"], CommandName::VideoRifePreviewMoviePlan),
            ("run", vec!["--request", "preview.json", "--output-directory", "new runs"], CommandName::VideoRifePreviewMovieRun),
            ("resume", vec!["--request", "preview.json", "--run-directory", "old run"], CommandName::VideoRifePreviewMovieResume),
            ("status", vec!["--run-directory", "old run"], CommandName::VideoRifePreviewMovieStatus),
        ] {
            assert_eq!(Cli::try_parse_from(base.into_iter().chain([command]).chain(tail)).unwrap().command.name(), name);
        }
        assert!(Cli::try_parse_from(base.into_iter().chain(["run", "--request", "preview.json"])).is_err());
        assert!(Cli::try_parse_from(base.into_iter().chain(["plan", "--request", "preview.json", "--approve-full-video"])).is_err());
    }
    #[test]
    fn refused_movie_resume_preserves_the_exact_recovery_location() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let failure = dispatch(MovieCommands::Resume { request: root.join("missing.json"), run_directory: root.clone() }, Presentation::Machine).unwrap_err();
        assert_eq!(failure.pipeline_v3_recovery.unwrap().run_directory, root);
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
}
