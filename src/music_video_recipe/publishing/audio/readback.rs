use super::*;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QcBinding {pub schema:String,pub intent_sha256:String,pub request_sha256:String,pub receipt_sha256:String}
fn no_successors(root:&Path,names:&[&str])->Result<()> {super::super::readback::no_successors(root,names)}
fn layout(root:&Path,request:&AudioPublishingRequest)->Result<()> {
    ensure_canonical_directory(root)?;ensure_canonical_directory(&root.join("children"))?;
    let mut files=vec!["audio-publishing-intent.json"];let mut directories=vec!["children"];
    for name in ["preflight.json","posting-review.json","audio-qc-binding.json","retention-handoff.json"] {
        if exists(&root.join(name))? {
            if name=="audio-qc-binding.json"&&request.audio_qc.is_none()||name=="retention-handoff.json"&&request.retention.is_none(){return Err(state("unselected audio publishing marker; preserve it for reconciliation"));}files.push(name);
        }
    }
    for name in ["audio-qc","package"] {
        if exists(&root.join(name))? {if name=="audio-qc"&&request.audio_qc.is_none(){return Err(state("unselected final audio QC child"));}directories.push(name);}
    }
    local::entries(root,&files,&directories,true)?;
    let export=exists(&export_root(root))?;if export&&request.export.is_none(){return Err(state("unselected native audio export child"));}
    local::entries(&root.join("children"),&[],&if export {vec!["audio_export"]}else{vec![]},false)
}
pub(super) fn retained(root:&Path,token:&CancellationToken)->Result<AudioPublishingIntent> {
    cancelled(token)?;ensure_canonical_directory(root)?;let intent:AudioPublishingIntent=local::read(&root.join("audio-publishing-intent.json"))?;intent.request.validate()?;
    if intent.schema!=schema(&intent.request,"intent")||intent.intent_sha256!=digest(&intent)?{return Err(state("audio publishing immutable intent/schema changed"));}
    layout(root,&intent.request)?;
    if plan(&intent.request,token)?.intent!=intent{return Err(state("audio publishing original native subject, source acceptance, profile, public metadata, components or selected work changed; retain prior work and start a new intent"));}Ok(intent)
}
/// Read-only current predecessor projections. A partial native export may be
/// resumed; incomplete QC/package initialization is never guessed accepted.
pub fn status(root:&Path,token:&CancellationToken)->Result<AudioPublishingStatus> {
    let intent=retained(root,token)?;let request=&intent.request;let mut steps=Vec::new();
    let mut subject=if request.export.is_none(){Some(request.subject.clone())}else{None};
    if let Some(expected)=&intent.export_plan {
        let path=export_root(root);let present=exists(&path)?;let mut phase_state=PublishingStepState::Pending;
        if present {
            if workspace::retained_plan(&path)?!=*expected{return Err(state("audio export differs from frozen exact native plan"));}
            let accepted=crate::status_v3(&path)?;
            if accepted.payload.plan_sha256!=expected.plan_sha256{return Err(state("audio export native manifest/plan mismatch"));}
            phase_state=PublishingStepState::Incomplete;
            if accepted.payload.state==crate::PipelineV3RunState::Complete&&accepted.payload.delivery.is_some() {
                let completion=native_export::observe(&path,token)?;
                if Some(&completion.report.operation.configuration)!=request.export.as_ref()||completion.report.operation.source.file!=intent.source.file.locator{return Err(state("accepted audio export differs from exact selected configuration/original file"));}
                subject=Some(FinalLosslessSubject::AcceptedAudioExport {run_directory:path.clone()});phase_state=PublishingStepState::Complete;
            }
        }
        steps.push(step(PublishingPhase::DestinationExport,phase_state,present.then_some(path),Some(expected.plan_sha256.clone())));
    }
    if subject.is_none(){no_successors(root,&["audio-qc","audio-qc-binding.json","preflight.json","posting-review.json","package","retention-handoff.json"])?;}
    let mut media=subject.as_ref().map(FinalLosslessSubject::input);let mut ready=subject.is_some();let mut qc_receipt_sha256=None;
    if request.audio_qc.is_some() {
        let path=qc_root(root);let present=exists(&path)?;let marker=root.join("audio-qc-binding.json");let bound=exists(&marker)?;
        if bound&&!present||present&&!bound&&!version2(request){return Err(state("audio publishing QC initialization/binding is incomplete; retain it for explicit reconciliation or use a new root"));}
        let mut phase_state=PublishingStepState::Pending;
        if present {
            let selected=subject.clone().ok_or_else(||state("final audio QC has no currently accepted predecessor"))?;let expected=qc_request(request,selected.clone())?;
            if version2(request) {
                let current=resumable_qc::status(&path,token)?;
                if current.intent_sha256!=resumable_qc::plan(&resumable_request(expected.clone()),token)?.intent_sha256 {
                    return Err(state("recognized audio QC differs from the exact selected final subject/settings"));
                }
                phase_state=PublishingStepState::Incomplete;
                // A complete child without its parent binding is recognized
                // work, not accepted parent completion. Explicit resume writes
                // that binding after exact current native readback.
                if bound&&!current.complete{return Err(state("audio QC parent binding precedes complete native QC"));}
            }
            if bound {
                let binding:QcBinding=local::read(&marker)?;
                if binding.schema!=schema(request,"qc-binding")||binding.intent_sha256!=intent.intent_sha256||binding.request_sha256!=canonical_sha256(&expected)?{return Err(state("audio QC marker differs from exact final-file request/intent"));}
                let observed=audio_qc::observe(&path,&binding.receipt_sha256,token)?;
                if observed.intent.request!=expected{return Err(state("audio publishing QC belongs to a different native final subject/settings"));}
                media=Some(MediaInput::AcceptedAudioWithQc {subject:selected,qc_directory:path.clone(),qc_receipt_sha256:binding.receipt_sha256.clone()});qc_receipt_sha256=Some(binding.receipt_sha256);phase_state=PublishingStepState::Complete;
            }else{ready=false;}
        }else{ready=false;}
        steps.push(step(PublishingPhase::FinalAudioQc,phase_state,present.then_some(path),None));
    }
    if !ready{no_successors(root,&["preflight.json","posting-review.json","package","retention-handoff.json"])?;}
    let path=root.join("preflight.json");let mut preflight=None;let mut preflight_state=PublishingStepState::Pending;
    if ready&&exists(&path)? {
        let saved:PreflightReport=local::read(&path)?;
        let expected=destination::preflight(&PreflightRequest {schema:"aniflow.destination-preflight-request/v1".into(),input:media.clone().unwrap(),profile:request.profile.clone(),evaluated_on:request.evaluated_on},token)?;
        if saved!=expected{return Err(state("audio publishing preflight differs from exact final native bytes/QC and frozen supplied profile/date"));}
        preflight_state=if saved.payload.disposition==PreflightDisposition::MatchesSuppliedProfile {PublishingStepState::Complete}else{PublishingStepState::Blocked};preflight=Some(saved);
    }
    steps.push(step(PublishingPhase::DestinationPreflight,preflight_state,None,None));
    let tail=common::inspect(root,&request.package,request.retention.as_ref(),preflight.clone(),token)?;
    steps.extend(tail.steps);let complete=steps.iter().all(|s|s.state==PublishingStepState::Complete);let mut next_action=super::super::readback::advise(&steps,tail.posting_plan.as_ref());
    if version2(request)&&steps.iter().find(|s|s.state!=PublishingStepState::Complete).is_some_and(|s|s.phase==PublishingPhase::FinalAudioQc&&s.state==PublishingStepState::Incomplete) {
        next_action.action="resume".into();next_action.reason="Resume the exact recognized final-audio QC boundaries/native checkpoints; retain unmarked partial initialization for reconciliation".into();
    }
    if retained(root,token)?!=intent{return Err(state("audio publishing source authority changed during status"));}
    Ok(AudioPublishingStatus {schema:schema(request,"status"),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256,steps,final_media:media,qc_receipt_sha256,preflight,
        posting_plan:tail.posting_plan,review:tail.review,package:tail.package,retention:tail.retention,complete,next_action,
        creative_approval:"not_inferred; supplied_exact_file_and_public_metadata_decision_is_separate_unverified_declaration".into(),publication_authority:"none; local manual posting handoff only".into()})
}
