//! Stream-copy picture and encode only the explicitly selected gain candidate.
use super::*;
use crate::delivery_audio::gain_video::{Operation,Media};

pub(crate) fn audio_clock_filter(ticks:i64)->String {
    if ticks<0 {format!("asetpts=PTS-{}",ticks.unsigned_abs())}else{format!("asetpts=PTS+{ticks}")}
}

pub(crate) fn execute(operation:&Operation,settings:&AssemblyConfiguration,source:&Path,audio:&Path,output:&Path,scratch:&Path,token:&CancellationToken)->Result<Media> {
    operation.validate()?;verify_versions(settings,scratch,token,&GroupPolicy::Inherit)?;
    let selection=StreamSelection {video_stream:Some(0),audio_stream:Some(1),no_audio:false,discard_streams:vec![],audio_extent:Some(crate::temporal::AudioExtentPolicy::PreserveExact)};
    let before=observe(source,&selection,settings,scratch,token,settings.maximum_source_bytes,&GroupPolicy::Inherit)?;
    admitted_source_video(&before)?;super::destination::source_matches(&before,&operation.artifact)?;
    let tags=crate::delivery_metadata::inventory(&before.inventory)?;
    if array(&before.inventory,"streams")?.len()!=2||!tags.chapters.is_empty()||!tags.programs.is_empty(){return Err(refused("gain video initially supports one plain/burned-caption H.264 track and one AAC-LC track, without extra tracks/chapters/programs"));}
    let wave=crate::audio_inspection::wav::inspect_native(audio)?;
    if wave.sample_format!=crate::audio_inspection::NativeSampleFormat::Float32||wave.pcm_sha256!=operation.gain.report.output_pcm_sha256||wave.sample_rate_hz!=48_000||wave.channels!=operation.gain.report.channels||wave.frame_count!=operation.gain.report.frame_count{return Err(refused("gain video audio input differs from complete selected candidate PCM"));}
    let video_before=stream(&before.inventory,0)?;
    let timebase=RationalTime::parse(text(video_before,"time_base")?).map_err(media_error)?;
    if timebase.numerator()!=1||timebase.denominator()>2_147_483_647{return Err(refused("gain video requires exact source video ticks supported by the MP4 timescale"));}
    let scale=timebase.denominator();let movie_scale=(scale/gcd(scale,48_000)).checked_mul(48_000).filter(|s|*s<=2_147_483_647).ok_or_else(||refused("gain video exact combined timescale exceeds MP4 profile"))?;
    let mut args=strings(&["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-copyts","-noautorotate","-i"]);args.push(source.as_os_str().to_owned());args.push("-i".into());args.push(audio.as_os_str().to_owned());
    args.extend(strings(&["-map","0:0","-map","1:0","-c:v","copy","-c:a","aac","-b:a"]));args.push(format!("{}k",operation.request.audio_bitrate_kbps).into());
    args.push("-af".into());args.push(audio_clock_filter(operation.audio_offset_ticks()?).into());
    args.extend(strings(&["-threads:a","1","-enc_time_base:a","1:48000","-video_track_timescale"]));args.push(scale.to_string().into());args.push("-movie_timescale".into());args.push(movie_scale.to_string().into());
    args.extend(strings(&["-avoid_negative_ts","disabled","-use_editlist","1","-movflags","+faststart","-map_metadata","-1","-map_chapters","-1"]));
    crate::delivery_metadata::append_arguments(&mut args,settings.delivery_metadata.as_ref().ok_or_else(||refused("explicit gain video metadata policy missing"))?,&tags,1,0,true)?;
    args.push("-fs".into());args.push(settings.maximum_output_bytes.to_string().into());args.extend(strings(&["-f","mp4","-n"]));args.push(output.as_os_str().to_owned());
    run(&settings.ffmpeg,"ffmpeg",&args,settings,scratch,token)?;
    let after=observe(output,&selection,settings,scratch,token,settings.maximum_output_bytes,&GroupPolicy::Inherit)?;
    admitted_source_video(&after)?;crate::temporal::validate_reconstruction(&before.temporal,&after.temporal,false).map_err(media_error)?;
    let video_after=stream(&after.inventory,0)?;
    for field in ["codec_name","profile","level","pix_fmt","width","height","sample_aspect_ratio","color_primaries","color_transfer","color_space","color_range","extradata_hash","extradata_size"] {
        if video_before.get(field)!=video_after.get(field){return Err(refused("gain video changed picture codec/configuration/geometry/color"));}
    }
    let packets=video_packets(source,0,&before,settings,scratch,token)?;
    if packets!=video_packets(output,0,&after,settings,scratch,token)?{return Err(refused("gain video changed exact ordered picture packets/timestamps"));}
    if array(&after.inventory,"streams")?.len()!=2{return Err(refused("gain video output gained or lost streams"));}
    let facts=after.audio.as_ref().ok_or_else(||refused("gain video output audio missing"))?;
    if facts.codec!="aac"||facts.profile!="LC"||facts.sample_rate!=48_000||facts.channels!=u64::from(wave.channels)||!matches!((facts.channels,facts.channel_layout.as_str()),(1,"mono")|(2,"stereo")){return Err(refused("gain video changed selected audio rate/channels/layout"));}
    let interval=after.temporal.timeline(1).and_then(|t|t.source_time).ok_or_else(||refused("gain video output audio clock missing"))?;
    if Some(interval)!=operation.artifact.audio.as_ref().map(|a|a.interval){return Err(refused("gain video changed full original audio start/end/tail"));}
    let mut metadata=crate::delivery_metadata::verify(settings.delivery_metadata.as_ref().ok_or_else(||refused("gain video metadata policy missing"))?,tags,crate::delivery_metadata::inventory(&after.inventory)?,&before.temporal.source_sha256,&after.temporal.source_sha256,0,Some(1))?;
    let container=crate::delivery_metadata::mp4::inspect(output,settings.maximum_output_bytes,token)?;crate::delivery_metadata::mp4::agrees(&container,&metadata.after)?;
    if metadata.public_summary.status!=crate::delivery_metadata::MetadataInspectionStatus::Unsupported{return Err(refused("gain video metadata dictionaries retain unsupported or failed policy fields"));}
    metadata.public_summary.status=crate::delivery_metadata::MetadataInspectionStatus::CleanWithinScope;
    metadata.public_summary.inspected_scope="ffprobe dictionaries plus bounded MP4 global mdta; separate picture packets and complete native audio/video clocks".into();
    metadata.public_summary.unsupported_scope="signal marks, credential authenticity, native/platform/quality qualification".into();
    full_decode(output,true,settings,scratch,token)?;
    if !faststart(output,settings.maximum_output_bytes)?{return Err(refused("gain video MP4 is not faststart"));}
    verify_versions(settings,scratch,token,&GroupPolicy::Inherit)?;
    let playback=MetadataRemuxPlayback {video_codec:"h264".into(),pixel_format:"yuv420p".into(),color_primaries:"bt709".into(),color_transfer:"bt709".into(),color_matrix:"bt709".into(),color_range:"tv".into(),sample_aspect_ratio:"1/1".into(),orientation_degrees:0,faststart:true,audio:Some(crate::final_video::FinalAudioProperties {codec:facts.codec.clone(),profile:facts.profile.clone(),sample_rate_hz:facts.sample_rate,channels:facts.channels,channel_layout:facts.channel_layout.clone(),interval,packet_configuration_timing_sha256:digest(facts)?})};
    Ok(Media {source:before.temporal,candidate:after.temporal,video_packets_sha256:packets,playback,metadata,container,full_decode_observed:true,native_qualified:false})
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn clock_mapping_uses_original_sample_ticks_without_rounding_to_decimal_seconds(){assert_eq!(audio_clock_filter(1),"asetpts=PTS+1");assert_eq!(audio_clock_filter(-1024),"asetpts=PTS-1024");}
}
