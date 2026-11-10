use super::*;
#[test]
fn qc_requires_the_complete_float_decoder_and_both_meter_passes() {
    let (flags,features)=qc_requirements();
    for required in ["encoder:pcm_f32le","decoder:pcm_f32le","muxer:wav","muxer:null","filter:ebur128","filter:ametadata","filter:apad"] {assert!(features.contains(&required));let missing=features.iter().filter(|v|**v!=required).map(|v|v.to_string()).collect::<Vec<_>>();assert!(includes(&missing,&[required],"chosen-ffmpeg","features").is_err());}
    for required in ["-af","-rf64","-fs","-flags:a","-loglevel"] {assert!(flags.contains(&required));}
    assert!(!features.iter().any(|v|v.contains("loudnorm")||v.contains("aresample")));
}
#[test]
fn accepted_subtitle_preservation_requires_extraction_and_disposition_but_no_mov_text_encoder() {
    let (plain_flags,plain)=metadata_requirements(MetadataSubtitleSetup::None);
    let (flags,features)=metadata_requirements(MetadataSubtitleSetup::PreserveAcceptedSoftSubtitle);
    assert!(!plain_flags.contains(&"-disposition:s:0"));assert!(!plain.contains(&"decoder:mov_text"));
    for required in ["-disposition:s:0","-map_metadata:s:s:0","-metadata:s:s:0"] {assert!(flags.contains(&required));}
    for required in ["decoder:mov_text","encoder:srt","muxer:srt"] {assert!(features.contains(&required));}
    assert!(!features.contains(&"encoder:mov_text"));
}
#[test]
fn lossless_setup_keeps_float_wav_separate_from_integer_flac() {
    let (_,wav)=lossless_requirements(crate::lossless_audio::LosslessFormat::Wav);
    let (_,flac)=lossless_requirements(crate::lossless_audio::LosslessFormat::Flac);
    assert!(wav.contains(&"decoder:pcm_f32le"));assert!(!wav.contains(&"encoder:flac"));assert!(!flac.contains(&"encoder:pcm_f32le"));assert!(flac.contains(&"decoder:flac"));
}
fn missing_request()->PublishingSetupRequest {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.toolchain.publishing-setup/v1",
        "profile":{"schema":"aniflow.toolchain-profile/v1","id":"publishing","default_capabilities":[],
            "dependencies":[{"id":"chosen-ffmpeg","kind":"tool","optional":false,"required_flags":[],"required_features":[],"suggested_locators":[]},{"id":"chosen-probe","kind":"tool","optional":false,"required_flags":[],"required_features":[],"suggested_locators":[]}],
            "capabilities":[{"id":"lossless","optional":false,"dependency_ids":["chosen-ffmpeg","chosen-probe"],"platforms":[{"os":std::env::consts::OS,"arch":std::env::consts::ARCH}],"effective_settings":{"format":"wav","tool_timeout_milliseconds":10000,"maximum_tool_output_bytes":1048576,"maximum_output_bytes":268435456,"maximum_working_bytes":1073741824,"minimum_free_bytes":0,"metadata_policy":"remove_inherited_descriptive_metadata"},"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]}]},
        "inventory":{"schema":"aniflow.toolchain-inventory/v1","platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH},"artifacts":[],"hardware":[]},
        "selections":[{"capability_id":"lossless","selection":{"kind":"lossless_audio_export","ffmpeg_dependency_id":"chosen-ffmpeg","ffprobe_dependency_id":"chosen-probe"}}]})).unwrap()
}
#[test]
fn missing_selected_dependencies_produce_no_configuration_or_runtime_authority() {
    let report=prepare_publishing_setup(&missing_request()).unwrap();assert!(!report.ready);assert!(!report.native_qualification);assert_eq!(report.selections.len(),1);assert!(report.selections[0].preparation.files.is_empty());
    assert!(!report.diagnostics.is_empty());assert_eq!(report.host_runtime_evidence,"unobserved");assert_eq!(report.source_media_evidence,"unobserved");assert_eq!(report.publication_authority,"none");
}
#[test]
fn empty_duplicate_unbounded_and_unrelated_selections_refuse_before_inspection() {
    let request=missing_request();request.validate().unwrap();
    let mut empty=request.clone();empty.selections.clear();assert!(empty.validate().is_err());
    let mut duplicate=request.clone();duplicate.selections.push(duplicate.selections[0].clone());assert!(duplicate.validate().is_err());
    let mut large=request.clone();large.selections=vec![request.selections[0].clone();9];assert!(large.validate().is_err());
    let mut unrelated=request;unrelated.selections[0].selection=MediaSetupSelection::AllenkImageRepair {ffmpeg_dependency_id:"chosen-ffmpeg".into(),processor_dependency_id:"chosen-probe".into()};assert!(unrelated.validate().is_err());
}
#[test]
fn setup_roles_are_explicit_tools_and_unknown_fields_are_not_execution_ports() {
    for selection in [MediaSetupSelection::FinalVideoAudioQc {ffmpeg_dependency_id:"f".into(),ffprobe_dependency_id:"p".into()},MediaSetupSelection::MetadataRemux {ffmpeg_dependency_id:"f".into(),ffprobe_dependency_id:"p".into()}] {assert_eq!(selection.dependencies().len(),2);assert!(selection.dependencies().iter().all(|(_,k)|*k==ToolchainDependencyKind::Tool));}
    let mut value=serde_json::to_value(missing_request()).unwrap();value["install"]=true;assert!(PublishingSetupRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
