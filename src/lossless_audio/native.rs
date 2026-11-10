//! Fixed native adapter; existing process groups/capture and v3 own lifecycle.
use super::*;
use std::{ffi::OsString,io::{Read,Write,Seek,SeekFrom}};
use crate::audio_inspection::{AudioInspectionConfiguration,NativeSampleFormat};
use crate::audio_inspection::process::{run_tool_isolated,GroupPolicy,open_regular};
use crate::frame_io::{copy_pinned,require_absent,ensure_parent};
pub(super) const METHOD:&str="native_container_fields_and_pinned_ffmpeg_full_pcm_sha256; codec_implementation_not_independent";
pub(super) const METADATA:&str="inherited_descriptive_tags_removed; regenerated_encoder_vendor_and_structural_fields_excluded; signature_or_signal_absence_not_asserted";
pub(super) const METER:&str="not_remeasured; original_accepted_measurements_referenced_only_for_identical_pcm";
fn native_encoding(encoding:PcmEncoding)->NativeSampleFormat {match encoding {PcmEncoding::Pcm16=>NativeSampleFormat::Pcm16,PcmEncoding::Pcm24=>NativeSampleFormat::Pcm24,PcmEncoding::Float32=>NativeSampleFormat::Float32}}
pub(super) fn flac_arguments(operation:&LosslessOperation,input:&Path,output:&Path)->Vec<OsString> {
    let native=native_encoding(operation.source.encoding);
    let mut args:Vec<OsString>=["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-i"].into_iter().map(Into::into).collect();
    args.push(input.as_os_str().to_owned());
    args.extend(["-map","0:a:0","-vn","-sn","-dn","-map_metadata","-1","-map_chapters","-1","-threads","1","-fflags","+bitexact","-flags:a","+bitexact","-codec:a"].into_iter().map(OsString::from));
    args.extend(["flac","-sample_fmt",native.sample_format(),"-bits_per_raw_sample"].into_iter().map(OsString::from));args.push(native.bits_per_sample().to_string().into());args.extend(["-compression_level","8","-f","flac"].into_iter().map(OsString::from));
    args.push("-n".into());args.push(output.as_os_str().to_owned());args
}
/// Rewrap the already validated native interleaved samples in minimal RIFF32;
/// avoid implicit WAVEFORMATEXTENSIBLE or sample conversion by a codec default.
pub(super) fn wave_export(input:&Path,output:&Path,operation:&LosslessOperation,token:&CancellationToken)->Result<()> {
    let wave=crate::audio_inspection::wav::inspect_native(input)?;
    write_wave_samples(input,output,&wave,operation.configuration.maximum_output_bytes,token)
}
fn write_wave_samples(input:&Path,output:&Path,wave:&crate::audio_inspection::wav::PcmWave,maximum:u64,token:&CancellationToken)->Result<()> {
    let float=wave.sample_format==NativeSampleFormat::Float32;
    let data=u32::try_from(wave.data_bytes).map_err(|_|invalid("WAV export exceeds RIFF32"))?;
    let size=36u32.checked_add(if float {12}else{0}).and_then(|n|n.checked_add(data)).and_then(|n|n.checked_add(data%2)).ok_or_else(||invalid("WAV output size overflow"))?;
    if u64::from(size)+8>maximum{return Err(invalid("WAV native samples exceed selected output bound"));}
    let mut header=b"RIFF".to_vec();header.extend(size.to_le_bytes());header.extend(b"WAVEfmt ");header.extend(16u32.to_le_bytes());header.extend((if float {3u16}else{1u16}).to_le_bytes());
    let align=wave.channels*wave.sample_format.bytes_per_sample();header.extend(wave.channels.to_le_bytes());header.extend(wave.sample_rate_hz.to_le_bytes());header.extend((wave.sample_rate_hz*u32::from(align)).to_le_bytes());header.extend(align.to_le_bytes());header.extend(wave.sample_format.bits_per_sample().to_le_bytes());
    if float {header.extend(b"fact");header.extend(4u32.to_le_bytes());header.extend(u32::try_from(wave.frame_count).map_err(|_|invalid("WAV fact sample count exceeds profile"))?.to_le_bytes());}
    header.extend(b"data");header.extend(data.to_le_bytes());
    let mut source=open_regular(input).map_err(|_|invalid("cannot open native PCM snapshot"))?;source.seek(SeekFrom::Start(wave.data_offset)).map_err(|_|invalid("cannot locate native PCM samples"))?;
    let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut target=options.open(output).map_err(|_|invalid("cannot reserve native WAV candidate"))?;target.write_all(&header).map_err(|_|invalid("cannot write native WAV header"))?;
    let mut remaining=wave.data_bytes;let mut buffer=[0u8;65_536];
    while remaining>0 {cancelled(token)?;let count=remaining.min(buffer.len() as u64) as usize;read(&mut source,&mut buffer[..count])?;target.write_all(&buffer[..count]).map_err(|_|invalid("cannot write exact native PCM"))?;remaining-=count as u64;}
    if data%2!=0 {target.write_all(&[0]).map_err(|_|invalid("cannot finish WAV alignment padding"))?;}
    target.sync_all().map_err(|_|invalid("cannot sync native WAV candidate"))?;Ok(())
}
fn tool(tools:&AudioInspectionConfiguration,id:&str,args:&[OsString],root:&Path,token:&CancellationToken)->Result<Vec<u8>> {
    let pin=if id=="ffmpeg" {&tools.ffmpeg}else{&tools.ffprobe};
    run_tool_isolated(pin,id,args,tools,token,GroupPolicy::Inherit,Some(root)).map(|capture|capture.stdout)
        .map_err(|_|invalid("bounded lossless native tool refused, failed or exceeded its limit; retain private work"))
}
fn native_probe(tools:&AudioInspectionConfiguration,path:&Path,operation:&LosslessOperation,root:&Path,token:&CancellationToken)->Result<()> {
    let mut args:Vec<OsString>=["-v","error","-protocol_whitelist","file,pipe","-show_entries","stream=index,codec_type,codec_name,sample_rate,channels,bits_per_raw_sample,bits_per_sample:format=format_name,nb_streams","-of","json","-i"].into_iter().map(Into::into).collect();args.push(path.as_os_str().to_owned());
    let value:serde_json::Value=crate::provider::decode_json(&tool(tools,"ffprobe",&args,root,token)?,"lossless native probe")?;
    let streams=value["streams"].as_array().ok_or_else(||invalid("lossless probe streams missing"))?;
    if streams.len()!=1||value["format"]["nb_streams"].as_u64()!=Some(1){return Err(invalid("lossless candidate must contain exactly one audio stream"));}
    let stream=&streams[0];let source=&operation.source.summary.source;let codec=if operation.configuration.format==LosslessFormat::Flac {"flac"}else{native_encoding(operation.source.encoding).codec()};
    let rate=stream["sample_rate"].as_str().and_then(|s|s.parse::<u32>().ok());
    let format=if operation.configuration.format==LosslessFormat::Flac {"flac"}else{"wav"};
    if stream["index"].as_u64()!=Some(0)||stream["codec_type"]!="audio"||stream["codec_name"]!=codec||rate!=Some(source.sample_rate_hz)
        ||stream["channels"].as_u64()!=Some(u64::from(source.channels))||value["format"]["format_name"]!=format {return Err(invalid("lossless candidate stream, codec, rate or channels changed"));}Ok(())
}
fn decoded_digest(tools:&AudioInspectionConfiguration,path:&Path,encoding:PcmEncoding,root:&Path,token:&CancellationToken)->Result<String> {
    let mut args:Vec<OsString>=["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-i"].into_iter().map(Into::into).collect();args.push(path.as_os_str().to_owned());
    args.extend(["-map","0:a:0","-vn","-sn","-dn","-threads","1","-codec:a",native_encoding(encoding).codec(),"-f","hash","-hash","sha256","pipe:1"].into_iter().map(OsString::from));
    let bytes=tool(tools,"ffmpeg",&args,root,token)?;let text=std::str::from_utf8(&bytes).map_err(|_|invalid("lossless decoded digest is not UTF-8"))?;
    let digest=text.trim().strip_prefix("SHA256=").ok_or_else(||invalid("lossless decode did not emit its full PCM hash"))?;
    crate::provider::require_sha256(digest,"lossless decoded PCM")?;Ok(digest.into())
}
fn read(input:&mut impl Read,bytes:&mut [u8])->Result<()> {input.read_exact(bytes).map_err(|_|invalid("truncated lossless container"))}
fn little(data:&[u8],position:&mut usize)->Result<u32> {
    let bytes=data.get(*position..*position+4).ok_or_else(||invalid("truncated FLAC comment length"))?;*position+=4;Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
fn comment_block(data:&[u8])->Result<()> {
    let mut position=0;let size=little(data,&mut position)? as usize;
    let vendor=data.get(position..position+size).ok_or_else(||invalid("FLAC vendor exceeds its block"))?;position+=size;
    if vendor.len()>256||vendor.iter().any(|b|!b.is_ascii()||b.is_ascii_control()){return Err(invalid("unsupported generated FLAC vendor field"));}
    let count=little(data,&mut position)?;if count>1{return Err(invalid("FLAC contains unexpected descriptive comment fields"));}
    for _ in 0..count {
        let size=little(data,&mut position)? as usize;let bytes=data.get(position..position+size).ok_or_else(||invalid("FLAC comment exceeds its block"))?;position+=size;
        let value=std::str::from_utf8(bytes).map_err(|_|invalid("FLAC comment is not UTF-8"))?;let Some((key,encoder))=value.split_once('=')else{return Err(invalid("invalid generated FLAC encoder field"));};
        if !key.eq_ignore_ascii_case("encoder")||encoder.len()>256||!encoder.bytes().all(|b|b.is_ascii_alphanumeric()||b" ._-".contains(&b)) {return Err(invalid("FLAC contains an unrequested descriptive tag"));}
    }
    if position!=data.len(){return Err(invalid("FLAC comment block contains trailing data"));}Ok(())
}
/// Independent bounded header/metadata inspection; full decode is separately
/// observed by the pinned codec, never inferred from these declared header bits.
pub(super) fn container(path:&Path,operation:&LosslessOperation)->Result<()> {
    let source=&operation.source.summary.source;let encoding=native_encoding(operation.source.encoding);
    let mut input=open_regular(path).map_err(|_|invalid("lossless candidate is not regular"))?;
    if operation.configuration.format==LosslessFormat::Wav {
        let wave=crate::audio_inspection::wav::inspect_native(path)?;
        if wave.sample_format!=encoding||wave.sample_rate_hz!=source.sample_rate_hz||wave.channels!=source.channels||wave.frame_count!=source.frame_count||wave.pcm_sha256!=operation.source.pcm_sha256 {return Err(invalid("WAV export changed exact native sample bytes or clock"));}
        let size=identity(path,operation.configuration.maximum_output_bytes)?.1;let mut header=[0u8;12];read(&mut input,&mut header)?;let mut offset=12;
        while offset<size {
            let mut head=[0u8;8];read(&mut input,&mut head)?;let count=u32::from_le_bytes(head[4..].try_into().unwrap()) as u64;offset+=8;
            if !matches!(&head[..4],b"fmt "|b"data"|b"fact"|b"JUNK"){return Err(invalid("WAV export retained an unrequested container metadata chunk"));}
            if &head[..4]==b"JUNK" {if count>1_048_576{return Err(invalid("WAV padding exceeds its bound"));}let mut pad=vec![0u8;count as usize];read(&mut input,&mut pad)?;if pad.iter().any(|b|*b!=0){return Err(invalid("WAV padding is not zero"));}}
            else {input.seek(SeekFrom::Current(count as i64)).map_err(|_|invalid("cannot seek bounded WAV chunk"))?;}
            if count%2!=0 {let mut pad=[0u8;1];read(&mut input,&mut pad)?;}offset+=count+count%2;
        }
        return Ok(());
    }
    let mut magic=[0u8;4];read(&mut input,&mut magic)?;if &magic!=b"fLaC" {return Err(invalid("FLAC signature missing"));}
    let mut seen=false;let mut total=0usize;
    for index in 0..64 {
        let mut head=[0u8;4];read(&mut input,&mut head)?;let kind=head[0]&0x7f;let size=u32::from_be_bytes([0,head[1],head[2],head[3]]) as usize;total+=size;
        if size>1_048_576||total>8_388_608{return Err(invalid("FLAC metadata exceeds the finite bound"));}
        let mut data=vec![0u8;size];read(&mut input,&mut data)?;
        match kind {
            0=>{
                if seen||index!=0||size!=34{return Err(invalid("FLAC must begin with exactly one STREAMINFO"));}seen=true;
                let packed=u64::from_be_bytes(data[10..18].try_into().unwrap());
                if (packed>>44) as u32!=source.sample_rate_hz||((packed>>41)&7)+1!=u64::from(source.channels)
                    ||((packed>>36)&31)+1!=u64::from(encoding.bits_per_sample())||(packed&0x0000_000f_ffff_ffff)!=source.frame_count {return Err(invalid("FLAC source rate, channels, precision or complete sample count differs"));}
            },
            1 if data.iter().all(|b|*b==0)=>{},
            3 if size%18==0=>{},
            4=>comment_block(&data)?,
            _=>return Err(invalid("FLAC contains unrequested/unsupported metadata or padding")),
        }
        if head[0]&0x80!=0 {if !seen{return Err(invalid("FLAC STREAMINFO missing"));}return Ok(());}
    }
    Err(invalid("FLAC metadata inventory exceeds 64 blocks"))
}
pub(crate) fn execute_invocation(request:&crate::ProviderInvocationRequest)->Result<()> {
    request.validate()?;let operation:LosslessOperation=serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|_|invalid("invalid lossless operation"))?).map_err(|_|invalid("invalid closed lossless operation"))?;operation.validate()?;
    if !cfg!(unix)||request.configuration!=operation.provider_configuration()?||request.stage_id!="lossless_audio"||request.inputs.len()!=1||request.outputs.len()!=2{return Err(invalid("invocation is outside the fixed Unix lossless profile"));}
    let source=&request.inputs[0];
    if source.port!="source"||source.artifact_id!="source_audio"||source.kind!=crate::ArtifactKind::File||source.artifact_type!="audio/wav"||source.artifact_role!=crate::ArtifactRole::TemporalSource||source.stream_role!=Some(crate::StreamRole::Audio){return Err(invalid("lossless source port differs"));}
    let select=|port:&str,id:&str,kind:&str,role:crate::ArtifactRole,stream:crate::StreamRole|->Result<&Path>{let outputs=request.outputs.iter().filter(|v|v.port==port).collect::<Vec<_>>();if outputs.len()!=1{return Err(invalid("lossless output missing/ambiguous"));}let output=outputs[0];
        if output.artifact_id!=id||output.kind!=crate::ArtifactKind::File||output.artifact_type!=kind||output.artifact_role!=role||output.stream_role!=Some(stream){return Err(invalid("lossless output port differs"));}Ok(&output.path)};
    let candidate=select("media","lossless_delivery",operation.configuration.format.media_type(),crate::ArtifactRole::CandidateMaster,crate::StreamRole::Audio)?;
    let evidence=select("evidence","lossless_evidence","application/vnd.aniflow.lossless-audio-report+json",crate::ArtifactRole::ValidationEvidence,crate::StreamRole::TimedMetadata)?;
    let parent=candidate.parent().ok_or_else(||invalid("lossless output parent missing"))?;
    if evidence.parent()!=Some(parent)||candidate==evidence{return Err(invalid("lossless outputs must be distinct siblings"));}
    for output in [candidate,evidence] {require_absent(output)?;for input in [&source.path,&operation.configuration.tools.ffmpeg.executable,&operation.configuration.tools.ffprobe.executable] {if output.starts_with(input)||input.starts_with(output){return Err(invalid("lossless outputs overlap source/tool bytes"));}}}
    ensure_parent(parent)?;crate::video_assembly::require_storage(parent,operation.working_bytes()?,operation.configuration.minimum_free_bytes)?;
    let token=CancellationToken::default();let source_identity=(operation.source.summary.source.artifact.sha256.clone(),operation.source.summary.source.artifact.byte_size);
    if identity(&source.path,MEDIA_BYTES)?!=source_identity{return Err(invalid("lossless source changed"));}
    let tools=observe_tools(&operation.configuration)?;let private=tempfile::Builder::new().prefix(".lossless-audio-").tempdir_in(parent).map_err(|_|invalid("cannot reserve private lossless work"))?;
    let root=private.path().canonicalize().map_err(|_|invalid("cannot resolve lossless work"))?;
    let result:Result<()>=(||{
        let mut settings=operation.configuration.tools.clone();settings.ffmpeg.executable=root.join("ffmpeg");settings.ffprobe.executable=root.join("ffprobe");
        for (pin,destination,expected) in [(&operation.configuration.tools.ffmpeg,&settings.ffmpeg.executable,&tools[0]),(&operation.configuration.tools.ffprobe,&settings.ffprobe.executable,&tools[1])] {
            copy_pinned(&pin.executable,destination,expected,TOOL_BYTES,&token)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(destination,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot retain private tool mode"))?;}
        }
        for id in ["ffmpeg","ffprobe"] {let bytes=tool(&settings,id,&[OsString::from("-version")],&root,&token)?;let version=std::str::from_utf8(&bytes).map_err(|_|invalid("native version is not UTF-8"))?;
            if !version.lines().next().is_some_and(|line|line.split_whitespace().take(3).eq([id,"version","8.0.1"])) {return Err(invalid("native lossless tool did not report exact version 8.0.1"));}}
        let snapshot=root.join("source.wav");copy_pinned(&source.path,&snapshot,&source_identity,MEDIA_BYTES,&token)?;
        let wave=crate::audio_inspection::wav::inspect_native(&snapshot)?;let original=&operation.source.summary.source;
        if wave.sample_format!=native_encoding(operation.source.encoding)||wave.pcm_sha256!=operation.source.pcm_sha256||wave.sample_rate_hz!=original.sample_rate_hz||wave.channels!=original.channels||wave.frame_count!=original.frame_count{return Err(invalid("native source differs from the exact lossless operation"));}
        let output=root.join(operation.configuration.format.filename());
        if operation.configuration.format==LosslessFormat::Wav {wave_export(&snapshot,&output,&operation,&token)?;}
        else {tool(&settings,"ffmpeg",&flac_arguments(&operation,&snapshot,&output),&root,&token)?;}
        let output_identity=identity(&output,operation.configuration.maximum_output_bytes)?;container(&output,&operation)?;native_probe(&settings,&output,&operation,&root,&token)?;
        let decoded_pcm_sha256=decoded_digest(&settings,&output,operation.source.encoding,&root,&token)?;
        if decoded_pcm_sha256!=operation.source.pcm_sha256{return Err(invalid("lossless candidate decoded samples differ; no export can be accepted"));}
        if identity(&source.path,MEDIA_BYTES)?!=source_identity||identity(&output,operation.configuration.maximum_output_bytes)?!=output_identity||observe_tools(&operation.configuration)?!=tools{return Err(invalid("lossless source, candidate or configured tool changed during execution"));}
        for (pin,expected) in [(&settings.ffmpeg,&tools[0]),(&settings.ffprobe,&tools[1])] {if identity(&pin.executable,TOOL_BYTES)?!=*expected{return Err(invalid("private lossless tool changed during use"));}}
        let report=LosslessReport {schema:"aniflow.lossless-audio-report/v1".into(),operation:operation.clone(),provider_lock_sha256:request.provider_lock_sha256.clone(),output_sha256:output_identity.0.clone(),output_byte_count:output_identity.1,decoded_pcm_sha256,
            sample_rate_hz:original.sample_rate_hz,channels:original.channels,frame_count:original.frame_count,encoding:operation.source.encoding,verification_method:METHOD.into(),signal_changes:"none".into(),metadata_scope:METADATA.into(),after_export_meter:METER.into(),native_qualified:false,creative_approval:"not_performed".into(),publication_authority:"none".into()};
        local::write(evidence,&local::bytes(&report)?)?;copy_pinned(&output,candidate,&output_identity,operation.configuration.maximum_output_bytes,&token)?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(candidate,std::fs::Permissions::from_mode(0o600)).map_err(|_|invalid("cannot retain private audio mode"))?;}
        std::fs::File::open(candidate).and_then(|f|f.sync_all()).map_err(|_|invalid("cannot sync private audio candidate"))?;local::sync(parent)?;Ok(())
    })();
    if let Err(error)=&result {let kept=private.keep();let _=local::write(&kept.join("failure.json"),&local::bytes(&serde_json::json!({"schema":"aniflow.lossless-audio-failure/v1","category":error.category(),"accepted":false,"cause":"unproven"}))?);}
    result
}

