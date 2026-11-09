use super::*;
use std::path::PathBuf;
struct Ready {operation:CaptionOperation,plan:crate::PipelineV3Plan,bindings:Vec<crate::PipelineInputBinding>,registry:crate::ProviderRegistry}
fn prepare(operation:CaptionOperation,provider:&Path,token:&CancellationToken)->Result<Ready> {
    cancelled(token)?;operation.validate()?;
    if identity(provider,crate::video_assembly::MAXIMUM_TOOL_BYTES)?.0!=operation.renderer_sha256{return Err(invalid("caption renderer differs from its preview-bound implementation"));}
    let work=operation.work;let audio=operation.source.artifact.audio.is_some();
    let selection=crate::temporal::StreamSelection {video_stream:Some(0),audio_stream:audio.then_some(1),no_audio:!audio,discard_streams:vec![],audio_extent:Some(crate::temporal::AudioExtentPolicy::PreserveExact)};
    let mut validations=vec![serde_json::json!({"id":"caption_media_integrity","artifact":work.artifact(),"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}),serde_json::json!({"id":"caption_report_integrity","artifact":work.evidence(),"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1})];
    let mut required=vec!["caption_media_integrity"];
    if work==CaptionWork::Deliver {validations.push(serde_json::json!({"id":"caption_temporal","artifact":work.artifact(),"contract":crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1,
        "validator":{"capability":{"id":crate::validation::native::NATIVE_VALIDATION_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"caption_validator"},"fallbacks":[]}},
        "temporal":{"source_artifact":"caption_source","source_selection":selection,"artifact_selection":selection,"allow_authored_subtitles":operation.request.configuration.mode==CaptionMode::SoftMux}}));required.push("caption_temporal");}
    let value=serde_json::json!({"schema":"aniflow.pipeline/v3","name":work.stage(),"inputs":[{"id":"caption_source","artifact_type":"video/mp4","artifact_role":"temporal_source","stream_role":"video"},{"id":"caption_srt","artifact_type":"application/x-subrip","artifact_role":"temporal_component","stream_role":"subtitle"},{"id":"caption_font","artifact_type":"application/octet-stream","artifact_role":"intermediate","stream_role":"attachment"}],
        "stages":[{"id":work.stage(),"depends_on":[],"capability":{"id":work.capability(),"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"caption_renderer"},"fallbacks":[]},
            "inputs":[{"port":"source","artifacts":["caption_source"]},{"port":"srt","artifacts":["caption_srt"]},{"port":"font","artifacts":["caption_font"]}],
            "outputs":[{"port":"media","artifacts":[{"id":work.artifact(),"relative_path":if work==CaptionWork::Preview {"artifacts/captions/frames"}else{"artifacts/captions/captioned.mp4"},"kind":if work==CaptionWork::Preview {"directory"}else{"file"}}]},
                {"port":"evidence","artifacts":[{"id":work.evidence(),"relative_path":"artifacts/captions/private.json","kind":"file"}]}],"validations":validations}],
        "outputs":[{"id":work.artifact(),"artifact":work.artifact(),"required_validations":required},{"id":work.evidence(),"artifact":work.evidence(),"required_validations":["caption_report_integrity"]}]});
    let configuration=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(&e.message))?;
    let mut registry=crate::ProviderRegistry::new();let settings=&operation.request.configuration.settings;
    registry.register(crate::ProviderRegistration::new("caption_renderer",operation.manifest()?,operation.configuration()?,provider.to_path_buf(),"aniflow-caption-render-v1",crate::video_assembly::component_inventory(settings))?)?;
    if work==CaptionWork::Deliver {registry.register(crate::validation::native::registration("caption_validator",provider.to_path_buf(),crate::video_assembly::native_validation_settings(settings),"video/mp4","video/mp4")?)?;}
    let bindings=vec![crate::PipelineInputBinding::new("caption_source",&operation.source.artifact.locator),crate::PipelineInputBinding::new("caption_srt",&operation.lyrics.srt_file),crate::PipelineInputBinding::new("caption_font",&operation.request.configuration.font.file)];
    let plan=crate::resolve_pipeline_v3(&configuration,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(&e.message))?;
    for (id,sha,size) in [("caption_source",operation.source.artifact.sha256.as_str(),operation.source.artifact.byte_count),("caption_srt",operation.lyrics.srt_sha256.as_str(),operation.lyrics.srt_byte_count),("caption_font",operation.request.configuration.font.sha256.as_str(),operation.font_byte_count)] {
        if !plan.payload.inputs.iter().any(|i|i.id==id&&i.content_sha256==sha&&i.byte_count==size){return Err(invalid("caption inputs changed during planning"));}
    }
    cancelled(token)?;Ok(Ready {operation,plan,bindings,registry})
}
fn preview_operation(request:&CaptionRunRequest,token:&CancellationToken)->Result<CaptionOperation> {capture(&request.request,&identity(&request.provider_executable,crate::video_assembly::MAXIMUM_TOOL_BYTES)?.0,token)}
fn delivery_operation(request:&CaptionDeliveryRequest,token:&CancellationToken)->Result<CaptionOperation> {
    let mut operation=preview_operation(&request.run,token)?;let preview=preview_status(&request.preview_run_directory,token)?;
    if preview.binding.context_sha256!=operation.context()? {return Err(invalid("accepted caption preview differs from current source, carrier, font, style or implementation"));}
    operation.work=CaptionWork::Deliver;operation.preview=Some(preview.binding);operation.declaration=Some(request.declaration.clone());operation.validate()?;Ok(operation)
}
pub fn preview_plan(request:&CaptionRunRequest,token:&CancellationToken)->Result<crate::PipelineV3Plan> {Ok(prepare(preview_operation(request,token)?,&request.provider_executable,token)?.plan)}
pub fn delivery_plan(request:&CaptionDeliveryRequest,token:&CancellationToken)->Result<crate::PipelineV3Plan> {Ok(prepare(delivery_operation(request,token)?,&request.run.provider_executable,token)?.plan)}
fn require_current(operation:&CaptionOperation,token:&CancellationToken)->Result<()> {
    let mut current=capture(&operation.request,&operation.renderer_sha256,token)?;
    if operation.work==CaptionWork::Deliver {
        let binding=operation.preview.as_ref().ok_or_else(||invalid("caption preview binding missing"))?;
        if preview_status(&binding.run_directory,token)?.binding!=*binding{return Err(invalid("caption preview changed"));}
        current.work=CaptionWork::Deliver;current.preview=operation.preview.clone();current.declaration=operation.declaration.clone();
    }
    if current!=*operation{return Err(invalid("caption source/reviews/style/font/settings changed; retain existing work for explicit reconciliation"));}Ok(())
}
fn limits(operation:&CaptionOperation)->crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits {timeout:std::time::Duration::from_secs(86_400),termination_grace_period:std::time::Duration::from_secs(2),maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,maximum_artifact_files:256,maximum_artifact_bytes:operation.request.configuration.settings.maximum_working_bytes}
}
fn run_ready<F:FnMut(&crate::PipelineV3RunProgress)>(ready:Ready,provider:&Path,parent:&Path,resume:bool,exact:Option<&Path>,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(parent)?;let op=&ready.operation;
    if !resume {
        let target=exact.unwrap_or(parent);
        for input in [&op.source.artifact.locator,&op.request.video_run_directory,&op.request.lyrics_run_directory,&op.request.lyrics_delivery_directory,&op.request.configuration.font.file,&op.request.configuration.settings.ffmpeg.executable,&op.request.configuration.settings.ffprobe.executable,provider] {
            if target.starts_with(input)||input.starts_with(target){return Err(invalid("caption run location overlaps source evidence or an executable/font"));}
        }
        if op.preview.as_ref().is_some_and(|p|target.starts_with(&p.run_directory)||p.run_directory.starts_with(target)){return Err(invalid("caption run location overlaps the accepted preview"));}
    } else if retained(parent,op.work)?!=ready.plan {return Err(invalid("caption resume inputs, approval, source or implementation changed"));}
    crate::video_assembly::require_storage(parent,op.working_bytes()?,op.request.configuration.settings.minimum_free_bytes)?;
    if op.work==CaptionWork::Deliver {crate::video_assembly::require_validator_storage(&op.request.configuration.settings)?;}
    let execution_limits=limits(op);
    let result=if resume {crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(parent,ready.bindings,ready.registry).with_execution_limits(execution_limits),token,progress)?}
        else {let mut request=crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_execution_limits(execution_limits);request.output_directory=Some(parent.to_path_buf());match exact {Some(root)=>crate::run_v3::run_v3_at_with_progress_and_cancellation(request,root,token,progress)?,None=>crate::run_v3_with_progress_and_cancellation(request,token,progress)?}};
    require_current(&ready.operation,token)?;Ok(result)
}
pub fn preview_run<F:FnMut(&crate::PipelineV3RunProgress)>(request:CaptionRunRequest,parent:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {let ready=prepare(preview_operation(&request,token)?,&request.provider_executable,token)?;run_ready(ready,&request.provider_executable,parent,false,None,token,progress)}
pub fn preview_resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:CaptionRunRequest,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {let ready=prepare(preview_operation(&request,token)?,&request.provider_executable,token)?;run_ready(ready,&request.provider_executable,root,true,None,token,progress)}
pub fn delivery_run<F:FnMut(&crate::PipelineV3RunProgress)>(request:CaptionDeliveryRequest,parent:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {let ready=prepare(delivery_operation(&request,token)?,&request.run.provider_executable,token)?;run_ready(ready,&request.run.provider_executable,parent,false,None,token,progress)}
pub fn delivery_resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:CaptionDeliveryRequest,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {let ready=prepare(delivery_operation(&request,token)?,&request.run.provider_executable,token)?;run_ready(ready,&request.run.provider_executable,root,true,None,token,progress)}
pub(crate) fn preview_run_at<F:FnMut(&crate::PipelineV3RunProgress)>(request:CaptionRunRequest,root:&Path,expected:&crate::PipelineV3Plan,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let ready=prepare(preview_operation(&request,token)?,&request.provider_executable,token)?;if ready.plan!=*expected{return Err(invalid("caption preview changed before fixed child initialization"));}
    run_ready(ready,&request.provider_executable,&crate::run_v3::exact_run_parent(root)?,false,Some(root),token,progress)
}
pub(crate) fn delivery_run_at<F:FnMut(&crate::PipelineV3RunProgress)>(request:CaptionDeliveryRequest,root:&Path,expected:&crate::PipelineV3Plan,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let ready=prepare(delivery_operation(&request,token)?,&request.run.provider_executable,token)?;if ready.plan!=*expected{return Err(invalid("caption delivery changed before fixed child initialization"));}
    run_ready(ready,&request.run.provider_executable,&crate::run_v3::exact_run_parent(root)?,false,Some(root),token,progress)
}
fn retained(root:&Path,work:CaptionWork)->Result<crate::PipelineV3Plan> {
    ensure_canonical_directory(root)?;let path=crate::PipelineV3Workspace::open_read_only(root)?.plan();let value:serde_json::Value=local::read(&path)?;
    let plan=crate::PipelineV3Plan::from_json_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(&e.message))?;
    if plan.payload.stages.len()!=1||plan.payload.inputs.len()!=3||plan.payload.outputs.len()!=2||plan.payload.stages[0].id!=work.stage()||plan.payload.pipeline_name!=work.stage()||plan.payload.stages[0].capability.id!=work.capability()||plan.payload.stages[0].provider_lock.payload.provider.id!=PROVIDER_ID{return Err(invalid("run is not the selected fixed caption profile"));}Ok(plan)
}
fn accepted(root:&Path,work:CaptionWork)->Result<(crate::PipelineV3Plan,crate::PipelineV3RunManifest,PathBuf,PathBuf,String)> {
    let plan=retained(root,work)?;let manifest=crate::status_v3(root)?;
    if manifest.payload.state!=crate::PipelineV3RunState::Complete||manifest.payload.delivery.is_none()||manifest.payload.plan_sha256!=plan.plan_sha256{return Err(invalid("caption observation requires current complete native acceptance"));}
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let values=manifest.payload.outputs.iter().filter(|a|a.id==id).collect::<Vec<_>>();if values.len()!=1{return Err(invalid("caption output missing or ambiguous"));}Ok(values[0])};
    let report=select(work.evidence())?;let media=select(work.artifact())?;
    let locate=|e:&crate::ArtifactEvidence|->Result<PathBuf>{let path=root.join(e.relative_path.as_deref().ok_or_else(||invalid("caption output locator missing"))?);if !path.starts_with(root)||path.canonicalize().ok().as_deref()!=Some(path.as_path()){return Err(invalid("caption output is not canonical/root-confined"));}Ok(path)};
    let report_path=locate(report)?;let media_path=locate(media)?;
    if identity(&report_path,local::DOCUMENT_BYTES)?!=(report.sha256.clone(),report.byte_count){return Err(invalid("caption report changed"));}
    if work==CaptionWork::Deliver&&identity(&media_path,u64::MAX)?!=(media.sha256.clone(),media.byte_count){return Err(invalid("caption media differs from native acceptance"));}
    let stage=&plan.payload.stages[0];
    let checkpoint_ref=manifest.payload.stages.iter().find(|s|s.stage_id==work.stage()).and_then(|s|s.checkpoint.as_ref()).ok_or_else(||invalid("caption checkpoint missing"))?;
    let checkpoint:crate::StageCheckpoint=local::read(&root.join(&checkpoint_ref.relative_path))?;checkpoint.validate()?;
    if checkpoint.checkpoint_sha256!=checkpoint_ref.checkpoint_sha256||checkpoint.payload.plan_sha256!=plan.plan_sha256||checkpoint.payload.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||![media,report].iter().all(|a|checkpoint.payload.validations.iter().any(|v|v.artifact_id==a.id&&v.artifact_sha256==a.sha256&&v.accepted&&v.contract==crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1))
        ||(work==CaptionWork::Deliver&&!checkpoint.payload.validations.iter().any(|v|v.artifact_id==media.id&&v.artifact_sha256==media.sha256&&v.accepted&&v.contract==crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1)){return Err(invalid("caption lacks exact native checkpoint acceptance"));}
    let delivery=manifest.payload.delivery.as_ref().ok_or_else(||invalid("caption delivery acceptance missing"))?;
    let acceptance:crate::validation::AcceptanceRecord=local::read(&root.join(&delivery.relative_path))?;acceptance.validate()?;
    if acceptance.sha256()?!=delivery.sha256||acceptance.plan_sha256!=plan.plan_sha256||acceptance.layer!=crate::validation::ValidationLayer::Delivery{return Err(invalid("caption delivery acceptance changed"));}
    if work==CaptionWork::Deliver&&(!plan.payload.stages[0].validations.iter().any(|v|v.artifact==work.artifact()&&v.contract==crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1)){return Err(invalid("caption delivery lacks native temporal validation"));}
    let hash=report.sha256.clone();Ok((plan,manifest,report_path,media_path,hash))
}
fn input_bindings(plan:&crate::PipelineV3Plan,operation:&CaptionOperation)->Result<()> {
    for (id,sha,size) in [("caption_source",operation.source.artifact.sha256.as_str(),operation.source.artifact.byte_count),("caption_srt",operation.lyrics.srt_sha256.as_str(),operation.lyrics.srt_byte_count),("caption_font",operation.request.configuration.font.sha256.as_str(),operation.font_byte_count)] {
        if !plan.payload.inputs.iter().any(|i|i.id==id&&i.content_sha256==sha&&i.byte_count==size){return Err(invalid("caption operation differs from actual accepted plan inputs"));}
    }Ok(())
}
pub fn preview_status(root:&Path,token:&CancellationToken)->Result<CaptionPreviewCompletion> {
    cancelled(token)?;let (plan,before,path,frames,report_sha256)=accepted(root,CaptionWork::Preview)?;let report:CaptionPreviewReport=local::read(&path)?;report.operation.validate()?;
    if report.schema!="aniflow.caption-preview-report/v1"||report.operation.work!=CaptionWork::Preview||report.operation.configuration()?.effective_configuration_sha256!=plan.payload.stages[0].provider_lock.payload.effective_configuration_sha256||report.provider_lock_sha256!=plan.payload.stages[0].provider_lock.lock_sha256
        ||report.context_sha256!=report.operation.context()?||report.subject_sha256!=canonical_sha256(&report.frames)?||report.review_scope!=PREVIEW_SCOPE||report.native_qualified||report.creative_approval!="not_performed"||report.frames.len()!=report.operation.preview_ordinals.len(){return Err(invalid("caption preview report differs from its exact native context/subject"));}
    input_bindings(&plan,&report.operation)?;
    let names:Vec<_>=report.frames.iter().map(|f|f.filename.as_str()).collect();local::entries(&frames,&names,&[],false)?;
    let declaration=crate::AuthoredPipelineInput {id:"caption_frames".into(),artifact_type:CaptionWork::Preview.media_type().into(),artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video)};
    let observed=crate::pipeline_v3::hash_pipeline_input(&declaration,&frames).map_err(|e|invalid(&e.message))?;
    let accepted=before.payload.outputs.iter().find(|a|a.id=="caption_frames").ok_or_else(||invalid("caption preview directory evidence missing"))?;
    if observed.content_sha256!=accepted.sha256||observed.byte_count!=accepted.byte_count||observed.file_count!=accepted.file_count{return Err(invalid("caption preview directory changed after acceptance"));}
    let video=&report.operation.source.artifact;let period=RationalTime::new(video.frame_rate.denominator() as i64,video.frame_rate.numerator() as u64).map_err(mapped)?;
    for (index,frame) in report.frames.iter().enumerate() {
        if frame.filename!=format!("preview-{index:04}.png")||frame.ordinal!=report.operation.preview_ordinals[index]||frame.time!=period.checked_mul(frame.ordinal).map_err(mapped)?
            ||crate::frame_io::inspect_png(&frames.join(&frame.filename),video.width,video.height,report.operation.request.configuration.settings.maximum_frame_bytes)?!=(frame.sha256.clone(),frame.byte_count){return Err(invalid("caption preview image or exact source frame changed"));}
    }
    require_current(&report.operation,token)?;
    if crate::status_v3(root)?!=before||identity(&path,local::DOCUMENT_BYTES)?.0!=report_sha256{return Err(invalid("caption preview changed during observation"));}
    Ok(CaptionPreviewCompletion {schema:"aniflow.caption-preview-completion/v1".into(),binding:PreviewBinding {run_directory:root.to_path_buf(),report_sha256,context_sha256:report.context_sha256.clone(),subject_sha256:report.subject_sha256.clone()},frames_directory:frames,report})
}
pub fn delivery_report(root:&Path,token:&CancellationToken)->Result<CaptionDeliveryReport> {
    cancelled(token)?;let (plan,before,path,media,sha)=accepted(root,CaptionWork::Deliver)?;let report:CaptionDeliveryReport=local::read(&path)?;report.operation.validate()?;
    input_bindings(&plan,&report.operation)?;
    let expected=&report.operation.source.artifact;let playback=&report.media.playback;
    let pixels=if report.operation.request.configuration.mode==CaptionMode::SoftMux {"encoded_video_packets_verified; no_video_reencode"}else{"explicit_drawtext_burn_in_and_H264_reencode; original_frame_identity_not_asserted"};
    if report.schema!="aniflow.caption-delivery-report/v1"||report.operation.work!=CaptionWork::Deliver||report.operation.configuration()?.effective_configuration_sha256!=plan.payload.stages[0].provider_lock.payload.effective_configuration_sha256||report.provider_lock_sha256!=plan.payload.stages[0].provider_lock.lock_sha256
        ||report.media.source_sha256!=expected.sha256||report.media.width!=expected.width||report.media.height!=expected.height||report.media.frame_count!=expected.frame_count||report.media.frame_rate!=expected.frame_rate||report.media.video_interval!=expected.video_interval
        ||playback.audio!=expected.audio||!playback.faststart||playback.video_codec!="h264"||playback.pixel_format!="yuv420p"||playback.color_primaries!="bt709"||playback.color_transfer!="bt709"||playback.color_matrix!="bt709"||playback.color_range!="tv"||playback.sample_aspect_ratio!="1:1"||playback.orientation_degrees!=0
        ||report.media.pixel_policy!=pixels||!report.media.full_decode_observed||report.media.authored_subtitle_track!=(report.operation.request.configuration.mode==CaptionMode::SoftMux)||report.media.video_packets_preserved!=(report.operation.request.configuration.mode==CaptionMode::SoftMux)
        ||report.review_scope!=DELIVERY_SCOPE||report.lyrics_video_audio_binding!="operator_declared; not established_by_PCM_to_compressed_audio_analysis"||report.native_qualified||report.creative_approval!="not_performed"||report.publication_authority!="none"
        ||identity(&media,report.operation.request.configuration.settings.maximum_output_bytes)?!=(report.media.output_sha256.clone(),report.media.output_byte_count){return Err(invalid("caption delivery report differs from current native byte/clock/audio/profile authority"));}
    require_current(&report.operation,token)?;
    if crate::status_v3(root)?!=before||identity(&path,local::DOCUMENT_BYTES)?.0!=sha{return Err(invalid("caption delivery changed during observation"));}Ok(report)
}
