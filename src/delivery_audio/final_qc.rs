//! Exact-final-file audio observation. Existing native children own acceptance;
//! this fixed private composition never rewrites media or grants musical approval.
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use crate::destination::{local,invalid,cancelled};
use crate::final_video::{FinalVideoArtifact,FinalVideoHandoff,FinalVideoVariant};
use crate::frame_io::{identity,ensure_canonical_directory};
use crate::provider::canonical_sha256;
use crate::temporal::{StreamSelection,AudioExtentPolicy};
use crate::video_assembly::AssemblyConfiguration;
use crate::{CancellationToken,Result};
use super::source_signal::{VideoAudioSignalRequest,VideoAudioSignalReceipt};
pub mod resumable;

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalAudioQcRequest {
    pub schema:String,pub source_run_directory:PathBuf,pub artifact_id:String,
    pub configuration:AssemblyConfiguration,pub provider_executable:PathBuf,pub maximum_pcm_bytes:u64,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalAudioQcIntent {
    pub schema:String,pub request:FinalAudioQcRequest,pub completion:FinalVideoHandoff,
    pub artifact:FinalVideoArtifact,pub signal_request:VideoAudioSignalRequest,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalAudioQcReport {
    pub schema:String,pub intent:FinalAudioQcIntent,pub intent_sha256:String,
    pub signal_receipt_sha256:String,pub signal:VideoAudioSignalReceipt,
    pub accepted_pcm:super::DeliveryLoudnessSummary,pub normalization:String,
    pub creative_approval:String,pub runtime_qualified:bool,pub publication_authority:String,
}
impl FinalAudioQcRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>crate::destination::MAXIMUM_REQUEST_BYTES{return Err(invalid("final audio QC request exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"final audio QC request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        self.configuration.validate()?;
        if self.schema!="aniflow.final-audio-qc-request/v1"||!crate::destination::identifier(&self.artifact_id)
            ||!(1024..=crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&self.maximum_pcm_bytes) {
            return Err(invalid("final audio QC requires an exact selected artifact and finite complete PCM bound"));
        }Ok(())
    }
}
fn selected(completion:&FinalVideoHandoff,id:&str)->Result<FinalVideoArtifact> {
    let artifact=completion.artifacts.iter().find(|a|a.artifact_id==id).cloned().ok_or_else(||invalid("final audio QC artifact is not currently accepted"))?;
    let audio=artifact.audio.as_ref().ok_or_else(||invalid("final audio QC requires real accepted audio; silence/missing audio is not invented"))?;
    if artifact.variant==FinalVideoVariant::SubtitleMuxedCandidate||audio.codec!="aac"||audio.profile!="LC"||audio.sample_rate_hz!=48_000
        ||!matches!(audio.channels,1|2)||audio.interval.duration().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?<=crate::temporal::RationalTime::ZERO {
        return Err(invalid("initial final audio QC requires a complete native 48 kHz mono/stereo AAC-LC picture/audio pair without soft subtitles"));
    }
    crate::provider::require_sha256(&audio.packet_configuration_timing_sha256,"final audio payload/time identity")?;
    Ok(artifact)
}
/// Current exact native final bytes, with no process execution.
pub fn plan(request:&FinalAudioQcRequest,token:&CancellationToken)->Result<FinalAudioQcIntent> {
    request.validate()?;cancelled(token)?;
    let completion=crate::final_video::observe(&request.source_run_directory)?;
    let artifact=selected(&completion,&request.artifact_id)?;
    if artifact.byte_count>request.configuration.maximum_source_bytes{return Err(invalid("final audio QC source exceeds selected input bound"));}
    let mut configuration=request.configuration.clone();configuration.delivery_metadata=None;
    let signal_request=VideoAudioSignalRequest {schema:"aniflow.video-audio-signal-request/v1".into(),source_video:artifact.locator.clone(),
        source_sha256:artifact.sha256.clone(),source_bytes:artifact.byte_count,
        selection:StreamSelection {video_stream:Some(0),audio_stream:Some(1),no_audio:false,discard_streams:vec![],audio_extent:Some(AudioExtentPolicy::PreserveExact)},
        configuration,provider_executable:request.provider_executable.clone(),maximum_pcm_bytes:request.maximum_pcm_bytes};
    cancelled(token)?;Ok(FinalAudioQcIntent {schema:"aniflow.final-audio-qc-intent/v1".into(),request:request.clone(),completion,artifact,signal_request})
}
fn bind(intent:&FinalAudioQcIntent,signal:&VideoAudioSignalReceipt)->Result<()> {
    let audio=intent.artifact.audio.as_ref().ok_or_else(||invalid("final QC lost selected native audio"))?;
    if signal.source_sha256!=intent.artifact.sha256||signal.source_bytes!=intent.artifact.byte_count||signal.audio_stream!=1
        ||signal.source_audio_extent!=audio.interval||signal.sample_rate_hz!=audio.sample_rate_hz||u64::from(signal.channels)!=audio.channels {
        return Err(invalid("decoded final-file QC differs from the current native final audio subject"));
    }Ok(())
}
/// Fresh only. Partial child work remains available for explicit reconciliation;
/// a missing final marker is never adopted as a successful measurement.
pub fn run(request:&FinalAudioQcRequest,root:&Path,token:&CancellationToken)->Result<FinalAudioQcReport> {
    let intent=plan(request,token)?;local::parent(root)?;
    local::disjoint(root,&crate::destination::MediaInput::AcceptedVideo {run_directory:request.source_run_directory.clone(),artifact_id:request.artifact_id.clone()},&intent.artifact.locator)?;
    for input in [&request.configuration.ffmpeg.executable,&request.configuration.ffprobe.executable,&request.provider_executable] {
        if root.starts_with(input)||input.starts_with(root){return Err(invalid("final QC must be separate from selected tool inputs"));}
    }
    local::directory(root)?;local::write(&root.join("intent.json"),&local::bytes(&intent)?)?;
    let child=root.join("signal");let signal=super::source_signal::measure(&intent.signal_request,&child,token)?;bind(&intent,&signal)?;
    let pin=identity(&child.join("video-audio-signal.json"),local::DOCUMENT_BYTES)?.0;
    let (observed,accepted_pcm)=super::source_signal::observe(&intent.signal_request,&child,&pin,token)?;
    if observed!=signal||plan(request,token)?!=intent{return Err(invalid("final native subject changed during QC"));}
    let report=FinalAudioQcReport {schema:"aniflow.final-audio-qc/v1".into(),intent_sha256:canonical_sha256(&intent)?,intent,
        signal_receipt_sha256:pin,signal,accepted_pcm,normalization:"none".into(),creative_approval:"not_performed; measurements are not musical quality review".into(),
        runtime_qualified:false,publication_authority:"none".into()};
    local::write(&root.join("qc.json"),&local::bytes(&report)?)?;Ok(report)
}
/// Caller supplies the receipt digest retained separately from this directory.
pub fn observe(root:&Path,receipt_sha256:&str,token:&CancellationToken)->Result<FinalAudioQcReport> {
    if resumable::recognized(root)? {return resumable::observe(root,receipt_sha256,token);}
    cancelled(token)?;ensure_canonical_directory(root)?;crate::provider::require_sha256(receipt_sha256,"final audio QC receipt")?;
    local::entries(root,&["intent.json","qc.json"],&["signal"],false)?;
    let path=root.join("qc.json");let pin=identity(&path,local::DOCUMENT_BYTES)?;
    if pin.0!=receipt_sha256{return Err(invalid("final audio QC receipt differs from the external digest"));}
    let report:FinalAudioQcReport=local::read(&path)?;let intent:FinalAudioQcIntent=local::read(&root.join("intent.json"))?;
    if report.schema!="aniflow.final-audio-qc/v1"||report.intent!=intent||report.intent_sha256!=canonical_sha256(&intent)?
        ||report.normalization!="none"||report.creative_approval!="not_performed; measurements are not musical quality review"
        ||report.runtime_qualified||report.publication_authority!="none"||plan(&intent.request,token)?!=intent {
        return Err(invalid("final audio QC no longer describes the exact current final-file completion"));
    }
    let (signal,summary)=super::source_signal::observe(&intent.signal_request,&root.join("signal"),&report.signal_receipt_sha256,token)?;
    bind(&intent,&signal)?;
    if signal!=report.signal||summary!=report.accepted_pcm||identity(&path,local::DOCUMENT_BYTES)?!=pin
        ||plan(&intent.request,token)?!=intent{return Err(invalid("final audio QC child/final bytes changed during observation"));}
    cancelled(token)?;Ok(report)
}

#[cfg(test)]
mod tests;
