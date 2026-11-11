//! Explicit public-field projection from an exact private archive and proof pairs.
mod storage;
pub use storage::{create, observe};
use super::*;
use serde::Deserialize;
use crate::provider::{canonical_sha256, decode_json, require_sha256};
use crate::release_evidence::ots::AttestationDetail;

const REQUEST: &str = "aniflow.release-evidence-handoff-request/v1";
const PUBLIC: &str = "aniflow.release-evidence-handoff/v1";
const SCOPE: &str = "archival_bytes_only; producer_acceptance_not_reobserved";
const BINDING: &str = "matches_selected_subject_sha256; timestamp_not_verified";
const AVAILABILITY: &str = "proof_bytes_not_included; separate_disclosure_required";
const PUBLIC_PATH: &str = "public/release-evidence.json";
fn absolute(path: &Path) -> bool {
    path.is_absolute() && path.as_os_str().len()<=4096 && path.to_str().is_some_and(|s|
        !s.chars().any(char::is_control) && !s.split('/').any(|p|matches!(p,"."|"..")))
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofSelection { pub directory: PathBuf, pub receipt_sha256: String }
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disclosure { pub id: String, pub proof: Option<ProofSelection> }
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffRequest {
    pub schema: String, pub bundle_directory: PathBuf, pub manifest_sha256: String,
    pub subjects: Vec<Disclosure>, pub maximum_archive_subject_bytes: u64, pub maximum_proof_bytes: u64, pub minimum_free_bytes: u64,
}
impl HandoffRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len()>131_072 {return Err(invalid("evidence handoff request exceeds 128 KiB"));}
        let value:Self=decode_json(bytes,"evidence handoff request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        require_sha256(&self.manifest_sha256,"handoff source manifest")?;
        if self.schema!=REQUEST || !absolute(&self.bundle_directory) || self.subjects.is_empty() || self.subjects.len()>64
            || !(1..=MAXIMUM_BYTES).contains(&self.maximum_archive_subject_bytes) || self.maximum_proof_bytes>67_108_864
            || self.minimum_free_bytes>MAXIMUM_BYTES {return Err(invalid("handoff requires an exact archive, explicit bounded public selections and finite byte limits"));}
        let mut ids=BTreeSet::new();
        for subject in &self.subjects {
            if !identifier(&subject.id) || !ids.insert(&subject.id) {return Err(invalid("handoff subject IDs must be unique and explicit"));}
            if let Some(proof)=&subject.proof {
                require_sha256(&proof.receipt_sha256,"selected retained proof receipt")?;
                if !absolute(&proof.directory) {return Err(invalid("proof selection needs a normalized absolute private root"));}
            }
        }
        if local::bytes(self)?.len()>131_072 {return Err(invalid("evidence handoff request exceeds 128 KiB"));}Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicProof {
    pub profile: String, pub subject_sha256: String, pub proof_sha256: String, pub proof_byte_count: u64,
    pub operation_count: u32, pub pending_calendar_count: u32, pub bitcoin_claim_count: u32, pub unknown_attestation_count: u32,
    pub subject_binding: String, pub timestamp_state: String, pub availability: String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicSubject {
    pub id: String, pub role: SubjectRole, pub sha256: String, pub byte_count: u64, pub parents: Vec<String>, pub proof: Option<PublicProof>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicHandoff {
    pub schema: String, pub release_id: String, pub version: String, pub subjects: Vec<PublicSubject>,
    pub evidence_scope: String, pub signature_state: String, pub timestamp_state: String, pub ownership: String, pub publication_authority: String,
}
impl PublicHandoff {
    pub fn validate(&self) -> Result<()> {
        if self.schema!=PUBLIC || !identifier(&self.release_id) || !identifier(&self.version) || self.subjects.is_empty() || self.subjects.len()>64
            || self.evidence_scope!=SCOPE || self.signature_state!="not_signed" || self.timestamp_state!="not_verified"
            || self.ownership!="not_established" || self.publication_authority!="none" {
            return Err(invalid("public evidence handoff has unsupported scope or authority claims"));
        }
        let mut ids=BTreeSet::new();
        for s in &self.subjects {
            require_sha256(&s.sha256,"public subject digest")?;
            if !identifier(&s.id) || ids.contains(&s.id) || s.byte_count>MAXIMUM_BYTES || s.parents.len()>8
                || s.parents.iter().any(|p|!ids.contains(p)) || s.parents.iter().enumerate().any(|(i,p)|s.parents[..i].contains(p)) {
                return Err(invalid("public handoff IDs/lineage must be unique and preserve retained topological order"));
            }
            ids.insert(s.id.clone());
            if let Some(p)=&s.proof {
                require_sha256(&p.proof_sha256,"public proof digest")?;
                let total=u64::from(p.pending_calendar_count)+u64::from(p.bitcoin_claim_count)+u64::from(p.unknown_attestation_count);
                if p.profile!=ots::PROFILE || p.subject_sha256!=s.sha256 || !(1..=1_048_576).contains(&p.proof_byte_count)
                    || p.operation_count>4096 || !(1..=128).contains(&total) || p.subject_binding!=BINDING || p.timestamp_state!="not_verified" || p.availability!=AVAILABILITY {
                    return Err(invalid("public proof observation differs from a selected local subject binding or claims stronger verification"));
                }
            }
        }
        local::bytes(self)?;Ok(())
    }
    /// Portable bytes/integrity readback only. A known hash is not authenticity,
    /// archived-producer reobservation, a trusted timestamp or permission to post.
    pub fn from_pinned_json(bytes: &[u8], expected_sha256: &str) -> Result<Self> {
        require_sha256(expected_sha256,"portable handoff external digest")?;
        if bytes.len() as u64>local::DOCUMENT_BYTES || local::hash(bytes)!=expected_sha256 {return Err(invalid("public handoff bytes differ from the exact external pin or exceed 2 MiB"));}
        let value:Self=decode_json(bytes,"public evidence handoff")?;value.validate()?;
        if local::bytes(&value)?!=bytes {return Err(invalid("portable handoff must retain its exact canonical bytes"));}Ok(value)
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffPlan {
    pub schema: String, pub request_sha256: String, pub source_manifest_sha256: String,
    pub public_handoff_sha256: String, pub public_handoff_byte_count: u64, pub public_handoff: PublicHandoff,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffReceipt {
    pub schema: String, pub request_sha256: String, pub source_manifest_sha256: String,
    pub public_handoff_sha256: String, pub public_handoff_byte_count: u64,
    pub subject_count: u32, pub proof_count: u32, pub evidence_scope: String, pub publication_authority: String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffStatus {
    pub schema: String, pub receipt_sha256: String, pub receipt: HandoffReceipt, pub public_handoff: PublicHandoff,
}
fn public_proof(selection: &ProofSelection, subject: &SubjectRecord, manifest_sha256: &str, token: &CancellationToken) -> Result<PublicProof> {
    let result=detached::inspection::inspect(&selection.directory,&selection.receipt_sha256,token)?;
    if !result.subject_matches() || result.retained.subject!= (detached::ProofSubject::Subject {id:subject.id.clone()})
        || result.retained.manifest_sha256!=manifest_sha256 || result.retained.subject_sha256!=subject.sha256 || result.retained.subject_bytes!=subject.byte_count {
        return Err(invalid("disclosed proof must bind the exact selected public file in this archive; aggregate/private manifest proofs are excluded"));
    }
    let mut pending=0;let mut bitcoin=0;let mut unknown=0;
    for a in &result.proof.attestations {match &a.detail {AttestationDetail::PendingCalendar {..}=>pending+=1,AttestationDetail::BitcoinBlockHeader {..}=>bitcoin+=1,AttestationDetail::Unknown {..}=>unknown+=1}}
    Ok(PublicProof {profile:ots::PROFILE.into(),subject_sha256:subject.sha256.clone(),proof_sha256:result.proof.proof_sha256,
        proof_byte_count:result.proof.proof_byte_count,operation_count:result.proof.operation_count,pending_calendar_count:pending,bitcoin_claim_count:bitcoin,unknown_attestation_count:unknown,
        subject_binding:BINDING.into(),timestamp_state:"not_verified".into(),availability:AVAILABILITY.into()})
}
fn project(request: &HandoffRequest, manifest: &SubjectManifest, token: &CancellationToken) -> Result<PublicHandoff> {
    for selection in &request.subjects {
        if !manifest.subjects.iter().any(|s|s.id==selection.id&&s.visibility==MemberVisibility::Public) {
            return Err(invalid("only existing explicitly public archive subjects may be disclosed; private/missing selection refused"));
        }
    }
    let selected=request.subjects.iter().map(|s|s.id.as_str()).collect::<BTreeSet<_>>();
    let mut subjects=vec![];let mut proof_bytes=0u64;
    for subject in manifest.subjects.iter().filter(|s|selected.contains(s.id.as_str())) {
        cancelled(token)?;
        let selection=request.subjects.iter().find(|s|s.id==subject.id).ok_or_else(||invalid("disclosure selection disappeared"))?;
        let proof=selection.proof.as_ref().map(|s|public_proof(s,subject,&request.manifest_sha256,token)).transpose()?;
        if let Some(p)=&proof {proof_bytes=proof_bytes.checked_add(p.proof_byte_count).filter(|n|*n<=request.maximum_proof_bytes).ok_or_else(||invalid("selected handoff proofs exceed total byte admission"))?;}
        subjects.push(PublicSubject {id:subject.id.clone(),role:subject.role,sha256:subject.sha256.clone(),byte_count:subject.byte_count,
            parents:subject.parents.iter().filter(|p|selected.contains(p.as_str())).cloned().collect(),proof});
    }
    let value=PublicHandoff {schema:PUBLIC.into(),release_id:manifest.release_id.clone(),version:manifest.version.clone(),subjects,
        evidence_scope:SCOPE.into(),signature_state:"not_signed".into(),timestamp_state:"not_verified".into(),ownership:"not_established".into(),publication_authority:"none".into()};
    value.validate()?;Ok(value)
}
pub fn plan(request: &HandoffRequest, token: &CancellationToken) -> Result<HandoffPlan> {
    cancelled(token)?;request.validate()?;ensure_canonical_directory(&request.bundle_directory)?;
    let path=request.bundle_directory.join("subjects.json");
    if identity(&path,local::DOCUMENT_BYTES)?.0!=request.manifest_sha256 {return Err(invalid("handoff archive manifest differs from external pin"));}
    let manifest:SubjectManifest=read_exact(&path,local::DOCUMENT_BYTES)?;
    if manifest.subjects.len()>64 {return Err(invalid("handoff archive subject count exceeds 64"));}
    manifest.subjects.iter().try_fold(0u64,|total,s|total.checked_add(s.byte_count).filter(|n|*n<=request.maximum_archive_subject_bytes)
        .ok_or_else(||invalid("complete archive subject bytes exceed selected handoff admission")))?;
    let archive=super::verify(&request.bundle_directory,&request.manifest_sha256,token)?;
    let public_handoff=project(request,&manifest,token)?;let bytes=local::bytes(&public_handoff)?;
    if super::verify(&request.bundle_directory,&request.manifest_sha256,token)?!=archive || read_exact::<SubjectManifest>(&path,local::DOCUMENT_BYTES)?!=manifest
        || project(request,&manifest,token)?!=public_handoff {return Err(invalid("archive/proof selections changed during public handoff planning"));}
    cancelled(token)?;
    Ok(HandoffPlan {schema:"aniflow.release-evidence-handoff-plan/v1".into(),request_sha256:canonical_sha256(request)?,source_manifest_sha256:request.manifest_sha256.clone(),
        public_handoff_sha256:local::hash(&bytes),public_handoff_byte_count:bytes.len() as u64,public_handoff})
}
fn receipt(plan: &HandoffPlan) -> HandoffReceipt {
    HandoffReceipt {schema:"aniflow.release-evidence-handoff-receipt/v1".into(),request_sha256:plan.request_sha256.clone(),source_manifest_sha256:plan.source_manifest_sha256.clone(),
        public_handoff_sha256:plan.public_handoff_sha256.clone(),public_handoff_byte_count:plan.public_handoff_byte_count,
        subject_count:plan.public_handoff.subjects.len() as u32,proof_count:plan.public_handoff.subjects.iter().filter(|s|s.proof.is_some()).count() as u32,
        evidence_scope:SCOPE.into(),publication_authority:"none".into()}
}
#[cfg(test)]
mod tests;
