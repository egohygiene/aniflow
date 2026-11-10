//! Recovered recipe entry: project existing owners, never execute recovery.
use super::*;
use crate::music_video_recipe::recovered::{self as recipe,RecoveredPhase,RecoveredStepState,with_rife::{self,WithRifePhase}};
use super::entry::{EntryState,NextAction};
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum RecipeKind {RecoveredV1,RecoveredWithRifeV2}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema:String,pub recipe_kind:RecipeKind,pub recipe_run_directory:PathBuf,
    pub expected_recipe_intent_sha256:String,pub artifact_id:String,pub release_request:Option<super::Request>,
}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{if bytes.len()>524_288{return Err(invalid("recovered stage entry exceeds 512 KiB"));}let value:Self=decode_json(bytes,"recovered stage entry request")?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        require_sha256(&self.expected_recipe_intent_sha256,"expected recovered recipe intent")?;
        if self.schema!="aniflow.release-stages-recovered-entry-request/v1"||!destination::identifier(&self.artifact_id)
            ||!self.recipe_run_directory.is_absolute()||self.recipe_run_directory.as_os_str().len()>4096
            ||self.recipe_run_directory.components().any(|c|matches!(c,std::path::Component::CurDir|std::path::Component::ParentDir)) {return Err(invalid("explicit existing recovered kind/root/intent/output required"));}
        if let Some(tail)=&self.release_request {tail.validate()?;match &tail.source {
            MediaInput::AcceptedVideo {run_directory,artifact_id} if *run_directory==self.recipe_run_directory&&*artifact_id==self.artifact_id=>{},
            _=>return Err(invalid("recovered release tail must select this same exact recipe/output")),
        }}
        if crate::provider::canonical_json_bytes(self)?.len()>524_288{return Err(invalid("recovered stage entry exceeds 512 KiB"));}Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id:String,pub phase:WithRifePhase,pub state:RecoveredStepState,pub ordinal_count:u64,
    pub run_directory:Option<PathBuf>,pub plan_sha256:Option<String>,pub native_state:Option<crate::PipelineV3RunState>,pub native_manifest_sha256:Option<String>,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub schema:String,pub context_sha256:String,pub recipe_kind:RecipeKind,pub recipe_intent_sha256:String,
    pub native_owner_status_sha256:String,pub reused_frames:u64,pub missing_frames:u64,pub interpolation_selected:bool,
    pub carrier_sha256:Option<String>,pub steps:Vec<Step>,pub recipe_complete:bool,pub state:EntryState,pub next_action:NextAction,
    pub master:Option<crate::final_video::FinalVideoHandoff>,pub release:Option<super::Status>,
    pub execution_authority:String,pub creative_approval:String,pub publication_authority:String,
}
fn native(root:&Path,mut step:Step,token:&CancellationToken)->Result<Step> {
    cancelled(token)?;
    if matches!(step.phase,WithRifePhase::MissingFrames|WithRifePhase::Interpolation|WithRifePhase::Assembly) {
        if let Some(path)=&step.run_directory {
            if *path!=root.join("children").join(&step.id)||path.canonicalize().ok().as_deref()!=Some(path.as_path()){return Err(invalid("recovered native child is redirected or outside the existing fixed owner"));}
            let current=crate::status_v3(path)?;
            if step.plan_sha256.as_deref()!=Some(current.payload.plan_sha256.as_str())
                ||(step.state==RecoveredStepState::Complete)!=(current.payload.state==crate::PipelineV3RunState::Complete&&current.payload.delivery.is_some()) {return Err(invalid("recovered native child differs from current owner plan/acceptance"));}
            step.native_state=Some(current.payload.state);step.native_manifest_sha256=Some(current.manifest_sha256);
        }else if step.state!=RecoveredStepState::Pending{return Err(invalid("resolved recovered native child lacks its actual run"));}
    }Ok(step)
}
fn pending(steps:&[Step])->(EntryState,NextAction) {
    let Some(next)=steps.iter().find(|s|s.state!=RecoveredStepState::Complete) else{return (EntryState::Incomplete,NextAction::InspectNativeRecovery)};
    if next.state==RecoveredStepState::Pending{return (EntryState::Pending,NextAction::ResumeExistingRecipe);}
    (match next.native_state {Some(crate::PipelineV3RunState::Failed)=>EntryState::Failed,Some(crate::PipelineV3RunState::Cancelled)=>EntryState::Cancelled,
        Some(crate::PipelineV3RunState::Running)=>EntryState::Running,_=>EntryState::Incomplete},NextAction::InspectNativeRecovery)
}
fn project(request:&Request,token:&CancellationToken)->Result<Status> {
    request.validate()?;cancelled(token)?;let root=crate::music_video_recipe::workspace::root(&request.recipe_run_directory)?;
    let (intent,owner_sha,reused_frames,missing_frames,interpolation_selected,carrier_sha256,steps,complete,owner_master)=match request.recipe_kind {
        RecipeKind::RecoveredV1=>{
            let current=recipe::status(&root,token)?;let hash=canonical_sha256(&current)?;
            let steps=current.steps.into_iter().map(|s|Step {id:s.id,phase:match s.phase{RecoveredPhase::MissingFrames=>WithRifePhase::MissingFrames,RecoveredPhase::CompleteCarrier=>WithRifePhase::CompleteCarrier,RecoveredPhase::Assembly=>WithRifePhase::Assembly},state:s.state,ordinal_count:s.ordinals.len() as u64,run_directory:s.run_directory,plan_sha256:s.native_plan_sha256,native_state:None,native_manifest_sha256:None}).collect::<Vec<_>>();
            (current.intent_sha256,hash,current.reused_frames,current.missing_frames,false,current.carrier_sha256,steps,current.complete,current.completion)
        },
        RecipeKind::RecoveredWithRifeV2=>{
            let current=with_rife::status(&root,token)?;let hash=canonical_sha256(&current)?;
            let steps=current.steps.into_iter().map(|s|Step {id:s.id,phase:s.phase,state:s.state,ordinal_count:s.ordinals.len() as u64,run_directory:s.run_directory,plan_sha256:s.native_plan_sha256,native_state:None,native_manifest_sha256:None}).collect::<Vec<_>>();
            (current.intent_sha256,hash,current.reused_frames,current.missing_frames,current.interpolation_selected,current.carrier_sha256,steps,current.complete,current.completion)
        },
    };
    if intent!=request.expected_recipe_intent_sha256||steps.is_empty()||steps.len()>1028||steps.last().is_none_or(|s|s.phase!=WithRifePhase::Assembly){return Err(invalid("recovered entry differs from selected intent or fixed phase set"));}
    let steps=steps.into_iter().map(|s|native(&root,s,token)).collect::<Result<Vec<_>>>()?;
    let mut master=None;let mut release=None;
    let (state,next_action)=if complete {
        if steps.iter().any(|s|s.state!=RecoveredStepState::Complete){return Err(invalid("recovered completion lacks all accepted predecessors"));}
        cancelled(token)?;let accepted=crate::final_video::observe(&root)?;
        if owner_master.as_ref()!=Some(&accepted){return Err(invalid("recovered owner/final-video acceptance changed"));}
        let selected=accepted.artifacts.iter().filter(|a|a.artifact_id==request.artifact_id).collect::<Vec<_>>();
        if selected.len()!=1{return Err(invalid("select one exact actually accepted recovered master"));}
        let result=if let Some(tail)=&request.release_request {
            if tail.source_sha256!=selected[0].sha256{return Err(invalid("recovered tail does not select actual accepted master bytes"));}
            let current=super::observe(tail,token)?;let done=current.complete;release=Some(current);
            if done{(EntryState::ReleaseComplete,NextAction::None)}else{(EntryState::ReleasePending,NextAction::InspectReleaseTail)}
        }else{(EntryState::MasterAccepted,NextAction::ConfigureReleaseTail)};
        master=Some(accepted);result
    }else{
        if owner_master.is_some(){return Err(invalid("incomplete recovered owner cannot expose a completed master"));}pending(&steps)
    };
    let mut output=Status {schema:"aniflow.release-stages-recovered-entry-status/v1".into(),context_sha256:String::new(),recipe_kind:request.recipe_kind,recipe_intent_sha256:intent,
        native_owner_status_sha256:owner_sha,reused_frames,missing_frames,interpolation_selected,carrier_sha256,steps,recipe_complete:complete,state,next_action,master,release,
        execution_authority:"none; explicitly use existing recovered recipe run/resume".into(),creative_approval:"none; imported-frame decisions do not approve the complete movie".into(),publication_authority:"none".into()};
    output.context_sha256=canonical_sha256(&(request,&output))?;local::bytes(&output)?;cancelled(token)?;Ok(output)
}
/// Reobserve both current native and reached release owners. No recovery,
/// import, review, assembly or release operation is executed by this view.
pub fn observe(request:&Request,token:&CancellationToken)->Result<Status> {
    let first=project(request,token)?;let second=project(request,token)?;
    if first!=second{return Err(invalid("recovered recipe/native/release state changed during observation"));}Ok(first)
}

