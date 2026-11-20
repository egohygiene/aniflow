//! Read-only recovery observations; native resume retains output-reuse authority.
use super::*;
use aniflow::upscayl_recovery::{self, inventory, progress, resources};
#[derive(Debug, Clone, Args)]
pub(super) struct RecoveryInputs {
    #[arg(long)]
    source_video: PathBuf,
    #[arg(long)]
    frames_directory: PathBuf,
    #[arg(long)]
    frame_index: PathBuf,
    #[arg(long)]
    configuration: PathBuf,
}
#[derive(Debug, Clone, Args)]
pub(super) struct RecoveryArguments {
    #[command(flatten)]
    inputs: RecoveryInputs,
    #[arg(long)]
    run_directory: PathBuf,
}
impl RecoveryInputs {
    fn request(&self) -> Result<upscayl_recovery::UpscaylRecoveryRequest> {
        let settings = upscayl_recovery::UpscaylRecoveryConfiguration::from_json_slice(
            &read_audio_configuration(&self.configuration, "Upscayl recovery settings")?,
        )?;
        let provider = std::env::current_exe().map_err(|_| {
            Error::new(
                ErrorCategory::Dependency,
                "cannot identify native aniflow provider",
            )
        })?;
        Ok(upscayl_recovery::UpscaylRecoveryRequest::new(
            absolute_delivery_path(self.source_video.clone())?,
            absolute_delivery_path(self.frames_directory.clone())?,
            absolute_delivery_path(self.frame_index.clone())?,
            settings,
            provider,
        ))
    }
}
#[derive(Debug, Subcommand)]
pub(super) enum RecoveryCommands {
    /// Measure actual local PNG/components and explain the finite storage policy.
    Resources {
        #[command(flatten)]
        arguments: RecoveryInputs,
        /// Existing directory on the intended output filesystem. Never created.
        #[arg(long)]
        storage_parent: PathBuf,
        /// Optional retained workspace occupancy; metadata lengths do not grant reuse.
        #[arg(long)]
        retained_run_directory: Option<PathBuf>,
    },
    Inventory {
        #[command(flatten)]
        arguments: RecoveryArguments,
    },
    Progress {
        #[command(flatten)]
        arguments: RecoveryArguments,
    },
}
impl RecoveryCommands {
    pub(super) fn name(&self) -> CommandName {
        match self {
            Self::Resources { .. } => CommandName::VideoUpscaleRecoveryResources,
            Self::Inventory { .. } => CommandName::VideoUpscaleRecoveryInventory,
            Self::Progress { .. } => CommandName::VideoUpscaleRecoveryProgress,
        }
    }
}
pub(super) fn dispatch(command: RecoveryCommands, presentation: Presentation) -> CommandResult<()> {
    let name = command.name();
    let token = cli_cancellation_token()?;
    match command {
        RecoveryCommands::Resources {
            arguments,
            storage_parent,
            retained_run_directory,
        } => {
            let mut value = resources::observe(
                &arguments.request()?,
                &absolute_delivery_path(storage_parent)?,
                &token,
            )?;
            if let Some(root) = retained_run_directory {
                value.retained_workspace = Some(resources::measure_retained_workspace(
                    &absolute_delivery_path(root)?,
                    &token,
                )?);
            }
            print_result(name, presentation, &value, || {
                println!(
                    "Measured {} input PNGs: {} bytes; dependencies: {} bytes",
                    value.input_pngs.frame_count,
                    value.input_pngs.total_bytes,
                    value.disk.external_original_component_bytes
                );
                println!(
                    "Fresh-run additional bound: {} bytes; working bound: {} bytes / {} bytes; policy fits: {}",
                    value.disk.additional_disk_bound_bytes,
                    value.disk.working_bound_bytes,
                    value.disk.maximum_working_bytes,
                    value.disk.fits_working_policy
                );
                println!(
                    "Free space observed: {:?} bytes; required including margin: {} bytes; storage fits: {:?}",
                    value.disk.available_bytes_observed,
                    value.disk.required_free_including_margin_bytes,
                    value.disk.fits_available_storage
                );
                println!(
                    "Native {}x{} geometry: {} RGB24 bytes per image; measured device/RSS unavailable; no native qualification or output reuse",
                    value.geometry.native_width,
                    value.geometry.native_height,
                    value.geometry.native_rgb24_bytes_per_image
                );
                if let Some(retained) = &value.retained_workspace {
                    println!(
                        "Retained workspace: {} logical bytes; private scratch: {} logical bytes; no lifetime retry bound",
                        retained.logical_file_bytes, retained.private_scratch_logical_file_bytes
                    );
                }
            })
        }
        RecoveryCommands::Inventory { arguments } => {
            let value = inventory::observe(
                &arguments.inputs.request()?,
                &absolute_delivery_path(arguments.run_directory)?,
                &token,
            )?;
            print_result(name, presentation, &value, || {
                println!(
                    "Recorded frames: {}/{} complete; {} pending; output reuse requires native resume",
                    value.frames.complete, value.frames.total, value.frames.pending
                )
            })
        }
        RecoveryCommands::Progress { arguments } => {
            let value = progress::observe(
                &arguments.inputs.request()?,
                &absolute_delivery_path(arguments.run_directory)?,
                &token,
            )?;
            print_result(name, presentation, &value, || {
                println!(
                    "Recorded frames: {}/{} complete; measured {} frames / {} provider milliseconds",
                    value.inventory.frames.complete,
                    value.inventory.frames.total,
                    value.timing.measured_frames,
                    value.timing.measured_provider_milliseconds
                );
                match value.timing.estimated_remaining_milliseconds {
                    Some(ms) => println!(
                        "Conditional remaining serial work: {ms} ms; excludes collector/validation/retries; no completion deadline"
                    ),
                    None => println!(
                        "Remaining-work estimate unavailable: no positive measured throughput"
                    ),
                };
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resources_require_an_existing_storage_locator_and_never_accept_execution_flags() {
        let base = [
            "aniflow",
            "video",
            "upscale-recovery",
            "resources",
            "--source-video",
            "original.mp4",
            "--frames-directory",
            "frames",
            "--frame-index",
            "index.json",
            "--configuration",
            "settings.json",
            "--storage-parent",
            "output filesystem",
        ];
        assert_eq!(
            Cli::try_parse_from(base).unwrap().command.name(),
            CommandName::VideoUpscaleRecoveryResources
        );
        for flag in ["--execute", "--approve", "--resume"] {
            assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());
        }
    }
    #[test]
    fn observations_require_exact_request_and_never_accept_execution_or_approval_flags() {
        let base = [
            "aniflow",
            "video",
            "upscale-recovery",
            "progress",
            "--source-video",
            "original.mp4",
            "--frames-directory",
            "frames",
            "--frame-index",
            "index.json",
            "--configuration",
            "settings.json",
            "--run-directory",
            "existing run",
        ];
        assert_eq!(
            Cli::try_parse_from(base).unwrap().command.name(),
            CommandName::VideoUpscaleRecoveryProgress
        );
        for flag in ["--execute", "--approve", "--adopt", "--resume"] {
            assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());
        }
        assert!(
            Cli::try_parse_from([
                "aniflow",
                "video",
                "upscale-recovery",
                "progress",
                "--run-directory",
                "existing run"
            ])
            .is_err()
        );
    }
}
