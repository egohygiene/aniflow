#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Duration;

use aniflow::{
    ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1, ArtifactCardinality, ArtifactValidation,
    CancellationToken, CapabilityReference, CompatibilityReasonCode, ComponentIdentity,
    ComponentInventory, ErrorCategory, EvidenceReference, ExpectedArtifact, FinalOutputRequirement,
    HostResources, PipelineInputBinding, PipelineInputKind, PipelinePlanningContext,
    PipelineV3Configuration, PipelineV3Plan, PipelineV3ProgressState, PipelineV3ResumeRequest,
    PipelineV3RunProgress, PipelineV3RunRequest, PipelineV3RunState, PipelineV3StageState,
    ProviderConfiguration, ProviderExecutionFailureCode, ProviderExecutionLimits,
    ProviderExecutionOutcome, ProviderExecutionReport, ProviderManifest, ProviderReference,
    ProviderRegistration, ProviderRegistry, SideEffect, StageCheckpoint, resume_v3,
    resume_v3_with_progress_and_cancellation, run_v3, run_v3_with_progress_and_cancellation,
    status_v3,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;
use walkdir::WalkDir;

const MODEL_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const PIPELINE: &str = r#"
schema: "aniflow.pipeline/v3"
name: "execution-copy"
inputs:
  - id: "source"
    artifact_type: "application/octet-stream"
    artifact_role: "temporal_component"
    stream_role: "video"
stages:
  - id: "enhance"
    capability:
      id: "aniflow/frame.process"
      version_requirement: "^1.0"
    provider:
      primary:
        registration_id: "fixture-local"
    inputs:
      - port: "frames"
        artifacts: ["source"]
    outputs:
      - port: "processed_frames"
        artifacts:
          - id: "copy"
            relative_path: "artifacts/enhance/copy.bin"
            kind: "file"
    validations:
      - id: "copy-integrity"
        artifact: "copy"
        contract: "aniflow.validation/artifact-integrity/v1"
outputs:
  - id: "result"
    artifact: "copy"
    required_validations: ["copy-integrity"]
"#;

const TWO_STAGE_PIPELINE: &str = r#"
schema: "aniflow.pipeline/v3"
name: "execution-recovery-chain"
inputs:
  - id: "source"
    artifact_type: "application/octet-stream"
    artifact_role: "temporal_component"
    stream_role: "video"
stages:
  - id: "stage-a"
    capability:
      id: "aniflow/frame.process"
      version_requirement: "^1.0"
    provider:
      primary:
        registration_id: "fixture-stage-a"
    inputs:
      - port: "frames"
        artifacts: ["source"]
    outputs:
      - port: "processed_frames"
        artifacts:
          - id: "stage-a-output"
            relative_path: "artifacts/stage-a/output.bin"
            kind: "file"
    validations:
      - id: "stage-a-integrity"
        artifact: "stage-a-output"
        contract: "aniflow.validation/artifact-integrity/v1"
  - id: "stage-b"
    depends_on: ["stage-a"]
    capability:
      id: "aniflow/frame.process"
      version_requirement: "^1.0"
    provider:
      primary:
        registration_id: "fixture-stage-b"
    inputs:
      - port: "frames"
        artifacts: ["stage-a-output"]
    outputs:
      - port: "processed_frames"
        artifacts:
          - id: "stage-b-output"
            relative_path: "artifacts/stage-b/output.bin"
            kind: "file"
    validations:
      - id: "stage-b-integrity"
        artifact: "stage-b-output"
        contract: "aniflow.validation/artifact-integrity/v1"
outputs:
  - id: "result"
    artifact: "stage-b-output"
    required_validations: ["stage-b-integrity"]
"#;

#[test]
fn successful_run_publishes_validated_output_and_immutable_checkpoint() {
    let fixture = ExecutionFixture::new("successful-run");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);

    let outcome = run_v3(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(fixture.runs_directory()),
    )
    .expect("Pipeline v3 run should succeed");

    assert_eq!(outcome.executed_stages, vec!["enhance".to_owned()]);
    assert!(outcome.reused_stages.is_empty());
    assert!(outcome.run_manifest.is_file());
    assert_eq!(outcome.outputs.len(), 1);
    assert_eq!(
        fs::read(&outcome.outputs[0].path).expect("published output should be readable"),
        fs::read(&fixture.source).expect("source should be readable")
    );
    assert_eq!(fixture.launch_count(), 1);

    let manifest = status_v3(&outcome.run_directory).expect("completed run should have status");
    assert_eq!(manifest.payload.state, PipelineV3RunState::Complete);
    assert_eq!(
        manifest.payload.stages[0].state,
        PipelineV3StageState::Complete
    );
    let checkpoint = manifest.payload.stages[0]
        .checkpoint
        .as_ref()
        .expect("completed stage should identify its checkpoint");
    assert!(
        outcome
            .run_directory
            .join(&checkpoint.relative_path)
            .is_file()
    );
    assert!(
        WalkDir::new(outcome.run_directory.join("work"))
            .into_iter()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_type().is_file()),
        "ephemeral provider invocation was retained in the run workspace"
    );
}

#[test]
fn started_exposes_workspace_before_fallible_state_initialization() {
    let fixture = ExecutionFixture::new("started-before-initialization");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let mut run_directory = None;

    let error = run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(fixture.runs_directory()),
        &CancellationToken::default(),
        |progress| {
            if let PipelineV3RunProgress::Started {
                run_directory: started,
                ..
            } = progress
            {
                run_directory = Some(started.clone());
                assert!(
                    started.is_dir(),
                    "Started must identify a durable workspace"
                );
                assert!(
                    !started.join("plan/plan.json").exists(),
                    "Started was emitted after fallible plan publication"
                );
                assert_eq!(
                    fs::read_dir(started.join("state/manifests"))
                        .expect("manifest directory should exist at Started")
                        .count(),
                    0,
                    "Started was emitted after the initial manifest"
                );

                fs::create_dir(started.join("plan/plan.json"))
                    .expect("initialization collision fixture should be created");
            }
        },
    )
    .expect_err("a plan-publication collision must fail initialization");

    assert_eq!(error.category(), ErrorCategory::State);
    let run_directory = run_directory.expect("Started should retain the allocated workspace");
    assert!(run_directory.is_dir());
    assert_eq!(
        fs::read_dir(run_directory.join("state/manifests"))
            .expect("manifest directory should remain inspectable")
            .count(),
        0,
        "failed initialization unexpectedly published a manifest"
    );
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn provider_hardlink_output_is_published_as_an_independent_inode() {
    let fixture = ExecutionFixture::new("hardlink-output");
    fs::write(fixture.root.join("hardlink-mode"), b"")
        .expect("hardlink provider mode should be enabled");

    let outcome = fixture.run();
    let source_metadata = fs::metadata(&fixture.source).expect("source metadata should exist");
    let output_metadata =
        fs::metadata(&outcome.outputs[0].path).expect("accepted output metadata should exist");

    assert_ne!(
        (source_metadata.dev(), source_metadata.ino()),
        (output_metadata.dev(), output_metadata.ino()),
        "accepted output retained the provider's hardlink alias"
    );
    fs::write(&outcome.outputs[0].path, b"mutated accepted output")
        .expect("accepted output should be writable for the regression check");
    assert_eq!(
        fs::read(&fixture.source).expect("immutable source should remain readable"),
        b"immutable source bytes",
        "mutating an accepted output changed its immutable input"
    );
}

