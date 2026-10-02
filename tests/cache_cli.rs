//! Offline CLI contracts authored for #34; execution deferred under #64.
use std::process::Command;
use aniflow::cache_v3::CachePolicy;
use serde_json::Value;

#[test]
fn inspection_machine_contract_does_not_initialize_the_namespace() {
    let root = tempfile::tempdir().unwrap();
    let policy = root.path().join("policy.json");
    let namespace = root.path().join("cache");
    std::fs::write(&policy, serde_json::to_vec(&CachePolicy::new(&namespace, "fixture")).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow")).args(["--output", "json", "cache", "inspect", "--policy"]).arg(&policy).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "cache_inspect");
    assert_eq!(value["result"]["exists"], false);
    assert!(!namespace.exists());
}

#[test]
fn prune_defaults_to_preview_and_reports_no_deletions() {
    let root = tempfile::tempdir().unwrap(); let path = root.path().join("policy.json");
    let namespace = root.path().join("cache");
    std::fs::write(&path, serde_json::to_vec(&CachePolicy::new(&namespace, "fixture")).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow")).args(["--output", "json", "cache", "prune", "--policy"]).arg(path).output().unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["dry_run"], true);
    assert_eq!(value["result"]["removed_keys"], serde_json::json!([]));
    assert!(!namespace.exists());
}

#[test]
fn invalid_policy_has_a_typed_cache_diagnostic() {
    let root = tempfile::tempdir().unwrap(); let path = root.path().join("policy.json");
    let mut policy = CachePolicy::new(root.path().join("cache"), "fixture"); policy.maximum_entries = 0;
    std::fs::write(&path, serde_json::to_vec(&policy).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow")).args(["--output", "json", "cache", "inspect", "--policy"]).arg(path).output().unwrap();
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["cache"]["code"], "invalid_policy");
}
