//! Separate fixed audio entry over existing native export/QC and shared manual
//! publishing owners. No uploader, general DAG or new media acceptance store.
use super::*;
use crate::delivery_audio::final_lossless_qc::{self as audio_qc,FinalLosslessSubject,FinalLosslessQcRequest};
use crate::lossless_audio::{self as native_export,LosslessConfiguration,LosslessRequest};
use audio_qc::resumable::{self as resumable_qc,ResumableQcRequest};
mod types;
pub use types::*;
mod readback;
pub use readback::status;
mod execution;
pub use execution::{run,resume,record_review};
#[cfg(test)]
mod tests;
fn source(request:&AudioPublishingRequest,token:&CancellationToken)->Result<destination::MediaObservation> {destination::observe_media(&request.subject.input(),token)}
fn export_request(request:&AudioPublishingRequest)->Result<LosslessRequest> {
    let FinalLosslessSubject::AcceptedPcm {file,signal_run_directory}=&request.subject else{return Err(state("publishing export requires explicitly accepted original PCM; re-export is not implicit"));};
    Ok(LosslessRequest {source:file.clone(),signal_run_directory:signal_run_directory.clone(),configuration:request.export.clone().ok_or_else(||state("audio export was not selected"))?,provider_executable:request.provider_executable.clone()})
}
fn qc_request(request:&AudioPublishingRequest,subject:FinalLosslessSubject)->Result<FinalLosslessQcRequest> {
    let options=request.audio_qc.as_ref().ok_or_else(||state("final audio QC was not selected"))?;
    Ok(FinalLosslessQcRequest {schema:if version2(request){"aniflow.final-lossless-audio-qc-request/v2"}else{"aniflow.final-lossless-audio-qc-request/v1"}.into(),subject,configuration:options.configuration.clone(),provider_executable:request.provider_executable.clone(),maximum_pcm_bytes:options.maximum_pcm_bytes,maximum_working_bytes:options.maximum_working_bytes,minimum_free_bytes:options.minimum_free_bytes})
}
fn version2(request:&AudioPublishingRequest)->bool {request.schema=="aniflow.audio-publishing-request/v2"}
fn schema(request:&AudioPublishingRequest,part:&str)->String {format!("aniflow.audio-publishing-{part}/{}",if version2(request){"v2"}else{"v1"})}
fn resumable_request(qc:FinalLosslessQcRequest)->ResumableQcRequest {ResumableQcRequest {schema:"aniflow.resumable-final-audio-qc-request/v1".into(),qc}}
fn components(request:&AudioPublishingRequest)->Result<Vec<PublishingComponent>> {
    let mut values=std::collections::BTreeMap::new();
    if request.export.is_some()||request.audio_qc.is_some(){values.insert(request.provider_executable.clone(),identity(&request.provider_executable,134_217_728)?);}
    for configuration in request.export.iter().map(|s|&s.tools).chain(request.audio_qc.iter().map(|s|&s.configuration)) {
        for pin in [&configuration.ffmpeg,&configuration.ffprobe] {
            crate::audio_inspection::process::verify_pin(pin,"audio publishing tool").map_err(|_|state("audio publishing explicit tool pin is unavailable"))?;
            let actual=identity(&pin.executable,134_217_728)?;
            if values.get(&pin.executable).is_some_and(|old|*old!=actual){return Err(state("audio publishing has conflicting component identities"));}values.insert(pin.executable.clone(),actual);
        }
    }
    Ok(values.into_iter().map(|(path,(sha256,byte_count))|PublishingComponent {path,sha256,byte_count}).collect())
}
fn digest(intent:&AudioPublishingIntent)->Result<String> {canonical_sha256(&(&intent.schema,&intent.request,&intent.source,&intent.components,&intent.export_plan))}
fn phases(request:&AudioPublishingRequest)->Vec<PublishingPhase> {
    let mut result=Vec::new();if request.export.is_some(){result.push(PublishingPhase::DestinationExport);}if request.audio_qc.is_some(){result.push(PublishingPhase::FinalAudioQc);}
    result.extend([PublishingPhase::DestinationPreflight,PublishingPhase::FinalReview,PublishingPhase::ManualPackage]);if request.retention.is_some(){result.push(PublishingPhase::RetentionHandoff);}result
}
/// Inspect current original acceptance/components and at most the first native
/// export plan. No future output/QC/review/package identity is invented.
pub fn plan(request:&AudioPublishingRequest,token:&CancellationToken)->Result<AudioPublishingPlan> {
    request.validate()?;cancelled(token)?;let source=source(request,token)?;
    let (summary,_,_)=audio_qc::source_facts(&source)?;
    if summary.source.stem.is_some()||summary.source.stream_index!=0||summary.source.origin.numerator!=0||summary.source.frame_count==0||summary.source.frame_count>u64::from(summary.source.sample_rate_hz)*600 {
        return Err(state("audio publishing requires complete original zero-origin native PCM within the existing ten-minute profile"));
    }
    let components=components(request)?;
    let export_plan=request.export.as_ref().map(|_|native_export::plan(&export_request(request)?,token)).transpose()?;
    if request.export.is_none()&&request.audio_qc.is_some(){
        let selected=qc_request(request,request.subject.clone())?;
        if version2(request){resumable_qc::plan(&resumable_request(selected),token)?;}else{audio_qc::plan(&selected,token)?;}
    }
    let mut intent=AudioPublishingIntent {schema:schema(request,"intent"),request:request.clone(),source,components,export_plan,intent_sha256:String::new()};intent.intent_sha256=digest(&intent)?;local::bytes(&intent)?;
    if self::source(request,token)?!=intent.source||self::components(request)?!=intent.components{return Err(state("audio publishing source/components changed during admission"));}
    let steps=phases(request).into_iter().map(|phase|step(phase,PublishingStepState::Pending,None,if phase==PublishingPhase::DestinationExport {intent.export_plan.as_ref().map(|p|p.plan_sha256.clone())}else{None})).collect();
    cancelled(token)?;Ok(AudioPublishingPlan {schema:schema(request,"plan"),intent,steps})
}
fn export_root(root:&Path)->PathBuf {root.join("children/audio_export")}
/// Advice is a read-only projection; supplying a review remains a separate API.
pub fn next_action(root:&Path,token:&CancellationToken)->Result<PublishingNextAction>{Ok(status(root,token)?.next_action)}