#[test]
fn directory_source_uses_planning_identity_during_execution_revalidation() {
    let fixture = ExecutionFixture::new("directory-source");
    let source_directory = fixture.root.join("source-directory");
    fs::create_dir(&source_directory).expect("directory source should be created");
    fs::write(source_directory.join("frame.bin"), b"immutable frame bytes")
        .expect("directory source content should be written");
    let bindings = vec![PipelineInputBinding::new("source", &source_directory)];
    let mut configuration = fixture.configuration();
    configuration.stages[0].outputs[0].artifacts[0].kind = Some(PipelineInputKind::Directory);
    let registry = fixture.registry(ArtifactCardinality::One);
    let plan =
        aniflow::resolve_pipeline_v3(&configuration, &bindings, &registry, &planning_context())
            .expect("directory source should plan");

    let outcome = run_v3(
        PipelineV3RunRequest::new(plan, bindings, registry)
            .with_output_directory(fixture.runs_directory()),
    )
    .expect("directory source should retain the planning digest during execution");

    assert_eq!(
        fs::read(outcome.outputs[0].path.join("frame.bin"))
            .expect("copied directory output should be readable"),
        b"immutable frame bytes"
    );
    assert_eq!(fixture.launch_count(), 1);
}

#[test]
fn unchanged_resume_reuses_checkpoint_without_relaunching_provider() {
    let fixture = ExecutionFixture::new("unchanged-resume");
    let outcome = fixture.run();
    assert_eq!(fixture.launch_count(), 1);

    let resumed = resume_v3(PipelineV3ResumeRequest::new(
        &outcome.run_directory,
        fixture.input_bindings(),
        fixture.registry(ArtifactCardinality::One),
    ))
    .expect("unchanged run should resume from its checkpoint");

    assert!(resumed.executed_stages.is_empty());
    assert_eq!(resumed.reused_stages, vec!["enhance".to_owned()]);
    assert_eq!(fixture.launch_count(), 1, "resume relaunched provider");
    assert_eq!(
        fs::read(&resumed.outputs[0].path).expect("reused output should be readable"),
        fs::read(&fixture.source).expect("source should be readable")
    );
}

#[test]
fn interrupted_after_durable_stage_completion_reuses_checkpoint_on_resume() {
    let fixture = ExecutionFixture::new("interrupted-after-stage-complete");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let mut run_directory = None;

    let aborted = catch_unwind(AssertUnwindSafe(|| {
        run_v3_with_progress_and_cancellation(
            PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
                .with_output_directory(fixture.runs_directory()),
            &CancellationToken::default(),
            |progress| match progress {
                PipelineV3RunProgress::Started {
                    run_directory: started,
                    ..
                } => run_directory = Some(started.clone()),
                PipelineV3RunProgress::Stage {
                    stage_id,
                    state: PipelineV3ProgressState::Complete,
                } if stage_id == "enhance" => {
                    panic!("simulate process abort after durable stage completion")
                }
                _ => {}
            },
        )
        .expect("the simulated process abort should preempt successful finalization");
    }));
    assert!(
        aborted.is_err(),
        "the crash fixture did not interrupt the run"
    );

    let run_directory = run_directory.expect("started run should expose its workspace");
    let interrupted = status_v3(&run_directory).expect("interrupted state should remain readable");
    assert_eq!(interrupted.payload.state, PipelineV3RunState::Running);
    assert_eq!(
        interrupted.payload.stages[0].state,
        PipelineV3StageState::Complete
    );
    let durable_checkpoint = interrupted.payload.stages[0]
        .checkpoint
        .clone()
        .expect("durably completed stage should retain its checkpoint");
    assert_eq!(fixture.launch_count(), 1);

    let resumed = fixture.resume(&run_directory);

    assert!(resumed.executed_stages.is_empty());
    assert_eq!(resumed.reused_stages, vec!["enhance".to_owned()]);
    assert_eq!(fixture.launch_count(), 1, "resume relaunched the provider");
    let completed = status_v3(&run_directory).expect("resumed run should complete");
    assert_eq!(completed.payload.state, PipelineV3RunState::Complete);
    assert_eq!(
        completed.payload.stages[0].checkpoint.as_ref(),
        Some(&durable_checkpoint)
    );
}

#[test]
fn changed_execution_limits_invalidate_checkpoint_and_relaunch_provider() {
    let fixture = ExecutionFixture::new("changed-execution-limits");
    let outcome = fixture.run();
    assert_eq!(fixture.launch_count(), 1);

    let execution_limits = ProviderExecutionLimits {
        timeout: Duration::from_secs(123),
        ..ProviderExecutionLimits::default()
    };
    let resumed = resume_v3(
        PipelineV3ResumeRequest::new(
            &outcome.run_directory,
            fixture.input_bindings(),
            fixture.registry(ArtifactCardinality::One),
        )
        .with_execution_limits(execution_limits),
    )
    .expect("changed execution limits should invalidate and rebuild the stage");

    assert_eq!(resumed.executed_stages, vec!["enhance".to_owned()]);
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(fixture.launch_count(), 2, "resume reused a stale policy");
}

#[test]
fn pre_cancelled_resume_preserves_completed_checkpoint_across_retries() {
    let fixture = ExecutionFixture::new("cancelled-resume");
    let outcome = fixture.run();
    let completed = status_v3(&outcome.run_directory).expect("completed run should have status");
    let checkpoint = completed.payload.stages[0]
        .checkpoint
        .clone()
        .expect("completed stage should retain its checkpoint");

    for _ in 0..2 {
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let error = resume_v3_with_progress_and_cancellation(
            PipelineV3ResumeRequest::new(
                &outcome.run_directory,
                fixture.input_bindings(),
                fixture.registry(ArtifactCardinality::One),
            ),
            &cancellation,
            |_| {},
        )
        .expect_err("pre-cancelled resume must stop before checkpoint assessment");
        assert_eq!(error.category(), ErrorCategory::Execution);

        let cancelled =
            status_v3(&outcome.run_directory).expect("cancelled resume should retain valid state");
        assert_eq!(cancelled.payload.state, PipelineV3RunState::Cancelled);
        assert_eq!(
            cancelled.payload.stages[0].state,
            PipelineV3StageState::Complete
        );
        assert_eq!(
            cancelled.payload.stages[0].checkpoint.as_ref(),
            Some(&checkpoint)
        );
    }
    assert_eq!(fixture.launch_count(), 1);
}

