//! A fixed, read-only release journey over current domain owners. Not a DAG or
//! execution plan. Existing owners alone import, validate, review and advance.
use std::path::{Path,PathBuf};
use serde::{Serialize,Deserialize};
use crate::{Result,CancellationToken};
use crate::destination::{self,invalid,cancelled,local,MediaInput,MediaObservation,MediaFacts};
use crate::provider::{canonical_sha256,decode_json,require_sha256};
use super::{lyrics,finishing,publishing,manual::ManualState};

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Stage {AcceptedMaster,TimedLyrics,Finishing,CreatorIdentity,ProvenanceInspection,CreatorWatermark,DestinationPublishing,C2pa,ExactByteEvidence,Signature,Timestamp,PostUpload}
pub const ORDER:[Stage;12]=[Stage::AcceptedMaster,Stage::TimedLyrics,Stage::Finishing,Stage::CreatorIdentity,Stage::ProvenanceInspection,Stage::CreatorWatermark,Stage::DestinationPublishing,Stage::C2pa,Stage::ExactByteEvidence,Stage::Signature,Stage::Timestamp,Stage::PostUpload];
impl Stage {pub fn optional(self)->bool {!matches!(self,Self::AcceptedMaster|Self::DestinationPublishing)}}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Mode {Automated,ManualArtifact,AutomatedWithReview,Disabled}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Selection {
    Lyrics {request:lyrics::ManualLyricsRequest,run_directory:PathBuf,delivery_directory:Option<PathBuf>},
    Finishing {request:finishing::FinishingRequest,run_directory:Option<PathBuf>,output_artifact_id:String},
    Publishing {request:publishing::PublishingRequest,run_directory:Option<PathBuf>},
    ExactBytes {directory:PathBuf,manifest_sha256:String},
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {pub stage:Stage,pub mode:Mode,pub selection:Option<Selection>,pub disabled_reason:Option<String>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {pub schema:String,pub source:MediaInput,pub source_sha256:String,pub stages:Vec<Intent>}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{if bytes.len()>262_144{return Err(invalid("release-stage request exceeds 256 KiB"));}let value:Self=decode_json(bytes,"release-stage request")?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        require_sha256(&self.source_sha256,"selected master digest")?;
        if self.schema!="aniflow.release-stages-request/v1"||self.stages.len()!=ORDER.len()||!matches!(self.source,MediaInput::AcceptedVideo {..}|MediaInput::AcceptedVideoWithQc {..}){return Err(invalid("fixed release journey requires one accepted video and every ordered stage"));}
        for (intent,expected) in self.stages.iter().zip(ORDER) {
            if intent.stage!=expected{return Err(invalid("release stage order/membership is fixed; no arbitrary dependencies"));}
            if intent.mode==Mode::Disabled {
                let reason=intent.disabled_reason.as_deref().unwrap_or("");
                if !intent.stage.optional()||intent.selection.is_some()||reason.is_empty()||reason.len()>256||reason.chars().any(char::is_control){return Err(invalid("only optional stages may be disabled with an explicit bounded reason and no selection"));}
            } else if intent.disabled_reason.is_some(){return Err(invalid("enabled stage cannot carry a disabled reason"));}
            let matching=matches!((&intent.selection,intent.stage),
                (None,_)|(Some(Selection::Lyrics {..}),Stage::TimedLyrics)|(Some(Selection::Finishing {..}),Stage::Finishing)|(Some(Selection::Publishing {..}),Stage::DestinationPublishing)|(Some(Selection::ExactBytes {..}),Stage::ExactByteEvidence));
            if !matching{return Err(invalid("stage selection belongs to another owner"));}
            if let Some(Selection::Finishing {request,output_artifact_id,..})=&intent.selection {
                request.validate()?;
                if !destination::identifier(output_artifact_id){return Err(invalid("select one exact finishing output identifier"));}
                if let Some(captions)=&request.captions {
                    match &self.stages[1].selection {
                        Some(Selection::Lyrics {run_directory,delivery_directory:Some(delivery),..}) if self.stages[1].mode!=Mode::Disabled&&*run_directory==captions.lyrics_run_directory&&*delivery==captions.lyrics_delivery_directory=>{},
                        _=>return Err(invalid("selected captions require the explicit current lyrics stage and matching reviewed delivery")),
                    }
                }
            }
            if let Some(Selection::Publishing {request,..})=&intent.selection {request.validate()?;}
            if let Some(Selection::ExactBytes {manifest_sha256,..})=&intent.selection {require_sha256(manifest_sha256,"release evidence manifest")?;}
        }
        local::bytes(self)?;Ok(())
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum State {Complete,Pending,Incomplete,AwaitingInput,AwaitingReview,Rejected,Invalidated,Unavailable,Blocked,Skipped}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Owner {FinalVideo,ManualLyrics,Finishing,Publishing,ReleaseEvidence,FutureHook}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Action {None,SupplyTimedLyrics,SupplyWordsReview,SupplyTimingReview,ExportReviewedLyrics,PlanFinishing,InspectFinishing,SupplyCaptionReview,PlanPublishing,InspectPublishing,SupplyFinalReview,VerifyRetainedBytes,ConfigureSupportedOwner,ResolvePredecessor,StartNewContext}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ReviewScope {Words,Timing,Caption,FinalVideo}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewBinding {pub scope:ReviewScope,pub context_sha256:String,pub subject_sha256:Option<String>,pub state:State}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {pub owner:Owner,pub action:Action,pub source_sha256:String,pub timeline_sha256:String,pub source_interval:crate::temporal::TimeRange,pub frame_rate:crate::temporal::RationalTime,pub reviews:Vec<ReviewBinding>,pub artifact_contract:String,pub instructions:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {pub stage:Stage,pub mode:Mode,pub optional:bool,pub state:State,pub configuration_sha256:Option<String>,pub owner_evidence_sha256:Option<String>,pub subject_sha256:Option<String>,pub disabled_reason:Option<String>,pub handoff:Handoff}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {pub schema:String,pub intent_sha256:String,pub context_sha256:String,pub source:MediaObservation,pub steps:Vec<Step>,pub next_stage:Option<Stage>,pub complete:bool,pub execution_authority:String,pub creative_approval:String,pub publication_authority:String}
fn artifact(observation:&MediaObservation)->Result<&crate::final_video::FinalVideoArtifact> {
    let (completion,id)=match &observation.facts {MediaFacts::Video {completion,artifact_id}|MediaFacts::VideoWithQc {completion,artifact_id,..}=>(completion,artifact_id),_=>return Err(invalid("release journey requires current accepted video"))};
    let selected=completion.artifacts.iter().filter(|a|&a.artifact_id==id).collect::<Vec<_>>();
    if selected.len()!=1||selected[0].sha256!=observation.file.sha256{return Err(invalid("release journey video identity differs"));}Ok(selected[0])
}
fn handoff(stage:Stage,source:&MediaObservation)->Result<Handoff> {
    let video=artifact(source)?;
    let (owner,contract)=match stage {
        Stage::AcceptedMaster=>(Owner::FinalVideo,"current_final_video_acceptance"),Stage::TimedLyrics=>(Owner::ManualLyrics,"manual_interval_srt_webvtt_ttml; separate_words_and_timing_reviews"),Stage::Finishing=>(Owner::Finishing,"current_caption_preview_review_and_native_delivery"),Stage::DestinationPublishing=>(Owner::Publishing,"current_final_file_qc_preflight_review_and_manual_package"),Stage::ExactByteEvidence=>(Owner::ReleaseEvidence,"exact_retained_manifest_and_current_native_subject"),_=>(Owner::FutureHook,"unavailable_hook; no_output_or_acceptance"),
    };
    Ok(Handoff {owner,action:Action::ConfigureSupportedOwner,source_sha256:source.file.sha256.clone(),timeline_sha256:canonical_sha256(&(video.frame_rate,video.video_interval))?,source_interval:video.video_interval,frame_rate:video.frame_rate,reviews:vec![],artifact_contract:contract.into(),instructions:"Invoke the named existing owner explicitly; this view starts no work and creates no approval.".into()})
}
fn step(intent:&Intent,source:&MediaObservation)->Result<Step> {Ok(Step {stage:intent.stage,mode:intent.mode,optional:intent.stage.optional(),state:State::Unavailable,configuration_sha256:intent.selection.as_ref().map(canonical_sha256).transpose()?,owner_evidence_sha256:None,subject_sha256:None,disabled_reason:intent.disabled_reason.clone(),handoff:handoff(intent.stage,source)?})}
fn manual_state(state:ManualState)->State {match state {ManualState::AwaitingInput=>State::AwaitingInput,ManualState::AwaitingReview=>State::AwaitingReview,ManualState::Approved=>State::Complete,ManualState::Rejected=>State::Rejected,ManualState::Invalidated=>State::Invalidated}}
fn review(scope:ReviewScope,value:&super::manual::ManualStatus)->ReviewBinding {ReviewBinding {scope,context_sha256:value.context_sha256.clone(),subject_sha256:value.subject_sha256.clone(),state:manual_state(value.state)}}
fn same_input(root:&Path,id:&str,current:&MediaObservation,token:&CancellationToken)->Result<()> {
    let selected=destination::observe_media(&MediaInput::AcceptedVideo {run_directory:root.to_owned(),artifact_id:id.into()},token)?;
    if selected.file!=current.file||artifact(&selected)?!=artifact(current)?{return Err(invalid("downstream stage does not consume the exact selected predecessor"));}Ok(())
}
fn observed_lyrics(intent:&Intent,source:&MediaObservation,output:&mut Step,token:&CancellationToken)->Result<()> {
    let Some(Selection::Lyrics {request,run_directory,delivery_directory})=&intent.selection else{return Ok(())};
    if intent.mode!=Mode::ManualArtifact{return Ok(())}
    let source_video=artifact(source)?;
    if request.timeline_sha256!=output.handoff.timeline_sha256||request.delivery_extent!=source_video.video_interval{return Err(invalid("lyrics stage clock differs from the current accepted final-video timeline"));}
    cancelled(token)?;
    if !run_directory.try_exists().map_err(|_|invalid("cannot inspect manual lyrics checkpoint"))? {output.state=State::AwaitingInput;output.handoff.action=Action::SupplyTimedLyrics;return Ok(())}
    let status=lyrics::status(request,run_directory)?;output.owner_evidence_sha256=Some(canonical_sha256(&status)?);
    output.handoff.reviews=vec![review(ReviewScope::Words,&status.words),review(ReviewScope::Timing,&status.timing)];
    let pending=output.handoff.reviews.iter().find(|r|r.state!=State::Complete);
    if let Some(pending)=pending {output.state=pending.state;output.handoff.action=match pending.state {State::AwaitingInput=>Action::SupplyTimedLyrics,State::AwaitingReview=>if pending.scope==ReviewScope::Words {Action::SupplyWordsReview}else{Action::SupplyTimingReview},State::Invalidated=>Action::StartNewContext,_=>Action::None};return Ok(())}
    output.state=State::AwaitingInput;output.handoff.action=Action::ExportReviewedLyrics;
    if let Some(directory)=delivery_directory {
        if directory.try_exists().map_err(|_|invalid("cannot inspect selected lyrics delivery"))? {
            let delivered=lyrics::observe_delivery(run_directory,directory)?;
            if delivered.delivery.request_sha256!=status.request_sha256{return Err(invalid("lyrics delivery belongs to different selected settings"));}
            output.owner_evidence_sha256=Some(canonical_sha256(&delivered)?);output.subject_sha256=Some(delivered.srt_sha256);output.state=State::Complete;output.handoff.action=Action::None;
        }
    }
    // This binds a final-video clock declaration. The caption owner separately
    // validates actual audio/caption compatibility; no PCM-origin proof is added.
    Ok(())
}
fn observed_finishing(intent:&Intent,current:&mut MediaObservation,output:&mut Step,token:&CancellationToken)->Result<()> {
    let Some(Selection::Finishing {request,run_directory,output_artifact_id})=&intent.selection else{return Ok(())};
    let supported=if request.captions.is_some(){Mode::AutomatedWithReview}else{Mode::Automated};
    if intent.mode!=supported{return Ok(())}
    same_input(&request.video_run_directory,&request.artifact_id,current,token)?;
    let Some(root)=run_directory else {output.state=State::Pending;output.handoff.action=Action::PlanFinishing;return Ok(())};
    let saved:finishing::FinishingIntent=local::read_bounded(&root.join("finishing-intent.json"),262_144)?;saved.validate()?;
    if saved.payload.request!=*request{return Err(invalid("finishing stage frozen settings changed"));}
    let status=finishing::status(root,token)?;output.owner_evidence_sha256=Some(canonical_sha256(&status)?);
    if status.complete {
        *current=destination::observe_media(&MediaInput::AcceptedVideo {run_directory:root.clone(),artifact_id:output_artifact_id.clone()},token)?;
        output.state=State::Complete;output.subject_sha256=Some(current.file.sha256.clone());output.handoff.action=Action::None;
    } else {
        let pending=status.steps.iter().find(|s|s.state!=finishing::FinishingStepState::Complete).ok_or_else(||invalid("finishing completion contradicts its owner"))?;
        output.state=match pending.state {finishing::FinishingStepState::Pending=>State::Pending,finishing::FinishingStepState::Incomplete=>State::Incomplete,finishing::FinishingStepState::AwaitingReview=>State::AwaitingReview,finishing::FinishingStepState::Rejected=>State::Rejected,finishing::FinishingStepState::Complete=>return Err(invalid("finishing next stage is already complete"))};
        output.handoff.action=if output.state==State::AwaitingReview {Action::SupplyCaptionReview}else{Action::InspectFinishing};
        if let Some(preview)=status.preview {output.handoff.reviews.push(ReviewBinding {scope:ReviewScope::Caption,context_sha256:preview.binding.context_sha256,subject_sha256:Some(preview.binding.subject_sha256),state:output.state});}
    }Ok(())
}
fn observed_publishing(intent:&Intent,current:&mut MediaObservation,output:&mut Step,token:&CancellationToken)->Result<()> {
    let Some(Selection::Publishing {request,run_directory})=&intent.selection else{return Ok(())};
    if intent.mode!=Mode::AutomatedWithReview{return Ok(())}
    same_input(&request.video_run_directory,&request.artifact_id,current,token)?;
    let Some(root)=run_directory else {output.state=State::Pending;output.handoff.action=Action::PlanPublishing;return Ok(())};
    let saved:publishing::PublishingIntent=local::read_bounded(&root.join("publishing-intent.json"),262_144)?;
    if saved.request!=*request{return Err(invalid("publishing stage frozen settings changed"));}
    let status=publishing::status(root,token)?;output.owner_evidence_sha256=Some(canonical_sha256(&status)?);
    if status.complete {
        let media=status.final_media.as_ref().ok_or_else(||invalid("completed publishing stage lacks exact native media"))?;
        *current=destination::observe_media(media,token)?;output.state=State::Complete;output.subject_sha256=Some(current.file.sha256.clone());output.handoff.action=Action::None;
    } else {
        let pending=status.steps.iter().find(|s|s.state!=publishing::PublishingStepState::Complete).ok_or_else(||invalid("publishing completion contradicts its owner"))?;
        output.state=match pending.state {publishing::PublishingStepState::Pending=>State::Pending,publishing::PublishingStepState::Incomplete=>State::Incomplete,publishing::PublishingStepState::AwaitingReview=>State::AwaitingReview,publishing::PublishingStepState::Rejected=>State::Rejected,publishing::PublishingStepState::Blocked=>State::Blocked,publishing::PublishingStepState::Complete=>return Err(invalid("publishing next stage is already complete"))};
        output.handoff.action=if output.state==State::AwaitingReview {Action::SupplyFinalReview}else{Action::InspectPublishing};
        if let Some(plan)=status.posting_plan {output.handoff.reviews.push(ReviewBinding {scope:ReviewScope::FinalVideo,context_sha256:plan.review_context_sha256,subject_sha256:Some(plan.review_subject_sha256),state:output.state});}
    }Ok(())
}
fn observed_bytes(intent:&Intent,current:&MediaObservation,output:&mut Step,token:&CancellationToken)->Result<()> {
    let Some(Selection::ExactBytes {directory,manifest_sha256})=&intent.selection else{return Ok(())};
    if intent.mode!=Mode::Automated{return Ok(())}
    let verified=crate::release_evidence::verify(directory,manifest_sha256,token)?;
    let manifest:crate::release_evidence::SubjectManifest=local::read(&directory.join("subjects.json"))?;
    if crate::frame_io::identity(&directory.join("subjects.json"),local::DOCUMENT_BYTES)?.0!=*manifest_sha256
        ||!manifest.subjects.iter().any(|s|s.role==crate::release_evidence::SubjectRole::FinalDelivery&&s.sha256==current.file.sha256&&s.byte_count==current.file.byte_count&&!matches!(&s.origin,crate::release_evidence::SubjectOrigin::DeclaredFile {..})) {return Err(invalid("retained evidence lacks the exact current native final delivery"));}
    output.state=State::Complete;output.owner_evidence_sha256=Some(canonical_sha256(&verified)?);output.subject_sha256=Some(current.file.sha256.clone());output.handoff.action=Action::None;Ok(())
}
fn project(request:&Request,token:&CancellationToken)->Result<Status> {
    request.validate()?;cancelled(token)?;let source=destination::observe_media(&request.source,token)?;
    if source.file.sha256!=request.source_sha256{return Err(invalid("accepted release-stage source changed"));}
    let mut current=source.clone();let mut steps=Vec::new();let mut blocked=false;
    for intent in &request.stages {
        cancelled(token)?;let mut output=step(intent,&current)?;
        if intent.mode==Mode::Disabled {output.state=State::Skipped;output.handoff.action=Action::None;}
        else if blocked {output.state=State::Blocked;output.handoff.action=Action::ResolvePredecessor;}
        else {match intent.stage {
            Stage::AcceptedMaster if intent.mode==Mode::Automated=>{output.state=State::Complete;output.subject_sha256=Some(current.file.sha256.clone());output.owner_evidence_sha256=Some(canonical_sha256(&current)?);output.handoff.action=Action::None;},
            Stage::TimedLyrics=>observed_lyrics(intent,&current,&mut output,token)?,
            Stage::Finishing=>observed_finishing(intent,&mut current,&mut output,token)?,
            Stage::DestinationPublishing=>observed_publishing(intent,&mut current,&mut output,token)?,
            Stage::ExactByteEvidence=>observed_bytes(intent,&current,&mut output,token)?,
            _=>{},
        }}
        if !matches!(output.state,State::Complete|State::Skipped){blocked=true;}
        steps.push(output);
    }
    let complete=steps.iter().all(|s|s.state==State::Complete||s.optional&&s.state==State::Skipped);
    let next_stage=steps.iter().find(|s|!matches!(s.state,State::Complete|State::Skipped)).map(|s|s.stage);
    let intent_sha256=canonical_sha256(request)?;let context_sha256=canonical_sha256(&(&intent_sha256,&source,&steps))?;
    Ok(Status {schema:"aniflow.release-stages-status/v1".into(),intent_sha256,context_sha256,source,steps,next_stage,complete,execution_authority:"none; explicit existing owner required".into(),creative_approval:"only retained exact declarations in existing owners; no new approval".into(),publication_authority:"none; local manual package only".into()})
}
/// Reobserve every reached owner before returning. This immutable view never
/// launches a tool, installs a provider, creates a run or grants completion.
pub fn observe(request:&Request,token:&CancellationToken)->Result<Status> {
    let first=project(request,token)?;let second=project(request,token)?;
    if first!=second{return Err(invalid("release-stage source, settings or acceptance changed during observation"));}Ok(first)
}

#[cfg(test)] mod tests;
