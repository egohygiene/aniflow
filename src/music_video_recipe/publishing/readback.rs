use super::*;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QcBinding {pub schema:String,pub intent_sha256:String,pub request_sha256:String,pub receipt_sha256:String}
fn layout(root:&Path,request:&PublishingRequest)->Result<()> {
    ensure_canonical_directory(root)?;ensure_canonical_directory(&root.join("children"))?;
    let mut files=vec!["publishing-intent.json"];let mut directories=vec!["children"];
    for name in ["preflight.json","posting-review.json","audio-qc-binding.json","retention-handoff.json"] {
        if exists(&root.join(name))? {
            if name=="audio-qc-binding.json"&&request.audio_qc.is_none()||name=="retention-handoff.json"&&request.retention.is_none(){return Err(state("unselected publishing evidence; preserve it for reconciliation"));}
            files.push(name);
        }
    }
    for name in ["audio-qc","package"] {
        if exists(&root.join(name))? {
            if name=="audio-qc"&&request.audio_qc.is_none(){return Err(state("unselected publishing QC child"));}directories.push(name);
        }
    }
    local::entries(root,&files,&directories,true)?;
    let export=exists(&export_root(root))?;
    if export&&request.export.is_none(){return Err(state("unselected destination-export child"));}
    local::entries(&root.join("children"),&[],&if export{vec!["destination_export"]}else{vec![]},false)
}
pub(super) fn retained(root:&Path,token:&CancellationToken)->Result<PublishingIntent> {
    cancelled(token)?;ensure_canonical_directory(root)?;
    let intent:PublishingIntent=local::read(&root.join("publishing-intent.json"))?;intent.request.validate()?;
    if intent.schema!="aniflow.publishing-intent/v1"||intent.intent_sha256!=intent_digest(&intent)?{return Err(state("publishing intent digest/schema changed"));}
    layout(root,&intent.request)?;
    if plan(&intent.request,token)?.intent!=intent{return Err(state("publishing source, profile, public metadata, tools or original acceptance changed; preserve work and select a new intent"));}
    Ok(intent)
}
pub(super) fn no_successors(root:&Path,names:&[&str])->Result<()> {
    for name in names {if exists(&root.join(name))?{return Err(state("publishing evidence exists before its current accepted predecessor/decision; preserve it for explicit reconciliation"));}}Ok(())
}
pub(super) fn retention(root:&Path,request:&PublishingRequest,token:&CancellationToken)->Result<RetentionHandoff> {
    common::retention(root,&request.package,request.retention.as_ref().ok_or_else(||state("retention handoff was not selected"))?,token)
}