#[test]
fn resume_started_precedes_the_first_state_mutation() {
    let fixture = ExecutionFixture::new("resume-started-before-mutation");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let mut run_directory = None;

    let aborted = catch_unwind(AssertUnwindSafe(|| {
        run_v3_with_progress_and_cancellation(
            PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
                .with_output_directory(fixture.runs_directory()),
            &CancellationToken::default(),
            |progress| match progress {
                PipelineV3RunProgress::Started {
                    run_directory: started,
                    ..
                } => run_directory = Some(started.clone()),
                PipelineV3RunProgress::Stage {
                    stage_id,
                    state: PipelineV3ProgressState::Running,
                } if stage_id == "enhance" => {
                    panic!("simulate process abort before provider launch")
                }
                _ => {}
            },
        )
        .expect("the simulated process abort should preempt execution");
    }));
    assert!(
        aborted.is_err(),
        "the crash fixture did not interrupt the run"
    );

    let run_directory = run_directory.expect("started run should expose its workspace");
    assert_eq!(
        status_v3(&run_directory)
            .expect("interrupted run should remain readable")
            .payload
            .state,
        PipelineV3RunState::Running
    );
    let manifest_count_before = fs::read_dir(run_directory.join("state/manifests"))
        .expect("manifest directory should remain readable")
        .count();
    let cancellation = CancellationToken::default();
    let mut observed_started = false;

    let error = resume_v3_with_progress_and_cancellation(
        PipelineV3ResumeRequest::new(
            &run_directory,
            fixture.input_bindings(),
            fixture.registry(ArtifactCardinality::One),
        ),
        &cancellation,
        |progress| {
            if let PipelineV3RunProgress::Started {
                run_directory: started,
                ..
            } = progress
            {
                observed_started = true;
                assert_eq!(started, &run_directory);
                assert_eq!(
                    status_v3(started)
                        .expect("Started should precede resume state mutation")
                        .payload
                        .state,
                    PipelineV3RunState::Running
                );
                assert_eq!(
                    fs::read_dir(started.join("state/manifests"))
                        .expect("manifest directory should remain readable")
                        .count(),
                    manifest_count_before,
                    "resume mutated state before emitting Started"
                );
                cancellation.cancel();
            }
        },
    )
    .expect_err("cancellation after Started should stop the resumed run");

    assert_eq!(error.category(), ErrorCategory::Execution);
    assert!(observed_started, "resume did not emit Started");
    assert_eq!(
        status_v3(&run_directory)
            .expect("cancelled resume should retain valid status")
            .payload
            .state,
        PipelineV3RunState::Cancelled
    );
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn tampered_output_invalidates_checkpoint_and_reruns_stage() {
    let fixture = ExecutionFixture::new("tampered-output");
    let outcome = fixture.run();
    fs::write(&outcome.outputs[0].path, b"tampered bytes")
        .expect("published fixture output should be mutable for the test");

    let resumed = fixture.resume(&outcome.run_directory);

    assert_eq!(resumed.executed_stages, vec!["enhance".to_owned()]);
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(fixture.launch_count(), 2);
    assert_eq!(
        fs::read(&resumed.outputs[0].path).expect("rebuilt output should be readable"),
        fs::read(&fixture.source).expect("source should be readable")
    );
}

#[test]
fn contradictory_execution_report_outputs_invalidate_checkpoint() {
    let fixture = ExecutionFixture::new("contradictory-report-outputs");
    let outcome = fixture.run();
    let (original_checkpoint_path, checkpoint) =
        stage_checkpoint(&outcome.run_directory, "enhance");
    let mut report = ProviderExecutionReport::from_json_slice(
        &fs::read(
            outcome
                .run_directory
                .join(&checkpoint.payload.execution_report.relative_path),
        )
        .expect("execution report should be readable"),
    )
    .expect("execution report should validate before the tamper fixture");

    report.payload.outputs[0].sha256 = "0".repeat(64);
    report.report_sha256 = canonical_sha256(&report.payload);
    report
        .validate()
        .expect("contradictory report should remain internally self-validating");
    let report_relative_path = bound_execution_report_path(
        "enhance",
        &checkpoint.payload.stage_invocation_sha256,
        &report.report_sha256,
    );
    write_json(&outcome.run_directory.join(&report_relative_path), &report);

    let mut payload = checkpoint.payload;
    payload.execution_report = EvidenceReference {
        relative_path: report_relative_path,
        sha256: report.report_sha256,
    };
    let contradictory_checkpoint =
        StageCheckpoint::new(payload).expect("contradictory checkpoint should self-validate");
    write_json(
        &outcome.run_directory.join(format!(
            "state/checkpoints/enhance-{}.json",
            contradictory_checkpoint.checkpoint_sha256
        )),
        &contradictory_checkpoint,
    );
    fs::write(original_checkpoint_path, b"{}")
        .expect("original checkpoint should be corruptible for the fixture");

    let resumed = fixture.resume(&outcome.run_directory);

    assert_eq!(resumed.executed_stages, vec!["enhance".to_owned()]);
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(fixture.launch_count(), 2);
}

#[test]
fn transplanted_execution_report_locator_invalidates_checkpoint() {
    let fixture = ExecutionFixture::new("transplanted-report-locator");
    let outcome = fixture.run();
    let (original_checkpoint_path, checkpoint) =
        stage_checkpoint(&outcome.run_directory, "enhance");
    let original_report_path = outcome
        .run_directory
        .join(&checkpoint.payload.execution_report.relative_path);
    let transplanted_relative_path = format!(
        "providers/foreign-stage-{}.report.json",
        checkpoint.payload.execution_report.sha256
    );
    fs::copy(
        original_report_path,
        outcome.run_directory.join(&transplanted_relative_path),
    )
    .expect("execution report should be transplantable for the fixture");

    let mut payload = checkpoint.payload;
    payload.execution_report.relative_path = transplanted_relative_path;
    let transplanted_checkpoint =
        StageCheckpoint::new(payload).expect("transplanted checkpoint should self-validate");
    write_json(
        &outcome.run_directory.join(format!(
            "state/checkpoints/enhance-{}.json",
            transplanted_checkpoint.checkpoint_sha256
        )),
        &transplanted_checkpoint,
    );
    fs::write(original_checkpoint_path, b"{}")
        .expect("original checkpoint should be corruptible for the fixture");

    let resumed = fixture.resume(&outcome.run_directory);

    assert_eq!(resumed.executed_stages, vec!["enhance".to_owned()]);
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(fixture.launch_count(), 2);
}

#[test]
fn tampered_upstream_output_rebuilds_the_complete_dependency_frontier() {
    let temporary = tempfile::Builder::new()
        .prefix("aniflow-v3-execution-dependency-recovery-")
        .tempdir()
        .expect("temporary recovery fixture should be created");
    let root = temporary.path();
    let source = root.join("source.bin");
    let source_bytes = b"immutable source bytes";
    fs::write(&source, source_bytes).expect("recovery source should be written");

    let stage_a_root = root.join("provider-stage-a");
    let stage_b_root = root.join("provider-stage-b");
    fs::create_dir(&stage_a_root).expect("stage A provider root should be created");
    fs::create_dir(&stage_b_root).expect("stage B provider root should be created");
    let stage_a_executable = write_provider_executable(&stage_a_root);
    let stage_b_executable = write_provider_executable(&stage_b_root);
    let stage_a_launch_count = stage_a_root.join("launch-count");
    let stage_b_launch_count = stage_b_root.join("launch-count");

    let configuration = PipelineV3Configuration::from_yaml_str(TWO_STAGE_PIPELINE)
        .expect("two-stage recovery pipeline should parse");
    let bindings = vec![PipelineInputBinding::new("source", &source)];
    let mut registry = ProviderRegistry::new();
    registry
        .register(provider_registration_named(
            &stage_a_executable,
            "fixture-stage-a",
            ArtifactCardinality::One,
        ))
        .expect("stage A provider registration should be unique");
    registry
        .register(provider_registration_named(
            &stage_b_executable,
            "fixture-stage-b",
            ArtifactCardinality::One,
        ))
        .expect("stage B provider registration should be unique");
    let plan =
        aniflow::resolve_pipeline_v3(&configuration, &bindings, &registry, &planning_context())
            .expect("two-stage recovery pipeline should plan");

    let outcome = run_v3(
        PipelineV3RunRequest::new(plan, bindings.clone(), registry.clone())
            .with_output_directory(root.join("runs")),
    )
    .expect("initial two-stage run should complete");
    assert_eq!(read_launch_count(&stage_a_launch_count), 1);
    assert_eq!(read_launch_count(&stage_b_launch_count), 1);
    let initial = status_v3(&outcome.run_directory).expect("initial run status should be valid");
    let initial_stage_a_checkpoint = initial.payload.stages[0]
        .checkpoint
        .clone()
        .expect("stage A should have a checkpoint");
    let initial_stage_b_checkpoint = initial.payload.stages[1]
        .checkpoint
        .clone()
        .expect("stage B should have a checkpoint");

    let accepted_stage_a = outcome.run_directory.join("artifacts/stage-a/output.bin");
    fs::write(&accepted_stage_a, b"tampered upstream bytes")
        .expect("accepted stage A output should be mutable for the recovery test");

    let resumed = resume_v3(PipelineV3ResumeRequest::new(
        &outcome.run_directory,
        bindings,
        registry,
    ))
    .expect("resume should rebuild the invalidated dependency frontier");

    assert_eq!(
        resumed.executed_stages,
        vec!["stage-a".to_owned(), "stage-b".to_owned()]
    );
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(read_launch_count(&stage_a_launch_count), 2);
    assert_eq!(read_launch_count(&stage_b_launch_count), 2);
    let recovered = status_v3(&outcome.run_directory).expect("recovered run should be valid");
    assert_ne!(
        recovered.payload.stages[0].checkpoint.as_ref(),
        Some(&initial_stage_a_checkpoint)
    );
    assert_ne!(
        recovered.payload.stages[1].checkpoint.as_ref(),
        Some(&initial_stage_b_checkpoint)
    );
    assert_eq!(
        fs::read(&accepted_stage_a).expect("rebuilt stage A output should be readable"),
        source_bytes
    );
    assert_eq!(
        fs::read(&resumed.outputs[0].path).expect("recovered final output should be readable"),
        source_bytes
    );
    assert_eq!(
        fs::read(&source).expect("immutable source should remain readable"),
        source_bytes
    );
}

#[test]
fn failed_upstream_rebuild_persists_the_complete_invalidation_frontier() {
    let temporary = tempfile::Builder::new()
        .prefix("aniflow-v3-execution-failed-frontier-recovery-")
        .tempdir()
        .expect("temporary recovery fixture should be created");
    let root = temporary.path();
    let source = root.join("source.bin");
    fs::write(&source, b"immutable source bytes").expect("recovery source should be written");

    let stage_a_root = root.join("provider-stage-a");
    let stage_b_root = root.join("provider-stage-b");
    fs::create_dir(&stage_a_root).expect("stage A provider root should be created");
    fs::create_dir(&stage_b_root).expect("stage B provider root should be created");
    let stage_a_executable = write_provider_executable(&stage_a_root);
    let stage_b_executable = write_provider_executable(&stage_b_root);
    let stage_a_launch_count = stage_a_root.join("launch-count");
    let stage_b_launch_count = stage_b_root.join("launch-count");

    let configuration = PipelineV3Configuration::from_yaml_str(TWO_STAGE_PIPELINE)
        .expect("two-stage recovery pipeline should parse");
    let bindings = vec![PipelineInputBinding::new("source", &source)];
    let mut registry = ProviderRegistry::new();
    registry
        .register(provider_registration_named(
            &stage_a_executable,
            "fixture-stage-a",
            ArtifactCardinality::One,
        ))
        .expect("stage A provider registration should be unique");
    registry
        .register(provider_registration_named(
            &stage_b_executable,
            "fixture-stage-b",
            ArtifactCardinality::One,
        ))
        .expect("stage B provider registration should be unique");
    let plan =
        aniflow::resolve_pipeline_v3(&configuration, &bindings, &registry, &planning_context())
            .expect("two-stage recovery pipeline should plan");

    let outcome = run_v3(
        PipelineV3RunRequest::new(plan, bindings.clone(), registry.clone())
            .with_output_directory(root.join("runs")),
    )
    .expect("initial two-stage run should complete");
    let accepted_stage_a = outcome.run_directory.join("artifacts/stage-a/output.bin");
    fs::write(&accepted_stage_a, b"tampered upstream bytes")
        .expect("accepted stage A output should be mutable for the recovery test");
    fs::write(stage_a_root.join("nonzero-mode"), b"")
        .expect("stage A retry failure mode should be enabled");

    let mut progress_events = Vec::new();
    let error = resume_v3_with_progress_and_cancellation(
        PipelineV3ResumeRequest::new(&outcome.run_directory, bindings, registry),
        &CancellationToken::default(),
        |event| progress_events.push(event.clone()),
    )
    .expect_err("the corrupted stage A retry should fail");

    assert_eq!(error.category(), ErrorCategory::Execution);
    assert_eq!(read_launch_count(&stage_a_launch_count), 2);
    assert_eq!(read_launch_count(&stage_b_launch_count), 1);
    let failed = status_v3(&outcome.run_directory).expect("failed resume state should be valid");
    assert_eq!(failed.payload.state, PipelineV3RunState::Failed);
    assert_eq!(failed.payload.stages[0].state, PipelineV3StageState::Failed);
    assert_eq!(
        failed.payload.stages[1].state,
        PipelineV3StageState::Invalidated
    );
    assert!(failed.payload.stages[1].checkpoint.is_none());
    let downstream_compatibility = failed.payload.stages[1]
        .compatibility
        .as_ref()
        .expect("invalidated stage B should retain its compatibility decision");
    assert!(!downstream_compatibility.reusable);
    assert!(
        downstream_compatibility
            .reasons
            .iter()
            .any(|reason| { reason.code == CompatibilityReasonCode::DependencyChanged })
    );

    let stage_b_invalidated = progress_events
        .iter()
        .position(|event| {
            matches!(
                event,
                PipelineV3RunProgress::Stage {
                    stage_id,
                    state: PipelineV3ProgressState::Invalidated,
                } if stage_id == "stage-b"
            )
        })
        .expect("stage B invalidation should be observable");
    let stage_a_running = progress_events
        .iter()
        .position(|event| {
            matches!(
                event,
                PipelineV3RunProgress::Stage {
                    stage_id,
                    state: PipelineV3ProgressState::Running,
                } if stage_id == "stage-a"
            )
        })
        .expect("stage A retry should become observable");
    assert!(
        stage_b_invalidated < stage_a_running,
        "the downstream invalidation was not durable before the upstream retry began"
    );
}

#[test]
fn missing_output_invalidates_checkpoint_and_reruns_stage() {
    let fixture = ExecutionFixture::new("missing-output");
    let outcome = fixture.run();
    fs::remove_file(&outcome.outputs[0].path)
        .expect("published fixture output should be removable for the test");

    let resumed = fixture.resume(&outcome.run_directory);

    assert_eq!(resumed.executed_stages, vec!["enhance".to_owned()]);
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(fixture.launch_count(), 2);
    assert_eq!(
        fs::read(&resumed.outputs[0].path).expect("rebuilt output should be readable"),
        fs::read(&fixture.source).expect("source should be readable")
    );
}

#[test]
fn changed_source_fails_as_state_error_without_mutating_run() {
    let fixture = ExecutionFixture::new("changed-source");
    let outcome = fixture.run();
    let before = snapshot_tree(&outcome.run_directory);
    fs::write(&fixture.source, b"different source bytes")
        .expect("source fixture should be mutable for the test");

    let error = resume_v3(PipelineV3ResumeRequest::new(
        &outcome.run_directory,
        fixture.input_bindings(),
        fixture.registry(ArtifactCardinality::One),
    ))
    .expect_err("changed immutable source must fail closed");

    assert_eq!(error.category(), ErrorCategory::State);
    assert_eq!(fixture.launch_count(), 1);
    assert_eq!(snapshot_tree(&outcome.run_directory), before);
}

#[test]
fn source_race_after_workspace_creation_records_failed_terminal_state() {
    let fixture = ExecutionFixture::new("source-race");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let source = fixture.source.clone();
    let mut run_directory = None;

    let error = run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(fixture.runs_directory()),
        &CancellationToken::default(),
        |progress| {
            if let PipelineV3RunProgress::Started {
                run_directory: started,
                ..
            } = progress
            {
                run_directory = Some(started.clone());
                fs::write(&source, b"source changed after preflight")
                    .expect("source race fixture should be writable");
            }
        },
    )
    .expect_err("source changes after workspace creation must fail closed");

    assert_eq!(error.category(), ErrorCategory::State);
    let status = status_v3(run_directory.expect("run should emit its workspace before execution"))
        .expect("failed run should retain valid terminal state");
    assert_eq!(status.payload.state, PipelineV3RunState::Failed);
    assert_eq!(status.payload.stages[0].state, PipelineV3StageState::Failed);
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn provider_exit_failure_records_failed_state_and_execution_evidence() {
    let fixture = ExecutionFixture::new("provider-exit-failure");
    fs::write(fixture.root.join("nonzero-mode"), b"")
        .expect("provider failure mode should be enabled");
    let immutable_source = fs::read(&fixture.source).expect("source fixture should be readable");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let mut run_directory = None;

    let error = run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(fixture.runs_directory()),
        &CancellationToken::default(),
        |progress| {
            if let PipelineV3RunProgress::Started {
                run_directory: started,
                ..
            } = progress
            {
                run_directory = Some(started.clone());
            }
        },
    )
    .expect_err("a non-zero provider exit must fail the Pipeline v3 run");

    assert_eq!(error.category(), ErrorCategory::Execution);
    let run_directory = run_directory.expect("started run should expose its workspace");
    let status = status_v3(&run_directory).expect("failed run should retain valid status");
    assert_eq!(status.payload.state, PipelineV3RunState::Failed);
    assert_eq!(status.payload.stages[0].state, PipelineV3StageState::Failed);
    assert!(status.payload.stages[0].checkpoint.is_none());

    let report_paths = fs::read_dir(run_directory.join("providers"))
        .expect("provider evidence directory should remain readable")
        .map(|entry| {
            entry
                .expect("provider evidence entry should be readable")
                .path()
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("enhance-") && name.ends_with(".report.json"))
        })
        .collect::<Vec<_>>();
    assert_eq!(report_paths.len(), 1, "execution report was not retained");
    let report = ProviderExecutionReport::from_json_slice(
        &fs::read(&report_paths[0]).expect("execution report should remain readable"),
    )
    .expect("retained execution report should validate");
    assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Failed);
    assert_eq!(report.payload.termination.exit_code, Some(23));
    let failure = report
        .payload
        .failure
        .as_ref()
        .expect("failed execution report should retain failure evidence");
    assert_eq!(failure.code, ProviderExecutionFailureCode::ExitFailure);
    assert!(!failure.detail.is_empty());
    assert_eq!(
        fs::read(&fixture.source).expect("immutable source should remain readable"),
        immutable_source
    );
    assert_eq!(fixture.launch_count(), 1);
}

