use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use aniflow::{
    CommandName, DoctorReport, ErrorCategory, HostResources, MachineEnvelope, MachineOutcome,
    MediaInspection, PipelineInputBinding, PipelinePlan, PipelinePlanningContext,
    PipelinePlanningDiagnosticCode, PipelinePlanningFailure, PipelineV3Plan, RunOutcome, RunStatus,
};
#[cfg(unix)]
use aniflow::{
    PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1, PipelineV3RunRecovery, PipelineV3RunState,
    PipelineV3Workspace,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn every_command_exposes_a_library_equivalent_machine_result() {
    let fixture = ContractFixture::new();

    let expected_doctor = aniflow::doctor(None).expect("library doctor should succeed");
    let doctor: DoctorReport =
        success_result(run_cli(["doctor", "--output", "json"]), CommandName::Doctor);
    assert_eq!(doctor, expected_doctor);

    let expected_inspection =
        aniflow::inspect(&fixture.source).expect("library inspection should succeed");
    let inspect: MediaInspection = success_result(
        run_cli_os([
            OsString::from("inspect"),
            fixture.source.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ]),
        CommandName::Inspect,
    );
    assert_eq!(inspect, expected_inspection);

    let expected_plan =
        aniflow::plan(&fixture.source, &fixture.pipeline).expect("library planning should succeed");
    let plan: PipelinePlan = success_result(
        run_cli_os([
            OsString::from("plan"),
            OsString::from("--input"),
            fixture.source.clone().into_os_string(),
            OsString::from("--pipeline"),
            fixture.pipeline.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ]),
        CommandName::Plan,
    );
    assert_eq!(plan, expected_plan);

    let expected_v3 = fixture
        .plan_v3(std::slice::from_ref(&fixture.v3_registration))
        .expect("library Pipeline v3 planning should succeed");
    let plan_v3: PipelineV3Plan = success_result(
        fixture.run_plan_v3_cli(std::slice::from_ref(&fixture.v3_registration)),
        CommandName::PlanV3,
    );
    assert_eq!(plan_v3, expected_v3);

    let run: RunOutcome = success_result(
        run_cli_os([
            OsString::from("run"),
            OsString::from("--input"),
            fixture.source.clone().into_os_string(),
            OsString::from("--pipeline"),
            fixture.pipeline.clone().into_os_string(),
            OsString::from("--output-directory"),
            fixture.runs.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ]),
        CommandName::Run,
    );
    assert!(run.output.is_file());
    assert!(run.delivery_manifest.is_file());
    assert!(run.run_manifest.is_file());

    let expected_status =
        aniflow::status(&run.run_directory).expect("library status should succeed");
    let status: RunStatus = success_result(
        run_cli_os([
            OsString::from("status"),
            run.run_directory.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ]),
        CommandName::Status,
    );
    assert_eq!(status, expected_status);

    let resumed: RunOutcome = success_result(
        run_cli_os([
            OsString::from("resume"),
            run.run_directory.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ]),
        CommandName::Resume,
    );
    assert_eq!(resumed, run);
}

#[test]
fn machine_failures_use_typed_errors_and_stable_exit_codes() {
    let output = run_cli(["inspect", "missing.mp4", "--output", "json"]);

    assert_eq!(
        output.status.code(),
        Some(ErrorCategory::Input.exit_code().into())
    );
    assert!(output.stdout.is_empty());
    let envelope = MachineEnvelope::<Value>::from_json_slice(&output.stderr)
        .expect("machine error should be a supported envelope");
    assert_eq!(envelope.command, CommandName::Inspect);
    match envelope.outcome {
        MachineOutcome::Error { error, result } => {
            assert_eq!(error.category, ErrorCategory::Input);
            assert!(error.message.contains("input video does not exist"));
            assert!(result.is_none());
        }
        _ => panic!("missing input should produce an error envelope"),
    }
}

#[test]
fn plan_v3_machine_failures_preserve_library_diagnostics() {
    let fixture = ContractFixture::new();
    let expected = fixture
        .plan_v3(&[])
        .expect_err("an unregistered provider must fail planning");
    assert_plan_v3_failure_parity(fixture.run_plan_v3_cli(&[]), &expected);
}

#[test]
fn plan_v3_registration_failures_have_library_cli_parity_and_exact_indexes() {
    let fixture = ContractFixture::new();
    let malformed = fixture
        .v3_registration
        .parent()
        .expect("registration fixture should have a parent")
        .join("malformed-registration.json");
    fs::write(&malformed, b"{").expect("malformed registration fixture should be written");

    let malformed_paths = [malformed];
    let expected_malformed = fixture
        .plan_v3(&malformed_paths)
        .expect_err("malformed registration JSON must fail planning");
    assert_eq!(
        expected_malformed.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidProviderRegistration
    );
    assert_eq!(
        expected_malformed.diagnostics()[0].field.as_deref(),
        Some("provider_registration_paths[0]")
    );
    assert_plan_v3_failure_parity(
        fixture.run_plan_v3_cli(&malformed_paths),
        &expected_malformed,
    );

    let duplicate_paths = [
        fixture.v3_registration.clone(),
        fixture.v3_registration.clone(),
    ];
    let expected_duplicate = fixture
        .plan_v3(&duplicate_paths)
        .expect_err("duplicate registration IDs must fail planning");
    assert_eq!(
        expected_duplicate.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidProviderRegistration
    );
    assert_eq!(
        expected_duplicate.diagnostics()[0].field.as_deref(),
        Some("provider_registration_paths[1]")
    );
    assert_plan_v3_failure_parity(
        fixture.run_plan_v3_cli(&duplicate_paths),
        &expected_duplicate,
    );
}

#[cfg(unix)]
#[test]
fn pipeline_v3_machine_failures_preserve_only_durable_recovery_locators() {
    let fixture = ContractFixture::new();

    let preflight = fixture.run_v3_cli(&fixture.v3_pipeline);
    assert_eq!(
        preflight.status.code(),
        Some(ErrorCategory::Configuration.exit_code().into())
    );
    assert!(preflight.stdout.is_empty());
    let envelope = MachineEnvelope::<Value>::from_json_slice(&preflight.stderr)
        .expect("preflight failure should be a supported machine envelope");
    assert_eq!(envelope.command, CommandName::RunV3);
    match envelope.outcome {
        MachineOutcome::Error { result, .. } => assert!(result.is_none()),
        _ => panic!("preflight should fail before a recovery locator exists"),
    }
    assert!(
        !fixture.runs.exists(),
        "preflight failure must not create a run parent"
    );

    let execution_pipeline = fixture
        .v3_pipeline
        .parent()
        .expect("Pipeline v3 fixture should have a parent")
        .join("execution-pipeline.yml");
    fs::write(
        &execution_pipeline,
        PIPELINE_V3_YAML.replace(
            "aniflow.validation/fixture/v1",
            "aniflow.validation/artifact-integrity/v1",
        ),
    )
    .expect("executable Pipeline v3 fixture should be written");

    let failed_run = fixture.run_v3_cli(&execution_pipeline);
    let run_recovery = pipeline_v3_recovery_result(failed_run, CommandName::RunV3);
    assert!(run_recovery.run_directory.is_dir());
    let run_manifest = run_recovery
        .run_manifest
        .as_ref()
        .expect("a validated failed-run manifest should be recoverable");
    assert!(run_manifest.is_file());
    let status = aniflow::status_v3(&run_recovery.run_directory)
        .expect("the failed run locator should resolve through status_v3");
    assert_eq!(status.payload.state, PipelineV3RunState::Failed);
    let workspace = PipelineV3Workspace::open_read_only(&run_recovery.run_directory)
        .expect("the failed run locator should identify a valid workspace");
    assert_eq!(
        run_manifest,
        &workspace.manifest_revision(status.payload.revision)
    );

    let failed_resume = fixture.resume_v3_cli(&run_recovery.run_directory);
    let resume_recovery = pipeline_v3_recovery_result(failed_resume, CommandName::ResumeV3);
    assert_eq!(resume_recovery.run_directory, run_recovery.run_directory);
    let resume_manifest = resume_recovery
        .run_manifest
        .as_ref()
        .expect("a validated failed-resume manifest should be recoverable");
    assert!(resume_manifest.is_file());
    assert_ne!(resume_manifest, run_manifest);
    let resumed_status = aniflow::status_v3(&resume_recovery.run_directory)
        .expect("the failed resume locator should resolve through status_v3");
    assert_eq!(resumed_status.payload.state, PipelineV3RunState::Failed);
    assert_eq!(
        resume_manifest,
        &workspace.manifest_revision(resumed_status.payload.revision)
    );
}

#[test]
fn clap_usage_failures_keep_exit_code_two() {
    let output = run_cli(["plan"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}

#[test]
fn plan_v3_help_exposes_explicit_policy_and_registration_inputs() {
    let output = run_cli(["plan-v3", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("help should be UTF-8");
    for flag in [
        "--pipeline",
        "--input",
        "--provider-registration",
        "--host-cpu-threads",
        "--host-memory-mib",
        "--host-storage-mib",
        "--host-gpu-available",
        "--host-network-available",
        "--allow-side-effect",
        "--offline",
    ] {
        assert!(help.contains(flag), "missing plan-v3 flag {flag}");
    }
    for side_effect in [
        "filesystem-read",
        "filesystem-write",
        "environment-read",
        "subprocess",
        "network",
        "ai",
        "gpu",
        "publish",
    ] {
        assert!(
            help.contains(side_effect),
            "missing side-effect value {side_effect}"
        );
    }
}

#[test]
fn human_errors_escape_terminal_control_characters() {
    let fixture = ContractFixture::new();
    let unsafe_pipeline = fixture
        .v3_pipeline
        .parent()
        .expect("Pipeline v3 fixture should have a parent")
        .join("unsafe-control.yml");
    fs::write(
        &unsafe_pipeline,
        format!("{PIPELINE_V3_YAML}\n\"\\u001b[31mEVIL\": true\n"),
    )
    .expect("unsafe Pipeline v3 fixture should be written");

    let mut input_binding = OsString::from("source=");
    input_binding.push(&fixture.v3_input);
    let output = run_cli_os([
        OsString::from("plan-v3"),
        OsString::from("--pipeline"),
        unsafe_pipeline.into_os_string(),
        OsString::from("--input"),
        input_binding,
        OsString::from("--provider-registration"),
        fixture.v3_registration.clone().into_os_string(),
        OsString::from("--host-cpu-threads"),
        OsString::from("1"),
        OsString::from("--host-memory-mib"),
        OsString::from("0"),
        OsString::from("--host-storage-mib"),
        OsString::from("0"),
        OsString::from("--offline"),
    ]);

    assert_eq!(
        output.status.code(),
        Some(ErrorCategory::Configuration.exit_code().into())
    );
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.contains(&0x1b));
    assert!(String::from_utf8_lossy(&output.stderr).contains("\\u{1b}"));
}

#[test]
fn v02_inspect_json_and_output_directory_alias_remain_accepted() {
    let fixture = ContractFixture::new();
    let expected = aniflow::inspect(&fixture.source).expect("library inspection should succeed");
    let output = run_cli_os([
        OsString::from("inspect"),
        fixture.source.into_os_string(),
        OsString::from("--json"),
    ]);

    assert!(output.status.success());
    let legacy: MediaInspection =
        serde_json::from_slice(&output.stdout).expect("legacy JSON should remain raw inspection");
    assert_eq!(legacy, expected);

    let help = run_cli(["run", "--help"]);
    let help = String::from_utf8(help.stdout).expect("help should be UTF-8");
    assert!(help.contains("--output-directory"));
    assert!(help.contains("--output-dir"));
}

fn success_result<T>(output: Output, command: CommandName) -> T
where
    T: DeserializeOwned,
{
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let envelope = MachineEnvelope::<T>::from_json_slice(&output.stdout)
        .expect("command should emit a supported envelope");
    assert_eq!(envelope.command, command);
    match envelope.outcome {
        MachineOutcome::Success { result } => result,
        _ => panic!("successful command should emit a success envelope"),
    }
}

fn assert_plan_v3_failure_parity(output: Output, expected: &PipelinePlanningFailure) {
    assert_eq!(
        output.status.code(),
        Some(expected.category().exit_code().into())
    );
    assert!(output.stdout.is_empty());
    let envelope = MachineEnvelope::<PipelinePlanningFailure>::from_json_slice(&output.stderr)
        .expect("planning error should be a supported machine envelope");
    assert_eq!(envelope.command, CommandName::PlanV3);
    match envelope.outcome {
        MachineOutcome::Error { error, result } => {
            assert_eq!(error.category, expected.category());
            assert_eq!(result.as_ref(), Some(expected));
        }
        _ => panic!("failed Pipeline v3 planning should emit an error envelope"),
    }
}

#[cfg(unix)]
fn pipeline_v3_recovery_result(output: Output, command: CommandName) -> PipelineV3RunRecovery {
    assert_eq!(
        output.status.code(),
        Some(ErrorCategory::Execution.exit_code().into())
    );
    assert!(output.stdout.is_empty());
    let envelope = MachineEnvelope::<PipelineV3RunRecovery>::from_json_slice(&output.stderr)
        .expect("runtime failure should be a supported machine envelope");
    assert_eq!(envelope.command, command);
    match envelope.outcome {
        MachineOutcome::Error { error, result } => {
            assert_eq!(error.category, ErrorCategory::Execution);
            let recovery = result.expect("runtime failure should retain a recovery locator");
            assert_eq!(recovery.schema, PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1);
            recovery
        }
        _ => panic!("runtime failure should emit an error envelope"),
    }
}

fn run_cli<const N: usize>(arguments: [&str; N]) -> Output {
    run_cli_os(arguments.map(OsString::from))
}

fn run_cli_os(arguments: impl IntoIterator<Item = OsString>) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(arguments)
        .output()
        .expect("aniflow CLI should execute")
}

struct ContractFixture {
    _temporary: TempDir,
    source: PathBuf,
    pipeline: PathBuf,
    runs: PathBuf,
    v3_input: PathBuf,
    v3_pipeline: PathBuf,
    v3_registration: PathBuf,
}

impl ContractFixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let source = temporary.path().join("source.mp4");
        generate_source(&source);
        let (v3_input, v3_pipeline, v3_registration) = write_pipeline_v3_fixture(temporary.path());
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        Self {
            runs: temporary.path().join("runs"),
            pipeline: repository.join("pipelines/passthrough.yml"),
            source,
            v3_input,
            v3_pipeline,
            v3_registration,
            _temporary: temporary,
        }
    }

    fn plan_v3(
        &self,
        provider_registration_paths: &[PathBuf],
    ) -> std::result::Result<PipelineV3Plan, PipelinePlanningFailure> {
        aniflow::plan_v3(
            &self.v3_pipeline,
            &[PipelineInputBinding::new("source", &self.v3_input)],
            provider_registration_paths,
            &planning_context(),
        )
    }

    fn run_plan_v3_cli(&self, provider_registration_paths: &[PathBuf]) -> Output {
        let mut input_binding = OsString::from("source=");
        input_binding.push(&self.v3_input);
        let mut arguments = vec![
            OsString::from("plan-v3"),
            OsString::from("--pipeline"),
            self.v3_pipeline.clone().into_os_string(),
            OsString::from("--input"),
            input_binding,
        ];
        for path in provider_registration_paths {
            arguments.push(OsString::from("--provider-registration"));
            arguments.push(path.clone().into_os_string());
        }
        arguments.extend([
            OsString::from("--host-cpu-threads"),
            OsString::from("1"),
            OsString::from("--host-memory-mib"),
            OsString::from("0"),
            OsString::from("--host-storage-mib"),
            OsString::from("0"),
            OsString::from("--offline"),
            OsString::from("--output"),
            OsString::from("json"),
        ]);
        run_cli_os(arguments)
    }

    #[cfg(unix)]
    fn run_v3_cli(&self, pipeline: &Path) -> Output {
        let mut input_binding = OsString::from("source=");
        input_binding.push(&self.v3_input);
        run_cli_os([
            OsString::from("run-v3"),
            OsString::from("--pipeline"),
            pipeline.to_path_buf().into_os_string(),
            OsString::from("--input"),
            input_binding,
            OsString::from("--provider-registration"),
            self.v3_registration.clone().into_os_string(),
            OsString::from("--host-cpu-threads"),
            OsString::from("1"),
            OsString::from("--host-memory-mib"),
            OsString::from("0"),
            OsString::from("--host-storage-mib"),
            OsString::from("0"),
            OsString::from("--offline"),
            OsString::from("--output-directory"),
            self.runs.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ])
    }

    #[cfg(unix)]
    fn resume_v3_cli(&self, run_directory: &Path) -> Output {
        let mut input_binding = OsString::from("source=");
        input_binding.push(&self.v3_input);
        run_cli_os([
            OsString::from("resume-v3"),
            run_directory.to_path_buf().into_os_string(),
            OsString::from("--input"),
            input_binding,
            OsString::from("--provider-registration"),
            self.v3_registration.clone().into_os_string(),
            OsString::from("--output"),
            OsString::from("json"),
        ])
    }
}

fn planning_context() -> PipelinePlanningContext {
    PipelinePlanningContext {
        host: HostResources {
            cpu_threads: 1,
            memory_mib: 0,
            storage_mib: 0,
            gpu_available: false,
            network_available: false,
        },
        allowed_side_effects: Vec::new(),
        offline: true,
    }
}

fn write_pipeline_v3_fixture(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let directory = root.join("pipeline v3");
    fs::create_dir(&directory).expect("Pipeline v3 fixture directory should be created");
    let input = directory.join("source input.bin");
    let pipeline = directory.join("pipeline.yml");
    let registration = directory.join("registration.json");
    let manifest = directory.join("manifest.json");
    let configuration = directory.join("configuration.json");
    let executable = directory.join("provider-bin");

    fs::write(&input, b"v3 fixture input").expect("Pipeline v3 input should be written");
    fs::write(&pipeline, PIPELINE_V3_YAML).expect("Pipeline v3 YAML should be written");
    fs::write(&registration, PROVIDER_REGISTRATION_JSON)
        .expect("provider registration document should be written");
    fs::write(&manifest, PROVIDER_MANIFEST_JSON).expect("provider manifest should be written");
    fs::write(&configuration, PROVIDER_CONFIGURATION_JSON)
        .expect("provider configuration should be written");
    fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("provider executable should be written");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&executable)
            .expect("provider executable metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions)
            .expect("provider executable should be runnable");
    }

    (input, pipeline, registration)
}

