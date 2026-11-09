//! Caption-specific use of the existing native observer, process and packet gates.
use super::*;
use crate::subtitle_delivery::{CaptionOperation,CaptionMode,CaptionMediaEvidence,CaptionPreviewFrame};
use crate::frame_io::{identity,inspect_png};
fn selection(operation:&CaptionOperation,output:bool)->StreamSelection {
    let audio=operation.source.artifact.audio.is_some();
    StreamSelection {video_stream:Some(0),audio_stream:audio.then_some(1),no_audio:!audio,
        discard_streams:if output&&operation.request.configuration.mode==CaptionMode::SoftMux {vec![if audio {2}else{1}]}else{vec![]},
        audio_extent:Some(crate::temporal::AudioExtentPolicy::PreserveExact)}
}
fn source_observation(operation:&CaptionOperation,settings:&AssemblyConfiguration,source:&Path,root:&Path,token:&CancellationToken)->Result<Observation> {
    verify_versions(settings,root,token,&GroupPolicy::Inherit)?;
    let observed=observe(source,&selection(operation,false),settings,root,token,settings.maximum_source_bytes,&GroupPolicy::Inherit)?;admitted_source_video(&observed)?;
    let expected=&operation.source.artifact;let timeline=observed.temporal.video_timeline().map_err(media_error)?;
    let video=stream(&observed.inventory,0)?;
    if positive(video,"width")?!=u64::from(expected.width)||positive(video,"height")?!=u64::from(expected.height)
        ||text(video,"codec_name")?!=expected.video_codec||text(video,"pix_fmt")?!=expected.pixel_format {return Err(refused("caption source picture properties differ from accepted evidence"));}
    if observed.temporal.source_sha256!=expected.sha256||observed.temporal.source_size_bytes!=expected.byte_count||timeline.frames.len() as u64!=expected.frame_count||timeline.source_time!=Some(expected.video_interval)
        ||timeline.frame_period!=Some(RationalTime::new(expected.frame_rate.denominator() as i64,expected.frame_rate.numerator() as u64).map_err(media_error)?)
        ||observed.audio.as_ref().map(digest).transpose()?!=expected.audio.as_ref().map(|a|a.packet_configuration_timing_sha256.clone()) {return Err(refused("caption source differs from its exact accepted frame/audio subject"));}Ok(observed)
}
fn write_new(path:&Path,bytes:&[u8])->Result<()> {crate::destination::local::write(path,bytes)}
pub(crate) fn filter(operation:&CaptionOperation)->String {
    filter_parameters(&operation.cues,&operation.request.configuration.style)
}
pub(crate) fn filter_parameters(cues:&[crate::subtitle_delivery::CaptionCue],style:&crate::subtitle_delivery::CaptionStyle)->String {
    let opacity=format!("{:.6}",f64::from(style.box_opacity)/255.0);
    cues.iter().enumerate().map(|(index,cue)|format!("drawtext=fontfile=font.ttf:textfile=cue-{index:04}.txt:expansion=none:fontsize={}:fontcolor=0x{}:x=(w-tw)/2:y=h-th-{}:box=1:boxcolor=0x{}@{}:boxborderw=8:enable=gte(n\\,{})*lt(n\\,{})",
        style.font_size,style.foreground,style.bottom_margin,style.box_color,opacity,cue.first_frame,cue.end_frame)).collect::<Vec<_>>().join(",")
}
fn write_filter(operation:&CaptionOperation,root:&Path,suffix:&str)->Result<PathBuf> {
    for (index,cue) in operation.cues.iter().enumerate() {
        let path=root.join(format!("cue-{index:04}.txt"));
        if !path.exists() {write_new(&path,cue.text.as_bytes())?;}
        else if identity(&path,2048)?!=(crate::destination::local::hash(cue.text.as_bytes()),cue.text.len() as u64) {return Err(refused("caption text snapshot changed"));}
    }
    let script=format!("{}{}",filter(operation),suffix);let path=root.join(format!("filter-{}.txt",crate::destination::local::hash(script.as_bytes())));
    if !path.exists() {write_new(&path,script.as_bytes())?;}Ok(path)
}
pub(crate) fn preview(operation:&CaptionOperation,settings:&AssemblyConfiguration,source:&Path,frames:&Path,root:&Path,token:&CancellationToken)->Result<Vec<CaptionPreviewFrame>> {
    let _source=source_observation(operation,settings,source,root,token)?;
    crate::destination::local::directory(frames)?;let mut result=Vec::new();
    let rate=operation.source.artifact.frame_rate;let period=RationalTime::new(rate.denominator() as i64,rate.numerator() as u64).map_err(media_error)?;
    for (index,ordinal) in operation.preview_ordinals.iter().enumerate() {
        let script=write_filter(operation,root,&format!(",select=eq(n\\,{ordinal})"))?;
        let filename=format!("preview-{index:04}.png");let output=frames.join(&filename);
        let mut args=strings(&["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-noautorotate","-i"]);args.push(source.as_os_str().to_owned());
        args.extend(strings(&["-map","0:v:0","-an","-sn","-dn","-filter_script:v"]));args.push(script.as_os_str().to_owned());
        args.extend(strings(&["-frames:v","1","-fps_mode","passthrough","-map_metadata","-1","-codec:v","png","-pix_fmt","rgb24","-threads","1","-n"]));args.push(output.as_os_str().to_owned());run(&settings.ffmpeg,"ffmpeg",&args,settings,root,token)?;
        let observed=inspect_png(&output,operation.source.artifact.width,operation.source.artifact.height,settings.maximum_frame_bytes)?;
        result.push(CaptionPreviewFrame {ordinal:*ordinal,time:period.checked_mul(*ordinal).map_err(media_error)?,filename,sha256:observed.0,byte_count:observed.1});
    }
    crate::destination::local::sync(frames)?;Ok(result)
}
pub(crate) fn deliver(operation:&CaptionOperation,settings:&AssemblyConfiguration,source:&Path,srt:&Path,output:&Path,root:&Path,token:&CancellationToken)->Result<CaptionMediaEvidence> {
    let before=source_observation(operation,settings,source,root,token)?;let copied=operation.request.configuration.mode==CaptionMode::SoftMux;
    let packets=if copied {Some(video_packets(source,0,&before,settings,root,token)?)}else{None};
    let mut args=strings(&["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-noautorotate","-copyts","-i"]);args.push(source.as_os_str().to_owned());
    if copied {args.extend(strings(&["-f","srt","-i"]));args.push(srt.as_os_str().to_owned());}
    args.extend(strings(&["-map","0:v:0"]));if operation.source.artifact.audio.is_some() {args.extend(strings(&["-map","0:a:0","-codec:a","copy"]));}
    if copied {args.extend(strings(&["-map","1:0","-codec:v","copy","-codec:s","mov_text"]));}
    else {
        let script=write_filter(operation,root,"")?;args.extend(strings(&["-sn","-filter_script:v"]));args.push(script.as_os_str().to_owned());
        args.extend(strings(&["-codec:v","libx264","-pix_fmt","yuv420p","-preset"]));args.push(settings.preset.clone().into());args.push("-crf".into());args.push(settings.delivery_crf.to_string().into());
        args.extend(strings(&["-threads","1","-fps_mode","passthrough","-color_primaries","bt709","-color_trc","bt709","-colorspace","bt709","-color_range","tv"]));
    }
    args.extend(strings(&["-dn","-map_metadata","-1","-map_chapters","-1","-avoid_negative_ts","disabled","-use_editlist","1","-movflags","+faststart","-f","mp4","-n"]));args.push(output.as_os_str().to_owned());run(&settings.ffmpeg,"ffmpeg",&args,settings,root,token)?;
    let after=observe(output,&selection(operation,true),settings,root,token,settings.maximum_output_bytes,&GroupPolicy::Inherit)?;admitted_source_video(&after)?;
    crate::temporal::validate_reconstruction(&before.temporal,&after.temporal,copied).map_err(media_error)?;
    let expected_streams=1+usize::from(operation.source.artifact.audio.is_some())+usize::from(copied);
    if array(&after.inventory,"streams")?.len()!=expected_streams||before.audio!=after.audio {return Err(refused("caption candidate changed stream membership or original AAC packets/configuration/timing"));}
    if copied&&packets!=Some(video_packets(output,0,&after,settings,root,token)?){return Err(refused("soft caption mux changed original video packets"));}
    if copied {
        let mut extract=strings(&["-hide_banner","-nostdin","-v","error","-xerror","-protocol_whitelist","file,pipe","-i"]);extract.push(output.as_os_str().to_owned());extract.extend(strings(&["-map","0:s:0","-f","srt","pipe:1"]));
        let bytes=run(&settings.ffmpeg,"ffmpeg",&extract,settings,root,token)?;
        let parsed=crate::timed_text::decode(&bytes,crate::timed_text::TimedTextFormat::Srt,&crate::timed_text::ImportContext::default())?.document;
        let shape=|document:&crate::timed_text::TimedTextDocument|document.cues.iter().map(|c|(c.text.clone(),c.timing.clone())).collect::<Vec<_>>();
        if shape(&parsed)!=shape(&operation.lyrics.document){return Err(refused("muxed captions did not round-trip exact selected words and millisecond intervals"));}
    }
    for field in ["width","height","pix_fmt","sample_aspect_ratio","color_range","color_space","color_primaries","color_transfer","field_order"] {if stream(&before.inventory,0)?.get(field)!=stream(&after.inventory,0)?.get(field){return Err(refused("caption candidate changed a required picture property"));}}
    full_decode(output,operation.source.artifact.audio.is_some(),settings,root,token)?;
    let audio=if let Some(facts)=&after.audio {let interval=after.temporal.selected.as_ref().and_then(|s|s.audio).and_then(|i|after.temporal.timeline(i)).and_then(|t|t.source_time).ok_or_else(||refused("caption audio interval unavailable"))?;
        Some(crate::final_video::FinalAudioProperties {codec:facts.codec.clone(),profile:facts.profile.clone(),sample_rate_hz:facts.sample_rate,channels:facts.channels,channel_layout:facts.channel_layout.clone(),interval,packet_configuration_timing_sha256:digest(facts)?})}else{None};
    let video=stream(&after.inventory,0)?;let fast=faststart(output,settings.maximum_output_bytes)?;if !fast{return Err(refused("caption MP4 is not faststart"));}
    let playback=super::MetadataRemuxPlayback {video_codec:text(video,"codec_name")?.into(),pixel_format:text(video,"pix_fmt")?.into(),color_primaries:text(video,"color_primaries")?.into(),color_transfer:text(video,"color_transfer")?.into(),color_matrix:text(video,"color_space")?.into(),color_range:text(video,"color_range")?.into(),sample_aspect_ratio:text(video,"sample_aspect_ratio")?.into(),orientation_degrees:0,faststart:fast,audio};
    let temporal=after.temporal.video_timeline().map_err(media_error)?;let interval=temporal.source_time.ok_or_else(||refused("caption picture interval missing"))?;
    Ok(CaptionMediaEvidence {source_sha256:before.temporal.source_sha256,output_sha256:after.temporal.source_sha256.clone(),output_byte_count:after.temporal.source_size_bytes,
        width:operation.source.artifact.width,height:operation.source.artifact.height,frame_count:temporal.frames.len() as u64,frame_rate:operation.source.artifact.frame_rate,video_interval:interval,playback,
        video_packets_preserved:copied,pixel_policy:if copied {"encoded_video_packets_verified; no_video_reencode"}else{"explicit_drawtext_burn_in_and_H264_reencode; original_frame_identity_not_asserted"}.into(),authored_subtitle_track:copied,full_decode_observed:true})
}
