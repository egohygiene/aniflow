use std::process::{Command, Output};

use aniflow::CommandName;
use serde_json::Value;

#[test]
fn run_v3_help_exposes_planning_workspace_and_provider_limits() {
    let output = run_cli(["run-v3", "--help"]);
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
        "--output-directory",
        "--provider-timeout-seconds",
        "--provider-termination-grace-milliseconds",
        "--maximum-stdout-bytes",
        "--maximum-stderr-bytes",
        "--maximum-artifact-files",
        "--maximum-artifact-bytes",
    ] {
        assert!(help.contains(flag), "missing run-v3 flag {flag}");
    }
    for default in [
        "[default: 21600]",
        "[default: 2000]",
        "[default: 67108864]",
        "[default: 1000000]",
        "[default: 1099511627776]",
    ] {
        assert!(help.contains(default), "missing provider limit {default}");
    }
}

#[test]
fn resume_v3_help_exposes_rebinding_and_provider_limits() {
    let output = run_cli(["resume-v3", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("help should be UTF-8");

    assert!(help.contains("<RUN_DIRECTORY>"));
    for flag in [
        "--input",
        "--provider-registration",
        "--provider-timeout-seconds",
        "--provider-termination-grace-milliseconds",
        "--maximum-stdout-bytes",
        "--maximum-stderr-bytes",
        "--maximum-artifact-files",
        "--maximum-artifact-bytes",
    ] {
        assert!(help.contains(flag), "missing resume-v3 flag {flag}");
    }
}

#[test]
fn status_v3_help_keeps_the_workspace_positional() {
    let output = run_cli(["status-v3", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("help should be UTF-8");

    assert!(help.contains("Usage: aniflow status-v3"));
    assert!(help.contains("<RUN_DIRECTORY>"));
}

#[test]
fn resume_v3_requires_explicit_input_and_provider_authority() {
    let output = run_cli(["resume-v3", "run-directory"]);

    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).expect("usage error should be UTF-8");
    assert!(error.contains("--input"));
    assert!(error.contains("--provider-registration"));
}

#[test]
fn pipeline_v3_execution_command_names_have_stable_machine_spellings() {
    for (command, expected) in [
        (CommandName::RunV3, "run_v3"),
        (CommandName::ResumeV3, "resume_v3"),
        (CommandName::StatusV3, "status_v3"),
    ] {
        assert_eq!(
            serde_json::to_value(command).expect("command name should serialize"),
            Value::String(expected.to_owned())
        );
        assert_eq!(command.to_string(), expected);
    }
}

fn run_cli<const N: usize>(arguments: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(arguments)
        .output()
        .expect("aniflow CLI should execute")
}
