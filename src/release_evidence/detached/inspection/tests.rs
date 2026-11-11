//! Synthetic retained binary candidates; authored uncompiled/unrun.
use super::*;
fn fixture(mode: &str) -> (tempfile::TempDir,restoration::RestoreRequest,ProofRetentionRequest) {
    let (temporary,mut request,root)=super::super::tests::fixture();
    if mode!="opaque" {
        let hash=if mode=="mismatch" {"a".repeat(64)}else{request.manifest_sha256.clone()};
        let mut bytes=b"\x00OpenTimestamps\x00\x00Proof\x00\xbf\x89\xe2\xe8\x84\xe8\x92\x94".to_vec();
        bytes.extend([if mode=="unsupported" {2}else{1},8]);
        for i in (0..64).step_by(2) {bytes.push(u8::from_str_radix(&hash[i..i+2],16).unwrap());}
        bytes.extend([0,0x05,0x88,0x96,0x0d,0x73,0xd7,0x19,0x01,1,100]);
        std::fs::write(&request.proof_file,&bytes).unwrap();request.expected_proof_sha256=local::hash(&bytes);
    }
    let receipt=super::super::retain(&request,&root,&CancellationToken::new()).unwrap();
    let restore=restoration::RestoreRequest {schema:"aniflow.detached-proof-restore-request/v1".into(),source_directory:root,
        receipt_sha256:local::hash(&local::bytes(&receipt).unwrap()),maximum_total_bytes:16_777_216,minimum_free_bytes:0};
    (temporary,restore,request)
}
#[test]
fn actual_copied_subject_matches_without_promoting_the_opaque_receipt() {
    let (_temp,r,original)=fixture("matched");let before=std::fs::read(r.source_directory.join("receipt.json")).unwrap();
    std::fs::remove_dir_all(&original.bundle_directory).unwrap();std::fs::remove_file(&original.proof_file).unwrap();
    let result=inspect(&r.source_directory,&r.receipt_sha256,&CancellationToken::new()).unwrap();
    assert!(result.subject_matches());assert_eq!(result.proof.expected_subject_sha256,result.retained.subject_sha256);
    assert_eq!(result.retained.subject_binding,BINDING);assert_eq!(result.timestamp_state,"not_verified");
    assert_eq!(result.producer_acceptance,"not_reobserved; archival_bytes_only");
    assert_eq!(std::fs::read(r.source_directory.join("receipt.json")).unwrap(),before);
}
#[test]
fn mismatched_header_is_visible_but_cannot_authorize_bound_restoration() {
    let (_temp,r,_)=fixture("mismatch");let output=r.source_directory.with_file_name("bound-copy");
    let result=inspect(&r.source_directory,&r.receipt_sha256,&CancellationToken::new()).unwrap();
    assert_eq!(result.proof.subject_binding,SubjectBinding::Mismatch);assert!(!result.subject_matches());
    assert!(restore_bound(&r,&output,&CancellationToken::new()).is_err());assert!(!output.exists());
}
#[test]
fn unsupported_and_malformed_proofs_keep_opaque_retention_available() {
    for mode in ["unsupported","opaque"] {
        let (_temp,r,_)=fixture(mode);let output=r.source_directory.with_file_name("refused-copy");
        let result=inspect(&r.source_directory,&r.receipt_sha256,&CancellationToken::new());
        if mode=="unsupported" {assert_eq!(result.unwrap().proof.structure,StructureState::Unsupported)}else{assert!(result.is_err())}
        assert!(restore_bound(&r,&output,&CancellationToken::new()).is_err());assert!(!output.exists());
        assert!(super::super::inspect(&r.source_directory,&r.receipt_sha256,&CancellationToken::new()).is_ok());
    }
}
#[test]
fn restored_bound_pair_preserves_exact_receipt_and_can_be_reinspected() {
    let (_temp,r,original)=fixture("matched");let token=CancellationToken::new();let output=r.source_directory.with_file_name("bound-copy");
    let before=inspect(&r.source_directory,&r.receipt_sha256,&token).unwrap();
    std::fs::remove_dir_all(original.bundle_directory).unwrap();std::fs::remove_file(original.proof_file).unwrap();
    let restored=restore_bound(&r,&output,&token).unwrap();assert_eq!(restored,before);
    assert_eq!(inspect(&output,&r.receipt_sha256,&token).unwrap(),before);
    assert_eq!(std::fs::read(output.join("receipt.json")).unwrap(),std::fs::read(r.source_directory.join("receipt.json")).unwrap());
    assert!(restored.subject_matches());assert_eq!(restored.timestamp_state,"not_verified");
}
#[test]
fn external_pin_and_current_copied_members_are_required() {
    let (_temp,r,_)=fixture("matched");assert!(inspect(&r.source_directory,&"0".repeat(64),&CancellationToken::new()).is_err());
    std::fs::write(r.source_directory.join("subject.bin"),b"changed").unwrap();
    assert!(inspect(&r.source_directory,&r.receipt_sha256,&CancellationToken::new()).is_err());
    let (_temp,r,_)=fixture("matched");std::fs::write(r.source_directory.join("proof.ots"),b"changed").unwrap();
    assert!(inspect(&r.source_directory,&r.receipt_sha256,&CancellationToken::new()).is_err());
}
#[test]
fn cancellation_existing_output_and_overlap_never_adopt_or_clear_work() {
    let (_temp,r,_)=fixture("matched");let token=CancellationToken::new();token.cancel();let output=r.source_directory.with_file_name("preserved");
    assert!(inspect(&r.source_directory,&r.receipt_sha256,&token).is_err());assert!(restore_bound(&r,&output,&token).is_err());assert!(!output.exists());
    std::fs::create_dir(&output).unwrap();std::fs::write(output.join("keep"),b"preserve").unwrap();
    assert!(restore_bound(&r,&output,&CancellationToken::new()).is_err());assert_eq!(std::fs::read(output.join("keep")).unwrap(),b"preserve");
    assert!(restore_bound(&r,&r.source_directory.join("overlap"),&CancellationToken::new()).is_err());
    assert!(!r.source_directory.join("overlap").exists());
}
