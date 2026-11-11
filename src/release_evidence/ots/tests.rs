//! Synthetic binary fixtures; authored and uncompiled/unrun.
use super::*;
include!("fixture_constants.rs");
fn uint(mut value: u64) -> Vec<u8> {
    let mut bytes=vec![];
    loop {let next=(value&127) as u8;value>>=7;bytes.push(next|if value==0 {0}else{128});if value==0{return bytes}}
}
fn leaf(tag: &[u8;8], payload: &[u8]) -> Vec<u8> {let mut b=vec![0];b.extend(tag);b.extend(uint(payload.len() as u64));b.extend(payload);b}
fn bitcoin(height: u32) -> Vec<u8> {leaf(&BITCOIN,&uint(u64::from(height)))}
fn pending() -> Vec<u8> {let uri=b"https://calendar.example";let mut b=uint(uri.len() as u64);b.extend(uri);leaf(&PENDING,&b)}
fn proof(body: &[u8]) -> Vec<u8> {let mut b=MAGIC.to_vec();b.extend([1,8]);b.extend([0;32]);b.extend(body);b}
fn read(body: &[u8]) -> ProofInspection {inspect(&proof(body),&"0".repeat(64),&CancellationToken::new()).unwrap()}
#[test]
fn bitcoin_height_and_root_are_claims_not_a_verified_time() {
    let report=read(&bitcoin(800000));
    assert_eq!(report.structure,StructureState::Parsed);
    assert_eq!(report.subject_binding,SubjectBinding::MatchesExpectedSha256);
    assert_eq!(report.attestations[0].detail,AttestationDetail::BitcoinBlockHeader {claimed_block_height:800000,claimed_merkle_root_hex:"0".repeat(64)});
    assert_eq!(report.timestamp_state,"not_verified");assert_eq!(report.ownership,"not_established");
    assert_eq!(report.chain_verification,"not_performed; no_chain_or_verifier_selected");
}
#[test]
fn evaluated_sha256_commitment_matches_a_fixed_independent_vector() {
    let mut body=vec![8];body.extend(bitcoin(100));let report=read(&body);
    assert_eq!(report.operation_count,1);
    assert_eq!(report.attestations[0].detail,AttestationDetail::BitcoinBlockHeader {claimed_block_height:100,
        claimed_merkle_root_hex:"66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925".into()});
    let mut body=vec![0xf0,1,0xaa,0xf1,1,0xbb,0xf2,0xf3,8];body.extend(bitcoin(101));
    let report=read(&body);assert_eq!(report.operation_count,5);
    assert_eq!(report.attestations[0].detail,AttestationDetail::BitcoinBlockHeader {claimed_block_height:101,claimed_merkle_root_hex:MIXED_OPERATION_ROOT.into()});
}
#[test]
fn sibling_branches_have_independent_messages_and_private_uri_hashes() {
    let mut body=vec![0xff];body.extend(pending());body.push(8);body.extend(bitcoin(7));let report=read(&body);
    assert_eq!(report.attestations.len(),2);
    assert_ne!(report.attestations[0].operation_path_sha256,report.attestations[1].operation_path_sha256);
    assert_ne!(report.attestations[0].commitment_sha256,report.attestations[1].commitment_sha256);
    let json=serde_json::to_string(&report).unwrap();assert!(!json.contains("https://calendar.example"));
    assert!(matches!(report.attestations[0].detail,AttestationDetail::PendingCalendar {..}));
    assert_eq!(report.timestamp_state,"not_verified");
}
#[test]
fn wrong_subject_remains_a_mismatch_and_opaque_attestations_are_not_anchors() {
    let bytes=proof(&leaf(b"unknown!",b"untrusted opaque data"));
    let report=inspect(&bytes,&"a".repeat(64),&CancellationToken::new()).unwrap();
    assert_eq!(report.subject_binding,SubjectBinding::Mismatch);
    assert!(matches!(report.attestations[0].detail,AttestationDetail::Unknown {..}));
    assert_eq!(report.timestamp_state,"not_verified");
}
#[test]
fn unsupported_version_hash_or_operation_discards_all_partial_results() {
    let mut bytes=proof(&bitcoin(7));bytes[MAGIC.len()]=2;
    let r=inspect(&bytes,&"0".repeat(64),&CancellationToken::new()).unwrap();
    assert_eq!(r.unsupported_feature.unwrap().kind,UnsupportedKind::MajorVersion);
    let mut bytes=proof(&bitcoin(7));bytes[MAGIC.len()+1]=2;
    let r=inspect(&bytes,&"0".repeat(64),&CancellationToken::new()).unwrap();
    assert_eq!(r.subject_binding,SubjectBinding::NotAssessedUnsupported);
    let mut body=vec![0xff];body.extend(pending());body.extend([0x02,0x00]);
    let r=read(&body);assert_eq!(r.structure,StructureState::Unsupported);
    assert_eq!(r.unsupported_feature.unwrap().tag,2);assert!(r.attestations.is_empty());assert_eq!(r.operation_count,0);
}
#[test]
fn malformed_truncated_nonminimal_and_trailing_documents_fail() {
    let valid=proof(&bitcoin(128));
    for end in [0,1,MAGIC.len(),MAGIC.len()+1,MAGIC.len()+20,valid.len()-1] {
        assert!(inspect(&valid[..end],&"0".repeat(64),&CancellationToken::new()).is_err());
    }
    let mut bytes=valid.clone();bytes[0]=1;assert!(inspect(&bytes,&"0".repeat(64),&CancellationToken::new()).is_err());
    let mut bytes=valid;bytes.push(0);assert!(inspect(&bytes,&"0".repeat(64),&CancellationToken::new()).is_err());
    for body in [leaf(&BITCOIN,&[0x80,0]),leaf(&BITCOIN,&[0x80;10]),leaf(&BITCOIN,&uint(u64::from(u32::MAX)+1)),vec![0xff,0xff]] {
        assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
    }
}
#[test]
fn duplicate_operations_attestations_and_oversized_nodes_fail() {
    let mut body=vec![0xff,8];body.extend(bitcoin(7));body.push(8);body.extend(bitcoin(8));
    assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
    let mut body=vec![0xff];body.extend(pending());body.extend(pending());
    assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
    let mut body=vec![];for n in 0..65 {if n<64{body.push(0xff)}body.extend(bitcoin(n));}
    assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
}
#[test]
fn recursion_argument_message_and_total_proof_bounds_fail_closed() {
    let mut body=vec![8;65];body.extend(bitcoin(7));assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
    let mut body=vec![0xf0];body.extend(uint(4096));body.extend(vec![1;4096]);body.extend(bitcoin(7));
    assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
    let mut body=vec![0xf0,0];body.extend(bitcoin(7));assert!(inspect(&proof(&body),&"0".repeat(64),&CancellationToken::new()).is_err());
    assert!(inspect(&vec![0;MAXIMUM_PROOF_BYTES+1],&"0".repeat(64),&CancellationToken::new()).is_err());
}
#[test]
fn pending_payload_validation_precedes_any_calendar_interpretation() {
    let uri=b"https://calendar.example/?secret=never-fetch";let mut payload=uint(uri.len() as u64);payload.extend(uri);
    assert!(inspect(&proof(&leaf(&PENDING,&payload)),&"0".repeat(64),&CancellationToken::new()).is_err());
    let mut payload=vec![1,b'x',0];assert!(inspect(&proof(&leaf(&PENDING,&payload)),&"0".repeat(64),&CancellationToken::new()).is_err());
    payload=vec![0];assert!(inspect(&proof(&leaf(&PENDING,&payload)),&"0".repeat(64),&CancellationToken::new()).is_err());
}
#[test]
fn cancellation_and_unknown_authority_fields_never_become_success() {
    let token=CancellationToken::new();token.cancel();assert!(inspect(&proof(&bitcoin(7)),&"0".repeat(64),&token).is_err());
    let mut value=serde_json::to_value(read(&bitcoin(7))).unwrap();value["verified_at"]=serde_json::json!("invented");
    assert!(serde_json::from_value::<ProofInspection>(value).is_err());
    assert!(inspect(&proof(&bitcoin(7)),"NOT_A_HASH",&CancellationToken::new()).is_err());
}
