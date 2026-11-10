//! Bounded sparse per-frame work over the existing native provider/runtime.
//! V2 preserves original ordinals up to 100000 while limiting each run to 128
//! explicitly selected frames. V1 operation/report/schema bytes stay unchanged.
use super::*;
use serde::{Deserialize,Serialize};
pub const MAX_SELECTED:usize=128;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedRecoveryRequest {
    pub schema:String,pub source_video:PathBuf,pub frames_directory:PathBuf,pub source_index:PathBuf,
    pub ordinals:Vec<u64>,pub configuration:UpscaylRecoveryConfiguration,pub provider_executable:PathBuf,
}
impl SelectedRecoveryRequest {
    pub fn validate(&self)->Result<()> {
        self.configuration.validate()?;carrier::ordinals(&self.ordinals,MAX_SELECTED)?;
        if self.schema!="aniflow.selected-spatial-recovery-request/v1"||!self.configuration.execution.is_per_frame()||self.ordinals.iter().any(|v|*v>=100_000) {
            return Err(invalid("selected recovery requires explicit sparse per-frame work within the original 100000-frame clock"));
        }Ok(())
    }
}
pub(crate) fn schema_reference()->crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference {id:UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2.into(),version:"2.0.0".into(),
        sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/upscayl-recovery/frame-operation-v2.schema.json")))}
}
pub(crate) fn manifest(settings:&UpscaylRecoveryConfiguration)->Result<crate::ProviderManifest> {
    let mut value=serde_json::to_value(super::manifest(settings,false)?).map_err(|_|invalid("cannot project selected frame provider"))?;
    value["configuration_schemas"]=json!([schema_reference()]);value["capabilities"][0]["configuration_schema"]=json!(schema_reference());
    crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
}
struct Prepared {plan:crate::PipelineV3Plan,bindings:Vec<crate::PipelineInputBinding>,registry:crate::ProviderRegistry,working:u64}
fn prepare(request:&SelectedRecoveryRequest,token:&CancellationToken)->Result<Prepared> {
    request.validate()?;check_cancelled(token)?;
    let settings=&request.configuration;let (index,index_pin)=frame_import::read_index(&request.source_index)?;
    if index.frames.len()>100_000{return Err(invalid("selected recovery original index exceeds 100000 frames"));}
    crate::frame_io::ensure_canonical_directory(&request.frames_directory)?;
    let source=identity(&request.source_video,settings.import.maximum_source_bytes)?;
    if source!=(index.temporal_inspection.source_sha256.clone(),index.temporal_inspection.source_size_bytes){return Err(invalid("selected recovery source differs from original index"));}
    let component_bytes=verify_components(settings,token)?;let index_digest=canonical_sha256(&index)?;
    let timeline=index.temporal_inspection.video_timeline().map_err(|e|Error::from_anyhow(ErrorCategory::Media,e))?;
    let mut inputs=vec![json!({"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}),json!({"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"})];
    let mut bindings=vec![crate::PipelineInputBinding::new("source_video",&request.source_video),crate::PipelineInputBinding::new("source_index",&request.source_index)];
    let mut stages=Vec::new();let mut outputs=Vec::new();let mut registry=crate::ProviderRegistry::new();let mut input_bytes=0u64;
    for ordinal in &request.ordinals {
        let frame=index.frames.get(*ordinal as usize).ok_or_else(||invalid("selected missing ordinal is outside original index"))?;
        let path=request.frames_directory.join(&frame.filename);
        let observed=crate::frame_io::inspect_png(&path,index.width,index.height,settings.import.maximum_frame_bytes)?;
        if observed.0!=frame.sha256{return Err(invalid("selected original PNG differs from exact source index"));}
        input_bytes=input_bytes.checked_add(observed.1).ok_or_else(||invalid("selected input byte overflow"))?;
        let operation=UpscaylFrameOperation {schema:UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2.into(),settings:settings.clone(),frame:UpscaylFrameSpec {
            source_sha256:source.0.clone(),source_byte_count:source.1,frame_index_sha256:index_digest.clone(),ordinal:*ordinal,filename:frame.filename.clone(),source_time:timeline.frames[*ordinal as usize].source_time,
            input_sha256:observed.0,input_byte_count:observed.1,input_width:index.width,input_height:index.height}};
        let stage_id=format!("upscale_frame_{ordinal:06}");let input_id=format!("source_frame_{ordinal:06}");let frame_id=format!("frame_{ordinal:06}");let evidence_id=format!("evidence_{ordinal:06}");
        registry.register(crate::ProviderRegistration::new(&stage_id,operation.manifest()?,operation.provider_configuration()?,request.provider_executable.clone(),"aniflow-selected-upscayl-frame-v2",inventory(settings))?)?;
        inputs.push(json!({"id":input_id,"artifact_type":"image/png","artifact_role":"intermediate","stream_role":"video"}));bindings.push(crate::PipelineInputBinding::new(&input_id,path));
        let validations=[format!("{frame_id}_integrity"),format!("{evidence_id}_integrity")];
        stages.push(json!({"id":stage_id,"depends_on":[],"capability":{"id":UPSCAYL_FRAME_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":stage_id},"fallbacks":[]},"inputs":[{"port":"frame","artifacts":[input_id]}],
            "outputs":[{"port":"frame","artifacts":[{"id":frame_id,"relative_path":format!("artifacts/upscayl/{ordinal:06}/frame.png"),"kind":"file"}]},{"port":"evidence","artifacts":[{"id":evidence_id,"relative_path":format!("artifacts/upscayl/{ordinal:06}/frame.json"),"kind":"file"}]}],
            "validations":[{"id":validations[0],"artifact":frame_id,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":validations[1],"artifact":evidence_id,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}));
        outputs.push(json!({"id":frame_id,"artifact":frame_id,"required_validations":[validations[0]]}));outputs.push(json!({"id":evidence_id,"artifact":evidence_id,"required_validations":[validations[1]]}));
    }
    let count=request.ordinals.len() as u64;
    let metadata=(count+1).checked_pow(2).and_then(|v|v.checked_mul(16_384)).ok_or_else(||invalid("selected metadata budget overflow"))?;
    let working=count.checked_mul(settings.maximum_output_frame_bytes).and_then(|v|v.checked_mul(3)).and_then(|v|v.checked_add(metadata))
        .and_then(|v|v.checked_add(input_bytes)).and_then(|v|v.checked_add(component_bytes.checked_mul(2)?)).and_then(|v|v.checked_add(4*UPSCAYL_MAXIMUM_REPORT_BYTES))
        .ok_or_else(||invalid("selected recovery working budget overflow"))?;
    if working>settings.maximum_working_bytes||input_bytes>settings.import.maximum_total_frame_bytes{return Err(invalid("selected recovery exceeds finite working/input bounds"));}
    let authored=json!({"schema":"aniflow.pipeline/v3","name":"selected-upscayl-frame-recovery","inputs":inputs,"stages":stages,"outputs":outputs});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&authored)?).map_err(|e|invalid(e.message))?;
    let mut context=crate::audio_inspection::context();context.allowed_side_effects.push(crate::SideEffect::Gpu);
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&context).map_err(|e|invalid(e.message))?;
    if !plan.payload.inputs.iter().any(|i|i.id=="source_index"&&i.content_sha256==index_pin.0&&i.byte_count==index_pin.1)
        ||!plan.payload.inputs.iter().any(|i|i.id=="source_video"&&i.content_sha256==source.0&&i.byte_count==source.1) {
        return Err(invalid("selected recovery original source/index changed during resolution"));
    }
    if canonical_json_bytes(&plan)?.len() as u64>MAXIMUM_PLAN_BYTES{return Err(invalid("selected recovery plan exceeds 64 MiB"));}
    Ok(Prepared {plan,bindings,registry,working})
}
pub fn plan(request:&SelectedRecoveryRequest,token:&CancellationToken)->Result<crate::PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
fn bounds(request:&SelectedRecoveryRequest)->crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits {timeout:std::time::Duration::from_secs(86_400),termination_grace_period:std::time::Duration::from_secs(2),maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,
        maximum_artifact_files:2,maximum_artifact_bytes:request.configuration.maximum_working_bytes}
}
pub(crate) fn run_at<F:FnMut(&crate::PipelineV3RunProgress)>(request:SelectedRecoveryRequest,root:&Path,expected:&crate::PipelineV3Plan,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let parent=crate::run_v3::exact_run_parent(root)?;let p=prepare(&request,token)?;
    if p.plan!=*expected{return Err(invalid("selected recovery changed before child initialization"));}
    for input in [&request.source_video,&request.source_index,&request.frames_directory,&request.provider_executable,&request.configuration.upscayl.executable,&request.configuration.import.ffmpeg.executable,&request.configuration.model.param.path,&request.configuration.model.weights.path] {
        if root.starts_with(input)||input.starts_with(root){return Err(invalid("selected recovery child overlaps immutable inputs/tools/models"));}
    }
    require_storage(&parent,p.working,request.configuration.minimum_free_bytes)?;
    crate::run_v3::run_v3_at_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(p.plan,p.bindings,p.registry).with_output_directory(parent).with_execution_limits(bounds(&request)),root,token,progress)
}
pub fn resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:SelectedRecoveryRequest,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let p=prepare(&request,token)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let saved:serde_json::Value=crate::destination::local::read_bounded(&workspace.plan(),MAXIMUM_PLAN_BYTES)?;
    if crate::PipelineV3Plan::from_json_slice(&canonical_json_bytes(&saved)?).map_err(|e|invalid(e.message))?!=p.plan{return Err(invalid("selected recovery retained plan differs from exact current source/settings"));}
    require_storage(root,p.working,request.configuration.minimum_free_bytes)?;
    crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,p.bindings,p.registry).with_execution_limits(bounds(&request)),token,progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn operation()->UpscaylFrameOperation {
        let settings:UpscaylRecoveryConfiguration=serde_json::from_value(json!({"schema":UPSCAYL_RECOVERY_CONFIGURATION_SCHEMA_V1,
            "import":{"schema":crate::frame_import::FRAME_IMPORT_CONFIGURATION_SCHEMA_V1,"ffmpeg":{"executable":"/synthetic/ffmpeg","version":"1.0.0","sha256":"a".repeat(64)}},
            "upscayl":{"executable":"/synthetic/upscayl","version":"1.0.0","sha256":"b".repeat(64)},
            "model":{"name":"digital-art-4x","native_scale":4,"param":{"path":"/synthetic/model.param","sha256":"c".repeat(64)},"weights":{"path":"/synthetic/model.bin","sha256":"d".repeat(64)}},"output_scale":2})).unwrap();
        UpscaylFrameOperation {schema:UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2.into(),settings,frame:UpscaylFrameSpec {source_sha256:"e".repeat(64),source_byte_count:1000,frame_index_sha256:"f".repeat(64),ordinal:5601,filename:"frame_00005601.png".into(),
            source_time:crate::temporal::TimeRange::new(crate::temporal::RationalTime::new(5601,30).unwrap(),crate::temporal::RationalTime::new(5602,30).unwrap()).unwrap(),input_sha256:"1".repeat(64),input_byte_count:100,input_width:2,input_height:2}}
    }
    #[test]
    fn sparse_v2_keeps_large_original_ordinals_and_v1_retains_its_old_bound() {
        let op=operation();op.validate().unwrap();assert_eq!(op.frame.ordinal,5601);
        assert_eq!(op.provider_configuration().unwrap().configuration_schema.id,UPSCAYL_SELECTED_FRAME_OPERATION_SCHEMA_V2);
        let mut old=op.clone();old.schema=UPSCAYL_FRAME_OPERATION_SCHEMA_V1.into();assert!(old.validate().is_err());
        old.frame.ordinal=1;old.validate().unwrap();assert_eq!(old.provider_configuration().unwrap().configuration_schema,frame_schema_reference());
    }
    #[test]
    fn v2_does_not_authorize_unbounded_ordinals_or_chunk_execution() {
        let mut op=operation();op.frame.ordinal=100000;assert!(op.validate().is_err());
        let mut op=operation();op.settings.execution=UpscaylExecutionPolicy::BoundedChunk {frames_per_chunk:8,timeout_milliseconds:1000,maximum_tool_output_bytes:1024};assert!(op.validate().is_err());
    }
}
