use super::*;

#[test]
fn gain_qc_selects_an_existing_native_owner_instead_of_adopting_a_loose_file() {
    let subject=FinalLosslessSubject::AcceptedGainCandidate {run_directory:"/synthetic/accepted-gain".into()};
    assert!(matches!(subject.input(),MediaInput::AcceptedGainCandidate {..}));
    let value=serde_json::json!({"kind":"accepted_gain_candidate","file":"/synthetic/arbitrary.wav"});
    assert!(serde_json::from_value::<FinalLosslessSubject>(value).is_err());
}
fn request(root:&Path)->FinalLosslessQcRequest {
    let pin=|name:&str|crate::audio_inspection::AudioToolPin {executable:root.join(name),version:"8.0.1".into(),sha256:"a".repeat(64)};
    FinalLosslessQcRequest {schema:"aniflow.final-lossless-audio-qc-request/v1".into(),subject:FinalLosslessSubject::AcceptedPcm {file:root.join("original.wav"),signal_run_directory:root.join("absent-native-run")},
        configuration:AudioInspectionConfiguration {schema:crate::audio_inspection::AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.into(),ffmpeg:pin("ffmpeg"),ffprobe:pin("ffprobe"),tool_timeout_milliseconds:1000,maximum_tool_output_bytes:4096},
        provider_executable:root.join("provider"),maximum_pcm_bytes:1048576,maximum_working_bytes:1073741824,minimum_free_bytes:0}
}
fn native_file(root:&Path,encoding:PcmEncoding,rate:u32,channels:u16)->(FinalLosslessQcIntent,Vec<u8>) {
    let format=native(encoding);let samples=vec![0u8;4*usize::from(channels)*usize::from(format.bytes_per_sample())];
    let float=encoding==PcmEncoding::Float32;let mut bytes=b"RIFF".to_vec();bytes.extend((36+if float {12}else{0}+samples.len() as u32).to_le_bytes());bytes.extend(b"WAVEfmt ");bytes.extend(16u32.to_le_bytes());bytes.extend((if float {3u16}else{1u16}).to_le_bytes());bytes.extend(channels.to_le_bytes());bytes.extend(rate.to_le_bytes());
    let align=channels*format.bytes_per_sample();bytes.extend((rate*u32::from(align)).to_le_bytes());bytes.extend(align.to_le_bytes());bytes.extend(format.bits_per_sample().to_le_bytes());
    if float {bytes.extend(b"fact");bytes.extend(4u32.to_le_bytes());bytes.extend(4u32.to_le_bytes());}
    bytes.extend(b"data");bytes.extend((samples.len() as u32).to_le_bytes());bytes.extend(samples);std::fs::write(root.join("decoded.wav"),&bytes).unwrap();let wave=crate::audio_inspection::wav::inspect_native(&root.join("decoded.wav")).unwrap();
    let (l,p,_)=unavailable(channels);let summary=DeliveryLoudnessSummary {schema:"aniflow.delivery-loudness-summary/v1".into(),report_sha256:"b".repeat(64),report_schema:crate::audio_signal::AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3.into(),
        source:crate::audio_analysis::AudioSource {artifact:crate::audio_analysis::AudioArtifactReference {id:"source".into(),sha256:local::hash(&bytes),byte_size:bytes.len() as u64},stream_index:0,sample_rate_hz:rate,channels,frame_count:4,origin:crate::audio_analysis::AudioRationalTime {numerator:0,denominator:1},stem:None},integrated_loudness:l,true_peak:p,normalization:"none".into(),video_audio_binding:"not_established_by_this_view".into()};
    let request=request(root);let source=MediaObservation {input:request.subject.input(),file:crate::destination::MediaFile {locator:root.join("original.wav"),sha256:local::hash(&bytes),byte_count:bytes.len() as u64},facts:MediaFacts::Pcm {summary,encoding,pcm_sha256:wave.pcm_sha256,run_manifest_sha256:"c".repeat(64)}};
    (FinalLosslessQcIntent {schema:"aniflow.final-lossless-audio-qc-intent/v1".into(),request,source,components:vec![],projected_working_bytes:0},bytes)
}
#[test]
fn native_precision_rate_and_stereo_count_are_preserved_in_the_exact_subject() {
    for (encoding,rate,channels) in [(PcmEncoding::Pcm24,44100,2),(PcmEncoding::Float32,96000,1),(PcmEncoding::Pcm16,48000,2)] {
        let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();let (intent,bytes)=native_file(&root,encoding,rate,channels);
        assert_eq!(wave(&root,&intent).unwrap(),(local::hash(&bytes),bytes.len() as u64));
        let (summary,actual,_)=source_facts(&intent.source).unwrap();assert_eq!(actual,encoding);assert_eq!(summary.source.sample_rate_hz,rate);assert_eq!(summary.source.channels,channels);
    }
}
#[test]
fn changed_samples_rate_precision_or_count_cannot_match_the_frozen_native_subject() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();let (intent,bytes)=native_file(&root,PcmEncoding::Pcm24,44100,2);
    for offset in [24,34,bytes.len()-1] {let mut changed=bytes.clone();changed[offset]^=1;std::fs::write(root.join("decoded.wav"),changed).unwrap();assert!(wave(&root,&intent).is_err());}
}
#[test]
fn unsupported_meter_keeps_two_typed_unavailable_quantities_with_real_channel_scope() {
    let (l,p,meter)=unavailable(2);assert_eq!(l.value,AudioSignalValue::Unavailable {reason:AudioSignalUnavailableReason::UnsupportedNativeSignalProfile});assert_eq!(p.value,l.value);
    assert_eq!(l.scope.channels,vec![0,1]);assert_eq!(p.scope,l.scope);assert!(matches!(meter,FinalLosslessMeter::Unavailable {..}));assert_ne!(l.value,AudioSignalValue::Measured {value:0.0});
}
#[test]
fn absent_native_acceptance_and_partial_markers_never_create_qc_completion() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();let request=request(&root);
    assert!(plan(&request,&CancellationToken::default()).is_err());let output=root.join("no-qc");assert!(run(&request,&output,&CancellationToken::default()).is_err());assert!(!output.exists());
    std::fs::write(root.join("qc.json"),b"{}").unwrap();assert!(observe(&root,&local::hash(b"{}"),&CancellationToken::default()).is_err());
}
#[test]
fn no_hidden_conversion_or_new_tool_version_can_be_selected() {
    let request=request(Path::new("/synthetic"));request.validate().unwrap();let mut wrong=request.clone();wrong.configuration.ffmpeg.version="9.0.0".into();assert!(wrong.validate().is_err());
    for field in ["normalize","resample","downmix","dither"] {let mut value=serde_json::to_value(&request).unwrap();value[field]=serde_json::json!(true);assert!(FinalLosslessQcRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());}
    let mut value=serde_json::to_value(&request).unwrap();value["subject"]["kind"]=serde_json::json!("accepted_audio_with_qc");assert!(FinalLosslessQcRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn new_qc_rate_selection_never_upgrades_old_unavailable_receipts() {
    let old=request(Path::new("/synthetic"));let mut new=old.clone();new.schema="aniflow.final-lossless-audio-qc-request/v2".into();new.validate().unwrap();
    assert_eq!(meter_schema(&old,44100,2),None);assert_eq!(meter_schema(&new,44100,2),Some("aniflow.native-pcm-loudness/v3"));assert_eq!(meter_schema(&new,48000,2),meter_schema(&old,48000,2));
    assert_eq!(meter_schema(&new,96000,2),None);assert_eq!(meter_schema(&new,44100,8),None);assert_eq!(report_schema(&old),"aniflow.final-lossless-audio-qc/v1");assert_eq!(report_schema(&new),"aniflow.final-lossless-audio-qc/v2");
}
#[test]
fn native_44100_qc_preserves_packed_precision_and_refuses_a_conversion_port() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();let (mut intent,bytes)=native_file(&root,PcmEncoding::Pcm24,44100,2);intent.request.schema="aniflow.final-lossless-audio-qc-request/v2".into();
    assert_eq!(wave(&root,&intent).unwrap(),(local::hash(&bytes),bytes.len() as u64));let mut value=serde_json::to_value(&intent.request).unwrap();value["sample_rate_hz"]=serde_json::json!(48000);assert!(FinalLosslessQcRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
