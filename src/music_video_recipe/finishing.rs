//! A fixed tail over accepted video: caption preview/review/delivery, then
//! optional metadata remux. Native children retain all media acceptance.
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use crate::{CancellationToken,Result,PipelineV3Plan,PipelineV3RunProgress};
use crate::destination::local;
use crate::frame_io::{ensure_canonical_directory,identity};
use crate::provider::{canonical_sha256,require_sha256};
use crate::final_video::{self,FinalVideoHandoff,FinalVideoArtifact,FinalVideoVariant};
use crate::subtitle_delivery::{self as captions,CaptionConfiguration,CaptionRequest,CaptionRunRequest,CaptionDeliveryRequest,CaptionPreviewCompletion,CaptionMode,PreviewBinding};
use crate::delivery_metadata::remux::{self,RemuxRequest};
use crate::video_assembly::AssemblyConfiguration;
use super::{state,workspace,manual::{ManualReview,ReviewDecision}};

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingCaptions {pub lyrics_run_directory:PathBuf,pub lyrics_delivery_directory:PathBuf,pub configuration:CaptionConfiguration}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingRequest {
    pub schema:String,pub video_run_directory:PathBuf,pub artifact_id:String,pub provider_executable:PathBuf,
    pub captions:Option<FinishingCaptions>,pub metadata:Option<AssemblyConfiguration>,
}
impl FinishingRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(state("finishing request exceeds 64 KiB"));}
        let request:Self=crate::provider::decode_json(bytes,"finishing request")?;request.validate()?;Ok(request)
    }
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.finishing-request/v1"||self.artifact_id.is_empty()||self.artifact_id.len()>64||self.captions.is_none()&&self.metadata.is_none(){return Err(state("select captions and/or metadata for one exact accepted video artifact"));}
        if let Some(captions)=&self.captions {captions.configuration.validate()?;}
        if let Some(settings)=&self.metadata {settings.validate()?;if settings.delivery_metadata.is_none(){return Err(state("selected finishing metadata requires its explicit approved-fields policy"));}}
        if self.metadata.is_some()&&self.captions.as_ref().is_some_and(|c|c.configuration.mode==CaptionMode::SoftMux){return Err(state("soft-mux captions plus metadata finishing is unsupported: the current remux profile does not preserve authored subtitle tracks"));}
        Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingComponent {pub path:PathBuf,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingPayload {pub request:FinishingRequest,pub source:FinalVideoHandoff,pub components:Vec<FinishingComponent>,pub initial_plan_sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingIntent {pub schema:String,pub payload:FinishingPayload,pub intent_sha256:String}
impl FinishingIntent {
    pub fn validate(&self)->Result<()> {
        self.payload.request.validate()?;require_sha256(&self.payload.initial_plan_sha256,"finishing first plan")?;
        if self.schema!="aniflow.finishing-intent/v1"||self.intent_sha256!=canonical_sha256(&(&self.schema,&self.payload))? {return Err(state("finishing intent differs from its immutable source/configuration/component binding"));}
        Ok(())
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum FinishingPhase {CaptionPreview,CaptionReview,CaptionDelivery,MetadataRemux}
impl FinishingPhase {fn id(self)->&'static str {match self {Self::CaptionPreview=>"caption_preview",Self::CaptionReview=>"caption_review",Self::CaptionDelivery=>"caption_delivery",Self::MetadataRemux=>"metadata_remux"}}}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum FinishingStepState {Pending,Incomplete,AwaitingReview,Rejected,Complete}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingStep {pub phase:FinishingPhase,pub state:FinishingStepState,pub plan:Option<PipelineV3Plan>,pub run_directory:Option<PathBuf>,pub native_manifest_sha256:Option<String>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingPlan {pub schema:String,pub intent:FinishingIntent,pub steps:Vec<FinishingStep>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingStatus {
    pub schema:String,pub run_directory:PathBuf,pub intent_sha256:String,pub steps:Vec<FinishingStep>,
    pub preview:Option<CaptionPreviewCompletion>,pub review:Option<ManualReview>,pub completion:Option<FinalVideoHandoff>,
    pub complete:bool,pub creative_approval:String,pub publication_authority:String,
}
fn cancelled(token:&CancellationToken)->Result<()> {if token.is_cancelled(){return Err(state("finishing cancelled; preserve child state for explicit resume"));}Ok(())}
fn phases(request:&FinishingRequest)->Vec<FinishingPhase> {
    let mut output=Vec::new();if request.captions.is_some(){output.extend([FinishingPhase::CaptionPreview,FinishingPhase::CaptionReview,FinishingPhase::CaptionDelivery]);}if request.metadata.is_some(){output.push(FinishingPhase::MetadataRemux);}output
}
fn child(root:&Path,phase:FinishingPhase)->PathBuf {root.join("children").join(phase.id())}
fn selected<'a>(request:&FinishingRequest,handoff:&'a FinalVideoHandoff)->Result<&'a FinalVideoArtifact> {
    let values=handoff.artifacts.iter().filter(|a|a.artifact_id==request.artifact_id).collect::<Vec<_>>();
    if values.len()!=1||values[0].variant!=FinalVideoVariant::DestinationCopy{return Err(state("finishing source must be one currently accepted H.264 destination artifact"));}Ok(values[0])
}
fn caption_request(request:&FinishingRequest)->Result<CaptionRunRequest> {
    let c=request.captions.as_ref().ok_or_else(||state("captions not selected"))?;
    Ok(CaptionRunRequest {provider_executable:request.provider_executable.clone(),request:CaptionRequest {schema:"aniflow.caption-request/v1".into(),video_run_directory:request.video_run_directory.clone(),artifact_id:request.artifact_id.clone(),lyrics_run_directory:c.lyrics_run_directory.clone(),lyrics_delivery_directory:c.lyrics_delivery_directory.clone(),configuration:c.configuration.clone()}})
}
fn remux_request(request:&FinishingRequest,source:&FinalVideoArtifact)->Result<RemuxRequest> {
    let audio=source.audio.is_some();
    Ok(RemuxRequest::new(source.locator.clone(),crate::temporal::StreamSelection {video_stream:Some(0),audio_stream:audio.then_some(1),no_audio:!audio,discard_streams:Vec::new(),audio_extent:Some(crate::temporal::AudioExtentPolicy::PreserveExact)},request.metadata.clone().ok_or_else(||state("metadata not selected"))?,request.provider_executable.clone()))
}
fn components(request:&FinishingRequest)->Result<Vec<FinishingComponent>> {
    let provider=identity(&request.provider_executable,134_217_728)?;
    let mut values=vec![FinishingComponent {path:request.provider_executable.clone(),sha256:provider.0,byte_count:provider.1}];
    for settings in request.captions.iter().map(|c|&c.configuration.settings).chain(request.metadata.iter()) {
        let observed=crate::video_assembly::observe_components(settings)?;
        for (pin,value) in [&settings.ffmpeg,&settings.ffprobe].into_iter().zip(observed) {values.push(FinishingComponent {path:pin.executable.clone(),sha256:value.0,byte_count:value.1});}
    }
    if let Some(c)=&request.captions {let observed=captions::observe_font(&c.configuration.font)?;values.push(FinishingComponent {path:c.configuration.font.file.clone(),sha256:observed.0,byte_count:observed.1});}
    values.sort_by(|a,b|a.path.cmp(&b.path));values.dedup();Ok(values)
}
/// Read-only admission. Later plans remain pending until current native outputs
/// and a separately supplied caption-preview declaration exist.
pub fn plan(request:&FinishingRequest,token:&CancellationToken)->Result<FinishingPlan> {
    cancelled(token)?;request.validate()?;ensure_canonical_directory(&request.video_run_directory)?;
    if request.video_run_directory.join("finishing-intent.json").try_exists().map_err(|_|state("cannot inspect finishing source"))? {return Err(state("nested finishing recipes are unsupported; select the original accepted video owner"));}
    let source=final_video::observe(&request.video_run_directory)?;let artifact=selected(request,&source)?;
    let observed=components(request)?;
    let initial=if request.captions.is_some(){captions::preview_plan(&caption_request(request)?,token)?}else{remux::plan(&remux_request(request,artifact)?,token)?};
    let payload=FinishingPayload {request:request.clone(),source:source.clone(),components:observed.clone(),initial_plan_sha256:initial.plan_sha256.clone()};
    let schema="aniflow.finishing-intent/v1".to_owned();let intent=FinishingIntent {intent_sha256:canonical_sha256(&(&schema,&payload))?,schema,payload};intent.validate()?;local::bytes(&intent)?;
    if components(request)?!=observed||final_video::observe(&request.video_run_directory)?!=source{return Err(state("finishing source or components changed during admission"));}
    let steps=phases(request).into_iter().enumerate().map(|(i,phase)|FinishingStep {phase,state:FinishingStepState::Pending,plan:(i==0).then(||initial.clone()),run_directory:None,native_manifest_sha256:None}).collect();
    cancelled(token)?;Ok(FinishingPlan {schema:"aniflow.finishing-plan/v1".into(),intent,steps})
}
fn exists_file(path:&Path)->Result<bool> {match std::fs::symlink_metadata(path) {Ok(_)=>{workspace::require_existing_canonical(path)?;if !path.is_file(){return Err(state("finishing declaration must be a regular file"));}Ok(true)},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(false),Err(_)=>Err(state("cannot inspect finishing declaration"))}}
fn layout(root:&Path,request:&FinishingRequest)->Result<()> {
    ensure_canonical_directory(root)?;ensure_canonical_directory(&root.join("children"))?;
    let mut files=vec!["finishing-intent.json"];if exists_file(&root.join("caption-review.json"))? {if request.captions.is_none(){return Err(state("unselected caption declaration"));}files.push("caption-review.json");}
    local::entries(root,&files,&["children"],true)?;
    let allowed=phases(request).into_iter().filter(|p|*p!=FinishingPhase::CaptionReview).map(|p|p.id()).collect::<Vec<_>>();
    for entry in std::fs::read_dir(root.join("children")).map_err(|_|state("cannot enumerate finishing children"))? {
        let entry=entry.map_err(|_|state("cannot read finishing child"))?;let name=entry.file_name();
        if !name.to_str().is_some_and(|n|allowed.contains(&n))||!workspace::exists(&entry.path())?{return Err(state("unknown or unselected finishing child; preserve it for reconciliation"));}
    }Ok(())
}
fn retained(root:&Path,token:&CancellationToken)->Result<(FinishingIntent,FinishingPlan)> {
    let saved:FinishingIntent=local::read(&root.join("finishing-intent.json"))?;saved.validate()?;layout(root,&saved.payload.request)?;
    let current=plan(&saved.payload.request,token)?;if current.intent!=saved{return Err(state("finishing source, settings, font, tools, lyrics or native acceptance changed; preserve existing work and start a new intent"));}Ok((saved,current))
}
fn native_step(root:&Path,phase:FinishingPhase,plan:Option<PipelineV3Plan>)->Result<FinishingStep> {
    let path=child(root,phase);let exists=workspace::exists(&path)?;
    let mut step=FinishingStep {phase,state:FinishingStepState::Pending,plan,run_directory:exists.then(||path.clone()),native_manifest_sha256:None};
    if exists {
        let expected=step.plan.as_ref().ok_or_else(||state("finishing child initialized before its accepted predecessor or supplied approval"))?;
        if workspace::retained_plan(&path)?!=*expected{return Err(state("finishing native child differs from the exact current fixed handoff"));}
        let manifest=crate::status_v3(&path)?;if manifest.payload.plan_sha256!=expected.plan_sha256{return Err(state("finishing child manifest/plan mismatch"));}
        step.state=if manifest.payload.state==crate::PipelineV3RunState::Complete&&manifest.payload.delivery.is_some(){FinishingStepState::Complete}else{FinishingStepState::Incomplete};step.native_manifest_sha256=Some(manifest.manifest_sha256);
    }Ok(step)
}
fn require_declaration(preview:&PreviewBinding,declaration:&ManualReview)->Result<()> {
    declaration.validate()?;
    if declaration.context_sha256!=preview.context_sha256||declaration.subject_sha256!=preview.subject_sha256{return Err(state("caption decision must name the exact current accepted preview context and subject"));}Ok(())
}
fn delivery_request(request:&FinishingRequest,root:&Path,review:&ManualReview)->Result<CaptionDeliveryRequest> {Ok(CaptionDeliveryRequest {run:caption_request(request)?,preview_run_directory:child(root,FinishingPhase::CaptionPreview),declaration:review.clone()})}
/// Rehash current authority without locks, writes, probes or provider execution.
pub fn status(root:&Path,token:&CancellationToken)->Result<FinishingStatus> {
    let (intent,initial)=retained(root,token)?;let request=&intent.payload.request;let mut steps=Vec::new();let mut preview=None;let mut review=None;let mut result=None;
    if request.captions.is_some() {
        let first=native_step(root,FinishingPhase::CaptionPreview,initial.steps[0].plan.clone())?;
        if first.state==FinishingStepState::Complete {preview=Some(captions::preview_status(&child(root,FinishingPhase::CaptionPreview),token)?);}
        steps.push(first);
        if exists_file(&root.join("caption-review.json"))? {
            let declaration:ManualReview=local::read_bounded(&root.join("caption-review.json"),65_536)?;require_declaration(&preview.as_ref().ok_or_else(||state("caption review lacks its current accepted preview"))?.binding,&declaration)?;review=Some(declaration);
        }
        let review_state=match review.as_ref().map(|r|r.decision) {Some(ReviewDecision::Approve)=>FinishingStepState::Complete,Some(ReviewDecision::Reject)=>FinishingStepState::Rejected,None if preview.is_some()=>FinishingStepState::AwaitingReview,None=>FinishingStepState::Pending};
        steps.push(FinishingStep {phase:FinishingPhase::CaptionReview,state:review_state,plan:None,run_directory:None,native_manifest_sha256:None});
        let next=if review_state==FinishingStepState::Complete {Some(captions::delivery_plan(&delivery_request(request,root,review.as_ref().unwrap())?,token)?)}else{None};
        let delivery=native_step(root,FinishingPhase::CaptionDelivery,next)?;
        if delivery.state==FinishingStepState::Complete {result=Some(final_video::observe_captions(child(root,FinishingPhase::CaptionDelivery))?);}
        steps.push(delivery);
    }
    if request.metadata.is_some() {
        let source=if request.captions.is_some(){result.as_ref().and_then(|r|r.artifacts.first())}else{Some(selected(request,&intent.payload.source)?)};
        let next=source.map(|a|remux_request(request,a).and_then(|r|remux::plan(&r,token))).transpose()?;
        let step=native_step(root,FinishingPhase::MetadataRemux,next)?;
        result=if step.state==FinishingStepState::Complete {Some(final_video::observe_remux(child(root,FinishingPhase::MetadataRemux))?)}else{None};steps.push(step);
    }
    let complete=steps.iter().all(|s|s.state==FinishingStepState::Complete);
    if plan(request,token)?.intent!=intent{return Err(state("finishing initial authority changed during status"));}
    Ok(FinishingStatus {schema:"aniflow.finishing-status/v1".into(),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256,steps,preview,review,completion:if complete{result}else{None},complete,creative_approval:"full_delivery_review_not_performed; preview_decision_only_authorizes_candidate_generation".into(),publication_authority:"none".into()})
}
fn disjoint(root:&Path,request:&FinishingRequest,intent:&FinishingIntent)->Result<()> {
    let mut inputs=vec![&request.video_run_directory];inputs.extend(intent.payload.components.iter().map(|c|&c.path));
    if let Some(c)=&request.captions {inputs.extend([&c.lyrics_run_directory,&c.lyrics_delivery_directory]);}
    for input in inputs {if root.starts_with(input)||input.starts_with(root){return Err(state("finishing root must be disjoint from source evidence, lyrics, tools and font"));}}Ok(())
}
/// Allocate one fresh private recipe root. A preview pauses before delivery;
/// no declaration is accepted as an execution argument or generated by default.
pub fn run<F:FnMut(FinishingPhase,&PipelineV3RunProgress)>(request:FinishingRequest,root:&Path,token:&CancellationToken,progress:F)->Result<FinishingStatus> {
    let initial=plan(&request,token)?;local::parent(root)?;disjoint(root,&request,&initial.intent)?;
    local::directory(root)?;local::directory(&root.join("children"))?;local::write(&root.join("finishing-intent.json"),&local::bytes(&initial.intent)?)?;drive(root,token,progress)
}
/// Explicitly resume only known native children with the same immutable intent.
pub fn resume<F:FnMut(FinishingPhase,&PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,progress:F)->Result<FinishingStatus> {retained(root,token)?;drive(root,token,progress)}
fn drive<F:FnMut(FinishingPhase,&PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,mut progress:F)->Result<FinishingStatus> {
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;let mut accepted=Vec::new();let mut locked=std::collections::BTreeSet::new();
    for _ in 0..4 {
        cancelled(token)?;let current=status(root,token)?;
        for step in &current.steps {if step.state==FinishingStepState::Complete {if let Some(path)=&step.run_directory {if locked.insert(path.clone()){accepted.push(crate::cache_v3::ExclusiveWriter::acquire(path,"writer.lock")?);}}}}
        if current.complete||current.steps.iter().any(|s|matches!(s.state,FinishingStepState::AwaitingReview|FinishingStepState::Rejected)){return Ok(current);}
        let next=current.steps.iter().find(|s|s.state!=FinishingStepState::Complete).ok_or_else(||state("finishing has no next native child"))?;
        let expected=next.plan.as_ref().ok_or_else(||state("finishing awaits an accepted predecessor; no future plan can be fabricated"))?;let phase=next.phase;let path=child(root,phase);let exists=workspace::exists(&path)?;
        let (intent,_)=retained(root,token)?;let request=&intent.payload.request;disjoint(root,request,&intent)?;
        let report=|p:&PipelineV3RunProgress|progress(phase,p);
        match phase {
            FinishingPhase::CaptionPreview=>{let request=caption_request(request)?;if exists {captions::preview_resume(request,&path,token,report)?;}else{captions::preview_run_at(request,&path,expected,token,report)?;}},
            FinishingPhase::CaptionDelivery=>{let request=delivery_request(request,root,current.review.as_ref().ok_or_else(||state("caption delivery requires the recorded preview declaration"))?)?;if exists {captions::delivery_resume(request,&path,token,report)?;}else{captions::delivery_run_at(request,&path,expected,token,report)?;}},
            FinishingPhase::MetadataRemux=>{
                let source=if request.captions.is_some(){final_video::observe_captions(child(root,FinishingPhase::CaptionDelivery))?.artifacts.into_iter().next().ok_or_else(||state("accepted caption artifact missing"))?}else{selected(request,&intent.payload.source)?.clone()};
                let request=remux_request(request,&source)?;if exists {remux::resume(request,&path,token,report)?;}else{remux::run_at(request,&path,expected,token,report)?;}
            },
            FinishingPhase::CaptionReview=>return Err(state("a supplied review cannot be generated by execution")),
        }
        if locked.insert(path.clone()){accepted.push(crate::cache_v3::ExclusiveWriter::acquire(&path,"writer.lock")?);}
    }
    Err(state("finishing exceeded its finite fixed native child count"))
}
/// Retain one caller-supplied approval or rejection; never execute the tail.
pub fn record_review(root:&Path,declaration:&ManualReview,token:&CancellationToken)->Result<FinishingStatus> {
    declaration.validate()?;retained(root,token)?;let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    let current=status(root,token)?;let preview=current.preview.as_ref().ok_or_else(||state("caption review requires an accepted preview"))?;
    let _preview=crate::cache_v3::ExclusiveWriter::acquire(&preview.binding.run_directory,"writer.lock")?;let before=status(root,token)?;
    require_declaration(&before.preview.as_ref().ok_or_else(||state("caption preview changed"))?.binding,declaration)?;
    if exists_file(&root.join("caption-review.json"))?||workspace::exists(&child(root,FinishingPhase::CaptionDelivery))? {return Err(state("a prior decision or initialized delivery cannot be replaced; preserve it and start a new recipe"));}
    local::write(&root.join("caption-review.json"),&local::bytes(declaration)?)?;let after=status(root,token)?;
    if before.preview!=after.preview{return Err(state("caption preview changed while recording the supplied decision"));}Ok(after)
}
/// Project existing final native acceptance only after all selected steps.
pub fn completion(root:&Path)->Result<FinalVideoHandoff> {
    status(root,&CancellationToken::default())?.completion.ok_or_else(||state("finishing is incomplete or awaiting/rejecting review; no final completion can be projected"))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request()->FinishingRequest {serde_json::from_value(json!({"schema":"aniflow.finishing-request/v1","video_run_directory":"/synthetic/base","artifact_id":"h264_delivery","provider_executable":"/synthetic/aniflow",
        "captions":{"lyrics_run_directory":"/synthetic/lyrics","lyrics_delivery_directory":"/synthetic/srt","configuration":{"schema":"aniflow.caption-configuration/v1","mode":"burn_in","settings":{"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"b".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"c".repeat(64)}},"font":{"file":"/synthetic/font.ttf","sha256":"d".repeat(64)},"style":{"font_size":40,"bottom_margin":32,"foreground":"ffffff","box_color":"000000","box_opacity":180},"preview_cue_indices":[]}},"metadata":null})).unwrap()}
    fn binding()->PreviewBinding {PreviewBinding {run_directory:"/synthetic/preview".into(),report_sha256:"a".repeat(64),context_sha256:"b".repeat(64),subject_sha256:"c".repeat(64)}}
    #[test]
    fn fixed_order_puts_preview_review_delivery_before_metadata() {
        let mut request=request();request.validate().unwrap();assert_eq!(phases(&request),[FinishingPhase::CaptionPreview,FinishingPhase::CaptionReview,FinishingPhase::CaptionDelivery]);
        let mut metadata=request.captions.as_ref().unwrap().configuration.settings.clone();metadata.delivery_metadata=Some(serde_json::from_value(json!({"schema":"aniflow.delivery-metadata-policy/v1","mode":"minimal","approved":{}})).unwrap());request.metadata=Some(metadata);request.validate().unwrap();
        assert_eq!(phases(&request).last(),Some(&FinishingPhase::MetadataRemux));request.captions.as_mut().unwrap().configuration.mode=CaptionMode::SoftMux;assert!(request.validate().is_err());
        request.metadata=None;request.validate().unwrap();request.captions=None;assert!(request.validate().is_err());
    }
    #[test]
    fn exact_supplied_rejection_is_retained_but_changed_context_or_subject_refuses() {
        let preview=binding();let mut decision=ManualReview {schema:"aniflow.manual-review/v1".into(),context_sha256:preview.context_sha256.clone(),subject_sha256:preview.subject_sha256.clone(),decision:ReviewDecision::Reject,reviewer:"synthetic operator".into(),notes:"fixture only".into()};
        require_declaration(&preview,&decision).unwrap();assert_eq!(decision.decision,ReviewDecision::Reject);decision.context_sha256="e".repeat(64);assert!(require_declaration(&preview,&decision).is_err());decision.context_sha256=preview.context_sha256.clone();decision.subject_sha256="f".repeat(64);assert!(require_declaration(&preview,&decision).is_err());
    }
    #[test]
    fn unknown_root_or_child_members_are_not_adopted() {
        let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();std::fs::create_dir(root.join("children")).unwrap();std::fs::write(root.join("finishing-intent.json"),b"{}").unwrap();
        layout(&root,&request()).unwrap();std::fs::create_dir(root.join("children/arbitrary-stage")).unwrap();assert!(layout(&root,&request()).is_err());assert!(!root.join("writer.lock").exists());
    }
    #[test]
    fn premature_child_without_real_predecessor_plan_cannot_be_promoted() {
        let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();std::fs::create_dir(root.join("children")).unwrap();std::fs::create_dir(child(&root,FinishingPhase::CaptionDelivery)).unwrap();
        assert!(native_step(&root,FinishingPhase::CaptionDelivery,None).is_err());assert!(!root.join("writer.lock").exists());assert!(completion(&root).is_err());
    }
    #[test]
    fn unknown_fields_and_execution_flags_are_not_request_authority() {
        let mut value=serde_json::to_value(request()).unwrap();value["approve"]=json!(true);assert!(FinishingRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
