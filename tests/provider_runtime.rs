#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use aniflow::{
    ArtifactKind, AvailabilityCode, CancellationToken, ComponentIdentity, ComponentInventory,
    ExpectedProviderOutput, HostResources, ProviderCandidate, ProviderConfiguration,
    ProviderExecutionFailureCode, ProviderExecutionLimits, ProviderExecutionOutcome,
    ProviderExecutionReport, ProviderExecutionRequest, ProviderManifest, ProviderRegistration,
    ProviderRegistry, ProviderResolutionRequest, ProviderSelectionSource, SideEffect,
    TerminationReason,
};
use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use tempfile::TempDir;

const MODEL_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn write_provider_script(directory: &Path) -> PathBuf {
    let path = directory.join("provider script");
    fs::write(
        &path,
        r#"#!/bin/sh
mode="$1"
output="$2"
case "$mode" in
  success)
    mkdir -p "$output/frames"
    printf "frame-a" > "$output/frames/0001.png"
    printf "frame-b" > "$output/frames/0002.png"
    printf "provider complete\n"
    ;;
  fail)
    mkdir -p "$output/frames"
    printf "partial" > "$output/frames/0001.png"
    printf "credential=%s\n" "$3" >&2
    exit 23
    ;;
  sleep)
    marker="$3"
    pid_file="$4"
    (
      trap 'printf "terminated" > "$marker"; exit 0' TERM INT
      while :; do sleep 1; done
    ) &
    child_pid=$!
    printf "%s" "$child_pid" > "$pid_file"
    wait "$child_pid"
    ;;
  spam)
    while :; do printf "0123456789012345678901234567890123456789"; done
    ;;
  big)
    mkdir -p "$output/frames"
    dd if=/dev/zero of="$output/frames/0001.png" bs=1024 count=32 2>/dev/null
    sleep 2
    ;;
  unexpected)
    printf "surprise" > "$output/surprise.txt"
    ;;
  missing)
    printf "no output\n"
    ;;
  symlink)
    mkdir -p "$output/frames"
    ln -s /etc/hosts "$output/frames/link"
    ;;
  *)
    exit 64
    ;;
esac
"#,
    )
    .expect("provider script should be written");
    let mut permissions = fs::metadata(&path)
        .expect("provider script metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("provider script should be executable");
    path
}

fn registration(id: &str, executable: &Path) -> ProviderRegistration {
    let mut manifest = ProviderManifest::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-manifest-v1.example.json"
    ))
    .expect("example manifest should be valid");
    let mut configuration = ProviderConfiguration::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-configuration-v1.example.json"
    ))
    .expect("example configuration should be valid");
    let provider_id = format!("org.egohygiene.aniflow.{id}");
    manifest.provider.id.clone_from(&provider_id);
    configuration.provider.id = provider_id;

    ProviderRegistration::new(
        id,
        manifest,
        configuration,
        executable,
        format!("{id}-process"),
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
    .expect("fixture registration should be valid")
}

fn resolution_request(primary: &str) -> ProviderResolutionRequest {
    ProviderResolutionRequest {
        capability_id: "aniflow/frame.process".to_owned(),
        capability_version_requirement: "^1.0".to_owned(),
        replacement: None,
        primary: ProviderCandidate {
            registration_id: primary.to_owned(),
        },
        fallbacks: Vec::new(),
        allowed_side_effects: vec![
            SideEffect::FilesystemRead,
            SideEffect::FilesystemWrite,
            SideEffect::Subprocess,
            SideEffect::Gpu,
        ],
        offline: true,
        host: HostResources {
            cpu_threads: 8,
            memory_mib: 16 * 1024,
            storage_mib: 64 * 1024,
            gpu_available: false,
            network_available: false,
        },
    }
}

fn resolved_provider(executable: &Path) -> aniflow::ResolvedProvider {
    let mut registry = ProviderRegistry::new();
    registry
        .register(registration("primary", executable))
        .expect("registration should succeed");
    registry
        .resolve(&resolution_request("primary"))
        .expect("provider should resolve")
}

fn execution_request(root: &TempDir, mode: &str) -> ProviderExecutionRequest {
    let working_directory = root.path().join("working directory");
    let output_directory = root.path().join("output directory");
    fs::create_dir(&working_directory).expect("working directory should be created");
    fs::create_dir(&output_directory).expect("output directory should be created");
    ProviderExecutionRequest {
        arguments: vec![
            OsString::from(mode),
            output_directory.as_os_str().to_owned(),
        ],
        working_directory,
        output_directory,
        expected_outputs: vec![ExpectedProviderOutput {
            port: "processed_frames".to_owned(),
            relative_path: PathBuf::from("frames"),
            kind: ArtifactKind::Directory,
        }],
        limits: ProviderExecutionLimits {
            timeout: Duration::from_secs(3),
            termination_grace_period: Duration::from_millis(150),
            maximum_stdout_bytes: 4 * 1024,
            maximum_stderr_bytes: 4 * 1024,
            maximum_artifact_files: 10,
            maximum_artifact_bytes: 4 * 1024,
        },
        sensitive_values: Vec::new(),
    }
}

