//! Fixed remux delivery; policy, preservation and acceptance stay library-owned.
use super::*;
use aniflow::delivery_metadata::remux;

#[derive(Debug, Clone, Args)]
pub(super) struct RemuxArguments {
    #[arg(long)]
    source_video: PathBuf,
    /// Existing AssemblyConfiguration JSON with explicit delivery_metadata policy.
    #[arg(long)]
    configuration: PathBuf,
    /// Explicit StreamSelection JSON, including preserve_exact audio extent policy.
    #[arg(long)]
    selection: PathBuf,
    /// Exact current native soft-caption child; paired with its artifact ID.
    #[arg(long,requires="subtitle_artifact_id")]
    preserve_subtitles_from_run:Option<PathBuf>,
    #[arg(long,requires="preserve_subtitles_from_run")]
    subtitle_artifact_id:Option<String>,
}
impl RemuxArguments {
    fn request(self) -> Result<remux::RemuxRequest> {
        let settings = aniflow::video_assembly::AssemblyConfiguration::from_json_slice(
            &read_audio_configuration(&self.configuration, "metadata remux assembly settings")?,
        )?;
        let selection = read_delivery_document(&self.selection, "metadata remux stream selection")?;
        let provider = std::env::current_exe().map_err(|_| {
            Error::new(ErrorCategory::Dependency, "cannot identify the native aniflow provider executable")
        })?;
        let mut request=remux::RemuxRequest::new(absolute_delivery_path(self.source_video)?, selection, settings, provider);
        if let (Some(root),Some(id))=(self.preserve_subtitles_from_run,self.subtitle_artifact_id) {
            request.subtitle_preservation=Some(remux::SubtitlePreservation::observe(&absolute_delivery_path(root)?,&id,&CancellationToken::default())?);
        }
        Ok(request)
    }
}
#[derive(Debug, Subcommand)]
pub(super) enum RemuxCommands {
    /// Bind source, policy, selected streams, tools and bounds; never launch tools.
    Plan {
        #[command(flatten)]
        arguments: RemuxArguments,
    },
    /// Create a fresh isolated candidate; never replace source or master bytes.
    Run {
        #[command(flatten)]
        arguments: RemuxArguments,
        /// Existing canonical parent separate from source and executable files.
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Resume the exact fixed run after fresh source, policy and tool-pin checks.
    Resume {
        #[command(flatten)]
        arguments: RemuxArguments,
        #[arg(long)]
        run_directory: PathBuf,
    },
    /// Read only counts/scope/status from a currently accepted fixed remux run.
    Status {
        #[arg(long)]
        run_directory: PathBuf,
    },
}
impl RemuxCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Plan { .. } => CommandName::VideoMetadataRemuxPlan,
            Self::Run { .. } => CommandName::VideoMetadataRemuxRun,
            Self::Resume { .. } => CommandName::VideoMetadataRemuxResume,
            Self::Status { .. } => CommandName::VideoMetadataRemuxStatus,
        }
    }
}
pub(super) fn dispatch(command: RemuxCommands, presentation: Presentation) -> CommandResult<()> {
    let name = command.name();
    match command {
        RemuxCommands::Plan { arguments } => {
            let result = remux::plan(&arguments.request()?, &cli_cancellation_token()?)?;
            print_result(name, presentation, &result, || print_pipeline_v3_plan(&result))
        }
        RemuxCommands::Run { arguments, output_directory } => {
            let request = arguments.request()?;
            let parent = absolute_delivery_path(output_directory)?;
            let cancellation = cli_cancellation_token()?;
            let mut recovery = None;
            let progress = |event: &PipelineV3RunProgress| observe_pipeline_v3_progress(presentation, &mut recovery, event);
            let result = remux::run(request, parent, &cancellation, progress)
                .map_err(|error| CommandFailure::pipeline_v3(error, recovery))?;
            print_result(name, presentation, &result, || print_pipeline_v3_outcome(&result))
        }
        RemuxCommands::Resume { arguments, run_directory } => {
            let root = absolute_delivery_path(run_directory)?;
            let request = arguments.request()
                .map_err(|error| CommandFailure::pipeline_v3(error, Some(root.clone())))?;
            let cancellation = cli_cancellation_token()
                .map_err(|error| CommandFailure::pipeline_v3(error, Some(root.clone())))?;
            let mut recovery = Some(root.clone());
            let progress = |event: &PipelineV3RunProgress| observe_pipeline_v3_progress(presentation, &mut recovery, event);
            let result = remux::resume(request, &root, &cancellation, progress)
                .map_err(|error| CommandFailure::pipeline_v3(error, recovery))?;
            print_result(name, presentation, &result, || print_pipeline_v3_outcome(&result))
        }
        RemuxCommands::Status { run_directory } => {
            // No private-report mode: fixed accepted owner supplies the allowlist.
            let result = remux::public_summary(&absolute_delivery_path(run_directory)?)?;
            print_result(name, presentation, &result, || {
                println!("Metadata: {:?}; policy {:?}", result.status, result.mode);
                println!("Fields: {} before, {} after, {} residual, {} regenerated", result.before_fields, result.after_fields, result.residual_fields, result.regenerated_fields);
                println!("Inspected scope: {}", escape_terminal_controls(&result.inspected_scope));
                println!("Unsupported scope: {}", escape_terminal_controls(&result.unsupported_scope));
                println!("Source credentials: {}", escape_terminal_controls(&result.source_credentials));
                println!("Signal marks: {}", escape_terminal_controls(&result.signal_marks));
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserving_subtitles_requires_the_native_run_and_exact_artifact_id_together() {
        let args=["aniflow","video","metadata-remux","plan","--source-video","selected.mp4","--configuration","tools.json","--selection","streams.json"];
        assert!(Cli::try_parse_from(args.into_iter().chain(["--preserve-subtitles-from-run","caption-run"])).is_err());
        assert!(Cli::try_parse_from(args.into_iter().chain(["--subtitle-artifact-id","caption_video"])).is_err());
        assert!(Cli::try_parse_from(args.into_iter().chain(["--preserve-subtitles-from-run","caption-run","--subtitle-artifact-id","caption_video"])).is_ok());
    }
    #[test]
    fn remux_execution_modes_require_explicit_bound_inputs_and_destination() {
        for (verb, extra, name) in [
            ("plan", vec![], CommandName::VideoMetadataRemuxPlan),
            ("run", vec!["--output-directory", "separate parent"], CommandName::VideoMetadataRemuxRun),
            ("resume", vec!["--run-directory", "existing run"], CommandName::VideoMetadataRemuxResume),
        ] {
            let prefix = ["aniflow", "--output", "json", "video", "metadata-remux", verb, "--source-video", "source with spaces.mp4", "--configuration", "policy settings.json", "--selection", "selected streams.json"];
            let parsed = Cli::try_parse_from(prefix.into_iter().chain(extra.clone())).unwrap();
            assert_eq!(parsed.command.name(), name);
            assert!(Cli::try_parse_from(prefix.into_iter().chain(extra).chain(["--replace-source"])).is_err());
        }
        let missing_selection = ["aniflow", "video", "metadata-remux", "run", "--source-video", "source.mp4", "--configuration", "settings.json", "--output-directory", "parent"];
        assert!(Cli::try_parse_from(missing_selection).is_err());
        for verb in ["run", "resume"] {
            assert!(Cli::try_parse_from(["aniflow", "video", "metadata-remux", verb, "--source-video", "source.mp4", "--configuration", "settings.json", "--selection", "selection.json"]).is_err());
        }
    }
    #[test]
    fn accepted_summary_delivery_has_no_private_or_execution_mode() {
        let prefix = ["aniflow", "--output", "json", "video", "metadata-remux", "status", "--run-directory", "accepted run"];
        let parsed = Cli::try_parse_from(prefix).unwrap();
        assert_eq!(parsed.command.name(), CommandName::VideoMetadataRemuxStatus);
        for flag in ["--private-report", "--execute", "--install", "--download", "--publish"] {
            assert!(Cli::try_parse_from(prefix.into_iter().chain([flag])).is_err());
        }
        assert!(Cli::try_parse_from(["aniflow", "video", "metadata-remux", "plan", "--source-video", "source.mp4", "--configuration", "settings.json", "--selection", "streams.json", "--output-directory", "parent"]).is_err());
    }
    #[test]
    fn empty_workspace_never_becomes_accepted_summary_or_written_state() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let result = dispatch(RemuxCommands::Status { run_directory: root.clone() }, Presentation::Machine);
        assert!(result.is_err());
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
    #[test]
    fn refused_resume_keeps_its_named_root_without_inventing_a_manifest() {
        let directory = tempfile::tempdir().unwrap();let root = directory.path().canonicalize().unwrap();
        let arguments = RemuxArguments { source_video: root.join("missing source.mp4"), configuration: root.join("missing settings.json"), selection: root.join("missing selection.json"),preserve_subtitles_from_run:None,subtitle_artifact_id:None };
        let failure = dispatch(RemuxCommands::Resume { arguments, run_directory: root.clone() }, Presentation::Machine).unwrap_err();
        let recovery = failure.pipeline_v3_recovery.unwrap();
        assert_eq!(recovery.run_directory, root);assert!(recovery.run_manifest.is_none());
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
}
