//! Fixed runtime delivery; no second interpolation implementation or journal.
use super::*;
use aniflow::video_interpolation::preview::{self, PreviewRange, PreviewRequest};
use aniflow::video_interpolation::runtime;
#[derive(Debug, Clone, Args)]
pub(super) struct RifeArguments {
    #[arg(long)]
    frames_directory: PathBuf,
    #[arg(long)]
    frame_index: PathBuf,
    /// Closed configuration with exact local pins and finite resource bounds.
    #[arg(long)]
    configuration: Option<PathBuf>,
    #[command(flatten)]
    managed: managed::ManagedRifeArguments,
}
#[derive(Debug, Clone, Args)]
pub(super) struct PreviewArguments {
    #[command(flatten)]
    arguments: RifeArguments,
    #[arg(long)]
    first_source_ordinal: u64,
    /// Exclusive original-source end, at most 120 frames / ten seconds.
    #[arg(long)]
    end_source_ordinal: u64,
}
impl PreviewArguments {
    fn request(
        self,
        plan: bool,
        cancellation: &aniflow::CancellationToken,
        presentation: Presentation,
        name: CommandName,
        output: Option<&Path>,
    ) -> Result<Option<PreviewRequest>> {
        let Some(request) =
            self.arguments
                .request(plan, cancellation, presentation, name, output)?
        else {
            return Ok(None);
        };
        Ok(Some(PreviewRequest::new(
            request,
            PreviewRange {
                first_source_ordinal: self.first_source_ordinal,
                end_source_ordinal: self.end_source_ordinal,
            },
        )))
    }
}
impl RifeArguments {
    fn request(
        self,
        plan: bool,
        cancellation: &aniflow::CancellationToken,
        presentation: Presentation,
        name: CommandName,
        output: Option<&Path>,
    ) -> Result<Option<runtime::RifeRunRequest>> {
        if !self.frames_directory.is_dir() || !self.frame_index.is_file() {
            return Err(Error::new(
                ErrorCategory::Input,
                "RIFE indexed frames or frame index are unavailable; no managed acquisition started",
            ));
        }
        let bytes = self
            .configuration
            .as_ref()
            .map(|path| read_audio_configuration(path, "RIFE configuration"))
            .transpose()?;
        let mut paths = vec![self.frames_directory.clone(), self.frame_index.clone()];
        paths.extend(self.configuration.clone());
        if let Some(path) = output {
            paths.push(path.to_path_buf());
        }
        let Some(configuration) = self.managed.prepare(
            bytes.as_deref(),
            plan,
            cancellation,
            presentation,
            name,
            &paths,
        )?
        else {
            return Ok(None);
        };
        let provider = std::env::current_exe().map_err(|_| {
            Error::new(
                ErrorCategory::Dependency,
                "cannot identify the native aniflow provider executable",
            )
        })?;
        Ok(Some(runtime::RifeRunRequest::new(
            absolute_delivery_path(self.frames_directory)?,
            absolute_delivery_path(self.frame_index)?,
            configuration,
            provider,
        )))
    }
}
impl RifeArguments {
    fn resume_request(
        self,
        root: &Path,
        is_preview: bool,
        cancellation: &aniflow::CancellationToken,
        presentation: Presentation,
        name: CommandName,
    ) -> Result<runtime::RifeRunRequest> {
        if let Some(path) = &self.configuration {
            let bytes = read_audio_configuration(path, "RIFE resume configuration")?;
            if managed::is_schema(&bytes, aniflow::managed_experience::MANAGED_RIFE_SCHEMA) {
                return Err(Error::new(
                    ErrorCategory::Configuration,
                    "managed RIFE resume reuses retained exact native pins: omit --configuration and all managed overrides; no acquisition started",
                ));
            }
            return self
                .request(false, cancellation, presentation, name, Some(root))?
                .ok_or_else(|| {
                    Error::new(
                        ErrorCategory::Internal,
                        "RIFE resume produced no exact request",
                    )
                });
        }
        if self.managed.present() {
            return Err(Error::new(
                ErrorCategory::Configuration,
                "RIFE resume without --configuration reuses retained exact settings; omit managed overrides",
            ));
        }
        let configuration = if is_preview {
            preview::retained_configuration(root)?
        } else {
            runtime::retained_configuration(root)?
        };
        let provider = std::env::current_exe().map_err(|_| {
            Error::new(
                ErrorCategory::Dependency,
                "cannot identify native provider executable",
            )
        })?;
        Ok(runtime::RifeRunRequest::new(
            absolute_delivery_path(self.frames_directory)?,
            absolute_delivery_path(self.frame_index)?,
            configuration,
            provider,
        ))
    }
}
impl PreviewArguments {
    fn resume_request(
        self,
        root: &Path,
        cancellation: &aniflow::CancellationToken,
        presentation: Presentation,
        name: CommandName,
    ) -> Result<PreviewRequest> {
        Ok(PreviewRequest::new(
            self.arguments
                .resume_request(root, true, cancellation, presentation, name)?,
            PreviewRange {
                first_source_ordinal: self.first_source_ordinal,
                end_source_ordinal: self.end_source_ordinal,
            },
        ))
    }
}

