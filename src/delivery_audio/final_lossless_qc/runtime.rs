use super::*;
fn bind_summary(intent:&FinalLosslessQcIntent,pin:&(String,u64),summary:&DeliveryLoudnessSummary)->Result<()> {
    let (source,_,_)=source_facts(&intent.source)?;let s=&summary.source;
    if summary.report_schema!=crate::audio_signal::AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3||summary.normalization!="none"||s.artifact.sha256!=pin.0||s.artifact.byte_size!=pin.1
        ||s.frame_count!=source.source.frame_count||s.channels!=source.source.channels||s.sample_rate_hz!=source.source.sample_rate_hz||s.stream_index!=0||s.stem.is_some()||s.origin.numerator!=0||s.origin.denominator!=1 {
        return Err(invalid("new accepted final-file signal measurements differ from complete native decoded subject"));
    }Ok(())
}
fn recheck_files(root:&Path,intent:&FinalLosslessQcIntent)->Result<()> {
    if identity(&root.join("source.media"),crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES)?!=(intent.source.file.sha256.clone(),intent.source.file.byte_count){return Err(invalid("private final-file snapshot changed"));}
    for (component,name) in intent.components.iter().zip(["ffmpeg","ffprobe","aniflow-provider"]) {
        let expected=(component.sha256.clone(),component.byte_count);
        if identity(&component.path,134_217_728)?!=expected||identity(&root.join(name),134_217_728)?!=expected{return Err(invalid("final QC original or private component changed"));}
    }
    if flac(intent) {
        let (summary,encoding,pcm)=source_facts(&intent.source)?;let count=summary.source.frame_count.checked_mul(u64::from(summary.source.channels)*u64::from(native(encoding).bytes_per_sample())).ok_or_else(||invalid("raw PCM count overflow"))?;
        if identity(&root.join("decoded.pcm"),intent.request.maximum_pcm_bytes)?!=(pcm.to_owned(),count){return Err(invalid("retained final FLAC raw native decode changed"));}
    }Ok(())
}
fn subject_disjoint(root:&Path,intent:&FinalLosslessQcIntent)->Result<()> {
    local::disjoint(root,&intent.request.subject.input(),&intent.source.file.locator)?;
    if let MediaFacts::LosslessAudio {report}=&intent.source.facts {let original=&report.operation.source;local::disjoint(root,&MediaInput::AcceptedPcm {file:original.file.clone(),signal_run_directory:original.signal_run_directory.clone()},&original.file)?;}
    for component in &intent.components {if root.starts_with(&component.path)||component.path.starts_with(root){return Err(invalid("final lossless QC output overlaps explicit component"));}}
    Ok(())
}
/// Fresh-only observation; all failed/partial native work is retained, never
/// adopted as completion. Native signal child owns its existing v3 acceptance.
pub fn run(request:&FinalLosslessQcRequest,root:&Path,token:&CancellationToken)->Result<FinalLosslessQcReport> {
    let intent=plan(request,token)?;let parent=local::parent(root)?;subject_disjoint(root,&intent)?;
    crate::video_assembly::require_storage(parent,intent.projected_working_bytes,request.minimum_free_bytes)?;
    let _source_writer=crate::cache_v3::ExclusiveWriter::acquire(request.subject.run_directory(),"writer.lock")?;
    if plan(request,token)?!=intent{return Err(invalid("final audio source changed before QC capture"));}
    local::directory(root)?;local::write(&root.join("intent.json"),&local::bytes(&intent)?)?;
    let snapshot=root.join("source.media");copy_pinned(&intent.source.file.locator,&snapshot,&(intent.source.file.sha256.clone(),intent.source.file.byte_count),crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,token)?;
    for (component,name) in intent.components.iter().zip(["ffmpeg","ffprobe","aniflow-provider"]) {
        let destination=root.join(name);copy_pinned(&component.path,&destination,&(component.sha256.clone(),component.byte_count),134_217_728,token)?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&destination,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot enable private final-QC component"))?;}
    }
    let pcm=root.join("decoded.wav");
    if flac(&intent) {
        let MediaFacts::LosslessAudio {report}=&intent.source.facts else{return Err(invalid("final FLAC source report missing"));};
        crate::lossless_audio::decode_for_qc(report,&snapshot,&pcm,root,&private_configuration(root,&intent),request.maximum_pcm_bytes,token)?;
    } else {copy_pinned(&snapshot,&pcm,&(intent.source.file.sha256.clone(),intent.source.file.byte_count),request.maximum_pcm_bytes,token)?;}
    let pin=wave(root,&intent)?;
    let analysis=crate::audio_signal::run(signal_request(root,&intent),Some(root.join("signal-runs")),token,|_|{}).map_err(|e|e.error)?;
    let accepted=crate::status_v3(&analysis.run_directory)?;let summary=super::super::loudness_summary(&analysis.run_directory)?;bind_summary(&intent,&pin,&summary)?;
    let source=&summary.source;
    let (integrated_loudness,true_peak,meter)=if source.sample_rate_hz==48_000&&matches!(source.channels,1|2) {
        let receipt=crate::audio_signal::loudness::measure_native(&pcm,&pin,&private_configuration(root,&intent),root,token)?;
        (receipt.integrated_loudness.clone(),receipt.true_peak.clone(),FinalLosslessMeter::Observed {receipt:Box::new(receipt)})
    }else{unavailable(source.channels)};
    let (_,encoding,pcm_sha)=source_facts(&intent.source)?;
    let report=FinalLosslessQcReport {schema:"aniflow.final-lossless-audio-qc/v1".into(),intent_sha256:crate::provider::canonical_sha256(&intent)?,decoded_wav_sha256:pin.0,decoded_wav_bytes:pin.1,decoded_pcm_sha256:pcm_sha.into(),encoding,
        signal_run_directory:analysis.run_directory,signal_plan_sha256:accepted.payload.plan_sha256.clone(),signal_manifest_sha256:accepted.manifest_sha256.clone(),accepted_pcm:summary,
        integrated_loudness,true_peak,meter,normalization:"none".into(),runtime_qualified:false,creative_approval:"not_performed; final-file measurements are not musical quality review".into(),publication_authority:"none".into(),intent};
    recheck_files(root,&report.intent)?;
    if plan(request,token)?!=report.intent||wave(root,&report.intent)?!=(report.decoded_wav_sha256.clone(),report.decoded_wav_bytes)||crate::status_v3(&report.signal_run_directory)?!=accepted{return Err(invalid("final source/PCM/child changed during measurement"));}
    cancelled(token)?;local::write(&root.join("qc.json"),&local::bytes(&report)?)?;Ok(report)
}
/// Read only current native child and exact retained bytes, with an externally
/// supplied QC report digest. No decoder, signal provider or meter is rerun.
pub fn observe(root:&Path,receipt_sha256:&str,token:&CancellationToken)->Result<FinalLosslessQcReport> {
    cancelled(token)?;ensure_canonical_directory(root)?;crate::provider::require_sha256(receipt_sha256,"final lossless QC receipt")?;
    let path=root.join("qc.json");let pin=identity(&path,local::DOCUMENT_BYTES)?;if pin.0!=receipt_sha256{return Err(invalid("final lossless QC receipt differs from external pin"));}
    let report:FinalLosslessQcReport=local::read(&path)?;let intent:FinalLosslessQcIntent=local::read(&root.join("intent.json"))?;
    if report.schema!="aniflow.final-lossless-audio-qc/v1"||report.intent!=intent||report.intent_sha256!=crate::provider::canonical_sha256(&intent)?
        ||report.normalization!="none"||report.runtime_qualified||report.creative_approval!="not_performed; final-file measurements are not musical quality review"||report.publication_authority!="none"||plan(&intent.request,token)?!=intent {
        return Err(invalid("final lossless QC is stale or differs from exact current final file/producer"));
    }
    let mut files=vec!["intent.json","qc.json","source.media","decoded.wav","ffmpeg","ffprobe","aniflow-provider"];if flac(&intent){files.push("decoded.pcm");}
    local::entries(root,&files,&["signal-runs"],false)?;
    if report.signal_run_directory.parent()!=Some(root.join("signal-runs").as_path()) {return Err(invalid("final QC signal child escaped fixed private parent"));}
    let child=report.signal_run_directory.file_name().and_then(|v|v.to_str()).ok_or_else(||invalid("final QC child name unavailable"))?;local::entries(&root.join("signal-runs"),&[],&[child],false)?;
    let accepted=crate::status_v3(&report.signal_run_directory)?;
    if accepted.payload.state!=crate::PipelineV3RunState::Complete||accepted.payload.delivery.is_none()||accepted.payload.plan_sha256!=report.signal_plan_sha256||accepted.manifest_sha256!=report.signal_manifest_sha256{return Err(invalid("final QC native signal acceptance changed"));}
    let summary=super::super::loudness_summary(&report.signal_run_directory)?;let wav_pin=wave(root,&intent)?;bind_summary(&intent,&wav_pin,&summary)?;
    let (_,encoding,pcm)=source_facts(&intent.source)?;
    if wav_pin!=(report.decoded_wav_sha256.clone(),report.decoded_wav_bytes)||summary!=report.accepted_pcm||report.decoded_pcm_sha256!=pcm||report.encoding!=encoding{return Err(invalid("final QC decoded subject/accepted native measurement differs"));}
    match &report.meter {
        FinalLosslessMeter::Observed {receipt}=>{
            crate::audio_signal::loudness::validate_receipt(receipt)?;
            if receipt.schema!="aniflow.native-pcm-loudness/v2"||receipt.sample_rate_hz!=summary.source.sample_rate_hz||receipt.channels!=summary.source.channels||receipt.frames!=summary.source.frame_count||receipt.pcm_sha256!=pcm
                ||report.integrated_loudness!=receipt.integrated_loudness||report.true_peak!=receipt.true_peak{return Err(invalid("final native meter differs from exact decoded final subject"));}
        },
        FinalLosslessMeter::Unavailable {reason}=>{
            let (l,p,meter)=unavailable(summary.source.channels);
            if summary.source.sample_rate_hz==48_000&&matches!(summary.source.channels,1|2)||*reason!=AudioSignalUnavailableReason::UnsupportedNativeSignalProfile||report.integrated_loudness!=l||report.true_peak!=p||report.meter!=meter{return Err(invalid("final unavailable quantities differ from the explicit unsupported meter profile"));}
        },
    }
    recheck_files(root,&intent)?;subject_disjoint(root,&intent)?;
    if plan(&intent.request,token)?!=intent||crate::status_v3(&report.signal_run_directory)?!=accepted||identity(&path,local::DOCUMENT_BYTES)?!=pin{return Err(invalid("final source or receipt changed during QC readback"));}
    cancelled(token)?;Ok(report)
}
