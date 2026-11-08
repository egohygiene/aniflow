//! Authored/uncompiled/unrun facade cases; no provider process or native
//! acceptance is represented. Reuse the existing synthetic provider fixture.
use super::*;
use std::fs;
fn fixture()->(tempfile::TempDir,RifeRunRequest) {super::super::provider::runtime_fixture()}
#[test]
fn fixed_rife_plan_records_exact_frames_index_models_and_no_ambient_effects() {
    let (_root,request)=fixture();let plan=plan(&request,&CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.pipeline_name,"serial-rife-interpolation");assert_eq!(plan.payload.stages.len(),1);assert_eq!(plan.payload.outputs.len(),2);
    let stage=&plan.payload.stages[0];assert_eq!(stage.capability.id,RIFE_CAPABILITY_ID);assert!(stage.provider_lock.payload.offline);
    assert!(stage.provider_lock.payload.authorized_side_effects.contains(&crate::SideEffect::Ai));assert!(!stage.provider_lock.payload.authorized_side_effects.contains(&crate::SideEffect::Network));
    assert!(plan.payload.inputs.iter().any(|i|i.id=="source_index"&&i.content_sha256==identity(&request.frame_index,RIFE_MAXIMUM_INDEX_BYTES).unwrap().0));
    assert_eq!(stage.provider_lock.payload.models.len(),2);let mut marker=request.configuration.settings.tool.executable.as_os_str().to_os_string();marker.push(".launched");assert!(!PathBuf::from(marker).exists());
}
#[test]
fn changed_frame_and_model_bytes_refuse_before_workspace_creation() {
    let (root,request)=fixture();let output=root.path().canonicalize().unwrap().join("runs");fs::write(request.frames_directory.join("undeclared.png"),b"extra").unwrap();
    assert!(run(request,Some(output.clone()),&CancellationToken::default(),|_|{}).is_err());assert!(!output.exists());
    let (root,request)=fixture();let output=root.path().canonicalize().unwrap().join("runs");fs::write(request.configuration.settings.model.directory.join("flownet.bin"),b"changed").unwrap();
    assert!(run(request,Some(output.clone()),&CancellationToken::default(),|_|{}).is_err());assert!(!output.exists());
}
#[test]
fn disabled_or_overlapping_rife_requests_cannot_become_native_runs() {
    let (_root,mut request)=fixture();request.configuration.enabled=false;assert!(plan(&request,&CancellationToken::default()).is_err());
    request.configuration.enabled=true;let output=request.frames_directory.join("must-not-create");
    assert!(run(request,Some(output.clone()),&CancellationToken::default(),|_|{}).is_err());assert!(!output.exists());
}
#[test]
fn loose_uninitialized_runs_and_cancelled_admission_preserve_existing_files() {
    let (root,request)=fixture();let loose=root.path().canonicalize().unwrap().join("loose");fs::create_dir(&loose).unwrap();fs::write(loose.join("index.json"),b"unaccepted").unwrap();
    assert!(status(&loose).is_err());assert!(resume(request.clone(),&loose,&CancellationToken::default(),|_|{}).is_err());
    let token=CancellationToken::default();token.cancel();assert!(plan(&request,&token).is_err());assert_eq!(fs::read(loose.join("index.json")).unwrap(),b"unaccepted");assert!(!loose.join("writer.lock").exists());
}
