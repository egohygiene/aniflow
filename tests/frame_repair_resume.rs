//! Synthetic runtime-policy regression, authored for deferred #64 execution.
//!
//! The fixture declares the repair capability to exercise resume admission. It
//! neither invokes the native repair adapter nor proves native qualification.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use aniflow::{
    CancellationToken, CapabilityReference, ComponentInventory, DeterminismClass,
    HostResources, PipelineInputBinding, PipelinePlanningContext, PipelineV3Configuration,
    PipelineV3ResumeRequest, PipelineV3RunProgress, PipelineV3RunRequest, PipelineV3RunState,
    ProviderConfiguration, ProviderManifest, ProviderReference, ProviderRegistration,
    ProviderRegistry, SideEffect, resume_v3_with_progress_and_cancellation,
    run_v3_with_progress_and_cancellation, status_v3,
};
use walkdir::WalkDir;

const PIPELINE: &str = r#"
schema: aniflow.pipeline/v3
name: synthetic-repair-resume-admission
inputs:
  - id: source
    artifact_type: application/octet-stream
    artifact_role: temporal_component
    stream_role: video
stages:
  - id: repair_frames
    capability:
      id: aniflow/frame-repair
      version_requirement: '=1.0.0'
    provider:
      primary:
        registration_id: repair-policy-fixture
    inputs:
      - port: frames
        artifacts: [source]
    outputs:
      - port: processed_frames
        artifacts:
          - id: copy
            relative_path: artifacts/repair_frames/copy.bin
            kind: file
    validations:
      - id: copy-integrity
        artifact: copy
        contract: aniflow.validation/artifact-integrity/v1
outputs:
  - id: result
    artifact: copy
    required_validations: [copy-integrity]
"#;

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    WalkDir::new(root).min_depth(1).into_iter().map(|entry| {
        let entry = entry.unwrap();
        assert!(!entry.file_type().is_symlink());
        let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
        let bytes = entry.file_type().is_file().then(|| fs::read(entry.path()).unwrap());
        (relative, bytes)
    }).collect()
}

#[test]
fn repair_v1_resume_requires_a_fresh_run_before_progress_or_durable_changes() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = root.join("source.bin");
    fs::write(&source, b"synthetic immutable input").unwrap();
    let executable = root.join("unexpected-provider-launch");
    fs::write(&executable, "#!/bin/sh\nprintf 'launched\\n' >> \"$(dirname \"$0\")/launched\"\nexit 87\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();

    let mut manifest = ProviderManifest::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-manifest-v1.example.json"
    )).unwrap();
    manifest.capabilities[0].id = "aniflow/frame-repair".to_owned();
    manifest.capabilities[0].inputs[0].artifact_type = "application/octet-stream".to_owned();
    manifest.capabilities[0].outputs[0].artifact_type = "application/octet-stream".to_owned();
    manifest.capabilities[0].requirements.tools.clear();
    manifest.capabilities[0].requirements.codecs.clear();
    manifest.capabilities[0].requirements.models.clear();
    manifest.capabilities[0].behavior.determinism = DeterminismClass::EnvironmentDependent;
    manifest.capabilities[0].behavior.cacheable = false;
    let published = ProviderConfiguration::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-configuration-v1.example.json"
    )).unwrap();
    let configuration = ProviderConfiguration::new(
        ProviderReference { id:manifest.provider.id.clone(), version:manifest.provider.version.clone() },
        CapabilityReference { id:"aniflow/frame-repair".to_owned(), version:"1.0.0".to_owned() },
        published.configuration_schema, published.values,
    ).unwrap();
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderRegistration::new(
        "repair-policy-fixture", manifest, configuration, executable, "synthetic-policy-only",
        ComponentInventory { tools:Vec::new(), codecs:Vec::new(), models:Vec::new() },
    ).unwrap()).unwrap();
    let bindings = vec![PipelineInputBinding::new("source", &source)];
    let plan = aniflow::resolve_pipeline_v3(
        &PipelineV3Configuration::from_yaml_str(PIPELINE).unwrap(), &bindings, &registry,
        &PipelinePlanningContext {
            host:HostResources { cpu_threads:8, memory_mib:16*1024, storage_mib:64*1024, gpu_available:true, network_available:false },
            allowed_side_effects:vec![SideEffect::FilesystemRead,SideEffect::FilesystemWrite,SideEffect::Subprocess,SideEffect::Gpu],
            offline:true,
        },
    ).unwrap();

    // Seed a valid interrupted run through the public runtime without launching
    // its provider. Resume must not turn this into an implicit processor retry.
    let cancellation = CancellationToken::default();
    cancellation.cancel();
    let mut run_directory = None;
    run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, bindings.clone(), registry.clone())
            .with_output_directory(root.join("runs")),
        &cancellation,
        |event| if let PipelineV3RunProgress::Started { run_directory:started, .. } = event {
            run_directory = Some(started.clone());
        },
    ).expect_err("pre-cancellation must retain a cancelled run before provider launch");
    let run_directory = run_directory.expect("a durable run must exist before the resume exercise");
    let status = status_v3(&run_directory).unwrap();
    assert_eq!(status.payload.state, PipelineV3RunState::Cancelled);
    assert!(!root.join("launched").exists());
    let before = snapshot(&run_directory);

    for rerun_stages in [Vec::new(), vec!["repair_frames".to_owned()]] {
        let mut progressed = false;
        let error = resume_v3_with_progress_and_cancellation(
            PipelineV3ResumeRequest::new(&run_directory, bindings.clone(), registry.clone())
                .with_rerun_stages(rerun_stages),
            &CancellationToken::default(),
            |_| progressed = true,
        ).expect_err("unobserved repair environment cannot be resumed or force-rerun in place");
        assert!(error.to_string().contains("unobserved model/environment identity"));
        assert!(error.to_string().contains("fresh video repair-frames run"));
        assert!(!progressed, "refusal must precede even the resume Started event");
        assert!(!root.join("launched").exists(), "refusal must not launch a provider");
        assert_eq!(snapshot(&run_directory), before, "refusal must preserve all durable run files");
        assert_eq!(status_v3(&run_directory).unwrap(), status);
    }
}
