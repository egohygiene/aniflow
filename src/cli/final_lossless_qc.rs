use super::*;
use aniflow::delivery_audio::final_lossless_qc::{self,FinalLosslessQcRequest,resumable::{self,ResumableQcRequest}};
#[derive(Debug,Subcommand)]
pub(super) enum FinalLosslessQcCommands {
    Resumable {#[command(subcommand)] command:ResumableQcCommands},
    Plan {#[arg(long)] request:PathBuf},Run {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Status {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
}
impl FinalLosslessQcCommands {pub(super) fn name(&self)->CommandName {match self {Self::Resumable {command}=>command.name(),Self::Plan {..}=>CommandName::DeliveryLosslessQcPlan,Self::Run {..}=>CommandName::DeliveryLosslessQcRun,Self::Status {..}=>CommandName::DeliveryLosslessQcStatus}}}
fn request(path:&Path)->Result<FinalLosslessQcRequest> {FinalLosslessQcRequest::from_json_slice(&read_audio_configuration(path,"final lossless audio QC request")?)}
pub(super) fn dispatch(command:FinalLosslessQcCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        FinalLosslessQcCommands::Resumable {command}=>dispatch_resumable(command,presentation,&token),
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
    fn resumable_qc_requires_explicit_start_and_keeps_resume_and_status_readback_scoped() {
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","resumable","run","--request","qc.json"]).is_err());
        for verb in ["resume","status"] {
            let args=["aniflow","delivery","lossless-qc","resumable",verb,"--run-directory","private/qc"];
            assert!(Cli::try_parse_from(args).is_ok());
            for extra in ["--request","--approve","--upload","--resample"] {assert!(Cli::try_parse_from(args.into_iter().chain([extra,"changed"])).is_err());}
        }
    }
    #[test]
    fn final_lossless_qc_needs_a_fresh_output_or_an_external_readback_pin() {
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","run","--request","qc.json"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","status","--directory","private/qc"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","delivery","lossless-qc","run","--request","qc.json","--output-directory","private/qc","--resample","48000"]).is_err());
    }
}

#[derive(Debug,Subcommand)]
pub(super) enum ResumableQcCommands {
    Plan {#[arg(long)] request:PathBuf},Run {#[arg(long)] request:PathBuf,#[arg(long)] run_directory:PathBuf},
    Resume {#[arg(long)] run_directory:PathBuf},Status {#[arg(long)] run_directory:PathBuf},
}
impl ResumableQcCommands {fn name(&self)->CommandName {match self {
    Self::Plan {..}=>CommandName::DeliveryLosslessQcResumablePlan,Self::Run {..}=>CommandName::DeliveryLosslessQcResumableRun,
    Self::Resume {..}=>CommandName::DeliveryLosslessQcResumableResume,Self::Status {..}=>CommandName::DeliveryLosslessQcResumableStatus,
}}}
fn dispatch_resumable(command:ResumableQcCommands,presentation:Presentation,token:&aniflow::CancellationToken)->CommandResult<()> {
    let name=command.name();
    let read=|path:&Path|ResumableQcRequest::from_json_slice(&read_audio_configuration(path,"resumable final-audio QC request")?);
    let value=match command {
        ResumableQcCommands::Plan {request}=>{let value=resumable::plan(&read(&request)?,token)?;
            return print_result(name,presentation,&value,||println!("Exact resumable final-audio QC intent: {}; no measurement executed",value.intent_sha256));},
        ResumableQcCommands::Run {request,run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            resumable::run(&read(&request)?,&root,token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        ResumableQcCommands::Resume {run_directory}=>{let root=absolute_delivery_path(run_directory)?;let mut recovery=Some(root.clone());
            resumable::resume(&root,token,|event|observe_pipeline_v3_progress(presentation,&mut recovery,event)).map_err(|e|CommandFailure::pipeline_v3(e,Some(root)))?},
        ResumableQcCommands::Status {run_directory}=>resumable::status(&absolute_delivery_path(run_directory)?,token)?,
    };
    print_result(name,presentation,&value,||println!("Final-audio QC: {:?}; next: {}",value.phase,value.next_action))
}
