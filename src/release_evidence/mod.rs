//! Local exact-byte retention. No signing, timestamp networking or posting.
mod types;
mod storage;
mod observations;
pub mod detached;
pub mod ots;
pub mod handoff;
pub use types::*;
pub use storage::{seal,verify};
pub use observations::record_verification;
use std::path::{Path,PathBuf};
use std::collections::BTreeSet;
use serde::{Serialize,de::DeserializeOwned};
use crate::{Result,CancellationToken,destination::{self,local,invalid,cancelled}};
use crate::destination::posting::{self,MemberVisibility,PostingReceipt};
use crate::frame_io::{identity,ensure_canonical_directory};
const MAXIMUM_BYTES:u64=1_099_511_627_776;
const HEADROOM:u64=8_388_608;
fn identifier(value:&str)->bool {
    !value.is_empty()&&value.len()<=64&&value.as_bytes()[0].is_ascii_alphanumeric()
        &&value.bytes().all(|c|c.is_ascii_alphanumeric()||b"-_.".contains(&c))
}
fn digest(value:&str)->bool {value.len()==64&&value.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c))}
impl EvidenceRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len() as u64>local::DOCUMENT_BYTES{return Err(invalid("release evidence request exceeds 2 MiB"));}
        let value:Self=crate::provider::decode_json(bytes,"release evidence request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.release-evidence-request/v1"||!identifier(&self.release_id)||!identifier(&self.version)
            ||self.subjects.is_empty()||self.subjects.len()>64||!(1..=MAXIMUM_BYTES).contains(&self.maximum_bundle_bytes)||self.minimum_free_bytes>MAXIMUM_BYTES {
            return Err(invalid("release evidence needs a bounded version, explicit selections and finite storage policy"));
        }
        let mut seen=BTreeSet::new();
        for subject in &self.subjects {
            if !identifier(&subject.id)||seen.contains(&subject.id)||subject.parents.len()>8
                ||subject.parents.iter().any(|p|!seen.contains(p))
                ||subject.parents.iter().enumerate().any(|(i,p)|subject.parents[..i].contains(p)) {
                return Err(invalid("subject IDs must be unique and lineage may reference distinct earlier subjects only"));
            }
            seen.insert(subject.id.clone());
            if let SubjectInput::DeclaredFile {expected_sha256,maximum_bytes,declared_origin,..}=&subject.input {
                if !digest(expected_sha256)||!(1..=MAXIMUM_BYTES).contains(maximum_bytes)||declared_origin.trim().is_empty()
                    ||declared_origin.len()>2048||declared_origin.chars().any(char::is_control) {
                    return Err(invalid("declared files need an exact SHA-256, finite limit and bounded explicit history"));
                }
            }
        }
        local::bytes(self)?;Ok(())
    }
}
fn posting_member(receipt:&PostingReceipt,member:PostingMember)->Result<&posting::PackageMember> {
    let matches:Vec<_>=receipt.members.iter().filter(|m|m.visibility==MemberVisibility::Public&&match member {
        PostingMember::Media=>matches!(m.path.as_str(),"public/delivery.mp4"|"public/audio.wav"|"public/audio.flac"),
        PostingMember::PublicMetadata=>m.path=="public/release.json",
    }).collect();
    if matches.len()!=1{return Err(invalid("posting subject must select exactly one fixed public package member"));}Ok(matches[0])
}
fn source(origin:&SubjectOrigin)->Result<PathBuf> {Ok(match origin {
    SubjectOrigin::AcceptedMedia {observation}=>observation.file.locator.clone(),
    SubjectOrigin::PostingMember {directory,member,receipt,..}=>directory.join(&posting_member(receipt,*member)?.path),
    SubjectOrigin::DeclaredFile {locator,..}=>locator.clone(),
})}
fn resolve(selection:&SubjectSelection,index:usize,token:&CancellationToken)->Result<SubjectRecord> {
    cancelled(token)?;
    let (sha256,byte_count,origin)=match &selection.input {
        SubjectInput::AcceptedMedia {input}=>{
            let observation=destination::observe_media(input,token)?;
            (observation.file.sha256.clone(),observation.file.byte_count,SubjectOrigin::AcceptedMedia {observation})
        },
        SubjectInput::PostingMember {directory,member}=>{
            ensure_canonical_directory(directory)?;let marker=directory.join("package.json");let before=identity(&marker,local::DOCUMENT_BYTES)?;
            let receipt=posting::verify(directory,token)?;
            if identity(&marker,local::DOCUMENT_BYTES)?!=before{return Err(invalid("posting receipt changed while selecting a release subject"));}
            let selected=posting_member(&receipt,*member)?;let file=directory.join(&selected.path);
            let current=identity(&file,MAXIMUM_BYTES)?;
            if current!=(selected.sha256.clone(),selected.byte_count){return Err(invalid("posting subject changed during selection"));}
            (current.0,current.1,SubjectOrigin::PostingMember {directory:directory.clone(),member:*member,receipt,receipt_sha256:before.0})
        },
        SubjectInput::DeclaredFile {file,expected_sha256,maximum_bytes,declared_origin}=>{
            let current=identity(file,*maximum_bytes)?;
            if current.0!=*expected_sha256{return Err(invalid("declared subject differs from the supplied SHA-256"));}
            (current.0,current.1,SubjectOrigin::DeclaredFile {locator:file.clone(),declared_origin:declared_origin.clone(),technical_acceptance:"not_established; history is operator_declared".into()})
        },
    };
    Ok(SubjectRecord {id:selection.id.clone(),role:selection.role,visibility:selection.visibility,parents:selection.parents.clone(),
        path:format!("subjects/{index:04}-{}.bin",selection.id),sha256,byte_count,origin})
}
fn subject_bytes(manifest:&SubjectManifest)->Result<u64> {
    manifest.subjects.iter().try_fold(0u64,|n,s|n.checked_add(s.byte_count).ok_or_else(||invalid("subject byte count overflow")))
}
fn projected(request:&EvidenceRequest,manifest:&SubjectManifest)->Result<u64> {
    let manifest_bytes=local::bytes(manifest)?.len() as u64;
    let total=subject_bytes(manifest)?.checked_add(local::bytes(request)?.len() as u64)
        .and_then(|n|n.checked_add(manifest_bytes))
        .and_then(|n|n.checked_add(HEADROOM)).ok_or_else(||invalid("release manifest or storage bound exceeded"))?;
    if total>request.maximum_bundle_bytes{return Err(invalid("retained subjects and separate evidence headroom exceed the bundle limit"));}Ok(total)
}
/// Resolve actual selected sources without writing, signing or approving them.
pub fn plan(request:&EvidenceRequest,token:&CancellationToken)->Result<EvidencePlan> {
    cancelled(token)?;request.validate()?;let mut subjects=Vec::new();
    for (index,selection) in request.subjects.iter().enumerate(){subjects.push(resolve(selection,index,token)?);}
    let manifest=SubjectManifest {schema:"aniflow.release-subjects/v1".into(),release_id:request.release_id.clone(),version:request.version.clone(),
        request_sha256:local::hash(&local::bytes(request)?),subjects,creative_approval:"not_performed".into(),ownership:"not_established".into(),publication_authority:"none".into()};
    let bytes=local::bytes(&manifest)?;let projected_bundle_bytes=projected(request,&manifest)?;cancelled(token)?;
    Ok(EvidencePlan {schema:"aniflow.release-evidence-plan/v1".into(),manifest,manifest_sha256:local::hash(&bytes),manifest_byte_count:bytes.len() as u64,projected_bundle_bytes})
}
fn read_exact<T:Serialize+DeserializeOwned>(path:&Path,maximum:u64)->Result<T> {
    let before=identity(path,maximum)?;let value:T=local::read_bounded(path,maximum)?;
    let bytes=local::bytes(&value)?;
    if (local::hash(&bytes),bytes.len() as u64)!=before||identity(path,maximum)?!=before{return Err(invalid("release evidence is changed or not its exact canonical document bytes"));}Ok(value)
}
fn receipt(manifest:&SubjectManifest)->Result<SealReceipt> {
    let bytes=local::bytes(manifest)?;
    Ok(SealReceipt {schema:"aniflow.release-seal/v1".into(),subject_manifest_sha256:local::hash(&bytes),subject_manifest_byte_count:bytes.len() as u64,
        request_sha256:manifest.request_sha256.clone(),subject_count:manifest.subjects.len() as u32,subject_bytes:subject_bytes(manifest)?,
        signature_state:"not_signed".into(),timestamp_state:"not_requested".into(),publication_authority:"none".into()})
}
/// Explicit allowlist only. Declaring a member public is not permission to post.
pub fn public_summary(manifest:&SubjectManifest)->serde_json::Value {
    let public:BTreeSet<_>=manifest.subjects.iter().filter(|s|s.visibility==MemberVisibility::Public).map(|s|s.id.as_str()).collect();
    serde_json::json!({"schema":"aniflow.public-release-subjects/v1","release_id":manifest.release_id,"version":manifest.version,
        "subjects":manifest.subjects.iter().filter(|s|s.visibility==MemberVisibility::Public).map(|s|serde_json::json!({"id":s.id,"role":s.role,
            "sha256":s.sha256,"byte_count":s.byte_count,"parents":s.parents.iter().filter(|p|public.contains(p.as_str())).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "ownership":"not_established","timestamp_state":"not_verified","publication_authority":"none"})
}
#[cfg(test)]
mod tests;
