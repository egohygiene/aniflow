//! Private cue-level review preparation. Notes never authorize a delivery.
use std::{collections::BTreeSet,path::Path};
use serde::{Deserialize,Serialize};
use crate::{CancellationToken,Result,timed_text::{TimedTextCue,TimedTextDocument}};
use super::{ManualLyricsRequest,ManualLyricsStatus,canonical_sha256,invalid,timed_text};

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueWorksheet {
    pub schema:String,pub worksheet_sha256:String,pub payload:CueWorksheetPayload,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueWorksheetPayload {
    pub request_sha256:String,pub input_sha256:String,pub audio_sha256:String,
    pub timeline_sha256:String,pub status:ManualLyricsStatus,
    pub audio_relative:TimedTextDocument,pub delivery_relative:Vec<TimedTextCue>,
    pub approval_authority:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueNote {
    pub cue_id:String,pub original_cue_sha256:String,
    pub words_checked:bool,pub timing_checked:bool,
    /// A correction request only; edit/import a fresh SRT and review it again.
    pub correction_needed:bool,pub note:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueNoteSummary {
    pub schema:String,pub worksheet_sha256:String,pub cue_count:u64,
    pub words_checked:u64,pub timing_checked:u64,pub corrections_needed:u64,
    pub requires_new_import:bool,pub approval_authority:String,
}
fn cancelled(token:&CancellationToken)->Result<()> {
    if token.is_cancelled(){Err(invalid("cue worksheet observation cancelled"))}else{Ok(())}
}
/// Read and recheck current manual snapshots without writes or tool execution.
/// Contains private lyrics. Do not put this result in portable telemetry.
pub fn observe(request:&ManualLyricsRequest,root:&Path,token:&CancellationToken)->Result<CueWorksheet> {
    cancelled(token)?;let status=super::status(request,root)?;
    let input=super::read_bytes(&root.join("words/artifact"),timed_text::MAX_TEXT_BYTES as u64)?;
    let document=super::decode(request,&input)?;
    if status.words.subject_sha256.as_deref()!=Some(document.source.sha256.as_str())
        ||status.timing.subject_sha256!=status.words.subject_sha256
        ||super::read_bytes(&root.join("audio-relative.json"),timed_text::MAX_EVIDENCE_BYTES)?!=document.canonical_json_bytes()? {
        return Err(invalid("cue worksheet requires matching current imported lyric snapshots"));
    }
    let delivery=super::delivery_projection(&document,request)?;
    let payload=CueWorksheetPayload {request_sha256:canonical_sha256(request)?,input_sha256:document.source.sha256.clone(),
        audio_sha256:request.audio.artifact.sha256.clone(),timeline_sha256:request.timeline_sha256.clone(),status:status.clone(),
        audio_relative:document,delivery_relative:delivery.cues,approval_authority:"none; cue notes do not replace separate artifact-bound words/timing/render decisions".into()};
    if super::status(request,root)?!=status{return Err(invalid("lyric review changed during worksheet observation"));}
    cancelled(token)?;
    Ok(CueWorksheet {schema:"aniflow.lyric-cue-worksheet/v1".into(),worksheet_sha256:canonical_sha256(&payload)?,payload})
}
fn summarize(worksheet_sha256:&str,document:&TimedTextDocument,notes:&[CueNote])->Result<CueNoteSummary> {
    document.validate()?;
    if notes.len()>document.cues.len(){return Err(invalid("cue notes exceed the selected document"));}
    let mut seen=BTreeSet::new();let mut words=0;let mut timing=0;let mut corrections=0;let mut total_bytes=0usize;
    for note in notes {
        total_bytes=total_bytes.checked_add(note.note.len()).ok_or_else(||invalid("cue note byte overflow"))?;
        if !seen.insert(note.cue_id.as_str())||note.note.len()>2048||total_bytes>65_536
            ||note.note.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\t'))
            ||note.correction_needed&&note.note.trim().is_empty(){return Err(invalid("cue notes require unique IDs, bounded text and a reason for corrections"));}
        let cue=document.cues.iter().find(|c|c.id==note.cue_id).ok_or_else(||invalid("cue note names an unknown cue"))?;
        if canonical_sha256(cue)?!=note.original_cue_sha256{return Err(invalid("cue note is stale for the exact text/timing/speaker/label"));}
        words+=u64::from(note.words_checked);timing+=u64::from(note.timing_checked);corrections+=u64::from(note.correction_needed);
    }
    Ok(CueNoteSummary {schema:"aniflow.lyric-cue-notes/v1".into(),worksheet_sha256:worksheet_sha256.into(),cue_count:document.cues.len() as u64,
        words_checked:words,timing_checked:timing,corrections_needed:corrections,requires_new_import:corrections>0,
        approval_authority:"none; supplied notes only, no human review authenticated or delivery approved".into()})
}
/// Match notes against a fresh worksheet, including source/offset/configuration
/// and review state. A stale worksheet never silently attaches to new content.
pub fn assess_notes(request:&ManualLyricsRequest,root:&Path,expected_worksheet_sha256:&str,notes:&[CueNote],token:&CancellationToken)->Result<CueNoteSummary> {
    let current=observe(request,root,token)?;
    if current.worksheet_sha256!=expected_worksheet_sha256{return Err(invalid("cue worksheet is stale; observe and review the current import"));}
    let summary=summarize(&current.worksheet_sha256,&current.payload.audio_relative,notes)?;cancelled(token)?;Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document()->TimedTextDocument {timed_text::decode("7\n00:00:00,100 --> 00:00:01,100\n大丈夫 — hey!\n".as_bytes(),timed_text::TimedTextFormat::Srt,&timed_text::ImportContext::default()).unwrap().document}
    fn note(d:&TimedTextDocument)->CueNote {CueNote {cue_id:d.cues[0].id.clone(),original_cue_sha256:canonical_sha256(&d.cues[0]).unwrap(),words_checked:true,timing_checked:false,correction_needed:false,note:String::new()}}
    #[test]
    fn words_and_timing_are_distinct_and_never_approve() {
        let d=document();let summary=summarize(&"a".repeat(64),&d,&[note(&d)]).unwrap();
        assert_eq!(summary.words_checked,1);assert_eq!(summary.timing_checked,0);assert!(summary.approval_authority.starts_with("none"));assert!(!summary.requires_new_import);
    }
    #[test]
    fn cue_hash_binds_authored_text_label_and_timing() {
        let d=document();let n=note(&d);let mut changed=d.clone();changed.cues[0].text.push('!');
        assert!(summarize(&"a".repeat(64),&changed,&[n]).is_err());assert!(d.cues[0].text.contains("大丈夫"));
    }
    #[test]
    fn duplicate_unknown_and_unbounded_notes_refuse() {
        let d=document();let n=note(&d);assert!(summarize("x",&d,&[n.clone(),n.clone()]).is_err());
        let mut unknown=n.clone();unknown.cue_id="unknown".into();assert!(summarize("x",&d,&[unknown]).is_err());
        let mut long=n;long.note="x".repeat(2049);assert!(summarize("x",&d,&[long]).is_err());
    }
    #[test]
    fn correction_is_a_new_import_request_not_an_edit_or_approval() {
        let d=document();let mut n=note(&d);n.correction_needed=true;assert!(summarize("x",&d,&[n.clone()]).is_err());
        n.note="Recheck the repeated word at this cue".into();let summary=summarize("x",&d,&[n]).unwrap();
        assert!(summary.requires_new_import);assert_eq!(summary.corrections_needed,1);assert_eq!(d.cues[0].text,"大丈夫 — hey!");
    }
    #[test]
    fn no_notes_leaves_every_cue_unchecked() {
        let summary=summarize("x",&document(),&[]).unwrap();assert_eq!(summary.cue_count,1);assert_eq!(summary.words_checked+summary.timing_checked,0);
    }
}