#[cfg(test)] mod tests {
    use super::*;
    fn step(state:RecoveredStepState,native_state:Option<crate::PipelineV3RunState>)->Step {Step {id:"assembly".into(),phase:WithRifePhase::Assembly,state,ordinal_count:0,run_directory:None,plan_sha256:None,native_state,native_manifest_sha256:None}}
    fn request()->Request {Request {schema:"aniflow.release-stages-recovered-entry-request/v1".into(),recipe_kind:RecipeKind::RecoveredV1,recipe_run_directory:"/synthetic/absent".into(),expected_recipe_intent_sha256:"a".repeat(64),artifact_id:"master".into(),release_request:None}}
    #[test] fn failed_cancelled_running_and_interrupted_native_work_requires_inspection(){for (native,expected) in [(crate::PipelineV3RunState::Failed,EntryState::Failed),(crate::PipelineV3RunState::Cancelled,EntryState::Cancelled),(crate::PipelineV3RunState::Running,EntryState::Running),(crate::PipelineV3RunState::Interrupted,EntryState::Incomplete)]{assert_eq!(pending(&[step(RecoveredStepState::Incomplete,Some(native))]),(expected,NextAction::InspectNativeRecovery));}}
    #[test] fn pending_is_advice_and_resolved_phases_do_not_create_parent_completion(){assert_eq!(pending(&[step(RecoveredStepState::Pending,None)]),(EntryState::Pending,NextAction::ResumeExistingRecipe));assert_eq!(pending(&[step(RecoveredStepState::Complete,Some(crate::PipelineV3RunState::Complete))]),(EntryState::Incomplete,NextAction::InspectNativeRecovery));}
    #[test] fn tail_cannot_substitute_another_recipe_or_output(){let mut r=request();let mut tail=super::super::tests::request();tail.source=MediaInput::AcceptedVideo {run_directory:r.recipe_run_directory.clone(),artifact_id:r.artifact_id.clone()};r.release_request=Some(tail);assert!(r.validate().is_ok());if let Some(tail)=&mut r.release_request{tail.source=MediaInput::AcceptedVideo {run_directory:"/other/recipe".into(),artifact_id:"master".into()};}assert!(r.validate().is_err());}
    #[test] fn unknown_kind_and_execution_or_approval_fields_are_not_authority(){let base=serde_json::to_value(request()).unwrap();for field in ["execute","automatic_approval","adopt"]{let mut value=base.clone();value[field]=serde_json::json!(true);assert!(Request::from_json_slice(&crate::provider::canonical_json_bytes(&value).unwrap()).is_err());}let mut value=base;value["recipe_kind"]=serde_json::json!("guess");assert!(Request::from_json_slice(&crate::provider::canonical_json_bytes(&value).unwrap()).is_err());}
    #[test] fn absent_or_cancelled_owner_cannot_create_a_master(){assert!(observe(&request(),&CancellationToken::new()).is_err());let token=CancellationToken::new();token.cancel();assert!(observe(&request(),&token).is_err());}
    #[test] fn redirected_native_child_refuses_before_reading_workspace(){let mut value=step(RecoveredStepState::Incomplete,None);value.run_directory=Some("/outside/child".into());assert!(native(Path::new("/synthetic/recipe"),value,&CancellationToken::new()).is_err());}
}