pub(super) fn advise(steps:&[PublishingStep],posting_plan:Option<&posting::PostingPlan>)->PublishingNextAction {
    let next=steps.iter().find(|s|s.state!=PublishingStepState::Complete);
    let (action,reason)=match next.map(|s|s.state) {
        Some(PublishingStepState::AwaitingReview)=>("record_review","Supply a decision after reviewing the entire exact final file, public metadata, requirements and declared scope; resume separately after approval"),
        Some(PublishingStepState::Rejected)=>("retain_rejection_start_new_intent","The supplied exact-subject rejection is immutable; no package can be created"),
        Some(PublishingStepState::Blocked)=>("change_media_or_requirements_in_new_intent","Saved preflight needs changes or lacks evidence; requirements and native media are not repaired or refreshed automatically"),
        Some(PublishingStepState::Incomplete)=>("resume_native_export","Resume only the exact existing native export with its unchanged immutable plan; preserve incomplete initialization for explicit reconciliation"),
        Some(_)=>("resume","Advance the next fixed selected owner; execution cannot supply a review or post remotely"),
        None=>("manual_posting_handoff","Local package is ready for the operator's separate manual delivery; any retention handoff is unsealed and no platform acceptance is established"),
    };
    let review=next.is_some_and(|s|s.state==PublishingStepState::AwaitingReview);
    PublishingNextAction {schema:"aniflow.publishing-next-action/v1".into(),phase:next.map(|s|s.phase),action:action.into(),reason:reason.into(),
        review_context_sha256:if review{posting_plan.map(|p|p.review_context_sha256.clone())}else{None},
        review_subject_sha256:if review{posting_plan.map(|p|p.review_subject_sha256.clone())}else{None}}
}
/// Read current child owners and immutable marker bindings. No probe, mutation,
/// provider process, automatic review or adoption of partial folders occurs.
pub fn status(root:&Path,token:&CancellationToken)->Result<PublishingStatus> {
    let intent=retained(root,token)?;let request=&intent.request;let mut steps=Vec::new();
    let mut media=if request.export.is_none(){Some(MediaInput::AcceptedVideo {run_directory:request.video_run_directory.clone(),artifact_id:request.artifact_id.clone()})}else{None};
    if let Some(expected)=&intent.export_plan {
        let path=export_root(root);let present=exists(&path)?;let mut phase_state=PublishingStepState::Pending;
        if present {
            if workspace::retained_plan(&path)?!=*expected{return Err(state("publishing export child differs from its exact frozen native plan"));}
            let current=crate::status_v3(&path)?;
            if current.payload.plan_sha256!=expected.plan_sha256{return Err(state("publishing native export manifest/plan differ"));}
            phase_state=PublishingStepState::Incomplete;
            if current.payload.state==crate::PipelineV3RunState::Complete&&current.payload.delivery.is_some() {
                video::observe(&path,token)?;let completion=final_video::observe_destination(&path)?;
                if completion.artifacts.len()!=1||completion.artifacts[0].artifact_id!="destination_video"{return Err(state("publishing export did not produce its one selected native destination"));}
                media=Some(MediaInput::AcceptedVideo {run_directory:path.clone(),artifact_id:"destination_video".into()});phase_state=PublishingStepState::Complete;
            }
        }
        steps.push(step(PublishingPhase::DestinationExport,phase_state,present.then_some(path),Some(expected.plan_sha256.clone())));
    }
    if media.is_none(){no_successors(root,&["audio-qc","audio-qc-binding.json","preflight.json","posting-review.json","package","retention-handoff.json"])?;}
    let mut qc_receipt_sha256=None;let mut ready=media.is_some();
    if request.audio_qc.is_some() {
        let path=qc_root(root);let present=exists(&path)?;let marker=root.join("audio-qc-binding.json");let bound=exists(&marker)?;
        if present!=bound{return Err(state("publishing QC initialization/binding is incomplete; retain it for explicit reconciliation or start a new root"));}
        let mut phase_state=PublishingStepState::Pending;
        if present {
            let signal_request=qc_request(request,media.as_ref().ok_or_else(||state("QC lacks its accepted native export"))?)?;
            let binding:QcBinding=local::read(&marker)?;
            if binding.schema!="aniflow.publishing-audio-qc-binding/v1"||binding.intent_sha256!=intent.intent_sha256||binding.request_sha256!=canonical_sha256(&signal_request)?{return Err(state("publishing QC marker differs from exact final-file request/intent"));}
            let observed=final_qc::observe(&path,&binding.receipt_sha256,token)?;
            if observed.intent.request!=signal_request{return Err(state("publishing QC belongs to different native media or tools"));}
            let MediaInput::AcceptedVideo {run_directory,artifact_id}=media.as_ref().unwrap() else{return Err(state("publishing QC source was not plain native video"));};
            media=Some(MediaInput::AcceptedVideoWithQc {run_directory:run_directory.clone(),artifact_id:artifact_id.clone(),qc_directory:path.clone(),qc_receipt_sha256:binding.receipt_sha256.clone()});
            qc_receipt_sha256=Some(binding.receipt_sha256);phase_state=PublishingStepState::Complete;
        } else {ready=false;}
        steps.push(step(PublishingPhase::FinalAudioQc,phase_state,present.then_some(path),None));
    }
    if !ready{no_successors(root,&["preflight.json","posting-review.json","package","retention-handoff.json"])?;}
    let preflight_path=root.join("preflight.json");let mut preflight=None;let mut preflight_state=PublishingStepState::Pending;
    if ready&&exists(&preflight_path)? {
        let saved:PreflightReport=local::read(&preflight_path)?;
        let expected=destination::preflight(&PreflightRequest {schema:"aniflow.destination-preflight-request/v1".into(),input:media.clone().unwrap(),profile:request.profile.clone(),evaluated_on:request.evaluated_on},token)?;
        if saved!=expected{return Err(state("publishing preflight differs from exact current final bytes/QC and frozen supplied requirements"));}
        preflight_state=if saved.payload.disposition==PreflightDisposition::MatchesSuppliedProfile{PublishingStepState::Complete}else{PublishingStepState::Blocked};preflight=Some(saved);
    }
    steps.push(step(PublishingPhase::DestinationPreflight,preflight_state,None,None));
    let tail=common::inspect(root,&request.package,request.retention.as_ref(),preflight.clone(),token)?;
    steps.extend(tail.steps);let posting_plan=tail.posting_plan;let review=tail.review;let package=tail.package;let handoff=tail.retention;
    let complete=steps.iter().all(|s|s.state==PublishingStepState::Complete);let next_action=advise(&steps,posting_plan.as_ref());
    if retained(root,token)?!=intent{return Err(state("publishing authority changed during status"));}
    Ok(PublishingStatus {schema:"aniflow.publishing-status/v1".into(),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256,steps,final_media:media,qc_receipt_sha256,
        preflight,posting_plan,review,package,retention:handoff,complete,next_action,
        creative_approval:"not_inferred; supplied_exact_file_and_public_metadata_decision_is_separate_unverified_declaration".into(),publication_authority:"none; local manual posting handoff only".into()})
}
