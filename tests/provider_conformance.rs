use std::fs;
use std::path::{Path, PathBuf};

use aniflow::{
    PipelineV3Configuration, ProviderConfiguration, ProviderManifest, ProviderRegistrationDocument,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

const SCHEMA_SHA256: &str = "e1afbc8a40457b98255976814a36297752914c3787ebf3aac534f152bca3a317";

#[derive(Clone, Copy)]
struct Profile {
    slug: &'static str,
    registration: &'static str,
    pipeline: &'static str,
    input: &'static str,
    stage: &'static str,
    directory_output: bool,
    validation_evidence: bool,
}

const PROFILES: [Profile; 4] = [
    Profile {
        slug: "frame",
        registration: "reference-frame.registration.json",
        pipeline: "pipelines/frame.yml",
        input: "inputs/frame",
        stage: "process_frames",
        directory_output: true,
        validation_evidence: false,
    },
    Profile {
        slug: "audio",
        registration: "reference-audio.registration.json",
        pipeline: "pipelines/audio.yml",
        input: "inputs/audio.txt",
        stage: "process_audio",
        directory_output: false,
        validation_evidence: false,
    },
    Profile {
        slug: "whole-video",
        registration: "reference-whole-video.registration.json",
        pipeline: "pipelines/whole-video.yml",
        input: "inputs/whole-video.txt",
        stage: "process_video",
        directory_output: false,
        validation_evidence: false,
    },
    Profile {
        slug: "artifact-validator",
        registration: "reference-artifact-validator.registration.json",
        pipeline: "pipelines/artifact-validator.yml",
        input: "inputs/candidate.txt",
        stage: "validate_artifact",
        directory_output: false,
        validation_evidence: true,
    },
];

fn kit_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("conformance/provider-v1")
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn published_bundle_is_cross_contract_coherent() {
    let root = kit_root();
    let manifest = ProviderManifest::from_json_slice(&read(&root.join("manifest.json")))
        .expect("reference provider manifest should satisfy the public model");
    assert_eq!(manifest.capabilities.len(), PROFILES.len());

    let schema = read(&root.join("schemas/reference-configuration-v1.schema.json"));
    assert_eq!(format!("{:x}", Sha256::digest(&schema)), SCHEMA_SHA256);
    assert_eq!(manifest.configuration_schemas[0].sha256, SCHEMA_SHA256);
    let schema_document: Value =
        serde_json::from_slice(&schema).expect("reference configuration schema should be JSON");
    assert_eq!(schema_document["additionalProperties"], false);
    assert_eq!(schema_document["required"], serde_json::json!(["mode"]));
    let allowed_modes = schema_document["properties"]["mode"]["enum"]
        .as_array()
        .expect("reference configuration schema should enumerate modes");

    for profile in PROFILES {
        let registration_path = root.join(profile.registration);
        let registration = ProviderRegistrationDocument::from_json_slice(&read(&registration_path))
            .unwrap_or_else(|error| panic!("{} registration is invalid: {error}", profile.slug));
        assert_eq!(
            registration.registration_id,
            format!("reference-{}", profile.slug)
        );
        assert!(!registration.manifest.contains(".."));
        assert!(!registration.configuration.contains(".."));
        assert!(!registration.executable.contains(".."));

        let configuration =
            ProviderConfiguration::from_json_slice(&read(&root.join(&registration.configuration)))
                .unwrap_or_else(|error| {
                    panic!("{} configuration is invalid: {error}", profile.slug)
                });
        assert_eq!(configuration.provider.id, manifest.provider.id);
        assert_eq!(configuration.provider.version, manifest.provider.version);
        assert_eq!(configuration.configuration_schema.sha256, SCHEMA_SHA256);
        assert_eq!(configuration.values.len(), 1);
        assert!(allowed_modes.contains(&configuration.values["mode"]));

        let pipeline = PipelineV3Configuration::load(root.join(profile.pipeline))
            .unwrap_or_else(|error| panic!("{} pipeline is invalid: {error:#?}", profile.slug));
        assert_eq!(pipeline.stages.len(), 1);
        let stage = &pipeline.stages[0];
        assert_eq!(stage.id, profile.stage);
        assert_eq!(stage.capability.id, configuration.capability.id);

        let capability = manifest
            .capabilities
            .iter()
            .find(|candidate| candidate.id == configuration.capability.id)
            .expect("configured capability should be declared by the manifest");
        assert_eq!(stage.inputs.len(), 1);
        assert_eq!(stage.outputs.len(), 1);
        assert_eq!(stage.inputs[0].port, capability.inputs[0].name);
        assert_eq!(stage.outputs[0].port, capability.outputs[0].name);

        let authored_input = pipeline
            .inputs
            .iter()
            .find(|input| input.id == stage.inputs[0].artifacts[0])
            .expect("stage input should identify one authored pipeline input");
        assert_eq!(
            authored_input.artifact_type,
            capability.inputs[0].artifact_type
        );
        assert_eq!(
            authored_input.artifact_role,
            capability.inputs[0].artifact_role
        );
        assert_eq!(authored_input.stream_role, capability.inputs[0].stream_role);
        assert!(root.join(profile.input).exists());
    }

    for (mode, registration_name, pipeline_name) in [
        (
            "missing_output",
            "reference-missing-output.registration.json",
            "pipelines/missing-output.yml",
        ),
        (
            "nonzero",
            "reference-nonzero.registration.json",
            "pipelines/nonzero.yml",
        ),
    ] {
        let registration =
            ProviderRegistrationDocument::from_json_slice(&read(&root.join(registration_name)))
                .expect("published failure registration should validate");
        let configuration =
            ProviderConfiguration::from_json_slice(&read(&root.join(&registration.configuration)))
                .expect("published failure configuration should validate");
        assert_eq!(configuration.values["mode"].as_str(), Some(mode),);
        assert_eq!(configuration.values.len(), 1);
        assert!(allowed_modes.contains(&configuration.values["mode"]));
        let pipeline = PipelineV3Configuration::load(root.join(pipeline_name))
            .expect("published failure pipeline should validate");
        assert_eq!(
            pipeline.stages[0].provider.primary.registration_id,
            registration.registration_id
        );
    }
}

#[cfg(unix)]
mod live {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use aniflow::{
        AvailabilityCode, CancellationToken, ErrorCategory, HostResources, PipelineInputBinding,
        PipelinePlanningContext, PipelineV3Configuration, PipelineV3ResumeRequest,
        PipelineV3RunRequest, PipelineV3RunState, PipelineV3StageState, PipelineV3Workspace,
        ProviderExecutionFailureCode, ProviderExecutionOutcome, ProviderExecutionReport,
        ProviderRegistrationDocument, ProviderRegistry, SideEffect, load_stage_checkpoint, plan_v3,
        resume_v3, run_v3, run_v3_with_progress_and_cancellation, status_v3,
    };
    use serde_json::Value;
    use tempfile::TempDir;
    use walkdir::WalkDir;

    use super::{PROFILES, Profile, kit_root, read};

    fn planning_context() -> PipelinePlanningContext {
        PipelinePlanningContext {
            host: HostResources {
                cpu_threads: 1,
                memory_mib: 256,
                storage_mib: 256,
                gpu_available: false,
                network_available: false,
            },
            allowed_side_effects: vec![SideEffect::FilesystemRead, SideEffect::FilesystemWrite],
            offline: true,
        }
    }

    fn configuration(root: &Path, profile: Profile) -> PipelineV3Configuration {
        PipelineV3Configuration::load(root.join(profile.pipeline))
            .unwrap_or_else(|error| panic!("{} pipeline should load: {error:#?}", profile.slug))
    }

    fn bindings(root: &Path, profile: Profile) -> Vec<PipelineInputBinding> {
        let configuration = configuration(root, profile);
        vec![PipelineInputBinding::new(
            &configuration.inputs[0].id,
            root.join(profile.input),
        )]
    }

    fn registry(root: &Path, profile: Profile) -> ProviderRegistry {
        registry_from_document(root, profile.registration)
    }

    fn registry_from_document(root: &Path, registration: &str) -> ProviderRegistry {
        let document = ProviderRegistrationDocument::load(root.join(registration))
            .unwrap_or_else(|error| panic!("{registration} should load: {error}"));
        let mut registry = ProviderRegistry::new();
        registry
            .register(
                document
                    .into_registration()
                    .unwrap_or_else(|error| panic!("{registration} should materialize: {error}")),
            )
            .expect("reference registration should be unique");
        registry
    }

    fn resolved_plan(root: &Path, profile: Profile) -> aniflow::PipelineV3Plan {
        plan_v3(
            root.join(profile.pipeline),
            &bindings(root, profile),
            &[root.join(profile.registration)],
            &planning_context(),
        )
        .unwrap_or_else(|failure| panic!("{} profile should plan: {failure:#?}", profile.slug))
    }

    #[test]
    fn every_reference_profile_runs_checkpoints_and_resumes_without_reexecution() {
        let root = kit_root();
        for profile in PROFILES {
            let temporary = tempfile::Builder::new()
                .prefix(&format!("aniflow-provider-conformance-{}-", profile.slug))
                .tempdir()
                .expect("temporary run root should be created");
            let plan = resolved_plan(&root, profile);
            let repeated_plan = resolved_plan(&root, profile);
            assert_eq!(plan.plan_sha256, repeated_plan.plan_sha256);
            assert_eq!(
                plan.payload.stages[0].provider_lock.lock_sha256,
                repeated_plan.payload.stages[0].provider_lock.lock_sha256
            );
            assert_eq!(
                plan.payload.stages[0].provider_lock.payload.registration_id,
                format!("reference-{}", profile.slug)
            );
            let provider_lock_sha256 = plan.payload.stages[0].provider_lock.lock_sha256.clone();
            let input_bindings = bindings(&root, profile);

            let outcome = run_v3(
                PipelineV3RunRequest::new(plan, input_bindings.clone(), registry(&root, profile))
                    .with_output_directory(temporary.path().join("runs")),
            )
            .unwrap_or_else(|error| panic!("{} profile should run: {error}", profile.slug));

            assert_eq!(outcome.executed_stages, vec![profile.stage.to_owned()]);
            assert!(outcome.reused_stages.is_empty());
            assert_eq!(outcome.outputs.len(), 1);
            let output = &outcome.outputs[0].path;
            if profile.directory_output {
                assert!(output.is_dir());
                assert!(
                    WalkDir::new(output)
                        .min_depth(1)
                        .into_iter()
                        .any(|entry| { entry.is_ok_and(|entry| entry.file_type().is_file()) })
                );
            } else {
                assert!(output.is_file());
                assert!(
                    fs::metadata(output)
                        .expect("output metadata should exist")
                        .len()
                        > 0
                );
            }
            if profile.validation_evidence {
                let evidence: Value = serde_json::from_slice(&read(output))
                    .expect("validator output should be JSON evidence");
                assert_eq!(evidence["accepted"], true);
                assert_eq!(
                    evidence["schema"],
                    "aniflow.reference-validation-evidence/v1"
                );
            }

            let status = status_v3(&outcome.run_directory)
                .expect("completed conformance run should have valid status");
            assert_eq!(status.payload.state, PipelineV3RunState::Complete);
            assert_eq!(
                status.payload.stages[0].state,
                PipelineV3StageState::Complete
            );
            let checkpoint_reference = status.payload.stages[0]
                .checkpoint
                .as_ref()
                .expect("completed stage should retain a checkpoint");
            let workspace = PipelineV3Workspace::open_read_only(&outcome.run_directory)
                .expect("run workspace should open read-only");
            let checkpoint = load_stage_checkpoint(&workspace, checkpoint_reference)
                .expect("published checkpoint should validate");
            assert_eq!(
                checkpoint.payload.provider_lock_sha256,
                provider_lock_sha256
            );
            assert_eq!(checkpoint.payload.outputs.len(), 1);
            assert_eq!(checkpoint.payload.validations.len(), 1);
            assert!(checkpoint.payload.compatibility_fingerprint.is_none());

            let report_path = outcome
                .run_directory
                .join(&checkpoint.payload.execution_report.relative_path);
            let report = ProviderExecutionReport::from_json_slice(&read(&report_path))
                .expect("published execution report should validate");
            assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Succeeded);
            assert_eq!(
                report.payload.provider_lock.lock_sha256,
                provider_lock_sha256
            );
            assert_eq!(
                report.report_sha256,
                checkpoint.payload.execution_report.sha256
            );

            let output_before_resume = artifact_snapshot(output);
            let repeated_temporary = tempfile::Builder::new()
                .prefix(&format!(
                    "aniflow-provider-conformance-repeat-{}-",
                    profile.slug
                ))
                .tempdir()
                .expect("repeated run root should be created");
            let repeated_outcome = run_v3(
                PipelineV3RunRequest::new(
                    repeated_plan,
                    input_bindings.clone(),
                    registry(&root, profile),
                )
                .with_output_directory(repeated_temporary.path().join("runs")),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{} profile should repeat deterministically: {error}",
                    profile.slug
                )
            });
            assert_eq!(
                artifact_snapshot(&repeated_outcome.outputs[0].path),
                output_before_resume,
                "{} profile should produce byte-identical fresh outputs",
                profile.slug
            );

            let resumed = resume_v3(PipelineV3ResumeRequest::new(
                &outcome.run_directory,
                input_bindings,
                registry(&root, profile),
            ))
            .unwrap_or_else(|error| panic!("{} profile should resume: {error}", profile.slug));
            assert!(resumed.executed_stages.is_empty());
            assert_eq!(resumed.reused_stages, vec![profile.stage.to_owned()]);
            assert_eq!(
                artifact_snapshot(&resumed.outputs[0].path),
                output_before_resume
            );
        }
    }

    #[test]
    fn denied_write_effect_fails_during_resolution_before_any_run_exists() {
        let root = kit_root();
        let profile = PROFILES[0];
        let mut context = planning_context();
        context.allowed_side_effects = vec![SideEffect::FilesystemRead];
        let failure = plan_v3(
            root.join(profile.pipeline),
            &bindings(&root, profile),
            &[root.join(profile.registration)],
            &context,
        )
        .expect_err("a missing filesystem-write grant must fail closed");
        assert!(
            failure
                .diagnostics
                .iter()
                .flat_map(|diagnostic| {
                    diagnostic
                        .attempts
                        .iter()
                        .flat_map(|attempt| attempt.reasons.iter())
                })
                .any(|reason| reason.code == AvailabilityCode::SideEffectDenied)
        );
    }

    #[test]
    fn reference_provider_rejects_the_wrong_argv_and_malformed_requests() {
        let executable = kit_root().join("reference-provider.py");
        let wrong_argv = Command::new(&executable)
            .status()
            .expect("reference provider should launch");
        assert_eq!(wrong_argv.code(), Some(64));

        let temporary = TempDir::new().expect("temporary malformed fixture should be created");
        let request = temporary.path().join("malformed.json");
        fs::write(&request, br#"{"unexpected":true}"#)
            .expect("malformed request should be written");
        let malformed = Command::new(&executable)
            .arg("--aniflow-invocation")
            .arg(&request)
            .status()
            .expect("reference provider should inspect malformed input");
        assert_eq!(malformed.code(), Some(64));

        let control_characters = Command::new("python3")
            .arg("-c")
            .arg(
                r#"import runpy, sys
module = runpy.run_path(sys.argv[1])
for value in ('\x7f', '\x85'):
    try:
        module['require_string'](value, 'test value')
    except module['ContractError']:
        pass
    else:
        raise SystemExit(1)
"#,
            )
            .arg(&executable)
            .status()
            .expect("reference provider helpers should load under Python 3");
        assert!(
            control_characters.success(),
            "DEL and C1 controls must not cross the provider ABI"
        );
    }

    #[test]
    fn missing_output_and_nonzero_exit_never_publish_a_checkpoint() {
        let root = kit_root();
        for (label, pipeline, registration, expected_code) in [
            (
                "missing-output",
                "pipelines/missing-output.yml",
                "reference-missing-output.registration.json",
                ProviderExecutionFailureCode::MissingOutput,
            ),
            (
                "nonzero",
                "pipelines/nonzero.yml",
                "reference-nonzero.registration.json",
                ProviderExecutionFailureCode::ExitFailure,
            ),
        ] {
            let temporary = tempfile::Builder::new()
                .prefix(&format!("aniflow-provider-conformance-{label}-"))
                .tempdir()
                .expect("temporary failure fixture should be created");
            let configuration = PipelineV3Configuration::load(root.join(pipeline))
                .expect("failure pipeline should load");
            let input_bindings = vec![PipelineInputBinding::new(
                &configuration.inputs[0].id,
                root.join("inputs/frame"),
            )];
            let plan = plan_v3(
                root.join(pipeline),
                &input_bindings,
                &[root.join(registration)],
                &planning_context(),
            )
            .expect("failure fixture should still resolve a plan");
            let runs = temporary.path().join("runs");
            let mut started_run = None;
            let error = run_v3_with_progress_and_cancellation(
                PipelineV3RunRequest::new(
                    plan,
                    input_bindings,
                    registry_from_document(&root, registration),
                )
                .with_output_directory(&runs),
                &CancellationToken::default(),
                |progress| {
                    if let aniflow::PipelineV3RunProgress::Started { run_directory, .. } = progress
                    {
                        started_run = Some(run_directory.clone());
                    }
                },
            )
            .expect_err("configured provider failure must fail the run");
            assert_eq!(error.category(), ErrorCategory::Execution);

            let run_directory = started_run.expect("failed execution should expose its workspace");
            let status = status_v3(&run_directory)
                .expect("failed conformance run should remain inspectable");
            assert_eq!(status.payload.state, PipelineV3RunState::Failed);
            assert_eq!(status.payload.stages[0].state, PipelineV3StageState::Failed);
            assert!(status.payload.stages[0].checkpoint.is_none());

            let reports = fs::read_dir(run_directory.join("providers"))
                .expect("provider evidence directory should be readable")
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.to_string_lossy().ends_with(".report.json"))
                .collect::<Vec<_>>();
            assert_eq!(reports.len(), 1);
            let report = ProviderExecutionReport::from_json_slice(&read(&reports[0]))
                .expect("failure report should validate");
            assert_eq!(
                report.payload.failure.as_ref().map(|failure| failure.code),
                Some(expected_code)
            );
            assert_ne!(report.payload.outcome, ProviderExecutionOutcome::Succeeded);
        }
    }

    fn artifact_snapshot(path: &Path) -> Vec<(String, Vec<u8>)> {
        if path.is_file() {
            return vec![(".".to_owned(), read(path))];
        }
        WalkDir::new(path)
            .min_depth(1)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                let relative = entry
                    .path()
                    .strip_prefix(path)
                    .expect("artifact entry should remain beneath its root")
                    .to_string_lossy()
                    .replace('\\', "/");
                (relative, read(entry.path()))
            })
            .collect()
    }
}
