//! Thin delivery over accepted-media destination preparation.
use super::*;
use aniflow::destination;
#[derive(Debug,Subcommand)]
pub(super) enum DeliveryCommands {
    /// Produce one explicitly selected native lossless audio candidate.
    AudioExport {#[command(subcommand)] command:audio_export::AudioExportCommands},
    /// Read current accepted facts against explicit dated supplied requirements.
    Preflight {#[arg(long)] request:PathBuf},
    /// Inspect an exact local package subject/context before supplying a decision.
    PostingPlan {#[arg(long)] request:PathBuf},
    /// Copy one selected file and explicit public metadata to a fresh local root.
    Package {#[arg(long)] request:PathBuf,#[arg(long)] declaration:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Recheck exact local package membership, declaration and current producer.
    PackageStatus {#[arg(long)] directory:PathBuf},
    /// Resolve exact selected files and their private retained origin evidence.
    EvidencePlan {#[arg(long)] request:PathBuf},
    /// Retain selected bytes in a fresh local root, without timestamp networking.
    EvidenceSeal {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Read-only archival byte check against a separately supplied known digest.
    EvidenceVerify {#[arg(long)] directory:PathBuf,#[arg(long)] manifest_sha256:String},
    /// Append a separate local observation after an exact archival byte check.
    EvidenceRecordVerification {#[arg(long)] directory:PathBuf,#[arg(long)] manifest_sha256:String},
}
impl DeliveryCommands {
    pub(super) fn name(&self)->CommandName {match self {
        Self::AudioExport {command}=>command.name(),
        Self::Preflight {..}=>CommandName::DeliveryPreflight,Self::PostingPlan {..}=>CommandName::DeliveryPostingPlan,
        Self::Package {..}=>CommandName::DeliveryPackage,Self::PackageStatus {..}=>CommandName::DeliveryPackageStatus,
        Self::EvidencePlan {..}=>CommandName::DeliveryEvidencePlan,Self::EvidenceSeal {..}=>CommandName::DeliveryEvidenceSeal,
        Self::EvidenceVerify {..}=>CommandName::DeliveryEvidenceVerify,Self::EvidenceRecordVerification {..}=>CommandName::DeliveryEvidenceRecordVerification,
    }}
}
pub(super) fn dispatch(command:DeliveryCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();match command {
        DeliveryCommands::AudioExport {command}=>audio_export::dispatch(command,presentation),
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
        DeliveryCommands::EvidencePlan {request}=>{
            let result=aniflow::release_evidence::plan(&evidence_request(&request)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,|| {
                println!("Local retention plan: {} subjects, {} bytes with evidence headroom",result.manifest.subjects.len(),result.projected_bundle_bytes);
                println!("  manifest_sha256: {}",result.manifest_sha256);println!("No signing, timestamp proof or publication authority");
            })
        },
        DeliveryCommands::EvidenceSeal {request,output_directory}=>{
            let result=aniflow::release_evidence::seal(&evidence_request(&request)?,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,|| {
                println!("Retained {} exact subjects ({} bytes)",result.subject_count,result.subject_bytes);
                println!("  manifest_sha256: {}",result.subject_manifest_sha256);println!("Keep this digest separately; timestamp state: not requested");
            })
        },
        DeliveryCommands::EvidenceVerify {directory,manifest_sha256}=>{
            let result=aniflow::release_evidence::verify(&absolute_delivery_path(directory)?,&manifest_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("{}: {} subjects; timestamp: not verified; producer acceptance: not reobserved",result.state,result.subject_count))
        },
        DeliveryCommands::EvidenceRecordVerification {directory,manifest_sha256}=>{
            let result=aniflow::release_evidence::record_verification(&absolute_delivery_path(directory)?,&manifest_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Local byte-check observation {} appended; wall clock is not a trusted timestamp",result.sequence))
        },
    }
}
fn posting_request(path:&std::path::Path)->Result<destination::posting::PostingRequest> {
    destination::posting::PostingRequest::from_json_slice(&read_bounded_configuration(path,"posting request",2_097_152)?)
}
fn evidence_request(path:&std::path::Path)->Result<aniflow::release_evidence::EvidenceRequest> {
    aniflow::release_evidence::EvidenceRequest::from_json_slice(&read_bounded_configuration(path,"release evidence request",2_097_152)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archival_checks_require_a_separately_supplied_digest_and_never_stamp_or_post() {
        let base=["aniflow","delivery","evidence-verify","--directory","retained evidence"];
        assert!(Cli::try_parse_from(base).is_err());
        let parsed=Cli::try_parse_from(base.into_iter().chain(["--manifest-sha256","known-digest"])).unwrap();assert_eq!(parsed.command.name(),CommandName::DeliveryEvidenceVerify);
        for flag in ["--stamp","--sign","--post","--repair","--adopt"] {assert!(Cli::try_parse_from(base.into_iter().chain(["--manifest-sha256","known-digest",flag])).is_err());}
        assert_eq!(Cli::try_parse_from(["aniflow","delivery","evidence-plan","--request","selected.json"]).unwrap().command.name(),CommandName::DeliveryEvidencePlan);
        assert_eq!(Cli::try_parse_from(["aniflow","delivery","evidence-seal","--request","selected.json","--output-directory","fresh"]).unwrap().command.name(),CommandName::DeliveryEvidenceSeal);
        assert_eq!(Cli::try_parse_from(["aniflow","delivery","evidence-record-verification","--directory","retained","--manifest-sha256","known"]).unwrap().command.name(),CommandName::DeliveryEvidenceRecordVerification);
    }
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
