use std::path::PathBuf;
use serde::{Deserialize,Serialize};
use crate::{audio_inspection::AudioInspectionConfiguration,destination::PcmEncoding,delivery_audio::DeliveryLoudnessSummary};
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum LosslessFormat {Wav,Flac}
impl LosslessFormat {
    pub fn filename(self)->&'static str {match self {Self::Wav=>"audio.wav",Self::Flac=>"audio.flac"}}
    pub fn media_type(self)->&'static str {match self {Self::Wav=>"audio/wav",Self::Flac=>"audio/flac"}}
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ExportMetadataPolicy {RemoveInheritedDescriptiveMetadata}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LosslessConfiguration {
    pub schema:String,pub format:LosslessFormat,pub tools:AudioInspectionConfiguration,pub metadata_policy:ExportMetadataPolicy,
    pub maximum_output_bytes:u64,pub maximum_working_bytes:u64,pub minimum_free_bytes:u64,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSource {
    pub file:PathBuf,pub signal_run_directory:PathBuf,pub summary:DeliveryLoudnessSummary,pub source_manifest_sha256:String,
    pub encoding:PcmEncoding,pub pcm_sha256:String,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LosslessOperation {
    pub schema:String,pub source:ExportSource,pub configuration:LosslessConfiguration,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LosslessReport {
    pub schema:String,pub operation:LosslessOperation,pub provider_lock_sha256:String,
    pub output_sha256:String,pub output_byte_count:u64,pub decoded_pcm_sha256:String,
    pub sample_rate_hz:u32,pub channels:u16,pub frame_count:u64,pub encoding:PcmEncoding,
    pub verification_method:String,pub signal_changes:String,pub metadata_scope:String,
    pub after_export_meter:String,pub native_qualified:bool,pub creative_approval:String,pub publication_authority:String,
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LosslessCompletion {pub schema:String,pub report:LosslessReport,pub file:crate::destination::MediaFile}
#[derive(Debug,Clone)]
pub struct LosslessRequest {
    pub source:PathBuf,pub signal_run_directory:PathBuf,pub configuration:LosslessConfiguration,pub provider_executable:PathBuf,
}
