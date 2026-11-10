//! Explicit accepted-spatial input for the same serial RIFE pair engine.
pub mod runtime;
use super::*;
use std::path::PathBuf;
use serde::{Deserialize,Serialize};
use crate::frame_io::{identity,inventory,inspect_png,ensure_canonical_directory};
use crate::upscayl_recovery::{UpscaylOutputIndex,UPSCAYL_COLLECT_PROVIDER_ID,UPSCAYL_CHUNK_COLLECT_PROVIDER_ID};
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.spatial-rife";
pub const CAPABILITY_ID:&str="aniflow/spatial-rife-interpolation";
pub const OUTPUT_SCHEMA:&str="aniflow.rife-spatial-output-index/v1";
pub const OUTPUT_TYPE:&str="application/vnd.aniflow.rife-spatial-output-index+json";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialBinding {pub run_directory:PathBuf,pub manifest_sha256:String,pub index_sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialInput {pub binding:SpatialBinding,pub index:UpscaylOutputIndex}
pub(crate) struct Observation {pub input:SpatialInput,pub frames:PathBuf,pub index_path:PathBuf,pub bytes:u64}
fn mapped(e:crate::PipelinePlanningFailure)->Error {Error::new(e.category(),e.message)}
pub(crate) fn observe_upscale(root:&Path)->Result<Observation> {
    ensure_canonical_directory(root)?;let before=crate::status_v3(root)?;
    if before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none(){return Err(invalid("spatial RIFE requires current complete accepted Upscayl output"));}
    let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let value:serde_json::Value=crate::destination::local::read_bounded(&workspace.plan(),67_108_864)?;
    let plan=crate::PipelineV3Plan::from_json_slice(&crate::provider::canonical_json_bytes(&value)?).map_err(mapped)?;
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let entries=before.payload.outputs.iter().filter(|o|o.id==id).collect::<Vec<_>>();if entries.len()!=1{return Err(invalid("accepted spatial output missing or ambiguous"));}Ok(entries[0])};
    let index_artifact=select("upscaled_index")?;let frame_artifact=select("upscaled_frames")?;
    let own=|a:&crate::ArtifactEvidence|->Result<PathBuf>{let path=root.join(a.relative_path.as_deref().ok_or_else(||invalid("spatial locator missing"))?);if !path.starts_with(root)||path.canonicalize().ok().as_deref()!=Some(path.as_path()){return Err(invalid("spatial output escaped its canonical run"));}Ok(path)};
    let index_path=own(index_artifact)?;let frames=own(frame_artifact)?;
    if identity(&index_path,67_108_864)?!=(index_artifact.sha256.clone(),index_artifact.byte_count){return Err(invalid("accepted spatial index changed"));}
    let index:UpscaylOutputIndex=crate::destination::local::read_bounded(&index_path,67_108_864)?;index.validate()?;
    let source=index.source_index.temporal_inspection.source_sha256.as_str();
    if plan.plan_sha256!=before.payload.plan_sha256||!plan.payload.inputs.iter().any(|i|i.id=="source_video"&&i.content_sha256==source)
        ||!plan.payload.stages.iter().any(|s|matches!(s.provider_lock.payload.provider.id.as_str(),UPSCAYL_COLLECT_PROVIDER_ID|UPSCAYL_CHUNK_COLLECT_PROVIDER_ID)
            &&["upscaled_index","upscaled_frames"].iter().all(|id|s.outputs.iter().any(|o|o.artifacts.iter().any(|a|a.id==*id))))
        ||frame_artifact.kind!=crate::ArtifactKind::Directory||index_artifact.kind!=crate::ArtifactKind::File{return Err(invalid("spatial input lacks the exact native collector/source authority"));}
    let names=index.frames.iter().map(|f|std::ffi::OsString::from(&f.filename)).collect::<Vec<_>>();inventory(&frames,&names)?;
    let declaration=crate::AuthoredPipelineInput {id:"spatial_frames".into(),artifact_type:"application/vnd.aniflow.rife-frames".into(),artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video)};
    let observed=crate::pipeline_v3::hash_pipeline_input(&declaration,&frames).map_err(mapped)?;
    if observed.byte_count!=frame_artifact.byte_count||observed.file_count!=frame_artifact.file_count{return Err(invalid("accepted spatial directory changed"));}
    // Run outputs use the provider evidence digest contract, while new inputs
    // use portable planning digests. status_v3 owns the former; PNG checks and
    // an unchanged accepted manifest bridge the fresh planning identity.
    for frame in &index.frames {if inspect_png(&frames.join(&frame.filename),index.output_width,index.output_height,268_435_456)?.0!=frame.sha256{return Err(invalid("accepted spatial PNG differs from its native index"));}}
    if crate::status_v3(root)?!=before||identity(&index_path,67_108_864)?.0!=index_artifact.sha256{return Err(invalid("Upscayl acceptance changed during spatial binding"));}
    Ok(Observation {input:SpatialInput {binding:SpatialBinding {run_directory:root.to_path_buf(),manifest_sha256:before.manifest_sha256.clone(),index_sha256:index_artifact.sha256.clone()},index},frames,index_path,bytes:observed.byte_count})
}
impl SpatialInput {
    pub fn validate_binding(&self)->Result<PathBuf> {
        self.index.validate()?;let current=observe_upscale(&self.binding.run_directory)?;
        if current.input!=*self{return Err(invalid("spatial RIFE input differs from current native Upscayl acceptance"));}Ok(current.frames)
    }
}
/// Original inspection remains intact; only processing geometry/resources and
/// the explicit spatial artifact identity differ from original-frame RIFE.
pub fn plan(configuration:&RifeConfiguration,index:&UpscaylOutputIndex)->Result<RifePlan> {
    index.validate()?;let mut value=super::plan(configuration,&index.source_index)?;
    value.frame_index_sha256=canonical_sha256(index)?;
    value.resources=Some(super::resolve_resources(configuration,index.output_width,index.output_height)?);
    if !value.enabled||value.output_frame_count>100_000{return Err(invalid("spatial RIFE requires enabled interpolation and at most 50000 spatial source frames"));}Ok(value)
}
pub(crate) fn admit_geometry(configuration:&RifeConfiguration,width:u32,height:u32)->Result<()> {configuration.validate()?;super::resolve_resources(configuration,width,height)?;Ok(())}
pub(crate) fn validate_output(output:&RifeOutputIndex,configuration:&RifeConfiguration,index:&UpscaylOutputIndex)->Result<()> {
    output.validate_mapping(plan(configuration,index)?,&index.frames,OUTPUT_SCHEMA)
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Wrapper {pub schema:String,pub configuration:RifeConfiguration,pub spatial:SpatialBinding}
impl Wrapper {
    pub(crate) fn validate(&self)->Result<()> {
        self.configuration.validate()?;
        if self.schema!="aniflow.spatial-rife-configuration/v1"||!self.configuration.enabled{return Err(invalid("spatial RIFE requires its enabled fixed profile"));}
        for sha in [&self.spatial.manifest_sha256,&self.spatial.index_sha256]{require_sha256(sha,"spatial RIFE binding")?;}Ok(())
    }
    fn reference()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:"aniflow.spatial-rife-configuration/v1".into(),version:"1.0.0".into(),sha256:format!("{:x}",Sha256::digest(include_bytes!("../../docs/contracts/spatial-rife-configuration-v1.schema.json")))}}
    pub(crate) fn provider_configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let value=serde_json::to_value(self).map_err(|_|invalid("cannot serialize spatial RIFE settings"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:CAPABILITY_ID.into(),version:"1.0.0".into()},Self::reference(),value.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    pub(crate) fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let source=RifeProviderConfiguration {schema:RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),configuration:self.configuration.clone()}.manifest()?;
        let mut value=serde_json::to_value(source).map_err(|_|invalid("cannot project spatial RIFE capability"))?;
        value["provider"]["id"]=serde_json::json!(PROVIDER_ID);value["configuration_schemas"]=serde_json::json!([Self::reference()]);
        let cap=&mut value["capabilities"][0];cap["id"]=serde_json::json!(CAPABILITY_ID);cap["configuration_schema"]=serde_json::json!(Self::reference());cap["inputs"][1]["artifact_type"]=serde_json::json!("application/vnd.aniflow.upscayl-output-index+json");cap["outputs"][1]["artifact_type"]=serde_json::json!(OUTPUT_TYPE);
        crate::ProviderManifest::from_json_slice(&crate::provider::canonical_json_bytes(&value)?)
    }
}
#[cfg(all(test,unix))]
mod tests {
    use super::*;
    fn fixture()->(tempfile::TempDir,RifeConfiguration,UpscaylOutputIndex) {
        let (root,request)=super::super::provider::runtime_fixture();let original=super::super::provider::read_index(&request.frame_index).unwrap().0;
        let mut frames=original.frames.clone();for frame in &mut frames {frame.sha256="e".repeat(64);}
        (root,request.configuration,UpscaylOutputIndex {schema:crate::upscayl_recovery::UPSCAYL_OUTPUT_INDEX_SCHEMA_V1.into(),output_width:original.width*2,output_height:original.height*2,output_profile:original.frame_profile.clone(),source_index:original,frames})
    }
    fn output(configuration:&RifeConfiguration,index:&UpscaylOutputIndex)->RifeOutputIndex {
        let plan=plan(configuration,index).unwrap();let mut frames=super::super::provider::output_frame_template(&plan).unwrap();
        for frame in &mut frames {frame.sha256=if frame.origin==RifeFrameOrigin::InterpolatedMidpoint {"f".repeat(64)}else{index.frames[frame.source_left_ordinal as usize].sha256.clone()};}
        RifeOutputIndex {schema:OUTPUT_SCHEMA.into(),plan,frames}
    }
    #[test]
    fn spatial_plan_keeps_original_inspection_and_exact_duration_but_distinct_identity() {
        let (_root,configuration,index)=fixture();let before=index.source_index.clone();let planned=plan(&configuration,&index).unwrap();let original=super::super::plan(&configuration,&before).unwrap();
        assert_eq!(index.source_index,before);assert_eq!(planned.source_sha256,original.source_sha256);assert_eq!(planned.duration,original.duration);assert_eq!(planned.output_frame_rate,original.output_frame_rate);assert_eq!(planned.frame_index_sha256,canonical_sha256(&index).unwrap());assert_ne!(planned.frame_index_sha256,original.frame_index_sha256);
    }
    #[test]
    fn spatial_held_frames_must_match_actual_upscaled_pixels_and_not_original_pngs() {
        let (_root,configuration,index)=fixture();let mut output=output(&configuration,&index);validate_output(&output,&configuration,&index).unwrap();assert!(output.validate_against(&configuration,&index.source_index).is_err());
        output.frames[0].sha256=index.source_index.frames[0].sha256.clone();assert!(validate_output(&output,&configuration,&index).is_err());
    }
    #[test]
    fn spatial_carrier_keeps_both_geometries_and_derives_the_original_clock() {
        let (_root,configuration,index)=fixture();let output=output(&configuration,&index);
        let carrier=crate::video_assembly::rife::RifeAssemblyIndex {schema:crate::video_assembly::rife::RIFE_ASSEMBLY_INDEX_SCHEMA_V1.into(),accepted_run:"/synthetic/rife".into(),accepted_manifest_sha256:"a".repeat(64),source_index_sha256:"b".repeat(64),configuration,source_index:index.source_index.clone(),output_index:output,
            spatial_input:Some(SpatialInput {binding:SpatialBinding {run_directory:"/synthetic/upscale".into(),manifest_sha256:"c".repeat(64),index_sha256:"b".repeat(64)},index:index.clone()}),recovered_input:None};
        let clock=carrier.clock().unwrap();assert_eq!(carrier.dimensions(),(index.output_width,index.output_height));assert_eq!(carrier.source_index.width,index.source_index.width);clock.validate_source(&index.source_index.temporal_inspection).unwrap();assert!(carrier.validate_binding().is_err());
    }
    #[test]
    fn unsupported_scale_or_memory_geometry_refuses_without_neural_work() {
        let (_root,configuration,mut index)=fixture();index.output_width+=1;assert!(plan(&configuration,&index).is_err());
        assert!(admit_geometry(&configuration,7680,4320).is_err());
    }
    #[test]
    fn loose_spatial_index_has_no_native_acceptance_and_observation_writes_nothing() {
        let root=tempfile::tempdir().unwrap();let path=root.path().canonicalize().unwrap();std::fs::write(path.join("index.json"),b"{}").unwrap();
        assert!(observe_upscale(&path).is_err());assert_eq!(std::fs::read_dir(path).unwrap().count(),1);
    }
}