#[test]
fn resolution_uses_replacement_primary_fallback_order_and_locks_exact_identity() {
    let root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(root.path());
    let mut registry = ProviderRegistry::new();
    registry
        .register(registration("primary", &executable))
        .expect("primary should register");
    registry
        .register(registration("fallback", &executable))
        .expect("fallback should register");
    registry
        .register(registration("replacement", &executable))
        .expect("replacement should register");

    let mut replacement_request = resolution_request("primary");
    replacement_request.replacement = Some(ProviderCandidate {
        registration_id: "replacement".to_owned(),
    });
    let replacement = registry
        .resolve(&replacement_request)
        .expect("available replacement should supersede primary");
    assert_eq!(replacement.registration().registration_id(), "replacement");
    assert_eq!(replacement.attempts().len(), 1);
    assert_eq!(
        replacement.provider_lock().payload.selection.source,
        ProviderSelectionSource::Replacement
    );

    let mut request = resolution_request("primary");
    request.replacement = Some(ProviderCandidate {
        registration_id: "missing-replacement".to_owned(),
    });
    request.fallbacks.push(ProviderCandidate {
        registration_id: "fallback".to_owned(),
    });
    let resolved = registry.resolve(&request).expect("primary should resolve");
    assert_eq!(resolved.registration().registration_id(), "primary");
    assert_eq!(resolved.attempts().len(), 2);
    assert_eq!(
        resolved.attempts()[0].reasons[0].code,
        AvailabilityCode::NotRegistered
    );
    assert_eq!(
        resolved.provider_lock().payload.selection.source,
        ProviderSelectionSource::Primary
    );
    resolved
        .provider_lock()
        .validate()
        .expect("derived lock should validate");

    let lock_path = root.path().join("locks/provider.json");
    resolved
        .provider_lock()
        .write_new(&lock_path)
        .expect("new lock should persist");
    assert!(resolved.provider_lock().write_new(&lock_path).is_err());
    let persisted = fs::read(&lock_path).expect("persisted lock should be readable");
    assert_eq!(
        aniflow::ProviderLock::from_json_slice(&persisted).expect("persisted lock should parse"),
        *resolved.provider_lock()
    );

    let mut tampered =
        serde_json::to_value(resolved.provider_lock()).expect("provider lock should serialize");
    tampered["payload"]["implementation"]["id"] =
        serde_json::Value::String("tampered-process".to_owned());
    let bytes = serde_json::to_vec(&tampered).expect("tampered lock should encode");
    assert!(aniflow::ProviderLock::from_json_slice(&bytes).is_err());

    let mut fallback_request = resolution_request("missing-primary");
    fallback_request.fallbacks.push(ProviderCandidate {
        registration_id: "fallback".to_owned(),
    });
    let fallback = registry
        .resolve(&fallback_request)
        .expect("fallback should resolve before execution");
    assert_eq!(fallback.registration().registration_id(), "fallback");
    assert_eq!(
        fallback.provider_lock().payload.selection.source,
        ProviderSelectionSource::Fallback
    );
    assert_eq!(
        fallback.provider_lock().payload.selection.fallback_index,
        Some(0)
    );
}

#[test]
fn unavailability_retains_component_resource_effect_and_version_evidence() {
    let root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(root.path());
    let base = registration("limited", &executable);
    let limited = ProviderRegistration::new(
        "limited-empty",
        base.manifest().clone(),
        base.configuration().clone(),
        &executable,
        "limited-process",
        ComponentInventory::default(),
    )
    .expect("empty inventory is a valid registration observation");
    let mut registry = ProviderRegistry::new();
    registry
        .register(limited)
        .expect("provider should register");
    let mut request = resolution_request("limited-empty");
    request.capability_version_requirement = ">=2.0".to_owned();
    request.allowed_side_effects.clear();
    request.host.cpu_threads = 1;
    request.host.memory_mib = 128;
    request.host.storage_mib = 128;

    let failure = registry
        .resolve(&request)
        .expect_err("provider should be unavailable");
    let codes = failure.attempts[0]
        .reasons
        .iter()
        .map(|reason| reason.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&AvailabilityCode::VersionMismatch));
    assert!(codes.contains(&AvailabilityCode::ComponentMissing));
    assert!(codes.contains(&AvailabilityCode::InsufficientCpu));
    assert!(codes.contains(&AvailabilityCode::InsufficientMemory));
    assert!(codes.contains(&AvailabilityCode::InsufficientStorage));
    assert!(codes.contains(&AvailabilityCode::SideEffectDenied));

    let base = registration("networked", &executable);
    let mut manifest = base.manifest().clone();
    manifest.capabilities[0].requirements.compute.network = aniflow::RequirementLevel::Optional;
    manifest.capabilities[0]
        .behavior
        .side_effects
        .push(SideEffect::Network);
    let networked = ProviderRegistration::new(
        "networked-offline",
        manifest,
        base.configuration().clone(),
        &executable,
        "networked-process",
        base.components().clone(),
    )
    .expect("network-capable registration should be coherent");
    registry
        .register(networked)
        .expect("network-capable provider should register");
    let mut offline = resolution_request("networked-offline");
    offline.allowed_side_effects.push(SideEffect::Network);
    let offline_failure = registry
        .resolve(&offline)
        .expect_err("network-declaring provider should not receive an offline lock");
    assert!(
        offline_failure.attempts[0]
            .reasons
            .iter()
            .any(|reason| reason.code == AvailabilityCode::OfflineIncompatible)
    );
}

