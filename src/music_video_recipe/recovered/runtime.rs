use super::*;
fn layout(root:&Path,intent:&RecoveredIntent)->Result<()> {
    ensure_canonical_directory(root)?;ensure_canonical_directory(&root.join("children"))?;
    let mut directories=vec!["children"];if present(&root.join("carrier"),true)?{directories.push("carrier");}
    local::entries(root,&["recovered-intent.json"],&directories,true)?;
    let mut names=Vec::new();
    for id in intent.batches.iter().enumerate().map(|(i,_)|format!("missing_{i:04}")).chain(std::iter::once("assembly".into())) {
        if present(&child(root,&id),true)?{names.push(id);}
    }
    local::entries(&root.join("children"),&[],&names.iter().map(|v|v.as_str()).collect::<Vec<_>>(),false)
}
fn retained(root:&Path,token:&CancellationToken)->Result<RecoveredIntent> {
    ensure_canonical_directory(root)?;let intent:RecoveredIntent=workspace::read_document(&root.join("recovered-intent.json"))?;
    if intent.schema!="aniflow.recovered-music-video-intent/v1"||intent.intent_sha256!=digest(&intent)?{return Err(state("recovered immutable intent changed"));}
    layout(root,&intent)?;
    if plan(&intent.request,token)?.intent!=intent{return Err(state("recovered original source/index, accepted selections, missing inputs, native/model or assembly settings changed; retain prior work"));}
    Ok(intent)
}
pub(super) fn native(root:&Path,id:String,phase:RecoveredPhase,ordinals:Vec<u64>,expected:Option<&PipelineV3Plan>)->Result<RecoveredStep> {
    let path=child(root,&id);let exists=present(&path,true)?;
    let mut step=RecoveredStep {id,phase,state:RecoveredStepState::Pending,ordinals,run_directory:exists.then(||path.clone()),native_plan_sha256:expected.map(|p|p.plan_sha256.clone())};
    if exists {
        let expected=expected.ok_or_else(||state("recovered native child exists before its exact accepted predecessor"))?;
        if workspace::retained_plan(&path)?!=*expected{return Err(state("recovered native child differs from exact current resolved plan"));}
        let manifest=crate::status_v3(&path)?;
        if manifest.payload.plan_sha256!=expected.plan_sha256{return Err(state("recovered native manifest/plan mismatch"));}
        step.state=if manifest.payload.state==crate::PipelineV3RunState::Complete&&manifest.payload.delivery.is_some(){RecoveredStepState::Complete}else{RecoveredStepState::Incomplete};
    }Ok(step)
}
pub fn status(root:&Path,token:&CancellationToken)->Result<RecoveredStatus> {
    crate::destination::cancelled(token)?;let intent=retained(root,token)?;let mut steps=Vec::new();let mut predecessors=true;
    for (i,ordinals) in intent.batches.iter().enumerate() {
        let expected=if predecessors{Some(selected::plan(&selected_request(&intent.request,ordinals.clone())?,token)?)}else{None};
        let step=native(root,format!("missing_{i:04}"),RecoveredPhase::MissingFrames,ordinals.clone(),expected.as_ref())?;
        predecessors&=step.state==RecoveredStepState::Complete;steps.push(step);
    }
    let carrier_root=root.join("carrier");let mut carrier_sha256=None;
    if present(&carrier_root,true)? {
        if !predecessors{return Err(state("complete recovered carrier exists before selected native work is accepted"));}
        let pin=identity(&carrier_root.join("carrier.json"),MAXIMUM_DOCUMENT_BYTES)?;
        let current=carrier::observe(&carrier_root,&pin.0,token)?;
        if current.selection.request!=completed_request(root,&intent){return Err(state("recovered carrier differs from exact selected original and missing-work children"));}
        carrier_sha256=Some(pin.0);
    }
    steps.push(RecoveredStep {id:"carrier".into(),phase:RecoveredPhase::CompleteCarrier,state:if carrier_sha256.is_some(){RecoveredStepState::Complete}else{RecoveredStepState::Pending},ordinals:vec![],run_directory:carrier_sha256.as_ref().map(|_|carrier_root),native_plan_sha256:None});
    let assembly_plan=if carrier_sha256.is_some(){Some(video_assembly::plan(&assembly_request(root,&intent.request),token)?)}else{None};
    let assembly=native(root,"assembly".into(),RecoveredPhase::Assembly,vec![],assembly_plan.as_ref())?;
    let complete=assembly.state==RecoveredStepState::Complete&&predecessors&&carrier_sha256.is_some();steps.push(assembly);
    let completion=if complete{Some(crate::final_video::observe_assembly(child(root,"assembly"))?)}else{None};
    if retained(root,token)?!=intent{return Err(state("recovered source authority changed during readback"));}
    Ok(RecoveredStatus {schema:"aniflow.recovered-music-video-status/v1".into(),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256,
        reused_frames:intent.initial_selection.frames.len() as u64,missing_frames:intent.initial_selection.missing.len() as u64,steps,carrier_sha256,complete,completion,
        creative_approval:"not_performed; imported-frame decisions and native measurements do not approve the complete movie".into(),publication_authority:"none".into()})
}
fn disjoint(root:&Path,intent:&RecoveredIntent)->Result<()> {
    let mut paths=vec![&intent.request.spatial.source_video,&intent.request.spatial.source_index];
    paths.extend(intent.components.iter().map(|p|&p.path));paths.extend(intent.initial_selection.origins.iter().map(|o|&o.run_directory));
    if let Some(path)=&intent.request.frames_directory{paths.push(path);}
    for input in paths {if root.starts_with(input)||input.starts_with(root){return Err(state("recovered root must be disjoint from original inputs, accepted sources, tools and models"));}}
    Ok(())
}
pub(super) fn write(path:&Path,bytes:&[u8])->Result<()> {
    use std::io::Write;
    let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut file=options.open(path).map_err(|_|state("cannot reserve immutable recovered intent"))?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|_|state("cannot sync recovered intent"))?;local::sync(path.parent().ok_or_else(||state("recovered root missing"))?)
}
pub fn run<F:FnMut(RecoveredPhase,&PipelineV3RunProgress)>(request:RecoveredRequest,root:&Path,token:&CancellationToken,progress:F)->Result<RecoveredStatus> {
    let prepared=plan(&request,token)?;let parent=local::parent(root)?;disjoint(root,&prepared.intent)?;
    video_assembly::require_storage(parent,prepared.intent.projected_working_bytes,request.minimum_free_bytes)?;
    local::directory(root)?;local::directory(&root.join("children"))?;write(&root.join("recovered-intent.json"),&bytes(&prepared.intent)?)?;
    drive(root,token,progress)
}
pub fn resume<F:FnMut(RecoveredPhase,&PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,progress:F)->Result<RecoveredStatus> {retained(root,token)?;drive(root,token,progress)}
fn drive<F:FnMut(RecoveredPhase,&PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,mut progress:F)->Result<RecoveredStatus> {
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;let intent=retained(root,token)?;disjoint(root,&intent)?;
    let mut locked=std::collections::BTreeSet::new();let mut guards=Vec::new();
    for origin in &intent.initial_selection.origins {if locked.insert(origin.run_directory.clone()){guards.push(crate::cache_v3::ExclusiveWriter::acquire(&origin.run_directory,"writer.lock")?);}}
    video_assembly::admit_source(&intent.request.assembly,&intent.request.spatial.source_video,&intent.initial_selection.source_index.temporal_inspection,root,token)?;
    for _ in 0..intent.batches.len()+3 {
        crate::destination::cancelled(token)?;let current=status(root,token)?;
        for step in &current.steps {if step.phase==RecoveredPhase::MissingFrames&&step.state==RecoveredStepState::Complete {let path=step.run_directory.as_ref().unwrap();if locked.insert(path.clone()){guards.push(crate::cache_v3::ExclusiveWriter::acquire(path,"writer.lock")?);}}}
        let current=status(root,token)?;if current.complete{return Ok(current);}
        let next=current.steps.iter().find(|s|s.state!=RecoveredStepState::Complete).ok_or_else(||state("recovered continuation has no next fixed step"))?;
        match next.phase {
            RecoveredPhase::MissingFrames=>{
                let request=selected_request(&intent.request,next.ordinals.clone())?;let path=child(root,&next.id);let expected=selected::plan(&request,token)?;
                if next.native_plan_sha256.as_deref()!=Some(expected.plan_sha256.as_str()){return Err(state("missing-work plan changed after status"));}
                let report=|event:&PipelineV3RunProgress|progress(RecoveredPhase::MissingFrames,event);
                if present(&path,true)?{selected::resume(request,&path,token,report)?;}else{selected::run_at(request,&path,&expected,token,report)?;}
            },
            RecoveredPhase::CompleteCarrier=>{carrier::create(&completed_request(root,&intent),&root.join("carrier"),token)?;},
            RecoveredPhase::Assembly=>{
                let request=assembly_request(root,&intent.request);let path=child(root,"assembly");let expected=video_assembly::plan(&request,token)?;
                if next.native_plan_sha256.as_deref()!=Some(expected.plan_sha256.as_str()){return Err(state("recovered assembly plan changed after status"));}
                let report=|event:&PipelineV3RunProgress|progress(RecoveredPhase::Assembly,event);
                if present(&path,true)?{video_assembly::resume(request,&path,token,report)?;}else{video_assembly::run_at(request,&path,&expected,token,report)?;}
            },
        }
    }Err(state("recovered continuation exceeded its fixed finite step count"))
}
pub fn completion(root:&Path,token:&CancellationToken)->Result<crate::final_video::FinalVideoHandoff> {
    let before=status(root,token)?;let result=before.completion.clone().ok_or_else(||state("recovered recipe has no complete accepted assembly"))?;
    if status(root,token)?!=before{return Err(state("recovered continuation changed during completion handoff"));}Ok(result)
}
