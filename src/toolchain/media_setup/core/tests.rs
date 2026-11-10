use super::*;
fn request()->CoreMediaSetupRequest {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.toolchain.core-media-setup/v1","profile":{"schema":"aniflow.toolchain-profile/v1","id":"synthetic-core","default_capabilities":[],
        "dependencies":[{"id":"f","kind":"tool","optional":false,"required_flags":[],"required_features":[],"suggested_locators":[]},{"id":"p","kind":"tool","optional":false,"required_flags":[],"required_features":[],"suggested_locators":[]}],
        "capabilities":[{"id":"extract","optional":false,"dependency_ids":["f","p"],"platforms":[{"os":std::env::consts::OS,"arch":std::env::consts::ARCH}],"effective_settings":{"source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]},"timeout_milliseconds":1000,"maximum_capture_bytes":65536,"maximum_source_bytes":1048576,"maximum_frame_bytes":1048576,"maximum_total_frame_bytes":16777216,"maximum_pixels":8400000,"maximum_frames":100,"maximum_working_bytes":1073741824,"minimum_free_bytes":0},"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]}]},
        "inventory":{"schema":"aniflow.toolchain-inventory/v1","platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH},"artifacts":[],"hardware":[]},"selections":[{"capability_id":"extract","selection":{"kind":"frame_extraction","ffmpeg_dependency_id":"f","ffprobe_dependency_id":"p"}}]})).unwrap()
}
#[test]
fn extraction_requires_original_clock_and_complete_frame_observation_features() {
    let (flags,features)=extraction_requirements();for f in ["-apply_cropping","-noautoscale","-enc_time_base:v","-frames:v","-atomic_writing"] {assert!(flags.contains(&f));}
    for f in ["encoder:png","encoder:rawvideo","muxer:framehash","hash:sha256"] {let incomplete=features.iter().filter(|v|**v!=f).map(|v|v.to_string()).collect::<Vec<_>>();assert!(includes(&incomplete,&[f],"f","features").is_err());}
}
#[test]
fn assembly_readiness_includes_clean_master_and_its_independent_full_frame_decode() {
    let (flags,features)=assembly_requirements();for f in ["encoder:ffv1","decoder:ffv1","encoder:libx264","decoder:png","encoder:rawvideo","muxer:framehash","pixel_format:bgr0"] {assert!(features.contains(&f));}
    for f in ["-level:v","-slicecrc","-copypriorss:a","-video_track_timescale"] {assert!(flags.contains(&f));}assert!(!features.contains(&"encoder:aac"));
}
#[test]
fn missing_tools_return_attributed_diagnostics_without_configuration_or_qualification() {
    let report=prepare_core_media_setup(&request()).unwrap();assert!(!report.ready);assert!(!report.native_qualification);assert_eq!(report.source_media_evidence,"unobserved");assert!(!report.diagnostics.is_empty());assert!(report.selections[0].preparation.files.is_empty());
}
#[test]
fn core_scope_refuses_empty_duplicate_neural_and_wrong_dependency_roles() {
    let original=request();original.validate().unwrap();let mut r=original.clone();r.selections.clear();assert!(r.validate().is_err());let mut r=original.clone();r.selections.push(r.selections[0].clone());assert!(r.validate().is_err());
    let mut r=original.clone();r.selections[0].selection=MediaSetupSelection::UpscaylRecovery {ffmpeg_dependency_id:"f".into(),processor_dependency_id:"p".into(),param_dependency_id:"m".into(),weights_dependency_id:"w".into()};assert!(r.validate().is_err());
    let mut r=original;r.profile.dependencies[0].kind=ToolchainDependencyKind::Model;assert!(r.validate().is_err());
}
#[test]
fn closed_core_options_preserve_selection_and_refuse_hidden_execution_or_install_fields() {
    let r=request();let selected=r.selected(&r.selections[0]);let Options::Extract(options)=selected.options().unwrap() else{panic!("fixture");};let config=options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe"));config.validate().unwrap();assert_eq!(config.maximum_frames,100);assert_eq!(config.source_selection.video_stream,Some(0));
    let mut value=serde_json::to_value(r).unwrap();value["install"]=true;assert!(CoreMediaSetupRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn core_and_publishing_selections_remain_explicit_distinct_scopes() {
    let r=request();let publishing=PublishingSetupRequest {schema:"aniflow.toolchain.publishing-setup/v1".into(),profile:r.profile,inventory:r.inventory,selections:r.selections};assert!(publishing.validate().is_err());
    for selection in [MediaSetupSelection::FrameExtraction {ffmpeg_dependency_id:"f".into(),ffprobe_dependency_id:"p".into()},MediaSetupSelection::CleanVideoAssembly {ffmpeg_dependency_id:"f".into(),ffprobe_dependency_id:"p".into()}] {assert_eq!(selection.dependencies().len(),2);assert!(selection.dependencies().iter().all(|(_,k)|*k==ToolchainDependencyKind::Tool));}
}
