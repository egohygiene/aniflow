use super::*;
use crate::{PipelineV3RunProgress,PipelineV3RunOutcome};
use crate::video_interpolation::runtime::RifeRunRequest;
fn mapped(e:crate::PipelinePlanningFailure)->Error {Error::new(e.category(),e.message)}
struct Prepared {plan:PipelineV3Plan,registry:crate::ProviderRegistry,bindings:Vec<crate::PipelineInputBinding>,input:RecoveredInput,limits:crate::ProviderExecutionLimits}
fn prepare(request:&RecoveredRifeRequest,token:&CancellationToken)->Result<Prepared> {
    request.validate()?;crate::destination::cancelled(token)?;let input=RecoveredInput::observe(&request.source,token)?;let configuration=&request.configuration;
    let index_path=request.source.carrier_directory.join("carrier.json");let frames=&input.carrier.frames_directory;
    let interpolation=super::plan(configuration,&input.carrier)?;let width=input.carrier.selection.request.output_width;let height=input.carrier.selection.request.output_height;
    let maximum=super::super::provider::frame_byte_bound_dimensions(width,height,&configuration.settings)?;
    for frame in &input.carrier.selection.frames {crate::destination::cancelled(token)?;if crate::frame_io::inspect_png(&frames.join(&frame.filename),width,height,maximum)?.0!=frame.sha256{return Err(invalid("recovered RIFE actual PNG differs"));}}
    let components=super::super::runtime::observe_components(configuration,token)?;
    let template=RifeOutputIndex {schema:OUTPUT_SCHEMA.into(),frames:super::super::provider::output_frame_template(&interpolation)?,plan:interpolation};
    let report_bytes=crate::provider::canonical_json_bytes(&template)?.len() as u64;if report_bytes>RIFE_MAXIMUM_INDEX_BYTES{return Err(invalid("recovered RIFE evidence exceeds current bound"));}
    let index=identity(&index_path,67_108_864)?;
    let reserved=input.carrier.selection.total_bytes.checked_add(maximum.checked_mul(template.plan.output_frame_count+3).ok_or_else(||invalid("recovered RIFE byte overflow"))?).and_then(|v|v.checked_add(components.iter().map(|c|c.byte_count).sum::<u64>())).and_then(|v|v.checked_add(index.1)).and_then(|v|v.checked_add(report_bytes)).ok_or_else(||invalid("recovered RIFE byte overflow"))?;
    if reserved>configuration.settings.maximum_artifact_bytes{return Err(invalid("recovered RIFE source/tools/models/pair/output exceed selected admission"));}
    let wrapper=Wrapper {schema:"aniflow.recovered-rife-configuration/v1".into(),configuration:configuration.clone(),recovered:request.source.clone()};
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("rife-native",wrapper.manifest()?,wrapper.provider_configuration()?,request.provider_executable.clone(),"aniflow-recovered-rife-v1",super::super::runtime::component_inventory(configuration))?)?;
    let mut authored:serde_json::Value=serde_yaml::from_slice(include_bytes!("../../../providers/rife/pipeline.yml")).map_err(|_|invalid("fixed RIFE pipeline malformed"))?;
    authored["name"]=serde_json::json!("serial-recovered-rife-interpolation");authored["inputs"][1]["artifact_type"]=serde_json::json!(INPUT_TYPE);authored["stages"][0]["capability"]["id"]=serde_json::json!(CAPABILITY_ID);
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&crate::provider::canonical_json_bytes(&authored)?).map_err(mapped)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_frames",frames),crate::PipelineInputBinding::new("source_index",&index_path)];
    let mut context=crate::audio_inspection::context();context.host.memory_mib=configuration.host.available_memory_mib;context.host.gpu_available=configuration.settings.device_id>=0;context.allowed_side_effects.push(crate::SideEffect::Ai);if context.host.gpu_available{context.allowed_side_effects.push(crate::SideEffect::Gpu);}
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&context).map_err(mapped)?;
    if !plan.payload.inputs.iter().any(|i|i.id=="source_index"&&i.content_sha256==index.0&&i.byte_count==index.1)||index.0!=request.source.carrier_sha256||RecoveredInput::observe(&request.source,token)?!=input{return Err(invalid("recovered RIFE carrier changed during planning"));}
    let limits=RifeRunRequest::new(frames,index_path,configuration.clone(),&request.provider_executable).execution_limits;
    Ok(Prepared {plan,registry,bindings,input,limits})
}
pub fn plan(request:&RecoveredRifeRequest,token:&CancellationToken)->Result<PipelineV3Plan>{Ok(prepare(request,token)?.plan)}
fn disjoint(request:&RecoveredRifeRequest,root:&Path,input:&RecoveredInput)->Result<()> {
    let paths=[&request.source.carrier_directory,&input.carrier.frames_directory,&request.provider_executable,&request.configuration.settings.tool.executable,&request.configuration.settings.model.directory];
    for path in paths {if root.starts_with(path)||path.starts_with(root){return Err(invalid("recovered RIFE output overlaps carrier/components"));}}
    for origin in &input.carrier.selection.origins {if root.starts_with(&origin.run_directory)||origin.run_directory.starts_with(root){return Err(invalid("recovered RIFE output overlaps accepted origin"));}}Ok(())
}
pub fn run<F:FnMut(&PipelineV3RunProgress)>(request:RecoveredRifeRequest,root:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let expected=plan(&request,token)?;run_at(request,root,&expected,token,progress)
}
pub(crate) fn run_at<F:FnMut(&PipelineV3RunProgress)>(request:RecoveredRifeRequest,root:&Path,expected:&PipelineV3Plan,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let prepared=prepare(&request,token)?;if prepared.plan!=*expected{return Err(invalid("recovered RIFE changed before fixed native initialization"));}disjoint(&request,root,&prepared.input)?;
    let result=crate::run_v3::run_v3_at_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(prepared.plan,prepared.bindings,prepared.registry).with_output_directory(crate::run_v3::exact_run_parent(root)?).with_execution_limits(prepared.limits),root,token,progress)?;
    if RecoveredInput::observe(&request.source,token)?!=prepared.input{return Err(invalid("recovered RIFE source authority changed during execution"));}Ok(result)
}
fn retained(root:&Path)->Result<PipelineV3Plan> {
    ensure_canonical_directory(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;let v:serde_json::Value=crate::destination::local::read_bounded(&workspace.plan(),67_108_864)?;
    let plan=PipelineV3Plan::from_json_slice(&crate::provider::canonical_json_bytes(&v)?).map_err(mapped)?;
    if plan.payload.pipeline_name!="serial-recovered-rife-interpolation"||plan.payload.stages.len()!=1||plan.payload.stages[0].capability.id!=CAPABILITY_ID||plan.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID||plan.payload.outputs.len()!=2{return Err(invalid("not a fixed recovered RIFE native run"));}Ok(plan)
}
pub fn resume<F:FnMut(&PipelineV3RunProgress)>(request:RecoveredRifeRequest,root:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let p=prepare(&request,token)?;if p.plan!=retained(root)?{return Err(invalid("recovered RIFE source/tool/model/provider/settings changed; no stale reuse"));}disjoint(&request,root,&p.input)?;
    let result=crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,p.bindings,p.registry).with_execution_limits(p.limits),token,progress)?;
    if RecoveredInput::observe(&request.source,token)?!=p.input{return Err(invalid("recovered source changed during RIFE resume"));}Ok(result)
}
pub fn status(root:&Path)->Result<crate::PipelineV3RunManifest> {let p=retained(root)?;let before=crate::status_v3(root)?;if before.payload.plan_sha256!=p.plan_sha256||retained(root)?!=p{return Err(invalid("recovered RIFE native plan changed"));}Ok(before)}
