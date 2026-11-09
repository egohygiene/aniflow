use std::path::PathBuf;
use serde::{Deserialize,Serialize};
use crate::{temporal::{RationalTime,TimeRange},video_assembly::AssemblyConfiguration,delivery_metadata::remux::MetadataRemuxPlayback,final_video::FinalVideoArtifact,music_video_recipe::{lyrics::ReviewedLyricsRendering,manual::ManualReview}};
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CaptionMode {SoftMux,BurnIn}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionFont {pub file:PathBuf,pub sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionStyle {pub font_size:u32,pub bottom_margin:u32,pub foreground:String,pub box_color:String,pub box_opacity:u8}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionConfiguration {
    pub schema:String,pub mode:CaptionMode,pub settings:AssemblyConfiguration,pub font:CaptionFont,pub style:CaptionStyle,
    /// Empty selects the first/middle/last cue deterministically, never approval.
    pub preview_cue_indices:Vec<usize>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionRequest {
    pub schema:String,pub video_run_directory:PathBuf,pub artifact_id:String,pub lyrics_run_directory:PathBuf,pub lyrics_delivery_directory:PathBuf,pub configuration:CaptionConfiguration,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionSource {pub completion_id:String,pub plan_sha256:String,pub run_manifest_sha256:String,pub artifact:FinalVideoArtifact}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionCue {pub id:String,pub text:String,pub first_frame:u64,pub end_frame:u64}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CaptionWork {Preview,Deliver}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewBinding {pub run_directory:PathBuf,pub report_sha256:String,pub context_sha256:String,pub subject_sha256:String}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionOperation {
    pub schema:String,pub work:CaptionWork,pub renderer_sha256:String,pub request:CaptionRequest,pub source:CaptionSource,pub lyrics:ReviewedLyricsRendering,
    pub font_byte_count:u64,pub cues:Vec<CaptionCue>,pub preview_ordinals:Vec<u64>,pub preview:Option<PreviewBinding>,pub declaration:Option<ManualReview>,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionPreviewFrame {pub ordinal:u64,pub time:RationalTime,pub filename:String,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionPreviewReport {
    pub schema:String,pub operation:CaptionOperation,pub provider_lock_sha256:String,pub context_sha256:String,pub subject_sha256:String,
    pub frames:Vec<CaptionPreviewFrame>,pub review_scope:String,pub native_qualified:bool,pub creative_approval:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionMediaEvidence {
    pub source_sha256:String,pub output_sha256:String,pub output_byte_count:u64,
    pub width:u32,pub height:u32,pub frame_count:u64,pub frame_rate:RationalTime,pub video_interval:TimeRange,
    pub playback:MetadataRemuxPlayback,pub video_packets_preserved:bool,pub pixel_policy:String,pub authored_subtitle_track:bool,pub full_decode_observed:bool,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionDeliveryReport {
    pub schema:String,pub operation:CaptionOperation,pub provider_lock_sha256:String,pub media:CaptionMediaEvidence,
    pub review_scope:String,pub lyrics_video_audio_binding:String,pub native_qualified:bool,pub creative_approval:String,pub publication_authority:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionPreviewCompletion {pub schema:String,pub binding:PreviewBinding,pub frames_directory:PathBuf,pub report:CaptionPreviewReport}
#[derive(Debug,Clone)]
pub struct CaptionRunRequest {pub request:CaptionRequest,pub provider_executable:PathBuf}
#[derive(Debug,Clone)]
pub struct CaptionDeliveryRequest {pub run:CaptionRunRequest,pub preview_run_directory:PathBuf,pub declaration:ManualReview}
