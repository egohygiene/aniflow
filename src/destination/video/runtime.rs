use super::*;
use std::path::{Path,PathBuf};
use crate::{PipelineV3Plan,PipelineV3RunProgress,PipelineV3RunOutcome,CancellationToken};
use crate::destination::cancelled;
use crate::frame_io::{identity,ensure_canonical_directory};
use serde_json::json;
pub(crate) struct Prepared {pub plan:PipelineV3Plan,pub registry:crate::ProviderRegistry,pub bindings:Vec<crate::PipelineInputBinding>,pub operation:VideoExportOperation}
pub(crate) fn prepare(request:&VideoExportRequest,token:&CancellationToken)->Result<Prepared> {
    let selected=intent(request,token)?;let operation=VideoExportOperation {schema:OPERATION_SCHEMA.into(),intent:selected};operation.validate()?;operation.working_bytes()?;
    let tools=&request.configuration.tools;crate::video_assembly::observe_components(tools)?;
    let selection=operation.selection();
    let value=json!({"schema":"aniflow.pipeline/v3","name":"selected-destination-video-export","inputs":[{"id":"source_video","artifact_type":"video/mp4","artifact_role":"temporal_source","stream_role":"video"}],
        "stages":[{"id":"destination_export","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"destination_export"},"fallbacks":[]},"inputs":[{"port":"source","artifacts":["source_video"]}],
            "outputs":[{"port":"media","artifacts":[{"id":"destination_video","relative_path":"artifacts/destination/delivery.mp4","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"destination_evidence","relative_path":"artifacts/destination/private.json","kind":"file"}]}],
            "validations":[{"id":"destination_temporal","artifact":"destination_video","contract":crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1,
                "validator":{"capability":{"id":crate::validation::native::NATIVE_VALIDATION_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"destination_validator"},"fallbacks":[]}},
                "temporal":{"source_artifact":"source_video","source_selection":selection,"artifact_selection":selection,"allow_authored_subtitles":false}},
                {"id":"destination_integrity","artifact":"destination_video","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"destination_evidence_integrity","artifact":"destination_evidence","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],
        "outputs":[{"id":"destination_video","artifact":"destination_video","required_validations":["destination_temporal","destination_integrity"]},{"id":"destination_evidence","artifact":"destination_evidence","required_validations":["destination_evidence_integrity"]}]});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(&e.message))?;
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("destination_export",operation.manifest()?,operation.configuration()?,request.provider_executable.clone(),"aniflow-destination-video-v1",crate::video_assembly::component_inventory(tools))?)?;
    registry.register(crate::validation::native::registration("destination_validator",request.provider_executable.clone(),crate::video_assembly::native_validation_settings(&validator_tools(&operation)),"video/mp4","video/mp4")?)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_video",&operation.intent.payload.artifact.locator)];
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(&e.message))?;
    if plan.payload.inputs.len()!=1||plan.payload.inputs[0].content_sha256!=operation.intent.payload.artifact.sha256||plan.payload.inputs[0].byte_count!=operation.intent.payload.artifact.byte_count{return Err(invalid("accepted destination source changed during planning"));}
    recheck(&operation.intent,token)?;Ok(Prepared {plan,registry,bindings,operation})
}
fn bounds(op:&VideoExportOperation)->crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits {timeout:std::time::Duration::from_millis(op.intent.payload.request.configuration.tools.timeout_milliseconds.saturating_add(120_000)),termination_grace_period:std::time::Duration::from_millis(2_000),maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,maximum_artifact_files:2,maximum_artifact_bytes:op.output_limit()+REPORT_BYTES}
}
fn validator_tools(op:&VideoExportOperation)->crate::video_assembly::AssemblyConfiguration {
    let mut tools=op.intent.payload.request.configuration.tools.clone();tools.maximum_source_bytes=op.intent.payload.artifact.byte_count;tools.maximum_output_bytes=op.output_limit();tools
}
fn disjoint(root:&Path,request:&VideoExportRequest)->Result<()> {
    for input in [&request.source_run_directory,&request.configuration.tools.ffmpeg.executable,&request.configuration.tools.ffprobe.executable,&request.provider_executable] {if root.starts_with(input)||input.starts_with(root){return Err(invalid("destination output overlaps its source evidence or tools"));}}Ok(())
}
pub fn plan(request:&VideoExportRequest,token:&CancellationToken)->Result<PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
pub fn run<F:FnMut(&PipelineV3RunProgress)>(request:VideoExportRequest,parent:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    ensure_canonical_directory(parent)?;disjoint(parent,&request)?;let p=prepare(&request,token)?;
    crate::video_assembly::require_storage(parent,p.operation.working_bytes()?,request.configuration.tools.minimum_free_bytes)?;crate::video_assembly::require_validator_storage(&validator_tools(&p.operation))?;
    let limits=bounds(&p.operation);let result=crate::run_v3_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(p.plan,p.bindings,p.registry).with_output_directory(parent).with_execution_limits(limits),token,progress)?;
    recheck(&p.operation.intent,token)?;Ok(result)
}
pub(crate) fn run_at<F:FnMut(&PipelineV3RunProgress)>(request:VideoExportRequest,root:&Path,expected:&PipelineV3Plan,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let parent=crate::run_v3::exact_run_parent(root)?;disjoint(root,&request)?;let p=prepare(&request,token)?;if p.plan!=*expected{return Err(invalid("destination child intent changed before initialization"));}
    crate::video_assembly::require_storage(&parent,p.operation.working_bytes()?,request.configuration.tools.minimum_free_bytes)?;crate::video_assembly::require_validator_storage(&validator_tools(&p.operation))?;
    let limits=bounds(&p.operation);let result=crate::run_v3::run_v3_at_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(p.plan,p.bindings,p.registry).with_output_directory(parent).with_execution_limits(limits),root,token,progress)?;recheck(&p.operation.intent,token)?;Ok(result)
}
pub fn resume<F:FnMut(&PipelineV3RunProgress)>(request:VideoExportRequest,root:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    ensure_canonical_directory(root)?;let p=prepare(&request,token)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;let v:serde_json::Value=read_document(&workspace.plan())?;
    if PipelineV3Plan::from_json_slice(&canonical_json_bytes(&v)?).map_err(|e|invalid(&e.message))?!=p.plan{return Err(invalid("destination source/profile/geometry/metadata/tools changed; no stale resume"));}
    crate::video_assembly::require_storage(root,p.operation.working_bytes()?,request.configuration.tools.minimum_free_bytes)?;crate::video_assembly::require_validator_storage(&validator_tools(&p.operation))?;
    let limits=bounds(&p.operation);let result=crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,p.bindings,p.registry).with_execution_limits(limits),token,progress)?;recheck(&p.operation.intent,token)?;Ok(result)
}
pub(crate) fn accepted(root:&Path,token:&CancellationToken)->Result<(PipelineV3Plan,crate::PipelineV3RunManifest,VideoExportReport,PathBuf)> {
    cancelled(token)?;ensure_canonical_directory(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;let v:serde_json::Value=read_document(&workspace.plan())?;
    let plan=PipelineV3Plan::from_json_slice(&canonical_json_bytes(&v)?).map_err(|e|invalid(&e.message))?;let before=crate::status_v3(root)?;
    if plan.payload.pipeline_name!="selected-destination-video-export"||plan.payload.stages.len()!=1||plan.payload.outputs.len()!=2||plan.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID
        ||before.payload.plan_sha256!=plan.plan_sha256||before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none(){return Err(invalid("destination observation requires exact current native completion"));}
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let values=before.payload.outputs.iter().filter(|a|a.id==id).collect::<Vec<_>>();if values.len()!=1{return Err(invalid("destination output missing or ambiguous"));}Ok(values[0])};
    let media=select("destination_video")?;let evidence=select("destination_evidence")?;
    let locate=|a:&crate::ArtifactEvidence|->Result<PathBuf>{let path=root.join(a.relative_path.as_deref().ok_or_else(||invalid("destination output locator missing"))?);if !path.starts_with(root)||identity(&path,REPORT_BYTES.max(a.byte_count))?!=(a.sha256.clone(),a.byte_count){return Err(invalid("accepted destination output changed"));}Ok(path)};
    let media_path=locate(media)?;let report_path=locate(evidence)?;let report:VideoExportReport=read_document(&report_path)?;report.operation.validate()?;
    let stage=&plan.payload.stages[0];
    if report.schema!="aniflow.destination-video-report/v1"||report.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||report.operation.configuration()?.effective_configuration_sha256!=stage.provider_lock.payload.effective_configuration_sha256
        ||report.output_sha256!=media.sha256||report.output_bytes!=media.byte_count||report.output_bytes==0||report.output_bytes>report.operation.output_limit()
        ||!report.media.audio_packets_preserved||!report.media.complete_clock_preserved||!report.media.full_decode_observed||report.media.native_qualified
        ||report.media.source.source_sha256!=report.operation.intent.payload.artifact.sha256||report.media.source.source_size_bytes!=report.operation.intent.payload.artifact.byte_count
        ||report.media.candidate.source_sha256!=media.sha256||report.media.candidate.source_size_bytes!=media.byte_count||report.media.geometry!=report.operation.intent.payload.geometry
        ||report.media.metadata.source_sha256!=report.media.source.source_sha256||report.media.metadata.candidate_sha256!=media.sha256
        ||report.media.metadata.public_summary.status!=crate::delivery_metadata::MetadataInspectionStatus::CleanWithinScope
        ||report.media.playback.audio!=report.operation.intent.payload.artifact.audio
        ||report.media.playback.video_codec!="h264"||report.media.playback.pixel_format!="yuv420p"||report.media.playback.color_primaries!="bt709"
        ||report.media.playback.color_transfer!="bt709"||report.media.playback.color_matrix!="bt709"||report.media.playback.color_range!="tv"
        ||!crate::final_video::square_pixels(&report.media.playback.sample_aspect_ratio)||report.media.playback.orientation_degrees!=0||!report.media.playback.faststart
        ||report.media.metadata.effective_policy!=report.operation.intent.payload.request.configuration.tools.delivery_metadata.clone().ok_or_else(||invalid("accepted destination metadata policy missing"))?
        ||report.quality_approval!="not_performed; lossy export requires separate final creative review"||report.publication_authority!="none" {
        return Err(invalid("destination report differs from current native identities/profile/acceptance"));
    }
    let checkpoint_ref=before.payload.stages.iter().find(|s|s.stage_id=="destination_export").and_then(|s|s.checkpoint.as_ref()).ok_or_else(||invalid("destination native checkpoint missing"))?;
    let checkpoint=crate::load_stage_checkpoint(&workspace,checkpoint_ref)?;
    if checkpoint.payload.plan_sha256!=plan.plan_sha256||checkpoint.payload.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||!checkpoint.payload.validations.iter().any(|v|v.artifact_id==media.id&&v.artifact_sha256==media.sha256&&v.accepted&&v.contract==crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1)
        ||!checkpoint.payload.validations.iter().any(|v|v.artifact_id==evidence.id&&v.artifact_sha256==evidence.sha256&&v.accepted&&v.contract==crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1) {
        return Err(invalid("destination lacks exact accepted native temporal/report gates"));
    }
    crate::delivery_metadata::mp4::agrees(&report.media.container,&report.media.metadata.after)?;recheck(&report.operation.intent,token)?;
    crate::temporal::validate_reconstruction(&report.media.source,&report.media.candidate,false).map_err(|_|invalid("accepted destination report clock changed"))?;
    if crate::status_v3(root)?!=before||identity(&report_path,REPORT_BYTES)?!=(evidence.sha256.clone(),evidence.byte_count){return Err(invalid("destination native authority changed during observation"));}
    Ok((plan,before,report,media_path))
}
pub fn observe(root:&Path,token:&CancellationToken)->Result<VideoExportReport> {Ok(accepted(root,token)?.2)}
