use super::*;
use aniflow::delivery_audio::final_qc::{self,FinalAudioQcRequest,resumable};
#[derive(Debug,Subcommand)]
pub(super) enum FinalAudioQcCommands {
    Resumable {#[command(subcommand)] command:ResumableCommands},
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Status {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
}
impl FinalAudioQcCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Resumable {command}=>command.name(),Self::Plan {..}=>CommandName::DeliveryAudioQcPlan,Self::Run {..}=>CommandName::DeliveryAudioQcRun,Self::Status {..}=>CommandName::DeliveryAudioQcStatus,
}}}
pub(super) fn dispatch(command:FinalAudioQcCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        FinalAudioQcCommands::Resumable {command}=>resumable_dispatch(command,presentation,&token),
        FinalAudioQcCommands::Plan {request}=>{let request=FinalAudioQcRequest::from_json_slice(&read_audio_configuration(&request,"final audio QC request")?)?;
            let value=final_qc::plan(&request,&token)?;print_result(name,presentation,&value,||println!("Exact final audio subject: {}; no measurement executed",value.artifact.sha256))},
        FinalAudioQcCommands::Run {request,output_directory}=>{let request=FinalAudioQcRequest::from_json_slice(&read_audio_configuration(&request,"final audio QC request")?)?;
            let value=final_qc::run(&request,&absolute_delivery_path(output_directory)?,&token)?;print_result(name,presentation,&value,||println!("Final-file audio observed without normalization; native meter qualification and musical review remain separate"))},
        FinalAudioQcCommands::Status {directory,receipt_sha256}=>{let value=final_qc::observe(&absolute_delivery_path(directory)?,&receipt_sha256,&token)?;
            print_result(name,presentation,&value,||println!("Current exact final audio/QC binding: {}",value.intent.artifact.sha256))},
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resumable_video_qc_requires_fresh_explicit_start_and_never_replaces_a_resume_request() {
        let args=["aniflow","delivery","audio-qc","resumable","run","--request","qc.json"];assert!(Cli::try_parse_from(args).is_err());assert!(Cli::try_parse_from(args.into_iter().chain(["--run-directory","private/qc"])).is_ok());
        for verb in ["resume","status"] {let args=["aniflow","delivery","audio-qc","resumable",verb,"--run-directory","private/qc"];assert!(Cli::try_parse_from(args).is_ok());assert!(Cli::try_parse_from(args.into_iter().chain(["--request","changed.json"])).is_err());}
    }
    #[test]
    fn qc_run_requires_fresh_output_and_status_requires_an_external_receipt_pin() {
        let args=["aniflow","delivery","audio-qc","run","--request","qc.json"];
        assert!(Cli::try_parse_from(args).is_err());assert!(Cli::try_parse_from(args.into_iter().chain(["--output-directory","private/qc"])).is_ok());
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-qc","status","--directory","private/qc"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-qc","run","--request","qc.json","--output-directory","private/qc","--normalize"]).is_err());
    }
}

#[derive(Debug,Subcommand)]
pub(super) enum ResumableCommands {
    Plan {#[arg(long)] request:PathBuf},Run {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Resume {#[arg(long)] run_directory:PathBuf},Status {#[arg(long)] run_directory:PathBuf},
}
impl ResumableCommands {fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryAudioQcResumablePlan,Self::Run {..}=>CommandName::DeliveryAudioQcResumableRun,
    Self::Resume {..}=>CommandName::DeliveryAudioQcResumableResume,Self::Status {..}=>CommandName::DeliveryAudioQcResumableStatus,
}}}
fn resumable_dispatch(command:ResumableCommands,presentation:Presentation,token:&aniflow::CancellationToken)->CommandResult<()> {
    let name=command.name();let read=|path:&Path|resumable::Request::from_json_slice(&read_audio_configuration(path,"resumable final-video audio QC request")?);
    let value=match command {
        ResumableCommands::Plan {request}=>{let value=resumable::plan(&read(&request)?,token)?;return print_result(name,presentation,&value,||println!("Exact resumable final-video QC intent: {}",value.intent_sha256));},
        ResumableCommands::Run {request,run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());resumable::run(&read(&request)?,&root,token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        ResumableCommands::Resume {run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());resumable::resume(&root,token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        ResumableCommands::Status {run_directory}=>resumable::status(&absolute_delivery_path(run_directory)?,token)?,
    };print_result(name,presentation,&value,||println!("Final-video audio QC phase: {:?}",value.phase))
}
