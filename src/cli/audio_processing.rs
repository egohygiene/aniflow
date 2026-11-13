use super::*;
use aniflow::delivery_audio::processing;

#[derive(Debug,Subcommand)]
pub(super) enum AudioProcessingCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Resume {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Status {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
}
impl AudioProcessingCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryAudioProcessingPlan,Self::Run {..}=>CommandName::DeliveryAudioProcessingRun,
    Self::Resume {..}=>CommandName::DeliveryAudioProcessingResume,Self::Status {..}=>CommandName::DeliveryAudioProcessingStatus,
}}}
pub(super) fn dispatch(command:AudioProcessingCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let read=|path:&Path|processing::Request::from_json_slice(&read_audio_configuration(path,"explicit audio-processing request")?);
    match command {
        AudioProcessingCommands::Plan {request}=>{let value=processing::plan(&read(&request)?,&token)?;print_result(name,presentation,&value,||println!("Selected native audio-processing profile: {}",value.payload.profile))},
        AudioProcessingCommands::Run {request,output_directory}=>{
            let request=read(&request)?;let parent=absolute_delivery_path(output_directory)?;let mut recovery=None;
            let value=processing::run(&request,&parent,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&value,||println!("Native handoff retained; qualification and human review remain separate"))
        },
        AudioProcessingCommands::Resume {request,run_directory}=>{
            let request=read(&request)?;let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            let value=processing::resume(&request,&root,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&value,||println!("Explicit native recovery completed; no publication authority"))
        },
        AudioProcessingCommands::Status {request,run_directory}=>{
            let value=processing::handoff(&read(&request)?,&absolute_delivery_path(run_directory)?,&token)?;
            print_result(name,presentation,&value,||println!("Current accepted native handoff; {}",value.payload.output_state))
        },
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn processing_recovery_requires_the_exact_request_and_never_accepts_automatic_effects() {
        let args=["aniflow","delivery","audio-processing","resume","--run-directory","retained 曲"];
        assert!(Cli::try_parse_from(args).is_err());
        let complete=args.into_iter().chain(["--request","selected.json"]).collect::<Vec<_>>();
        assert_eq!(Cli::try_parse_from(&complete).unwrap().command.name(),CommandName::DeliveryAudioProcessingResume);
        for flag in ["--normalize","--limiter","--replace-source","--upload"] {assert!(Cli::try_parse_from(complete.iter().copied().chain([flag])).is_err());}
    }
}