#[derive(Debug, Subcommand)]
pub(super) enum RifeCommands {
    Recovered {
        #[command(subcommand)]
        command: recovered_rife::RecoveredRifeCommands,
    },
    PreviewMovie {
        #[command(subcommand)]
        command: rife_movie::MovieCommands,
    },
    PreviewPlan {
        #[command(flatten)]
        arguments: PreviewArguments,
    },
    PreviewRun {
        #[command(flatten)]
        arguments: PreviewArguments,
        #[arg(long)]
        output_directory: PathBuf,
    },
    PreviewResume {
        #[command(flatten)]
        arguments: PreviewArguments,
        #[arg(long)]
        run_directory: PathBuf,
    },
    /// Read current accepted preview PNGs/mapping; use status-v3 for partial work.
    PreviewStatus {
        #[arg(long)]
        run_directory: PathBuf,
    },
    /// Recheck actual PNG/index/tool/model bytes and bounds without launching tools.
    Plan {
        #[command(flatten)]
        arguments: RifeArguments,
    },
    /// Start one isolated ordinary v3 serial interpolation run.
    Run {
        #[command(flatten)]
        arguments: RifeArguments,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Resume the exact run with unchanged inputs, pins and settings.
    Resume {
        #[command(flatten)]
        arguments: RifeArguments,
        #[arg(long)]
        run_directory: PathBuf,
    },
    /// Read fixed-profile native state without processes or repair.
    Status {
        #[arg(long)]
        run_directory: PathBuf,
    },
}
impl RifeCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Recovered { command } => command.name(),
            Self::PreviewMovie { command } => command.name(),
            Self::PreviewPlan { .. } => CommandName::VideoRifePreviewPlan,
            Self::PreviewRun { .. } => CommandName::VideoRifePreviewRun,
            Self::PreviewResume { .. } => CommandName::VideoRifePreviewResume,
            Self::PreviewStatus { .. } => CommandName::VideoRifePreviewStatus,
            Self::Plan { .. } => CommandName::VideoRifePlan,
            Self::Run { .. } => CommandName::VideoRifeRun,
            Self::Resume { .. } => CommandName::VideoRifeResume,
            Self::Status { .. } => CommandName::VideoRifeStatus,
        }
    }
}
pub(super) fn dispatch(command: RifeCommands, presentation: Presentation) -> CommandResult<()> {
    let name = command.name();
    match command {
        RifeCommands::Recovered { command } => recovered_rife::dispatch(command, presentation),
        RifeCommands::PreviewMovie { command } => rife_movie::dispatch(command, presentation),
        RifeCommands::PreviewPlan { arguments } => {
            let token = cli_cancellation_token()?;
            let Some(request) = arguments.request(true, &token, presentation, name, None)? else {
                return Ok(());
            };
            let result = preview::plan(&request, &token)?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_plan(&result)
            })
        }
        RifeCommands::PreviewRun {
            arguments,
            output_directory,
        } => {
            let root = absolute_delivery_path(output_directory)?;
            let token = cli_cancellation_token()?;
            let request = arguments
                .request(false, &token, presentation, name, Some(&root))?
                .ok_or_else(|| {
                    Error::new(
                        ErrorCategory::Internal,
                        "managed RIFE execution produced no exact request",
                    )
                })?;
            let mut recovery = None;
            let result = preview::run(request, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        RifeCommands::PreviewResume {
            arguments,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let token = cli_cancellation_token()
                .map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let request = arguments
                .resume_request(&root, &token, presentation, name)
                .map_err(|e| CommandFailure::pipeline_v3(e, Some(root.clone())))?;
            let mut recovery = Some(root.clone());
            let result = preview::resume(request, &root, &token, |event| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            })
            .map_err(|e| CommandFailure::pipeline_v3(e, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        RifeCommands::PreviewStatus { run_directory } => {
            let result = preview::observe(
                &absolute_delivery_path(run_directory)?,
                &cli_cancellation_token()?,
            )?;
            print_result(name, presentation, &result, || {
                println!(
                    "{} preview PNGs on the original clock; no full-run or perceptual acceptance",
                    result.report.frames.len()
                )
            })
        }
        RifeCommands::Plan { arguments } => {
            let token = cli_cancellation_token()?;
            let Some(request) = arguments.request(true, &token, presentation, name, None)? else {
                return Ok(());
            };
            let result = runtime::plan(&request, &token)?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_plan(&result)
            })
        }
        RifeCommands::Run {
            arguments,
            output_directory,
        } => {
            let parent = absolute_delivery_path(output_directory)?;
            let token = cli_cancellation_token()?;
            let request = arguments
                .request(false, &token, presentation, name, Some(&parent))?
                .ok_or_else(|| {
                    Error::new(
                        ErrorCategory::Internal,
                        "managed RIFE execution produced no exact request",
                    )
                })?;
            let mut recovery = None;
            let progress = |event: &PipelineV3RunProgress| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            };
            let result = runtime::run(request, Some(parent), &token, progress)
                .map_err(|error| CommandFailure::pipeline_v3(error, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        RifeCommands::Resume {
            arguments,
            run_directory,
        } => {
            let root = absolute_delivery_path(run_directory)?;
            let token = cli_cancellation_token()
                .map_err(|error| CommandFailure::pipeline_v3(error, Some(root.clone())))?;
            let request = arguments
                .resume_request(&root, false, &token, presentation, name)
                .map_err(|error| CommandFailure::pipeline_v3(error, Some(root.clone())))?;
            let mut recovery = Some(root.clone());
            let progress = |event: &PipelineV3RunProgress| {
                observe_pipeline_v3_progress(presentation, &mut recovery, event)
            };
            let result = runtime::resume(request, &root, &token, progress)
                .map_err(|error| CommandFailure::pipeline_v3(error, recovery))?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_outcome(&result)
            })
        }
        RifeCommands::Status { run_directory } => {
            let result = runtime::status(absolute_delivery_path(run_directory)?)?;
            print_result(name, presentation, &result, || {
                print_pipeline_v3_status(&result)
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_requires_an_explicit_window_and_keeps_full_run_commands_separate() {
        let base = [
            "aniflow",
            "video",
            "rife",
            "preview-plan",
            "--frames-directory",
            "frames",
            "--frame-index",
            "index.json",
            "--configuration",
            "rife.json",
        ];
        assert!(Cli::try_parse_from(base).is_err());
        assert_eq!(
            Cli::try_parse_from(base.into_iter().chain([
                "--first-source-ordinal",
                "30",
                "--end-source-ordinal",
                "90"
            ]))
            .unwrap()
            .command
            .name(),
            CommandName::VideoRifePreviewPlan
        );
        assert!(
            Cli::try_parse_from(base.into_iter().chain([
                "--first-source-ordinal",
                "30",
                "--end-source-ordinal",
                "90",
                "--approve-full-video"
            ]))
            .is_err()
        );
    }
    #[test]
    fn refused_preview_resume_retains_the_explicit_recovery_root() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let arguments = PreviewArguments {
            arguments: RifeArguments {
                frames_directory: root.join("frames"),
                frame_index: root.join("index.json"),
                configuration: Some(root.join("missing.json")),
                managed: managed::ManagedRifeArguments::default(),
            },
            first_source_ordinal: 0,
            end_source_ordinal: 1,
        };
        let failure = dispatch(
            RifeCommands::PreviewResume {
                arguments,
                run_directory: root.clone(),
            },
            Presentation::Machine,
        )
        .unwrap_err();
        assert_eq!(failure.pipeline_v3_recovery.unwrap().run_directory, root);
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
    #[test]
    fn rife_modes_require_indexed_inputs_and_separate_destinations() {
        for (mode, tail, name) in [
            ("plan", vec![], CommandName::VideoRifePlan),
            (
                "run",
                vec!["--output-directory", "new runs"],
                CommandName::VideoRifeRun,
            ),
            (
                "resume",
                vec!["--run-directory", "existing run"],
                CommandName::VideoRifeResume,
            ),
        ] {
            let base = [
                "aniflow",
                "--output",
                "json",
                "video",
                "rife",
                mode,
                "--frames-directory",
                "frames 音",
                "--frame-index",
                "index.json",
                "--configuration",
                "rife.json",
            ];
            assert_eq!(
                Cli::try_parse_from(base.into_iter().chain(tail.clone()))
                    .unwrap()
                    .command
                    .name(),
                name
            );
            assert!(
                Cli::try_parse_from(base.into_iter().chain(tail).chain(["--download"])).is_err()
            );
        }
        for mode in ["run", "resume"] {
            assert!(
                Cli::try_parse_from([
                    "aniflow",
                    "video",
                    "rife",
                    mode,
                    "--frames-directory",
                    "frames",
                    "--frame-index",
                    "index.json",
                    "--configuration",
                    "rife.json"
                ])
                .is_err()
            );
        }
    }
    #[test]
    fn rife_status_has_no_execution_or_model_selection_effects() {
        let base = [
            "aniflow",
            "video",
            "rife",
            "status",
            "--run-directory",
            "existing run",
        ];
        assert_eq!(
            Cli::try_parse_from(base).unwrap().command.name(),
            CommandName::VideoRifeStatus
        );
        for flag in [
            "--execute",
            "--force",
            "--install",
            "--download",
            "--auto-model",
        ] {
            assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());
        }
    }
    #[test]
    fn loose_status_refuses_without_creating_runtime_authority() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        std::fs::write(root.join("output-index.json"), b"unaccepted loose bytes").unwrap();
        assert!(
            dispatch(
                RifeCommands::Status {
                    run_directory: root.clone()
                },
                Presentation::Machine
            )
            .is_err()
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        assert!(!root.join("writer.lock").exists());
    }
    #[test]
    fn refused_resume_keeps_named_root_without_inventing_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let arguments = RifeArguments {
            frames_directory: root.join("frames"),
            frame_index: root.join("index.json"),
            configuration: Some(root.join("missing.json")),
            managed: managed::ManagedRifeArguments::default(),
        };
        let failure = dispatch(
            RifeCommands::Resume {
                arguments,
                run_directory: root.clone(),
            },
            Presentation::Machine,
        )
        .unwrap_err();
        let recovery = failure.pipeline_v3_recovery.unwrap();
        assert_eq!(recovery.run_directory, root);
        assert!(recovery.run_manifest.is_none());
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    }
}
