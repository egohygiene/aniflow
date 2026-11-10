//! Exact-final-file PCM/WAV/FLAC measurement using existing native owners.
//! Source acceptance, new native signal acceptance and unqualified meter
//! observations remain distinct; this facade never changes or approves media.
use std::path::{Path,PathBuf};
use serde::{Serialize,Deserialize};
use crate::{Result,CancellationToken};
use crate::destination::{MediaInput,MediaFacts,MediaObservation,PcmEncoding,invalid,cancelled,local};
use crate::audio_inspection::{AudioInspectionConfiguration,NativeSampleFormat,AudioInspectionRequest};
use crate::audio_signal::{AudioSignalMeasurement,AudioSignalValue,AudioSignalUnit,AudioSignalUnavailableReason,NativeSignalAnalysisConfiguration,SignalAnalysisRequest};
use crate::frame_io::{identity,copy_pinned,ensure_canonical_directory};
use super::DeliveryLoudnessSummary;
pub mod resumable;
mod runtime;
pub use runtime::{run,observe};
#[cfg(test)]
mod tests;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum FinalLosslessSubject {
    AcceptedPcm {file:PathBuf,signal_run_directory:PathBuf},
    AcceptedAudioExport {run_directory:PathBuf},
}
impl FinalLosslessSubject {
    pub fn input(&self)->MediaInput {match self {Self::AcceptedPcm {file,signal_run_directory}=>MediaInput::AcceptedPcm {file:file.clone(),signal_run_directory:signal_run_directory.clone()},Self::AcceptedAudioExport {run_directory}=>MediaInput::AcceptedAudioExport {run_directory:run_directory.clone()}}}
    pub fn run_directory(&self)->&Path {match self {Self::AcceptedPcm {signal_run_directory,..}=>signal_run_directory,Self::AcceptedAudioExport {run_directory}=>run_directory}}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalLosslessQcRequest {
    pub schema:String,pub subject:FinalLosslessSubject,pub configuration:AudioInspectionConfiguration,pub provider_executable:PathBuf,
    pub maximum_pcm_bytes:u64,pub maximum_working_bytes:u64,pub minimum_free_bytes:u64,
}
impl FinalLosslessQcRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("final lossless QC request exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"final lossless audio QC request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        self.configuration.validate()?;
        if !matches!(self.schema.as_str(),"aniflow.final-lossless-audio-qc-request/v1"|"aniflow.final-lossless-audio-qc-request/v2")||self.configuration.ffmpeg.version!="8.0.1"||self.configuration.ffprobe.version!="8.0.1"
            ||self.configuration.ffmpeg.executable==self.configuration.ffprobe.executable||!(1024..=crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&self.maximum_pcm_bytes)
            ||!(8_388_608..=1_099_511_627_776).contains(&self.maximum_working_bytes)||self.minimum_free_bytes>1_099_511_627_776 {
            return Err(invalid("final lossless QC requires exact native tools, original PCM precision and finite complete decode/storage bounds"));
        }Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QcComponent {pub path:PathBuf,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalLosslessQcIntent {pub schema:String,pub request:FinalLosslessQcRequest,pub source:MediaObservation,pub components:Vec<QcComponent>,pub projected_working_bytes:u64}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum FinalLosslessMeter {
    Observed {receipt:Box<crate::audio_signal::loudness::NativeLoudnessReceipt>},
    Unavailable {reason:AudioSignalUnavailableReason},
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalLosslessQcReport {
    pub schema:String,pub intent:FinalLosslessQcIntent,pub intent_sha256:String,
    pub decoded_wav_sha256:String,pub decoded_wav_bytes:u64,pub decoded_pcm_sha256:String,pub encoding:PcmEncoding,
    pub signal_run_directory:PathBuf,pub signal_plan_sha256:String,pub signal_manifest_sha256:String,pub accepted_pcm:DeliveryLoudnessSummary,
    pub integrated_loudness:AudioSignalMeasurement,pub true_peak:AudioSignalMeasurement,pub meter:FinalLosslessMeter,
    pub normalization:String,pub runtime_qualified:bool,pub creative_approval:String,pub publication_authority:String,
}
pub(crate) fn source_facts(source:&MediaObservation)->Result<(&DeliveryLoudnessSummary,PcmEncoding,&str)> {
    match &source.facts {
        MediaFacts::Pcm {summary,encoding,pcm_sha256,..}=>Ok((summary,*encoding,pcm_sha256)),
        MediaFacts::LosslessAudio {report}=>Ok((&report.operation.source.summary,report.encoding,&report.decoded_pcm_sha256)),
        _=>Err(invalid("final lossless QC requires plain accepted native PCM or lossless export, never nested QC or video")),
    }
}
fn native(encoding:PcmEncoding)->NativeSampleFormat {match encoding {PcmEncoding::Pcm16=>NativeSampleFormat::Pcm16,PcmEncoding::Pcm24=>NativeSampleFormat::Pcm24,PcmEncoding::Float32=>NativeSampleFormat::Float32}}
fn flac(intent:&FinalLosslessQcIntent)->bool {matches!(&intent.source.facts,MediaFacts::LosslessAudio {report} if report.operation.configuration.format==crate::lossless_audio::LosslessFormat::Flac)}
fn components(request:&FinalLosslessQcRequest)->Result<Vec<QcComponent>> {
    let mut result=Vec::new();
    for pin in [&request.configuration.ffmpeg,&request.configuration.ffprobe] {
        crate::audio_inspection::process::verify_pin(pin,"final lossless QC tool").map_err(|_|invalid("final lossless QC explicit tool pin unavailable"))?;
        let value=identity(&pin.executable,134_217_728)?;result.push(QcComponent {path:pin.executable.clone(),sha256:value.0,byte_count:value.1});
    }
    let value=identity(&request.provider_executable,134_217_728)?;result.push(QcComponent {path:request.provider_executable.clone(),sha256:value.0,byte_count:value.1});Ok(result)
}
/// Read only current accepted source and explicit component identities.
pub fn plan(request:&FinalLosslessQcRequest,token:&CancellationToken)->Result<FinalLosslessQcIntent> {
    request.validate()?;cancelled(token)?;let source=crate::destination::observe_media(&request.subject.input(),token)?;
    let (summary,encoding,_)=source_facts(&source)?;let original=&summary.source;
    if original.stem.is_some()||original.stream_index!=0||original.origin.numerator!=0||original.frame_count==0
        ||original.frame_count>u64::from(original.sample_rate_hz)*600||source.file.byte_count>crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES {
        return Err(invalid("final lossless QC requires complete original zero-origin native samples within the existing ten-minute profile"));
    }
    let pcm=original.frame_count.checked_mul(u64::from(original.channels)*u64::from(native(encoding).bytes_per_sample())).and_then(|v|v.checked_add(4096)).ok_or_else(||invalid("final lossless PCM bound overflow"))?;
    if pcm>request.maximum_pcm_bytes||!matches!(&source.facts,MediaFacts::LosslessAudio {report} if report.operation.configuration.format==crate::lossless_audio::LosslessFormat::Flac)&&source.file.byte_count>request.maximum_pcm_bytes{return Err(invalid("complete native PCM cannot fit the selected final-QC bound; no truncation or precision reduction"));}
    let components=components(request)?;let tool_bytes=components.iter().try_fold(0u64,|n,p|n.checked_add(p.byte_count).ok_or_else(||invalid("QC component sum overflow")))?;
    let projected_working_bytes=pcm.checked_mul(12).and_then(|v|v.checked_add(source.file.byte_count.checked_mul(2)?)).and_then(|v|v.checked_add(tool_bytes.checked_mul(3)?)).and_then(|v|v.checked_add(32*1024*1024)).ok_or_else(||invalid("final QC working budget overflow"))?;
    if projected_working_bytes>request.maximum_working_bytes{return Err(invalid("final native decode/signal working estimate exceeds selected admission"));}
    cancelled(token)?;Ok(FinalLosslessQcIntent {schema:intent_schema(request).into(),request:request.clone(),source,components,projected_working_bytes})
}
fn private_configuration(root:&Path,intent:&FinalLosslessQcIntent)->AudioInspectionConfiguration {
    let mut configuration=intent.request.configuration.clone();configuration.ffmpeg.executable=root.join("ffmpeg");configuration.ffprobe.executable=root.join("ffprobe");configuration
}
fn signal_request(root:&Path,intent:&FinalLosslessQcIntent)->SignalAnalysisRequest {
    SignalAnalysisRequest::new_native(AudioInspectionRequest::new(root.join("decoded.wav"),private_configuration(root,intent),root.join("aniflow-provider")),NativeSignalAnalysisConfiguration::default())
}
fn wave(root:&Path,intent:&FinalLosslessQcIntent)->Result<(String,u64)> {
    let path=root.join("decoded.wav");let pin=identity(&path,intent.request.maximum_pcm_bytes)?;let actual=crate::audio_inspection::wav::inspect_native(&path)?;
    let (summary,encoding,pcm)=source_facts(&intent.source)?;let expected=&summary.source;
    if actual.sample_format!=native(encoding)||actual.sample_rate_hz!=expected.sample_rate_hz||actual.channels!=expected.channels||actual.frame_count!=expected.frame_count||actual.pcm_sha256!=pcm {
        return Err(invalid("final decoded native samples, precision, rate, channels or complete count changed"));
    }Ok(pin)
}
fn unavailable(channels:u16)->(AudioSignalMeasurement,AudioSignalMeasurement,FinalLosslessMeter) {
    let reason=AudioSignalUnavailableReason::UnsupportedNativeSignalProfile;let scope=crate::audio_analysis::AudioScope {channels:(0..channels).collect(),stem_id:None};
    (AudioSignalMeasurement {unit:AudioSignalUnit::LoudnessUnitsFullScale,scope:scope.clone(),value:AudioSignalValue::Unavailable {reason}},
     AudioSignalMeasurement {unit:AudioSignalUnit::DecibelsTruePeak,scope,value:AudioSignalValue::Unavailable {reason}},FinalLosslessMeter::Unavailable {reason})
}

fn intent_schema(request:&FinalLosslessQcRequest)->&'static str {if request.schema=="aniflow.final-lossless-audio-qc-request/v2" {"aniflow.final-lossless-audio-qc-intent/v2"}else{"aniflow.final-lossless-audio-qc-intent/v1"}}
fn report_schema(request:&FinalLosslessQcRequest)->&'static str {if request.schema=="aniflow.final-lossless-audio-qc-request/v2" {"aniflow.final-lossless-audio-qc/v2"}else{"aniflow.final-lossless-audio-qc/v1"}}
fn meter_schema(request:&FinalLosslessQcRequest,rate:u32,channels:u16)->Option<&'static str> {
    if !matches!(channels,1|2){return None;}match rate {48000=>Some("aniflow.native-pcm-loudness/v2"),44100 if request.schema=="aniflow.final-lossless-audio-qc-request/v2"=>Some("aniflow.native-pcm-loudness/v3"),_=>None}
}