#[test]
fn changed_provider_executable_fails_exact_lock_preflight_without_workspace_mutation() {
    let fixture = ExecutionFixture::new("changed-provider-executable");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let mut executable =
        fs::read(&fixture.executable).expect("provider executable should be readable");
    executable.extend_from_slice(b"\n# implementation changed after planning\n");
    fs::write(&fixture.executable, executable).expect("provider executable should be mutable");
    let runs = fixture.runs_directory();

    let error = run_v3(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(&runs),
    )
    .expect_err("changed provider implementation must fail exact-lock preflight");

    assert_eq!(error.category(), ErrorCategory::State);
    assert_eq!(fixture.launch_count(), 0);
    assert!(
        !runs.exists(),
        "exact-lock preflight created a run workspace"
    );
}

#[test]
fn unsupported_validation_fails_before_workspace_creation_or_provider_launch() {
    let fixture = ExecutionFixture::new("unsupported-validation");
    let mut configuration = fixture.configuration();
    configuration.stages[0].validations[0].contract =
        "example.validation/unsupported-v1".to_owned();
    let (plan, registry) = fixture.resolve(configuration, ArtifactCardinality::One);
    let runs = fixture.runs_directory();

    let error = run_v3(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(&runs),
    )
    .expect_err("unknown validation authority must fail closed");

    assert_eq!(error.category(), ErrorCategory::Configuration);
    assert!(!runs.exists(), "preflight created a run parent");
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn missing_output_kind_fails_before_workspace_creation_or_provider_launch() {
    let fixture = ExecutionFixture::new("missing-output-kind");
    let mut configuration = fixture.configuration();
    configuration.stages[0].outputs[0].artifacts[0].kind = None;
    let (plan, registry) = fixture.resolve(configuration, ArtifactCardinality::One);
    let runs = fixture.runs_directory();

    let error = run_v3(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(&runs),
    )
    .expect_err("execution requires an explicit output filesystem kind");

    assert_eq!(error.category(), ErrorCategory::Configuration);
    assert!(!runs.exists(), "preflight created a run parent");
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn unsupported_output_cardinality_fails_before_workspace_creation_or_provider_launch() {
    let fixture = ExecutionFixture::new("unsupported-cardinality");
    let mut configuration = fixture.configuration();
    configuration.stages[0].outputs[0]
        .artifacts
        .push(ExpectedArtifact {
            id: "second-copy".to_owned(),
            relative_path: "artifacts/enhance/second-copy.bin".to_owned(),
            kind: Some(PipelineInputKind::File),
        });
    configuration.stages[0]
        .validations
        .push(ArtifactValidation {
            id: "second-copy-integrity".to_owned(),
            artifact: "second-copy".to_owned(),
            contract: ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1.to_owned(),
        });
    configuration.outputs.push(FinalOutputRequirement {
        id: "second-result".to_owned(),
        artifact: "second-copy".to_owned(),
        required_validations: vec!["second-copy-integrity".to_owned()],
    });
    let (plan, registry) = fixture.resolve(configuration, ArtifactCardinality::OneOrMore);
    let runs = fixture.runs_directory();

    let error = run_v3(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(&runs),
    )
    .expect_err("multi-artifact output ports are outside the executable v1 subset");

    assert_eq!(error.category(), ErrorCategory::Configuration);
    assert!(!runs.exists(), "preflight created a run parent");
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn unbound_optional_capability_output_fails_before_workspace_creation_or_provider_launch() {
    let fixture = ExecutionFixture::new("unbound-optional-output");
    let mut registry = ProviderRegistry::new();
    registry
        .register(provider_registration_with_optional_output(
            &fixture.executable,
            ArtifactCardinality::One,
        ))
        .expect("fixture registration should be unique");
    let plan = aniflow::resolve_pipeline_v3(
        &fixture.configuration(),
        &fixture.input_bindings(),
        &registry,
        &planning_context(),
    )
    .expect("an unbound optional capability output remains a valid planning artifact");
    let runs = fixture.runs_directory();

    let error = run_v3(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(&runs),
    )
    .expect_err("execution requires one bound artifact for every capability output");

    assert_eq!(error.category(), ErrorCategory::Configuration);
    assert!(!runs.exists(), "preflight created a run parent");
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn workspace_parent_inside_directory_input_fails_before_mutation() {
    let fixture = ExecutionFixture::new("workspace-inside-input");
    let source_directory = fixture.root.join("directory-source");
    fs::create_dir(&source_directory).expect("directory input should be created");
    fs::write(
        source_directory.join("source.bin"),
        b"immutable source bytes",
    )
    .expect("directory input content should be written");
    let bindings = vec![PipelineInputBinding::new("source", &source_directory)];
    let registry = fixture.registry(ArtifactCardinality::One);
    let plan = aniflow::resolve_pipeline_v3(
        &fixture.configuration(),
        &bindings,
        &registry,
        &planning_context(),
    )
    .expect("directory source should remain a valid planning input");
    let runs = source_directory.join("runs");

    let error =
        run_v3(PipelineV3RunRequest::new(plan, bindings, registry).with_output_directory(&runs))
            .expect_err("workspace creation beneath an immutable directory input must fail closed");

    assert_eq!(error.category(), ErrorCategory::Configuration);
    assert!(!runs.exists(), "preflight mutated the directory input");
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn cancellation_at_validating_persists_cancelled_without_checkpoint() {
    let fixture = ExecutionFixture::new("cancel-at-validating");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let cancellation = CancellationToken::default();
    let mut run_directory = None;
    let mut observed_validating = false;

    let error = run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(fixture.runs_directory()),
        &cancellation,
        |progress| match progress {
            PipelineV3RunProgress::Started {
                run_directory: started,
                ..
            } => run_directory = Some(started.clone()),
            PipelineV3RunProgress::Stage {
                stage_id,
                state: PipelineV3ProgressState::Validating,
            } if stage_id == "enhance" => {
                observed_validating = true;
                cancellation.cancel();
            }
            _ => {}
        },
    )
    .expect_err("cancellation during validation must stop the run");

    assert_eq!(error.category(), ErrorCategory::Execution);
    assert!(observed_validating, "provider never reached validation");
    assert_eq!(fixture.launch_count(), 1, "provider did not exit normally");
    let run_directory = run_directory.expect("started run should expose its workspace");
    let status = status_v3(&run_directory).expect("cancelled run should retain valid status");
    assert_eq!(status.payload.state, PipelineV3RunState::Cancelled);
    assert_eq!(
        status.payload.stages[0].state,
        PipelineV3StageState::Cancelled
    );
    assert!(status.payload.stages[0].checkpoint.is_none());
    assert!(
        fs::read_dir(run_directory.join("state/checkpoints"))
            .expect("checkpoint directory should remain readable")
            .next()
            .is_none(),
        "cancellation published a stage checkpoint"
    );
}

#[test]
fn provider_cancellation_report_is_published_without_accepting_a_checkpoint() {
    let fixture = ExecutionFixture::new("cancelled-provider-report");
    let (plan, registry) = fixture.plan(ArtifactCardinality::One);
    let cancellation = CancellationToken::default();
    let mut run_directory = None;

    let error = run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, fixture.input_bindings(), registry)
            .with_output_directory(fixture.runs_directory()),
        &cancellation,
        |progress| match progress {
            PipelineV3RunProgress::Started {
                run_directory: started,
                ..
            } => run_directory = Some(started.clone()),
            PipelineV3RunProgress::Stage {
                stage_id,
                state: PipelineV3ProgressState::Running,
            } if stage_id == "enhance" => cancellation.cancel(),
            _ => {}
        },
    )
    .expect_err("provider cancellation must stop the run");

    assert_eq!(error.category(), ErrorCategory::Execution);
    let run_directory = run_directory.expect("started run should expose its workspace");
    let status = status_v3(&run_directory).expect("cancelled run should retain valid status");
    assert_eq!(status.payload.state, PipelineV3RunState::Cancelled);
    assert_eq!(
        status.payload.stages[0].state,
        PipelineV3StageState::Cancelled
    );
    assert!(status.payload.stages[0].checkpoint.is_none());
    assert!(
        fs::read_dir(run_directory.join("state/checkpoints"))
            .expect("checkpoint directory should remain readable")
            .next()
            .is_none(),
        "provider cancellation published a stage checkpoint"
    );

    let report_paths = fs::read_dir(run_directory.join("providers"))
        .expect("provider evidence directory should remain readable")
        .map(|entry| {
            entry
                .expect("provider evidence entry should be readable")
                .path()
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("enhance-") && name.ends_with(".report.json"))
        })
        .collect::<Vec<_>>();
    assert_eq!(report_paths.len(), 1, "execution report was not retained");
    let report = ProviderExecutionReport::from_json_slice(
        &fs::read(&report_paths[0]).expect("execution report should remain readable"),
    )
    .expect("retained cancellation report should validate");
    assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Cancelled);
    assert_eq!(
        report.payload.failure.as_ref().map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::Cancelled)
    );
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn status_is_a_read_only_observation() {
    let fixture = ExecutionFixture::new("read-only-status");
    let outcome = fixture.run();
    let before = snapshot_tree(&outcome.run_directory);

    let status = status_v3(&outcome.run_directory).expect("status should load a valid run");

    assert_eq!(status.payload.state, PipelineV3RunState::Complete);
    assert_eq!(snapshot_tree(&outcome.run_directory), before);
    assert_eq!(fixture.launch_count(), 1);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SnapshotEntry {
    Directory,
    File(Vec<u8>),
}

fn snapshot_tree(root: &Path) -> BTreeMap<String, SnapshotEntry> {
    WalkDir::new(root)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .map(|entry| {
            let entry = entry.expect("run workspace should be traversable");
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("workspace entry should remain beneath its root")
                .to_string_lossy()
                .replace('\\', "/");
            let value = if entry.file_type().is_dir() {
                SnapshotEntry::Directory
            } else {
                SnapshotEntry::File(
                    fs::read(entry.path()).expect("workspace evidence should be readable"),
                )
            };
            (relative, value)
        })
        .collect()
}

fn stage_checkpoint(run_directory: &Path, stage_id: &str) -> (PathBuf, StageCheckpoint) {
    let manifest = status_v3(run_directory).expect("completed run should have valid status");
    let reference = manifest
        .payload
        .stages
        .iter()
        .find(|stage| stage.stage_id == stage_id)
        .and_then(|stage| stage.checkpoint.as_ref())
        .expect("completed stage should reference a checkpoint");
    let path = run_directory.join(&reference.relative_path);
    let checkpoint = StageCheckpoint::from_json_slice(
        &fs::read(&path).expect("stage checkpoint should be readable"),
    )
    .expect("stage checkpoint should validate");
    (path, checkpoint)
}

#[derive(Serialize)]
struct ExecutionReportBinding<'a> {
    stage_id: &'a str,
    stage_invocation_sha256: &'a str,
    report_sha256: &'a str,
}

