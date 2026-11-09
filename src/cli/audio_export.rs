//! Thin delivery over the fixed native lossless export stage.
use super::*;
use aniflow::lossless_audio::{self,LosslessConfiguration,LosslessRequest};
#[derive(Debug,Clone,Args)]
pub(super) struct AudioExportArguments {
    #[arg(long)] source:PathBuf,
    #[arg(long)] signal_run_directory:PathBuf,
    #[arg(long)] configuration:PathBuf,
}
impl AudioExportArguments {
    fn request(self)->Result<LosslessRequest> {
        let configuration=LosslessConfiguration::from_json_slice(&read_audio_configuration(&self.configuration,"lossless audio configuration")?)?;
        Ok(LosslessRequest {source:absolute_delivery_path(self.source)?,signal_run_directory:absolute_delivery_path(self.signal_run_directory)?,configuration,
            provider_executable:std::env::current_exe().map_err(|_|Error::new(ErrorCategory::Dependency,"cannot identify the native aniflow provider executable"))?})
    }
}
#[derive(Debug,Subcommand)]
pub(super) enum AudioExportCommands {
    Plan {#[command(flatten)] arguments:AudioExportArguments},
    Run {#[command(flatten)] arguments:AudioExportArguments,#[arg(long)] output_directory:PathBuf},
    Resume {#[command(flatten)] arguments:AudioExportArguments,#[arg(long)] run_directory:PathBuf},
    /// Read exact current accepted completion; inspect unfinished runs with status-v3.
    Status {#[arg(long)] run_directory:PathBuf},
}
impl AudioExportCommands {
    pub(super) fn name(&self)->CommandName {match self {Self::Plan {..}=>CommandName::DeliveryAudioExportPlan,Self::Run {..}=>CommandName::DeliveryAudioExportRun,Self::Resume {..}=>CommandName::DeliveryAudioExportResume,Self::Status {..}=>CommandName::DeliveryAudioExportStatus}}
}
pub(super) fn dispatch(command:AudioExportCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();match command {
        AudioExportCommands::Plan {arguments}=>{let result=lossless_audio::plan(&arguments.request()?,&cli_cancellation_token()?)?;print_result(name,presentation,&result,||print_pipeline_v3_plan(&result))},
        AudioExportCommands::Run {arguments,output_directory}=>{
            let request=arguments.request()?;let parent=absolute_delivery_path(output_directory)?;let token=cli_cancellation_token()?;let mut recovery=None;
            let progress=|event:&PipelineV3RunProgress|observe_pipeline_v3_progress(presentation,&mut recovery,event);
            let result=lossless_audio::run(request,&parent,&token,progress).map_err(|error|CommandFailure::pipeline_v3(error,recovery))?;
            print_result(name,presentation,&result,||print_pipeline_v3_outcome(&result))
        },
        AudioExportCommands::Resume {arguments,run_directory}=>{
            let root=absolute_delivery_path(run_directory)?;let request=arguments.request().map_err(|error|CommandFailure::pipeline_v3(error,Some(root.clone())))?;
            let token=cli_cancellation_token().map_err(|error|CommandFailure::pipeline_v3(error,Some(root.clone())))?;let mut recovery=Some(root.clone());
            let progress=|event:&PipelineV3RunProgress|observe_pipeline_v3_progress(presentation,&mut recovery,event);
            let result=lossless_audio::resume(request,&root,&token,progress).map_err(|error|CommandFailure::pipeline_v3(error,recovery))?;
            print_result(name,presentation,&result,||print_pipeline_v3_outcome(&result))
        },
        AudioExportCommands::Status {run_directory}=>{
            let result=lossless_audio::observe(&absolute_delivery_path(run_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Lossless {:?} candidate: {} bytes; exact original PCM binding retained; creative review remains separate",result.report.operation.configuration.format,result.file.byte_count))
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_native_source_evidence_and_separate_output_are_required() {
        let base=["aniflow","delivery","audio-export","run","--source","mix 音.wav","--signal-run-directory","signal root","--configuration","lossless.json"];
        assert!(Cli::try_parse_from(base).is_err());
        assert_eq!(Cli::try_parse_from(base.into_iter().chain(["--output-directory","new candidates"])).unwrap().command.name(),CommandName::DeliveryAudioExportRun);
        for flag in ["--normalize","--resample","--quantize","--downmix","--replace-source","--download","--post"] {assert!(Cli::try_parse_from(base.into_iter().chain(["--output-directory","new candidates",flag])).is_err());}
    }
    #[test]
    fn refused_resume_keeps_named_root_without_fabricated_native_authority() {
        let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();
        let args=AudioExportArguments {source:root.join("source.wav"),signal_run_directory:root.join("signal"),configuration:root.join("missing.json")};
        let failure=dispatch(AudioExportCommands::Resume {arguments:args,run_directory:root.clone()},Presentation::Machine).unwrap_err();
        assert_eq!(failure.pipeline_v3_recovery.unwrap().run_directory,root);assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
    }
}