const PIPELINE_V3_YAML: &str = r#"schema: aniflow.pipeline/v3
name: cli-contract
inputs:
  - id: source
    artifact_type: application/octet-stream
    artifact_role: temporal_source
stages:
  - id: transform
    capability:
      id: aniflow/fixture.process
      version_requirement: ^1.0
    provider:
      primary:
        registration_id: fixture
    inputs:
      - port: source
        artifacts: [source]
    outputs:
      - port: result
        artifacts:
          - id: result
            relative_path: artifacts/result.bin
            kind: file
    validations:
      - id: result-valid
        artifact: result
        contract: aniflow.validation/fixture/v1
outputs:
  - id: final
    artifact: result
    required_validations: [result-valid]
"#;

const PROVIDER_REGISTRATION_JSON: &str = r#"{
  "schema": "aniflow.provider-registration/v1",
  "registration_id": "fixture",
  "manifest": "manifest.json",
  "configuration": "configuration.json",
  "executable": "provider-bin",
  "implementation_id": "fixture-implementation"
}"#;

const PROVIDER_CONFIGURATION_JSON: &str = r#"{
  "schema": "aniflow.provider-configuration/v1",
  "provider": {"id": "org.aniflow.fixture", "version": "1.0.0"},
  "capability": {"id": "aniflow/fixture.process", "version": "1.0.0"},
  "configuration_schema": {
    "id": "aniflow.fixture.configuration/v1",
    "version": "1.0.0",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "values": {},
  "effective_configuration_sha256": "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
}"#;