fn bound_execution_report_path(
    stage_id: &str,
    stage_invocation_sha256: &str,
    report_sha256: &str,
) -> String {
    let binding_sha256 = canonical_sha256(&ExecutionReportBinding {
        stage_id,
        stage_invocation_sha256,
        report_sha256,
    });
    format!("providers/{stage_id}-{binding_sha256}.report.json")
}

fn canonical_sha256(value: &impl Serialize) -> String {
    let value = serde_json::to_value(value).expect("canonical fixture value should serialize");
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).expect("canonical fixture JSON should encode");
    format!("{:x}", Sha256::digest(bytes))
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        scalar => scalar,
    }
}

fn write_json(path: &Path, value: &impl Serialize) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("fixture JSON should encode");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("fixture JSON should be writable");
}

struct ExecutionFixture {
    _temporary: TempDir,
    root: PathBuf,
    source: PathBuf,
    executable: PathBuf,
    launch_count: PathBuf,
}

impl ExecutionFixture {
    fn new(label: &str) -> Self {
        let temporary = tempfile::Builder::new()
            .prefix(&format!("aniflow-v3-execution-{label}-"))
            .tempdir()
            .expect("temporary fixture root should be created");
        let root = temporary.path().to_path_buf();
        let source = root.join("source.bin");
        fs::write(&source, b"immutable source bytes").expect("source fixture should be written");
        let executable = write_provider_executable(&root);
        let launch_count = root.join("launch-count");
        Self {
            _temporary: temporary,
            root,
            source,
            executable,
            launch_count,
        }
    }

