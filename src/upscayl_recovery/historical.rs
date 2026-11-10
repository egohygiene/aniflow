//! Current selected-frame import. No historical Upscayl execution is inferred.
mod native;
pub mod preparation;
pub use native::execute_invocation;
pub use native::observe;
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use crate::{CancellationToken,Result,PipelineV3Plan,PipelineV3RunProgress,PipelineV3RunOutcome};
use crate::provider::{canonical_sha256,canonical_json_bytes,require_sha256};
use crate::frame_io::{identity,inspect_png,ensure_canonical_directory};
use crate::frame_import::{self,FrameImportConfiguration};
use crate::music_video_recipe::manual::{ManualReview,ReviewDecision};
use super::{invalid,check_cancelled};
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.historical-spatial-import";
pub const CAPABILITY_ID:&str="aniflow/import-historical-spatial-frames";
pub const SUBSET_SCHEMA:&str="aniflow.imported-spatial-selection/v1";
const OPERATION_SCHEMA:&str="aniflow.historical-spatial-import-operation/v1";
const MAX_FRAMES:usize=128;

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedFile {pub ordinal:u64,pub file:PathBuf,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportSettings {pub decoder:FrameImportConfiguration,pub output_width:u32,pub output_height:u32,pub minimum_free_bytes:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {pub schema:String,pub source_video:PathBuf,pub source_index:PathBuf,pub frames:Vec<SelectedFile>,pub settings:ImportSettings,pub provider_executable:PathBuf}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalFrame {pub ordinal:u64,pub filename:String,pub sha256:String,pub byte_count:u64,pub source_time:crate::temporal::TimeRange}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportIntent {
    pub settings:ImportSettings,pub source_sha256:String,pub source_bytes:u64,pub source_index_sha256:String,pub source_index_bytes:u64,
    pub source_width:u32,pub source_height:u32,pub original_frame_count:u64,pub frames:Vec<HistoricalFrame>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportReviewPlan {pub schema:String,pub context_sha256:String,pub subject_sha256:String,pub intent:ImportIntent,pub historical_processing:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportOperation {pub schema:String,pub intent:ImportIntent,pub declaration:ManualReview}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedSpatialSelection {
    pub schema:String,pub source_sha256:String,pub source_index_sha256:String,pub original_frame_count:u64,
    pub output_width:u32,pub output_height:u32,pub frames:Vec<HistoricalFrame>,pub import_decision_sha256:String,
    pub historical_processing:String,pub source_relationship:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportReport {pub schema:String,pub operation:ImportOperation,pub provider_lock_sha256:String,pub decoded_rgb24_sha256:Vec<String>,pub subset:ImportedSpatialSelection,pub native_qualified:bool}
impl ImportSettings {
    pub fn validate(&self,source_width:u32,source_height:u32)->Result<()> {
        self.decoder.validate()?;
        if !(2..=4).any(|s|source_width.checked_mul(s)==Some(self.output_width)&&source_height.checked_mul(s)==Some(self.output_height))
            ||u64::from(self.output_width)*u64::from(self.output_height)>self.decoder.maximum_pixels
            ||self.minimum_free_bytes>1_099_511_627_776{return Err(invalid("historical import requires explicit uniform 2x/3x/4x geometry within finite bounds"));}Ok(())
    }
}
impl ImportIntent {
    fn validate(&self)->Result<()> {
        self.settings.validate(self.source_width,self.source_height)?;
        for s in [&self.source_sha256,&self.source_index_sha256]{require_sha256(s,"historical source/index")?;}
        if self.source_bytes==0||self.source_bytes>self.settings.decoder.maximum_source_bytes||self.frames.is_empty()||self.frames.len()>MAX_FRAMES
            ||self.original_frame_count==0||self.source_index_bytes>crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES {
            return Err(invalid("historical selection exceeds source/index/frame bounds"));
        }
        let mut total=0u64;let mut previous=None;let mut names=std::collections::BTreeSet::new();
        for f in &self.frames {
            require_sha256(&f.sha256,"historical frame")?;
            crate::temporal::TimeRange::new(f.source_time.start,f.source_time.end).map_err(|_|invalid("invalid historical source time"))?;
            if f.ordinal>=self.original_frame_count||previous.is_some_and(|p|f.ordinal<=p)||f.filename.is_empty()||f.filename.len()>128
                ||!f.filename.ends_with(".png")||f.filename.starts_with('.')||!f.filename.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b))||!names.insert(&f.filename)
                ||f.byte_count==0||f.byte_count>self.settings.decoder.maximum_frame_bytes{return Err(invalid("historical frames need unique ordered original ordinals, canonical names and exact bounded identities"));}
            total=total.checked_add(f.byte_count).ok_or_else(||invalid("historical byte sum overflow"))?;previous=Some(f.ordinal);
        }
        if total>self.settings.decoder.maximum_total_frame_bytes{return Err(invalid("historical selection exceeds aggregate bytes"));}Ok(())
    }
}
fn review(intent:ImportIntent)->Result<ImportReviewPlan> {
    intent.validate()?;Ok(ImportReviewPlan {schema:"aniflow.historical-import-review-plan/v1".into(),context_sha256:canonical_sha256(&intent)?,
        subject_sha256:canonical_sha256(&intent.frames)?,intent,historical_processing:"unknown; current import does not attest prior tools/models/settings or creative quality".into()})
}
pub fn review_plan(request:&ImportRequest,token:&CancellationToken)->Result<ImportReviewPlan> {
    check_cancelled(token)?;
    if request.schema!="aniflow.historical-spatial-import-request/v1"||request.frames.is_empty()||request.frames.len()>MAX_FRAMES{return Err(invalid("select 1..128 explicit historical frames"));}
    let (index,index_pin)=frame_import::read_index(&request.source_index)?;
    request.settings.validate(index.width,index.height)?;
    let source=identity(&request.source_video,request.settings.decoder.maximum_source_bytes)?;
    if source!=(index.temporal_inspection.source_sha256.clone(),index.temporal_inspection.source_size_bytes){return Err(invalid("historical selection differs from the original source identity"));}
    let timeline=index.temporal_inspection.video_timeline().map_err(|_|invalid("historical source timeline unavailable"))?;
    let mut frames=Vec::new();
    for f in &request.frames {
        check_cancelled(token)?;
        let original=index.frames.get(f.ordinal as usize).filter(|v|v.ordinal==f.ordinal).ok_or_else(||invalid("historical ordinal is outside the original index"))?;
        let observed=inspect_png(&f.file,request.settings.output_width,request.settings.output_height,request.settings.decoder.maximum_frame_bytes)?;
        if observed!=(f.sha256.clone(),f.byte_count){return Err(invalid("selected historical PNG differs from its explicit identity"));}
        frames.push(HistoricalFrame {ordinal:f.ordinal,filename:original.filename.clone(),sha256:f.sha256.clone(),byte_count:f.byte_count,
            source_time:timeline.frames.get(f.ordinal as usize).ok_or_else(||invalid("original frame time is missing"))?.source_time});
    }
    review(ImportIntent {settings:request.settings.clone(),source_sha256:source.0,source_bytes:source.1,source_index_sha256:index_pin.0,source_index_bytes:index_pin.1,
        source_width:index.width,source_height:index.height,original_frame_count:index.frames.len() as u64,frames})
}
impl ImportOperation {
    fn validate(&self)->Result<()> {
        let p=review(self.intent.clone())?;self.declaration.validate()?;
        if self.schema!=OPERATION_SCHEMA||self.declaration.decision!=ReviewDecision::Approve||self.declaration.context_sha256!=p.context_sha256||self.declaration.subject_sha256!=p.subject_sha256 {
            return Err(invalid("historical import requires a supplied exact approving import decision"));}Ok(())
    }
    fn reference()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:OPERATION_SCHEMA.into(),version:"1.0.0".into(),sha256:format!("{:x}",Sha256::digest(include_bytes!("../../docs/contracts/historical-spatial-import-operation-v1.schema.json")))}}
    fn configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let value=serde_json::to_value(self).map_err(|_|invalid("cannot serialize historical import"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:CAPABILITY_ID.into(),version:"1.0.0".into()},Self::reference(),value.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let mut value=serde_json::to_value(self.intent.settings.decoder.manifest()?).map_err(|_|invalid("cannot project historical importer manifest"))?;
        value["provider"]["id"]=json!(PROVIDER_ID);value["provider"]["display_name"]=json!("aniflow current historical-frame validator");value["configuration_schemas"]=json!([Self::reference()]);
        let cap=&mut value["capabilities"][0];cap["id"]=json!(CAPABILITY_ID);cap["configuration_schema"]=json!(Self::reference());
        let port=|name:&str,kind:&str,role:&str,stream:&str,cardinality:&str|json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":cardinality,"immutable":true});
        cap["inputs"]=json!([port("source","video/*","temporal_source","video","one"),port("index","application/vnd.aniflow.rife-frame-index+json","validation_evidence","timed_metadata","one"),port("frames","image/png","intermediate","video","one_or_more")]);
        cap["outputs"]=json!([port("frames","application/vnd.aniflow.historical-spatial-frames","intermediate","video","one"),port("index","application/vnd.aniflow.imported-spatial-selection+json","validation_evidence","timed_metadata","one"),port("evidence","application/vnd.aniflow.historical-spatial-report+json","validation_evidence","timed_metadata","one")]);
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
}
struct Prepared {plan:PipelineV3Plan,registry:crate::ProviderRegistry,bindings:Vec<crate::PipelineInputBinding>,operation:ImportOperation}
fn prepare(request:&ImportRequest,declaration:&ManualReview,token:&CancellationToken)->Result<Prepared> {
    let p=review_plan(request,token)?;let operation=ImportOperation {schema:OPERATION_SCHEMA.into(),intent:p.intent,declaration:declaration.clone()};operation.validate()?;
    let settings=&request.settings.decoder;
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("historical-import",operation.manifest()?,operation.configuration()?,request.provider_executable.clone(),"aniflow-historical-spatial-import-v1",crate::ComponentInventory {tools:vec![crate::ComponentIdentity {id:"ffmpeg".into(),version:settings.ffmpeg.version.clone(),sha256:Some(settings.ffmpeg.sha256.clone())}],codecs:Vec::new(),models:Vec::new()})?)?;
    let mut bindings=vec![crate::PipelineInputBinding::new("source_video",&request.source_video),crate::PipelineInputBinding::new("source_index",&request.source_index)];
    let mut inputs=vec![json!({"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}),json!({"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"})];let mut ids=Vec::new();
    for f in &request.frames {let id=format!("historical_frame_{:06}",f.ordinal);bindings.push(crate::PipelineInputBinding::new(&id,&f.file));inputs.push(json!({"id":id,"artifact_type":"image/png","artifact_role":"intermediate","stream_role":"video"}));ids.push(id);}
    let mut outputs=Vec::new();let mut validations=Vec::new();let mut final_outputs=Vec::new();
    for (port,id,kind,path) in [("frames","historical_frames","directory","artifacts/historical/frames"),("index","historical_index","file","artifacts/historical/index.json"),("evidence","historical_evidence","file","artifacts/historical/report.json")] {
        let validation=format!("{id}_integrity");outputs.push(json!({"port":port,"artifacts":[{"id":id,"relative_path":path,"kind":kind}]}));validations.push(json!({"id":validation,"artifact":id,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}));final_outputs.push(json!({"id":id,"artifact":id,"required_validations":[validation]}));
    }
    let value=json!({"schema":"aniflow.pipeline/v3","name":"explicit-historical-spatial-import","inputs":inputs,"stages":[{"id":"historical_import","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"historical-import"},"fallbacks":[]},"inputs":[{"port":"source","artifacts":["source_video"]},{"port":"index","artifacts":["source_index"]},{"port":"frames","artifacts":ids}],"outputs":outputs,"validations":validations}],"outputs":final_outputs});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(e.message))?;
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(e.message))?;
    for (id,sha,bytes) in [("source_video",&operation.intent.source_sha256,operation.intent.source_bytes),("source_index",&operation.intent.source_index_sha256,operation.intent.source_index_bytes)] {
        if !plan.payload.inputs.iter().any(|i|i.id==id&&i.content_sha256==*sha&&i.byte_count==bytes){return Err(invalid("historical source/index changed during native planning"));}
    }
    for (f,expected) in request.frames.iter().zip(&operation.intent.frames) {if !plan.payload.inputs.iter().any(|i|i.id==format!("historical_frame_{:06}",f.ordinal)&&i.content_sha256==expected.sha256&&i.byte_count==expected.byte_count){return Err(invalid("historical frame changed during native planning"));}}
    if review_plan(request,token)?.intent!=operation.intent{return Err(invalid("historical inputs changed during planning"));}Ok(Prepared {plan,registry,bindings,operation})
}
pub fn plan(request:&ImportRequest,declaration:&ManualReview,token:&CancellationToken)->Result<PipelineV3Plan> {Ok(prepare(request,declaration,token)?.plan)}
fn limits(intent:&ImportIntent)->crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits {timeout:std::time::Duration::from_secs(21_600),termination_grace_period:std::time::Duration::from_millis(2_000),maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,
        maximum_artifact_files:intent.frames.len() as u64+2,maximum_artifact_bytes:intent.frames.iter().map(|f|f.byte_count).sum::<u64>()+8_388_608}
}
pub fn run<F:FnMut(&PipelineV3RunProgress)>(request:ImportRequest,declaration:ManualReview,parent:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    ensure_canonical_directory(parent)?;
    for input in [&request.source_video,&request.source_index,&request.settings.decoder.ffmpeg.executable,&request.provider_executable].into_iter().chain(request.frames.iter().map(|f|&f.file)) {if parent.starts_with(input)||input.starts_with(parent){return Err(invalid("historical import parent overlaps selected inputs or tools"));}}
    let p=prepare(&request,&declaration,token)?;let total=p.operation.intent.frames.iter().map(|f|f.byte_count).sum::<u64>();
    crate::video_assembly::require_storage(parent,total*3+8_388_608,request.settings.minimum_free_bytes)?;
    let bounds=limits(&p.operation.intent);
    crate::run_v3_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(p.plan,p.bindings,p.registry).with_output_directory(parent).with_execution_limits(bounds),token,progress)
}
pub fn resume<F:FnMut(&PipelineV3RunProgress)>(request:ImportRequest,declaration:ManualReview,root:&Path,token:&CancellationToken,progress:F)->Result<PipelineV3RunOutcome> {
    let p=prepare(&request,&declaration,token)?;let saved=crate::PipelineV3Workspace::open_read_only(root)?;let bytes=crate::destination::local::read_bounded::<serde_json::Value>(&saved.plan(),67_108_864)?;
    let plan=PipelineV3Plan::from_json_slice(&canonical_json_bytes(&bytes)?).map_err(|e|invalid(e.message))?;
    if plan!=p.plan{return Err(invalid("historical import source, selection, decision or tools changed"));}
    crate::video_assembly::require_storage(root,p.operation.intent.frames.iter().map(|f|f.byte_count).sum::<u64>()*3+8_388_608,request.settings.minimum_free_bytes)?;
    let bounds=limits(&p.operation.intent);
    crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,p.bindings,p.registry).with_execution_limits(bounds),token,progress)
}
#[cfg(test)]
mod tests;
