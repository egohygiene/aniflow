use super::*;
use std::{ffi::OsString,io::{Read,Seek,SeekFrom}};
use crate::audio_inspection::process::{GroupPolicy,open_regular,run_tool_isolated_with_limits};
use crate::frame_io::{copy_pinned,ensure_parent,require_absent};

pub(super) fn arguments(operation:&Operation,input:&Path,output:&Path)->Vec<OsString> {
    let filter=filter(operation.gain_milli_db);
    let mut a:Vec<OsString>=["-hide_banner","-nostdin","-v","error","-xerror","-err_detect","explode","-threads","1","-protocol_whitelist","file,pipe","-i"].into_iter().map(Into::into).collect();a.push(input.as_os_str().to_owned());
    a.extend(["-map","0:a:0","-vn","-sn","-dn","-map_metadata","-1","-map_chapters","-1","-af"].into_iter().map(OsString::from));a.push(filter.into());
    a.extend(["-codec:a","pcm_f32le","-flags:a","+bitexact","-fflags","+bitexact","-threads:a","1","-f","f32le","-fs"].into_iter().map(OsString::from));a.push(operation.request.configuration.maximum_output_bytes.to_string().into());a.push("-n".into());a.push(output.as_os_str().to_owned());a
}
pub(super) fn filter(gain:i32)->String {
    let sign=if gain<0 {"-"}else{""};let magnitude=gain.unsigned_abs();
    format!("volume={sign}{}.{:03}dB:precision=double",magnitude/1000,magnitude%1000)
}

fn finite_samples(path:&Path,offset:u64,bytes:u64)->Result<()> {
    if bytes%4!=0{return Err(invalid("gain float sample bytes are incomplete"));}
    let mut f=open_regular(path).map_err(|_|invalid("gain float data unavailable"))?;f.seek(SeekFrom::Start(offset)).map_err(|_|invalid("gain sample offset unavailable"))?;
    let mut remaining=bytes;let mut buffer=[0u8;65_536];
    while remaining>0 {let n=remaining.min(buffer.len() as u64) as usize;f.read_exact(&mut buffer[..n]).map_err(|_|invalid("gain samples truncated"))?;
        for sample in buffer[..n].chunks_exact(4) {let value=f32::from_le_bytes(sample.try_into().unwrap());if !value.is_finite()||value.abs()>1.0{return Err(invalid("gain candidate contains nonfinite or over-full-scale samples"));}}
        remaining-=n as u64;
    }Ok(())
}

pub(super) fn require_candidate(path:&Path,operation:&Operation)->Result<()> {
    let wave=crate::audio_inspection::wav::inspect_native(path)?;
    if wave.sample_format!=NativeSampleFormat::Float32||wave.sample_rate_hz!=operation.source.sample_rate_hz||wave.channels!=operation.source.channels||wave.frame_count!=operation.source.frame_count {
        return Err(invalid("gain candidate changed sample clock/channels/complete extent or float encoding"));
    }
    identity(path,operation.request.configuration.maximum_output_bytes)?;
    finite_samples(path,wave.data_offset,wave.data_bytes)
}

