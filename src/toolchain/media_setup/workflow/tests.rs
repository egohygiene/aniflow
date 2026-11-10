use super::*;
pub(super) fn request()->WorkflowSetupRequest {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.toolchain.workflow-setup/v1","profile":{"schema":"aniflow.toolchain-profile/v1","id":"synthetic-core","default_capabilities":[],
        "dependencies":[{"id":"f","kind":"tool","optional":false,"required_flags":[],"required_features":[],"suggested_locators":[]},{"id":"p","kind":"tool","optional":false,"required_flags":[],"required_features":[],"suggested_locators":[]}],
        "capabilities":[{"id":"extract","optional":false,"dependency_ids":["f","p"],"platforms":[{"os":std::env::consts::OS,"arch":std::env::consts::ARCH}],"effective_settings":{"source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]},"timeout_milliseconds":1000,"maximum_capture_bytes":65536,"maximum_source_bytes":1048576,"maximum_frame_bytes":1048576,"maximum_total_frame_bytes":16777216,"maximum_pixels":8400000,"maximum_frames":100,"maximum_working_bytes":1073741824,"minimum_free_bytes":0},"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]}]},
        "inventory":{"schema":"aniflow.toolchain-inventory/v1","platform":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH},"artifacts":[],"hardware":[]},"selections":[{"capability_id":"extract","selection":{"kind":"frame_extraction","ffmpeg_dependency_id":"f","ffprobe_dependency_id":"p"}}]})).unwrap()
}

#[test]
fn mixed_core_and_publishing_preparation_keeps_every_selected_failure_attributed() {
    let mut value=serde_json::to_value(request()).unwrap();let mut capability=value["profile"]["capabilities"][0].clone();capability["id"]=serde_json::json!("audio");
    capability["effective_settings"]=serde_json::json!({"format":"wav","tool_timeout_milliseconds":1000,"maximum_tool_output_bytes":65536,"maximum_output_bytes":268435456,"maximum_working_bytes":1073741824,"minimum_free_bytes":0,"metadata_policy":"remove_inherited_descriptive_metadata"});
    value["profile"]["capabilities"].as_array_mut().unwrap().push(capability);value["selections"].as_array_mut().unwrap().push(serde_json::json!({"capability_id":"audio","selection":{"kind":"lossless_audio_export","ffmpeg_dependency_id":"f","ffprobe_dependency_id":"p"}}));
    let selected=WorkflowSetupRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();let report=prepare_workflow_setup(&selected).unwrap();
    assert!(!report.ready);assert_eq!(report.selections.len(),2);assert_eq!(report.omitted_roles.len(),9);assert!(report.selections.iter().all(|s|s.preparation.files.is_empty()));assert!(report.diagnostics.iter().any(|d|d.starts_with("frame_extraction / extract")));assert!(report.diagnostics.iter().any(|d|d.starts_with("lossless_audio_export / audio")));
}
#[test]
fn omitted_neural_roles_do_not_inspect_or_require_their_models() {
    let mut r=request();let mut model=r.profile.dependencies[0].clone();model.id="unused-model".into();model.kind=ToolchainDependencyKind::Model;model.optional=true;r.profile.dependencies.push(model);
    let report=prepare_workflow_setup(&r).unwrap();assert!(report.omitted_roles.contains(&"interpolation".into()));assert!(report.omitted_roles.contains(&"upscale".into()));
    assert!(report.selections[0].preparation.inspection.facts.iter().all(|f|f.dependency_id.as_deref()!=Some("unused-model")));assert!(!report.native_qualification);assert_eq!(report.execution_authority,"none; selection order is not a runtime plan");
}
#[test]
fn repeated_domain_roles_and_wrong_dependency_kinds_refuse_before_files() {
    let original=request();let mut r=original.clone();r.selections.push(r.selections[0].clone());r.selections[1].capability_id="other-extraction".into();assert!(r.validate().is_err());
    let mut r=original.clone();r.selections.clear();assert!(r.validate().is_err());let mut r=original;r.profile.dependencies[0].kind=ToolchainDependencyKind::Model;assert!(r.validate().is_err());
}
#[test]
fn workflow_identity_changes_with_selection_and_has_no_install_or_execution_port() {
    let r=request();let first=prepare_workflow_setup(&r).unwrap();let second=prepare_workflow_setup(&r).unwrap();assert_eq!(first,second);
    for field in ["install","execute","source_media","auto_select"] {let mut value=serde_json::to_value(&r).unwrap();value[field]=serde_json::json!(true);assert!(WorkflowSetupRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());}
    assert_eq!(first.source_media_evidence,"unobserved");assert_eq!(first.publication_authority,"none");
}