#[test]
fn successful_execution_requires_valid_outputs_and_emits_self_validating_evidence() {
    let root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(root.path());
    let resolved = resolved_provider(&executable);
    let request = execution_request(&root, "success");
    let mut observed = Vec::new();
    let report = resolved
        .execute(&request, &CancellationToken::default(), |event| {
            observed.push(event.clone());
        })
        .expect("provider execution should return a report");

    assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Succeeded);
    assert_eq!(report.payload.termination.exit_code, Some(0));
    assert_eq!(report.payload.outputs.len(), 1);
    assert_eq!(report.payload.outputs[0].file_count, 2);
    assert_eq!(report.payload.outputs[0].byte_count, 14);
    assert_eq!(observed, report.payload.events);
    report.validate().expect("execution report should validate");
    let encoded = serde_json::to_vec(&report).expect("report should serialize");
    let parsed = ProviderExecutionReport::from_json_slice(&encoded)
        .expect("serialized report should round-trip");
    assert_eq!(parsed, report);
    let text = String::from_utf8(encoded).expect("report should be UTF-8");
    assert!(!text.contains(&request.output_directory.display().to_string()));
    assert!(request.output_directory.join("frames/0001.png").is_file());
}

#[test]
fn exit_failure_redacts_diagnostics_and_cleans_partial_outputs() {
    let root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(root.path());
    let resolved = resolved_provider(&executable);
    let mut request = execution_request(&root, "fail");
    let secret = "super-secret-token";
    request.arguments.push(OsString::from(secret));
    request.sensitive_values.push(secret.to_owned());

    let report = resolved
        .execute(&request, &CancellationToken::default(), |_| {})
        .expect("provider failure should return evidence");
    assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Failed);
    assert_eq!(
        report.payload.termination.reason,
        TerminationReason::NaturalExit
    );
    assert_eq!(report.payload.termination.exit_code, Some(23));
    assert_eq!(
        report.payload.failure.as_ref().map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::ExitFailure)
    );
    assert!(report.payload.stderr.redacted);
    assert!(report.payload.stderr.retained_text.contains("[REDACTED]"));
    assert!(
        !serde_json::to_string(&report)
            .expect("report should encode")
            .contains(secret)
    );
    assert!(
        fs::read_dir(&request.output_directory)
            .expect("output directory should remain")
            .next()
            .is_none()
    );
}

#[test]
fn execution_rejects_an_executable_changed_after_resolution() {
    let root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(root.path());
    let resolved = resolved_provider(&executable);
    let mut script = fs::read_to_string(&executable).expect("script should be readable");
    script.push_str("\n# implementation changed after lock\n");
    fs::write(&executable, script).expect("script should be changed");
    let request = execution_request(&root, "success");

    let report = resolved
        .execute(&request, &CancellationToken::default(), |_| {})
        .expect("preflight rejection should return evidence");
    assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Failed);
    assert_eq!(
        report.payload.termination.reason,
        TerminationReason::PreflightRejected
    );
    assert_eq!(
        report.payload.failure.as_ref().map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::ImplementationChanged)
    );
    assert!(
        fs::read_dir(&request.output_directory)
            .expect("output directory should remain untouched")
            .next()
            .is_none()
    );
}

