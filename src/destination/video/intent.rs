use std::path::PathBuf;
use chrono::NaiveDate;
use serde::{Deserialize,Serialize};
use crate::{CancellationToken,Result,final_video::{self,FinalVideoHandoff,FinalVideoArtifact,FinalVideoVariant},video_assembly::AssemblyConfiguration};
use crate::provider::canonical_sha256;
use crate::destination::{Destination,DestinationProfile,MediaRequirements,invalid,cancelled};

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="mode",rename_all="snake_case",deny_unknown_fields)]
pub enum GeometryRule {
    ExactAspect {width:u32,height:u32},
    FitWithin {maximum_width:u32,maximum_height:u32},
    /// Opaque black padding; chroma-aligned offsets may differ by two pixels.
    FitAndPad {width:u32,height:u32},
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedGeometry {
    pub width:u32,pub height:u32,pub scaled_width:u32,pub scaled_height:u32,
    pub pad_left:u32,pub pad_top:u32,pub pad_right:u32,pub pad_bottom:u32,
}
fn axes(width:u32,height:u32)->Result<()> {
    if !(2..=8192).contains(&width)||!(2..=8192).contains(&height)||width%2!=0||height%2!=0{return Err(invalid("destination axes must be even and within 2..8192 pixels"));}Ok(())
}
fn gcd(mut a:u32,mut b:u32)->u32 {while b!=0 {let next=a%b;a=b;b=next;}a}
pub fn resolve_geometry(source_width:u32,source_height:u32,rule:&GeometryRule,allow_upscale:bool)->Result<ResolvedGeometry> {
    if !(2..=16_384).contains(&source_width)||!(2..=16_384).contains(&source_height)||source_width%2!=0||source_height%2!=0{return Err(invalid("destination input requires explicit even source geometry"));}
    let divisor=gcd(source_width,source_height);let unit_w=source_width/divisor;let unit_h=source_height/divisor;
    let (bound_w,bound_h)=match *rule {GeometryRule::ExactAspect {width,height}|GeometryRule::FitAndPad {width,height}=>(width,height),GeometryRule::FitWithin {maximum_width,maximum_height}=>(maximum_width,maximum_height)};
    axes(bound_w,bound_h)?;
    let (scaled_width,scaled_height)=if matches!(rule,GeometryRule::ExactAspect {..}) {
        if u64::from(bound_w)*u64::from(source_height)!=u64::from(bound_h)*u64::from(source_width){return Err(invalid("exact destination geometry changes aspect; select explicit fit/padding instead"));}
        (bound_w,bound_h)
    }else{
        let mut multiple=(bound_w/unit_w).min(bound_h/unit_h);if !allow_upscale{multiple=multiple.min(divisor);}multiple-=multiple%2;
        if multiple==0{return Err(invalid("requested bounds cannot represent the exact source aspect with even dimensions"));}(unit_w*multiple,unit_h*multiple)
    };
    if !allow_upscale&&(scaled_width>source_width||scaled_height>source_height){return Err(invalid("destination upscaling requires explicit opt-in"));}
    let (width,height)=if matches!(rule,GeometryRule::FitAndPad {..}){(bound_w,bound_h)}else{(scaled_width,scaled_height)};
    let left=((width-scaled_width)/2)&!1;let top=((height-scaled_height)/2)&!1;
    Ok(ResolvedGeometry {width,height,scaled_width,scaled_height,pad_left:left,pad_top:top,pad_right:width-scaled_width-left,pad_bottom:height-scaled_height-top})
}
impl ResolvedGeometry {
    /// Closed filter intent. No source text, expressions or arbitrary argv.
    pub(crate) fn filter(&self,source_range:&str)->Result<String> {
        axes(self.width,self.height)?;axes(self.scaled_width,self.scaled_height)?;
        if self.pad_left%2!=0||self.pad_top%2!=0||self.pad_left.checked_add(self.scaled_width).and_then(|v|v.checked_add(self.pad_right))!=Some(self.width)
            ||self.pad_top.checked_add(self.scaled_height).and_then(|v|v.checked_add(self.pad_bottom))!=Some(self.height){return Err(invalid("invalid destination padding geometry"));}
        let range=match source_range {"pc"=>"full","tv"=>"limited",_=>return Err(invalid("unsupported declared source color range"))};
        let mut filter=format!("scale={}:{}:flags=lanczos:in_range={range}:out_range=limited:in_color_matrix=bt709:out_color_matrix=bt709",self.scaled_width,self.scaled_height);
        if self.width!=self.scaled_width||self.height!=self.scaled_height {filter.push_str(&format!(",pad={}:{}:{}:{}:color=black",self.width,self.height,self.pad_left,self.pad_top));}
        filter.push_str(",setsar=1,format=yuv420p");Ok(filter)
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportConfiguration {
    pub schema:String,pub tools:AssemblyConfiguration,pub geometry:GeometryRule,pub allow_upscale:bool,
    pub profile:DestinationProfile,pub evaluated_on:NaiveDate,
}
impl VideoExportConfiguration {
    pub fn validate(&self)->Result<()> {
        self.tools.validate()?;self.profile.validate()?;
        if self.schema!="aniflow.destination-video-configuration/v1"||self.profile.destination!=Destination::Tiktok||!matches!(self.profile.requirements,MediaRequirements::Video {..})
            ||self.tools.delivery_metadata.is_none()||self.evaluated_on<self.profile.evidence.checked_on||self.evaluated_on>self.profile.evidence.valid_through {
            return Err(invalid("destination video needs a current supplied manual-video profile and explicit metadata policy"));}
        let (w,h)=match self.geometry {GeometryRule::ExactAspect {width,height}|GeometryRule::FitAndPad {width,height}=>(width,height),GeometryRule::FitWithin {maximum_width,maximum_height}=>(maximum_width,maximum_height)};axes(w,h)
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportRequest {pub schema:String,pub source_run_directory:PathBuf,pub artifact_id:String,pub configuration:VideoExportConfiguration,pub provider_executable:PathBuf}
impl VideoExportRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>65_536{return Err(invalid("destination-video request exceeds 64 KiB"));}let v:Self=crate::provider::decode_json(bytes,"destination-video request")?;v.configuration.validate()?;
        if v.schema!="aniflow.destination-video-request/v1"{return Err(invalid("unsupported destination-video request schema"));}Ok(v)
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportPayload {pub request:VideoExportRequest,pub source:FinalVideoHandoff,pub artifact:FinalVideoArtifact,pub geometry:ResolvedGeometry}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoExportIntent {pub schema:String,pub intent_sha256:String,pub payload:VideoExportPayload,pub quality_approval:String,pub platform_authority:String}

pub(crate) fn check_source(a:&FinalVideoArtifact,configuration:&VideoExportConfiguration,geometry:&ResolvedGeometry)->Result<()> {
    configuration.validate()?;
    let format=match a.variant {
        FinalVideoVariant::CleanArchivalMaster=>a.video_codec=="ffv1"&&matches!(a.pixel_format.as_str(),"bgr0"|"rgb24"|"gbrp"),
        FinalVideoVariant::DestinationCopy=>a.video_codec=="h264"&&a.pixel_format=="yuv420p",
        FinalVideoVariant::SubtitleMuxedCandidate=>false,
    };
    if !format||a.color_primaries!="bt709"||a.color_transfer!="bt709"||!matches!(a.color_matrix.as_str(),"bt709"|"gbr")||!matches!(a.color_range.as_str(),"tv"|"pc")
        ||!final_video::square_pixels(&a.sample_aspect_ratio)||a.orientation_degrees!=0||a.frame_count==0||a.frame_count>100_000
        ||a.video_interval.start!=crate::temporal::RationalTime::ZERO||a.byte_count>configuration.tools.maximum_source_bytes
        ||u64::from(geometry.width)*u64::from(geometry.height)>configuration.tools.maximum_pixels {
        return Err(invalid("destination source must be a current closed SDR native master/delivery without authored subtitle tracks"));
    }
    if a.audio.as_ref().is_some_and(|a|a.codec!="aac"||a.profile!="LC"||a.sample_rate_hz!=48_000||!matches!((a.channels,a.channel_layout.as_str()),(1,"mono")|(2,"stereo"))) {
        return Err(invalid("initial destination audio is preserved 48 kHz mono/stereo AAC-LC; no audio encoding or resampling"));
    }
    if let MediaRequirements::Video {minimum_axis,maximum_axis,required_width,required_height,minimum_frame_rate,maximum_frame_rate,audio_required,maximum_audio_channels,require_zero_origin,require_matching_audio_extent}=&configuration.profile.requirements {
        let audio=a.audio.as_ref();
        let start=audio.map_or(a.video_interval.start,|v|v.interval.start.min(a.video_interval.start));let end=audio.map_or(a.video_interval.end,|v|v.interval.end.max(a.video_interval.end));
        let duration=end.checked_sub(start).map_err(|_|invalid("destination source extent overflow"))?;
        if [geometry.width,geometry.height].iter().any(|v|v<minimum_axis||v>maximum_axis)||required_width.is_some_and(|v|v!=geometry.width)||required_height.is_some_and(|v|v!=geometry.height)
            ||a.frame_rate<*minimum_frame_rate||a.frame_rate>*maximum_frame_rate||duration>configuration.profile.maximum_duration
            ||*audio_required&&audio.is_none()||audio.is_some_and(|v|v.channels>*maximum_audio_channels)
            ||*require_zero_origin&&(start!=crate::temporal::RationalTime::ZERO)||*require_matching_audio_extent&&audio.is_some_and(|v|v.interval!=a.video_interval) {
            return Err(invalid("requested video geometry/full source clock/audio extent does not match the supplied destination profile"));
        }
    }Ok(())
}
/// Source-only current acceptance/geometry plan. No native tool/provider launch.
pub fn intent(request:&VideoExportRequest,token:&CancellationToken)->Result<VideoExportIntent> {
    cancelled(token)?;request.configuration.validate()?;
    if request.schema!="aniflow.destination-video-request/v1"||request.artifact_id.is_empty()||request.artifact_id.len()>64{return Err(invalid("select an exact supported native video artifact"));}
    // Keep the selected native source independent of this export's own current
    // acceptance. Repeated export chains need a separate explicit profile.
    if !request.source_run_directory.join("intent.json").try_exists().map_err(|_|invalid("cannot inspect source recipe kind"))?
        &&!request.source_run_directory.join("finishing-intent.json").try_exists().map_err(|_|invalid("cannot inspect finishing source kind"))? {
        let workspace=crate::PipelineV3Workspace::open_read_only(&request.source_run_directory)?;
        let p:serde_json::Value=crate::destination::local::read_bounded(&workspace.plan(),67_108_864)?;
        if p.get("payload").and_then(|v|v.get("pipeline_name")).and_then(serde_json::Value::as_str)==Some("selected-destination-video-export") {
            return Err(invalid("select the original accepted master/finishing source; recursive destination export is unsupported"));
        }
    }
    let source=final_video::observe(&request.source_run_directory)?;
    let matches=source.artifacts.iter().filter(|a|a.artifact_id==request.artifact_id).collect::<Vec<_>>();
    if matches.len()!=1{return Err(invalid("selected accepted video is missing or ambiguous"));}let artifact=(*matches[0]).clone();
    let geometry=resolve_geometry(artifact.width,artifact.height,&request.configuration.geometry,request.configuration.allow_upscale)?;
    check_source(&artifact,&request.configuration,&geometry)?;
    if final_video::observe(&request.source_run_directory)?!=source{return Err(invalid("accepted source changed during destination intent"));}
    let payload=VideoExportPayload {request:request.clone(),source,artifact,geometry};cancelled(token)?;
    Ok(VideoExportIntent {schema:"aniflow.destination-video-intent/v1".into(),intent_sha256:canonical_sha256(&payload)?,payload,
        quality_approval:"not_performed; source acceptance and geometry planning do not approve this lossy candidate".into(),platform_authority:"none; supplied dated requirements are not live platform qualification".into()})
}
pub fn recheck(expected:&VideoExportIntent,token:&CancellationToken)->Result<()> {
    if expected.schema!="aniflow.destination-video-intent/v1"||expected.intent_sha256!=canonical_sha256(&expected.payload)?||intent(&expected.payload.request,token)?!=*expected{return Err(invalid("destination intent is stale or changed"));}Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_portrait_reduction_keeps_aspect_and_needs_no_padding() {
        let g=resolve_geometry(2160,3840,&GeometryRule::ExactAspect {width:1080,height:1920},false).unwrap();assert_eq!((g.scaled_width,g.scaled_height),(1080,1920));assert_eq!(g.pad_left+g.pad_right+g.pad_top+g.pad_bottom,0);
    }
    #[test]
    fn landscape_to_portrait_is_an_explicit_padded_choice() {
        assert!(resolve_geometry(1920,1080,&GeometryRule::ExactAspect {width:1080,height:1920},false).is_err());
        let g=resolve_geometry(1920,1080,&GeometryRule::FitAndPad {width:1080,height:1920},false).unwrap();assert_eq!(g.scaled_width*1080,g.scaled_height*1920);assert_eq!((g.width,g.height),(1080,1920));assert!(g.pad_top>0);assert_eq!(g.pad_top%2,0);assert!(g.filter("tv").unwrap().contains("pad="));
    }
    #[test]
    fn fit_without_padding_can_return_a_smaller_exact_aspect() {
        let g=resolve_geometry(1920,1080,&GeometryRule::FitWithin {maximum_width:1080,maximum_height:1920},false).unwrap();assert_eq!((g.width,g.height),(1056,594));assert_eq!(g.width*1080,g.height*1920);
    }
    #[test]
    fn neural_or_filter_upscaling_is_never_implicit() {
        assert!(resolve_geometry(1080,1920,&GeometryRule::ExactAspect {width:2160,height:3840},false).is_err());assert!(resolve_geometry(1080,1920,&GeometryRule::ExactAspect {width:2160,height:3840},true).is_ok());
    }
    #[test]
    fn malformed_geometry_or_unrepresentable_even_aspect_refuses() {
        for (w,h) in [(0,1920),(1081,1920),(9000,1920)] {assert!(resolve_geometry(1920,1080,&GeometryRule::FitAndPad {width:w,height:h},false).is_err());}
        assert!(resolve_geometry(2,16384,&GeometryRule::FitWithin {maximum_width:2,maximum_height:2},false).is_err());
    }
    #[test]
    fn direct_deserialized_padding_and_color_cannot_inject_filter_expressions() {
        let mut g=resolve_geometry(1920,1080,&GeometryRule::FitAndPad {width:1080,height:1920},false).unwrap();g.pad_left+=2;assert!(g.filter("tv").is_err());
        g.pad_left-=2;assert!(g.filter("tv,drawtext=private").is_err());
    }
}
