//! Complete recovered input for the existing serial RIFE owner. Historical
//! processing stays distinct from newly observed native interpolation.
use super::*;
use std::path::PathBuf;
use serde::{Serialize,Deserialize};
use crate::{CancellationToken,PipelineV3Plan};
use crate::upscayl_recovery::carrier::{self,CompleteSpatialCarrier};
use crate::frame_io::{identity,ensure_canonical_directory};
pub mod runtime;
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.recovered-rife";
pub const CAPABILITY_ID:&str="aniflow/recovered-rife-interpolation";
pub const OUTPUT_SCHEMA:&str="aniflow.rife-recovered-output-index/v1";
pub const OUTPUT_TYPE:&str="application/vnd.aniflow.rife-recovered-output-index+json";
pub const INPUT_TYPE:&str="application/vnd.aniflow.complete-spatial-carrier+json";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredBinding {pub carrier_directory:PathBuf,pub carrier_sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredInput {pub binding:RecoveredBinding,pub carrier:CompleteSpatialCarrier}
impl RecoveredInput {
    pub fn observe(binding:&RecoveredBinding,token:&CancellationToken)->Result<Self> {
        let carrier=carrier::observe(&binding.carrier_directory,&binding.carrier_sha256,token)?;Ok(Self {binding:binding.clone(),carrier})
    }
    pub fn validate_binding(&self,token:&CancellationToken)->Result<PathBuf> {
        self.carrier.validate_shape()?;
        if Self::observe(&self.binding,token)?!=*self{return Err(invalid("recovered RIFE input differs from exact current carrier/origins"));}Ok(self.carrier.frames_directory.clone())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredRifeRequest {pub schema:String,pub source:RecoveredBinding,pub configuration:RifeConfiguration,pub provider_executable:PathBuf}
impl RecoveredRifeRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{if bytes.len()>65_536{return Err(invalid("recovered RIFE request exceeds 64 KiB"));}let value:Self=decode_json(bytes,"recovered RIFE request")?;value.validate()?;Ok(value)}
    pub fn validate(&self)->Result<()> {
        self.configuration.validate()?;require_sha256(&self.source.carrier_sha256,"recovered RIFE carrier")?;
        if self.schema!="aniflow.recovered-rife-request/v1"||!self.configuration.enabled{return Err(invalid("recovered RIFE requires its explicit enabled bounded profile"));}Ok(())
    }
}
pub fn plan(configuration:&RifeConfiguration,carrier:&CompleteSpatialCarrier)->Result<RifePlan> {
    carrier.validate_shape()?;let mut value=super::plan(configuration,&carrier.selection.source_index)?;
    value.frame_index_sha256=canonical_sha256(carrier)?;
    value.resources=Some(super::resolve_resources(configuration,carrier.selection.request.output_width,carrier.selection.request.output_height)?);
    if !value.enabled||value.output_frame_count>RIFE_MAXIMUM_FRAMES{return Err(invalid("recovered RIFE requires enabled interpolation within 50000 source / 100000 derived frames"));}Ok(value)
}
pub(crate) fn validate_output(output:&RifeOutputIndex,configuration:&RifeConfiguration,carrier:&CompleteSpatialCarrier)->Result<()> {output.validate_mapping(plan(configuration,carrier)?,&carrier.frames(),OUTPUT_SCHEMA)}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Wrapper {pub schema:String,pub configuration:RifeConfiguration,pub recovered:RecoveredBinding}
impl Wrapper {
    pub(crate) fn validate(&self)->Result<()> {
        self.configuration.validate()?;require_sha256(&self.recovered.carrier_sha256,"recovered RIFE carrier")?;
        if self.schema!="aniflow.recovered-rife-configuration/v1"||!self.configuration.enabled{return Err(invalid("recovered RIFE wrapper needs explicit enabled configuration"));}Ok(())
    }
    fn reference()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:"aniflow.recovered-rife-configuration/v1".into(),version:"1.0.0".into(),sha256:format!("{:x}",Sha256::digest(include_bytes!("../../docs/contracts/recovered-rife-configuration-v1.schema.json")))}}
    pub(crate) fn provider_configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let v=serde_json::to_value(self).map_err(|_|invalid("cannot encode recovered RIFE configuration"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:CAPABILITY_ID.into(),version:"1.0.0".into()},Self::reference(),v.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    pub(crate) fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let original=RifeProviderConfiguration {schema:RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),configuration:self.configuration.clone()}.manifest()?;
        let mut value=serde_json::to_value(original).map_err(|_|invalid("cannot project recovered RIFE manifest"))?;value["provider"]["id"]=serde_json::json!(PROVIDER_ID);value["configuration_schemas"]=serde_json::json!([Self::reference()]);
        let cap=&mut value["capabilities"][0];cap["id"]=serde_json::json!(CAPABILITY_ID);cap["configuration_schema"]=serde_json::json!(Self::reference());cap["inputs"][1]["artifact_type"]=serde_json::json!(INPUT_TYPE);cap["outputs"][1]["artifact_type"]=serde_json::json!(OUTPUT_TYPE);
        crate::ProviderManifest::from_json_slice(&crate::provider::canonical_json_bytes(&value)?)
    }
}
#[cfg(all(test,unix))]
pub(super) mod tests;
