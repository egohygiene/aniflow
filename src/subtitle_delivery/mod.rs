//! Explicit reviewed-SRT preview and selected local caption candidates.
mod types;
mod runtime;
mod provider;
pub use types::*;
pub use runtime::{preview_plan,preview_run,preview_resume,preview_status,delivery_plan,delivery_run,delivery_resume,delivery_report};
pub(crate) use provider::execute_invocation;
use std::path::Path;
use crate::{Result,CancellationToken};
use crate::destination::{invalid,cancelled,local};
use crate::provider::{canonical_json_bytes,canonical_sha256};
use crate::frame_io::{identity,ensure_canonical_directory};
use crate::music_video_recipe::{lyrics,manual::{ManualReview,ReviewDecision}};
use crate::temporal::RationalTime;
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.caption-render";
const SCHEMA:&str="aniflow.caption-operation/v1";
const FONT_BYTES:u64=16_777_216;
const PREVIEW_SCOPE:&str="selected_frames_only; soft_mux_player_typography_not_qualified; full_delivery_review_remains_separate";
const DELIVERY_SCOPE:&str="supplied_preview_declaration_authorizes_candidate_generation_only; full_delivery_creative_review_not_performed";
fn mapped(error:anyhow::Error)->crate::Error {crate::Error::from_anyhow(crate::ErrorCategory::Media,error)}
fn hex(value:&str)->bool {value.len()==6&&value.bytes().all(|b|b.is_ascii_hexdigit())}
impl CaptionConfiguration {
    pub fn validate(&self)->Result<()> {
        self.settings.validate()?;crate::provider::require_sha256(&self.font.sha256,"caption font")?;
        if self.schema!="aniflow.caption-configuration/v1"||self.settings.delivery_metadata.is_some()
            ||!(8..=128).contains(&self.style.font_size)||self.style.bottom_margin>4096||!hex(&self.style.foreground)||!hex(&self.style.box_color)
            ||self.preview_cue_indices.len()>8||self.preview_cue_indices.iter().enumerate().any(|(i,n)|self.preview_cue_indices[..i].contains(n)) {
            return Err(invalid("caption configuration needs finite plain-text style, explicit font and existing native settings; metadata remux remains separately selected"));
        }Ok(())
    }
}
impl CaptionRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("caption request exceeds 64 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"caption request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        self.configuration.validate()?;
        if self.schema!="aniflow.caption-request/v1"||self.artifact_id.is_empty()||self.artifact_id.len()>64{return Err(invalid("caption request needs its exact selected video artifact"));}Ok(())
    }
}
fn frame_at(time:crate::audio_analysis::AudioRationalTime,rate:RationalTime)->Result<u64> {
    rate.validate().map_err(mapped)?;
    if time.numerator<0||time.denominator==0||rate.numerator()<=0{return Err(invalid("caption time must be a nonnegative exact interval"));}
    let top=(time.numerator as u128).checked_mul(rate.numerator() as u128).ok_or_else(||invalid("caption frame clock overflow"))?;
    let bottom=u128::from(time.denominator).checked_mul(u128::from(rate.denominator())).ok_or_else(||invalid("caption frame clock overflow"))?;
    u64::try_from(top.div_ceil(bottom)).map_err(|_|invalid("caption ordinal exceeds its bound"))
}
fn cues(lyrics:&lyrics::ReviewedLyricsRendering,video:&crate::final_video::FinalVideoArtifact)->Result<Vec<CaptionCue>> {
    lyrics.document.validate()?;
    if lyrics.delivery.delivery_extent!=video.video_interval||lyrics.document.cues.is_empty()||lyrics.document.cues.len()>64{return Err(invalid("caption rendering requires 1..64 complete cues in the exact selected picture extent; no implicit excerpt/trim"));}
    let mut output=Vec::new();let mut previous=0;
    for cue in &lyrics.document.cues {
        if cue.text.trim().is_empty()||cue.text.chars().count()>512||cue.text.lines().count()>4||cue.text.contains(['<','>','{','}']) {return Err(invalid("initial caption renderer accepts bounded plain authored lines without rich markup"));}
        let crate::timed_text::CueTiming::Interval {start,end}=cue.timing else {return Err(invalid("caption cues require explicit intervals"));};
        let first=frame_at(start,video.frame_rate)?;let last=frame_at(end,video.frame_rate)?;
        if first>=last||last>video.frame_count||first<previous{return Err(invalid("caption intervals overlap on the selected CFR grid, fall outside picture or have no visible frame"));}
        output.push(CaptionCue {id:cue.id.clone(),text:cue.text.clone(),first_frame:first,end_frame:last});previous=last;
    }Ok(output)
}
fn selection(configuration:&CaptionConfiguration,cues:&[CaptionCue])->Result<Vec<u64>> {
    let indices=if configuration.preview_cue_indices.is_empty() {let mut v=vec![0,cues.len()/2,cues.len()-1];v.sort();v.dedup();v}else{configuration.preview_cue_indices.clone()};
    let mut ordinals=Vec::new();
    for index in indices {let cue=cues.get(index).ok_or_else(||invalid("preview cue index is outside the reviewed carrier"))?;ordinals.push(cue.first_frame+(cue.end_frame-cue.first_frame-1)/2);}
    ordinals.sort();ordinals.dedup();Ok(ordinals)
}
fn capture(request:&CaptionRequest,renderer_sha256:&str,token:&CancellationToken)->Result<CaptionOperation> {
    cancelled(token)?;request.validate()?;let completion=crate::final_video::observe(&request.video_run_directory)?;
    let source=completion.artifacts.iter().find(|a|a.artifact_id==request.artifact_id).ok_or_else(||invalid("selected accepted video artifact missing"))?.clone();
    if identity(&source.locator,request.configuration.settings.maximum_source_bytes)?!=(source.sha256.clone(),source.byte_count)
        ||u64::from(source.width)*u64::from(source.height)>request.configuration.settings.maximum_pixels {return Err(invalid("caption source exceeds selected media/pixel bounds"));}
    if source.artifact_id=="captioned_delivery"||source.variant!=crate::final_video::FinalVideoVariant::DestinationCopy||source.video_codec!="h264"||source.pixel_format!="yuv420p"
        ||source.color_primaries!="bt709"||source.color_transfer!="bt709"||source.color_matrix!="bt709"||source.color_range!="tv"||source.sample_aspect_ratio!="1:1"||source.orientation_degrees!=0
        ||source.video_interval.start!=RationalTime::ZERO||source.frame_count==0||source.frame_count>100_000||request.configuration.style.bottom_margin>=source.height
        ||source.audio.as_ref().is_some_and(|a|a.codec!="aac"||a.profile!="LC") {return Err(invalid("caption source must be a current uncaptioned fixed H.264 SDR/CFR destination copy with original AAC and square/unrotated zero-origin picture"));}
    let lyrics=lyrics::observe_delivery(&request.lyrics_run_directory,&request.lyrics_delivery_directory)?;
    let cues=cues(&lyrics,&source)?;let preview_ordinals=selection(&request.configuration,&cues)?;
    let font=identity(&request.configuration.font.file,FONT_BYTES)?;
    if font.0!=request.configuration.font.sha256{return Err(invalid("caption font changed"));}
    use std::io::Read;
    let mut head=[0u8;4];crate::audio_inspection::process::open_regular(&request.configuration.font.file).map_err(|_|invalid("font unavailable"))?.read_exact(&mut head).map_err(|_|invalid("font header truncated"))?;
    if !matches!(&head,b"OTTO"|b"\0\x01\0\0"){return Err(invalid("initial caption font requires one explicitly pinned TrueType/OpenType face; no font collection or discovery"));}
    crate::video_assembly::observe_components(&request.configuration.settings)?;
    let result=CaptionOperation {schema:SCHEMA.into(),work:CaptionWork::Preview,renderer_sha256:renderer_sha256.into(),request:request.clone(),source:CaptionSource {completion_id:completion.completion_id,plan_sha256:completion.plan_sha256,run_manifest_sha256:completion.run_manifest_sha256,artifact:source},
        lyrics,font_byte_count:font.1,cues,preview_ordinals,preview:None,declaration:None};
    result.validate()?;cancelled(token)?;Ok(result)
}
impl CaptionOperation {
    fn context(&self)->Result<String> {canonical_sha256(&("aniflow.caption-preview-context/v1",&self.renderer_sha256,&self.request,&self.source,&self.lyrics,self.font_byte_count,&self.cues,&self.preview_ordinals,"drawtext-file-plain-cfr/1"))}
    fn validate(&self)->Result<()> {
        self.request.validate()?;crate::provider::require_sha256(&self.renderer_sha256,"caption renderer implementation")?;
        if self.schema!=SCHEMA||self.cues!=cues(&self.lyrics,&self.source.artifact)?||self.preview_ordinals!=selection(&self.request.configuration,&self.cues)?||self.font_byte_count>FONT_BYTES{return Err(invalid("caption operation differs from its exact source/carrier/frame selection"));}
        match self.work {
            CaptionWork::Preview if self.preview.is_none()&&self.declaration.is_none()=>{},
            CaptionWork::Deliver=>{let binding=self.preview.as_ref().ok_or_else(||invalid("caption delivery needs accepted preview evidence"))?;let declaration=self.declaration.as_ref().ok_or_else(||invalid("caption delivery needs a supplied preview declaration"))?;declaration.validate()?;
                validate_declaration(&self.context()?,binding,declaration)?;},
            _=>return Err(invalid("caption preview cannot contain a declaration or existing preview")),
        }
        local::bytes(self)?;Ok(())
    }
    fn schema_reference()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:SCHEMA.into(),version:"1.0.0".into(),sha256:local::hash(include_bytes!("../../docs/contracts/caption-operation-v1.schema.json"))}}
    fn configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let value=serde_json::to_value(self).map_err(|_|invalid("cannot serialize caption operation"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:self.work.capability().into(),version:"1.0.0".into()},Self::schema_reference(),value.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    fn manifest(&self)->Result<crate::ProviderManifest> {
        let port=|name:&str,kind:&str,role:&str,stream:&str|serde_json::json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value=serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow selected caption preview/delivery"},"configuration_schemas":[Self::schema_reference()],"capabilities":[{
            "id":self.work.capability(),"version":"1.0.0","kind":"assembler_encoder","configuration_schema":Self::schema_reference(),
            "inputs":[port("source","video/mp4","temporal_source","video"),port("srt","application/x-subrip","temporal_component","subtitle"),port("font","application/octet-stream","intermediate","attachment")],
            "outputs":[port("media",self.work.media_type(),if self.work==CaptionWork::Preview {"intermediate"}else{"candidate_master"},"video"),port("evidence","application/vnd.aniflow.caption-report+json","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":crate::video_assembly::component_inventory(&self.request.configuration.settings).tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":format!("={}",t.version),"sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":if self.work==CaptionWork::Deliver&&self.request.configuration.mode==CaptionMode::SoftMux {"lossless"}else{"lossy"},"cacheable":false,"content_changes":self.work==CaptionWork::Preview||self.request.configuration.mode==CaptionMode::BurnIn,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
    fn working_bytes(&self)->Result<u64> {
        let settings=&self.request.configuration.settings;let tools=crate::video_assembly::observe_components(settings)?;
        let total=self.source.artifact.byte_count.checked_mul(3).and_then(|n|n.checked_add(settings.maximum_output_bytes.checked_mul(3)?))
            .and_then(|n|n.checked_add(settings.maximum_frame_bytes.checked_mul(self.preview_ordinals.len() as u64*2)?))
            .and_then(|n|n.checked_add((tools[0].1+tools[1].1).checked_mul(2)?)).and_then(|n|n.checked_add(self.font_byte_count*2+268_435_456)).ok_or_else(||invalid("caption storage bound overflow"))?;
        if total>settings.maximum_working_bytes{return Err(invalid("caption source/tools/preview/native evidence exceed selected working-byte admission"));}Ok(total)
    }
}
fn validate_declaration(context:&str,binding:&PreviewBinding,declaration:&ManualReview)->Result<()> {
    declaration.validate()?;
    for hash in [&binding.report_sha256,&binding.context_sha256,&binding.subject_sha256] {crate::provider::require_sha256(hash,"caption preview binding")?;}
    if declaration.decision!=ReviewDecision::Approve||binding.context_sha256!=context||declaration.context_sha256!=binding.context_sha256||declaration.subject_sha256!=binding.subject_sha256{return Err(invalid("caption declaration must approve the exact selected preview context and bytes"));}Ok(())
}
impl CaptionWork {
    fn stage(self)->&'static str {match self {Self::Preview=>"caption_preview",Self::Deliver=>"caption_delivery"}}
    fn capability(self)->&'static str {match self {Self::Preview=>"aniflow/caption-preview",Self::Deliver=>"aniflow/caption-delivery"}}
    fn media_type(self)->&'static str {match self {Self::Preview=>"application/vnd.aniflow.caption-preview-png-set",Self::Deliver=>"video/mp4"}}
    fn artifact(self)->&'static str {match self {Self::Preview=>"caption_frames",Self::Deliver=>"captioned_delivery"}}
    fn evidence(self)->&'static str {match self {Self::Preview=>"caption_preview_evidence",Self::Deliver=>"caption_delivery_evidence"}}
}
#[cfg(test)]
mod tests;
