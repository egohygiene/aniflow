//! Manual timed-lyrics import/review/sidecar composition over #47 and #107.
//! No recognition, model selection, automatic alignment, muxing or burn-in.
use std::fs::{self,OpenOptions};
use std::io::{Read,Write};
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use crate::audio_analysis::{AudioSource,AudioRationalTime,AudioArtifactReference,AudioReviewedAuthority};
use crate::frame_io::{ensure_canonical_directory,identity,require_absent};
use crate::provider::{canonical_json_bytes,canonical_sha256,decode_json,require_sha256};
use crate::temporal::{RationalTime,TimeRange};
use crate::timed_text::{self,TimedTextFormat,TimedTextDocument,ImportContext,TextProvenance,OverlapPolicy,CueTiming,ConversionOptions,ConversionLoss,CarrierOmission};
use crate::{CancellationToken,Error,ErrorCategory,Result};
use super::manual::{self,ManualContext,ManualRole,ManualState,ManualStatus,ManualReview};

/// Separate operator decisions about authored words and supplied intervals.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum LyricsReviewScope {Words,Timing}
impl LyricsReviewScope {
    fn directory(self)->&'static str {match self {Self::Words=>"words",Self::Timing=>"timing"}}
}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualLyricsRequest {
    pub schema:String,pub audio_file:PathBuf,pub audio:AudioSource,
    /// External timeline/recipe identities are caller-supplied bindings, not
    /// proof that this PCM originated in a particular compressed video.
    pub timeline_sha256:String,pub recipe_intent_sha256:String,
    pub offset:RationalTime,pub delivery_extent:TimeRange,
    pub input_format:TimedTextFormat,pub language:Option<String>,pub overlap_policy:OverlapPolicy,
    pub targets:Vec<TimedTextFormat>,pub conversion:ConversionOptions,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualLyricsStatus {
    pub schema:String,pub request_sha256:String,pub words:ManualStatus,pub timing:ManualStatus,
    pub ready_for_sidecar_export:bool,pub render_review:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LyricsExportFile {
    pub format:TimedTextFormat,pub filename:String,pub sha256:String,pub byte_count:u64,
    pub losses:Vec<ConversionLoss>,pub carrier_omissions:Vec<CarrierOmission>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualLyricsDelivery {
    pub schema:String,pub request_sha256:String,pub input_sha256:String,pub audio:AudioSource,
    pub timeline_sha256:String,pub offset:RationalTime,pub delivery_extent:TimeRange,
    pub words_review_sha256:String,pub timing_review_sha256:String,pub exports:Vec<LyricsExportFile>,
    pub render_review:String,pub runtime_qualified:bool,
}
fn invalid(message:&str)->Error {Error::new(ErrorCategory::State,message)}
fn mapped(e:anyhow::Error)->Error {Error::from_anyhow(ErrorCategory::Media,e)}
fn sha(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn read_bytes(path:&Path,maximum:u64)->Result<Vec<u8>> {
    let expected=identity(path,maximum)?;let mut bytes=Vec::new();
    crate::audio_inspection::process::open_regular(path).map_err(|_|invalid("lyrics input unavailable"))?.take(maximum+1).read_to_end(&mut bytes).map_err(|_|invalid("lyrics input unreadable"))?;
    if bytes.len() as u64!=expected.1||sha(&bytes)!=expected.0||identity(path,maximum)?!=expected{return Err(invalid("lyrics input changed during read"));}Ok(bytes)
}
fn write(root:&Path,name:&str,bytes:&[u8])->Result<()> {
    let mut file=OpenOptions::new().write(true).create_new(true).open(root.join(name)).map_err(|_|invalid("lyrics output exists or cannot be reserved"))?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|_|invalid("lyrics output could not be synced"))?;Ok(())
}
fn context(request:&ManualLyricsRequest,scope:&str)->Result<ManualContext> {
    Ok(ManualContext {schema:"aniflow.manual-context/v1".into(),role:ManualRole::TimedLyrics,
        source_sha256:request.audio.artifact.sha256.clone(),timeline_sha256:request.timeline_sha256.clone(),
        configuration_sha256:canonical_sha256(&("aniflow.manual-lyrics-review/v1",scope,request))?,recipe_intent_sha256:request.recipe_intent_sha256.clone()})
}
fn validate(request:&ManualLyricsRequest)->Result<()> {
    request.audio.validate()?;request.conversion.validate()?;request.offset.validate().map_err(mapped)?;
    TimeRange::new(request.delivery_extent.start,request.delivery_extent.end).map_err(mapped)?;
    require_sha256(&request.timeline_sha256,"lyrics timeline")?;require_sha256(&request.recipe_intent_sha256,"lyrics recipe")?;
    if request.schema!="aniflow.manual-lyrics-request/v1"||!matches!(request.input_format,TimedTextFormat::Srt|TimedTextFormat::Webvtt|TimedTextFormat::Ttml)
        ||request.audio.stem.is_some()||request.targets.is_empty()||request.targets.len()>6||request.offset.checked_mul(1000).map_err(mapped)?.denominator()!=1
        ||request.delivery_extent.start<RationalTime::ZERO{return Err(invalid("manual lyrics require explicit interval text, original zero-origin audio, exact millisecond offset and a nonnegative delivery extent"));}
    for (index,target) in request.targets.iter().enumerate(){if request.targets[..index].contains(target){return Err(invalid("duplicate lyric export format"));}}
    if identity(&request.audio_file,crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES)?!=(request.audio.artifact.sha256.clone(),request.audio.artifact.byte_size){return Err(invalid("lyrics audio bytes differ from their exact binding"));}
    let wave=crate::audio_inspection::wav::inspect_native(&request.audio_file)?;
    if wave.sample_rate_hz!=request.audio.sample_rate_hz||wave.channels!=request.audio.channels||wave.frame_count!=request.audio.frame_count{return Err(invalid("lyrics audio clock differs from independently parsed PCM"));}
    ImportContext {audio_source:Some(request.audio.clone()),language:request.language.clone(),overlap_policy:request.overlap_policy,..ImportContext::default()}.validate()
}
fn saved(root:&Path,request:&ManualLyricsRequest)->Result<()> {
    ensure_canonical_directory(root)?;
    let old:ManualLyricsRequest=decode_json(&read_bytes(&root.join("request.json"),65_536)?,"manual lyrics request")?;
    if old!=*request{return Err(invalid("lyrics source, audio edit, clock/offset, language, delivery or requested formats changed; use a new workflow and review"));}Ok(())
}
fn decode(request:&ManualLyricsRequest,bytes:&[u8])->Result<TimedTextDocument> {
    let import=ImportContext {audio_source:Some(request.audio.clone()),language:request.language.clone(),overlap_policy:request.overlap_policy,..ImportContext::default()};
    let doc=timed_text::decode(bytes,request.input_format,&import)?.document;
    if doc.cues.iter().any(|c|!matches!(c.timing,CueTiming::Interval {..})){return Err(invalid("manual lyrics require positive start/end intervals; no end times are inferred"));}
    delivery_projection(&doc,request)?;Ok(doc)
}
fn delivery_projection(input:&TimedTextDocument,request:&ManualLyricsRequest)->Result<TimedTextDocument> {
    let mut output=input.clone();
    // Shifted delivery coordinates are not the audio v1 zero-origin clock. The
    // external workflow receipt retains their exact audio/offset relationship.
    output.audio_source=None;
    let shift=|time:AudioRationalTime|->Result<AudioRationalTime> {
        let time=RationalTime::new(time.numerator,u64::from(time.denominator)).map_err(mapped)?.checked_add(request.offset).map_err(mapped)?;
        if time<request.delivery_extent.start||time>request.delivery_extent.end{return Err(invalid("lyric cue is outside the explicitly selected delivery extent; no implicit trim/pad"));}
        Ok(AudioRationalTime {numerator:time.numerator(),denominator:u32::try_from(time.denominator()).map_err(|_|invalid("lyric timestamp denominator overflow"))?})
    };
    for cue in &mut output.cues {if let CueTiming::Interval {start,end}=cue.timing {cue.timing=CueTiming::Interval {start:shift(start)?,end:shift(end)?};}}
    output.validate()?;Ok(output)
}

/// Create a manual work area; omit input while authoring an SRT externally.
pub fn initialize(request:&ManualLyricsRequest,root:&Path,input:Option<&Path>,token:&CancellationToken)->Result<ManualLyricsStatus> {
    if token.is_cancelled(){return Err(invalid("manual lyric initialization cancelled"));}
    validate(request)?;ensure_canonical_directory(root.parent().ok_or_else(||invalid("lyrics root needs an existing parent"))?)?;require_absent(root)?;
    if request.audio_file.starts_with(root)||input.is_some_and(|p|p.starts_with(root)){return Err(invalid("lyrics work area must be separate from original inputs"));}
    let bytes=canonical_json_bytes(request)?;if bytes.len()>65_536{return Err(invalid("lyrics request exceeds 64 KiB"));}
    fs::create_dir(root).map_err(|_|invalid("cannot reserve lyrics work area"))?;
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;fs::set_permissions(root,fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot make lyrics work area private"))?;}
    ensure_canonical_directory(root)?;write(root,"request.json",&bytes)?;
    manual::create(&root.join("words"),&context(request,"words")?)?;manual::create(&root.join("timing"),&context(request,"timing")?)?;
    if let Some(input)=input {import(request,root,input,token)} else {status(request,root)}
}
/// Parse only explicitly supplied intervals using #47; import supplies no review.
pub fn import(request:&ManualLyricsRequest,root:&Path,input:&Path,token:&CancellationToken)->Result<ManualLyricsStatus> {
    if token.is_cancelled(){return Err(invalid("manual lyric import cancelled"));}
    validate(request)?;saved(root,request)?;
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    let bytes=read_bytes(input,timed_text::MAX_TEXT_BYTES as u64)?;let doc=decode(request,&bytes)?;
    for scope in ["words","timing"] {
        let checkpoint=root.join(scope);let context=context(request,scope)?;let current=manual::status(&checkpoint,&context)?;
        if current.state==ManualState::AwaitingInput {manual::attach_file(&checkpoint,&context,input,token)?;}
        else if current.state==ManualState::Invalidated||current.subject_sha256.as_deref()!=Some(doc.source.sha256.as_str()){return Err(invalid("manual lyric edit changed attached content; keep prior review and start a new workflow"));}
    }
    let normalized=doc.canonical_json_bytes()?;let path=root.join("audio-relative.json");
    match fs::symlink_metadata(&path) {Ok(_)=>{if read_bytes(&path,timed_text::MAX_EVIDENCE_BYTES)?!=normalized{return Err(invalid("normalized lyric document changed"));}},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>write(root,"audio-relative.json",&normalized)?,Err(_)=>return Err(invalid("lyrics snapshot unavailable"))}
    validate(request)?;status(request,root)
}
/// Retain one exact supplied decision; never generate a reviewer or approval.
/// Rechecks the saved request and original audio before the manual checkpoint.
pub fn review(request:&ManualLyricsRequest,root:&Path,scope:LyricsReviewScope,declaration:&ManualReview)->Result<ManualLyricsStatus> {
    validate(request)?;saved(root,request)?;
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    let scope=scope.directory();
    manual::review(&root.join(scope),&context(request,scope)?,declaration)?;
    status(request,root)
}
pub fn status(request:&ManualLyricsRequest,root:&Path)->Result<ManualLyricsStatus> {
    validate(request)?;saved(root,request)?;
    let words=manual::status(&root.join("words"),&context(request,"words")?)?;
    let timing=manual::status(&root.join("timing"),&context(request,"timing")?)?;
    let normalized=(||->Result<bool>{
        let input=read_bytes(&root.join("words/artifact"),timed_text::MAX_TEXT_BYTES as u64)?;
        let document=decode(request,&input)?;
        Ok(Some(document.source.sha256.as_str())==words.subject_sha256.as_deref()
            &&read_bytes(&root.join("audio-relative.json"),timed_text::MAX_EVIDENCE_BYTES)?==document.canonical_json_bytes()?)
    })().unwrap_or(false);
    let ready=normalized&&words.state==ManualState::Approved&&timing.state==ManualState::Approved&&words.subject_sha256==timing.subject_sha256;
    Ok(ManualLyricsStatus {schema:"aniflow.manual-lyrics-status/v1".into(),request_sha256:canonical_sha256(request)?,words,timing,
        ready_for_sidecar_export:ready,render_review:"not_performed; sidecar export does not approve mux/burn-in appearance".into()})
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {root:tempfile::TempDir,request:ManualLyricsRequest,input:PathBuf,work:PathBuf}
    fn fixture()->Fixture {
        let root=tempfile::tempdir().unwrap();let base=root.path().canonicalize().unwrap();
        let mut wav=b"RIFF".to_vec();let count=96_000u32;
        wav.extend((36+count*2).to_le_bytes());wav.extend(b"WAVEfmt ");wav.extend(16u32.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(1u16.to_le_bytes());
        wav.extend(48_000u32.to_le_bytes());wav.extend(96_000u32.to_le_bytes());wav.extend(2u16.to_le_bytes());wav.extend(16u16.to_le_bytes());wav.extend(b"data");wav.extend((count*2).to_le_bytes());wav.resize(44+count as usize*2,0);
        let audio_file=base.join("original mix.wav");fs::write(&audio_file,&wav).unwrap();
        let audio=AudioSource {artifact:AudioArtifactReference {id:"source_audio".into(),sha256:sha(&wav),byte_size:wav.len() as u64},stream_index:0,sample_rate_hz:48_000,channels:1,frame_count:u64::from(count),origin:AudioRationalTime {numerator:0,denominator:1},stem:None};
        let input=base.join("artist lyrics.srt");fs::write(&input,"1\n00:00:00,100 --> 00:00:01,200\n大丈夫 — hey!\n").unwrap();
        let request=ManualLyricsRequest {schema:"aniflow.manual-lyrics-request/v1".into(),audio_file,audio,timeline_sha256:"a".repeat(64),recipe_intent_sha256:"b".repeat(64),
            offset:RationalTime::new(1,2).unwrap(),delivery_extent:TimeRange::new(RationalTime::ZERO,RationalTime::new(5,2).unwrap()).unwrap(),input_format:TimedTextFormat::Srt,
            language:None,overlap_policy:OverlapPolicy::Reject,targets:vec![TimedTextFormat::Srt,TimedTextFormat::Json],conversion:ConversionOptions::default()};
        Fixture {root,request,input,work:base.join("work")}
    }
    fn approve(f:&Fixture,scope:&str) {
        let context=context(&f.request,scope).unwrap();let path=f.work.join(scope);let pending=manual::status(&path,&context).unwrap();
        manual::review(&path,&context,&manual::ManualReview {schema:"aniflow.manual-review/v1".into(),context_sha256:pending.context_sha256,
            subject_sha256:pending.subject_sha256.unwrap(),decision:manual::ReviewDecision::Approve,reviewer:"synthetic-reviewer".into(),notes:"Synthetic declaration only".into()}).unwrap();
    }
    #[test]
    fn words_and_timing_are_separate_and_selected_export_preserves_authored_text() {
        let f=fixture();let before=read_bytes(&f.input,1024).unwrap();
        let initial=initialize(&f.request,&f.work,None,&CancellationToken::default()).unwrap();assert_eq!(initial.words.state,ManualState::AwaitingInput);
        import(&f.request,&f.work,&f.input,&CancellationToken::default()).unwrap();approve(&f,"words");
        let output=f.root.path().canonicalize().unwrap().join("delivery");assert!(export(&f.request,&f.work,&output).is_err());assert!(!output.exists());
        approve(&f,"timing");let receipt=export(&f.request,&f.work,&output).unwrap();assert_eq!(receipt.exports.len(),2);
        let text=String::from_utf8(fs::read(output.join("lyrics.srt")).unwrap()).unwrap();assert!(text.contains("大丈夫 — hey!"));assert!(text.contains("00:00:00,600 --> 00:00:01,700"));
        assert_eq!(read_bytes(&f.input,1024).unwrap(),before);assert!(receipt.render_review.contains("not_performed"));
    }
    #[test]
    fn scoped_review_requires_exact_declarations_and_retains_rejection() {
        let f=fixture();let token=CancellationToken::default();
        let pending=initialize(&f.request,&f.work,Some(&f.input),&token).unwrap();
        let declaration=ManualReview {schema:"aniflow.manual-review/v1".into(),context_sha256:pending.words.context_sha256,
            subject_sha256:pending.words.subject_sha256.unwrap(),decision:manual::ReviewDecision::Approve,
            reviewer:"synthetic operator".into(),notes:"Fixture only".into()};
        assert!(review(&f.request,&f.work,LyricsReviewScope::Timing,&declaration).is_err());
        assert!(!f.work.join("timing/review.json").exists());
        let words=review(&f.request,&f.work,LyricsReviewScope::Words,&declaration).unwrap();
        assert_eq!(words.words.state,ManualState::Approved);assert!(!words.ready_for_sidecar_export);
        assert!(review(&f.request,&f.work,LyricsReviewScope::Words,&declaration).is_err());
        let rejection=ManualReview {context_sha256:words.timing.context_sha256,decision:manual::ReviewDecision::Reject,..declaration};
        let rejected=review(&f.request,&f.work,LyricsReviewScope::Timing,&rejection).unwrap();
        assert_eq!(rejected.timing.state,ManualState::Rejected);assert!(!rejected.ready_for_sidecar_export);
        let output=f.root.path().canonicalize().unwrap().join("rejected delivery");
        assert!(export(&f.request,&f.work,&output).is_err());assert!(!output.exists());
    }
    #[test]
    fn review_rechecks_request_and_original_audio_before_saving_decision() {
        let f=fixture();let pending=initialize(&f.request,&f.work,Some(&f.input),&CancellationToken::default()).unwrap();
        let declaration=ManualReview {schema:"aniflow.manual-review/v1".into(),context_sha256:pending.words.context_sha256,
            subject_sha256:pending.words.subject_sha256.unwrap(),decision:manual::ReviewDecision::Approve,
            reviewer:"synthetic operator".into(),notes:String::new()};
        let mut changed=f.request.clone();changed.offset=RationalTime::ZERO;
        assert!(review(&changed,&f.work,LyricsReviewScope::Words,&declaration).is_err());
        assert!(!f.work.join("words/review.json").exists());
        let mut audio=fs::read(&f.request.audio_file).unwrap();*audio.last_mut().unwrap()=1;fs::write(&f.request.audio_file,audio).unwrap();
        assert!(review(&f.request,&f.work,LyricsReviewScope::Words,&declaration).is_err());
        assert!(!f.work.join("words/review.json").exists());
    }
    #[test]
    fn changed_audio_offset_and_manual_edits_cannot_reuse_review() {
        let f=fixture();initialize(&f.request,&f.work,Some(&f.input),&CancellationToken::default()).unwrap();approve(&f,"words");approve(&f,"timing");
        let mut shifted=f.request.clone();shifted.offset=RationalTime::new(1,1).unwrap();assert!(status(&shifted,&f.work).is_err());
        fs::write(&f.input,b"manually changed text and timing").unwrap();assert!(!status(&f.request,&f.work).unwrap().ready_for_sidecar_export);
        let output=f.root.path().canonicalize().unwrap().join("delivery");assert!(export(&f.request,&f.work,&output).is_err());
        let mut audio=fs::read(&f.request.audio_file).unwrap();*audio.last_mut().unwrap()=1;fs::write(&f.request.audio_file,audio).unwrap();assert!(status(&f.request,&f.work).is_err());
    }
    #[test]
    fn invalid_or_outside_extent_timing_is_not_attached_as_an_approved_candidate() {
        let f=fixture();initialize(&f.request,&f.work,None,&CancellationToken::default()).unwrap();
        fs::write(&f.input,b"1\n00:00:01,900 --> 00:00:03,000\noutside original audio\n").unwrap();
        assert!(import(&f.request,&f.work,&f.input,&CancellationToken::default()).is_err());
        assert_eq!(manual::status(&f.work.join("words"),&context(&f.request,"words").unwrap()).unwrap().state,ManualState::AwaitingInput);
        let mut invalid=f.request.clone();invalid.offset=RationalTime::new(1,48_000).unwrap();assert!(validate(&invalid).is_err());
    }
    #[test]
    fn end_time_loss_is_explicit_and_word_decision_cannot_substitute_for_timing() {
        let mut f=fixture();f.request.targets=vec![TimedTextFormat::Lrc];initialize(&f.request,&f.work,Some(&f.input),&CancellationToken::default()).unwrap();
        let words=manual::status(&f.work.join("words"),&context(&f.request,"words").unwrap()).unwrap();
        let wrong=manual::ManualReview {schema:"aniflow.manual-review/v1".into(),context_sha256:words.context_sha256,subject_sha256:words.subject_sha256.unwrap(),decision:manual::ReviewDecision::Approve,reviewer:"synthetic-reviewer".into(),notes:String::new()};
        assert!(manual::review(&f.work.join("timing"),&context(&f.request,"timing").unwrap(),&wrong).is_err());
        approve(&f,"words");approve(&f,"timing");let output=f.root.path().canonicalize().unwrap().join("lossy");assert!(export(&f.request,&f.work,&output).is_err());assert!(!output.exists());
    }
    #[test]
    fn recipe_attachment_rechecks_exported_sidecar_bytes() {
        let f=fixture();initialize(&f.request,&f.work,Some(&f.input),&CancellationToken::default()).unwrap();approve(&f,"words");approve(&f,"timing");
        let output=f.root.path().canonicalize().unwrap().join("recipe-sidecars");let delivery=export(&f.request,&f.work,&output).unwrap();
        super::super::recipe_lyrics::check_files(&output,&delivery).unwrap();
        fs::write(output.join("lyrics.srt"),b"different words and timing").unwrap();
        assert!(super::super::recipe_lyrics::check_files(&output,&delivery).is_err());
    }
    #[test]
    fn recipe_attachment_refuses_undeclared_sidecar_members() {
        let f=fixture();initialize(&f.request,&f.work,Some(&f.input),&CancellationToken::default()).unwrap();approve(&f,"words");approve(&f,"timing");
        let output=f.root.path().canonicalize().unwrap().join("recipe-sidecars");let delivery=export(&f.request,&f.work,&output).unwrap();
        fs::write(output.join("extra.srt"),b"undeclared").unwrap();
        assert!(super::super::recipe_lyrics::check_files(&output,&delivery).is_err());
    }
}
/// Publish selected sidecars only after separate exact words and timing review.
pub fn export(request:&ManualLyricsRequest,root:&Path,output:&Path)->Result<ManualLyricsDelivery> {
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;
    let before=status(request,root)?;
    if !before.ready_for_sidecar_export{return Err(invalid("lyrics await separate exact words and timing review"));}
    let original=read_bytes(&root.join("words/artifact"),timed_text::MAX_TEXT_BYTES as u64)?;
    let mut source=decode(request,&original)?;
    if Some(source.source.sha256.as_str())!=before.words.subject_sha256.as_deref()
        ||read_bytes(&root.join("audio-relative.json"),timed_text::MAX_EVIDENCE_BYTES)?!=source.canonical_json_bytes()?{return Err(invalid("lyric source, normalized snapshot or review subject changed"));}
    let words_review=read_bytes(&root.join("words/review.json"),65_536)?;
    let timing_review=read_bytes(&root.join("timing/review.json"),65_536)?;
    let declaration:manual::ManualReview=decode_json(&words_review,"manual words review")?;
    source.provenance=TextProvenance::ReviewedLyrics {authority:AudioReviewedAuthority {supplied_by:declaration.reviewer,provenance_artifact_id:"manual_words_review".into()},
        evidence:AudioArtifactReference {id:"manual_words_review".into(),sha256:sha(&words_review),byte_size:words_review.len() as u64},source_sha256:source.source.sha256.clone()};
    source.validate()?;let delivery=delivery_projection(&source,request)?;
    let mut prepared=Vec::new();let mut exports=Vec::new();
    for format in &request.targets {
        let encoded=timed_text::encode(&delivery,*format,&request.conversion).map_err(|e|e.error)?;
        let filename=format!("lyrics.{}",format.extension());
        exports.push(LyricsExportFile {format:*format,filename:filename.clone(),sha256:sha(&encoded.bytes),byte_count:encoded.bytes.len() as u64,losses:encoded.losses,carrier_omissions:encoded.carrier_omissions});
        prepared.push((filename,encoded.bytes));
    }
    ensure_canonical_directory(output.parent().ok_or_else(||invalid("lyrics export needs an existing parent"))?)?;require_absent(output)?;
    if output.starts_with(root)||root.starts_with(output)||request.audio_file.starts_with(output){return Err(invalid("lyrics delivery must be separate from workflow/source"));}
    if status(request,root)?!=before||read_bytes(&root.join("words/review.json"),65_536)?!=words_review||read_bytes(&root.join("timing/review.json"),65_536)?!=timing_review{return Err(invalid("lyrics review changed before export"));}
    fs::create_dir(output).map_err(|_|invalid("cannot reserve lyrics delivery"))?;
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;fs::set_permissions(output,fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot make lyric delivery private"))?;}
    for (name,bytes) in prepared {write(output,&name,&bytes)?;}
    let receipt=ManualLyricsDelivery {schema:"aniflow.manual-lyrics-delivery/v1".into(),request_sha256:before.request_sha256.clone(),input_sha256:source.source.sha256,
        audio:request.audio.clone(),timeline_sha256:request.timeline_sha256.clone(),offset:request.offset,delivery_extent:request.delivery_extent,
        words_review_sha256:sha(&words_review),timing_review_sha256:sha(&timing_review),exports,
        render_review:"not_performed; external audio/timeline binding retained here because delivery coordinates differ from audio v1; no automatic recognition, mux, burn-in or publishing".into(),runtime_qualified:false};
    if status(request,root)?!=before||read_bytes(&root.join("words/review.json"),65_536)?!=words_review
        ||read_bytes(&root.join("timing/review.json"),65_536)?!=timing_review{return Err(invalid("lyrics or exact audio/timeline/review changed during sidecar publication; partial files are unaccepted"));}
    write(output,"lyrics-delivery.json",&canonical_json_bytes(&receipt)?)?;
    fs::File::open(output).and_then(|f|f.sync_all()).map_err(|_|invalid("lyrics delivery directory sync failed"))?;Ok(receipt)
}
