use super::*;
use crate::frame_io::ensure_canonical_directory;
struct Prepared {plan:crate::PipelineV3Plan,operation:Operation,registry:crate::ProviderRegistry,bindings:Vec<crate::PipelineInputBinding>}
fn prepare(request:&Request,token:&CancellationToken)->Result<Prepared> {
    let operation=current(request,token)?;
    let value=serde_json::json!({"schema":"aniflow.pipeline/v3","name":"selected-gain-video","inputs":[{"id":"gain_video_source","artifact_type":"video/mp4","artifact_role":"temporal_source","stream_role":"video"},{"id":"gain_video_audio","artifact_type":"audio/wav","artifact_role":"temporal_source","stream_role":"audio"}],"stages":[{"id":"gain_video","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"gain_video"},"fallbacks":[]},"inputs":[{"port":"video","artifacts":["gain_video_source"]},{"port":"audio","artifacts":["gain_video_audio"]}],"outputs":[{"port":"media","artifacts":[{"id":"gain_video","relative_path":"artifacts/gain-video/video.mp4","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"gain_video_evidence","relative_path":"artifacts/gain-video/private.json","kind":"file"}]}],"validations":[{"id":"gain_video_integrity","artifact":"gain_video","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"gain_video_report_integrity","artifact":"gain_video_evidence","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],"outputs":[{"id":"gain_video","artifact":"gain_video","required_validations":["gain_video_integrity"]},{"id":"gain_video_evidence","artifact":"gain_video_evidence","required_validations":["gain_video_report_integrity"]}]});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(&e.message))?;
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("gain_video",operation.manifest()?,operation.configuration()?,request.provider_executable.clone(),"aniflow-gain-video-v1",crate::video_assembly::component_inventory(&request.settings))?)?;
    let bindings=vec![crate::PipelineInputBinding::new("gain_video_source",&operation.artifact.locator),crate::PipelineInputBinding::new("gain_video_audio",&operation.gain.file.locator)];
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(&e.message))?;
    for (id,pin,size) in [("gain_video_source",&operation.artifact.sha256,operation.artifact.byte_count),("gain_video_audio",&operation.gain.file.sha256,operation.gain.file.byte_count)] {
        if !plan.payload.inputs.iter().any(|i|i.id==id&&&i.content_sha256==pin&&i.byte_count==size){return Err(invalid("gain video input changed during native planning"));}
    }
    if current(request,token)?!=operation{return Err(invalid("gain video source/QC/components changed during planning"));}
    Ok(Prepared {plan,operation,registry,bindings})
}
pub fn plan(request:&Request,token:&CancellationToken)->Result<crate::PipelineV3Plan>{Ok(prepare(request,token)?.plan)}
fn limits(request:&Request)->crate::ProviderExecutionLimits {crate::ProviderExecutionLimits {timeout:std::time::Duration::from_millis(request.settings.timeout_milliseconds.saturating_mul(12).saturating_add(30_000)),termination_grace_period:std::time::Duration::from_secs(2),maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,maximum_artifact_files:64,maximum_artifact_bytes:request.settings.maximum_output_bytes.saturating_add(REPORT_BYTES)}}
fn disjoint(root:&Path,op:&Operation)->Result<()> {
    let mut paths=vec![op.artifact.locator.as_path(),op.gain.file.locator.as_path(),op.request.gain_run_directory.as_path(),op.request.gain_qc_directory.as_path()];
    if let MediaInput::AcceptedVideoWithQc {run_directory,qc_directory,..}=&op.gain.report.operation.request.check.input {paths.extend([run_directory.as_path(),qc_directory.as_path()]);}
    paths.extend(op.components.iter().map(|c|c.path.as_path()));
    if paths.iter().any(|p|root.starts_with(p)||p.starts_with(root)){return Err(invalid("gain video output overlaps original/candidate/QC/component"));}Ok(())
}
pub fn run<F:FnMut(&crate::PipelineV3RunProgress)>(request:&Request,parent:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(parent)?;let ready=prepare(request,token)?;disjoint(parent,&ready.operation)?;
    crate::video_assembly::require_storage(parent,ready.operation.working_bytes()?,request.settings.minimum_free_bytes)?;
    let result=crate::run_v3_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_output_directory(parent).with_execution_limits(limits(request)),token,progress)?;
    if current(request,token)?!=ready.operation{return Err(invalid("gain video source/QC/components changed during execution; preserve run"));}Ok(result)
}
fn retained(root:&Path)->Result<crate::PipelineV3Plan> {
    ensure_canonical_directory(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let value=local::read::<serde_json::Value>(&workspace.plan())?;let plan=crate::PipelineV3Plan::from_json_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(&e.message))?;
    if plan.payload.pipeline_name!="selected-gain-video"||plan.payload.stages.len()!=1||plan.payload.stages[0].id!="gain_video"||plan.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID||plan.payload.stages[0].capability.id!=CAPABILITY_ID||plan.payload.inputs.len()!=2||plan.payload.outputs.len()!=2{return Err(invalid("not a fixed gain video native run"));}Ok(plan)
}
pub fn resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:&Request,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let ready=prepare(request,token)?;disjoint(root,&ready.operation)?;if retained(root)?!=ready.plan{return Err(invalid("gain video source/QC/settings/provider changed; no stale reuse"));}
    crate::video_assembly::require_storage(root,ready.operation.working_bytes()?,request.settings.minimum_free_bytes)?;
    let result=crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,ready.bindings,ready.registry).with_execution_limits(limits(request)),token,progress)?;
    if current(request,token)?!=ready.operation{return Err(invalid("gain video context changed during resume"));}Ok(result)
}

