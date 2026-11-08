//! Bind assembly to currently accepted RIFE output without inventing clocks.
use std::fs::OpenOptions;
use std::io::{Read,Write};
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use super::*;
use crate::video_interpolation::{RifeConfiguration,RifeOutputIndex,RifeProviderConfiguration,RIFE_OUTPUT_INDEX_SCHEMA_V1,
    RIFE_CAPABILITY_ID,RIFE_PROVIDER_ID,RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1};
use crate::temporal::DerivedVideoClock;

pub const RIFE_ASSEMBLY_INDEX_SCHEMA_V1:&str="aniflow.rife-assembly-index/v1";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeAssemblyIndex {
    pub schema:String,pub accepted_run:PathBuf,pub accepted_manifest_sha256:String,
    pub source_index_sha256:String,pub configuration:RifeConfiguration,
    pub source_index:RifeFrameIndex,pub output_index:RifeOutputIndex,
}
fn read(path:&Path,limit:u64)->Result<(Vec<u8>,(String,u64))> {
    let expected=identity(path,limit)?;let mut bytes=Vec::new();
    open_regular(path).map_err(|_|invalid("RIFE assembly evidence unavailable"))?.take(limit+1).read_to_end(&mut bytes).map_err(|_|invalid("cannot read bounded RIFE assembly evidence"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64)!=expected{return Err(invalid("RIFE assembly evidence changed"));}Ok((bytes,expected))
}
fn selected<'a>(manifest:&'a crate::PipelineV3RunManifest,artifact_type:&str)->Result<&'a crate::ArtifactEvidence> {
    let mut matches=manifest.payload.outputs.iter().filter(|a|a.artifact_type==artifact_type);
    let result=matches.next().ok_or_else(||invalid("accepted RIFE output type is missing"))?;
    if matches.next().is_some(){return Err(invalid("accepted RIFE output type is ambiguous"));}Ok(result)
}
fn output_path(root:&Path,record:&crate::ArtifactEvidence)->Result<PathBuf> {
    record.validate()?;let path=root.join(record.relative_path.as_deref().ok_or_else(||invalid("accepted RIFE output needs an owned locator"))?);
    if path.canonicalize().ok().as_deref()!=Some(path.as_path())||!path.starts_with(root){return Err(invalid("accepted RIFE output escaped its workspace"));}Ok(path)
}
impl RifeAssemblyIndex {
    pub fn clock(&self)->Result<DerivedVideoClock> {
        self.output_index.validate_against(&self.configuration,&self.source_index)?;
        let plan=&self.output_index.plan;
        let clock=DerivedVideoClock {schema:"aniflow.rife-derived-clock/v1".into(),source_sha256:plan.source_sha256.clone(),
            rife_plan_sha256:crate::provider::canonical_sha256(plan)?,source_frames:plan.source_frame_count,
            source_period:plan.source_frame_rate.reciprocal().map_err(|e|Error::from_anyhow(ErrorCategory::Media,e))?,
            output_frames:plan.output_frame_count,output_period:plan.output_frame_rate.reciprocal().map_err(|e|Error::from_anyhow(ErrorCategory::Media,e))?,duration:plan.duration};
        clock.validate_source(&self.source_index.temporal_inspection)?;Ok(clock)
    }
    /// Full current acceptance and exact output/config/source-index binding.
    /// This launches no RIFE/model/tool process and grants no quality approval.
    pub fn validate_binding(&self)->Result<PathBuf> {
        if self.schema!=RIFE_ASSEMBLY_INDEX_SCHEMA_V1{return Err(invalid("unsupported RIFE assembly index"));}
        ensure_canonical_directory(&self.accepted_run)?;
        require_sha256(&self.accepted_manifest_sha256,"RIFE acceptance")?;require_sha256(&self.source_index_sha256,"RIFE source index")?;
        self.clock()?;
        let before=crate::status_v3(&self.accepted_run)?;
        if before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none()||before.manifest_sha256!=self.accepted_manifest_sha256 {
            return Err(invalid("RIFE assembly requires the exact currently complete accepted run"));
        }
        let workspace=crate::PipelineV3Workspace::open_read_only(&self.accepted_run)?;
        let (plan_bytes,_)=read(&workspace.plan(),MAXIMUM_PLAN_BYTES)?;
        let plan=crate::PipelineV3Plan::from_json_slice(&plan_bytes).map_err(|e|invalid(e.message))?;
        if plan.plan_sha256!=before.payload.plan_sha256{return Err(invalid("accepted RIFE plan changed"));}
        let mut stages=plan.payload.stages.iter().filter(|s|s.capability.id==RIFE_CAPABILITY_ID);
        let stage=stages.next().ok_or_else(||invalid("accepted run has no RIFE producer"))?;
        if stages.next().is_some()||stage.provider_lock.payload.provider.id!=RIFE_PROVIDER_ID{return Err(invalid("RIFE producer is ambiguous or has a different owner"));}
        let configuration=RifeProviderConfiguration {schema:RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),configuration:self.configuration.clone()};
        if stage.provider_lock.payload.effective_configuration_sha256!=configuration.provider_configuration()?.effective_configuration_sha256
            ||stage.provider_lock.payload.manifest_sha256!=crate::provider::canonical_sha256(&configuration.manifest()?)? {return Err(invalid("accepted RIFE configuration/provider differs from the exact output index"));}
        let source_port=stage.inputs.iter().find(|p|p.port=="index").ok_or_else(||invalid("RIFE original index port is missing"))?;
        if source_port.artifacts.len()!=1||!plan.payload.inputs.iter().any(|i|i.id==source_port.artifacts[0]&&i.content_sha256==self.source_index_sha256){return Err(invalid("RIFE original index is not the bound immutable stage input"));}
        let index=selected(&before,"application/vnd.aniflow.rife-output-index+json")?;
        let index_path=output_path(&self.accepted_run,index)?;
        let (bytes,digest)=read(&index_path,crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES)?;
        if digest!=(index.sha256.clone(),index.byte_count)||RifeOutputIndex::from_json_slice(&bytes)?!=self.output_index{return Err(invalid("RIFE output index differs from its accepted artifact"));}
        let frames=selected(&before,"application/vnd.aniflow.rife-frames")?;
        if frames.kind!=crate::ArtifactKind::Directory{return Err(invalid("accepted RIFE frame output must be a directory"));}
        let directory=output_path(&self.accepted_run,frames)?;
        if crate::status_v3(&self.accepted_run)?!=before{return Err(invalid("RIFE acceptance changed during assembly binding"));}Ok(directory)
    }
}

