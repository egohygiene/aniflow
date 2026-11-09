//! Fixed v3 adapter; native pair work and accepted state retain existing owners.
use super::*;
use crate::{CancellationToken,PipelineV3Plan,PipelineV3RunProgress,PipelineV3RunOutcome};
use crate::video_interpolation::runtime::RifeRunRequest;
pub struct SpatialRequest {pub run:RifeRunRequest,pub upscale_run_directory:PathBuf}
struct Prepared {plan:PipelineV3Plan,registry:crate::ProviderRegistry,bindings:Vec<crate::PipelineInputBinding>,input:SpatialInput}
fn cancelled(token:&CancellationToken)->Result<()> {if token.is_cancelled(){return Err(Error::new(ErrorCategory::Execution,"spatial RIFE cancelled"));}Ok(())}
fn prepare(request:&SpatialRequest,token:&CancellationToken)->Result<Prepared> {
    cancelled(token)?;let source=observe_upscale(&request.upscale_run_directory)?;let configuration=&request.run.configuration;
    if request.run.frames_directory!=source.frames||request.run.frame_index!=source.index_path{return Err(invalid("spatial RIFE paths must be the exact selected accepted Upscayl outputs"));}
    let interpolation=super::plan(configuration,&source.input.index)?;
    let maximum=super::super::provider::frame_byte_bound_dimensions(source.input.index.output_width,source.input.index.output_height,&configuration.settings)?;
    for frame in &source.input.index.frames {cancelled(token)?;inspect_png(&source.frames.join(&frame.filename),source.input.index.output_width,source.input.index.output_height,maximum)?;}
    let components=super::super::runtime::observe_components(configuration,token)?;
    let template=RifeOutputIndex {schema:OUTPUT_SCHEMA.into(),frames:super::super::provider::output_frame_template(&interpolation)?,plan:interpolation};
    let report_bytes=crate::provider::canonical_json_bytes(&template)?.len() as u64;
    if report_bytes>RIFE_MAXIMUM_INDEX_BYTES{return Err(invalid("spatial RIFE output evidence exceeds its finite bound"));}
    let bounds=&request.run.execution_limits;
    if bounds.timeout.is_zero()||bounds.timeout>std::time::Duration::from_secs(21_600)||bounds.termination_grace_period>std::time::Duration::from_secs(10)
        ||!(1..=1_048_576).contains(&bounds.maximum_stdout_bytes)||!(1..=1_048_576).contains(&bounds.maximum_stderr_bytes)||!(1..=RIFE_MAXIMUM_FRAMES+2).contains(&bounds.maximum_artifact_files)||bounds.maximum_artifact_bytes==0||bounds.maximum_artifact_bytes>configuration.settings.maximum_artifact_bytes{return Err(invalid("spatial RIFE execution bounds exceed the existing finite profile"));}
    let index_bytes=identity(&source.index_path,67_108_864)?.1;
    let reserved=source.bytes.checked_add(maximum.checked_mul(template.plan.output_frame_count+3).ok_or_else(||invalid("spatial RIFE byte overflow"))?).and_then(|n|n.checked_add(components.iter().map(|c|c.byte_count).sum::<u64>())).and_then(|n|n.checked_add(index_bytes)).and_then(|n|n.checked_add(report_bytes)).ok_or_else(||invalid("spatial RIFE byte overflow"))?;
    if reserved>configuration.settings.maximum_artifact_bytes{return Err(invalid("spatial RIFE input/model/tool/pair/output admission exceeds selected bytes"));}
    let wrapper=Wrapper {schema:"aniflow.spatial-rife-configuration/v1".into(),configuration:configuration.clone(),spatial:source.input.binding.clone()};
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("rife-native",wrapper.manifest()?,wrapper.provider_configuration()?,request.run.provider_executable.clone(),"aniflow-spatial-rife-v1",super::super::runtime::component_inventory(configuration))?)?;
    let mut pipeline:serde_json::Value=serde_yaml::from_slice(include_bytes!("../../../providers/rife/pipeline.yml")).map_err(|_|invalid("fixed spatial RIFE pipeline is malformed"))?;
    pipeline["name"]=serde_json::json!("serial-spatial-rife-interpolation");pipeline["inputs"][1]["artifact_type"]=serde_json::json!("application/vnd.aniflow.upscayl-output-index+json");pipeline["stages"][0]["capability"]["id"]=serde_json::json!(CAPABILITY_ID);
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&crate::provider::canonical_json_bytes(&pipeline)?).map_err(mapped)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_frames",&source.frames),crate::PipelineInputBinding::new("source_index",&source.index_path)];
    let mut context=crate::audio_inspection::context();context.host.memory_mib=configuration.host.available_memory_mib;context.host.gpu_available=configuration.settings.device_id>=0;context.allowed_side_effects.push(crate::SideEffect::Ai);if context.host.gpu_available {context.allowed_side_effects.push(crate::SideEffect::Gpu);}
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&context).map_err(mapped)?;
    if !plan.payload.inputs.iter().any(|i|i.id=="source_index"&&i.content_sha256==source.input.binding.index_sha256){return Err(invalid("spatial RIFE index changed during planning"));}
    if observe_upscale(&request.upscale_run_directory)?.input!=source.input{return Err(invalid("spatial RIFE source changed during planning"));}cancelled(token)?;
    Ok(Prepared {plan,registry,bindings,input:source.input})
}
pub fn plan(request:&SpatialRequest,token:&CancellationToken)->Result<PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
fn retained(root:&Path)->Result<PipelineV3Plan> {
    ensure_canonical_directory(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;let value:serde_json::Value=crate::destination::local::read_bounded(&workspace.plan(),67_108_864)?;
    let p=PipelineV3Plan::from_json_slice(&crate::provider::canonical_json_bytes(&value)?).map_err(mapped)?;
    if p.payload.pipeline_name!="serial-spatial-rife-interpolation"||p.payload.stages.len()!=1||p.payload.stages[0].capability.id!=CAPABILITY_ID||p.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID{return Err(invalid("not a fixed spatial RIFE run"));}Ok(p)
}
pub(crate) fn run_at<F:FnMut(&PipelineV3RunProgress)>(request:SpatialRequest,root:&Path,expected:&PipelineV3Plan,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let prepared=prepare(&request,token)?;if prepared.plan!=*expected{return Err(invalid("spatial RIFE handoff changed before initialization"));}
    for input in [&request.upscale_run_directory,&request.run.frames_directory,&request.run.frame_index,&request.run.provider_executable,&request.run.configuration.settings.tool.executable,&request.run.configuration.settings.model.directory] {if root.starts_with(input)||input.starts_with(root){return Err(invalid("spatial RIFE child overlaps input evidence or components"));}}
    let execution=crate::PipelineV3RunRequest::new(prepared.plan,prepared.bindings,prepared.registry).with_output_directory(crate::run_v3::exact_run_parent(root)?).with_execution_limits(request.run.execution_limits);
    let result=crate::run_v3::run_v3_at_with_progress_and_cancellation(execution,root,token,progress)?;
    if observe_upscale(&request.upscale_run_directory)?.input!=prepared.input{return Err(invalid("spatial RIFE predecessor changed during execution"));}Ok(result)
}
pub fn resume<F:FnMut(&PipelineV3RunProgress)>(request:SpatialRequest,root:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let prepared=prepare(&request,token)?;if prepared.plan!=retained(root)?{return Err(invalid("spatial RIFE source/tool/model/configuration changed; no stale reuse"));}
    let result=crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,prepared.bindings,prepared.registry).with_execution_limits(request.run.execution_limits),token,progress)?;
    if observe_upscale(&request.upscale_run_directory)?.input!=prepared.input{return Err(invalid("spatial RIFE predecessor changed during resume"));}Ok(result)
}
