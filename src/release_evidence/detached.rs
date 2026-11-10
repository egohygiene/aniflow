//! Offline exact-byte retention of opaque proof candidates. No OTS verification.
pub mod restoration;
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use crate::{CancellationToken,Result,frame_io::{identity,ensure_canonical_directory,copy_pinned}};
use crate::destination::{local,invalid,cancelled};
use super::{SubjectManifest,digest,read_exact};
const MAX_PROOF:u64=1_048_576;
const MAX_SUBJECT:u64=268_435_456;
const FORMAT:&str="opaque_opentimestamps_candidate; unparsed";
const BINDING:&str="operator_declared; cryptographic_subject_binding_not_verified";

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum ProofSubject {Manifest {},Subject {id:String}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofPredecessor {pub directory:PathBuf,pub receipt_sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofRetentionRequest {
    pub schema:String,pub bundle_directory:PathBuf,pub manifest_sha256:String,pub subject:ProofSubject,
    pub proof_file:PathBuf,pub expected_proof_sha256:String,pub previous:Option<ProofPredecessor>,
    pub maximum_subject_bytes:u64,pub minimum_free_bytes:u64,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofReceipt {
    pub schema:String,pub request_sha256:String,pub manifest_sha256:String,pub subject:ProofSubject,
    pub subject_sha256:String,pub subject_bytes:u64,pub proof_sha256:String,pub proof_bytes:u64,
    pub sequence:u32,pub previous_receipt_sha256:Option<String>,pub previous_proof_sha256:Option<String>,
    pub proof_format:String,pub subject_binding:String,pub timestamp_state:String,pub ownership:String,
}
impl ProofRetentionRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{if bytes.len() as u64>local::DOCUMENT_BYTES{return Err(invalid("detached proof retention request exceeds 2 MiB"));}let value:Self=crate::provider::decode_json(bytes,"detached proof retention request")?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.detached-proof-retention-request/v1"||!digest(&self.manifest_sha256)||!digest(&self.expected_proof_sha256)
            ||!(1..=MAX_SUBJECT).contains(&self.maximum_subject_bytes)||self.minimum_free_bytes>1_099_511_627_776
            ||self.previous.as_ref().is_some_and(|p|!digest(&p.receipt_sha256)) {
            return Err(invalid("detached retention needs exact digests and bounded storage"));
        }
        if let ProofSubject::Subject {id}=&self.subject {if !super::identifier(id){return Err(invalid("invalid selected proof subject ID"));}}
        local::bytes(self)?;Ok(())
    }
}
fn validate_receipt(r:&ProofReceipt)->Result<()> {
    match &r.subject {
        ProofSubject::Manifest {} if r.subject_sha256!=r.manifest_sha256=>return Err(invalid("manifest proof subject must retain the exact manifest digest")),
        ProofSubject::Subject {id} if !super::identifier(id)=>return Err(invalid("invalid retained proof subject ID")),
        _=>{},
    }
    if r.schema!="aniflow.detached-proof-candidate/v1"||![&r.request_sha256,&r.manifest_sha256,&r.subject_sha256,&r.proof_sha256].iter().all(|s|digest(s))
        ||r.subject_bytes>MAX_SUBJECT||!(1..=MAX_PROOF).contains(&r.proof_bytes)||!(1..=256).contains(&r.sequence)
        ||r.previous_receipt_sha256.is_some()!=(r.sequence>1)||r.previous_proof_sha256.is_some()!=(r.sequence>1)
        ||r.previous_receipt_sha256.as_ref().is_some_and(|s|!digest(s))||r.previous_proof_sha256.as_ref().is_some_and(|s|!digest(s))
        ||r.proof_format!=FORMAT||r.subject_binding!=BINDING||r.timestamp_state!="not_verified"||r.ownership!="not_established" {
        return Err(invalid("detached proof receipt has unsupported identity, bounds, lineage or authority claims"));
    }Ok(())
}
fn linkage(current:&ProofReceipt,previous:&ProofReceipt,pin:&str)->Result<()> {
    validate_receipt(previous)?;
    if local::hash(&local::bytes(previous)?)!=pin||current.previous_receipt_sha256.as_deref()!=Some(pin)
        ||current.previous_proof_sha256.as_ref()!=Some(&previous.proof_sha256)||current.sequence!=previous.sequence+1
        ||current.manifest_sha256!=previous.manifest_sha256||current.subject!=previous.subject
        ||current.subject_sha256!=previous.subject_sha256||current.subject_bytes!=previous.subject_bytes
        ||current.proof_sha256==previous.proof_sha256{return Err(invalid("successor proof must retain its exact immediate receipt and unchanged subject, with distinct candidate bytes"));}Ok(())
}
fn resolve(request:&ProofRetentionRequest,token:&CancellationToken)->Result<(PathBuf,(String,u64))> {
    super::verify(&request.bundle_directory,&request.manifest_sha256,token)?;
    let manifest_path=request.bundle_directory.join("subjects.json");
    let manifest:SubjectManifest=read_exact(&manifest_path,local::DOCUMENT_BYTES)?;
    if local::hash(&local::bytes(&manifest)?)!=request.manifest_sha256{return Err(invalid("retained subject manifest changed during proof selection"));}
    let (path,pin)=match &request.subject {
        ProofSubject::Manifest {}=>(manifest_path,(request.manifest_sha256.clone(),local::bytes(&manifest)?.len() as u64)),
        ProofSubject::Subject {id}=>{let subject=manifest.subjects.iter().find(|s|&s.id==id).ok_or_else(||invalid("proof subject is not in the retained manifest"))?;
            (request.bundle_directory.join(&subject.path),(subject.sha256.clone(),subject.byte_count))},
    };
    if pin.1>request.maximum_subject_bytes||identity(&path,request.maximum_subject_bytes)?!=pin{return Err(invalid("selected proof subject exceeds its limit or changed bytes"));}Ok((path,pin))
}
fn private_copy(source:&Path,target:&Path,pin:&(String,u64),maximum:u64,token:&CancellationToken)->Result<()> {
    copy_pinned(source,target,pin,maximum,token)?;
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(target,std::fs::Permissions::from_mode(0o600)).map_err(|_|invalid("cannot retain private proof member permissions"))?;
        std::fs::File::open(target).and_then(|f|f.sync_all()).map_err(|_|invalid("cannot sync private proof member"))?;}
    Ok(())
}
/// Explicit fresh-root retention, not stamp/upgrade/verify execution. Failures
/// leave partial evidence in place; no adoption, overwrite or cleanup occurs.
pub fn retain(request:&ProofRetentionRequest,root:&Path,token:&CancellationToken)->Result<ProofReceipt> {
    cancelled(token)?;request.validate()?;let parent=local::parent(root)?;
    for input in [&request.bundle_directory,&request.proof_file].into_iter().chain(request.previous.iter().map(|p|&p.directory)) {
        if root.starts_with(input)||input.starts_with(root){return Err(invalid("detached proof root must be disjoint from its selected inputs"));}
    }
    let (subject_path,subject)=resolve(request,token)?;let proof=identity(&request.proof_file,MAX_PROOF)?;
    if proof.0!=request.expected_proof_sha256||proof.1==0{return Err(invalid("supplied proof bytes differ from the explicit nonempty digest"));}
    let previous=request.previous.as_ref().map(|p|inspect(&p.directory,&p.receipt_sha256,token)).transpose()?;
    let receipt=ProofReceipt {schema:"aniflow.detached-proof-candidate/v1".into(),request_sha256:local::hash(&local::bytes(request)?),
        manifest_sha256:request.manifest_sha256.clone(),subject:request.subject.clone(),subject_sha256:subject.0.clone(),subject_bytes:subject.1,
        proof_sha256:proof.0.clone(),proof_bytes:proof.1,sequence:previous.as_ref().map_or(1,|p|p.sequence+1),
        previous_receipt_sha256:request.previous.as_ref().map(|p|p.receipt_sha256.clone()),previous_proof_sha256:previous.as_ref().map(|p|p.proof_sha256.clone()),
        proof_format:FORMAT.into(),subject_binding:BINDING.into(),timestamp_state:"not_verified".into(),ownership:"not_established".into()};
    validate_receipt(&receipt)?;
    if let (Some(p),Some(pin))=(&previous,&receipt.previous_receipt_sha256){linkage(&receipt,p,pin)?;}
    crate::video_assembly::require_storage(parent,subject.1+proof.1+8_388_608,request.minimum_free_bytes)?;
    cancelled(token)?;local::directory(root)?;let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    local::write(&root.join("request.json"),&local::bytes(request)?)?;
    private_copy(&subject_path,&root.join("subject.bin"),&subject,request.maximum_subject_bytes,token)?;
    private_copy(&request.proof_file,&root.join("proof.ots"),&proof,MAX_PROOF,token)?;
    if let Some(previous)=&previous {local::write(&root.join("previous.json"),&local::bytes(previous)?)?;}
    if resolve(request,token)?!=(subject_path,subject)||identity(&request.proof_file,MAX_PROOF)?!=proof{return Err(invalid("proof selection changed during retention"));}
    let mut files=vec!["request.json","subject.bin","proof.ots"];if previous.is_some(){files.push("previous.json");}
    local::entries(root,&files,&[],true)?;
    if identity(&root.join("subject.bin"),request.maximum_subject_bytes)?!=(receipt.subject_sha256.clone(),receipt.subject_bytes)
        ||identity(&root.join("proof.ots"),MAX_PROOF)?!=(receipt.proof_sha256.clone(),receipt.proof_bytes) {
        return Err(invalid("copied proof or subject changed before the completion receipt"));
    }
    cancelled(token)?;
    local::write(&root.join("receipt.json"),&local::bytes(&receipt)?)?;Ok(receipt)
}
/// Byte/one-hop receipt readback against an external pin. Original source paths
/// may be unavailable. Proof parsing, full ancestry and trusted time are absent.
pub fn inspect(root:&Path,receipt_sha256:&str,token:&CancellationToken)->Result<ProofReceipt> {
    cancelled(token)?;if !digest(receipt_sha256){return Err(invalid("proof inspection requires a known exact receipt SHA-256"));}
    ensure_canonical_directory(root)?;let receipt:ProofReceipt=read_exact(&root.join("receipt.json"),local::DOCUMENT_BYTES)?;validate_receipt(&receipt)?;
    if local::hash(&local::bytes(&receipt)?)!=receipt_sha256{return Err(invalid("proof receipt differs from its external pin"));}
    let request:ProofRetentionRequest=read_exact(&root.join("request.json"),local::DOCUMENT_BYTES)?;request.validate()?;
    if local::hash(&local::bytes(&request)?)!=receipt.request_sha256||request.manifest_sha256!=receipt.manifest_sha256||request.subject!=receipt.subject
        ||request.expected_proof_sha256!=receipt.proof_sha256||receipt.subject_bytes>request.maximum_subject_bytes
        ||request.previous.as_ref().map(|p|&p.receipt_sha256)!=receipt.previous_receipt_sha256.as_ref() {
        return Err(invalid("proof receipt differs from its retained private selection"));
    }
    let mut files=vec!["request.json","subject.bin","proof.ots","receipt.json"];
    if let Some(pin)=&receipt.previous_receipt_sha256 {let previous:ProofReceipt=read_exact(&root.join("previous.json"),local::DOCUMENT_BYTES)?;linkage(&receipt,&previous,pin)?;files.push("previous.json");}
    local::entries(root,&files,&[],false)?;
    if identity(&root.join("subject.bin"),request.maximum_subject_bytes)?!=(receipt.subject_sha256.clone(),receipt.subject_bytes)
        ||identity(&root.join("proof.ots"),MAX_PROOF)?!=(receipt.proof_sha256.clone(),receipt.proof_bytes)
        ||identity(&root.join("receipt.json"),local::DOCUMENT_BYTES)?.0!=receipt_sha256{return Err(invalid("retained proof/subject/receipt bytes changed"));}
    cancelled(token)?;Ok(receipt)
}
/// Safe explicit identity projection; no paths, supplied history or proof data.
pub fn public_summary(receipt:&ProofReceipt)->Result<serde_json::Value> {
    validate_receipt(receipt)?;
    Ok(serde_json::json!({"schema":"aniflow.detached-proof-summary/v1","manifest_sha256":receipt.manifest_sha256,
        "subject_sha256":receipt.subject_sha256,"proof_sha256":receipt.proof_sha256,"sequence":receipt.sequence,
        "subject_binding":BINDING,"timestamp_state":"not_verified","ownership":"not_established"}))
}

#[cfg(test)]
mod tests;