    fn configuration(&self) -> PipelineV3Configuration {
        PipelineV3Configuration::from_yaml_str(PIPELINE)
            .expect("Pipeline v3 execution fixture should parse")
    }

    fn input_bindings(&self) -> Vec<PipelineInputBinding> {
        vec![PipelineInputBinding::new("source", &self.source)]
    }

    fn registry(&self, cardinality: ArtifactCardinality) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        registry
            .register(provider_registration(&self.executable, cardinality))
            .expect("fixture registration should be unique");
        registry
    }

    fn resolve(
        &self,
        configuration: PipelineV3Configuration,
        cardinality: ArtifactCardinality,
    ) -> (PipelineV3Plan, ProviderRegistry) {
        let registry = self.registry(cardinality);
        let plan = aniflow::resolve_pipeline_v3(
            &configuration,
            &self.input_bindings(),
            &registry,
            &planning_context(),
        )
        .unwrap_or_else(|failure| panic!("execution fixture should plan: {failure:#?}"));
        (plan, registry)
    }

    fn plan(&self, cardinality: ArtifactCardinality) -> (PipelineV3Plan, ProviderRegistry) {
        self.resolve(self.configuration(), cardinality)
    }

    fn runs_directory(&self) -> PathBuf {
        self.root.join("runs")
    }

    fn run(&self) -> aniflow::PipelineV3RunOutcome {
        let (plan, registry) = self.plan(ArtifactCardinality::One);
        run_v3(
            PipelineV3RunRequest::new(plan, self.input_bindings(), registry)
                .with_output_directory(self.runs_directory()),
        )
        .expect("Pipeline v3 fixture run should succeed")
    }

    fn resume(&self, run_directory: &Path) -> aniflow::PipelineV3RunOutcome {
        resume_v3(PipelineV3ResumeRequest::new(
            run_directory,
            self.input_bindings(),
            self.registry(ArtifactCardinality::One),
        ))
        .expect("Pipeline v3 fixture resume should succeed")
    }

    fn launch_count(&self) -> u64 {
        read_launch_count(&self.launch_count)
    }
}

