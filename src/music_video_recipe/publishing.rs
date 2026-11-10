//! Fixed publishing continuation over existing native media, QC, preflight and
//! copy-only package owners. No DAG, second acceptance DB or remote publication.
use std::path::{Path,PathBuf};
use serde::{Serialize,Deserialize};
use crate::{CancellationToken,Result,PipelineV3Plan,PipelineV3RunProgress};
use crate::destination::{self,local,video,posting,MediaInput,PreflightReport,PreflightRequest,PreflightDisposition};
use crate::delivery_audio::final_qc::{self,FinalAudioQcRequest,resumable as resumable_video_qc};
use crate::final_video::{self,FinalVideoHandoff,FinalVideoArtifact,FinalVideoVariant};
use crate::frame_io::{identity,ensure_canonical_directory};
use crate::provider::canonical_sha256;
use super::{state,workspace,manual::{ManualReview,ReviewDecision}};
pub mod audio;
mod common;
mod types;
pub use types::*;
mod readback;
pub use readback::status;
mod execution;
pub use execution::{run,resume,record_review};
#[cfg(test)]
mod tests;
fn cancelled(token:&CancellationToken)->Result<()> {destination::cancelled(token)}
fn selected<'a>(request:&PublishingRequest,source:&'a FinalVideoHandoff)->Result<&'a FinalVideoArtifact> {
    let artifact=source.artifacts.iter().find(|a|a.artifact_id==request.artifact_id).ok_or_else(||state("publishing source artifact is missing"))?;
    if artifact.variant==FinalVideoVariant::SubtitleMuxedCandidate||request.export.is_none()&&artifact.variant!=FinalVideoVariant::DestinationCopy {
        return Err(state("initial publishing requires a plain destination candidate or an explicitly selected export from an accepted master; soft subtitles need a compatible future destination lane"));
    }
    if request.audio_qc.is_some()&&artifact.audio.is_none(){return Err(state("selected publishing audio QC cannot invent missing audio"));}
    Ok(artifact)
}
fn export_request(request:&PublishingRequest)->Result<video::VideoExportRequest> {
    Ok(video::VideoExportRequest {schema:"aniflow.destination-video-request/v1".into(),source_run_directory:request.video_run_directory.clone(),artifact_id:request.artifact_id.clone(),
        configuration:request.export.clone().ok_or_else(||state("publishing export was not selected"))?,provider_executable:request.provider_executable.clone()})
}
fn qc_request(request:&PublishingRequest,source:&MediaInput)->Result<FinalAudioQcRequest> {
    let MediaInput::AcceptedVideo {run_directory,artifact_id}=source else{return Err(state("publishing QC needs one exact current native final video"));};
    let options=request.audio_qc.as_ref().ok_or_else(||state("publishing QC was not selected"))?;
    Ok(FinalAudioQcRequest {schema:"aniflow.final-audio-qc-request/v1".into(),source_run_directory:run_directory.clone(),artifact_id:artifact_id.clone(),
        configuration:options.configuration.clone(),provider_executable:request.provider_executable.clone(),maximum_pcm_bytes:options.maximum_pcm_bytes})
}
fn video_v2(request:&PublishingRequest)->bool {request.schema=="aniflow.publishing-request/v2"}
fn publishing_schema(request:&PublishingRequest,part:&str)->String {format!("aniflow.publishing-{part}/{}",if video_v2(request){"v2"}else{"v1"})}
fn resumable_request(qc:FinalAudioQcRequest)->resumable_video_qc::Request {resumable_video_qc::Request {schema:"aniflow.resumable-final-video-audio-qc-request/v1".into(),qc}}
fn components(request:&PublishingRequest)->Result<Vec<PublishingComponent>> {
    let mut pins=std::collections::BTreeMap::new();let provider=identity(&request.provider_executable,512*1024*1024)?;
    pins.insert(request.provider_executable.clone(),provider);
    for tools in request.export.iter().map(|e|&e.tools).chain(request.audio_qc.iter().map(|q|&q.configuration)) {
        let observed=crate::video_assembly::observe_components(tools)?;
        for (pin,value) in [&tools.ffmpeg,&tools.ffprobe].into_iter().zip(observed) {
            if pins.get(&pin.executable).is_some_and(|old|*old!=value){return Err(state("publishing component path has conflicting exact identities"));}
            pins.insert(pin.executable.clone(),value);
        }
    }
    Ok(pins.into_iter().map(|(path,(sha256,byte_count))|PublishingComponent {path,sha256,byte_count}).collect())
}
fn intent_digest(intent:&PublishingIntent)->Result<String> {
    canonical_sha256(&(&intent.schema,&intent.request,&intent.source,&intent.components,&intent.export_plan))
}
fn step(phase:PublishingPhase,state:PublishingStepState,run_directory:Option<PathBuf>,native_plan_sha256:Option<String>)->PublishingStep {
    PublishingStep {phase,state,run_directory,native_plan_sha256}
}
/// Admission is read only. No downstream output hash, review or acceptance is
/// invented; only the optional first native export plan can already be resolved.
pub fn plan(request:&PublishingRequest,token:&CancellationToken)->Result<PublishingPlan> {
    request.validate()?;cancelled(token)?;ensure_canonical_directory(&request.video_run_directory)?;
    if request.video_run_directory.join("publishing-intent.json").try_exists().map_err(|_|state("cannot inspect publishing source"))? {
        return Err(state("nested publishing roots cannot supply native media acceptance"));
    }
    let source=final_video::observe(&request.video_run_directory)?;selected(request,&source)?;let components=components(request)?;
    let export_plan=request.export.as_ref().map(|_|video::plan(&export_request(request)?,token)).transpose()?;
    if video_v2(request)&&request.export.is_none()&&request.audio_qc.is_some() {
        let selected=qc_request(request,&MediaInput::AcceptedVideo {run_directory:request.video_run_directory.clone(),artifact_id:request.artifact_id.clone()})?;resumable_video_qc::plan(&resumable_request(selected),token)?;
    }
    let mut intent=PublishingIntent {schema:publishing_schema(request,"intent"),request:request.clone(),source,components,export_plan,intent_sha256:String::new()};
    intent.intent_sha256=intent_digest(&intent)?;local::bytes(&intent)?;
    if final_video::observe(&request.video_run_directory)?!=intent.source||self::components(request)?!=intent.components{return Err(state("publishing source or tools changed during admission"));}
    let steps=phases(request).into_iter().map(|phase|step(phase,PublishingStepState::Pending,None,
        if phase==PublishingPhase::DestinationExport {intent.export_plan.as_ref().map(|p|p.plan_sha256.clone())}else{None})).collect();
    cancelled(token)?;Ok(PublishingPlan {schema:publishing_schema(request,"plan"),intent,steps})
}
fn phases(request:&PublishingRequest)->Vec<PublishingPhase> {
    let mut phases=Vec::new();if request.export.is_some(){phases.push(PublishingPhase::DestinationExport);}
    if request.audio_qc.is_some(){phases.push(PublishingPhase::FinalAudioQc);}
    phases.extend([PublishingPhase::DestinationPreflight,PublishingPhase::FinalReview,PublishingPhase::ManualPackage]);
    if request.retention.is_some(){phases.push(PublishingPhase::RetentionHandoff);}phases
}
fn export_root(root:&Path)->PathBuf {root.join("children/destination_export")}
fn qc_root(root:&Path)->PathBuf {root.join("audio-qc")}
fn package_root(root:&Path)->PathBuf {root.join("package")}
// Publishing slots include regular marker files as well as native directories.
// The enclosing exact layout still assigns and checks each filesystem kind.
fn exists(path:&Path)->Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_)=>{workspace::require_existing_canonical(path)?;Ok(true)},
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(false),Err(_)=>Err(state("cannot inspect publishing slot")),
    }
}
fn posting_request(request:&PublishingRequest,preflight:PreflightReport)->posting::PostingRequest {
    posting_request_for(&request.package,preflight)
}
fn posting_request_for(package:&PublishingPackage,preflight:PreflightReport)->posting::PostingRequest {
    posting::PostingRequest {schema:"aniflow.posting-request/v1".into(),preflight,public_metadata:package.public_metadata.clone(),
        metadata_scope_policy:package.metadata_scope_policy,maximum_package_bytes:package.maximum_package_bytes,minimum_free_bytes:package.minimum_free_bytes}
}
fn require_review(plan:&posting::PostingPlan,declaration:&ManualReview)->Result<()> {
    declaration.validate()?;
    if declaration.context_sha256!=plan.review_context_sha256||declaration.subject_sha256!=plan.review_subject_sha256 {
        return Err(state("publishing decision must name the exact current final file, public metadata, requirements, QC and scope context"));
    }Ok(())
}
/// Deterministic advice only; no probe, child execution or review creation.
pub fn next_action(root:&Path,token:&CancellationToken)->Result<PublishingNextAction> {Ok(status(root,token)?.next_action)}
