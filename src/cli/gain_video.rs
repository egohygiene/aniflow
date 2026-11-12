use super::*;
use aniflow::delivery_audio::gain_video;
#[derive(Debug,Subcommand)]
pub(super) enum GainVideoCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Resume {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Status {#[arg(long)] run_directory:PathBuf},
}
impl GainVideoCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryGainVideoPlan,Self::Run {..}=>CommandName::DeliveryGainVideoRun,
    Self::Resume {..}=>CommandName::DeliveryGainVideoResume,Self::Status {..}=>CommandName::DeliveryGainVideoStatus,
}}}
pub(super) fn dispatch(command:GainVideoCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    let read=|path:&Path|gain_video::Request::from_json_slice(&read_audio_configuration(path,"selected gain video request")?);
    match command {
        GainVideoCommands::Plan {request}=>{let value=gain_video::plan(&read(&request)?,&token)?;print_result(name,presentation,&value,||print_pipeline_v3_plan(&value))},
        GainVideoCommands::Run {request,output_directory}=>{let request=read(&request)?;let parent=absolute_delivery_path(output_directory)?;let mut recovery=None;
            let value=gain_video::run(&request,&parent,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&value,||print_pipeline_v3_outcome(&value))},
        GainVideoCommands::Resume {request,run_directory}=>{let request=read(&request)?;let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            let value=gain_video::resume(&request,&root,&token,|e|observe_pipeline_v3_progress(presentation,&mut recovery,e)).map_err(|e|CommandFailure::pipeline_v3(e,recovery))?;
            print_result(name,presentation,&value,||print_pipeline_v3_outcome(&value))},
        GainVideoCommands::Status {run_directory}=>{let value=gain_video::completion(&absolute_delivery_path(run_directory)?,&token)?;print_result(name,presentation,&value,||println!("Gain video candidate retained; exact encoded-file QC and human review remain required"))},
    }
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn encoded_candidate_has_an_explicit_separate_output_and_no_trimming_or_upload_flags() {
        let args=["aniflow","delivery","gain-video","run","--request","video.json"];
        assert!(Cli::try_parse_from(args).is_err());assert!(Cli::try_parse_from(args.into_iter().chain(["--output-directory","new video"])).is_ok());
        for flag in ["--shortest","--trim","--resample","--replace-source","--upload"] {assert!(Cli::try_parse_from(args.into_iter().chain(["--output-directory","new video",flag])).is_err());}
    }
}