/// Decode an accepted final FLAC to its complete original native integer
/// representation. The minimal WAV wrapper avoids a codec-default extensible
/// header and makes the existing independent native parser the sample owner.
pub(crate) fn decode_for_qc(report:&LosslessReport,input:&Path,output:&Path,root:&Path,tools:&AudioInspectionConfiguration,maximum:u64,token:&CancellationToken)->Result<()> {
    if report.operation.configuration.format!=LosslessFormat::Flac||report.encoding==PcmEncoding::Float32{return Err(invalid("final FLAC QC requires accepted integer native export"));}
    let native=native_encoding(report.encoding);let bytes=report.frame_count.checked_mul(u64::from(report.channels)*u64::from(native.bytes_per_sample())).ok_or_else(||invalid("final decoded PCM bound overflow"))?;
    if bytes.checked_add(4096).is_none_or(|v|v>maximum){return Err(invalid("complete final FLAC decode exceeds selected WAV bound"));}
    let raw=root.join("decoded.pcm");require_absent(&raw)?;require_absent(output)?;
    let arguments=qc_decode_arguments(input,&raw,native,bytes.checked_add(4096).ok_or_else(||invalid("raw PCM bound overflow"))?);
    let capture=run_tool_isolated(&tools.ffmpeg,"ffmpeg",&arguments,tools,token,GroupPolicy::Own,Some(root)).map_err(|_|invalid("complete final FLAC native decode failed/cancelled or exceeded capture bounds"))?;
    if !capture.stdout.is_empty()||!capture.stderr.is_empty()||identity(&raw,maximum)?!=(report.decoded_pcm_sha256.clone(),bytes){return Err(invalid("final FLAC native decode changed exact complete sample bytes"));}
    let wave=crate::audio_inspection::wav::PcmWave {sample_format:native,sample_rate_hz:report.sample_rate_hz,channels:report.channels,frame_count:report.frame_count,pcm_sha256:report.decoded_pcm_sha256.clone(),data_offset:0,data_bytes:bytes};
    write_wave_samples(&raw,output,&wave,maximum,token)
}

pub(super) fn qc_decode_arguments(input:&Path,raw:&Path,native:NativeSampleFormat,maximum:u64)->Vec<OsString> {
    let mut arguments:Vec<OsString>=["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-i"].into_iter().map(Into::into).collect();arguments.push(input.as_os_str().to_owned());
    arguments.extend(["-map","0:a:0","-vn","-sn","-dn","-map_metadata","-1","-map_chapters","-1","-codec:a",native.codec(),"-flags:a","+bitexact","-fflags","+bitexact","-threads:a","1","-f",if native==NativeSampleFormat::Pcm16 {"s16le"}else{"s24le"},"-fs"].into_iter().map(OsString::from));
    arguments.push(maximum.to_string().into());arguments.push("-n".into());arguments.push(raw.as_os_str().to_owned());
    arguments
}