fn read_launch_count(path: &Path) -> u64 {
    match fs::read_to_string(path) {
        Ok(value) => value
            .parse()
            .expect("provider launch count should be an integer"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("provider launch count should be readable: {error}"),
    }
}

fn provider_registration(
    executable: &Path,
    output_cardinality: ArtifactCardinality,
) -> ProviderRegistration {
    provider_registration_with_outputs(executable, output_cardinality, false)
}

fn provider_registration_with_optional_output(
    executable: &Path,
    output_cardinality: ArtifactCardinality,
) -> ProviderRegistration {
    provider_registration_with_outputs(executable, output_cardinality, true)
}

fn provider_registration_with_outputs(
    executable: &Path,
    output_cardinality: ArtifactCardinality,
    add_optional_output: bool,
) -> ProviderRegistration {
    provider_registration_with_name(
        executable,
        "fixture-local",
        output_cardinality,
        add_optional_output,
    )
}

fn provider_registration_named(
    executable: &Path,
    registration_id: &str,
    output_cardinality: ArtifactCardinality,
) -> ProviderRegistration {
    provider_registration_with_name(executable, registration_id, output_cardinality, false)
}

fn provider_registration_with_name(
    executable: &Path,
    registration_id: &str,
    output_cardinality: ArtifactCardinality,
    add_optional_output: bool,
) -> ProviderRegistration {
    let mut manifest = ProviderManifest::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-manifest-v1.example.json"
    ))
    .expect("published provider manifest fixture should validate");
    manifest.capabilities[0].inputs[0].artifact_type = "application/octet-stream".to_owned();
    manifest.capabilities[0].outputs[0].artifact_type = "application/octet-stream".to_owned();
    manifest.capabilities[0].outputs[0].cardinality = output_cardinality;
    if add_optional_output {
        let mut optional = manifest.capabilities[0].outputs[0].clone();
        optional.name = "optional_metadata".to_owned();
        optional.cardinality = ArtifactCardinality::Optional;
        manifest.capabilities[0].outputs.push(optional);
    }

    let published_configuration = ProviderConfiguration::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-configuration-v1.example.json"
    ))
    .expect("published provider configuration fixture should validate");
    let mut values = BTreeMap::new();
    values.insert("scale".to_owned(), Value::from(2));
    values.insert("tile_size".to_owned(), Value::from(128));
    let configuration = ProviderConfiguration::new(
        ProviderReference {
            id: manifest.provider.id.clone(),
            version: manifest.provider.version.clone(),
        },
        CapabilityReference {
            id: manifest.capabilities[0].id.clone(),
            version: manifest.capabilities[0].version.clone(),
        },
        published_configuration.configuration_schema,
        values,
    )
    .expect("fixture provider configuration should validate");

    ProviderRegistration::new(
        registration_id,
        manifest,
        configuration,
        executable,
        format!("{registration_id}-process"),
        ComponentInventory {
            tools: vec![ComponentIdentity {
                id: "upscayl-bin".to_owned(),
                version: "2.15.0".to_owned(),
                sha256: None,
            }],
            codecs: vec![ComponentIdentity {
                id: "png".to_owned(),
                version: "1.6.43".to_owned(),
                sha256: None,
            }],
            models: vec![ComponentIdentity {
                id: "realesr-animevideov3".to_owned(),
                version: "1.0.0".to_owned(),
                sha256: Some(MODEL_DIGEST.to_owned()),
            }],
        },
    )
    .expect("fixture provider registration should validate")
}

