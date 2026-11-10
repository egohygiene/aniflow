use super::*;
use aniflow::upscayl_recovery::carrier::{self,CarrierRequest};
#[derive(Debug,Subcommand)]
pub(super) enum SpatialCarrierCommands {
    Inspect {#[arg(long)] request:PathBuf},Plan {#[arg(long)] request:PathBuf},
    Create {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    Status {#[arg(long)] directory:PathBuf,#[arg(long)] carrier_sha256:String},
}
impl SpatialCarrierCommands {pub(super) fn name(&self)->CommandName {match self {
    Self::Inspect {..}=>CommandName::VideoSpatialCarrierInspect,Self::Plan {..}=>CommandName::VideoSpatialCarrierPlan,
    Self::Create {..}=>CommandName::VideoSpatialCarrierCreate,Self::Status {..}=>CommandName::VideoSpatialCarrierStatus,
}}}
fn read(path:&Path)->Result<CarrierRequest> {CarrierRequest::from_json_slice(&read_bounded_configuration(path,"complete spatial carrier request",4_194_304)?)}
pub(super) fn dispatch(command:SpatialCarrierCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();let token=cli_cancellation_token()?;
    match command {
        SpatialCarrierCommands::Inspect {request}=>{let value=carrier::inspect(&read(&request)?,&token)?;print_result(name,presentation,&value,||println!("Accepted selected frames: {}; missing: {}; no work executed",value.frames.len(),value.missing.len()))},
        SpatialCarrierCommands::Plan {request}=>{let value=carrier::plan(&read(&request)?,&token)?;print_result(name,presentation,&value,||println!("Complete spatial selection: {} frames; source clock retained",value.frames.len()))},
        SpatialCarrierCommands::Create {request,output_directory}=>{let value=carrier::create(&read(&request)?,&absolute_delivery_path(output_directory)?,&token)?;print_result(name,presentation,&value,||println!("Pinned complete frame copy created; full-movie creative review and native assembly remain separate"))},
        SpatialCarrierCommands::Status {directory,carrier_sha256}=>{let value=carrier::observe(&absolute_delivery_path(directory)?,&carrier_sha256,&token)?;print_result(name,presentation,&value,||println!("Current complete spatial carrier: {} exact frames",value.selection.frames.len()))},
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copied_carrier_requires_fresh_output_and_readback_requires_separate_digest() {
        assert!(Cli::try_parse_from(["aniflow","video","spatial-carrier","create","--request","carrier.json"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","video","spatial-carrier","status","--directory","private/carrier"]).is_err());
        assert!(Cli::try_parse_from(["aniflow","video","spatial-carrier","inspect","--request","carrier.json"]).is_ok());
    }
}
