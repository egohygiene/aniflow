use super::*;
pub fn run<F:FnMut(&crate::PipelineV3RunProgress)>(request:&Request,root:&Path,token:&CancellationToken,progress:F)->Result<Status> {
    let i=plan(request,token)?;let parent=local::parent(root)?;disjoint(root,&i)?;crate::video_assembly::require_storage(parent,i.projected_working_bytes,request.qc.configuration.minimum_free_bytes)?;
    local::directory(root)?;local::directory(&area(root))?;local::directory(&area(root).join("children"))?;local::write(&root.join("resume-intent.json"),&local::bytes(&i)?)?;drive(root,token,progress)
}
pub fn resume<F:FnMut(&crate::PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,progress:F)->Result<Status>{retained(root,token)?;drive(root,token,progress)}
fn drive<F:FnMut(&crate::PipelineV3RunProgress)>(root:&Path,token:&CancellationToken,mut progress:F)->Result<Status> {
    let _writer=crate::cache_v3::ExclusiveWriter::acquire(root,"writer.lock")?;let i=retained(root,token)?;let current=status(root,token)?;if current.complete{return Ok(current);}
    let _source=crate::cache_v3::ExclusiveWriter::acquire(&i.request.qc.source_run_directory,"writer.lock")?;
    for _ in 0..6 {
        cancelled(token)?;let current=status(root,token)?;if current.complete{return Ok(current);}
        let config=&i.request.qc.configuration;crate::video_assembly::require_storage(root,i.projected_working_bytes,config.minimum_free_bytes)?;
        match current.phase {
            Phase::Capture=>{
                copy_pinned(&i.native.artifact.locator,&area(root).join("source.media"),&(i.native.artifact.sha256.clone(),i.native.artifact.byte_count),config.maximum_source_bytes,token)?;
                for (c,name) in i.components.iter().zip(["ffmpeg","ffprobe","aniflow-provider"]) {let path=area(root).join(name);copy_pinned(&c.path,&path,&(c.sha256.clone(),c.byte_count),536_870_912,token)?;
                    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot enable private video QC component"))?;}}
                snapshots(root,&i)?;if retained(root,token)?!=i{return Err(invalid("final video changed during capture"));}cancelled(token)?;local::write(&root.join("capture.json"),&local::bytes(&capture_marker(&i))?)?;
            },
            Phase::Decode=>{
                let c=private(root,&i);let a=area(root);let source=a.join("source.media");let output=a.join("selected-audio.wav");
                let (inspection,channels)=crate::video_assembly::media::observe_meter_source(&c,&source,&i.native.signal_request.selection,&a,token)?;
                let (stream,range,frames)=source_signal::extent(&inspection)?;let audio=i.native.artifact.audio.as_ref().ok_or_else(||invalid("accepted final audio missing"))?;
                if range!=audio.interval||u64::from(channels)!=audio.channels||stream!=1{return Err(invalid("probe differs from exact accepted final audio extent/channels/stream"));}
                let arguments=source_signal::decode_arguments(&source,&output,stream,i.request.qc.maximum_pcm_bytes);
                let captured=crate::audio_inspection::process::run_tool_isolated_with_limits(&c.ffmpeg,"ffmpeg",&arguments,token,crate::audio_inspection::process::GroupPolicy::Own,Some(&a),c.timeout_milliseconds,c.maximum_capture_bytes as u64).map_err(|_|invalid("final audio decode failed/cancelled/exceeded finite capture"))?;
                if !captured.stdout.is_empty()||!captured.stderr.is_empty(){return Err(invalid("final audio decoder returned unexpected diagnostics"));}
                let wav=crate::audio_inspection::wav::inspect_native(&output)?;if wav.frame_count!=frames{return Err(invalid("complete final audio was truncated or padded"));}
                let pin=identity(&output,i.request.qc.maximum_pcm_bytes)?;
                let d=Decoded {schema:"aniflow.resumable-final-video-audio-qc-decoded/v1".into(),intent_sha256:i.intent_sha256.clone(),inspection,channels,wav_sha256:pin.0,wav_bytes:pin.1,pcm_sha256:wav.pcm_sha256,
                    plan:crate::audio_signal::plan_read_only(&analysis(root,&i),token)?};decoded(root,&i,&d,token)?;snapshots(root,&i)?;
                if retained(root,token)?!=i{return Err(invalid("current final video changed during complete decode"));}cancelled(token)?;local::write(&root.join("decoded.json"),&local::bytes(&d)?)?;
            },
            Phase::NativeSignal=>{
                let d:Decoded=local::read(&root.join("decoded.json"))?;
                if present(&child(root))?{crate::audio_signal::resume(analysis(root,&i),child(root),token,&mut progress).map_err(|e|e.error)?;}
                else{crate::audio_signal::run_at(analysis(root,&i),&child(root),&d.plan,token,&mut progress)?;}
            },
            Phase::Meter=>{
                let _child=crate::cache_v3::ExclusiveWriter::acquire(&child(root),"writer.lock")?;if status(root,token)?!=current{return Err(invalid("native final audio changed before meter"));}
                let d:Decoded=local::read(&root.join("decoded.json"))?;
                if !present(&root.join("meter.json"))? {let m=measure(root,&i,&d,token)?;meter_report(root,&i,&d,&m,token)?;if retained(root,token)?!=i{return Err(invalid("final video changed during meter"));}cancelled(token)?;local::write(&root.join("meter.json"),&local::bytes(&m)?)?;}
                let m:Meter=local::read(&root.join("meter.json"))?;let report=meter_report(root,&i,&d,&m,token)?;
                if !present(&area(root).join("video-audio-signal.json"))? {cancelled(token)?;local::write(&area(root).join("video-audio-signal.json"),&local::bytes(&m.signal)?)?;}
                // Recheck any already-published signal marker before the final
                // receipt. Unknown or half-written publication stays refused.
                status(root,token)?;if retained(root,token)?!=i{return Err(invalid("final video changed before final QC receipt"));}cancelled(token)?;local::write(&root.join("qc.json"),&local::bytes(&report)?)?;
            },
            Phase::Complete=>return Ok(current),
        }
    }Err(invalid("video QC exceeded its finite restart phase count"))
}
fn measure(root:&Path,i:&Intent,d:&Decoded,token:&CancellationToken)->Result<Meter> {
    let accepted=crate::status_v3(&child(root))?;let summary=crate::delivery_audio::loudness_summary(&child(root))?;
    let a=area(root);let pcm=a.join("selected-audio.wav");let c=private(root,i);
    let loudness=crate::audio_signal::loudness::measure(&pcm,&(d.wav_sha256.clone(),d.wav_bytes),&c.tool_limits(),&a,token)?;
    let (stream,range,frames)=source_signal::extent(&d.inspection)?;
    let signal=VideoAudioSignalReceipt {schema:"aniflow.video-audio-signal/v1".into(),source_sha256:i.native.artifact.sha256.clone(),source_bytes:i.native.artifact.byte_count,audio_stream:stream,
        temporal_sha256:d.inspection.sha256().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?,source_audio_extent:range,sample_rate_hz:48_000,channels:d.channels,frames,
        decoded_wav_sha256:d.wav_sha256.clone(),decoded_wav_bytes:d.wav_bytes,decoded_pcm_sha256:d.pcm_sha256.clone(),ffmpeg_sha256:c.ffmpeg.sha256,ffprobe_sha256:c.ffprobe.sha256,provider_sha256:i.components[2].sha256.clone(),
        decode_arguments:source_signal::decode_arguments(&a.join("source.media"),&pcm,stream,i.request.qc.maximum_pcm_bytes).iter().map(|v|v.to_string_lossy().into_owned()).collect(),
        signal_run:"children/signal".into(),signal_plan_sha256:accepted.payload.plan_sha256,signal_report_sha256:summary.report_sha256,loudness,normalization:"none".into(),runtime_qualified:false};
    Ok(Meter {schema:"aniflow.resumable-final-video-audio-qc-meter/v1".into(),intent_sha256:i.intent_sha256.clone(),native_manifest_sha256:accepted.manifest_sha256,signal})
}