#[test]
fn timeout_and_cancellation_terminate_descendant_processes() {
    let timeout_root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(timeout_root.path());
    let resolved = resolved_provider(&executable);
    let mut timeout_request = execution_request(&timeout_root, "sleep");
    timeout_request.limits.timeout = Duration::from_millis(150);
    let timeout_marker = timeout_request.working_directory.join("timeout-marker");
    let timeout_pid = timeout_request.working_directory.join("timeout-pid");
    timeout_request
        .arguments
        .extend([timeout_marker.clone().into(), timeout_pid.clone().into()]);
    let timeout_report = resolved
        .execute(&timeout_request, &CancellationToken::default(), |_| {})
        .expect("timeout should return evidence");
    assert_eq!(
        timeout_report.payload.outcome,
        ProviderExecutionOutcome::TimedOut
    );
    assert_eq!(
        timeout_report.payload.termination.reason,
        TerminationReason::TimedOut
    );
    assert_process_stopped(&timeout_pid);

    let cancel_root = TempDir::new().expect("temporary directory should be created");
    let mut cancel_request = execution_request(&cancel_root, "sleep");
    cancel_request.limits.timeout = Duration::from_secs(5);
    let cancel_marker = cancel_request.working_directory.join("cancel-marker");
    let cancel_pid = cancel_request.working_directory.join("cancel-pid");
    cancel_request
        .arguments
        .extend([cancel_marker.clone().into(), cancel_pid.clone().into()]);
    let cancellation = CancellationToken::default();
    let trigger = cancellation.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        trigger.cancel();
    });
    let cancel_report = resolved
        .execute(&cancel_request, &cancellation, |_| {})
        .expect("cancellation should return evidence");
    canceller.join().expect("canceller should finish");
    assert_eq!(
        cancel_report.payload.outcome,
        ProviderExecutionOutcome::Cancelled
    );
    assert_process_stopped(&cancel_pid);
}

#[test]
fn capture_artifact_and_output_contract_violations_fail_closed() {
    let executable_root = TempDir::new().expect("temporary directory should be created");
    let executable = write_provider_script(executable_root.path());
    let resolved = resolved_provider(&executable);

    let spam_root = TempDir::new().expect("temporary directory should be created");
    let mut spam = execution_request(&spam_root, "spam");
    spam.limits.maximum_stdout_bytes = 128;
    let spam_report = resolved
        .execute(&spam, &CancellationToken::default(), |_| {})
        .expect("capture limit should return evidence");
    assert_eq!(
        spam_report.payload.outcome,
        ProviderExecutionOutcome::CaptureLimitExceeded
    );
    assert_eq!(
        spam_report
            .payload
            .failure
            .as_ref()
            .map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::StdoutLimit)
    );
    assert!(spam_report.payload.stdout.truncated);

    let big_root = TempDir::new().expect("temporary directory should be created");
    let mut big = execution_request(&big_root, "big");
    big.limits.maximum_artifact_bytes = 1024;
    let big_report = resolved
        .execute(&big, &CancellationToken::default(), |_| {})
        .expect("artifact limit should return evidence");
    assert_eq!(
        big_report.payload.outcome,
        ProviderExecutionOutcome::ArtifactLimitExceeded
    );
    assert!(
        fs::read_dir(&big.output_directory)
            .expect("output directory should remain")
            .next()
            .is_none()
    );

    let unexpected_root = TempDir::new().expect("temporary directory should be created");
    let unexpected = execution_request(&unexpected_root, "unexpected");
    let unexpected_report = resolved
        .execute(&unexpected, &CancellationToken::default(), |_| {})
        .expect("invalid output should return evidence");
    assert_eq!(
        unexpected_report.payload.outcome,
        ProviderExecutionOutcome::InvalidOutput
    );
    assert_eq!(
        unexpected_report
            .payload
            .failure
            .as_ref()
            .map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::UnexpectedOutput)
    );

    let missing_root = TempDir::new().expect("temporary directory should be created");
    let missing = execution_request(&missing_root, "missing");
    let missing_report = resolved
        .execute(&missing, &CancellationToken::default(), |_| {})
        .expect("missing output should return evidence");
    assert_eq!(
        missing_report
            .payload
            .failure
            .as_ref()
            .map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::MissingOutput)
    );

    let symlink_root = TempDir::new().expect("temporary directory should be created");
    let symlink = execution_request(&symlink_root, "symlink");
    let symlink_report = resolved
        .execute(&symlink, &CancellationToken::default(), |_| {})
        .expect("symlink output should return evidence");
    assert_eq!(
        symlink_report
            .payload
            .failure
            .as_ref()
            .map(|failure| failure.code),
        Some(ProviderExecutionFailureCode::SymlinkOutput)
    );
}

fn assert_process_stopped(pid_file: &Path) {
    let started = Instant::now();
    while !pid_file.is_file() && started.elapsed() < Duration::from_secs(1) {
        thread::sleep(Duration::from_millis(10));
    }
    let pid = fs::read_to_string(pid_file)
        .expect("provider should record its descendant pid")
        .parse::<i32>()
        .expect("descendant pid should be numeric");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match kill(Pid::from_raw(pid), None) {
            Err(Errno::ESRCH) => return,
            _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            result => panic!("descendant process {pid} survived termination: {result:?}"),
        }
    }
}
