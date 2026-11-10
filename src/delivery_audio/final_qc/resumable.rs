//! Fixed video-AAC capture/decode/native-signal/meter restart boundaries.
use super::*;
use crate::audio_inspection::{AudioInspectionRequest,NativeSampleFormat};
use crate::audio_signal::{SignalAnalysisRequest,NativeSignalAnalysisConfiguration};
use crate::delivery_audio::source_signal;
use crate::frame_io::copy_pinned;
mod execution;
pub use execution::{run,resume};
#[cfg(test)] mod tests;
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {pub schema:String,pub qc:FinalAudioQcRequest}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("resumable video-QC request exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"resumable final-video audio QC request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {self.qc.validate()?;if self.schema!="aniflow.resumable-final-video-audio-qc-request/v1"{return Err(invalid("explicit resumable video-QC request version required"));}Ok(())}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {pub path:PathBuf,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {pub schema:String,pub request:Request,pub native:FinalAudioQcIntent,pub components:Vec<Component>,pub projected_working_bytes:u64,pub intent_sha256:String}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Phase {Capture,Decode,NativeSignal,Meter,Complete}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {pub schema:String,pub run_directory:PathBuf,pub intent_sha256:String,pub phase:Phase,pub native_plan_sha256:Option<String>,pub native_manifest_sha256:Option<String>,pub complete:bool,pub report:Option<FinalAudioQcReport>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Decoded {schema:String,intent_sha256:String,inspection:crate::temporal::TemporalInspection,channels:u16,wav_sha256:String,wav_bytes:u64,pcm_sha256:String,plan:crate::PipelineV3Plan}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Meter {schema:String,intent_sha256:String,native_manifest_sha256:String,signal:VideoAudioSignalReceipt}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {schema:String,intent_sha256:String}
fn digest(intent:&Intent)->Result<String> {canonical_sha256(&(&intent.schema,&intent.request,&intent.native,&intent.components,intent.projected_working_bytes))}
fn present(path:&Path)->Result<bool> {match std::fs::symlink_metadata(path) {Ok(m) if m.is_file()||m.is_dir()=>Ok(true),Ok(_)=>Err(invalid("video-QC member is symbolic/special; preserve it")),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(false),Err(_)=>Err(invalid("cannot inspect video-QC member"))}}
pub(crate) fn recognized(root:&Path)->Result<bool>{present(&root.join("resume-intent.json"))}
fn area(root:&Path)->PathBuf {root.join("signal")}
fn child(root:&Path)->PathBuf {area(root).join("children/signal")}
fn private(root:&Path,intent:&Intent)->AssemblyConfiguration {let mut c=intent.native.signal_request.configuration.clone();c.ffmpeg.executable=area(root).join("ffmpeg");c.ffprobe.executable=area(root).join("ffprobe");c}
fn analysis(root:&Path,intent:&Intent)->SignalAnalysisRequest {SignalAnalysisRequest::new_native(AudioInspectionRequest::new(area(root).join("selected-audio.wav"),private(root,intent).tool_limits(),area(root).join("aniflow-provider")),NativeSignalAnalysisConfiguration::default())}
fn disjoint(root:&Path,intent:&Intent)->Result<()> {
    local::disjoint(root,&crate::destination::MediaInput::AcceptedVideo {run_directory:intent.request.qc.source_run_directory.clone(),artifact_id:intent.request.qc.artifact_id.clone()},&intent.native.artifact.locator)?;
    for c in &intent.components{if root.starts_with(&c.path)||c.path.starts_with(root){return Err(invalid("resumable video-QC output overlaps a component"));}}Ok(())
}
/// Current native producer/file and explicit component identities; no process.
pub fn plan(request:&Request,token:&CancellationToken)->Result<Intent> {
    request.validate()?;let native=super::plan(&request.qc,token)?;let mut components=Vec::new();
    for path in [&request.qc.configuration.ffmpeg.executable,&request.qc.configuration.ffprobe.executable,&request.qc.provider_executable] {
        let pin=identity(path,536_870_912)?;components.push(Component {path:path.clone(),sha256:pin.0,byte_count:pin.1});
    }
    if components[0].sha256!=request.qc.configuration.ffmpeg.sha256||components[1].sha256!=request.qc.configuration.ffprobe.sha256{return Err(invalid("resumable video-QC tool pin differs"));}
    let a=native.artifact.audio.as_ref().ok_or_else(||invalid("video QC requires accepted audio"))?;
    let frames=a.interval.duration().and_then(|v|v.checked_mul(48_000)).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;
    if frames.denominator()!=1||frames.numerator()<=0{return Err(invalid("final AAC extent is not a complete positive sample count"));}
    let wav=(frames.numerator() as u64).checked_mul(a.channels).and_then(|v|v.checked_mul(4)).and_then(|v|v.checked_add(4096)).ok_or_else(||invalid("video QC PCM bound overflow"))?;
    if wav>request.qc.maximum_pcm_bytes{return Err(invalid("full final audio exceeds the PCM bound; no truncation/resampling"));}
    let tools=components.iter().try_fold(0u64,|n,c|n.checked_add(c.byte_count).ok_or_else(||invalid("video QC tool bound overflow")))?;
    let projected_working_bytes=native.artifact.byte_count.checked_mul(2).and_then(|v|v.checked_add(tools.checked_mul(3)?)).and_then(|v|v.checked_add(wav.checked_mul(12)?)).and_then(|v|v.checked_add(33_554_432)).ok_or_else(||invalid("video QC working bound overflow"))?;
    if projected_working_bytes>request.qc.configuration.maximum_working_bytes{return Err(invalid("full video QC capture/decode/native/metadata projection exceeds selected working bound"));}
    let mut intent=Intent {schema:"aniflow.resumable-final-video-audio-qc-intent/v1".into(),request:request.clone(),native,components,projected_working_bytes,intent_sha256:String::new()};intent.intent_sha256=digest(&intent)?;local::bytes(&intent)?;cancelled(token)?;Ok(intent)
}
fn retained(root:&Path,token:&CancellationToken)->Result<Intent> {ensure_canonical_directory(root)?;let i:Intent=local::read(&root.join("resume-intent.json"))?;
    if i.schema!="aniflow.resumable-final-video-audio-qc-intent/v1"||i.intent_sha256!=digest(&i)?||plan(&i.request,token)?!=i{return Err(invalid("video-QC current source acceptance/components/settings differ; retain root and start a new intent"));}disjoint(root,&i)?;Ok(i)}
fn capture_marker(i:&Intent)->Capture{Capture {schema:"aniflow.resumable-final-video-audio-qc-capture/v1".into(),intent_sha256:i.intent_sha256.clone()}}
fn snapshots(root:&Path,i:&Intent)->Result<()> {
    if identity(&area(root).join("source.media"),i.request.qc.configuration.maximum_source_bytes)?!=(i.native.artifact.sha256.clone(),i.native.artifact.byte_count){return Err(invalid("video QC snapshot changed"));}
    for (c,name) in i.components.iter().zip(["ffmpeg","ffprobe","aniflow-provider"]) {let pin=(c.sha256.clone(),c.byte_count);if identity(&c.path,536_870_912)?!=pin||identity(&area(root).join(name),536_870_912)?!=pin{return Err(invalid("video QC component changed"));}}Ok(())
}
fn decoded(root:&Path,i:&Intent,d:&Decoded,token:&CancellationToken)->Result<()> {
    let (stream,range,frames)=source_signal::extent(&d.inspection)?;let a=i.native.artifact.audio.as_ref().ok_or_else(||invalid("accepted audio missing"))?;
    d.inspection.require_processing().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;
    let pcm=area(root).join("selected-audio.wav");let wav=crate::audio_inspection::wav::inspect_native(&pcm)?;
    if d.schema!="aniflow.resumable-final-video-audio-qc-decoded/v1"||d.intent_sha256!=i.intent_sha256||d.inspection.source_sha256!=i.native.artifact.sha256||d.inspection.source_size_bytes!=i.native.artifact.byte_count
        ||d.inspection.selection_intent!=i.native.signal_request.selection||stream!=1||range!=a.interval||u64::from(d.channels)!=a.channels
        ||identity(&pcm,i.request.qc.maximum_pcm_bytes)?!=(d.wav_sha256.clone(),d.wav_bytes)||wav.sample_format!=NativeSampleFormat::Float32||wav.sample_rate_hz!=48_000||wav.channels!=d.channels||wav.frame_count!=frames||wav.pcm_sha256!=d.pcm_sha256
        ||crate::audio_signal::plan_read_only(&analysis(root,i),token)?!=d.plan{return Err(invalid("video QC decoded extent/precision/identity or actual native plan changed"));}Ok(())
}
fn meter_report(root:&Path,i:&Intent,d:&Decoded,m:&Meter,token:&CancellationToken)->Result<FinalAudioQcReport> {
    let accepted=crate::status_v3(&child(root))?;
    if m.schema!="aniflow.resumable-final-video-audio-qc-meter/v1"||m.intent_sha256!=i.intent_sha256||m.native_manifest_sha256!=accepted.manifest_sha256||m.signal.signal_run!="children/signal"
        ||accepted.payload.state!=crate::PipelineV3RunState::Complete||accepted.payload.delivery.is_none()||accepted.payload.plan_sha256!=d.plan.plan_sha256
        ||m.signal.temporal_sha256!=d.inspection.sha256().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))? {return Err(invalid("retained video meter lacks exact complete native child/decoded observation"));}
    let summary=source_signal::validate_retained(&i.native.signal_request,&area(root),&m.signal,token)?;super::bind(&i.native,&m.signal)?;
    let signal_receipt_sha256=local::hash(&local::bytes(&m.signal)?);
    Ok(FinalAudioQcReport {schema:"aniflow.final-audio-qc/v1".into(),intent:i.native.clone(),intent_sha256:canonical_sha256(&i.native)?,signal_receipt_sha256,signal:m.signal.clone(),accepted_pcm:summary,normalization:"none".into(),creative_approval:"not_performed; measurements are not musical quality review".into(),runtime_qualified:false,publication_authority:"none".into()})
}
/// Process-free, exact layout/current native checkpoint projection.
pub fn status(root:&Path,token:&CancellationToken)->Result<Status> {
    let i=retained(root,token)?;let a=area(root);ensure_canonical_directory(&a)?;ensure_canonical_directory(&a.join("children"))?;
    let c=present(&root.join("capture.json"))?;let d=present(&root.join("decoded.json"))?;let n=present(&child(root))?;let m=present(&root.join("meter.json"))?;let s=present(&a.join("video-audio-signal.json"))?;let q=present(&root.join("qc.json"))?;
    if !c&&(d||n||m||s||q)||!d&&(n||m||s||q)||!m&&(s||q)||!s&&q{return Err(invalid("video QC successor exists without its committed predecessor"));}
    let mut root_files=vec!["resume-intent.json"];let mut files=Vec::new();let mut phase=Phase::Capture;let mut native_plan_sha256=None;let mut native_manifest_sha256=None;let mut report=None;
    if c {let saved:Capture=local::read(&root.join("capture.json"))?;if saved!=capture_marker(&i){return Err(invalid("video QC capture binding differs"));}snapshots(root,&i)?;root_files.push("capture.json");files.extend(["source.media","ffmpeg","ffprobe","aniflow-provider"]);phase=Phase::Decode;}
    if d {
        let saved:Decoded=local::read(&root.join("decoded.json"))?;decoded(root,&i,&saved,token)?;root_files.push("decoded.json");files.push("selected-audio.wav");native_plan_sha256=Some(saved.plan.plan_sha256.clone());phase=Phase::NativeSignal;
        if n {crate::audio_inspection::require_same_plan(&child(root),&saved.plan)?;let accepted=crate::status_v3(&child(root))?;if accepted.payload.plan_sha256!=saved.plan.plan_sha256{return Err(invalid("video QC child plan/manifest differs"));}native_manifest_sha256=Some(accepted.manifest_sha256);if accepted.payload.state==crate::PipelineV3RunState::Complete&&accepted.payload.delivery.is_some(){phase=Phase::Meter;}}
        if m {if phase!=Phase::Meter{return Err(invalid("video QC meter precedes accepted native signal"));}let meter:Meter=local::read(&root.join("meter.json"))?;let expected=meter_report(root,&i,&saved,&meter,token)?;root_files.push("meter.json");
            if s {let signal:VideoAudioSignalReceipt=local::read(&a.join("video-audio-signal.json"))?;if signal!=meter.signal||identity(&a.join("video-audio-signal.json"),local::DOCUMENT_BYTES)?.0!=expected.signal_receipt_sha256{return Err(invalid("video signal publication differs from committed meter"));}files.push("video-audio-signal.json");}
            if q {let saved:FinalAudioQcReport=local::read(&root.join("qc.json"))?;if saved!=expected{return Err(invalid("final QC receipt differs from exact retained video signal"));}root_files.push("qc.json");phase=Phase::Complete;report=Some(saved);}
        }
    }
    local::entries(root,&root_files,&["signal"],true)?;local::entries(&a,&files,&["children"],false)?;local::entries(&a.join("children"),&[],&if n{vec!["signal"]}else{vec![]},false)?;
    if retained(root,token)?!=i{return Err(invalid("current final video/components changed during QC status"));}
    Ok(Status {schema:"aniflow.resumable-final-video-audio-qc-status/v1".into(),run_directory:root.to_path_buf(),intent_sha256:i.intent_sha256,phase,native_plan_sha256,native_manifest_sha256,complete:q,report})
}
pub fn observe(root:&Path,pin:&str,token:&CancellationToken)->Result<FinalAudioQcReport> {
    crate::provider::require_sha256(pin,"resumable video-QC receipt")?;let before=identity(&root.join("qc.json"),local::DOCUMENT_BYTES)?;if before.0!=pin{return Err(invalid("video QC receipt differs from external pin"));}
    let report=status(root,token)?.report.ok_or_else(||invalid("video QC is incomplete"))?;if identity(&root.join("qc.json"),local::DOCUMENT_BYTES)?!=before{return Err(invalid("video QC receipt changed during readback"));}Ok(report)
}
