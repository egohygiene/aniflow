//! Explicit attachment over existing manual lyrics/review/export owners.
//! Rendering stays separate; one accepted video is neither muxed nor rewritten.
use super::*;
use serde::{Serialize,Deserialize};
use crate::temporal::TimeRange;
use crate::timed_text::{TimedTextFormat,OverlapPolicy,ConversionOptions};
use super::lyrics::{ManualLyricsRequest,ManualLyricsDelivery};
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeLyricsOptions {pub input_format:TimedTextFormat,pub language:Option<String>,pub overlap_policy:OverlapPolicy,pub targets:Vec<TimedTextFormat>,pub conversion:ConversionOptions}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeLyricsAttachment {
    pub schema:String,pub intent_sha256:String,pub assembly_manifest_sha256:String,
    pub audio_receipt_sha256:String,pub workflow:PathBuf,pub request:ManualLyricsRequest,
    pub delivery_receipt_sha256:String,pub delivery:ManualLyricsDelivery,
}
fn resolved(root:&Path)->Result<(MusicVideoRequest,String)> {
    let token=CancellationToken::default();
    match entry_kind(root)? {
        MusicVideoEntryKind::Indexed=>{let intent=workspace::read_intent(root)?;require_intent(&intent.request(),&intent,&token)?;Ok((intent.request(),intent.intent_sha256))},
        MusicVideoEntryKind::Source=>{
            let intent=workspace::read_source_intent(root)?;let request=intent.request();let plan=source::require_source_intent(&request,&intent,&token)?;
            let child=workspace::extraction_child(root);let manifest=crate::status_v3(&child)?;
            let (indexed,_)=source_handoff::indexed(&request,&child,source::extraction_plan(&plan)?,&manifest,&token)?;Ok((indexed,intent.intent_sha256))
        },
    }
}
fn bound_request(recipe:&MusicVideoRequest,intent:&str,audio:&MusicVideoAudioEvidence,options:RecipeLyricsOptions)->Result<ManualLyricsRequest> {
    let receipt:crate::delivery_audio::source_signal::VideoAudioSignalReceipt=workspace::read_document(&audio.receipt)?;
    let (index,_)=crate::video_assembly::read_index(&recipe.frame_index)?;let temporal=index.temporal();
    // Meter observation resolves automatic selection into explicit stream IDs.
    // Keep both inspection digests honest; join actual source/stream/extent
    // facts instead of claiming their differently declared JSON is identical.
    let audio_stream=temporal.selected.as_ref().and_then(|s|s.audio);
    if temporal.source_sha256!=receipt.source_sha256||temporal.source_size_bytes!=receipt.source_bytes||audio_stream!=Some(receipt.audio_stream)
        ||audio_stream.and_then(|i|temporal.timeline(i)).and_then(|t|t.source_time)!=Some(receipt.source_audio_extent) {return Err(state("recipe lyric source audio and original timeline differ"));}
    let timeline_sha256=temporal.sha256().map_err(|e|Error::from_anyhow(ErrorCategory::Media,e))?;
    let video=temporal.video_timeline().map_err(|e|Error::from_anyhow(ErrorCategory::Media,e))?.source_time.ok_or_else(||state("recipe lyric video extent missing"))?;
    let extent=TimeRange::new(video.start.min(receipt.source_audio_extent.start),video.end.max(receipt.source_audio_extent.end)).map_err(|e|Error::from_anyhow(ErrorCategory::Media,e))?;
    let signal=crate::delivery_audio::loudness_summary(&audio.signal_run)?;
    Ok(ManualLyricsRequest {schema:"aniflow.manual-lyrics-request/v1".into(),audio_file:audio.receipt.parent().ok_or_else(||state("audio receipt parent missing"))?.join("selected-audio.wav"),
        audio:signal.source,timeline_sha256,recipe_intent_sha256:intent.into(),offset:receipt.source_audio_extent.start,delivery_extent:extent,
        input_format:options.input_format,language:options.language,overlap_policy:options.overlap_policy,targets:options.targets,conversion:options.conversion})
}
/// Prepare exact native audio/timeline/recipe bindings for the existing manual
/// initialize/import/review workflow. Requires selected completed audio evidence.
pub fn request(run_directory:impl AsRef<Path>,options:RecipeLyricsOptions)->Result<ManualLyricsRequest> {
    let root=workspace::root(run_directory.as_ref())?;let (recipe,intent)=resolved(&root)?;
    let audio=audio::observe(&recipe,&root,&intent,&CancellationToken::default())?.ok_or_else(||state("recipe lyrics require its completed optional original-audio observation"))?;
    bound_request(&recipe,&intent,&audio,options)
}
fn options(request:&ManualLyricsRequest)->RecipeLyricsOptions {RecipeLyricsOptions {input_format:request.input_format,language:request.language.clone(),overlap_policy:request.overlap_policy,targets:request.targets.clone(),conversion:request.conversion.clone()}}
fn assembly(phases:&[MusicVideoPhaseStatus])->Result<String> {
    let phase=phases.iter().find(|p|p.phase==MusicVideoPhase::Assembly&&p.state==MusicVideoPhaseState::Complete).ok_or_else(||state("lyric attachment requires accepted final assembly"))?;
    canonical_sha256(phase.manifest.as_ref().ok_or_else(||state("assembly manifest missing"))?)
}
pub(super) fn observe(recipe:&MusicVideoRequest,root:&Path,intent:&str,audio:Option<&MusicVideoAudioEvidence>,phases:&[MusicVideoPhaseStatus])->Result<Option<RecipeLyricsAttachment>> {
    let marker=root.join("lyrics-attachment.json");let output=root.join("lyrics-delivery");
    if !marker.try_exists().map_err(|_|state("cannot inspect lyrics attachment"))? {
        if output.try_exists().map_err(|_|state("cannot inspect lyrics delivery"))?{return Err(state("partial lyric delivery has no committed attachment; preserve for explicit reconciliation"));}return Ok(None);
    }
    let attachment:RecipeLyricsAttachment=workspace::read_document(&marker)?;let audio=audio.ok_or_else(||state("lyric attachment lost source-bound audio evidence"))?;
    if attachment.schema!="aniflow.recipe-lyrics-attachment/v1"||attachment.intent_sha256!=intent||attachment.assembly_manifest_sha256!=assembly(phases)?
        ||attachment.audio_receipt_sha256!=audio.receipt_sha256||attachment.request!=bound_request(recipe,intent,audio,options(&attachment.request))? {return Err(state("lyrics attachment recipe/audio/clock/delivery binding changed"));}
    let current=lyrics::status(&attachment.request,&attachment.workflow)?;
    let receipt=output.join("lyrics-delivery.json");let delivery:ManualLyricsDelivery=workspace::read_document(&receipt)?;
    if !current.ready_for_sidecar_export||delivery!=attachment.delivery||identity(&receipt,MAXIMUM_DOCUMENT_BYTES)?.0!=attachment.delivery_receipt_sha256
        ||delivery.request_sha256!=current.request_sha256||delivery.words_review_sha256!=identity(&attachment.workflow.join("words/review.json"),65536)?.0
        ||delivery.timing_review_sha256!=identity(&attachment.workflow.join("timing/review.json"),65536)?.0||delivery.runtime_qualified {return Err(state("lyrics review or delivery receipt changed"));}
    check_files(&output,&delivery)?;Ok(Some(attachment))
}
pub(super) fn check_files(output:&Path,delivery:&ManualLyricsDelivery)->Result<()> {
    let mut expected=std::collections::BTreeSet::from(["lyrics-delivery.json".to_owned()]);
    for file in &delivery.exports {
        if file.filename!=format!("lyrics.{}",file.format.extension())||!expected.insert(file.filename.clone())
            ||identity(&output.join(&file.filename),crate::timed_text::MAX_EVIDENCE_BYTES)?!=(file.sha256.clone(),file.byte_count){return Err(state("lyric sidecar membership or bytes changed"));}
    }
    let mut actual=std::collections::BTreeSet::new();for file in std::fs::read_dir(&output).map_err(|_|state("cannot inspect lyric sidecars"))? {
        if actual.len()>=8{return Err(state("too many lyric sidecars"));}let file=file.map_err(|_|state("cannot inspect lyric sidecar"))?;workspace::require_existing_canonical(&file.path())?;
        if !file.path().is_file(){return Err(state("lyric delivery contains an unexpected directory"));}actual.insert(file.file_name().to_str().ok_or_else(||state("lyric filename not UTF-8"))?.to_owned());
    }
    if actual!=expected{return Err(state("lyric delivery has missing or undeclared files"));}Ok(())
}
/// Explicit final attachment: reviewed words AND timing must already exist.
/// Copies only selected codec exports; no approval decision is generated here.
pub fn attach(run_directory:impl AsRef<Path>,request:&ManualLyricsRequest,workflow:&Path)->Result<RecipeLyricsAttachment> {
    let root=workspace::root(run_directory.as_ref())?;let _writer=crate::cache_v3::ExclusiveWriter::acquire(&root,"writer.lock")?;
    let (recipe,intent)=resolved(&root)?;let initial=plan(&recipe,&CancellationToken::default())?;
    let status=observe_tail(&recipe,&root,&intent,&initial,&CancellationToken::default())?;
    if status.lyrics_delivery.is_some(){return Err(state("recipe already has a lyric attachment; never replace it"));}
    let audio=status.audio_signal.as_ref().ok_or_else(||state("recipe lyrics need original-audio evidence"))?;
    if *request!=bound_request(&recipe,&intent,audio,options(request))?{return Err(state("manual lyrics request does not match this recipe's original audio, offset and timeline"));}
    let assembly_manifest_sha256=assembly(&status.phases)?;workspace::require_existing_canonical(workflow)?;
    if root.starts_with(workflow)||workflow.starts_with(&root){return Err(state("reviewed manual lyrics workflow must be separate from recipe outputs"));}
    let assembly_root=status.phases.iter().find(|p|p.phase==MusicVideoPhase::Assembly).and_then(|p|p.run_directory.as_ref()).ok_or_else(||state("accepted assembly directory missing"))?;
    let _assembly_writer=crate::cache_v3::ExclusiveWriter::acquire(assembly_root,"writer.lock")?;
    let delivery=lyrics::export(request,workflow,&root.join("lyrics-delivery"))?;
    if audio::observe(&recipe,&root,&intent,&CancellationToken::default())?.as_ref()!=Some(audio){return Err(state("source audio evidence changed during lyric export; preserve the uncommitted sidecars"));}
    let attachment=RecipeLyricsAttachment {schema:"aniflow.recipe-lyrics-attachment/v1".into(),intent_sha256:intent.clone(),assembly_manifest_sha256,audio_receipt_sha256:audio.receipt_sha256.clone(),
        workflow:workflow.into(),request:request.clone(),delivery_receipt_sha256:identity(&root.join("lyrics-delivery/lyrics-delivery.json"),MAXIMUM_DOCUMENT_BYTES)?.0,delivery};
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(root.join("lyrics-attachment.json")).map_err(|_|state("cannot commit recipe lyrics attachment"))?;
    use std::io::Write;file.write_all(&crate::provider::canonical_json_bytes(&attachment)?).and_then(|_|file.sync_all()).map_err(|_|state("cannot sync lyrics attachment"))?;
    observe(&recipe,&root,&intent,Some(audio),&status.phases)?.ok_or_else(||state("lyrics attachment did not commit"))
}
