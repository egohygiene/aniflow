//! Thin delivery over accepted-media destination preparation.
use super::*;
use aniflow::destination;
#[derive(Debug,Subcommand)]
pub(super) enum DeliveryCommands {
    /// Read current accepted facts against explicit dated supplied requirements.
    Preflight {#[arg(long)] request:PathBuf},
    /// Inspect an exact local package subject/context before supplying a decision.
    PostingPlan {#[arg(long)] request:PathBuf},
    /// Copy one selected file and explicit public metadata to a fresh local root.
    Package {#[arg(long)] request:PathBuf,#[arg(long)] declaration:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Recheck exact local package membership, declaration and current producer.
    PackageStatus {#[arg(long)] directory:PathBuf},
}
impl DeliveryCommands {
    pub(super) fn name(&self)->CommandName {match self {
        Self::Preflight {..}=>CommandName::DeliveryPreflight,Self::PostingPlan {..}=>CommandName::DeliveryPostingPlan,
        Self::Package {..}=>CommandName::DeliveryPackage,Self::PackageStatus {..}=>CommandName::DeliveryPackageStatus,
    }}
}
pub(super) fn dispatch(command:DeliveryCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();match command {
        DeliveryCommands::Preflight {request}=>{
            let request=destination::PreflightRequest::from_json_slice(&read_audio_configuration(&request,"destination preflight request")?)?;
            let result=destination::preflight(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,|| {
                println!("Destination preflight: {:?}",result.payload.disposition);
                for check in &result.payload.checks {println!("  {}: {:?}; {}",check.id,check.state,check.reason);}
                println!("Supplied requirement evidence; human review and publication remain separate");
            })
        },
        DeliveryCommands::PostingPlan {request}=>{
            let request=posting_request(&request)?;let result=destination::posting::plan(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,|| {
                println!("Local posting plan: {} ({} bytes with evidence headroom)",result.media_filename,result.projected_package_bytes);
                println!("  context_sha256: {}",result.review_context_sha256);println!("  subject_sha256: {}",result.review_subject_sha256);
                println!("  metadata scope: {}",result.metadata_scope);
            })
        },
        DeliveryCommands::Package {request,declaration,output_directory}=>{
            let request=posting_request(&request)?;let declaration=read_delivery_document(&declaration,"posting review declaration")?;
            let result=destination::posting::create(&request,&declaration,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Local package {} created with {} exact members; manual posting remains separate",result.package_id,result.members.len()))
        },
        DeliveryCommands::PackageStatus {directory}=>{
            let result=destination::posting::verify(&absolute_delivery_path(directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Current package bindings verified for {}; publication authority: none",result.package_id))
        },
    }
}
fn posting_request(path:&std::path::Path)->Result<destination::posting::PostingRequest> {
    destination::posting::PostingRequest::from_json_slice(&read_bounded_configuration(path,"posting request",2_097_152)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_is_explicit_bounded_input_without_transform_or_post_flags() {
        let base=["aniflow","--output","json","delivery","preflight","--request","request 音.json"];
        assert_eq!(Cli::try_parse_from(base).unwrap().command.name(),CommandName::DeliveryPreflight);
        for flag in ["--post","--normalize","--trim","--resize","--download","--approve"] {assert!(Cli::try_parse_from(base.into_iter().chain([flag])).is_err());}
        assert!(Cli::try_parse_from(["aniflow","delivery","preflight"]).is_err());
    }
    #[test]
    fn posting_copy_requires_declaration_and_has_no_upload_or_implicit_approval() {
        let base=["aniflow","delivery","package","--request","posting.json","--output-directory","new package"];
        assert!(Cli::try_parse_from(base).is_err());
        let parsed=Cli::try_parse_from(base.into_iter().chain(["--declaration","decision.json"])).unwrap();assert_eq!(parsed.command.name(),CommandName::DeliveryPackage);
        for flag in ["--upload","--post","--approve","--replace-source"] {assert!(Cli::try_parse_from(base.into_iter().chain(["--declaration","decision.json",flag])).is_err());}
        assert_eq!(Cli::try_parse_from(["aniflow","delivery","posting-plan","--request","posting.json"]).unwrap().command.name(),CommandName::DeliveryPostingPlan);
        assert_eq!(Cli::try_parse_from(["aniflow","delivery","package-status","--directory","package"]).unwrap().command.name(),CommandName::DeliveryPackageStatus);
    }
}
