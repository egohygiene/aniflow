//! Read-only recovery observations; native resume retains output-reuse authority.
use super::*;
use aniflow::upscayl_recovery::{self,inventory,progress};
#[derive(Debug,Clone,Args)]
pub(super) struct RecoveryArguments {
    #[arg(long)] source_video:PathBuf,
    #[arg(long)] frames_directory:PathBuf,
    #[arg(long)] frame_index:PathBuf,
    #[arg(long)] configuration:PathBuf,
    #[arg(long)] run_directory:PathBuf,
}
impl RecoveryArguments {
    fn request(&self)->Result<upscayl_recovery::UpscaylRecoveryRequest> {
        let settings=upscayl_recovery::UpscaylRecoveryConfiguration::from_json_slice(&read_audio_configuration(&self.configuration,"Upscayl recovery settings")?)?;
        let provider=std::env::current_exe().map_err(|_|Error::new(ErrorCategory::Dependency,"cannot identify native aniflow provider"))?;
        Ok(upscayl_recovery::UpscaylRecoveryRequest::new(absolute_delivery_path(self.source_video.clone())?,absolute_delivery_path(self.frames_directory.clone())?,absolute_delivery_path(self.frame_index.clone())?,settings,provider))
    }
}
#[derive(Debug,Subcommand)]
pub(super) enum RecoveryCommands {
    Inventory {#[command(flatten)] arguments:RecoveryArguments},
    Progress {#[command(flatten)] arguments:RecoveryArguments},
}
impl RecoveryCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Inventory {..}=>CommandName::VideoUpscaleRecoveryInventory,Self::Progress {..}=>CommandName::VideoUpscaleRecoveryProgress,
}}}
pub(super) fn dispatch(command:RecoveryCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        RecoveryCommands::Inventory {arguments}=>{let value=inventory::observe(&arguments.request()?,&absolute_delivery_path(arguments.run_directory)?,&token)?;
            print_result(name,presentation,&value,||println!("Recorded frames: {}/{} complete; {} pending; output reuse requires native resume",value.frames.complete,value.frames.total,value.frames.pending))},
        RecoveryCommands::Progress {arguments}=>{let value=progress::observe(&arguments.request()?,&absolute_delivery_path(arguments.run_directory)?,&token)?;
            print_result(name,presentation,&value,||{println!("Recorded frames: {}/{} complete; measured {} frames / {} provider milliseconds",value.inventory.frames.complete,value.inventory.frames.total,value.timing.measured_frames,value.timing.measured_provider_milliseconds);match value.timing.estimated_remaining_milliseconds{Some(ms)=>println!("Conditional remaining serial work: {ms} ms; excludes collector/validation/retries; no completion deadline"),None=>println!("Remaining-work estimate unavailable: no positive measured throughput")};})},
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn observations_require_exact_request_and_never_accept_execution_or_approval_flags(){let base=["aniflow","video","upscale-recovery","progress","--source-video","original.mp4","--frames-directory","frames","--frame-index","index.json","--configuration","settings.json","--run-directory","existing run"];
        assert_eq!(Cli::try_parse_from(base).unwrap().command.name(),CommandName::VideoUpscaleRecoveryProgress);
        for flag in ["--execute","--approve","--adopt","--resume"]{assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());}
        assert!(Cli::try_parse_from(["aniflow","video","upscale-recovery","progress","--run-directory","existing run"]).is_err());
    }
}
