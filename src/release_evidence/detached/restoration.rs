//! Restore exact retained candidates. No cryptographic proof verification.
use super::*;
const MAX_TOTAL:u64=MAX_SUBJECT+MAX_PROOF+8_388_608;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreRequest {
    pub schema:String,pub source_directory:PathBuf,pub receipt_sha256:String,
    pub maximum_total_bytes:u64,pub minimum_free_bytes:u64,
}
impl RestoreRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{if bytes.len()>65_536{return Err(invalid("detached restore request exceeds 64 KiB"));}let value:Self=crate::provider::decode_json(bytes,"detached restore request")?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.detached-proof-restore-request/v1"||!digest(&self.receipt_sha256)
            ||!self.source_directory.is_absolute()||self.source_directory.as_os_str().len()>4096||self.source_directory.components().any(|c|matches!(c,std::path::Component::CurDir|std::path::Component::ParentDir))
            ||!(1..=MAX_TOTAL).contains(&self.maximum_total_bytes)||self.minimum_free_bytes>1_099_511_627_776 {return Err(invalid("detached restoration requires one exact retained receipt and finite local-copy bounds"));}Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreMember {pub path:String,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestorePlan {
    pub schema:String,pub request_sha256:String,pub receipt_sha256:String,pub receipt:ProofReceipt,
    pub files:Vec<RestoreMember>,pub copy_bytes:u64,pub verification_scope:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoredPair {
    pub schema:String,pub receipt_sha256:String,pub subject_sha256:String,pub proof_sha256:String,
    pub copy_bytes:u64,pub copied_files:usize,pub byte_integrity:String,
    pub subject_binding:String,pub timestamp_state:String,pub ownership:String,
}
fn maximum(name:&str)->u64 {match name {"subject.bin"=>MAX_SUBJECT,"proof.ots"=>MAX_PROOF,_=>local::DOCUMENT_BYTES}}
/// Observe only the current fixed retained candidate inventory. No original
/// bundle/production path, calendar, verifier or model is required.
pub fn plan(request:&RestoreRequest,token:&CancellationToken)->Result<RestorePlan> {
    cancelled(token)?;request.validate()?;let receipt=inspect(&request.source_directory,&request.receipt_sha256,token)?;
    let mut names=vec!["proof.ots","request.json","subject.bin"];
    if receipt.previous_receipt_sha256.is_some(){names.push("previous.json");}names.push("receipt.json");names.sort();
    let mut files=Vec::new();let mut total=0u64;
    for name in names {cancelled(token)?;let pin=identity(&request.source_directory.join(name),maximum(name))?;
        total=total.checked_add(pin.1).filter(|v|*v<=request.maximum_total_bytes).ok_or_else(||invalid("retained pair exceeds the selected total-copy budget"))?;
        files.push(RestoreMember {path:name.into(),sha256:pin.0,byte_count:pin.1});
    }
    if inspect(&request.source_directory,&request.receipt_sha256,token)?!=receipt{return Err(invalid("retained pair changed while planning restoration"));}
    Ok(RestorePlan {schema:"aniflow.detached-proof-restore-plan/v1".into(),request_sha256:local::hash(&local::bytes(request)?),receipt_sha256:request.receipt_sha256.clone(),receipt,files,copy_bytes:total,
        verification_scope:"exact retained bytes and immediate receipt linkage only; no cryptographic proof parsing, timestamp verification or full ancestry".into()})
}
/// Copy to a new private root and write the unchanged source receipt last.
/// Failures preserve partial evidence; no partial-folder adoption or overwrite.
pub fn restore(request:&RestoreRequest,root:&Path,token:&CancellationToken)->Result<RestoredPair> {
    cancelled(token)?;request.validate()?;let parent=local::parent(root)?;
    if root.starts_with(&request.source_directory)||request.source_directory.starts_with(root){return Err(invalid("restore output must be separate from its retained source"));}
    let expected=plan(request,token)?;
    crate::video_assembly::require_storage(parent,expected.copy_bytes.checked_add(8_388_608).ok_or_else(||invalid("restore storage sum overflow"))?,request.minimum_free_bytes)?;
    cancelled(token)?;local::directory(root)?;
    for file in expected.files.iter().filter(|f|f.path!="receipt.json") {
        cancelled(token)?;private_copy(&request.source_directory.join(&file.path),&root.join(&file.path),&(file.sha256.clone(),file.byte_count),maximum(&file.path),token)?;
    }
    if plan(request,token)?!=expected{return Err(invalid("retained pair changed while copying; preserve incomplete restoration"));}
    let receipt_file=expected.files.iter().find(|f|f.path=="receipt.json").ok_or_else(||invalid("restore receipt missing"))?;
    cancelled(token)?;private_copy(&request.source_directory.join("receipt.json"),&root.join("receipt.json"),&(receipt_file.sha256.clone(),receipt_file.byte_count),local::DOCUMENT_BYTES,token)?;
    local::sync(root)?;
    let actual=inspect(root,&request.receipt_sha256,token)?;
    if actual!=expected.receipt||plan(request,token)?!=expected{return Err(invalid("restored exact pair differs from its pinned source"));}
    for file in &expected.files {cancelled(token)?;if identity(&root.join(&file.path),maximum(&file.path))?!=(file.sha256.clone(),file.byte_count){return Err(invalid("restored candidate member changed during final byte check"));}}
    cancelled(token)?;Ok(RestoredPair {schema:"aniflow.detached-proof-restoration/v1".into(),receipt_sha256:request.receipt_sha256.clone(),subject_sha256:actual.subject_sha256,proof_sha256:actual.proof_sha256,
        copy_bytes:expected.copy_bytes,copied_files:expected.files.len(),byte_integrity:"matches_external_receipt_and_retained_members".into(),subject_binding:BINDING.into(),timestamp_state:"not_verified".into(),ownership:"not_established".into()})
}
#[cfg(test)] mod tests;
