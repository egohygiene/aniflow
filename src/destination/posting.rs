//! One explicitly selected local posting copy; never a destination uploader.
use super::*;
use super::local;
use std::path::Path;
use serde::{Deserialize,Serialize};
use crate::music_video_recipe::manual::{ManualReview,ReviewDecision};
use crate::frame_io::{identity,copy_pinned};

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicReleaseMetadata {
    pub title:String,pub creator:String,pub release_id:String,pub version:String,pub credits:String,
    pub description:Option<String>,pub artwork_references:Vec<String>,pub provenance_reference:Option<String>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum MetadataScopePolicy {RequireCleanWithinDeclaredScope,AcknowledgeInheritedOrUninspectedScope}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostingRequest {
    pub schema:String,pub preflight:PreflightReport,pub public_metadata:PublicReleaseMetadata,pub metadata_scope_policy:MetadataScopePolicy,
    pub maximum_package_bytes:u64,pub minimum_free_bytes:u64,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostingPlan {
    pub schema:String,pub request_sha256:String,pub review_context_sha256:String,pub review_subject_sha256:String,
    pub media_filename:String,pub projected_package_bytes:u64,pub metadata_scope:String,pub publication_authority:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicMedia {pub filename:String,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicPostingManifest {
    pub schema:String,pub destination:Destination,pub route:DeliveryRoute,pub media:PublicMedia,pub release:PublicReleaseMetadata,
    pub metadata_scope:String,pub review_state:String,pub publication_authority:String,pub proof_state:String,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum MemberVisibility {Public,Private}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMember {pub path:String,pub sha256:String,pub byte_count:u64,pub visibility:MemberVisibility}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostingReceipt {
    pub schema:String,pub package_id:String,pub preflight_sha256:String,pub review_context_sha256:String,pub review_subject_sha256:String,
    pub review_sha256:String,pub members:Vec<PackageMember>,pub publication_authority:String,pub proof_state:String,
}
fn public_url(value:&str)->bool {value.len()<=2048&&value.starts_with("https://")&&value.len()>8&&!value.chars().any(|c|c.is_control()||c.is_whitespace())&&!value.contains('@')&&value[8..].split('/').next().is_some_and(|s|!s.is_empty())}
fn text(value:&str,maximum:usize,required:bool)->bool {(!required||!value.trim().is_empty())&&value.len()<=maximum&&!value.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\t'))}
impl PublicReleaseMetadata {
    /// Shared admission before a publishing recipe starts expensive children.
    pub fn validate(&self)->Result<()> {
        let m=self;
        if !text(&m.title,256,true)||!text(&m.creator,256,true)||!identifier(&m.release_id)||!identifier(&m.version)||!text(&m.credits,4096,false)
            ||m.description.as_ref().is_some_and(|v|!text(v,8192,false))||m.artwork_references.len()>8||m.artwork_references.iter().any(|u|!public_url(u))
            ||m.artwork_references.iter().enumerate().any(|(i,u)|m.artwork_references[..i].contains(u))||m.provenance_reference.as_ref().is_some_and(|u|!public_url(u)) {
            return Err(invalid("posting requires explicit bounded public metadata and references"));
        }Ok(())
    }
}
impl PostingRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len() as u64>local::DOCUMENT_BYTES{return Err(invalid("posting request exceeds 2 MiB"));}
        let value:Self=decode_json(bytes,"posting request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        self.public_metadata.validate()?;
        if self.schema!="aniflow.posting-request/v1"||!(1..=1_099_511_627_776).contains(&self.maximum_package_bytes)||self.minimum_free_bytes>1_099_511_627_776
            {return Err(invalid("posting requires explicit bounded public metadata, references and package limits"));}
        local::bytes(self)?;Ok(())
    }
}
fn scope(request:&PostingRequest)->Result<String> {
    use crate::delivery_metadata::MetadataInspectionStatus;
    let summary=match &request.preflight.payload.media.facts {MediaFacts::Video {completion,artifact_id}|MediaFacts::VideoWithQc {completion,artifact_id,..}=>video(completion,artifact_id)?.metadata.as_ref(),_=>None};
    match summary.map(|s|s.status) {
        Some(MetadataInspectionStatus::CleanWithinScope) if summary.is_some_and(|s|s.mode!=crate::delivery_metadata::MetadataMode::Preserve)=>Ok("clean_within_declared_scope; opaque/signature/signal exclusions remain".into()),
        Some(MetadataInspectionStatus::ResidualFields|MetadataInspectionStatus::Failed)=>Err(invalid("metadata inspection has residual or failed fields; a declaration cannot bypass it")),
        _ if request.metadata_scope_policy==MetadataScopePolicy::AcknowledgeInheritedOrUninspectedScope=>Ok("operator_acknowledged_inherited_or_uninspected_scope; no sanitation or credential-validity claim".into()),
        _=>Err(invalid("media metadata is not sanitized within a declared scope; choose an accepted scoped copy or explicitly acknowledge inherited/uninspected scope")),
    }
}
fn filename(request:&PostingRequest)->&'static str {match &request.preflight.payload.media.facts {MediaFacts::Video {..}|MediaFacts::VideoWithQc {..}=>"delivery.mp4",MediaFacts::Pcm {..}=>"audio.wav",MediaFacts::LosslessAudio {report}=>report.operation.configuration.format.filename()}}
fn public_manifest(request:&PostingRequest,plan:&PostingPlan)->PublicPostingManifest {
    let file=&request.preflight.payload.media.file;
    PublicPostingManifest {schema:"aniflow.public-posting-package/v1".into(),destination:request.preflight.payload.request.profile.destination,route:DeliveryRoute::Manual,
        media:PublicMedia {filename:plan.media_filename.clone(),sha256:file.sha256.clone(),byte_count:file.byte_count},release:request.public_metadata.clone(),metadata_scope:plan.metadata_scope.clone(),
        review_state:"operator_declared_for_exact_file_and_metadata; not authenticated identity".into(),publication_authority:"none; manual posting remains separate".into(),proof_state:"not_asserted; supplied provenance references are not verified proofs".into()}
}
fn build_plan(request:&PostingRequest)->Result<PostingPlan> {
    request.validate()?;
    if request.preflight.payload.disposition!=PreflightDisposition::MatchesSuppliedProfile{return Err(invalid("posting package needs matching explicit requirements with no missing evidence"));}
    let request_sha256=canonical_sha256(request)?;
    let mut plan=PostingPlan {schema:"aniflow.posting-plan/v1".into(),review_context_sha256:canonical_sha256(&("aniflow.posting-review-context/v1",&request_sha256))?,
        review_subject_sha256:request.preflight.payload.media.file.sha256.clone(),request_sha256,media_filename:filename(request).into(),projected_package_bytes:0,
        metadata_scope:scope(request)?,publication_authority:"none".into()};
    let size=request.preflight.payload.media.file.byte_count.checked_add(local::bytes(request)?.len() as u64)
        .and_then(|n|n.checked_add(131_072)).ok_or_else(||invalid("posting storage bound overflow"))?;
    let public=local::bytes(&public_manifest(request,&plan))?;
    plan.projected_package_bytes=size.checked_add(public.len() as u64).ok_or_else(||invalid("posting storage bound overflow"))?;
    if plan.projected_package_bytes>request.maximum_package_bytes{return Err(invalid("posting copy/evidence exceeds the explicit package byte bound"));}Ok(plan)
}
/// Expose exact review bindings, without writing or making an approval.
pub fn plan(request:&PostingRequest,token:&CancellationToken)->Result<PostingPlan> {cancelled(token)?;request.validate()?;recheck(&request.preflight,token)?;build_plan(request)}
fn review(plan:&PostingPlan,declaration:&ManualReview)->Result<()> {
    declaration.validate()?;
    if declaration.decision!=ReviewDecision::Approve||declaration.context_sha256!=plan.review_context_sha256||declaration.subject_sha256!=plan.review_subject_sha256 {return Err(invalid("posting needs a supplied approving declaration for its exact file, metadata, profile and scope context"));}Ok(())
}
fn member(path:&str,bytes:&[u8],visibility:MemberVisibility)->PackageMember {PackageMember {path:path.into(),sha256:local::hash(bytes),byte_count:bytes.len() as u64,visibility}}
fn receipt(request:&PostingRequest,plan:&PostingPlan,declaration:&ManualReview)->Result<PostingReceipt> {
    let file=&request.preflight.payload.media.file;let mut members=vec![
        PackageMember {path:format!("public/{}",plan.media_filename),sha256:file.sha256.clone(),byte_count:file.byte_count,visibility:MemberVisibility::Public},
        member("public/release.json",&local::bytes(&public_manifest(request,plan))?,MemberVisibility::Public),
        member("private/request.json",&local::bytes(request)?,MemberVisibility::Private),
        member("private/review.json",&local::bytes(declaration)?,MemberVisibility::Private),
    ];members.sort_by(|a,b|a.path.cmp(&b.path));
    Ok(PostingReceipt {schema:"aniflow.posting-receipt/v1".into(),package_id:plan.request_sha256.clone(),preflight_sha256:request.preflight.preflight_sha256.clone(),
        review_context_sha256:plan.review_context_sha256.clone(),review_subject_sha256:plan.review_subject_sha256.clone(),review_sha256:canonical_sha256(declaration)?,members,publication_authority:"none".into(),proof_state:"not_asserted".into()})
}
/// Copy selected already accepted bytes and explicit public fields into a fresh
/// private root. Never encode, replace a source, fetch references or post.
pub fn create(request:&PostingRequest,declaration:&ManualReview,root:&Path,token:&CancellationToken)->Result<PostingReceipt> {
    let plan=plan(request,token)?;review(&plan,declaration)?;
    let parent=local::parent(root)?;let media=&request.preflight.payload.media;
    local::disjoint(root,&media.input,&media.file.locator)?;
    crate::video_assembly::require_storage(parent,plan.projected_package_bytes,request.minimum_free_bytes)?;
    cancelled(token)?;local::directory(root)?;local::sync(parent)?;
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    local::directory(&root.join("public"))?;local::directory(&root.join("private"))?;
    local::write(&root.join("private/request.json"),&local::bytes(request)?)?;
    local::write(&root.join("private/review.json"),&local::bytes(declaration)?)?;
    copy_pinned(&media.file.locator,&root.join("public").join(&plan.media_filename),&(media.file.sha256.clone(),media.file.byte_count),request.maximum_package_bytes,token)?;
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;let copy=root.join("public").join(&plan.media_filename);std::fs::set_permissions(&copy,std::fs::Permissions::from_mode(0o600)).map_err(|_|invalid("cannot retain private copy permissions"))?;std::fs::File::open(copy).and_then(|f|f.sync_all()).map_err(|_|invalid("cannot sync private copy permissions"))?;}
    local::write(&root.join("public/release.json"),&local::bytes(&public_manifest(request,&plan))?)?;
    recheck(&request.preflight,token)?;let value=receipt(request,&plan,declaration)?;
    check_members(root,request,&plan,&value)?;cancelled(token)?;
    local::sync(&root.join("public"))?;local::sync(&root.join("private"))?;
    local::entries(root,&[],&["public","private"],true)?;
    local::write(&root.join("package.json"),&local::bytes(&value)?)?;
    local::sync(root)?;Ok(value)
}
fn check_members(root:&Path,request:&PostingRequest,plan:&PostingPlan,receipt:&PostingReceipt)->Result<()> {
    for m in &receipt.members {if identity(&root.join(&m.path),request.maximum_package_bytes.max(local::DOCUMENT_BYTES))?!=(m.sha256.clone(),m.byte_count){return Err(invalid("posting member changed or differs from its exact selected bytes"));}}
    local::entries(&root.join("public"),&[plan.media_filename.as_str(),"release.json"],&[],false)?;
    local::entries(&root.join("private"),&["request.json","review.json"],&[],false)
}
/// Strict private current verification, including producer acceptance. Losing
/// originals does not delete package bytes, but cannot assert current readiness.
pub fn verify(root:&Path,token:&CancellationToken)->Result<PostingReceipt> {
    cancelled(token)?;local::entries(root,&["package.json"],&["public","private"],true)?;
    let request:PostingRequest=local::read(&root.join("private/request.json"))?;
    let declaration:ManualReview=local::read(&root.join("private/review.json"))?;
    let current:PostingReceipt=local::read(&root.join("package.json"))?;
    let plan=plan(&request,token)?;review(&plan,&declaration)?;
    let expected=receipt(&request,&plan,&declaration)?;if current!=expected{return Err(invalid("posting receipt differs from its fixed request, declaration or membership"));}
    check_members(root,&request,&plan,&current)?;recheck(&request.preflight,token)?;cancelled(token)?;Ok(current)
}

#[cfg(test)]
mod tests;
