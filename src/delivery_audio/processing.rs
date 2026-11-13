//! Versioned delivery over existing native owners; no second execution engine.
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use serde_json::Value;
use crate::{CancellationToken,Result,PipelineV3RunProgress};
use crate::destination::{cancelled,invalid,local,MediaFile};
use crate::frame_io::{identity,ensure_canonical_directory};
use crate::provider::canonical_sha256;
use super::{gain,gain_video,final_qc,final_lossless_qc};

pub const PLAN_SCHEMA:&str="aniflow.audio-processing-plan/v1";
pub const HANDOFF_SCHEMA:&str="aniflow.audio-processing-handoff/v1";

#[derive(Debug,Clone,PartialEq)]
enum Operation {
    FixedGain(gain::Request),
    LosslessQc(final_lossless_qc::resumable::ResumableQcRequest),
    VideoQc(final_qc::resumable::Request),
    GainVideo(gain_video::Request),
}
#[derive(Debug,Clone,PartialEq)]
pub struct Request {operation:Operation,supplied:Value}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("audio-processing request exceeds 64 KiB"));}
        let value:Value=crate::provider::decode_json(bytes,"audio-processing request")?;
        let operation=match value["schema"].as_str() {
            Some("aniflow.audio-gain-request/v1")=>Operation::FixedGain(gain::Request::from_json_slice(bytes)?),
            Some("aniflow.resumable-final-audio-qc-request/v1")=>Operation::LosslessQc(final_lossless_qc::resumable::ResumableQcRequest::from_json_slice(bytes)?),
            Some("aniflow.resumable-final-video-audio-qc-request/v1")=>Operation::VideoQc(final_qc::resumable::Request::from_json_slice(bytes)?),
            Some("aniflow.gain-video-request/v1")=>Operation::GainVideo(gain_video::Request::from_json_slice(bytes)?),
            _=>return Err(invalid("unsupported explicit audio-processing request")),
        };Ok(Self {operation,supplied:value})
    }
    pub fn profile(&self)->&'static str {match &self.operation {
        Operation::FixedGain(_)=>"fixed-audio-gain-v1",Operation::LosslessQc(_)=>"final-lossless-audio-qc-v1",
        Operation::VideoQc(_)=>"final-video-audio-qc-v1",Operation::GainVideo(_)=>"gain-video-v1",
    }}
    fn value(&self)->Result<Value> {Ok(self.supplied.clone())}
    fn native_value(&self)->Result<Value> {
        let result=match &self.operation {Operation::FixedGain(r)=>serde_json::to_value(r),Operation::LosslessQc(r)=>serde_json::to_value(r),Operation::VideoQc(r)=>serde_json::to_value(r),Operation::GainVideo(r)=>serde_json::to_value(r)};
        result.map_err(|_|invalid("audio-processing request encoding"))
    }
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPayload {pub profile:String,pub request:Value,pub source_files:Vec<MediaFile>,pub native:Value}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {pub schema:String,pub payload:PlanPayload,pub plan_sha256:String}

