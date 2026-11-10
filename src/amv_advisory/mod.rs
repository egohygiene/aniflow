//! Evidence-only optional AMV notes. No model runtime, media sample or approval.
use std::{collections::BTreeSet,path::PathBuf};
use serde::{Serialize,Deserialize};
use crate::{Result,CancellationToken};
use crate::destination::{invalid,cancelled,identifier,local};
use crate::provider::{canonical_sha256,require_sha256,decode_json};
use crate::temporal::{RationalTime,TimeRange};
use crate::final_video::{FinalVideoHandoff,FinalVideoArtifact};
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Mode {Disabled,EvidenceOnly}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {pub schema:String,pub mode:Mode,pub video_run_directory:PathBuf,pub artifact_id:String}
impl Request {
    pub fn from_json_slice(bytes:&[u8])->Result<Self>{bounded(bytes,16_384)?;let r:Self=decode_json(bytes,"AMV advisory request")?;r.validate()?;Ok(r)}
    pub fn validate(&self)->Result<()> {if self.schema!="aniflow.amv-advisory-request/v1"||!identifier(&self.artifact_id)||self.artifact_id.len()>96{return Err(invalid("closed explicit AMV advisory request required"));}Ok(())}
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Value {Count {value:u64},Geometry {width:u32,height:u32},Rational {value:RationalTime},Interval {value:TimeRange},Flag {value:bool}}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {pub fact_id:String,pub record_sha256:String,pub value:Value}
/// Portable normalized record evidence. No source locator, metadata text, lyrics,
/// raw report, media samples or provider credentials enter this packet.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {pub schema:String,pub packet_sha256:String,pub context_sha256:String,pub artifact_sha256:String,pub checkpoint_sha256:String,pub frame_count:u64,pub frame_rate:RationalTime,pub selected_final_video_interval:TimeRange,pub facts:Vec<Fact>,pub omissions:Vec<String>,pub media_samples:u64,pub full_movie_reviewed:bool}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preparation {pub schema:String,pub enabled:bool,pub packet:Option<Packet>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {pub fact_id:String,pub final_frame_ordinal:Option<u64>,pub final_pts:Option<RationalTime>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {pub text:String,pub citations:Vec<Citation>}
/// These identities are declarations on supplied untrusted candidate data, not
/// observed service execution, verified model weights or suite profile adoption.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateIdentity {pub provider_id:String,pub model_id:String,pub runtime_revision:String,pub profile_sha256:String,pub template_sha256:String,pub output_schema_sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {pub schema:String,pub packet_sha256:String,pub declared_identity:CandidateIdentity,pub observed_fact_ids:Vec<String>,pub suggestions:Vec<Note>,pub questions:Vec<Note>}
impl Candidate {pub fn from_json_slice(bytes:&[u8])->Result<Self>{bounded(bytes,65_536)?;decode_json(bytes,"supplied AMV advisory candidate")}}
impl Packet {pub fn from_json_slice(bytes:&[u8])->Result<Self>{bounded(bytes,65_536)?;let p:Self=decode_json(bytes,"AMV evidence packet")?;p.validate()?;Ok(p)}
    pub fn validate(&self)->Result<()> {for p in [&self.packet_sha256,&self.context_sha256,&self.artifact_sha256,&self.checkpoint_sha256]{require_sha256(p,"AMV packet identity")?;}
        if self.schema!="aniflow.amv-evidence-packet/v1"||self.packet_sha256!=packet_hash(self)?||self.facts.is_empty()||self.facts.len()>16||self.media_samples!=0||self.full_movie_reviewed||self.frame_count==0||self.frame_count>1_000_000||self.frame_rate.numerator()<=0||self.omissions!=omissions(){return Err(invalid("AMV packet shape/coverage/identity differs"));}
        let mut ids=BTreeSet::new();for f in &self.facts{if !identifier(&f.fact_id)||!ids.insert(&f.fact_id){return Err(invalid("AMV packet fact identifiers duplicate"));}require_sha256(&f.record_sha256,"AMV cited record")?;}Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advisory {pub schema:String,pub advisory_sha256:String,pub packet_sha256:String,pub candidate_sha256:String,pub declared_identity:CandidateIdentity,pub observed_record_facts:Vec<Fact>,pub suggestions:Vec<Note>,pub questions:Vec<Note>,pub omissions:Vec<String>,pub semantic_grounding:String,pub input_kind:String,pub media_samples:u64,pub full_movie_reviewed:bool,pub technical_acceptance_authority:bool,pub creative_approval:String,pub publication_authority:String,pub provider_execution_observed:bool}
fn bounded(bytes:&[u8],limit:usize)->Result<()> {if bytes.len()>limit{return Err(invalid("AMV advisory document exceeds explicit byte bound"));}Ok(())}
fn omissions()->Vec<String>{["no_video_frames_or_audio_inspected","no_lyrics_or_metadata_text_shared","no_visual_quality_or_watermark_review","no_original_source_frame_mapping_inferred","no_runtime_model_execution_observed","no_human_approval_or_publication"].into_iter().map(String::from).collect()}
fn packet_hash(p:&Packet)->Result<String>{let mut copy=p.clone();copy.packet_sha256.clear();canonical_sha256(&copy)}
fn project(source:&FinalVideoHandoff,artifact:&FinalVideoArtifact)->Result<Packet>{
    if artifact.frame_count==0||artifact.frame_count>1_000_000||artifact.frame_rate.numerator()<=0{return Err(invalid("AMV record clock outside bounded evidence profile"));}
    let report=&artifact.private_report_sha256;let mut facts=vec![Fact {fact_id:"video_geometry".into(),record_sha256:report.clone(),value:Value::Geometry {width:artifact.width,height:artifact.height}},Fact {fact_id:"video_frames".into(),record_sha256:report.clone(),value:Value::Count {value:artifact.frame_count}},Fact {fact_id:"video_rate".into(),record_sha256:report.clone(),value:Value::Rational {value:artifact.frame_rate}},Fact {fact_id:"video_interval".into(),record_sha256:report.clone(),value:Value::Interval {value:artifact.video_interval}},Fact {fact_id:"artifact_bytes".into(),record_sha256:source.run_manifest_sha256.clone(),value:Value::Count {value:artifact.byte_count}},Fact {fact_id:"audio_present".into(),record_sha256:report.clone(),value:Value::Flag {value:artifact.audio.is_some()}}];
    if let Some(audio)=&artifact.audio{facts.push(Fact {fact_id:"audio_interval".into(),record_sha256:report.clone(),value:Value::Interval {value:audio.interval}});facts.push(Fact {fact_id:"audio_channels".into(),record_sha256:report.clone(),value:Value::Count {value:audio.channels}});facts.push(Fact {fact_id:"audio_sample_rate".into(),record_sha256:report.clone(),value:Value::Count {value:u64::from(audio.sample_rate_hz)}});}
    let mut p=Packet {schema:"aniflow.amv-evidence-packet/v1".into(),packet_sha256:String::new(),context_sha256:canonical_sha256(source)?,artifact_sha256:artifact.sha256.clone(),checkpoint_sha256:artifact.stage_checkpoint_sha256.clone(),frame_count:artifact.frame_count,frame_rate:artifact.frame_rate,selected_final_video_interval:artifact.video_interval,facts,omissions:omissions(),media_samples:0,full_movie_reviewed:false};p.packet_sha256=packet_hash(&p)?;p.validate()?;Ok(p)
}
/// Existing final-video owner rechecks current actual native acceptance. This
/// surface projects records only and starts no media/model operation.
pub fn prepare(request:&Request,token:&CancellationToken)->Result<Preparation>{request.validate()?;if request.mode==Mode::Disabled{return Ok(Preparation {schema:"aniflow.amv-advisory-preparation/v1".into(),enabled:false,packet:None});}cancelled(token)?;
    let before=crate::final_video::observe(&request.video_run_directory)?;let artifacts=before.artifacts.iter().filter(|a|a.artifact_id==request.artifact_id).collect::<Vec<_>>();if artifacts.len()!=1{return Err(invalid("select one exact current final-video artifact"));}let p=project(&before,artifacts[0])?;
    cancelled(token)?;if crate::final_video::observe(&request.video_run_directory)?!=before{return Err(invalid("AMV evidence changed during packet projection"));}local::bytes(&p)?;Ok(Preparation {schema:"aniflow.amv-advisory-preparation/v1".into(),enabled:true,packet:Some(p)})
}
fn validate_note(packet:&Packet,note:&Note)->Result<()> {
    if note.text.is_empty()||note.text.len()>1024||note.text.chars().any(char::is_control)||note.citations.is_empty()||note.citations.len()>8{return Err(invalid("AMV note requires bounded plain-text data and citations"));}let mut seen=BTreeSet::new();
    for c in &note.citations {if !packet.facts.iter().any(|f|f.fact_id==c.fact_id)||!seen.insert((&c.fact_id,c.final_frame_ordinal)){return Err(invalid("AMV note cites absent/duplicate evidence"));}
        match (c.final_frame_ordinal,c.final_pts){(None,None)=>{},(Some(n),Some(pts)) if n<packet.frame_count=>{let expected=packet.frame_rate.reciprocal().and_then(|p|p.checked_mul(n)).and_then(|d|packet.selected_final_video_interval.start.checked_add(d)).map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?;if pts!=expected||pts>=packet.selected_final_video_interval.end{return Err(invalid("AMV citation changed exact final-video PTS"));}},_=>return Err(invalid("AMV frame/time citation must bind both exact values")),}
    }Ok(())
}
fn validate_supplied(packet:&Packet,candidate:&Candidate)->Result<Advisory> {
    packet.validate()?;if candidate.schema!="aniflow.amv-advisory-candidate/v1"||candidate.packet_sha256!=packet.packet_sha256||candidate.observed_fact_ids.len()>16||candidate.suggestions.len()>16||candidate.questions.len()>16{return Err(invalid("AMV candidate identity or item bound differs"));}
    let i=&candidate.declared_identity;for s in [&i.provider_id,&i.model_id,&i.runtime_revision]{if s.is_empty()||s.len()>128||!s.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-:/+".contains(&c)){return Err(invalid("bounded declared AMV provider/model identity required"));}}for p in [&i.profile_sha256,&i.template_sha256,&i.output_schema_sha256]{require_sha256(p,"AMV declared model/template/schema identity")?;}
    if i.output_schema_sha256!=local::hash(include_bytes!("../../docs/contracts/amv-advisory-candidate-v1.schema.json")){return Err(invalid("AMV candidate declares a different exact output contract"));}
    let mut selected=BTreeSet::new();let mut facts=Vec::new();for id in &candidate.observed_fact_ids {if !selected.insert(id){return Err(invalid("duplicate AMV observation reference"));}let f=packet.facts.iter().find(|f|&f.fact_id==id).ok_or_else(||invalid("unknown AMV observation reference"))?;facts.push(f.clone());}
    for n in candidate.suggestions.iter().chain(&candidate.questions){validate_note(packet,n)?;}
    let mut result=Advisory {schema:"aniflow.amv-advisory/v1".into(),advisory_sha256:String::new(),packet_sha256:packet.packet_sha256.clone(),candidate_sha256:canonical_sha256(candidate)?,declared_identity:i.clone(),observed_record_facts:facts,suggestions:candidate.suggestions.clone(),questions:candidate.questions.clone(),omissions:packet.omissions.clone(),semantic_grounding:"citation_membership_and_exact_fact_projection_only; suggestion_meaning_unverified".into(),input_kind:"supplied_untrusted_candidate; no_provider_invocation".into(),media_samples:0,full_movie_reviewed:false,technical_acceptance_authority:false,creative_approval:"not_performed".into(),publication_authority:"none".into(),provider_execution_observed:false};result.advisory_sha256=canonical_sha256(&result)?;local::bytes(&result)?;Ok(result)
}
/// Current owner binding around validation. A self-consistent saved packet is
/// never enough to establish source freshness or native acceptance.
pub fn validate_candidate(request:&Request,packet:&Packet,candidate:&Candidate,token:&CancellationToken)->Result<Advisory>{
    if request.mode!=Mode::EvidenceOnly{return Err(invalid("AMV advisory is disabled"));}let before=prepare(request,token)?.packet.ok_or_else(||invalid("AMV evidence unavailable"))?;if before!=*packet{return Err(invalid("AMV packet is stale or belongs to another current acceptance"));}let report=validate_supplied(packet,candidate)?;cancelled(token)?;if prepare(request,token)?.packet.as_ref()!=Some(packet){return Err(invalid("AMV acceptance changed during notes validation"));}Ok(report)
}
#[cfg(test)] mod tests;
