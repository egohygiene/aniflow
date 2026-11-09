use super::*;
use crate::{CancellationToken,PipelineV3Plan,PipelineV3RunProgress,PipelineV3RunOutcome};
use crate::frame_io::{identity,inventory,inspect_png,ensure_canonical_directory};
use crate::destination::local;
use std::time::Duration;
fn mapped(failure:crate::PipelinePlanningFailure)->Error {Error::new(failure.category(),failure.message)}
fn cancelled(token:&CancellationToken)->Result<()> {if token.is_cancelled(){return Err(Error::new(ErrorCategory::Execution,"RIFE preview cancelled"));}Ok(())}
pub(crate) struct SourceObservation {pub index:RifeFrameIndex,pub index_identity:(String,u64),pub directory:crate::PipelineInputIdentity,pub source_bytes:u64}
pub(crate) fn inputs(operation:&PreviewConfiguration,token:&CancellationToken)->Result<SourceObservation> {
    cancelled(token)?;operation.validate()?;ensure_canonical_directory(&operation.frames_directory)?;
    let (index,index_identity)=super::super::provider::read_index(&operation.frame_index)?;operation.mapping(&index)?;
    let settings=&operation.configuration.settings;let maximum=super::super::provider::frame_byte_bound(&index,settings)?;
    let names=index.frames.iter().map(|f|std::ffi::OsString::from(&f.filename)).collect::<Vec<_>>();inventory(&operation.frames_directory,&names)?;
    let declaration=crate::AuthoredPipelineInput {id:"source_frames".into(),artifact_type:"application/vnd.aniflow.rife-frames".into(),artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video)};
    let directory=crate::pipeline_v3::hash_pipeline_input(&declaration,&operation.frames_directory).map_err(mapped)?;let mut total=0u64;
    for frame in &index.frames {
        cancelled(token)?;let observed=inspect_png(&operation.frames_directory.join(&frame.filename),index.width,index.height,maximum)?;
        if observed.0!=frame.sha256{return Err(invalid("RIFE preview original PNG differs from its complete immutable index"));}
        total=total.checked_add(observed.1).filter(|v|*v<=settings.maximum_artifact_bytes).ok_or_else(||invalid("RIFE preview source exceeds bounded input admission"))?;
    }
    if directory.byte_count!=total||identity(&operation.frame_index,RIFE_MAXIMUM_INDEX_BYTES)?!=index_identity||crate::pipeline_v3::hash_pipeline_input(&declaration,&operation.frames_directory).map_err(mapped)?!=directory{return Err(invalid("RIFE preview source changed during observation"));}
    Ok(SourceObservation {index,index_identity,directory,source_bytes:total})
}
fn operation(request:&PreviewRequest)->PreviewConfiguration {PreviewConfiguration {schema:CONFIGURATION.into(),configuration:request.run.configuration.clone(),range:request.range,frames_directory:request.run.frames_directory.clone(),frame_index:request.run.frame_index.clone()}}
struct Prepared {plan:PipelineV3Plan,registry:crate::ProviderRegistry,bindings:Vec<crate::PipelineInputBinding>,operation:PreviewConfiguration}
fn prepare(request:&PreviewRequest,token:&CancellationToken)->Result<Prepared> {
    let operation=operation(request);let source=inputs(&operation,token)?;let limits=&request.run.execution_limits;
    if limits.timeout.is_zero()||limits.timeout>Duration::from_secs(3600)||limits.termination_grace_period>Duration::from_secs(10)
        ||!(1..=1_048_576).contains(&limits.maximum_stdout_bytes)||!(1..=1_048_576).contains(&limits.maximum_stderr_bytes)
        ||!(1..=256).contains(&limits.maximum_artifact_files)||limits.maximum_artifact_bytes==0||limits.maximum_artifact_bytes>operation.configuration.settings.maximum_artifact_bytes{return Err(invalid("RIFE preview execution limits exceed finite short-window bounds"));}
    let components=super::super::runtime::observe_components(&operation.configuration,token)?;
    let template=report_template(&operation,&source.index,&source.index_identity,&source.directory.content_sha256,&"0".repeat(64))?;
    let report_bytes=crate::provider::canonical_json_bytes(&template)?.len() as u64;
    super::super::provider::admit_storage(&operation.configuration.settings,&source.index,source.source_bytes,components[0].byte_count,source.index_identity.1,report_bytes,template.frames.len() as u64)?;
    let values=serde_json::json!({"schema":"aniflow.pipeline/v3","name":"bounded-rife-preview","inputs":[{"id":"source_frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video"},{"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"}],
        "stages":[{"id":"rife_preview","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"rife-preview"},"fallbacks":[]},"inputs":[{"port":"frames","artifacts":["source_frames"]},{"port":"index","artifacts":["source_index"]}],
            "outputs":[{"port":"frames","artifacts":[{"id":"preview_frames","relative_path":"artifacts/rife-preview/frames","kind":"directory"}]},{"port":"index","artifacts":[{"id":"preview_report","relative_path":"artifacts/rife-preview/report.json","kind":"file"}]}],
            "validations":[{"id":"preview_frames_integrity","artifact":"preview_frames","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"preview_report_integrity","artifact":"preview_report","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],
        "outputs":[{"id":"preview_frames","artifact":"preview_frames","required_validations":["preview_frames_integrity"]},{"id":"preview_report","artifact":"preview_report","required_validations":["preview_report_integrity"]}]});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&crate::provider::canonical_json_bytes(&values)?).map_err(mapped)?;
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("rife-preview",operation.manifest()?,operation.provider_configuration()?,request.run.provider_executable.clone(),"aniflow-rife-preview-v1",super::super::runtime::component_inventory(&operation.configuration))?)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_frames",&operation.frames_directory),crate::PipelineInputBinding::new("source_index",&operation.frame_index)];
    let mut context=crate::audio_inspection::context();context.host.memory_mib=operation.configuration.host.available_memory_mib;context.host.gpu_available=operation.configuration.settings.device_id>=0;
    context.allowed_side_effects.push(crate::SideEffect::Ai);if context.host.gpu_available {context.allowed_side_effects.push(crate::SideEffect::Gpu);}
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&context).map_err(mapped)?;
    input_bindings(&plan,&source.index_identity,&source.directory)?;cancelled(token)?;Ok(Prepared {plan,registry,bindings,operation})
}
fn input_bindings(plan:&PipelineV3Plan,index:&(String,u64),directory:&crate::PipelineInputIdentity)->Result<()> {
    if !plan.payload.inputs.iter().any(|i|i.id=="source_index"&&i.content_sha256==index.0&&i.byte_count==index.1)
        ||!plan.payload.inputs.iter().any(|i|i.id=="source_frames"&&i.content_sha256==directory.content_sha256&&i.byte_count==directory.byte_count&&i.file_count==directory.file_count){return Err(invalid("RIFE preview source identity differs from retained native plan"));}Ok(())
}
pub fn plan(request:&PreviewRequest,token:&CancellationToken)->Result<PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
fn disjoint(request:&PreviewRequest,parent:&Path)->Result<()> {
    ensure_canonical_directory(parent)?;
    for input in [&request.run.frames_directory,&request.run.frame_index,&request.run.provider_executable,&request.run.configuration.settings.tool.executable,&request.run.configuration.settings.model.directory] {
        if parent.starts_with(input)||input.starts_with(parent){return Err(invalid("RIFE preview parent overlaps original inputs/tools/models"));}
    }Ok(())
}
pub fn run<F:FnMut(&PipelineV3RunProgress)>(request:PreviewRequest,parent:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    disjoint(&request,parent)?;let ready=prepare(&request,token)?;
    let mut execution=crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_execution_limits(request.run.execution_limits);execution.output_directory=Some(parent.to_path_buf());
    let result=crate::run_v3_with_progress_and_cancellation(execution,token,progress)?;
    let current=inputs(&ready.operation,token)?;input_bindings(&retained(&result.run_directory)?,&current.index_identity,&current.directory)?;Ok(result)
}
fn retained(root:&Path)->Result<PipelineV3Plan> {
    ensure_canonical_directory(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let value:serde_json::Value=local::read_bounded(&workspace.plan(),RIFE_MAXIMUM_INDEX_BYTES)?;
    let plan=PipelineV3Plan::from_json_slice(&crate::provider::canonical_json_bytes(&value)?).map_err(mapped)?;
    if plan.payload.pipeline_name!="bounded-rife-preview"||plan.payload.inputs.len()!=2||plan.payload.outputs.len()!=2||plan.payload.stages.len()!=1
        ||plan.payload.stages[0].id!="rife_preview"||plan.payload.stages[0].capability.id!=CAPABILITY_ID||plan.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID{return Err(invalid("run is not a fixed bounded RIFE preview"));}Ok(plan)
}
pub fn resume<F:FnMut(&PipelineV3RunProgress)>(request:PreviewRequest,root:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let ready=prepare(&request,token)?;if ready.plan!=retained(root)?{return Err(invalid("RIFE preview window, complete source/index, tool/model/provider or settings changed; no stale reuse"));}
    let result=crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,ready.bindings,ready.registry).with_execution_limits(request.run.execution_limits),token,progress)?;
    let current=inputs(&ready.operation,token)?;input_bindings(&retained(root)?,&current.index_identity,&current.directory)?;Ok(result)
}
pub fn status(root:&Path)->Result<crate::PipelineV3RunManifest> {
    let plan=retained(root)?;let manifest=crate::status_v3(root)?;
    if manifest.payload.plan_sha256!=plan.plan_sha256||retained(root)?!=plan{return Err(invalid("RIFE preview native status differs from its exact retained plan"));}Ok(manifest)
}
pub fn observe(root:&Path,token:&CancellationToken)->Result<PreviewCompletion> {
    cancelled(token)?;let plan=retained(root)?;let before=status(root)?;
    if before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none(){return Err(invalid("preview completion requires current complete ordinary v3 acceptance"));}
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let values=before.payload.outputs.iter().filter(|a|a.id==id).collect::<Vec<_>>();if values.len()!=1{return Err(invalid("RIFE preview output missing or ambiguous"));}Ok(values[0])};
    let report_artifact=select("preview_report")?;let frames_artifact=select("preview_frames")?;
    let locate=|a:&crate::ArtifactEvidence|->Result<PathBuf>{let path=root.join(a.relative_path.as_deref().ok_or_else(||invalid("RIFE preview output locator missing"))?);if !path.starts_with(root)||path.canonicalize().ok().as_deref()!=Some(path.as_path()){return Err(invalid("RIFE preview output escaped canonical workspace"));}Ok(path)};
    let path=locate(report_artifact)?;let frames=locate(frames_artifact)?;
    if identity(&path,RIFE_MAXIMUM_INDEX_BYTES)?!=(report_artifact.sha256.clone(),report_artifact.byte_count){return Err(invalid("RIFE preview report changed"));}
    let report:PreviewReport=local::read_bounded(&path,RIFE_MAXIMUM_INDEX_BYTES)?;
    let source=inputs(&report.operation,token)?;report.validate_against(&source.index)?;input_bindings(&plan,&source.index_identity,&source.directory)?;
    let stage=&plan.payload.stages[0];
    if report.operation.provider_configuration()?.effective_configuration_sha256!=stage.provider_lock.payload.effective_configuration_sha256||stage.provider_lock.payload.manifest_sha256!=canonical_sha256(&report.operation.manifest()?)?||report.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||report.source_index_sha256!=source.index_identity.0||report.source_index_byte_count!=source.index_identity.1||report.source_frames_sha256!=source.directory.content_sha256 {return Err(invalid("RIFE preview report differs from actual accepted source/provider"));}
    let maximum=super::super::provider::frame_byte_bound(&source.index,&report.operation.configuration.settings)?;
    let names=report.frames.iter().map(|f|std::ffi::OsString::from(&f.frame.filename)).collect::<Vec<_>>();inventory(&frames,&names)?;
    for frame in &report.frames {cancelled(token)?;if inspect_png(&frames.join(&frame.frame.filename),source.index.width,source.index.height,maximum)?.0!=frame.frame.sha256{return Err(invalid("RIFE preview PNG changed"));}}
    let declaration=crate::AuthoredPipelineInput {id:"preview_frames".into(),artifact_type:"application/vnd.aniflow.rife-preview-frames".into(),artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video)};
    let directory=crate::pipeline_v3::hash_pipeline_input(&declaration,&frames).map_err(mapped)?;
    if directory.content_sha256!=frames_artifact.sha256||directory.byte_count!=frames_artifact.byte_count||directory.file_count!=frames_artifact.file_count{return Err(invalid("RIFE preview directory differs from accepted bytes"));}
    let checkpoint_ref=before.payload.stages.iter().find(|s|s.stage_id=="rife_preview").and_then(|s|s.checkpoint.as_ref()).ok_or_else(||invalid("RIFE preview checkpoint missing"))?;
    let checkpoint:crate::StageCheckpoint=local::read(&root.join(&checkpoint_ref.relative_path))?;checkpoint.validate()?;
    if checkpoint.checkpoint_sha256!=checkpoint_ref.checkpoint_sha256||checkpoint.payload.plan_sha256!=plan.plan_sha256||checkpoint.payload.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||![report_artifact,frames_artifact].iter().all(|a|checkpoint.payload.validations.iter().any(|v|v.accepted&&v.artifact_id==a.id&&v.artifact_sha256==a.sha256&&v.contract==crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1)){return Err(invalid("RIFE preview lacks current exact native checkpoint acceptance"));}
    let delivery=before.payload.delivery.as_ref().ok_or_else(||invalid("RIFE preview delivery acceptance missing"))?;let acceptance:crate::validation::AcceptanceRecord=local::read(&root.join(&delivery.relative_path))?;acceptance.validate()?;
    if acceptance.sha256()?!=delivery.sha256||acceptance.plan_sha256!=plan.plan_sha256||acceptance.layer!=crate::validation::ValidationLayer::Delivery{return Err(invalid("RIFE preview delivery acceptance changed"));}
    let current=inputs(&report.operation,token)?;
    if current.index_identity!=source.index_identity||current.directory!=source.directory{return Err(invalid("RIFE preview original source changed during completion observation"));}
    if status(root)?!=before||identity(&path,RIFE_MAXIMUM_INDEX_BYTES)?.0!=report_artifact.sha256{return Err(invalid("RIFE preview changed during current observation"));}
    Ok(PreviewCompletion {schema:"aniflow.rife-preview-completion/v1".into(),run_directory:root.to_path_buf(),plan_sha256:plan.plan_sha256,run_manifest_sha256:before.manifest_sha256.clone(),report_sha256:report_artifact.sha256.clone(),frames_directory:frames,report})
}