pub fn completion(root:&Path,token:&CancellationToken)->Result<FinalVideoHandoff> {
    cancelled(token)?;let plan=retained(root)?;let before=crate::status_v3(root)?;
    if before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none()||before.payload.plan_sha256!=plan.plan_sha256{return Err(invalid("gain video completion requires current native acceptance"));}
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let v=before.payload.outputs.iter().filter(|a|a.id==id).collect::<Vec<_>>();if v.len()!=1{return Err(invalid("gain video output missing/ambiguous"));}v[0].validate()?;Ok(v[0])};
    let audio_video=select("gain_video")?;let evidence=select("gain_video_evidence")?;
    let locate=|item:&crate::ArtifactEvidence,maximum:u64|->Result<PathBuf>{let path=root.join(item.relative_path.as_deref().ok_or_else(||invalid("gain video locator missing"))?);if !path.starts_with(root)||path.canonicalize().ok().as_deref()!=Some(path.as_path())||identity(&path,maximum)?!=(item.sha256.clone(),item.byte_count){return Err(invalid("gain video owned bytes changed"));}Ok(path)};
    let report_path=locate(evidence,REPORT_BYTES)?;let report:Report=local::read_bounded(&report_path,REPORT_BYTES)?;report.operation.validate()?;
    let op=&report.operation;let path=locate(audio_video,op.request.settings.maximum_output_bytes)?;let stage=&plan.payload.stages[0];let m=&report.media;
    for (id,pin,size) in [("gain_video_source",&op.artifact.sha256,op.artifact.byte_count),("gain_video_audio",&op.gain.file.sha256,op.gain.file.byte_count)] {
        if !plan.payload.inputs.iter().any(|i|i.id==id&&&i.content_sha256==pin&&i.byte_count==size){return Err(invalid("gain video current native input identity differs"));}
    }
    let timeline=m.source.video_timeline().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;
    crate::temporal::validate_reconstruction(&m.source,&m.candidate,false).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;
    if report.schema!="aniflow.gain-video-report/v1"||report.provider_lock_sha256!=stage.provider_lock.lock_sha256||op.configuration()?.effective_configuration_sha256!=stage.provider_lock.payload.effective_configuration_sha256
        ||report.output_sha256!=audio_video.sha256||report.output_bytes!=audio_video.byte_count||report.output_policy_status!="requires_fresh_encoded_file_qc"||report.creative_approval!="not_performed"||report.publication_authority!="none"
        ||m.source.source_sha256!=op.artifact.sha256||m.source.source_size_bytes!=op.artifact.byte_count||m.candidate.source_sha256!=audio_video.sha256||m.candidate.source_size_bytes!=audio_video.byte_count
        ||timeline.frames.len() as u64!=op.artifact.frame_count||timeline.source_time!=Some(op.artifact.video_interval)||timeline.frame_period!=Some(op.artifact.frame_rate.reciprocal().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?)
        ||!m.full_decode_observed||m.native_qualified||m.playback.audio.as_ref().map(|a|a.interval)!=op.artifact.audio.as_ref().map(|a|a.interval)
        ||m.metadata.source_sha256!=op.artifact.sha256||m.metadata.candidate_sha256!=audio_video.sha256||m.metadata.effective_policy!=op.request.settings.delivery_metadata.clone().ok_or_else(||invalid("gain video metadata policy missing"))?
        ||m.metadata.public_summary.status!=crate::delivery_metadata::MetadataInspectionStatus::CleanWithinScope||current(&op.request,token)?!=*op {
        return Err(invalid("gain video report differs from current exact source/QC/native temporal observations"));
    }
    crate::provider::require_sha256(&m.video_packets_sha256,"gain video unchanged picture packets")?;
    crate::delivery_metadata::mp4::agrees(&m.container,&m.metadata.after)?;
    let props=&m.playback;let audio=props.audio.as_ref().ok_or_else(||invalid("gain video accepted audio missing"))?;
    if props.video_codec!="h264"||props.pixel_format!="yuv420p"||props.color_primaries!="bt709"||props.color_transfer!="bt709"||props.color_matrix!="bt709"||props.color_range!="tv"||!crate::final_video::square_pixels(&props.sample_aspect_ratio)||props.orientation_degrees!=0||!props.faststart||audio.codec!="aac"||audio.profile!="LC"||audio.sample_rate_hz!=48_000||audio.channels!=u64::from(op.gain.report.channels){return Err(invalid("gain video playback/source channel facts differ"));}
    let reference=before.payload.stages.iter().find(|s|s.stage_id==stage.id).and_then(|s|s.checkpoint.as_ref()).ok_or_else(||invalid("gain video accepted checkpoint missing"))?;
    let workspace=crate::PipelineV3Workspace::open_read_only(root)?;let checkpoint=crate::load_stage_checkpoint(&workspace,reference)?;
    if checkpoint.payload.plan_sha256!=plan.plan_sha256||checkpoint.payload.provider_lock_sha256!=stage.provider_lock.lock_sha256||![audio_video,evidence].iter().all(|a|checkpoint.payload.validations.iter().any(|v|v.artifact_id==a.id&&v.artifact_sha256==a.sha256&&v.accepted&&v.contract==crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1)){return Err(invalid("gain video lacks exact current native artifact acceptance"));}
    let delivery=before.payload.delivery.as_ref().ok_or_else(||invalid("gain video delivery acceptance missing"))?;
    let acceptance:crate::validation::AcceptanceRecord=local::read_bounded(&root.join(&delivery.relative_path),REPORT_BYTES)?;acceptance.validate()?;
    if acceptance.sha256()?!=delivery.sha256||acceptance.plan_sha256!=plan.plan_sha256||acceptance.layer!=crate::validation::ValidationLayer::Delivery{return Err(invalid("gain video current delivery acceptance differs"));}
    let mut artifact=op.artifact.clone();artifact.artifact_id="gain_video".into();artifact.locator=path;artifact.sha256=audio_video.sha256.clone();artifact.byte_count=audio_video.byte_count;artifact.audio=Some(audio.clone());artifact.metadata=Some(m.metadata.public_summary.clone());artifact.producer_lock_sha256=stage.provider_lock.lock_sha256.clone();artifact.stage_invocation_sha256=checkpoint.payload.stage_invocation_sha256;artifact.stage_checkpoint_sha256=reference.checkpoint_sha256.clone();artifact.private_report_sha256=evidence.sha256.clone();
    let id=canonical_sha256(&(crate::final_video::FINAL_VIDEO_HANDOFF_V1,&plan.plan_sha256,&delivery.sha256,vec![(&artifact.artifact_id,&artifact.sha256,artifact.byte_count)]))?;
    if crate::status_v3(root)?!=before||identity(&report_path,REPORT_BYTES)?!=(evidence.sha256.clone(),evidence.byte_count){return Err(invalid("gain video changed during completion readback"));}
    Ok(FinalVideoHandoff {schema:crate::final_video::FINAL_VIDEO_HANDOFF_V1.into(),completion_id:id,plan_sha256:plan.plan_sha256,run_manifest_sha256:before.manifest_sha256.clone(),delivery_acceptance_sha256:delivery.sha256.clone(),source_lineage:plan.payload.inputs.iter().map(|i|crate::final_video::FinalSourceIdentity {id:i.id.clone(),artifact_type:i.artifact_type.clone(),sha256:i.content_sha256.clone(),byte_count:i.byte_count,file_count:i.file_count}).collect(),artifacts:vec![artifact],creative_approval:"not_recorded_by_fixed_automated_profile".into(),publication_authority:"none".into(),proof_state:"not_asserted".into()})
}
