use super::*;
use aniflow::delivery_audio::final_lossless_qc::{self,FinalLosslessQcRequest};
#[derive(Debug,Subcommand)]
pub(super) enum FinalLosslessQcCommands {
    Plan {#[arg(long)] request:PathBuf},Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Status {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
}
impl FinalLosslessQcCommands {pub(super) fn name(&self)->CommandName {match self {Self::Plan {..}=>CommandName::DeliveryLosslessQcPlan,Self::Run {..}=>CommandName::DeliveryLosslessQcRun,Self::Status {..}=>CommandName::DeliveryLosslessQcStatus}}}
fn request(path:&Path)->Result<FinalLosslessQcRequest> {FinalLosslessQcRequest::from_json_slice(&read_audio_configuration(path,"final lossless audio QC request")?)}
pub(super) fn dispatch(command:FinalLosslessQcCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        FinalLosslessQcCommands::Plan {request:path}=>{let value=final_lossless_qc::plan(&request(&path)?,&token)?;
            print_result(name,presentation,&value,||println!("Exact final native audio subject: {}; no measurement executed",value.source.file.sha256))},
        FinalLosslessQcCommands::Run {request:path,output_directory}=>{let value=final_lossless_qc::run(&request(&path)?,&absolute_delivery_path(output_directory)?,&token)?;
            print_result(name,presentation,&value,||println!("Complete exact final audio measured without conversion or gain change; native meter remains unqualified"))},
        FinalLosslessQcCommands::Status {directory,receipt_sha256}=>{let value=final_lossless_qc::observe(&absolute_delivery_path(directory)?,&receipt_sha256,&token)?;
            print_result(name,presentation,&value,||println!("Current exact final audio/QC binding: {}",value.intent.source.file.sha256))},
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_lossless_qc_needs_a_fresh_output_or_an_external_readback_pin() {
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","run","--request","qc.json"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","status","--directory","private/qc"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","run","--request","qc.json","--output-directory","private/qc","--resample","48000"]).is_err());
    }
}
