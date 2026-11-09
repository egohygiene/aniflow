use super::*;
fn disjoint(root:&Path,intent:&PublishingIntent)->Result<()> {
    let mut inputs=vec![&intent.request.video_run_directory];inputs.extend(intent.components.iter().map(|p|&p.path));
    inputs.extend(intent.source.artifacts.iter().map(|a|&a.locator));
    for input in inputs {if root.starts_with(input)||input.starts_with(root){return Err(state("publishing root must be disjoint from original media, accepted evidence and selected executables"));}}Ok(())
}
fn protect_completed(root:&Path,current:&PublishingStatus,locked:&mut std::collections::BTreeSet<PathBuf>,guards:&mut Vec<crate::cache_v3::ExclusiveWriter>,token:&CancellationToken)->Result<()> {
    let mut paths=Vec::new();
    if current.steps.iter().any(|s|s.phase==PublishingPhase::DestinationExport&&s.state==PublishingStepState::Complete){paths.push(export_root(root));}
    if let Some(pin)=&current.qc_receipt_sha256 {
        let report=final_qc::observe(&qc_root(root),pin,token)?;
        paths.push(qc_root(root).join("signal").join(report.signal.signal_run));
    }
    if current.package.is_some(){paths.push(package_root(root));}
    for path in paths {if locked.insert(path.clone()){guards.push(crate::cache_v3::ExclusiveWriter::acquire(&path,"writer.lock")?);}}Ok(())
}
/// Fresh private parent; selected media owners execute only within fixed slots.
/// The first final-review pause is returned as status, never an implicit approval.
pub fn run<F:FnMut(PublishingPhase,&PipelineV3RunProgress)>(request:PublishingRequest,root:&Path,token:&CancellationToken,progress:F)->Result<PublishingStatus> {
    let initial=plan(&request,token)?;local::parent(root)?;disjoint(root,&initial.intent)?;
    local::directory(root)?;local::directory(&root.join("children"))?;
    local::write(&root.join("publishing-intent.json"),&local::bytes(&initial.intent)?)?;
    drive(root,token,progress)
}
/// Resume unchanged known native work. Partial QC/package initialization is
/// retained and refused rather than silently overwritten or adopted.
pub fn resume<F:FnMut(PublishingPhase,&PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,progress:F)->Result<PublishingStatus> {
    readback::retained(root,token)?;drive(root,token,progress)
}
fn drive<F:FnMut(PublishingPhase,&PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,mut progress:F)->Result<PublishingStatus> {
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    let intent=readback::retained(root,token)?;disjoint(root,&intent)?;
    let _source=crate::cache_v3::ExclusiveWriter::acquire(&intent.request.video_run_directory,"writer.lock")?;
    let mut locked=std::collections::BTreeSet::new();let mut guards=Vec::new();
    for _ in 0..8 {
        cancelled(token)?;let current=status(root,token)?;protect_completed(root,&current,&mut locked,&mut guards,token)?;
        let current=status(root,token)?;
        if current.complete||current.steps.iter().any(|s|matches!(s.state,PublishingStepState::AwaitingReview|PublishingStepState::Rejected|PublishingStepState::Blocked)){return Ok(current);}
        let next=current.steps.iter().find(|s|s.state!=PublishingStepState::Complete).ok_or_else(||state("publishing has no next fixed phase"))?;
        let phase=next.phase;let request=&intent.request;
        match phase {
            PublishingPhase::DestinationExport=>{
                let selected=export_request(request)?;let path=export_root(root);let report=|event:&PipelineV3RunProgress|progress(phase,event);
                if exists(&path)?{video::resume(selected,&path,token,report)?;}
                else{video::run_at(selected,&path,intent.export_plan.as_ref().ok_or_else(||state("publishing initial export plan is missing"))?,token,report)?;}
            },
            PublishingPhase::FinalAudioQc=>{
                let selected=qc_request(request,current.final_media.as_ref().ok_or_else(||state("publishing QC awaits its accepted native final video"))?)?;
                final_qc::run(&selected,&qc_root(root),token)?;
                let binding=readback::QcBinding {schema:"aniflow.publishing-audio-qc-binding/v1".into(),intent_sha256:intent.intent_sha256.clone(),
                    request_sha256:canonical_sha256(&selected)?,receipt_sha256:identity(&qc_root(root).join("qc.json"),local::DOCUMENT_BYTES)?.0};
                final_qc::observe(&qc_root(root),&binding.receipt_sha256,token)?;cancelled(token)?;
                local::write(&root.join("audio-qc-binding.json"),&local::bytes(&binding)?)?;
            },
            PublishingPhase::DestinationPreflight=>{
                let selected=PreflightRequest {schema:"aniflow.destination-preflight-request/v1".into(),input:current.final_media.clone().ok_or_else(||state("publishing preflight awaits accepted final bytes and selected QC"))?,
                    profile:request.profile.clone(),evaluated_on:request.evaluated_on};
                let report=destination::preflight(&selected,token)?;local::write(&root.join("preflight.json"),&local::bytes(&report)?)?;
            },
            PublishingPhase::ManualPackage=>{
                let selected=posting_request(request,current.preflight.clone().ok_or_else(||state("manual package awaits exact matching preflight"))?);
                posting::create(&selected,current.review.as_ref().ok_or_else(||state("manual package requires the separately supplied final-file decision"))?,&package_root(root),token)?;
            },
            PublishingPhase::RetentionHandoff=>{
                let handoff=readback::retention(root,request,token)?;local::write(&root.join("retention-handoff.json"),&local::bytes(&handoff)?)?;
            },
            PublishingPhase::FinalReview=>return Err(state("execution cannot generate or bypass the supplied final-file review")),
        }
    }
    Err(state("publishing exceeded its finite fixed phase count"))
}
/// Retain exactly one supplied approval/rejection, without executing packaging.
/// A rejected or superseded subject needs a new immutable intent, not overwrite.
pub fn record_review(root:&Path,declaration:&ManualReview,token:&CancellationToken)->Result<PublishingStatus> {
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    let intent=readback::retained(root,token)?;
    let _source=crate::cache_v3::ExclusiveWriter::acquire(&intent.request.video_run_directory,"writer.lock")?;
    let current=status(root,token)?;let mut guards=Vec::new();let mut locked=std::collections::BTreeSet::new();
    protect_completed(root,&current,&mut locked,&mut guards,token)?;let current=status(root,token)?;
    let plan=current.posting_plan.as_ref().ok_or_else(||state("final-file decision awaits complete native final bytes, selected QC and matching preflight"))?;
    require_review(plan,declaration)?;
    if current.review.is_some()||current.package.is_some(){return Err(state("publishing decision already exists; retain it and select a new intent rather than overwriting"));}
    if current.next_action.phase!=Some(PublishingPhase::FinalReview)||current.next_action.action!="record_review"{return Err(state("publishing is not at its explicit final-review pause"));}
    cancelled(token)?;local::write(&root.join("posting-review.json"),&local::bytes(declaration)?)?;
    status(root,token)
}