fn planning_context() -> PipelinePlanningContext {
    PipelinePlanningContext {
        host: HostResources {
            cpu_threads: 8,
            memory_mib: 16 * 1024,
            storage_mib: 64 * 1024,
            gpu_available: true,
            network_available: false,
        },
        allowed_side_effects: vec![
            SideEffect::FilesystemRead,
            SideEffect::FilesystemWrite,
            SideEffect::Subprocess,
            SideEffect::Gpu,
        ],
        offline: true,
    }
}

fn write_provider_executable(root: &Path) -> PathBuf {
    let executable = root.join("fixture-provider.py");
    fs::write(
        &executable,
        r#"#!/usr/bin/env python3
import json
import os
import shutil
import sys

if len(sys.argv) != 3 or sys.argv[1] != "--aniflow-invocation":
    raise SystemExit(64)

counter_path = os.path.join(os.path.dirname(os.path.realpath(__file__)), "launch-count")
try:
    with open(counter_path, "r", encoding="utf-8") as counter_file:
        launch_count = int(counter_file.read())
except FileNotFoundError:
    launch_count = 0
with open(counter_path, "w", encoding="utf-8") as counter_file:
    counter_file.write(str(launch_count + 1))

if os.path.exists(os.path.join(os.path.dirname(os.path.realpath(__file__)), "nonzero-mode")):
    sys.stderr.write("fixture provider exited non-zero\n")
    raise SystemExit(23)

with open(sys.argv[2], "r", encoding="utf-8") as request_file:
    request = json.load(request_file)
source = request["inputs"][0]["path"]
destination = request["outputs"][0]["path"]
os.makedirs(os.path.dirname(destination), exist_ok=True)
hardlink_mode = os.path.exists(os.path.join(os.path.dirname(os.path.realpath(__file__)), "hardlink-mode"))
if os.path.isdir(source):
    shutil.copytree(source, destination)
elif hardlink_mode:
    os.link(source, destination)
else:
    shutil.copyfile(source, destination)
"#,
    )
    .expect("provider fixture should be written");
    let mut permissions = fs::metadata(&executable)
        .expect("provider fixture metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("provider fixture should be executable");
    fs::canonicalize(executable).expect("provider fixture path should resolve")
}
