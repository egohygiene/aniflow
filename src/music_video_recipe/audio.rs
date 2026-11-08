//! Fixed optional prelude. Completed observations are hash-bound, never adopted
//! from partial folders or treated as delivery/content approval.
use super::*;
use serde::{Serialize,Deserialize};
use crate::delivery_audio::source_signal::{VideoAudioSignalRequest,VideoAudioSignalReceipt};
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {schema:String,intent_sha256:String,request_sha256:String,receipt_sha256:String}
pub(super) fn request(request:&MusicVideoRequest)->Result<Option<VideoAudioSignalRequest>> {
    let Some(option)=&request.configuration.audio_signal else{return Ok(None);};
    let (index,_)=crate::video_assembly::read_index(&request.frame_index)?;
    let temporal=index.temporal();let mut configuration=request.configuration.assembly.clone();configuration.delivery_metadata=None;
    if temporal.selected.as_ref().and_then(|s|s.audio).is_none(){return Err(invalid("optional recipe audio analysis requires selected original audio; no missing stream is invented"));}
    Ok(Some(VideoAudioSignalRequest {schema:"aniflow.video-audio-signal-request/v1".into(),source_video:request.source_video.clone(),
        source_sha256:temporal.source_sha256.clone(),source_bytes:temporal.source_size_bytes,selection:temporal.selection_intent.clone(),
        configuration,provider_executable:request.provider_executable.clone(),maximum_pcm_bytes:option.maximum_pcm_bytes}))
}
pub(super) fn observe(recipe:&MusicVideoRequest,root:&Path,intent:&str,token:&CancellationToken)->Result<Option<MusicVideoAudioEvidence>> {
    let Some(request)=request(recipe)? else{return Ok(None);};cancelled(token)?;
    let output=root.join("audio-signal");let marker=root.join("audio-signal-binding.json");
    if !output.try_exists().map_err(|_|state("cannot inspect audio observation slot"))? {
        if marker.try_exists().map_err(|_|state("cannot inspect audio binding"))?{return Err(state("audio observation missing with retained binding; preserve for reconciliation"));}return Ok(None);
    }
    let binding:Binding=workspace::read_document(&marker).map_err(|_|state("audio observation initialization is incomplete; preserve it and reconcile explicitly before resume"))?;
    let receipt_path=output.join("video-audio-signal.json");let receipt_identity=identity(&receipt_path,MAXIMUM_DOCUMENT_BYTES)?;
    if binding.schema!="aniflow.recipe-audio-binding/v1"||binding.intent_sha256!=intent||binding.request_sha256!=canonical_sha256(&request)?||binding.receipt_sha256!=receipt_identity.0{return Err(state("recipe audio binding or observation changed"));}
    let receipt:VideoAudioSignalReceipt=workspace::read_document(&receipt_path)?;
    if receipt.schema!="aniflow.video-audio-signal/v1"||receipt.source_sha256!=request.source_sha256||receipt.source_bytes!=request.source_bytes
        ||Some(receipt.audio_stream)!=request.selection.audio_stream||receipt.normalization!="none"||receipt.runtime_qualified
        ||receipt.ffmpeg_sha256!=request.configuration.ffmpeg.sha256||receipt.ffprobe_sha256!=request.configuration.ffprobe.sha256
        ||receipt.provider_sha256!=identity(&request.provider_executable,1_073_741_824)?.0
        ||receipt.signal_run.contains('\\')||Path::new(&receipt.signal_run).is_absolute()||Path::new(&receipt.signal_run).components().any(|c|!matches!(c,std::path::Component::Normal(_))) {
        return Err(state("recipe audio receipt differs from exact original source/tools or fixed local scope"));
    }
    let source=(request.source_sha256.clone(),request.source_bytes);
    if identity(&request.source_video,request.configuration.maximum_source_bytes)?!=source||identity(&output.join("source.media"),request.configuration.maximum_source_bytes)?!=source
        ||identity(&output.join("selected-audio.wav"),request.maximum_pcm_bytes)?!=(receipt.decoded_wav_sha256.clone(),receipt.decoded_wav_bytes) {return Err(state("recipe audio source or retained complete PCM changed"));}
    let child=output.join(&receipt.signal_run);workspace::require_existing_canonical(&child)?;
    let accepted=crate::status_v3(&child)?;let signal=crate::delivery_audio::loudness_summary(&child)?;
    if accepted.payload.state!=crate::PipelineV3RunState::Complete||signal.report_sha256!=receipt.signal_report_sha256
        ||signal.source.artifact.sha256!=receipt.decoded_wav_sha256||signal.source.artifact.byte_size!=receipt.decoded_wav_bytes
        ||workspace::retained_plan(&child)?.plan_sha256!=receipt.signal_plan_sha256
        ||receipt.loudness.pcm_sha256!=receipt.decoded_pcm_sha256||receipt.loudness.frames!=receipt.frames||receipt.loudness.channels!=receipt.channels
        ||identity(&receipt_path,MAXIMUM_DOCUMENT_BYTES)?!=receipt_identity {return Err(state("recipe audio accepted child/receipt changed"));}
    Ok(Some(MusicVideoAudioEvidence {receipt:receipt_path,receipt_sha256:binding.receipt_sha256,signal_run:child,
        signal_plan_sha256:receipt.signal_plan_sha256,signal_report_sha256:receipt.signal_report_sha256,normalization:"none".into(),runtime_qualified:false}))
}
pub(super) fn advance(recipe:&MusicVideoRequest,root:&Path,intent:&str,token:&CancellationToken)->Result<()> {
    let Some(request)=request(recipe)? else{return Ok(());};
    if observe(recipe,root,intent,token)?.is_some(){return Ok(());}
    crate::delivery_audio::source_signal::measure(&request,&root.join("audio-signal"),token)?;
    cancelled(token)?;
    let binding=Binding {schema:"aniflow.recipe-audio-binding/v1".into(),intent_sha256:intent.into(),request_sha256:canonical_sha256(&request)?,
        receipt_sha256:identity(&root.join("audio-signal/video-audio-signal.json"),MAXIMUM_DOCUMENT_BYTES)?.0};
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(root.join("audio-signal-binding.json")).map_err(|_|state("cannot commit recipe audio binding"))?;
    use std::io::Write;file.write_all(&crate::provider::canonical_json_bytes(&binding)?).and_then(|_|file.sync_all()).map_err(|_|state("cannot sync recipe audio binding"))?;
    observe(recipe,root,intent,token)?.ok_or_else(||state("recipe audio observation did not commit"))?;Ok(())
}
