//! Local supplied-copy retention. No platform fetch, post or media acceptance.
use std::path::{Path,PathBuf};
use chrono::{DateTime,Utc};
use serde::{Deserialize,Serialize,de::DeserializeOwned};
use crate::{CancellationToken,Result,frame_io::{identity,ensure_canonical_directory,copy_pinned}};
use super::{Destination,posting::{self,PostingReceipt,MemberVisibility,PublicPostingManifest},local,invalid,cancelled};
const MAXIMUM_COPY:u64=1_073_741_824;
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Acquisition {OperatorSuppliedDownload,OperatorSuppliedRecording,Unknown}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SpotCheck {NotReviewed,Acceptable,Concerns,Unavailable}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityDeclaration {
    pub picture:SpotCheck,pub audio_stereo:SpotCheck,pub timing:SpotCheck,
    pub metadata:SpotCheck,pub watermark:SpotCheck,pub notes:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationDeclaration {
    pub url:String,pub posted_at:Option<DateTime<Utc>>,pub observed_at:DateTime<Utc>,
    pub observer:String,pub acquisition:Acquisition,pub quality:QualityDeclaration,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRequest {
    pub schema:String,pub package_directory:PathBuf,pub package_receipt_sha256:String,
    pub destination:Destination,pub declaration:PublicationDeclaration,pub observed_file:PathBuf,
    pub expected_observed_sha256:String,pub maximum_copy_bytes:u64,pub minimum_free_bytes:u64,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ByteRelationship {Identical,Different}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationReceipt {
    pub schema:String,pub request_sha256:String,pub package_receipt_sha256:String,pub package_id:String,
    pub destination:Destination,pub local_media_sha256:String,pub local_media_bytes:u64,
    pub observed_sha256:String,pub observed_bytes:u64,pub byte_relationship:ByteRelationship,
    pub declaration:PublicationDeclaration,pub platform_authentication:String,pub technical_validation:String,
    pub declaration_authority:String,pub publication_authority:String,
}
fn locator(value:&str)->bool {
    let Some(rest)=value.strip_prefix("https://")else{return false};let host=rest.split('/').next().unwrap_or("");
    value.len()<=2048&&!value.chars().any(|c|c.is_control()||c.is_whitespace())&&!value.contains(['@','?','#','\\'])
        &&host.len()<=253&&host.contains('.')&&host.split('.').all(|part|!part.is_empty()&&part.len()<=63
            &&!part.starts_with('-')&&!part.ends_with('-')&&part.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'))
}
impl ObservationRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("platform-copy request exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"platform-copy request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        crate::provider::require_sha256(&self.package_receipt_sha256,"posting receipt")?;
        crate::provider::require_sha256(&self.expected_observed_sha256,"observed copy")?;
        let d=&self.declaration;let checks=[d.quality.picture,d.quality.audio_stereo,d.quality.timing,d.quality.metadata,d.quality.watermark];
        if self.schema!="aniflow.post-upload-observation-request/v1"||!(1..=MAXIMUM_COPY).contains(&self.maximum_copy_bytes)
            ||self.minimum_free_bytes>1_099_511_627_776||!locator(&d.url)||d.posted_at.as_ref().is_some_and(|p|p>&d.observed_at)
            ||d.observer.trim().is_empty()||d.observer.len()>256||d.observer.chars().any(char::is_control)
            ||d.quality.notes.len()>4096||d.quality.notes.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\t'))
            ||checks.contains(&SpotCheck::Concerns)&&d.quality.notes.trim().is_empty() {
            return Err(invalid("platform-copy request needs bounded explicit declaration, chronology, URL and copy limits"));
        }
        if local::bytes(self)?.len()>65_536{return Err(invalid("platform-copy request exceeds 64 KiB"));}Ok(())
    }
}
fn read<T:Serialize+DeserializeOwned>(path:&Path)->Result<T> {
    let before=identity(path,local::DOCUMENT_BYTES)?;let value:T=local::read(path)?;
    if (local::hash(&local::bytes(&value)?),local::bytes(&value)?.len() as u64)!=before||identity(path,local::DOCUMENT_BYTES)?!=before {
        return Err(invalid("platform-copy evidence is changed or not exact canonical bytes"));
    }Ok(value)
}
fn media(receipt:&PostingReceipt)->Result<&posting::PackageMember> {
    if receipt.schema!="aniflow.posting-receipt/v1"||receipt.members.len()!=4
        ||receipt.publication_authority!="none"||receipt.proof_state!="not_asserted" {
        return Err(invalid("platform observation requires the fixed non-publishing posting receipt"));
    }
    for digest in [&receipt.package_id,&receipt.preflight_sha256,&receipt.review_context_sha256,&receipt.review_subject_sha256,&receipt.review_sha256] {
        crate::provider::require_sha256(digest,"posting snapshot identity")?;
    }
    let mut names=std::collections::BTreeSet::new();
    for member in &receipt.members {
        crate::provider::require_sha256(&member.sha256,"posting snapshot member")?;
        if !names.insert(member.path.as_str())||match member.visibility {
            MemberVisibility::Public=>!matches!(member.path.as_str(),"public/delivery.mp4"|"public/audio.wav"|"public/audio.flac"|"public/release.json"),
            MemberVisibility::Private=>!matches!(member.path.as_str(),"private/request.json"|"private/review.json"),
        } {return Err(invalid("posting snapshot has duplicate or unsupported membership"));}
    }
    if !["public/release.json","private/request.json","private/review.json"].iter().all(|name|names.contains(name)) {
        return Err(invalid("posting snapshot is missing fixed evidence members"));
    }
    let members:Vec<_>=receipt.members.iter().filter(|m|m.visibility==MemberVisibility::Public
        &&matches!(m.path.as_str(),"public/delivery.mp4"|"public/audio.wav"|"public/audio.flac")).collect();
    if members.len()!=1{return Err(invalid("posting package must identify one exact fixed public media member"));}Ok(members[0])
}
fn relation(local:&(String,u64),observed:&(String,u64))->Result<ByteRelationship> {
    if local.0==observed.0&&local.1!=observed.1{return Err(invalid("equal SHA-256 with contradictory byte counts"));}
    Ok(if local==observed {ByteRelationship::Identical}else{ByteRelationship::Different})
}
fn build(request:&ObservationRequest,package:&PostingReceipt,observed:(String,u64))->Result<ObservationReceipt> {
    let original=media(package)?;let byte_relationship=relation(&(original.sha256.clone(),original.byte_count),&observed)?;
    Ok(ObservationReceipt {schema:"aniflow.post-upload-observation/v1".into(),request_sha256:local::hash(&local::bytes(request)?),
        package_receipt_sha256:request.package_receipt_sha256.clone(),package_id:package.package_id.clone(),destination:request.destination,
        local_media_sha256:original.sha256.clone(),local_media_bytes:original.byte_count,observed_sha256:observed.0,observed_bytes:observed.1,
        byte_relationship,declaration:request.declaration.clone(),platform_authentication:"not_established; URL and acquisition are operator_declared".into(),
        technical_validation:"not_performed; byte comparison is not decode, quality, timing, metadata or watermark verification".into(),
        declaration_authority:"operator_supplied; timestamps and quality notes are not authenticated".into(),publication_authority:"none".into()})
}
fn current_package(request:&ObservationRequest,token:&CancellationToken)->Result<PostingReceipt> {
    let path=request.package_directory.join("package.json");let pin=identity(&path,local::DOCUMENT_BYTES)?;
    if pin.0!=request.package_receipt_sha256{return Err(invalid("posting package differs from its selected receipt digest"));}
    let package=posting::verify(&request.package_directory,token)?;
    let public:PublicPostingManifest=read(&request.package_directory.join("public/release.json"))?;
    if public.destination!=request.destination||identity(&path,local::DOCUMENT_BYTES)?!=pin{return Err(invalid("declared destination or current posting package changed"));}Ok(package)
}
/// Explicitly retain a separately supplied local file after exact package
/// recheck. This does not obtain a platform copy or assert that posting occurred.
pub fn retain(request:&ObservationRequest,root:&Path,token:&CancellationToken)->Result<ObservationReceipt> {
    cancelled(token)?;request.validate()?;let parent=local::parent(root)?;
    if root.starts_with(&request.package_directory)||request.package_directory.starts_with(root)
        ||root.starts_with(&request.observed_file)||request.observed_file.starts_with(root){return Err(invalid("platform-copy output must be separate from its inputs"));}
    let package=current_package(request,token)?;let observed=identity(&request.observed_file,request.maximum_copy_bytes)?;
    if observed.0!=request.expected_observed_sha256||observed.1==0{return Err(invalid("observed copy differs from its supplied nonempty identity"));}
    let receipt=build(request,&package,observed.clone())?;
    crate::video_assembly::require_storage(parent,observed.1+8_388_608,request.minimum_free_bytes)?;
    cancelled(token)?;local::directory(root)?;let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    local::write(&root.join("request.json"),&local::bytes(request)?)?;
    copy_pinned(&request.observed_file,&root.join("observed.bin"),&observed,request.maximum_copy_bytes,token)?;
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(root.join("observed.bin"),std::fs::Permissions::from_mode(0o600)).map_err(|_|invalid("cannot retain private observed-copy permissions"))?;
        std::fs::File::open(root.join("observed.bin")).and_then(|f|f.sync_all()).map_err(|_|invalid("cannot sync observed copy"))?;}
    if current_package(request,token)?!=package||identity(&request.observed_file,request.maximum_copy_bytes)?!=observed
        ||identity(&root.join("observed.bin"),request.maximum_copy_bytes)?!=observed{return Err(invalid("selected package or observed bytes changed during retention"));}
    local::write(&root.join("package-snapshot.json"),&local::bytes(&package)?)?;
    local::entries(root,&["request.json","observed.bin","package-snapshot.json"],&[],true)?;cancelled(token)?;
    local::write(&root.join("observation.json"),&local::bytes(&receipt)?)?;Ok(receipt)
}
/// Archival byte readback; it does not reassert current producer/platform state.
pub fn inspect(root:&Path,receipt_sha256:&str,token:&CancellationToken)->Result<ObservationReceipt> {
    cancelled(token)?;crate::provider::require_sha256(receipt_sha256,"platform observation receipt")?;ensure_canonical_directory(root)?;
    let receipt:ObservationReceipt=read(&root.join("observation.json"))?;
    if local::hash(&local::bytes(&receipt)?)!=receipt_sha256{return Err(invalid("platform-copy receipt differs from its external pin"));}
    let request:ObservationRequest=read(&root.join("request.json"))?;request.validate()?;
    let package:PostingReceipt=read(&root.join("package-snapshot.json"))?;
    let observed=identity(&root.join("observed.bin"),request.maximum_copy_bytes)?;
    if observed.0!=request.expected_observed_sha256||observed.1==0
        ||local::hash(&local::bytes(&package)?)!=request.package_receipt_sha256
        ||build(&request,&package,observed)?!=receipt{return Err(invalid("observed bytes, selected posting snapshot or declaration changed"));}
    local::entries(root,&["request.json","observed.bin","package-snapshot.json","observation.json"],&[],false)?;
    if identity(&root.join("observation.json"),local::DOCUMENT_BYTES)?.0!=receipt_sha256{return Err(invalid("platform-copy receipt changed during readback"));}
    cancelled(token)?;Ok(receipt)
}
/// Disclosure-minimized identities only. Supplied platform history stays private.
pub fn public_summary(receipt:&ObservationReceipt)->Result<serde_json::Value> {
    crate::provider::require_sha256(&receipt.local_media_sha256,"local media summary")?;
    crate::provider::require_sha256(&receipt.observed_sha256,"observed copy summary")?;
    if receipt.schema!="aniflow.post-upload-observation/v1"||relation(&(receipt.local_media_sha256.clone(),receipt.local_media_bytes),&(receipt.observed_sha256.clone(),receipt.observed_bytes))?!=receipt.byte_relationship {
        return Err(invalid("platform-copy summary requires consistent explicit byte identities"));
    }
    Ok(serde_json::json!({"schema":"aniflow.post-upload-summary/v1","destination":receipt.destination,
        "local_media_sha256":receipt.local_media_sha256,"observed_sha256":receipt.observed_sha256,"byte_relationship":receipt.byte_relationship,
        "platform_authentication":"not_established","technical_validation":"not_performed","publication_authority":"none"}))
}
#[cfg(test)]
mod tests;
