use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use aniflow::{
    CommandName, DoctorReport, ErrorCategory, MachineEnvelope, MachineOutcome, MediaInspection,
    PipelinePlan, RunOutcome, RunStatus,
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
fn clap_usage_failures_keep_exit_code_two() {
    let output = run_cli(["plan"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
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
}

impl ContractFixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let source = temporary.path().join("source.mp4");
        generate_source(&source);
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        Self {
            runs: temporary.path().join("runs"),
            pipeline: repository.join("pipelines/passthrough.yml"),
            source,
            _temporary: temporary,
        }
    }
}

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