const PROVIDER_MANIFEST_JSON: &str = r#"{
  "schema": "aniflow.provider-manifest/v1",
  "provider": {
    "id": "org.aniflow.fixture",
    "version": "1.0.0",
    "display_name": "CLI fixture"
  },
  "configuration_schemas": [{
    "id": "aniflow.fixture.configuration/v1",
    "version": "1.0.0",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  }],
  "capabilities": [{
    "id": "aniflow/fixture.process",
    "version": "1.0.0",
    "kind": "whole_video_processor",
    "configuration_schema": {
      "id": "aniflow.fixture.configuration/v1",
      "version": "1.0.0",
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    },
    "inputs": [{
      "name": "source",
      "artifact_type": "application/octet-stream",
      "artifact_role": "temporal_source",
      "cardinality": "one",
      "immutable": true
    }],
    "outputs": [{
      "name": "result",
      "artifact_type": "application/octet-stream",
      "artifact_role": "candidate_master",
      "cardinality": "one",
      "immutable": true
    }],
    "batching": "whole_artifact",
    "max_concurrency": 1,
    "requirements": {
      "tools": [],
      "codecs": [],
      "models": [],
      "compute": {
        "minimum_cpu_threads": 1,
        "minimum_memory_mib": 0,
        "minimum_storage_mib": 0,
        "gpu": "forbidden",
        "network": "forbidden"
      }
    },
    "behavior": {
      "determinism": "deterministic",
      "fidelity": "lossless",
      "cacheable": true,
      "content_changes": true,
      "side_effects": []
    },
    "lifecycle": {"progress": "none", "cancellation": "unsupported"}
  }],
  "provenance": {
    "required": [
      "input_digests",
      "output_digests",
      "pipeline_configuration",
      "effective_configuration",
      "provider_identity",
      "capability_identity",
      "tool_versions",
      "codec_versions",
      "model_identities",
      "validation_evidence"
    ]
  }
}"#;

fn generate_source(destination: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x64:rate=2:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(destination)
        .status()
        .expect("ffmpeg should execute");
    assert!(
        status.success(),
        "ffmpeg should create the contract fixture"
    );
}
