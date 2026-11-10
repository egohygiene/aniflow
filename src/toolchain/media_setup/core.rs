//! Inert selected core preparation; no media, model or execution port.
use super::*;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameExtractionSetupOptions {
    pub source_selection:crate::temporal::StreamSelection,pub timeout_milliseconds:u64,pub maximum_capture_bytes:usize,
    pub maximum_source_bytes:u64,pub maximum_frame_bytes:u64,pub maximum_total_frame_bytes:u64,pub maximum_pixels:u64,
    pub maximum_frames:u64,pub maximum_working_bytes:u64,pub minimum_free_bytes:u64,
}
impl FrameExtractionSetupOptions {
    pub(super) fn configuration(&self,ffmpeg:AudioToolPin,ffprobe:AudioToolPin)->crate::frame_extraction::FrameExtractionConfiguration {
        crate::frame_extraction::FrameExtractionConfiguration {schema:crate::frame_extraction::FRAME_EXTRACTION_CONFIGURATION_SCHEMA_V1.into(),ffmpeg,ffprobe,
            source_selection:self.source_selection.clone(),timeout_milliseconds:self.timeout_milliseconds,maximum_capture_bytes:self.maximum_capture_bytes,
            maximum_source_bytes:self.maximum_source_bytes,maximum_frame_bytes:self.maximum_frame_bytes,maximum_total_frame_bytes:self.maximum_total_frame_bytes,maximum_pixels:self.maximum_pixels,
            maximum_frames:self.maximum_frames,maximum_working_bytes:self.maximum_working_bytes,minimum_free_bytes:self.minimum_free_bytes}
    }
}
pub(super) fn extraction_requirements()->(Vec<&'static str>,Vec<&'static str>) {
    (vec!["-version","-hide_banner","-nostdin","-v","-xerror","-n","-copyts","-filter_threads","-threads","-err_detect","-protocol_whitelist","-noautorotate","-apply_cropping","-i","-map","-an","-sn","-dn","-map_metadata","-map_chapters","-noautoscale","-vf","-pix_fmt","-threads:v","-fps_mode:v","-enc_time_base:v","-frames:v","-c:v","-compression_level","-pred","-colorspace","-color_range","-color_primaries","-color_trc","-f","-start_number","-atomic_writing","-format_version","-hash"],
    vec!["protocol:file","protocol:pipe","demuxer:mov","decoder:h264","decoder:aac","encoder:png","encoder:rawvideo","muxer:image2","muxer:framehash","filter:scale","filter:format","filter:setsar","pixel_format:rgb24","hash:sha256"])
}
pub(super) fn assembly_requirements()->(Vec<&'static str>,Vec<&'static str>) {
    (vec!["-version","-hide_banner","-nostdin","-v","-xerror","-n","-copyts","-copytb","-filter_threads","-threads","-err_detect","-protocol_whitelist","-f","-pattern_type","-start_number","-start_number_range","-framerate","-c:v","-i","-noautorotate","-map","-an","-c:a","-copypriorss:a","-sn","-dn","-map_metadata","-map_chapters","-fps_mode:v","-enc_time_base:v","-vf","-level:v","-slicecrc","-pix_fmt","-colorspace","-color_range","-crf","-preset","-bf","-threads:v","-color_primaries","-color_trc","-avoid_negative_ts","-use_editlist","-video_track_timescale","-movie_timescale","-movflags","-format_version","-hash"],
    vec!["protocol:file","protocol:pipe","demuxer:mov","demuxer:image2","decoder:png","decoder:ffv1","decoder:h264","decoder:aac","encoder:ffv1","encoder:libx264","encoder:rawvideo","encoder:wrapped_avframe","encoder:pcm_s16le","muxer:mp4","muxer:framehash","muxer:null","filter:scale","filter:setsar","filter:format","pixel_format:rgb24","pixel_format:bgr0","pixel_format:yuv420p","hash:sha256"])
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreMediaSetupRequest {pub schema:String,pub profile:ToolchainProfile,pub inventory:ToolchainInventory,pub selections:Vec<PublishingSetupChoice>}
impl CoreMediaSetupRequest {
    pub fn load(path:impl AsRef<Path>)->Result<Self>{Self::from_json_slice(&super::super::types::read_json(path.as_ref())?)}
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>MAXIMUM_REQUEST_BYTES{return Err(invalid("core media setup exceeds one MiB"));}
        let value:Self=super::super::types::decode_unique_json(bytes)?;value.validate()?;Ok(value)
    }
    fn selected(&self,choice:&PublishingSetupChoice)->MediaSetupRequest {MediaSetupRequest {schema:MEDIA_SETUP_REQUEST_SCHEMA.into(),profile:self.profile.clone(),inventory:self.inventory.clone(),capability_id:choice.capability_id.clone(),selection:choice.selection.clone()}}
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.toolchain.core-media-setup/v1"||!(1..=2).contains(&self.selections.len()){return Err(invalid("core setup needs 1..2 explicitly selected core capabilities"));}
        let mut ids=BTreeSet::new();let mut kinds=BTreeSet::new();
        for choice in &self.selections {
            let kind=match choice.selection {MediaSetupSelection::FrameExtraction {..}=>"extract",MediaSetupSelection::CleanVideoAssembly {..}=>"assemble",_=>return Err(invalid("core setup accepts only frame extraction and clean master/delivery assembly"))};
            if !ids.insert(&choice.capability_id)||!kinds.insert(kind){return Err(invalid("core setup selections must have distinct capability IDs and domain roles"));}self.selected(choice).validate()?;
        }Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreMediaSetupPreparation {
    pub schema:String,pub request_sha256:String,pub ready:bool,pub selections:Vec<PublishingSetupStep>,pub diagnostics:Vec<String>,
    pub native_qualification:bool,pub host_runtime_evidence:String,pub source_media_evidence:String,pub provider_evidence:String,pub publication_authority:String,
}
pub fn prepare_core_media_setup(request:&CoreMediaSetupRequest)->Result<CoreMediaSetupPreparation> {
    request.validate()?;if crate::provider::canonical_json_bytes(request)?.len()>MAXIMUM_REQUEST_BYTES{return Err(invalid("core media setup exceeds one MiB"));}
    let mut selections=Vec::new();let mut diagnostics=Vec::new();
    for choice in &request.selections {let preparation=prepare_media_setup(&request.selected(choice))?;diagnostics.extend(preparation.diagnostics.iter().map(|d|format!("{}: {d}",choice.capability_id)));selections.push(PublishingSetupStep {capability_id:choice.capability_id.clone(),selection:choice.selection.clone(),preparation});}
    let value=CoreMediaSetupPreparation {schema:"aniflow.toolchain.core-media-setup-preparation/v1".into(),request_sha256:crate::provider::canonical_sha256(request)?,ready:selections.iter().all(|s|s.preparation.ready),selections,diagnostics,
        native_qualification:false,host_runtime_evidence:"unobserved".into(),source_media_evidence:"unobserved".into(),provider_evidence:"unobserved; actual facade planning/execution supplies native provider identity".into(),publication_authority:"none".into()};
    if crate::provider::canonical_json_bytes(&value)?.len()>MAXIMUM_PREPARATION_BYTES{return Err(invalid("core media preparation exceeds eight MiB"));}Ok(value)
}
#[cfg(test)]
mod tests;