/// Read-only native planning. Gain refusal never becomes an unchanged success.
pub fn plan(request:&Request,token:&CancellationToken)->Result<Plan> {
    cancelled(token)?;
    let encode=|v|serde_json::to_value(v).map_err(|_|invalid("native processing plan encoding"));
    let (source_files,native)=match &request.operation {
        Operation::FixedGain(r)=>{
            let native=gain::plan(r,token)?;
            if native.native_plan.is_none(){return Err(invalid("fixed gain is not ready; inspect the native decision, preserve the source and select another explicit action"));}
            (vec![native.source.assessment.subject.file.clone()],encode(native)?)
        },
        Operation::LosslessQc(r)=>{let native=final_lossless_qc::resumable::plan(r,token)?;(vec![native.native.source.file.clone()],serde_json::to_value(native).map_err(|_|invalid("lossless-QC plan encoding"))?)},
        Operation::VideoQc(r)=>{
            let native=final_qc::resumable::plan(r,token)?;let a=&native.native.artifact;
            (vec![MediaFile {locator:a.locator.clone(),sha256:a.sha256.clone(),byte_count:a.byte_count}],serde_json::to_value(native).map_err(|_|invalid("video-QC plan encoding"))?)
        },
        Operation::GainVideo(r)=>{
            let files=gain_video::selected_files(r,token)?;let native=gain_video::plan(r,token)?;
            for (id,file) in ["gain_video_source","gain_video_audio"].iter().zip(&files) {
                if native.payload.inputs.iter().filter(|i|i.id==*id&&i.content_sha256==file.sha256&&i.byte_count==file.byte_count).count()!=1{return Err(invalid("gain-video handoff source files differ from the resolved native input identities"));}
            }
            if gain_video::selected_files(r,token)?!=files{return Err(invalid("gain-video sources changed during handoff planning"));}
            (files,serde_json::to_value(native).map_err(|_|invalid("gain-video plan encoding"))?)
        },
    };
    let payload=PlanPayload {profile:request.profile().into(),request:request.value()?,source_files,native};
    let result=Plan {schema:PLAN_SCHEMA.into(),plan_sha256:canonical_sha256(&payload)?,payload};
    local::bytes(&result)?;cancelled(token)?;Ok(result)
}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {pub id:String,pub purpose:String,pub file:MediaFile}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffPayload {
    pub plan:Plan,pub run_directory:PathBuf,pub native_intent_sha256:String,
    pub artifacts:Vec<Artifact>,pub report:Value,pub output_state:String,
    pub runtime_qualified:bool,pub creative_approval:String,pub publication_authority:String,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {pub schema:String,pub payload:HandoffPayload,pub handoff_sha256:String}

fn qc_artifact(root:&Path)->Result<Artifact> {
    let file=root.join("qc.json");let pin=identity(&file,local::DOCUMENT_BYTES)?;
    Ok(Artifact {id:"final_audio_qc".into(),purpose:"supporting_document".into(),file:MediaFile {locator:file,sha256:pin.0,byte_count:pin.1}})
}
/// Current domain readback, never acceptance of a previously supplied handoff.
pub fn handoff(request:&Request,root:&Path,token:&CancellationToken)->Result<Handoff> {
    ensure_canonical_directory(root)?;let admitted=plan(request,token)?;
    let (native_intent_sha256,artifacts,report,output_state)=match &request.operation {
        Operation::FixedGain(r)=>{
            let completion=gain::observe(root,token)?;let status=crate::status_v3(root)?;
            if completion.report.operation.request!=*r||admitted.payload.native["native_plan"]["plan_sha256"]!=status.payload.plan_sha256{return Err(invalid("gain handoff differs from the selected current native request/plan"));}
            (status.payload.plan_sha256,vec![Artifact {id:"gain_audio".into(),purpose:"audio_derivative".into(),file:completion.file.clone()}],serde_json::to_value(completion).map_err(|_|invalid("gain completion encoding"))?,"requires_fresh_exact_file_qc")
        },
        Operation::LosslessQc(_)=>{
            let status=final_lossless_qc::resumable::status(root,token)?;
            if !status.complete||admitted.payload.native["intent_sha256"]!=status.intent_sha256{return Err(invalid("lossless-QC handoff requires the exact completed owner intent"));}
            let report=status.report.ok_or_else(||invalid("completed lossless-QC report missing"))?;
            (status.intent_sha256,vec![qc_artifact(root)?],serde_json::to_value(report).map_err(|_|invalid("lossless-QC report encoding"))?,"measurements_only_selected_policy_check_required")
        },
        Operation::VideoQc(_)=>{
            let status=final_qc::resumable::status(root,token)?;
            if !status.complete||admitted.payload.native["intent_sha256"]!=status.intent_sha256{return Err(invalid("video-QC handoff requires the exact completed owner intent"));}
            let report=status.report.ok_or_else(||invalid("completed video-QC report missing"))?;
            (status.intent_sha256,vec![qc_artifact(root)?],serde_json::to_value(report).map_err(|_|invalid("video-QC report encoding"))?,"measurements_only_selected_policy_check_required")
        },
        Operation::GainVideo(_)=>{
            let completion=gain_video::completion(root,token)?;
            if admitted.payload.native["plan_sha256"]!=completion.plan_sha256||completion.artifacts.len()!=1{return Err(invalid("gain-video handoff differs from the selected current native plan"));}
            let a=&completion.artifacts[0];
            let artifacts=vec![Artifact {id:a.artifact_id.clone(),purpose:"video_delivery".into(),file:MediaFile {locator:a.locator.clone(),sha256:a.sha256.clone(),byte_count:a.byte_count}}];
            (completion.plan_sha256.clone(),artifacts,serde_json::to_value(completion).map_err(|_|invalid("gain-video completion encoding"))?,"requires_fresh_encoded_file_qc")
        },
    };
    if plan(request,token)?!=admitted{return Err(invalid("audio-processing source/settings changed during handoff"));}
    let payload=HandoffPayload {plan:admitted,run_directory:root.to_path_buf(),native_intent_sha256,artifacts,report,output_state:output_state.into(),runtime_qualified:false,creative_approval:"not_performed".into(),publication_authority:"none".into()};
    let result=Handoff {schema:HANDOFF_SCHEMA.into(),handoff_sha256:canonical_sha256(&payload)?,payload};local::bytes(&result)?;cancelled(token)?;Ok(result)
}

/// Delegate to each existing owner's fresh-run entrypoint. QC roots are fixed
/// fresh children of the explicit parent; media owners retain their run naming.
pub fn run<F:FnMut(&PipelineV3RunProgress)>(request:&Request,parent:&Path,token:&CancellationToken,mut progress:F)->Result<Handoff> {
    ensure_canonical_directory(parent)?;plan(request,token)?;
    let root=match &request.operation {
        Operation::FixedGain(r)=>gain::run(r,parent,token,&mut progress)?.run_directory,
        Operation::GainVideo(r)=>gain_video::run(r,parent,token,&mut progress)?.run_directory,
        Operation::LosslessQc(r)=>{let root=parent.join("final-lossless-qc");final_lossless_qc::resumable::run(r,&root,token,&mut progress)?;root},
        Operation::VideoQc(r)=>{let root=parent.join("final-video-audio-qc");final_qc::resumable::run(r,&root,token,&mut progress)?;root},
    };
    handoff(request,&root,token)
}
/// Explicit recovery only. QC owners store their own complete request; compare
/// it before delegation because their resume APIs intentionally take only root.
pub fn resume<F:FnMut(&PipelineV3RunProgress)>(request:&Request,root:&Path,token:&CancellationToken,mut progress:F)->Result<Handoff> {
    plan(request,token)?;
    match &request.operation {
        Operation::FixedGain(r)=>{gain::resume(r,root,token,&mut progress)?;},
        Operation::GainVideo(r)=>{gain_video::resume(r,root,token,&mut progress)?;},
        Operation::LosslessQc(_)|Operation::VideoQc(_)=>{
            let intent:Value=local::read(&root.join("resume-intent.json"))?;
            if intent["request"]!=request.native_value()?{return Err(invalid("QC recovery request differs from its retained owner intent"));}
            match &request.operation {
                Operation::LosslessQc(_)=>{final_lossless_qc::resumable::resume(root,token,&mut progress)?;},
                Operation::VideoQc(_)=>{final_qc::resumable::resume(root,token,&mut progress)?;},
                _=>unreachable!(),
            }
        },
    }
    handoff(request,root,token)
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn unsupported_and_implicit_processing_requests_are_refused() {
        for schema in ["aniflow.audio-policy-check-request/v1","aniflow.final-lossless-audio-qc-request/v1","automatic_normalize"] {
            assert!(Request::from_json_slice(&serde_json::to_vec(&serde_json::json!({"schema":schema})).unwrap()).is_err());
        }
        assert!(Request::from_json_slice(&vec![b' ';65_537]).is_err());
    }
}
