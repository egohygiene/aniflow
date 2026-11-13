//! Bounded OTS v1 format/commitment inspection, without calendars or chain trust.
//! Protocol reference: python-opentimestamps 8c3dc6fbdf6fcba054e78643e9df5eccd8c9fce4.
mod parser;
use crate::{CancellationToken, Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAXIMUM_PROOF_BYTES: usize = 1_048_576;
pub const PROFILE: &str = "aniflow.ots-v1-sha256-commitments/v1";
const SCHEMA: &str = "aniflow.ots-proof-inspection/v1";
const MAGIC: &[u8] = b"\x00OpenTimestamps\x00\x00Proof\x00\xbf\x89\xe2\xe8\x84\xe8\x92\x94";
const BITCOIN: [u8; 8] = [0x05, 0x88, 0x96, 0x0d, 0x73, 0xd7, 0x19, 0x01];
const PENDING: [u8; 8] = [0x83, 0xdf, 0xe3, 0x0d, 0x2e, 0xf9, 0x0c, 0x8e];
const MAXIMUM_MESSAGE_BYTES: usize = 4096;
const MAXIMUM_DEPTH: usize = 64;
const MAXIMUM_OPERATIONS: u32 = 4096;
const MAXIMUM_ATTESTATIONS: usize = 128;
const MAXIMUM_NODE_ENTRIES: usize = 64;
const MAXIMUM_WORK_BYTES: u64 = 33_554_432;
fn invalid(message: &'static str) -> Error {
    Error::new(ErrorCategory::Input, message)
}
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        return Err(Error::new(
            ErrorCategory::Execution,
            "OTS inspection cancelled",
        ));
    }
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}
fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureState {
    Parsed,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectBinding {
    MatchesExpectedSha256,
    Mismatch,
    NotAssessedUnsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedKind {
    MajorVersion,
    FileHashOperation,
    CommitmentOperation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsupportedFeature {
    pub kind: UnsupportedKind,
    pub tag: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttestationDetail {
    PendingCalendar {
        uri_sha256: String,
        uri_byte_count: u32,
    },
    /// Height/root are unverified claims until independently checked against a chain.
    BitcoinBlockHeader {
        claimed_block_height: u32,
        claimed_merkle_root_hex: String,
    },
    Unknown {
        tag_hex: String,
        payload_sha256: String,
        payload_byte_count: u32,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attestation {
    pub operation_path_sha256: String,
    /// Hash of the resulting commitment message, not an authenticated chain root.
    pub commitment_sha256: String,
    pub commitment_byte_count: u32,
    pub detail: AttestationDetail,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofInspection {
    pub schema: String,
    pub profile: String,
    pub proof_sha256: String,
    pub proof_byte_count: u64,
    pub expected_subject_sha256: String,
    pub declared_subject_sha256: Option<String>,
    pub structure: StructureState,
    pub subject_binding: SubjectBinding,
    pub unsupported_feature: Option<UnsupportedFeature>,
    pub operation_count: u32,
    pub commitment_work_bytes: u64,
    pub attestations: Vec<Attestation>,
    pub timestamp_state: String,
    pub chain_verification: String,
    pub ownership: String,
}

/// Inspect supplied bytes against a caller-selected SHA-256. The caller owns
/// observation of the actual subject. Unsupported profiles return no partial
/// attestation/operation results; malformed, excessive and cancelled input fails.
/// No file, network, process, key or local-clock access occurs here.
pub fn inspect(
    bytes: &[u8],
    expected_subject_sha256: &str,
    token: &CancellationToken,
) -> Result<ProofInspection> {
    cancelled(token)?;
    crate::provider::require_sha256(expected_subject_sha256, "expected OTS subject SHA-256")?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_PROOF_BYTES {
        return Err(invalid("OTS proof is empty or exceeds 1 MiB"));
    }
    let mut report = ProofInspection {
        schema: SCHEMA.into(),
        profile: PROFILE.into(),
        proof_sha256: sha(bytes),
        proof_byte_count: bytes.len() as u64,
        expected_subject_sha256: expected_subject_sha256.into(),
        declared_subject_sha256: None,
        structure: StructureState::Unsupported,
        subject_binding: SubjectBinding::NotAssessedUnsupported,
        unsupported_feature: None,
        operation_count: 0,
        commitment_work_bytes: 0,
        attestations: vec![],
        timestamp_state: "not_verified".into(),
        chain_verification: "not_performed; no_chain_or_verifier_selected".into(),
        ownership: "not_established".into(),
    };
    match parser::parse(bytes, token) {
        Ok(parsed) => {
            let subject = hex(&parsed.subject);
            report.subject_binding = if subject == expected_subject_sha256 {
                SubjectBinding::MatchesExpectedSha256
            } else {
                SubjectBinding::Mismatch
            };
            report.declared_subject_sha256 = Some(subject);
            report.structure = StructureState::Parsed;
            report.operation_count = parsed.operations;
            report.commitment_work_bytes = parsed.work_bytes;
            report.attestations = parsed.attestations;
        }
        Err(parser::Failure::Unsupported { feature, subject }) => {
            report.unsupported_feature = Some(feature);
            report.declared_subject_sha256 = subject.map(|s| hex(&s));
        }
        Err(parser::Failure::Invalid(message)) => return Err(invalid(message)),
        Err(parser::Failure::Cancelled) => {
            return Err(Error::new(
                ErrorCategory::Execution,
                "OTS inspection cancelled",
            ));
        }
    }
    cancelled(token)?;
    Ok(report)
}

#[cfg(test)]
mod tests;
