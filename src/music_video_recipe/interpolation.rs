//! Fixed accepted-RIFE input bridge; no loose index adoption or neural replay.
use super::*;
use crate::video_assembly::rife::{self,RifeAssemblyIndex};
use crate::video_interpolation::{RifeProviderConfiguration,RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1,RIFE_CAPABILITY_ID,RIFE_PROVIDER_ID};
pub(super) fn validate_declared(configuration:&MusicVideoConfiguration,binding:Option<&MusicVideoInterpolationBinding>)->Result<()> {
    match (&configuration.interpolation,binding) {
        (None,None)=>Ok(()),
        (Some(option),Some(binding)) if option.accepted_run==binding.run_directory=>{
            crate::provider::require_sha256(&binding.plan_sha256,"recipe RIFE plan")?;crate::provider::require_sha256(&binding.manifest_sha256,"recipe RIFE acceptance")
        },
        _=>Err(state("recipe interpolation intent must retain exactly its selected native binding")),
    }
}
pub(super) fn binding(configuration:&MusicVideoConfiguration)->Result<Option<MusicVideoInterpolationBinding>> {
    let Some(option)=&configuration.interpolation else{return Ok(None);};
    Ok(Some(binding_for(option)?))
}
fn binding_for(option:&MusicVideoInterpolation)->Result<MusicVideoInterpolationBinding> {
    workspace::require_existing_canonical(&option.accepted_run)?;
    let manifest=crate::status_v3(&option.accepted_run)?;let plan=workspace::retained_plan(&option.accepted_run)?;
    let mut stages=plan.payload.stages.iter().filter(|s|s.capability.id==RIFE_CAPABILITY_ID);
    let stage=stages.next().ok_or_else(||state("accepted interpolation requires its native RIFE producer"))?;
    let wrapper=RifeProviderConfiguration {schema:RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),configuration:option.configuration.clone()};
    if stages.next().is_some()||manifest.payload.state!=crate::PipelineV3RunState::Complete||manifest.payload.delivery.is_none()
        ||stage.provider_lock.payload.provider.id!=RIFE_PROVIDER_ID||stage.provider_lock.payload.effective_configuration_sha256!=wrapper.provider_configuration()?.effective_configuration_sha256
        ||stage.provider_lock.payload.manifest_sha256!=canonical_sha256(&wrapper.manifest()?)?||plan.plan_sha256!=manifest.payload.plan_sha256 {return Err(state("interpolation is not the exact currently accepted native configuration/run"));}
    Ok(MusicVideoInterpolationBinding {run_directory:option.accepted_run.clone(),plan_sha256:plan.plan_sha256,manifest_sha256:manifest.manifest_sha256})
}
fn require_inputs(plan:&crate::PipelineV3Plan,frames:&FrameHandoff)->Result<()> {
    let stage=plan.payload.stages.iter().find(|s|s.capability.id==RIFE_CAPABILITY_ID).ok_or_else(||state("RIFE input producer missing"))?;
    for (port,digest) in [("index",frames.index_sha256.as_str()),("frames",frames.directory.content_sha256.as_str())] {
        let input=stage.inputs.iter().find(|p|p.port==port).ok_or_else(||state("RIFE original input port missing"))?;
        if input.artifacts.len()!=1||!plan.payload.inputs.iter().any(|i|i.id==input.artifacts[0]&&i.content_sha256==digest){return Err(state("accepted RIFE input is not the recipe's exact accepted prepared frame/index set"));}
    }Ok(())
}
pub(super) fn handoff(request:&MusicVideoRequest,root:&Path,frames:FrameHandoff,create:bool,token:&CancellationToken)->Result<Option<(FrameHandoff,MusicVideoInterpolationEvidence)>> {
    cancelled(token)?;
    let option=if let Some(option)=&request.configuration.interpolation {option.clone()}
        else {MusicVideoInterpolation {accepted_run:workspace::child(root,MusicVideoPhase::Interpolate),configuration:request.configuration.interpolation_run.clone().ok_or_else(||state("interpolation not selected"))?}};
    if request.configuration.interpolation.is_some()&&(option.accepted_run.starts_with(root)||root.starts_with(&option.accepted_run)){return Err(state("external accepted RIFE input and recipe output must be disjoint"));}
    let accepted=binding_for(&option)?;
    let (original,_)=crate::video_assembly::read_index(&frames.index)?;
    if !matches!(original,crate::video_assembly::AssemblyFrameIndex::Original(_)){return Err(state("initial recipe RIFE bridge requires original-clock prepared frames; spatial/derived inputs need a separate explicit workflow"));}
    require_inputs(&workspace::retained_plan(&option.accepted_run)?,&frames)?;
    let expected=rife::observe_accepted_run(&option.accepted_run,option.configuration.clone(),&frames.index)?;
    if expected.accepted_manifest_sha256!=accepted.manifest_sha256{return Err(state("RIFE native acceptance changed before handoff"));}
    let path=root.join("rife-assembly-index.json");
    let carrier:RifeAssemblyIndex=if path.try_exists().map_err(|_|state("cannot inspect interpolation carrier"))? {
        let carrier:RifeAssemblyIndex=workspace::read_document(&path)?;
        if carrier!=expected{return Err(state("recipe interpolation carrier differs from current exact accepted inputs"));}carrier
    }else if create {rife::bind_accepted_run(&option.accepted_run,option.configuration.clone(),&frames.index,&path)?}
    else{return Ok(None);};
    let directory=carrier.validate_binding()?;let (derived,index_identity)=crate::video_assembly::read_index(&path)?;
    let declaration=crate::AuthoredPipelineInput {id:"source_frames".into(),artifact_type:"application/vnd.aniflow.assembly-frames".into(),artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video)};
    let identity=crate::pipeline_v3::hash_pipeline_input(&declaration,&directory).map_err(|e|Error::new(e.category(),e.message))?;
    let evidence=MusicVideoInterpolationEvidence {accepted,carrier_sha256:index_identity.0.clone(),clock:carrier.clock()?};
    let next=FrameHandoff {frames:directory,index:path,index_sha256:index_identity.0,directory:identity,frame_hashes:derived.frames().iter().map(|f|f.sha256.clone()).collect(),
        source_sha256:frames.source_sha256,target_resolution:frames.target_resolution};
    if binding_for(&option)?!=evidence.accepted{return Err(state("accepted interpolation changed while observing the derived handoff"));}
    Ok(Some((next,evidence)))
}
