use super::*;
use crate::frame_io::ensure_canonical_directory;
struct Prepared {plan:crate::PipelineV3Plan,registry:crate::ProviderRegistry,bindings:Vec<crate::PipelineInputBinding>,operation:LosslessOperation}
fn prepare(request:&LosslessRequest,token:&CancellationToken)->Result<Prepared> {
    cancelled(token)?;request.configuration.validate()?;
    let original=source(&request.source,&request.signal_run_directory,token)?;
    let operation=LosslessOperation {schema:SCHEMA.into(),source:original,configuration:request.configuration.clone()};operation.validate()?;observe_tools(&request.configuration)?;
    let value=serde_json::json!({"schema":"aniflow.pipeline/v3","name":"selected-lossless-audio","inputs":[{"id":"source_audio","artifact_type":"audio/wav","artifact_role":"temporal_source","stream_role":"audio"}],
        "stages":[{"id":"lossless_audio","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"lossless_audio"},"fallbacks":[]},
            "inputs":[{"port":"source","artifacts":["source_audio"]}],"outputs":[{"port":"media","artifacts":[{"id":"lossless_delivery","relative_path":format!("artifacts/lossless/{}",request.configuration.format.filename()),"kind":"file"}]},
                {"port":"evidence","artifacts":[{"id":"lossless_evidence","relative_path":"artifacts/lossless/private.json","kind":"file"}]}],
            "validations":[{"id":"lossless_file_integrity","artifact":"lossless_delivery","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"lossless_report_integrity","artifact":"lossless_evidence","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],
        "outputs":[{"id":"lossless_delivery","artifact":"lossless_delivery","required_validations":["lossless_file_integrity"]},{"id":"lossless_evidence","artifact":"lossless_evidence","required_validations":["lossless_report_integrity"]}]});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(&e.message))?;
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("lossless_audio",operation.manifest()?,operation.provider_configuration()?,request.provider_executable.clone(),"aniflow-lossless-audio-v1",components(&request.configuration))?)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_audio",&request.source)];
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(&e.message))?;
    let expected=&operation.source.summary.source.artifact;
    if !plan.payload.inputs.iter().any(|a|a.id=="source_audio"&&a.content_sha256==expected.sha256&&a.byte_count==expected.byte_size){return Err(invalid("lossless input changed during planning"));}
    cancelled(token)?;Ok(Prepared {plan,registry,bindings,operation})
}
pub fn plan(request:&LosslessRequest,token:&CancellationToken)->Result<crate::PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
pub fn run<F:FnMut(&crate::PipelineV3RunProgress)>(request:LosslessRequest,parent:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(parent)?;
    for original in [&request.source,&request.signal_run_directory,&request.configuration.tools.ffmpeg.executable,&request.configuration.tools.ffprobe.executable,&request.provider_executable] {
        if parent.starts_with(original)||original.starts_with(parent){return Err(invalid("lossless output parent overlaps source evidence or an executable"));}
    }
    let ready=prepare(&request,token)?;crate::video_assembly::require_storage(parent,ready.operation.working_bytes()?,request.configuration.minimum_free_bytes)?;
    let mut execution=crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_execution_limits(limits(&request.configuration));execution.output_directory=Some(parent.to_path_buf());
    let result=crate::run_v3_with_progress_and_cancellation(execution,token,progress)?;
    if source(&request.source,&request.signal_run_directory,token)?!=ready.operation.source{return Err(invalid("original acceptance changed during lossless execution; retain the run and reconcile"));}Ok(result)
}
fn retained(root:&Path)->Result<crate::PipelineV3Plan> {
    ensure_canonical_directory(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let bytes=crate::destination::local::read::<serde_json::Value>(&workspace.plan())?;
    let plan=crate::PipelineV3Plan::from_json_slice(&canonical_json_bytes(&bytes)?).map_err(|e|invalid(&e.message))?;
    if plan.payload.stages.len()!=1||plan.payload.stages[0].id!="lossless_audio"||plan.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID
        ||plan.payload.stages[0].capability.id!=CAPABILITY_ID||plan.payload.inputs.len()!=1||plan.payload.outputs.len()!=2 {return Err(invalid("run is not the fixed lossless audio profile"));}Ok(plan)
}
pub fn resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:LosslessRequest,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let ready=prepare(&request,token)?;
    if retained(root)?!=ready.plan{return Err(invalid("lossless source, signal evidence, configuration, tools or provider changed; no stale reuse"));}
    crate::video_assembly::require_storage(root,ready.operation.working_bytes()?,request.configuration.minimum_free_bytes)?;
    let execution=crate::PipelineV3ResumeRequest::new(root,ready.bindings,ready.registry).with_execution_limits(limits(&request.configuration));
    let result=crate::resume_v3_with_progress_and_cancellation(execution,token,progress)?;
    if source(&request.source,&request.signal_run_directory,token)?!=ready.operation.source{return Err(invalid("original acceptance changed during lossless resume"));}Ok(result)
}
/// Current read-only native-profile evidence. No codec/probe/meter is rerun.
pub fn observe(root:&Path,token:&CancellationToken)->Result<LosslessCompletion> {
    cancelled(token)?;let plan=retained(root)?;let before=crate::status_v3(root)?;
    if before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none()||before.payload.plan_sha256!=plan.plan_sha256 {return Err(invalid("lossless observation needs current complete v3 acceptance"));}
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let values=before.payload.outputs.iter().filter(|a|a.id==id).collect::<Vec<_>>();if values.len()!=1{return Err(invalid("lossless output missing or ambiguous"));}Ok(values[0])};
    let evidence=select("lossless_evidence")?;let output=select("lossless_delivery")?;
    let locate=|artifact:&crate::ArtifactEvidence|->Result<std::path::PathBuf>{let path=root.join(artifact.relative_path.as_deref().ok_or_else(||invalid("lossless locator missing"))?);if !path.starts_with(root)||identity(&path,MEDIA_BYTES)?!=(artifact.sha256.clone(),artifact.byte_count){return Err(invalid("lossless output changed"));}Ok(path)};
    let report_path=locate(evidence)?;let media=locate(output)?;let report:LosslessReport=local::read(&report_path)?;report.operation.validate()?;
    let s=&report.operation.source;let a=&s.summary.source;let stage=&plan.payload.stages[0];
    if report.schema!="aniflow.lossless-audio-report/v1"||report.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||report.operation.provider_configuration()?.effective_configuration_sha256!=stage.provider_lock.payload.effective_configuration_sha256
        ||!plan.payload.inputs.iter().any(|i|i.id=="source_audio"&&i.content_sha256==a.artifact.sha256&&i.byte_count==a.artifact.byte_size)
        ||report.output_sha256!=output.sha256||report.output_byte_count!=output.byte_count||report.output_byte_count>report.operation.configuration.maximum_output_bytes||report.decoded_pcm_sha256!=s.pcm_sha256
        ||report.sample_rate_hz!=a.sample_rate_hz||report.channels!=a.channels||report.frame_count!=a.frame_count||report.encoding!=s.encoding
        ||report.verification_method!=native::METHOD||report.signal_changes!="none"||report.metadata_scope!=native::METADATA
        ||report.after_export_meter!=native::METER||report.native_qualified||report.creative_approval!="not_performed"||report.publication_authority!="none" {
        return Err(invalid("lossless report differs from the current fixed producer/byte/clock/evidence bindings"));
    }
    if source(&s.file,&s.signal_run_directory,token)?!=*s{return Err(invalid("original signal evidence is no longer current"));}
    native::container(&media,&report.operation)?;
    if crate::status_v3(root)?!=before||identity(&report_path,local::DOCUMENT_BYTES)?!=(evidence.sha256.clone(),evidence.byte_count){return Err(invalid("lossless run changed during observation"));}
    cancelled(token)?;Ok(LosslessCompletion {schema:"aniflow.lossless-audio-completion/v1".into(),report,file:crate::destination::MediaFile {locator:media,sha256:output.sha256.clone(),byte_count:output.byte_count}})
}
