use super::*;
pub(super) fn fixture()->(tempfile::TempDir,ProofRetentionRequest,PathBuf) {
    use crate::release_evidence::*;
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let file=root.join("original.txt");std::fs::write(&file,b"synthetic original").unwrap();
    let request=EvidenceRequest {schema:"aniflow.release-evidence-request/v1".into(),release_id:"fixture".into(),version:"v1".into(),subjects:vec![SubjectSelection {id:"original".into(),role:SubjectRole::Original,visibility:crate::destination::posting::MemberVisibility::Private,parents:vec![],input:SubjectInput::DeclaredFile {file,expected_sha256:local::hash(b"synthetic original"),maximum_bytes:4096,declared_origin:"synthetic declared history".into()}}],maximum_bundle_bytes:16_777_216,minimum_free_bytes:0};
    let bundle_directory=root.join("bundle");let sealed=seal(&request,&bundle_directory,&CancellationToken::default()).unwrap();
    let proof_file=root.join("candidate.ots");std::fs::write(&proof_file,b"unparsed synthetic proof candidate; not valid OTS").unwrap();
    let proof=ProofRetentionRequest {schema:"aniflow.detached-proof-retention-request/v1".into(),bundle_directory,manifest_sha256:sealed.subject_manifest_sha256,subject:ProofSubject::Manifest {},proof_file,expected_proof_sha256:local::hash(b"unparsed synthetic proof candidate; not valid OTS"),previous:None,maximum_subject_bytes:2_097_152,minimum_free_bytes:0};
    (temporary,proof,root.join("retained-proof"))
}
#[test]
fn opaque_candidate_retains_exact_subject_and_never_claims_verified_time() {
    let (_temporary,request,root)=fixture();let token=CancellationToken::default();let receipt=retain(&request,&root,&token).unwrap();
    let pin=local::hash(&local::bytes(&receipt).unwrap());assert_eq!(inspect(&root,&pin,&token).unwrap(),receipt);
    assert_eq!(receipt.timestamp_state,"not_verified");assert!(receipt.subject_binding.contains("not_verified"));
    assert_eq!(std::fs::read(root.join("subject.bin")).unwrap(),std::fs::read(request.bundle_directory.join("subjects.json")).unwrap());
}
#[test]
fn successor_links_exact_previous_receipt_without_rewriting_it() {
    let (_temporary,mut request,root)=fixture();let token=CancellationToken::default();let first=retain(&request,&root,&token).unwrap();let first_bytes=std::fs::read(root.join("receipt.json")).unwrap();
    request.previous=Some(ProofPredecessor {directory:root.clone(),receipt_sha256:local::hash(&first_bytes)});
    let next=root.with_file_name("successor");assert!(retain(&request,&next,&token).is_err());assert!(!next.exists());
    std::fs::write(&request.proof_file,b"different unparsed candidate").unwrap();request.expected_proof_sha256=local::hash(b"different unparsed candidate");
    let second=retain(&request,&next,&token).unwrap();assert_eq!(second.sequence,2);assert_eq!(second.previous_proof_sha256,Some(first.proof_sha256));
    assert_eq!(std::fs::read(root.join("receipt.json")).unwrap(),first_bytes);assert_eq!(inspect(&next,&local::hash(&local::bytes(&second).unwrap()),&token).unwrap(),second);
}
#[test]
fn readback_survives_missing_original_paths_but_rejects_changed_copied_proof() {
    let (_temporary,request,root)=fixture();let token=CancellationToken::default();let receipt=retain(&request,&root,&token).unwrap();let pin=local::hash(&local::bytes(&receipt).unwrap());
    std::fs::remove_file(&request.proof_file).unwrap();std::fs::remove_dir_all(&request.bundle_directory).unwrap();assert!(inspect(&root,&pin,&token).is_ok());
    std::fs::write(root.join("proof.ots"),b"tampered").unwrap();assert!(inspect(&root,&pin,&token).is_err());
}
#[test]
fn wrong_pins_unknown_members_and_partial_roots_are_not_adopted() {
    let (_temporary,mut request,root)=fixture();let token=CancellationToken::default();request.expected_proof_sha256="0".repeat(64);assert!(retain(&request,&root,&token).is_err());assert!(!root.exists());
    request.expected_proof_sha256=identity(&request.proof_file,MAX_PROOF).unwrap().0;let receipt=retain(&request,&root,&token).unwrap();let pin=local::hash(&local::bytes(&receipt).unwrap());
    assert!(inspect(&root,&"0".repeat(64),&token).is_err());std::fs::write(root.join("unknown"),b"preserve").unwrap();assert!(inspect(&root,&pin,&token).is_err());assert!(retain(&request,&root,&token).is_err());assert!(root.join("unknown").exists());
}
#[test]
fn summary_omits_private_inputs_and_no_anchored_or_verified_state_is_accepted() {
    let (_temporary,request,root)=fixture();let token=CancellationToken::default();let mut receipt=retain(&request,&root,&token).unwrap();let summary=public_summary(&receipt).unwrap().to_string();
    assert!(!summary.contains(root.parent().unwrap().to_str().unwrap()));assert!(!summary.contains("original"));
    receipt.timestamp_state="verified".into();assert!(public_summary(&receipt).is_err());
}
#[test]
fn cancellation_and_subject_limit_fail_before_root_creation() {
    let (_temporary,mut request,root)=fixture();let token=CancellationToken::default();token.cancel();assert!(retain(&request,&root,&token).is_err());assert!(!root.exists());
    request.maximum_subject_bytes=1;assert!(retain(&request,&root,&CancellationToken::default()).is_err());assert!(!root.exists());
}
