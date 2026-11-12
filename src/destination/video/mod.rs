//! Selected destination-video intent over current accepted native video.
mod intent;
pub use intent::*;
mod runtime;
mod native;
pub use runtime::{plan,run,resume,observe};
pub(crate) use runtime::{accepted as accepted_export,run_at};
pub use native::execute_invocation;
use serde::{Deserialize,Serialize};
use crate::{Result,CancellationToken};
use crate::destination::{invalid,local};
use crate::provider::{canonical_json_bytes,canonical_sha256};
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.destination-video";
pub const CAPABILITY_ID:&str="aniflow/encode-destination-video";
pub(crate) const OPERATION_SCHEMA:&str="aniflow.destination-video-operation/v1";
pub(crate) const REPORT_BYTES:u64=67_108_864;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportOperation {pub schema:String,pub intent:VideoExportIntent}
impl VideoExportOperation {
    pub(crate) fn validate(&self)->Result<()> {
        let p=&self.intent.payload;
        p.request.configuration.validate()?;
        if self.schema!=OPERATION_SCHEMA||self.intent.schema!="aniflow.destination-video-intent/v1"||self.intent.intent_sha256!=canonical_sha256(p)?
            ||p.request.artifact_id!=p.artifact.artifact_id||!p.source.artifacts.contains(&p.artifact)
            ||resolve_geometry(p.artifact.width,p.artifact.height,&p.request.configuration.geometry,p.request.configuration.allow_upscale)?!=p.geometry {
            return Err(invalid("destination native operation differs from its exact source/configuration/geometry intent"));
        }
        intent::check_source(&p.artifact,&p.request.configuration,&p.geometry)?;
        if canonical_json_bytes(self)?.len()>262_144{return Err(invalid("destination operation exceeds 256 KiB"));}Ok(())
    }
    fn reference()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:OPERATION_SCHEMA.into(),version:"1.0.0".into(),sha256:local::hash(include_bytes!("../../../docs/contracts/destination-video-operation-v1.schema.json"))}}
    pub(crate) fn configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let v=serde_json::to_value(self).map_err(|_|invalid("cannot serialize destination operation"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:CAPABILITY_ID.into(),version:"1.0.0".into()},Self::reference(),v.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    pub(crate) fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let settings=&self.intent.payload.request.configuration.tools;
        let port=|name:&str,media:&str,role:&str,stream:&str|serde_json::json!({"name":name,"artifact_type":media,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value=serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow selected destination video encoder"},"configuration_schemas":[Self::reference()],"capabilities":[{
            "id":CAPABILITY_ID,"version":"1.0.0","kind":"assembler_encoder","configuration_schema":Self::reference(),"inputs":[port("source","video/mp4","temporal_source","video")],"outputs":[port("media","video/mp4","candidate_master","video"),port("evidence","application/vnd.aniflow.destination-video-report+json","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":crate::video_assembly::component_inventory(settings).tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":format!("={}",t.version),"sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
    pub(crate) fn output_limit(&self)->u64 {self.intent.payload.request.configuration.tools.maximum_output_bytes.min(self.intent.payload.request.configuration.profile.maximum_file_bytes)}
    pub(crate) fn working_bytes(&self)->Result<u64> {
        let tools=&self.intent.payload.request.configuration.tools;
        let value=self.intent.payload.artifact.byte_count.checked_mul(2).and_then(|v|v.checked_add(self.output_limit().checked_mul(3)?)).and_then(|v|v.checked_add(REPORT_BYTES*3+268_435_456))
            .ok_or_else(||invalid("destination working bytes overflow"))?;
        if value>tools.maximum_working_bytes{return Err(invalid("destination source/candidate/report/scratch bounds exceed maximum_working_bytes"));}Ok(value)
    }
    pub(crate) fn selection(&self)->crate::temporal::StreamSelection {
        let audio=self.intent.payload.artifact.audio.is_some();crate::temporal::StreamSelection {video_stream:Some(0),audio_stream:audio.then_some(1),no_audio:!audio,discard_streams:Vec::new(),audio_extent:audio.then_some(crate::temporal::AudioExtentPolicy::PreserveExact)}
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportMedia {
    pub source:crate::temporal::TemporalInspection,pub candidate:crate::temporal::TemporalInspection,
    pub geometry:ResolvedGeometry,pub playback:crate::delivery_metadata::remux::MetadataRemuxPlayback,
    pub metadata:crate::delivery_metadata::MetadataPrivateReceipt,pub container:crate::delivery_metadata::mp4::Mp4MetadataObservation,
    pub audio_packets_preserved:bool,pub complete_clock_preserved:bool,pub full_decode_observed:bool,pub native_qualified:bool,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportReport {pub schema:String,pub operation:VideoExportOperation,pub provider_lock_sha256:String,pub output_sha256:String,pub output_bytes:u64,pub media:VideoExportMedia,pub quality_approval:String,pub publication_authority:String}
pub(crate) fn read_document<T:serde::de::DeserializeOwned>(path:&std::path::Path)->Result<T> {local::read_bounded(path,REPORT_BYTES)}
#[cfg(test)]
pub(crate) mod tests;
