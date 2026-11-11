//! Thin delivery over accepted-media destination preparation.
use super::*;
use aniflow::destination;
#[derive(Debug,Subcommand)]
pub(super) enum DeliveryCommands {
    /// Retain opaque supplied proof candidates; no timestamp networking or verification.
    ProofRetain {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    ProofInspect {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
    ProofRestorePlan {#[arg(long)] request:PathBuf},
    ProofRestore {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Parse exact retained OTS commitments; trusted time remains unverified.
    ProofInspectOts {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
    /// Restore only a supported proof matching the copied subject's SHA-256.
    ProofRestoreBound {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Fixed audio export/QC/preflight/supplied-review/manual-package continuation.
    AudioPublishing {#[command(subcommand)] command:audio_publishing::AudioPublishingCommands},
    /// Observe exact final native WAV/FLAC samples with separate unqualified meter evidence.
    LosslessQc {#[command(subcommand)] command:final_lossless_qc::FinalLosslessQcCommands},
    /// Fixed local export/QC/preflight/review/manual-package continuation.
    Publishing {#[command(subcommand)] command:publishing::PublishingCommands},
    /// Observe complete audio decoded from one exact current final video.
    AudioQc {#[command(subcommand)] command:final_audio_qc::FinalAudioQcCommands},
    /// Plan one explicitly selected destination-video derivative.
    VideoExport {#[command(subcommand)] command:video_export::VideoExportCommands},
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
    /// Retain an explicitly supplied platform-copy candidate without fetching or posting.
    PostUploadRecord {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Read retained observation bytes against an externally supplied receipt digest.
    PostUploadInspect {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
    /// Resolve exact selected files and their private retained origin evidence.
    EvidencePlan {#[arg(long)] request:PathBuf},
    /// Retain selected bytes in a fresh local root, without timestamp networking.
    EvidenceSeal {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Read-only archival byte check against a separately supplied known digest.
    EvidenceVerify {#[arg(long)] directory:PathBuf,#[arg(long)] manifest_sha256:String},
    /// Append a separate local observation after an exact archival byte check.
    EvidenceRecordVerification {#[arg(long)] directory:PathBuf,#[arg(long)] manifest_sha256:String},
    /// Select explicitly public archival subjects and optional matched proof observations.
    EvidenceHandoffPlan {#[arg(long)] request:PathBuf},
    EvidenceHandoff {#[arg(long)] request:PathBuf,#[arg(long)] output_directory:PathBuf},
    /// Reobserve private archive/proofs and exact local packet bytes.
    EvidenceHandoffStatus {#[arg(long)] directory:PathBuf,#[arg(long)] receipt_sha256:String},
    /// Read portable pinned metadata without private sources or authenticity claims.
    EvidenceHandoffInspect {#[arg(long)] file:PathBuf,#[arg(long)] handoff_sha256:String},
}
impl DeliveryCommands {
    pub(super) fn name(&self)->CommandName {match self {
        Self::EvidenceHandoffPlan {..}=>CommandName::DeliveryEvidenceHandoffPlan,
        Self::EvidenceHandoff {..}=>CommandName::DeliveryEvidenceHandoff,
        Self::EvidenceHandoffStatus {..}=>CommandName::DeliveryEvidenceHandoffStatus,
        Self::EvidenceHandoffInspect {..}=>CommandName::DeliveryEvidenceHandoffInspect,
        Self::ProofRetain {..}=>CommandName::DeliveryProofRetain,Self::ProofInspect {..}=>CommandName::DeliveryProofInspect,
        Self::ProofRestorePlan {..}=>CommandName::DeliveryProofRestorePlan,Self::ProofRestore {..}=>CommandName::DeliveryProofRestore,
        Self::ProofInspectOts {..}=>CommandName::DeliveryProofInspectOts,Self::ProofRestoreBound {..}=>CommandName::DeliveryProofRestoreBound,
        Self::Publishing {command}=>command.name(),
        Self::AudioPublishing {command}=>command.name(),
        Self::AudioQc {command}=>command.name(),
        Self::LosslessQc {command}=>command.name(),
        Self::VideoExport {command}=>command.name(),
        Self::AudioExport {command}=>command.name(),
        Self::Preflight {..}=>CommandName::DeliveryPreflight,Self::PostingPlan {..}=>CommandName::DeliveryPostingPlan,
        Self::Package {..}=>CommandName::DeliveryPackage,Self::PackageStatus {..}=>CommandName::DeliveryPackageStatus,
        Self::PostUploadRecord {..}=>CommandName::DeliveryPostUploadRecord,Self::PostUploadInspect {..}=>CommandName::DeliveryPostUploadInspect,
        Self::EvidencePlan {..}=>CommandName::DeliveryEvidencePlan,Self::EvidenceSeal {..}=>CommandName::DeliveryEvidenceSeal,
        Self::EvidenceVerify {..}=>CommandName::DeliveryEvidenceVerify,Self::EvidenceRecordVerification {..}=>CommandName::DeliveryEvidenceRecordVerification,
    }}
}
pub(super) fn dispatch(command:DeliveryCommands,presentation:Presentation)->CommandResult<()> {
    let name=command.name();match command {
        DeliveryCommands::EvidenceHandoffPlan {request}=>{
            let request=aniflow::release_evidence::handoff::HandoffRequest::from_json_slice(&read_bounded_configuration(&request,"public evidence handoff request",131_072)?)?;
            let result=aniflow::release_evidence::handoff::plan(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("{} selected public subjects; {} handoff bytes",result.public_handoff.subjects.len(),result.public_handoff_byte_count))
        },
        DeliveryCommands::EvidenceHandoff {request,output_directory}=>{
            let request=aniflow::release_evidence::handoff::HandoffRequest::from_json_slice(&read_bounded_configuration(&request,"public evidence handoff request",131_072)?)?;
            let result=aniflow::release_evidence::handoff::create(&request,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Prepared {} public subject records; no posting or trusted timestamp",result.receipt.subject_count))
        },
        DeliveryCommands::EvidenceHandoffStatus {directory,receipt_sha256}=>{
            let result=aniflow::release_evidence::handoff::observe(&absolute_delivery_path(directory)?,&receipt_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Current archival handoff: {} selected subjects; producer acceptance not reobserved",result.receipt.subject_count))
        },
        DeliveryCommands::EvidenceHandoffInspect {file,handoff_sha256}=>{
            let bytes=read_bounded_configuration(&file,"portable public evidence handoff",2_097_152)?;
            let result=aniflow::release_evidence::handoff::PublicHandoff::from_pinned_json(&bytes,&handoff_sha256)?;
            print_result(name,presentation,&result,||println!("Portable handoff bytes/schema match: {} subjects; authenticity and current sources not assessed",result.subjects.len()))
        },
        DeliveryCommands::ProofRetain {request,output_directory}=>{
            let request=aniflow::release_evidence::detached::ProofRetentionRequest::from_json_slice(&read_bounded_configuration(&request,"detached proof retention request",2_097_152)?)?;
            let result=aniflow::release_evidence::detached::retain(&request,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Retained opaque proof candidate {}; timestamp not verified; no ownership claim",result.sequence))
        },
        DeliveryCommands::ProofInspect {directory,receipt_sha256}=>{
            let result=aniflow::release_evidence::detached::inspect(&absolute_delivery_path(directory)?,&receipt_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Retained pair bytes match supplied receipt; proof remains unparsed and timestamp unverified (candidate {})",result.sequence))
        },
        DeliveryCommands::ProofRestorePlan {request}=>{
            let request=aniflow::release_evidence::detached::restoration::RestoreRequest::from_json_slice(&read_bounded_configuration(&request,"detached proof restore request",65_536)?)?;
            let result=aniflow::release_evidence::detached::restoration::plan(&request,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Exact pair restoration: {} files / {} bytes; cryptographic verification is separate",result.files.len(),result.copy_bytes))
        },
        DeliveryCommands::ProofRestore {request,output_directory}=>{
            let request=aniflow::release_evidence::detached::restoration::RestoreRequest::from_json_slice(&read_bounded_configuration(&request,"detached proof restore request",65_536)?)?;
            let result=aniflow::release_evidence::detached::restoration::restore(&request,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Restored {} exact retained files; timestamp remains not verified",result.copied_files))
        },
        DeliveryCommands::ProofInspectOts {directory,receipt_sha256}=>{
            let result=aniflow::release_evidence::detached::inspection::inspect(&absolute_delivery_path(directory)?,&receipt_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Retained OTS subject binding: {:?}; timestamp not verified",result.proof.subject_binding))
        },
        DeliveryCommands::ProofRestoreBound {request,output_directory}=>{
            let request=aniflow::release_evidence::detached::restoration::RestoreRequest::from_json_slice(&read_bounded_configuration(&request,"detached proof restore request",65_536)?)?;
            let result=aniflow::release_evidence::detached::inspection::restore_bound(&request,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Restored exact pair with matching OTS subject commitments; timestamp not verified"))
        },
        DeliveryCommands::Publishing {command}=>publishing::dispatch(command,presentation),
        DeliveryCommands::AudioPublishing {command}=>audio_publishing::dispatch(command,presentation),
        DeliveryCommands::AudioQc {command}=>final_audio_qc::dispatch(command,presentation),
        DeliveryCommands::LosslessQc {command}=>final_lossless_qc::dispatch(command,presentation),
        DeliveryCommands::VideoExport {command}=>video_export::dispatch(command,presentation),
        DeliveryCommands::PostUploadRecord {request,output_directory}=>{
            let request=destination::post_upload::ObservationRequest::from_json_slice(&read_audio_configuration(&request,"post-upload observation request")?)?;
            let result=destination::post_upload::retain(&request,&absolute_delivery_path(output_directory)?,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Supplied-copy observation retained: {:?}; platform authentication and technical validation not established",result.byte_relationship))
        },
        DeliveryCommands::PostUploadInspect {directory,receipt_sha256}=>{
            let result=destination::post_upload::inspect(&absolute_delivery_path(directory)?,&receipt_sha256,&cli_cancellation_token()?)?;
            print_result(name,presentation,&result,||println!("Retained observed bytes match their receipt: {:?}; declarations remain unverified",result.byte_relationship))
        },
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
    fn evidence_handoff_requires_selection_and_pins_and_offers_no_publication_switch() {
        for (verb,args,name) in [
            ("evidence-handoff-plan",vec!["--request","disclosure.json"],CommandName::DeliveryEvidenceHandoffPlan),
            ("evidence-handoff",vec!["--request","disclosure.json","--output-directory","new packet"],CommandName::DeliveryEvidenceHandoff),
            ("evidence-handoff-status",vec!["--directory","packet","--receipt-sha256","known"],CommandName::DeliveryEvidenceHandoffStatus),
            ("evidence-handoff-inspect",vec!["--file","release-evidence.json","--handoff-sha256","known"],CommandName::DeliveryEvidenceHandoffInspect),
        ] {
            let base=["aniflow","delivery",verb];assert_eq!(Cli::try_parse_from(base.into_iter().chain(args.clone())).unwrap().command.name(),name);
            for flag in ["--include-private","--publish","--verify-timestamp"] {assert!(Cli::try_parse_from(base.into_iter().chain(args.clone()).chain([flag])).is_err());}
        }
        assert!(Cli::try_parse_from(["aniflow","delivery","evidence-handoff","--request","disclosure.json"]).is_err());
    }
    #[test]
    fn ots_inspection_and_bound_restore_keep_explicit_pins_and_no_chain_flags() {
        let inspect=["aniflow","delivery","proof-inspect-ots","--directory","retained","--receipt-sha256","known"];
        assert_eq!(Cli::try_parse_from(inspect).unwrap().command.name(),CommandName::DeliveryProofInspectOts);
        assert!(Cli::try_parse_from(["aniflow","delivery","proof-inspect-ots","--directory","retained"]).is_err());
        let restore=["aniflow","delivery","proof-restore-bound","--request","restore.json","--output-directory","new pair"];
        assert_eq!(Cli::try_parse_from(restore).unwrap().command.name(),CommandName::DeliveryProofRestoreBound);
        for flag in ["--verify-timestamp","--network","--trust-calendar","--overwrite"] {assert!(Cli::try_parse_from(restore.into_iter().chain([flag])).is_err());}
    }
    #[test] fn proof_delivery_requires_explicit_pins_and_never_accepts_network_or_verification_flags(){assert!(Cli::try_parse_from(["aniflow","delivery","proof-inspect","--directory","retained"]).is_err());let args=["aniflow","delivery","proof-restore","--request","restore.json","--output-directory","fresh restored pair"];assert_eq!(Cli::try_parse_from(args).unwrap().command.name(),CommandName::DeliveryProofRestore);for flag in ["--stamp","--upgrade","--verify-timestamp","--network","--overwrite"]{assert!(Cli::try_parse_from(args.into_iter().chain([flag])).is_err());}assert!(Cli::try_parse_from(["aniflow","delivery","proof-restore","--request","restore.json"]).is_err());}
    #[test]
    fn platform_copy_commands_require_supplied_files_and_never_offer_fetch_or_post() {
        let record=["aniflow","delivery","post-upload-record","--request","private request.json","--output-directory","new private root"];
        assert_eq!(Cli::try_parse_from(record).unwrap().command.name(),CommandName::DeliveryPostUploadRecord);
        for flag in ["--fetch","--post","--approve","--download","--verify-platform"] {assert!(Cli::try_parse_from(record.into_iter().chain([flag])).is_err());}
        let inspect=["aniflow","delivery","post-upload-inspect","--directory","private root"];
        assert!(Cli::try_parse_from(inspect).is_err());assert_eq!(Cli::try_parse_from(inspect.into_iter().chain(["--receipt-sha256","known-digest"])).unwrap().command.name(),CommandName::DeliveryPostUploadInspect);
    }
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
