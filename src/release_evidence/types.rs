use crate::destination::{
    MediaInput, MediaObservation,
    posting::{MemberVisibility, PostingReceipt},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectRole {
    Original,
    Milestone,
    FinalDelivery,
    Sidecar,
    ProductionEvidence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostingMember {
    Media,
    PublicMetadata,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubjectInput {
    AcceptedMedia {
        input: MediaInput,
    },
    PostingMember {
        directory: PathBuf,
        member: PostingMember,
    },
    /// Exact bytes with declared history; never synthetic provider acceptance.
    DeclaredFile {
        file: PathBuf,
        expected_sha256: String,
        maximum_bytes: u64,
        declared_origin: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectSelection {
    pub id: String,
    pub role: SubjectRole,
    pub visibility: MemberVisibility,
    pub parents: Vec<String>,
    pub input: SubjectInput,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRequest {
    pub schema: String,
    pub release_id: String,
    pub version: String,
    pub subjects: Vec<SubjectSelection>,
    pub maximum_bundle_bytes: u64,
    pub minimum_free_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubjectOrigin {
    AcceptedMedia {
        observation: MediaObservation,
    },
    PostingMember {
        directory: PathBuf,
        member: PostingMember,
        receipt: PostingReceipt,
        receipt_sha256: String,
    },
    DeclaredFile {
        locator: PathBuf,
        declared_origin: String,
        technical_acceptance: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectRecord {
    pub id: String,
    pub role: SubjectRole,
    pub visibility: MemberVisibility,
    pub parents: Vec<String>,
    pub path: String,
    pub sha256: String,
    pub byte_count: u64,
    pub origin: SubjectOrigin,
}
/// The exact canonical file bytes of this manifest are the potential future
/// timestamp subject. It has no self-digest, proof or verification observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectManifest {
    pub schema: String,
    pub release_id: String,
    pub version: String,
    pub request_sha256: String,
    pub subjects: Vec<SubjectRecord>,
    pub creative_approval: String,
    pub ownership: String,
    pub publication_authority: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePlan {
    pub schema: String,
    pub manifest: SubjectManifest,
    pub manifest_sha256: String,
    pub manifest_byte_count: u64,
    pub projected_bundle_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealReceipt {
    pub schema: String,
    pub subject_manifest_sha256: String,
    pub subject_manifest_byte_count: u64,
    pub request_sha256: String,
    pub subject_count: u32,
    pub subject_bytes: u64,
    pub signature_state: String,
    pub timestamp_state: String,
    pub publication_authority: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteVerification {
    pub schema: String,
    pub subject_manifest_sha256: String,
    pub subject_count: u32,
    pub subject_bytes: u64,
    pub state: String,
    pub producer_acceptance: String,
    pub timestamp_state: String,
    pub ownership: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationObservation {
    pub schema: String,
    pub sequence: u32,
    pub previous_observation_sha256: Option<String>,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    pub clock_authority: String,
    pub verification: ByteVerification,
}
