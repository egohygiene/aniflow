//! Selected source-preserving WAV/FLAC candidates over ordinary Pipeline v3.
mod types;
mod runtime;
mod native;
pub use types::*;
pub use runtime::{plan,run,resume,observe};
pub(crate) use runtime::run_at;
pub(crate) use native::{execute_invocation,decode_for_qc};
use std::path::Path;
use crate::{Result,CancellationToken};
use crate::destination::{invalid,cancelled,PcmEncoding,MediaInput,MediaFacts};
use crate::destination::local;
use crate::frame_io::identity;
use crate::provider::canonical_json_bytes;
const SCHEMA:&str="aniflow.lossless-audio-operation/v1";
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.lossless-audio";
pub const CAPABILITY_ID:&str="aniflow/lossless-audio-export";
const TOOL_BYTES:u64=134_217_728;
const MEDIA_BYTES:u64=268_435_456;
impl LosslessConfiguration {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("lossless configuration exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"lossless audio configuration")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        self.tools.validate()?;
        if self.schema!="aniflow.lossless-audio-configuration/v1"||self.tools.ffmpeg.version!="8.0.1"||self.tools.ffprobe.version!="8.0.1"
            ||self.tools.ffmpeg.executable==self.tools.ffprobe.executable||!(1024..=MEDIA_BYTES).contains(&self.maximum_output_bytes)
            ||!(8_388_608..=1_099_511_627_776).contains(&self.maximum_working_bytes)||self.minimum_free_bytes>1_099_511_627_776 {
            return Err(invalid("lossless export requires exact FFmpeg/FFprobe 8.0.1, selected metadata policy and finite storage bounds"));
        }Ok(())
    }
}
fn source(file:&Path,signal:&Path,token:&CancellationToken)->Result<ExportSource> {
    let observation=crate::destination::observe_media(&MediaInput::AcceptedPcm {file:file.to_path_buf(),signal_run_directory:signal.to_path_buf()},token)?;
    let MediaFacts::Pcm {summary,encoding,pcm_sha256,run_manifest_sha256}=observation.facts else {return Err(invalid("lossless export requires current exact original PCM signal evidence"));};
    Ok(ExportSource {file:file.to_path_buf(),signal_run_directory:signal.to_path_buf(),summary,source_manifest_sha256:run_manifest_sha256,encoding,pcm_sha256})
}
impl LosslessOperation {
    fn validate(&self)->Result<()> {
        self.configuration.validate()?;self.source.summary.source.validate()?;
        for digest in [&self.source.pcm_sha256,&self.source.source_manifest_sha256,&self.source.summary.report_sha256] {crate::provider::require_sha256(digest,"lossless source evidence")?;}
        if self.schema!=SCHEMA||self.source.summary.schema!="aniflow.delivery-loudness-summary/v1"||self.source.summary.normalization!="none"
            ||self.source.summary.source.stem.is_some()||self.source.summary.source.stream_index!=0||self.source.summary.source.origin.numerator!=0
            ||self.source.summary.source.frame_count==0||self.source.summary.source.artifact.byte_size>MEDIA_BYTES
            ||self.source.summary.source.frame_count>u64::from(self.source.summary.source.sample_rate_hz)*600
            ||self.configuration.format==LosslessFormat::Flac&&self.source.encoding==PcmEncoding::Float32 {
            return Err(invalid("lossless export requires complete zero-origin native PCM; float-to-FLAC needs a separate explicitly approved quantization profile"));
        }
        local::bytes(self)?;Ok(())
    }
    fn schema_reference()->crate::ConfigurationSchemaReference {
        crate::ConfigurationSchemaReference {id:SCHEMA.into(),version:"1.0.0".into(),sha256:local::hash(include_bytes!("../../docs/contracts/lossless-audio-operation-v1.schema.json"))}
    }
    fn provider_configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let value=serde_json::to_value(self).map_err(|_|invalid("cannot serialize lossless operation"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:CAPABILITY_ID.into(),version:"1.0.0".into()},Self::schema_reference(),value.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let port=|name:&str,kind:&str,role:&str,stream:&str|serde_json::json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value=serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow selected lossless audio export"},"configuration_schemas":[Self::schema_reference()],"capabilities":[{
            "id":CAPABILITY_ID,"version":"1.0.0","kind":"assembler_encoder","configuration_schema":Self::schema_reference(),"inputs":[port("source","audio/wav","temporal_source","audio")],
            "outputs":[port("media",self.configuration.format.media_type(),"candidate_master","audio"),port("evidence","application/vnd.aniflow.lossless-audio-report+json","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":components(&self.configuration).tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":format!("={}",t.version),"sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":"lossless","cacheable":false,"content_changes":false,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
    fn working_bytes(&self)->Result<u64> {
        let tools=observe_tools(&self.configuration)?;
        let total=self.source.summary.source.artifact.byte_size.checked_mul(3)
            .and_then(|n|n.checked_add(self.configuration.maximum_output_bytes.checked_mul(3)?))
            .and_then(|n|n.checked_add((tools[0].1+tools[1].1).checked_mul(2)?))
            .and_then(|n|n.checked_add(16_777_216)).ok_or_else(||invalid("lossless storage estimate overflow"))?;
        if total>self.configuration.maximum_working_bytes{return Err(invalid("lossless snapshots/candidate/evidence exceed the selected working-byte admission"));}Ok(total)
    }
}
fn components(configuration:&LosslessConfiguration)->crate::ComponentInventory {
    crate::ComponentInventory {tools:[("ffmpeg",&configuration.tools.ffmpeg),("ffprobe",&configuration.tools.ffprobe)].into_iter().map(|(id,p)|crate::ComponentIdentity {id:id.into(),version:p.version.clone(),sha256:Some(p.sha256.clone())}).collect(),codecs:vec![],models:vec![]}
}
fn observe_tools(configuration:&LosslessConfiguration)->Result<[(String,u64);2]> {
    let mut result=[(String::new(),0),(String::new(),0)];
    for ((id,pin),value) in [("ffmpeg",&configuration.tools.ffmpeg),("ffprobe",&configuration.tools.ffprobe)].into_iter().zip(&mut result) {
        crate::audio_inspection::process::verify_pin(pin,id).map_err(|_|invalid("lossless tool is absent or differs from its explicit pin"))?;*value=identity(&pin.executable,TOOL_BYTES)?;
    }Ok(result)
}
fn limits(configuration:&LosslessConfiguration)->crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits {timeout:std::time::Duration::from_millis(configuration.tools.tool_timeout_milliseconds*6+30_000),termination_grace_period:std::time::Duration::from_secs(2),
        maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,maximum_artifact_files:64,maximum_artifact_bytes:configuration.maximum_working_bytes}
}
#[cfg(test)]
mod tests;
