//! Synthetic headers/pure refusal only. No encoder, meter or native run qualified.
use super::*;
use crate::audio_analysis::{AudioArtifactReference,AudioSource,AudioRationalTime,AudioScope};
use crate::audio_signal::{AudioSignalMeasurement,AudioSignalUnit,AudioSignalValue,AudioSignalUnavailableReason};
fn wave()->Vec<u8> {
    let pcm:Vec<u8>=[0i16,1,-1,32767,-32768,2,-2,17].into_iter().flat_map(i16::to_le_bytes).collect();
    let mut bytes=b"RIFF".to_vec();bytes.extend((36u32+pcm.len() as u32).to_le_bytes());bytes.extend(b"WAVEfmt ");bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());bytes.extend(2u16.to_le_bytes());bytes.extend(16000u32.to_le_bytes());bytes.extend(64000u32.to_le_bytes());bytes.extend(4u16.to_le_bytes());bytes.extend(16u16.to_le_bytes());bytes.extend(b"data");bytes.extend((pcm.len() as u32).to_le_bytes());bytes.extend(pcm);bytes
}
fn operation(root:&Path)->LosslessOperation {
    let bytes=wave();let file=root.join("original mix 音.wav");std::fs::write(&file,&bytes).unwrap();let native=crate::audio_inspection::wav::inspect_native(&file).unwrap();
    let scope=AudioScope {channels:vec![0,1],stem_id:None};
    let measurement=|unit|AudioSignalMeasurement {unit,scope:scope.clone(),value:AudioSignalValue::Unavailable {reason:AudioSignalUnavailableReason::InsufficientDuration}};
    let summary=crate::delivery_audio::DeliveryLoudnessSummary {schema:"aniflow.delivery-loudness-summary/v1".into(),report_sha256:"a".repeat(64),report_schema:crate::audio_signal::AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3.into(),
        source:AudioSource {artifact:AudioArtifactReference {id:"source".into(),sha256:local::hash(&bytes),byte_size:bytes.len() as u64},stream_index:0,sample_rate_hz:16000,channels:2,frame_count:4,origin:AudioRationalTime {numerator:0,denominator:1},stem:None},
        integrated_loudness:measurement(AudioSignalUnit::LoudnessUnitsFullScale),true_peak:measurement(AudioSignalUnit::DecibelsTruePeak),normalization:"none".into(),video_audio_binding:"not_established_by_this_view".into()};
    let pin=|id:&str|crate::audio_inspection::AudioToolPin {executable:std::path::PathBuf::from(format!("/synthetic/{id}")),version:"8.0.1".into(),sha256:"b".repeat(64)};
    LosslessOperation {schema:SCHEMA.into(),source:ExportSource {file,signal_run_directory:root.join("no accepted signal"),summary,source_manifest_sha256:"c".repeat(64),encoding:PcmEncoding::Pcm16,pcm_sha256:native.pcm_sha256},
        configuration:LosslessConfiguration {schema:"aniflow.lossless-audio-configuration/v1".into(),format:LosslessFormat::Wav,
            tools:crate::audio_inspection::AudioInspectionConfiguration {schema:crate::audio_inspection::AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.into(),ffmpeg:pin("ffmpeg"),ffprobe:pin("ffprobe"),tool_timeout_milliseconds:1000,maximum_tool_output_bytes:4096},
            metadata_policy:ExportMetadataPolicy::RemoveInheritedDescriptiveMetadata,maximum_output_bytes:1024,maximum_working_bytes:16_777_216,minimum_free_bytes:0}}
}
#[test]
fn native_wave_identity_retains_stereo_full_extent_and_original_bytes() {
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let op=operation(&root);let copy=root.join("candidate.wav");let original=std::fs::read(&op.source.file).unwrap();native::wave_export(&op.source.file,&copy,&op,&CancellationToken::default()).unwrap();
    native::container(&copy,&op).unwrap();assert_eq!(std::fs::read(&op.source.file).unwrap(),original);
    let mut changed=original;*changed.last_mut().unwrap()^=1;std::fs::write(copy,changed).unwrap();assert!(native::container(&root.join("candidate.wav"),&op).is_err());
}
#[test]
fn inherited_descriptive_metadata_is_not_accepted_as_a_minimal_wave_export() {
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let op=operation(&root);let copy=root.join("candidate.wav");let mut bytes=wave();
    bytes.extend(b"LIST");bytes.extend(4u32.to_le_bytes());bytes.extend(b"TEST");let size=bytes.len() as u32-8;bytes[4..8].copy_from_slice(&size.to_le_bytes());std::fs::write(&copy,bytes).unwrap();
    assert!(native::container(&copy,&op).is_err());assert!(op.source.file.exists());
}
#[test]
fn streaminfo_fields_are_checked_but_a_header_alone_never_gains_run_acceptance() {
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let mut op=operation(&root);op.configuration.format=LosslessFormat::Flac;
    let packed=(16000u64<<44)|(1u64<<41)|(15u64<<36)|4;let mut info=[0u8;34];info[..2].copy_from_slice(&16u16.to_be_bytes());info[2..4].copy_from_slice(&16u16.to_be_bytes());info[10..18].copy_from_slice(&packed.to_be_bytes());
    let mut bytes=b"fLaC\x80\0\0\x22".to_vec();bytes.extend(info);let file=root.join("header-only.flac");std::fs::write(&file,&bytes).unwrap();native::container(&file,&op).unwrap();
    assert!(observe(&root,&CancellationToken::default()).is_err());
    bytes[18..26].copy_from_slice(&(packed+1).to_be_bytes());std::fs::write(&file,&bytes).unwrap();assert!(native::container(&file,&op).is_err());
}
#[test]
fn float_flac_and_hidden_signal_transform_fields_refuse_before_work() {
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let mut op=operation(&root);op.configuration.format=LosslessFormat::Flac;op.source.encoding=PcmEncoding::Float32;
    assert!(op.validate().is_err());op.configuration.format=LosslessFormat::Wav;op.validate().unwrap();
    let mut value=serde_json::to_value(&op.configuration).unwrap();value["normalize"]=serde_json::json!(true);assert!(LosslessConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn native_arguments_have_literal_paths_and_no_trim_gain_rate_or_channel_changes() {
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let mut op=operation(&root);let output=root.join("output 音.flac");op.configuration.format=LosslessFormat::Flac;
    let args=native::flac_arguments(&op,&op.source.file,&output);assert!(args.contains(&op.source.file.as_os_str().to_owned()));assert_eq!(args.last().unwrap().as_os_str(),output.as_os_str());
    for absent in ["-ar","-ac","-af","-ss","-t","-to","-y"] {assert!(!args.contains(&std::ffi::OsString::from(absent)));}
    assert!(args.contains(&std::ffi::OsString::from("-n")));assert!(args.contains(&std::ffi::OsString::from("flac")));
    assert!(native::METER.starts_with("not_remeasured"));
}
#[test]
fn invented_signal_or_loose_export_files_do_not_create_provider_authority() {
    let temporary=tempfile::tempdir().unwrap();let root=temporary.path().canonicalize().unwrap();let op=operation(&root);let expected=std::fs::read(&op.source.file).unwrap();
    let request=LosslessRequest {source:op.source.file.clone(),signal_run_directory:op.source.signal_run_directory.clone(),configuration:op.configuration.clone(),provider_executable:root.join("absent provider")};
    assert!(plan(&request,&CancellationToken::default()).is_err());assert!(run(request,&root.join("no output"),&CancellationToken::default(),|_|{}).is_err());assert!(!root.join("no output").exists());
    assert!(observe(&root,&CancellationToken::default()).is_err());assert_eq!(std::fs::read(op.source.file).unwrap(),expected);
}

#[test]
fn final_qc_flac_decode_selects_exact_native_precision_without_signal_changes() {
    for (native,codec,muxer) in [(crate::audio_inspection::NativeSampleFormat::Pcm16,"pcm_s16le","s16le"),(crate::audio_inspection::NativeSampleFormat::Pcm24,"pcm_s24le","s24le")] {
        let input=std::path::Path::new("/synthetic/final 音.flac");let output=std::path::Path::new("/synthetic/decoded.pcm");let args=native::qc_decode_arguments(input,output,native,100000);
        assert!(args.windows(2).any(|v|v==[std::ffi::OsString::from("-codec:a"),std::ffi::OsString::from(codec)]));
        assert!(args.windows(2).any(|v|v==[std::ffi::OsString::from("-f"),std::ffi::OsString::from(muxer)]));
        assert_eq!(args.last().unwrap().as_os_str(),output.as_os_str());
        for absent in ["-ar","-ac","-af","-ss","-t","-to","-y"] {assert!(!args.contains(&std::ffi::OsString::from(absent)));}
    }
}
