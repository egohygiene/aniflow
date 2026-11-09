//! Read-only destination checks over current accepted media and explicitly
//! supplied requirement evidence. No platform lookup, transform or posting.
mod types;
mod observation;
pub(crate) use observation::observe as observe_media;
pub mod posting;
pub mod post_upload;
pub mod video;
pub(crate) mod local;
pub use types::*;
use crate::{CancellationToken,Error,ErrorCategory,Result};
use crate::provider::{canonical_sha256,decode_json};
use crate::temporal::{RationalTime,TimeRange};
pub const MAXIMUM_REQUEST_BYTES:usize=65_536;
pub(crate) fn invalid(message:&str)->Error {Error::new(ErrorCategory::State,message)}
fn mapped(error:anyhow::Error)->Error {Error::from_anyhow(ErrorCategory::Media,error)}
pub(crate) fn cancelled(token:&CancellationToken)->Result<()> {if token.is_cancelled(){Err(Error::new(ErrorCategory::Execution,"delivery preparation cancelled"))}else{Ok(())}}
fn positive(time:RationalTime)->Result<()> {time.validate().map_err(mapped)?;if time<=RationalTime::ZERO||time>RationalTime::new(86_400,1).map_err(mapped)? {return Err(invalid("destination time/rate bound is outside its finite supported interval"));}Ok(())}
pub(crate) fn identifier(value:&str)->bool {!value.is_empty()&&value.len()<=64&&value.bytes().all(|v|v.is_ascii_alphanumeric()||b"-_.".contains(&v))}
impl DestinationProfile {
    pub fn validate(&self)->Result<()> {
        positive(self.maximum_duration)?;
        if self.schema!="aniflow.destination-profile/v1"||!identifier(&self.id)||!identifier(&self.revision)
            ||!(1..=1_099_511_627_776).contains(&self.maximum_file_bytes)
            ||self.evidence.checked_on>self.evidence.valid_through||self.evidence.checked_by.trim().is_empty()
            ||self.evidence.checked_by.len()>256||self.evidence.checked_by.chars().any(char::is_control)
            ||self.evidence.source_urls.is_empty()||self.evidence.source_urls.len()>8 {return Err(invalid("destination profile requires explicit bounded version, dates, requirement sources and checker declaration"));}
        let expected=match self.destination {Destination::Tiktok=>"developers.tiktok.com",Destination::Soundcloud=>"help.soundcloud.com",Destination::Distrokid=>"support.distrokid.com"};
        for (index,url) in self.evidence.source_urls.iter().enumerate(){
            if url.len()>2048||url.chars().any(|c|c.is_control()||c.is_whitespace())||url.contains(['@','?','#'])
                ||url.strip_prefix("https://").and_then(|s|s.split('/').next())!=Some(expected)
                ||self.evidence.source_urls[..index].contains(url) {return Err(invalid("destination requirement references must be distinct bounded official HTTPS locators without credentials"));}
        }
        match &self.requirements {
            MediaRequirements::Video {minimum_axis,maximum_axis,required_width,required_height,minimum_frame_rate,maximum_frame_rate,maximum_audio_channels,..}=>{
                positive(*minimum_frame_rate)?;positive(*maximum_frame_rate)?;
                if self.destination!=Destination::Tiktok||*minimum_axis<2||*maximum_axis<*minimum_axis||*maximum_axis>16_384
                    ||minimum_frame_rate>maximum_frame_rate||*maximum_frame_rate>RationalTime::new(240,1).map_err(mapped)?
                    ||!(1..=8).contains(maximum_audio_channels)||required_width.is_some()!=required_height.is_some()
                    ||[required_width,required_height].iter().filter_map(|v|**v).any(|v|v<*minimum_axis||v>*maximum_axis||v%2!=0) {
                    return Err(invalid("video destination requirements exceed the explicit bounded MP4 profile"));
                }
            },
            MediaRequirements::Pcm {sample_rates_hz,channels,encodings}=>{
                if !matches!(self.destination,Destination::Soundcloud|Destination::Distrokid)
                    ||sample_rates_hz.is_empty()||sample_rates_hz.len()>16||sample_rates_hz.iter().any(|v|!(8_000..=192_000).contains(v))
                    ||channels.is_empty()||channels.len()>2||channels.iter().any(|v|!matches!(*v,1|2))||encodings.is_empty()||encodings.len()>3
                    ||sample_rates_hz.iter().enumerate().any(|(i,v)|sample_rates_hz[..i].contains(v))
                    ||channels.iter().enumerate().any(|(i,v)|channels[..i].contains(v))
                    ||encodings.iter().enumerate().any(|(i,v)|encodings[..i].contains(v)) {return Err(invalid("PCM destination requires explicit supported rates, channels and native encodings"));}
            },
        }Ok(())
    }
}
impl PreflightRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>MAXIMUM_REQUEST_BYTES{return Err(invalid("destination request exceeds 64 KiB"));}
        let value:Self=decode_json(bytes,"destination preflight request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {if self.schema!="aniflow.destination-preflight-request/v1"{return Err(invalid("unsupported destination request"));}self.profile.validate()}
}
fn check(checks:&mut Vec<RequirementCheck>,id:&str,matches:bool,reason:&str){checks.push(RequirementCheck {id:id.into(),state:if matches {CheckState::Matches}else{CheckState::NeedsChanges},reason:reason.into()});}
fn video<'a>(completion:&'a crate::final_video::FinalVideoHandoff,id:&str)->Result<&'a crate::final_video::FinalVideoArtifact> {
    completion.artifacts.iter().find(|a|a.artifact_id==id).ok_or_else(||invalid("selected accepted video artifact disappeared"))
}
fn extent(artifact:&crate::final_video::FinalVideoArtifact)->Result<TimeRange> {
    let audio=artifact.audio.as_ref().map(|a|a.interval);
    TimeRange::new(audio.map_or(artifact.video_interval.start,|a|a.start.min(artifact.video_interval.start)),
        audio.map_or(artifact.video_interval.end,|a|a.end.max(artifact.video_interval.end))).map_err(mapped)
}
fn evaluate(request:&PreflightRequest,media:MediaObservation)->Result<PreflightReport> {
    let profile=&request.profile;let mut checks=Vec::new();
    let current=profile.evidence.checked_on<=request.evaluated_on&&request.evaluated_on<=profile.evidence.valid_through;
    checks.push(RequirementCheck {id:"requirement_dates".into(),state:if current {CheckState::Matches}else{CheckState::MissingEvidence},reason:"Only the supplied checker declaration and its explicit validity interval are evaluated; live platform policy was not fetched".into()});
    check(&mut checks,"file_size",media.file.byte_count<=profile.maximum_file_bytes,"Exact current file size compared with the supplied maximum; no recompression performed");
    let duration=match &media.facts {
        MediaFacts::Video {completion,artifact_id}|MediaFacts::VideoWithQc {completion,artifact_id,..}=>{let range=extent(video(completion,artifact_id)?)?;range.end.checked_sub(range.start).map_err(mapped)?},
        facts=>{let (summary,_)=pcm_facts(facts).ok_or_else(||invalid("PCM destination facts unavailable"))?;RationalTime::new(i64::try_from(summary.source.frame_count).map_err(|_|invalid("PCM sample count exceeds rational bound"))?,u64::from(summary.source.sample_rate_hz)).map_err(mapped)?},
    };
    check(&mut checks,"complete_duration",duration<=profile.maximum_duration,"Full picture/song union or complete PCM duration compared exactly; no trim, pad or excerpt inferred");
    match (&profile.requirements,&media.facts) {
        (MediaRequirements::Video {minimum_axis,maximum_axis,required_width,required_height,minimum_frame_rate,maximum_frame_rate,audio_required,maximum_audio_channels,require_zero_origin,require_matching_audio_extent},MediaFacts::Video {completion,artifact_id}|MediaFacts::VideoWithQc {completion,artifact_id,..})=>{
            let a=video(completion,artifact_id)?;
            check(&mut checks,"video_format",a.variant==crate::final_video::FinalVideoVariant::DestinationCopy&&a.video_codec=="h264"&&a.pixel_format=="yuv420p"&&a.color_primaries=="bt709"&&a.color_transfer=="bt709"&&a.color_matrix=="bt709"&&a.color_range=="tv"&&crate::final_video::square_pixels(&a.sample_aspect_ratio)&&a.orientation_degrees==0,"First video lane requires the exact accepted fixed SDR H.264 MP4 destination copy; clean archival masters remain separate");
            check(&mut checks,"video_axes",[a.width,a.height].iter().all(|v|v>=minimum_axis&&v<=maximum_axis),"Observed axes compared with declared bounds; no resize or crop performed");
            check(&mut checks,"video_geometry",required_width.is_none_or(|v|a.width==v)&&required_height.is_none_or(|v|a.height==v),"Optional exact geometry must already match; choose a separately approved derived copy for changes");
            check(&mut checks,"video_rate",a.frame_rate>=*minimum_frame_rate&&a.frame_rate<=*maximum_frame_rate,"Observed rational frame rate compared without dropping or synthesizing frames");
            check(&mut checks,"audio",(!*audio_required||a.audio.is_some())&&a.audio.as_ref().is_none_or(|a|a.codec=="aac"&&a.profile=="LC"&&a.channels<=*maximum_audio_channels),"Optional accepted AAC-LC audio/channels checked; mix, stereo and codec are never transformed here");
            check(&mut checks,"origins",!*require_zero_origin||(a.video_interval.start==RationalTime::ZERO&&a.audio.as_ref().is_none_or(|a|a.interval.start==RationalTime::ZERO)),"Requested zero origin is checked without timestamp normalization");
            check(&mut checks,"audio_extent",!*require_matching_audio_extent||a.audio.as_ref().is_none_or(|audio|audio.interval==a.video_interval),"Picture/song extents must already match when requested; offsets and tails are retained");
        },
        (MediaRequirements::Pcm {sample_rates_hz,channels,encodings},facts) if pcm_facts(facts).is_some()=>{
            let (summary,encoding)=pcm_facts(facts).ok_or_else(||invalid("lossless PCM facts unavailable"))?;
            check(&mut checks,"sample_rate",sample_rates_hz.contains(&summary.source.sample_rate_hz),"Original rate checked against explicit choices; no resampling");
            check(&mut checks,"channels",channels.contains(&summary.source.channels),"Original channel count checked; no mono fold-down or stereo synthesis");
            check(&mut checks,"pcm_encoding",encodings.contains(&encoding),"Exact native PCM representation checked; no precision reduction, dither or gain change");
        },
        _=>check(&mut checks,"media_kind",false,"Selected file kind differs from destination profile; no audio extraction or video synthesis is implicit"),
    }
    if profile.require_loudness_evidence {
        let measured=match &media.facts {
            MediaFacts::AudioWithQc {qc}=>measured_loudness(&qc.integrated_loudness,&qc.true_peak),
            MediaFacts::VideoWithQc {qc,..}=>measured_loudness(&qc.signal.loudness.integrated_loudness,&qc.signal.loudness.true_peak),
            facts=>pcm_facts(facts).is_some_and(|(summary,_)|measured_loudness(&summary.integrated_loudness,&summary.true_peak)),
        };
        let reason=if matches!(&media.facts,MediaFacts::LosslessAudio {..}) {"Only current exact-PCM accepted measurements count; verified lossless candidates reference unchanged PCM without a new meter pass. Unavailable is not zero; video loudness is not inferred"}
            else {"Only explicit current exact-final-file QC or exact-subject PCM observations count; unavailable is not zero. Numeric observations do not qualify the native meter, judge musical quality or change gain"};
        checks.push(RequirementCheck {id:"loudness_evidence".into(),state:if measured {CheckState::Matches}else{CheckState::MissingEvidence},reason:reason.into()});
    }
    let disposition=if checks.iter().any(|c|c.state==CheckState::NeedsChanges){PreflightDisposition::NeedsChanges}else if checks.iter().any(|c|c.state==CheckState::MissingEvidence){PreflightDisposition::MissingEvidence}else{PreflightDisposition::MatchesSuppliedProfile};
    let payload=PreflightPayload {request:request.clone(),profile_sha256:canonical_sha256(profile)?,media,checks,disposition,
        requirement_authority:"operator_supplied_dated_requirements; not independently verified live platform policy".into(),creative_approval:"not_performed".into(),publication_authority:"none".into()};
    Ok(PreflightReport {schema:"aniflow.destination-preflight/v1".into(),preflight_sha256:canonical_sha256(&payload)?,payload})
}
fn measured_loudness(integrated:&crate::audio_signal::AudioSignalMeasurement,peak:&crate::audio_signal::AudioSignalMeasurement)->bool {
    matches!(&integrated.value,crate::audio_signal::AudioSignalValue::Measured {value} if value.is_finite())
        &&matches!(&peak.value,crate::audio_signal::AudioSignalValue::Measured {value} if value.is_finite())
}
fn pcm_facts(facts:&MediaFacts)->Option<(&crate::delivery_audio::DeliveryLoudnessSummary,PcmEncoding)> {
    match facts {
        MediaFacts::Pcm {summary,encoding,..}=>Some((summary,*encoding)),
        MediaFacts::LosslessAudio {report}=>Some((&report.operation.source.summary,report.encoding)),
        MediaFacts::AudioWithQc {qc}=>pcm_facts(&qc.intent.source.facts),
        _=>None,
    }
}
/// Read only actual currently accepted facts; never execute a media tool.
pub fn preflight(request:&PreflightRequest,token:&CancellationToken)->Result<PreflightReport> {
    cancelled(token)?;request.validate()?;let media=observation::observe(&request.input,token)?;
    let report=evaluate(request,media)?;cancelled(token)?;Ok(report)
}
/// A saved request date is preserved, not refreshed to today's platform policy.
pub fn recheck(expected:&PreflightReport,token:&CancellationToken)->Result<()> {
    if expected.schema!="aniflow.destination-preflight/v1"||expected.preflight_sha256!=canonical_sha256(&expected.payload)?
        ||preflight(&expected.payload.request,token)?!=*expected{return Err(invalid("destination preflight is stale or differs from current accepted media/requirements"));}Ok(())
}

#[cfg(test)]
mod tests;