/// Author a new explicit carrier from existing accepted outputs. Originals,
/// model files and the accepted RIFE run are never modified or rerun.
pub fn bind_accepted_run(run:&Path,configuration:RifeConfiguration,source_index:&Path,destination:&Path)->Result<RifeAssemblyIndex> {
    ensure_canonical_directory(destination.parent().ok_or_else(||invalid("RIFE assembly carrier needs an existing parent"))?)?;require_absent(destination)?;
    if destination.starts_with(run){return Err(invalid("assembly carrier must be outside the accepted RIFE run"));}
    let value=observe_accepted_run(run,configuration,source_index)?;
    let bytes=canonical_json_bytes(&value)?;
    if bytes.len() as u64>ASSEMBLY_MAXIMUM_INDEX_BYTES{return Err(invalid("RIFE assembly carrier exceeds the index bound"));}
    let mut options=OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut file=options.open(destination).map_err(|_|invalid("cannot exclusively create RIFE assembly carrier"))?;
    file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|invalid("cannot sync RIFE assembly carrier"))?;Ok(value)
}

/// Same native join as binding, without creating a carrier or launching a tool.
pub fn observe_accepted_run(run:&Path,configuration:RifeConfiguration,source_index:&Path)->Result<RifeAssemblyIndex> {
    ensure_canonical_directory(run)?;
    let before=crate::status_v3(run)?;
    let source_path=source_index;
    let (source_bytes,source_identity)=read(source_index,crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES)?;
    let source_index=RifeFrameIndex::from_json_slice(&source_bytes)?;
    let output=selected(&before,"application/vnd.aniflow.rife-output-index+json")?;
    let (bytes,_)=read(&output_path(run,output)?,crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES)?;
    let output_index=RifeOutputIndex::from_json_slice(&bytes)?;
    if output_index.schema!=RIFE_OUTPUT_INDEX_SCHEMA_V1{return Err(invalid("accepted output is not a RIFE output index"));}
    let value=RifeAssemblyIndex {schema:RIFE_ASSEMBLY_INDEX_SCHEMA_V1.into(),accepted_run:run.to_path_buf(),accepted_manifest_sha256:before.manifest_sha256,
        source_index_sha256:source_identity.0.clone(),configuration,source_index,output_index};
    value.validate_binding()?;
    if identity(source_path,crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES)?!=source_identity{return Err(invalid("original RIFE index changed during binding"));}
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loose_output_index_cannot_be_adopted_as_an_accepted_assembly_carrier() {
        let root=tempfile::tempdir().unwrap();let path=root.path().canonicalize().unwrap().join("loose.json");
        std::fs::write(&path,b"{\"schema\":\"aniflow.rife-output-index/v1\",\"frames\":[]}").unwrap();
        assert!(super::super::read_index(&path).is_err());
    }
}
