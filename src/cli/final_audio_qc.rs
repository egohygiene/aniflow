use super::*;
use aniflow::delivery_audio::final_qc::{self,FinalAudioQcRequest};
#[derive(Debug,Subcommand)]
pub(super) enum FinalAudioQcCommands {
    Plan {#[arg(long)] request:PathBuf},
    Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Status {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
}
impl FinalAudioQcCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryAudioQcPlan,Self::Run {..}=>CommandName::DeliveryAudioQcRun,Self::Status {..}=>CommandName::DeliveryAudioQcStatus,
}}}
pub(super) fn dispatch(command:FinalAudioQcCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
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
    fn qc_run_requires_fresh_output_and_status_requires_an_external_receipt_pin() {
        let args=["aniflow","delivery","audio-qc","run","--request","qc.json"];
        assert!(Cli::try_parse_from(args).is_err());assert!(Cli::try_parse_from(args.into_iter().chain(["--output-directory","private/qc"])).is_ok());
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-qc","status","--directory","private/qc"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","audio-qc","run","--request","qc.json","--output-directory","private/qc","--normalize"]).is_err());
    }
}
