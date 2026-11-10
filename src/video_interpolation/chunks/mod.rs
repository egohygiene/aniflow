//! Explicit recovered-frame RIFE chunks; ordinary v3 owns every checkpoint.
use super::*;
use std::path::{Path,PathBuf};
use serde::{Serialize,Deserialize};
use crate::{CancellationToken,PipelineV3Plan};
use crate::frame_io::{identity,ensure_canonical_directory};
use crate::destination::local;
use crate::provider::canonical_json_bytes;
use crate::video_interpolation::recovered::{RecoveredRifeRequest,RecoveredInput};
mod runtime;mod native;
pub use runtime::{plan,run,resume,status,bind_assembly};
pub(crate) use native::execute_invocation;
pub(crate) use runtime::{preflight_plan,validate_accepted};
#[cfg(all(test,unix))] mod tests;
pub(crate) const PIPELINE:&str="chunked-recovered-rife-interpolation";
pub(crate) const PROVIDER:&str="org.egohygiene.aniflow.rife-chunks";
const CHUNK_CAP:&str="aniflow/recovered-rife-chunk";
const COLLECT_CAP:&str="aniflow/recovered-rife-chunk-collection";
const CHUNK_TYPE:&str="application/vnd.aniflow.rife-chunk+json";
const CHUNK_SCHEMA:&str="aniflow.rife-chunk-output/v1";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {pub schema:String,pub base:RecoveredRifeRequest,pub output_frames_per_chunk:u64}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{if bytes.len()>65_536{return Err(invalid("chunked RIFE request exceeds 64 KiB"));}let r:Self=decode_json(bytes,"chunked recovered RIFE request")?;r.validate()?;Ok(r)}
    pub fn validate(&self)->Result<()> {self.base.validate()?;if self.schema!="aniflow.chunked-recovered-rife-request/v1"||!(2..=256).contains(&self.output_frames_per_chunk)||self.output_frames_per_chunk%2!=0{return Err(invalid("explicit even 2..256 output-frame chunk bound required"));}Ok(())}
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Role {Chunk {first_output:u64,end_output:u64},Collect}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {pub schema:String,pub request:Request,pub role:Role}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkReport {pub schema:String,pub operation:Operation,pub provider_lock_sha256:String,pub output:RifeOutputIndex}
fn ranges(count:u64,limit:u64)->Result<Vec<(u64,u64)>> {if count==0||!(2..=256).contains(&limit)||limit%2!=0||count.div_ceil(limit)>128{return Err(invalid("chunked RIFE requires a nonempty complete set of at most 128 bounded chunks"));}Ok((0..count).step_by(limit as usize).map(|start|(start,(start+limit).min(count))).collect())}
fn id(n:usize)->String{format!("rife_chunk_{n:04}")}
impl Operation {
    fn capability(&self)->&'static str{if self.role==Role::Collect{COLLECT_CAP}else{CHUNK_CAP}}
    fn validate(&self)->Result<()> {self.request.validate()?;if self.schema!="aniflow.rife-chunk-operation/v1"{return Err(invalid("chunk operation version differs"));}if let Role::Chunk {first_output,end_output}=self.role {if first_output>=end_output||first_output%self.request.output_frames_per_chunk!=0||end_output-first_output>self.request.output_frames_per_chunk{return Err(invalid("chunk range is not one exact selected interval"));}}Ok(())}
    fn schema_ref()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:"aniflow.rife-chunk-operation/v1".into(),version:"1.0.0".into(),sha256:local::hash(include_bytes!("../../../docs/contracts/rife-chunk-operation-v1.schema.json"))}}
    fn configuration(&self)->Result<crate::ProviderConfiguration> {self.validate()?;let v=serde_json::to_value(self).map_err(|_|invalid("chunk operation serialization"))?;crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER.into(),version:"1.0.0".into()},crate::CapabilityReference {id:self.capability().into(),version:"1.0.0".into()},Self::schema_ref(),v.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())}
    fn manifest(&self)->Result<crate::ProviderManifest> {
        let wrapper=recovered::Wrapper {schema:"aniflow.recovered-rife-configuration/v1".into(),configuration:self.request.base.configuration.clone(),recovered:self.request.base.source.clone()};
        let mut v=serde_json::to_value(wrapper.manifest()?).map_err(|_|invalid("chunk manifest serialization"))?;v["provider"]["id"]=serde_json::json!(PROVIDER);v["configuration_schemas"]=serde_json::json!([Self::schema_ref()]);let cap=&mut v["capabilities"][0];cap["id"]=serde_json::json!(self.capability());cap["configuration_schema"]=serde_json::json!(Self::schema_ref());
        if self.role==Role::Collect {
            cap["kind"]=serde_json::json!("assembler_encoder");cap["inputs"][0]["cardinality"]=serde_json::json!("one_or_more");cap["inputs"].as_array_mut().unwrap().push(serde_json::json!({"name":"chunks","artifact_type":CHUNK_TYPE,"artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one_or_more","immutable":true}));
            cap["requirements"]["tools"]=serde_json::json!([]);cap["requirements"]["models"]=serde_json::json!([]);cap["requirements"]["compute"]["gpu"]=serde_json::json!("forbidden");cap["behavior"]["side_effects"]=serde_json::json!(["filesystem_read","filesystem_write"]);cap["behavior"]["fidelity"]=serde_json::json!("lossless");
        }else{cap["outputs"][1]["artifact_type"]=serde_json::json!(CHUNK_TYPE);}
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&v)?)
    }
}
fn chunk_output(op:&Operation,input:&RecoveredInput,index:&RifeOutputIndex)->Result<()> {
    let Role::Chunk {first_output,end_output}=op.role else{return Err(invalid("collector cannot stand in for a chunk"));};let expected=recovered::plan(&op.request.base.configuration,&input.carrier)?;
    if !ranges(expected.output_frame_count,op.request.output_frames_per_chunk)?.contains(&(first_output,end_output))||index.schema!=CHUNK_SCHEMA||index.plan!=expected||index.frames.len() as u64!=end_output-first_output{return Err(invalid("RIFE chunk lost exact full-plan/range identity"));}
    let original=input.carrier.frames();
    for (n,f) in index.frames.iter().enumerate() {let ordinal=first_output+n as u64;let (left,right,origin)=super::mapped_output_frame(&expected,ordinal)?;
        if f.ordinal!=ordinal||f.filename!=format!("frame_{ordinal:08}.png")||f.source_left_ordinal!=left||f.source_right_ordinal!=right||f.origin!=origin{return Err(invalid("RIFE chunk changed a cut/endpoint/original/midpoint mapping"));}require_sha256(&f.sha256,"RIFE chunk frame")?;
        if origin!=RifeFrameOrigin::InterpolatedMidpoint&&f.sha256!=original[left as usize].sha256{return Err(invalid("RIFE chunk changed an original/held source frame"));}
    }Ok(())
}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {pub schema:String,pub request:Request,pub native_plan:PipelineV3Plan,pub intent_sha256:String}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {pub schema:String,pub run_directory:PathBuf,pub native_run_directory:PathBuf,pub intent_sha256:String,pub completed_chunks:u64,pub pending_chunks:u64,pub failed_chunks:u64,pub complete:bool,pub native:Option<crate::PipelineV3RunManifest>}