pub(crate) fn execute_invocation(inv:&crate::ProviderInvocationRequest)->Result<()> {
    inv.validate()?;
    let operation:Operation=serde_json::from_value(serde_json::to_value(&inv.configuration.values).map_err(|_|invalid("gain operation encoding"))?).map_err(|_|invalid("closed gain operation required"))?;
    operation.validate()?;
    if !cfg!(unix)||inv.configuration!=operation.configuration()?||inv.stage_id!="audio_gain"||inv.inputs.len()!=1||inv.outputs.len()!=2{return Err(invalid("outside fixed gain native profile"));}
    let input=&inv.inputs[0];
    if input.port!="source"||input.artifact_id!="gain_source"||input.kind!=crate::ArtifactKind::File||input.artifact_type!="audio/wav"||input.artifact_role!=crate::ArtifactRole::TemporalSource||input.stream_role!=Some(crate::StreamRole::Audio){return Err(invalid("gain source port differs"));}
    let select=|port:&str,id:&str,kind:&str,role:crate::ArtifactRole,stream:crate::StreamRole|->Result<&Path>{let v=inv.outputs.iter().filter(|o|o.port==port).collect::<Vec<_>>();if v.len()!=1{return Err(invalid("gain output missing/ambiguous"));}let o=v[0];if o.artifact_id!=id||o.kind!=crate::ArtifactKind::File||o.artifact_type!=kind||o.artifact_role!=role||o.stream_role!=Some(stream){return Err(invalid("gain output contract differs"));}Ok(&o.path)};
    let candidate=select("media","gain_audio","audio/wav",crate::ArtifactRole::CandidateMaster,crate::StreamRole::Audio)?;
    let evidence=select("evidence","gain_evidence","application/vnd.aniflow.audio-gain-report+json",crate::ArtifactRole::ValidationEvidence,crate::StreamRole::TimedMetadata)?;
    let parent=candidate.parent().ok_or_else(||invalid("gain output parent missing"))?;
    if evidence.parent()!=Some(parent)||candidate==evidence{return Err(invalid("gain outputs must be distinct siblings"));}
    ensure_parent(parent)?;
    for output in [candidate,evidence] {require_absent(output)?;for original in std::iter::once(&input.path).chain(operation.components.iter().map(|c|&c.path)) {if output.starts_with(original)||original.starts_with(output){return Err(invalid("gain output overlaps source or component"));}}}
    let original=(operation.source.pcm.sha256.clone(),operation.source.pcm.byte_count);
    if identity(&input.path,MEDIA_BYTES)?!=original||components(&operation.request)?!=operation.components{return Err(invalid("gain source/components changed before execution"));}
    let renderer=std::env::current_exe().map_err(|_|invalid("gain provider executable unavailable"))?;
    if identity(&renderer,TOOL_BYTES)?.0!=operation.components[2].sha256{return Err(invalid("gain actual provider differs from selected pin"));}
    crate::video_assembly::require_storage(parent,operation.working_bytes()?,operation.request.configuration.minimum_free_bytes)?;
    let private=tempfile::Builder::new().prefix(".gain-native-").tempdir_in(parent).map_err(|_|invalid("cannot reserve private gain work"))?;
    let root=private.path().canonicalize().map_err(|_|invalid("cannot resolve private gain work"))?;let token=CancellationToken::new();
    let result:Result<()>=(||{
        let mut settings=operation.request.configuration.tools.clone();settings.ffmpeg.executable=root.join("ffmpeg");settings.ffprobe.executable=root.join("ffprobe");
        for (component,destination) in operation.components[..2].iter().zip([&settings.ffmpeg.executable,&settings.ffprobe.executable]) {
            copy_pinned(&component.path,destination,&(component.sha256.clone(),component.byte_count),TOOL_BYTES,&token)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(destination,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot enable private gain tool"))?;}
        }
        for (id,pin) in [("ffmpeg",&settings.ffmpeg),("ffprobe",&settings.ffprobe)] {
            let v=run_tool_isolated_with_limits(pin,id,&["-version".into()],&settings,&token,GroupPolicy::Inherit,Some(&root),settings.tool_timeout_milliseconds,settings.maximum_tool_output_bytes).map_err(|_|invalid("gain tool version preflight failed"))?;
            let text=std::str::from_utf8(&v.stdout).map_err(|_|invalid("gain version text invalid"))?;let words=text.lines().next().unwrap_or("").split_whitespace().collect::<Vec<_>>();
            if !v.stderr.is_empty()||words.len()<3||words[0]!=id||words[1]!="version"||words[2]!=pin.version{return Err(invalid("gain exact native tool version differs"));}
        }
        let source=root.join("source.wav");copy_pinned(&input.path,&source,&original,MEDIA_BYTES,&token)?;
        let source_wave=crate::audio_inspection::wav::inspect_native(&source)?;
        if source_wave.pcm_sha256!=operation.source.pcm_sha256||source_wave.sample_format!=operation.source.encoding||source_wave.sample_rate_hz!=operation.source.sample_rate_hz||source_wave.channels!=operation.source.channels||source_wave.frame_count!=operation.source.frame_count{return Err(invalid("gain source PCM clock/encoding differs"));}
        let raw=root.join("gain.pcm");let v=run_tool_isolated_with_limits(&settings.ffmpeg,"ffmpeg",&arguments(&operation,&source,&raw),&settings,&token,GroupPolicy::Inherit,Some(&root),settings.tool_timeout_milliseconds,settings.maximum_tool_output_bytes).map_err(|_|invalid("gain processor failed/cancelled/exceeded bounds"))?;
        if !v.stdout.is_empty()||!v.stderr.is_empty(){return Err(invalid("gain processor returned unexpected output"));}
        let pin=identity(&raw,operation.request.configuration.maximum_output_bytes)?;
        let count=operation.source.frame_count.checked_mul(u64::from(operation.source.channels)*4).ok_or_else(||invalid("gain sample count overflow"))?;
        if pin.1!=count{return Err(invalid("gain changed complete PCM extent"));}finite_samples(&raw,0,count)?;
        let wave=crate::audio_inspection::wav::PcmWave {sample_format:NativeSampleFormat::Float32,sample_rate_hz:operation.source.sample_rate_hz,channels:operation.source.channels,frame_count:operation.source.frame_count,pcm_sha256:pin.0.clone(),data_offset:0,data_bytes:count};
        let output=root.join("audio.wav");crate::lossless_audio::write_native_wave_samples(&raw,&output,&wave,operation.request.configuration.maximum_output_bytes,&token)?;
        require_candidate(&output,&operation)?;let file=identity(&output,operation.request.configuration.maximum_output_bytes)?;
        if identity(&input.path,MEDIA_BYTES)?!=original||components(&operation.request)?!=operation.components{return Err(invalid("gain source/components changed during native work"));}
        copy_pinned(&output,candidate,&file,operation.request.configuration.maximum_output_bytes,&token)?;
        let report=Report {schema:"aniflow.audio-gain-report/v1".into(),operation:operation.clone(),provider_lock_sha256:inv.provider_lock_sha256.clone(),output_sha256:file.0,output_bytes:file.1,output_pcm_sha256:pin.0,sample_rate_hz:wave.sample_rate_hz,channels:wave.channels,frame_count:wave.frame_count,output_policy_status:"requires_fresh_exact_file_qc".into(),runtime_qualified:false,creative_approval:"not_performed".into(),publication_authority:"none".into()};
        local::write(evidence,&local::bytes(&report)?)?;local::sync(parent)
    })();
    if result.is_err(){let _retained=private.keep();}result
}
