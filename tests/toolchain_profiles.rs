//! Synthetic offline toolchain boundary cases. Authored; execution deferred #64.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use aniflow::toolchain::{ToolchainInventory, ToolchainProfile, inspect_profile};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Fixture {
    _root: tempfile::TempDir,
    directory: PathBuf,
    tool: PathBuf,
    profile: Value,
    inventory: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap();
        let tool = directory.join("native ü 'quoted' $(touch SENTINEL); tool");
        let bytes = b"#!/bin/sh\n: > \"$0.launched\"\n";
        fs::write(&tool, bytes).unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        let sha256 = digest(bytes);
        let profile = json!({
            "schema": "aniflow.toolchain-profile/v1", "id": "offline-fixture",
            "default_capabilities": ["inspect"],
            "dependencies": [
                {"id": "ffmpeg", "kind": "tool", "optional": false,
                 "version_requirement": ">=6.1.0, <7.0.0", "sha256": sha256,
                 "required_flags": ["-i", "-map"], "required_features": ["encoder:libx264"], "suggested_locators": []},
                {"id": "upscayl", "kind": "tool", "optional": true,
                 "version_requirement": "=2.17.0", "required_flags": ["-i", "-o"],
                 "required_features": [], "suggested_locators": []},
                {"id": "weights", "kind": "model", "optional": true,
                 "required_flags": [], "required_features": [], "native_scale": 4, "suggested_locators": []}
            ],
            "capabilities": [
                {"id": "inspect", "optional": false, "dependency_ids": ["ffmpeg"],
                 "platforms": [{"os": "linux", "arch": "x86_64"}],
                 "effective_settings": {"offline": true}, "side_effects": ["filesystem_read", "subprocess"]},
                {"id": "upscale", "optional": true, "dependency_ids": ["upscayl", "weights"],
                 "platforms": [{"os": "linux", "arch": "x86_64"}],
                 "backend": {"name": "vulkan", "required_features": ["compute"]},
                 "scale": {"native": 4, "requested": 2, "mode": "post_resize"},
                 "effective_settings": {"tile": 128}, "side_effects": ["filesystem_read", "filesystem_write", "gpu", "subprocess"]}
            ]
        });
        let inventory = json!({
            "schema": "aniflow.toolchain-inventory/v1",
            "platform": {"os": "linux", "arch": "x86_64"},
            "artifacts": [{"dependency_id": "ffmpeg", "path": tool,
                "expected_sha256": sha256, "maximum_bytes": 4096,
                "observation": {"executable_sha256": sha256, "version": "6.1.1", "package_revision": "synthetic-ffmpeg-build",
                    "flags": ["-i", "-map"], "features": ["encoder:libx264"],
                    "provenance": "caller supplied synthetic observation"}}],
            "hardware": []
        });
        Self { _root: root, directory, tool, profile, inventory }
    }

    fn inspect(&self, selected: &[&str]) -> Value {
        let profile = ToolchainProfile::from_json_slice(&serde_json::to_vec(&self.profile).unwrap()).unwrap();
        let inventory = ToolchainInventory::from_json_slice(&serde_json::to_vec(&self.inventory).unwrap()).unwrap();
        let selected = selected.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        serde_json::to_value(inspect_profile(&profile, &inventory, &selected).unwrap()).unwrap()
    }

    fn install_optional_fixtures(&mut self) {
        let model = self.directory.join("model 'weights'; $().bin");
        let weights = b"synthetic model bytes only";
        fs::write(&model, weights).unwrap();
        let tool_hash = digest(&fs::read(&self.tool).unwrap());
        self.inventory["artifacts"].as_array_mut().unwrap().extend([
            json!({"dependency_id":"upscayl", "path":self.tool, "expected_sha256":tool_hash,
                "maximum_bytes":4096, "observation":{"executable_sha256":tool_hash,
                    "version":"2.17.0", "package_revision":"synthetic-upscayl-build", "flags":["-i","-o"], "features":[], "provenance":"synthetic tool observation"}}),
            json!({"dependency_id":"weights", "path":model, "expected_sha256":digest(weights),
                "maximum_bytes":4096, "observation":{"executable_sha256":digest(weights),
                    "package_revision":"synthetic-weights-v1", "native_scale":4,
                    "flags":[], "features":[], "provenance":"synthetic reviewed model identity"}}),
        ]);
        self.inventory["hardware"] = json!([{"name":"vulkan", "available":true,
            "features":["compute"], "provenance":"caller supplied synthetic hardware observation"}]);
    }
}

fn has_status(report: &Value, dependency: Option<&str>, status: &str) -> bool {
    report["facts"].as_array().unwrap().iter().any(|fact| {
        fact["status"] == status && dependency.is_none_or(|id| fact["dependency_id"] == id)
    })
}

fn file_tree(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = walkdir::WalkDir::new(path).follow_links(false).into_iter().map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| (entry.path().strip_prefix(path).unwrap().to_owned(), fs::read(entry.path()).unwrap()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
fn local_paths_are_literal_and_inspection_never_launches_or_changes_files() {
    let fixture = Fixture::new();
    let before = file_tree(&fixture.directory);
    let report = fixture.inspect(&[]);
    assert_eq!(report["ready"], true);
    assert_eq!(report["native_qualification"], false);
    assert_eq!(report["selected_capabilities"], json!(["inspect"]));
    assert!(!fixture.tool.with_file_name(format!("{}.launched", fixture.tool.file_name().unwrap().to_str().unwrap())).exists());
    assert_eq!(file_tree(&fixture.directory), before);
    assert!(report["facts"].as_array().unwrap().iter().any(|fact|
        fact["check"] == "observation_binding" && fact["status"] == "installed"
            && fact["provenance"] == "caller supplied synthetic observation"));
}

#[test]
fn stale_bytes_wrong_versions_and_absent_observations_are_distinct_refusals() {
    for case in ["changed_file", "wrong_version", "raw_version", "missing_revision", "stale_observation", "no_observation"] {
        let mut fixture = Fixture::new();
        match case {
            "changed_file" => fs::write(&fixture.tool, b"different bytes").unwrap(),
            "wrong_version" => fixture.inventory["artifacts"][0]["observation"]["version"] = json!("5.0.0"),
            "raw_version" => fixture.inventory["artifacts"][0]["observation"]["version"] = json!("ffmpeg version unqualified-custom-build"),
            "missing_revision" => { fixture.inventory["artifacts"][0]["observation"].as_object_mut().unwrap().remove("package_revision"); },
            "stale_observation" => fixture.inventory["artifacts"][0]["observation"]["executable_sha256"] = json!("a".repeat(64)),
            "no_observation" => { fixture.inventory["artifacts"][0].as_object_mut().unwrap().remove("observation"); },
            _ => unreachable!(),
        }
        let report = fixture.inspect(&["inspect"]);
        assert_eq!(report["ready"], false, "{case}");
        let expected_status = if matches!(case, "changed_file" | "wrong_version") { "incompatible" } else { "unverified" };
        assert!(has_status(&report, Some("ffmpeg"), expected_status), "{case}: {report}");
        if case == "stale_observation" {
            assert!(report["facts"].as_array().unwrap().iter().any(|fact|
                fact["check"] == "observation_binding" && fact["status"] == "incompatible"));
            assert!(report["facts"].as_array().unwrap().iter()
                .filter(|fact| matches!(fact["check"].as_str(), Some("version" | "package_revision" | "flags" | "features")))
                .all(|fact| fact["status"] == "unverified"));
        }
        assert_eq!(report["native_qualification"], false);
    }
}

#[test]
fn missing_optional_tools_models_and_backends_do_not_block_the_default_capability() {
    let fixture = Fixture::new();
    assert_eq!(fixture.inspect(&[])["ready"], true);
    let selected = fixture.inspect(&["upscale"]);
    assert_eq!(selected["ready"], false);
    assert!(has_status(&selected, Some("upscayl"), "missing"));
    assert!(has_status(&selected, Some("weights"), "missing"));
    assert!(selected["facts"].as_array().unwrap().iter().all(|fact| fact["capability_id"] == "upscale"));
    assert!(!selected["actions"].as_array().unwrap().is_empty());
}

#[test]
fn missing_codec_and_flag_observations_never_inherit_executable_readiness() {
    for field in ["features", "flags"] {
        let mut fixture = Fixture::new();
        fixture.inventory["artifacts"][0]["observation"][field] = json!([]);
        let report = fixture.inspect(&[]);
        assert_eq!(report["ready"], false);
        assert!(has_status(&report, Some("ffmpeg"), "incompatible"));
        assert!(report["facts"].as_array().unwrap().iter().any(|fact|
            fact["dependency_id"] == "ffmpeg" && fact["check"] == field && fact["status"] == "incompatible"));
    }
}

#[test]
fn model_scale_and_backend_evidence_remain_separate_from_requested_resize() {
    let mut fixture = Fixture::new();
    fixture.install_optional_fixtures();
    let ready = fixture.inspect(&["upscale"]);
    assert_eq!(ready["ready"], true);
    assert_eq!(ready["native_qualification"], false);
    fixture.inventory["artifacts"][2]["observation"]["native_scale"] = json!(2);
    let mismatch = fixture.inspect(&["upscale"]);
    assert_eq!(mismatch["ready"], false);
    assert!(has_status(&mismatch, Some("weights"), "incompatible"));
    fixture.inventory["artifacts"][2]["observation"].as_object_mut().unwrap().remove("native_scale");
    assert!(has_status(&fixture.inspect(&["upscale"]), Some("weights"), "unverified"));
    fixture.inventory["artifacts"][2]["observation"]["native_scale"] = json!(4);
    fixture.inventory["artifacts"][2]["observation"].as_object_mut().unwrap().remove("package_revision");
    assert!(has_status(&fixture.inspect(&["upscale"]), Some("weights"), "unverified"));
    fixture.inventory["artifacts"][2]["observation"]["package_revision"] = json!("synthetic-weights-v1");
    fixture.inventory["hardware"][0]["available"] = json!(false);
    assert_eq!(fixture.inspect(&["upscale"])["ready"], false);
    fixture.inventory["hardware"][0]["available"] = json!(true);
    fixture.inventory["hardware"][0]["features"] = json!([]);
    assert_eq!(fixture.inspect(&["upscale"])["ready"], false);
    fixture.profile["capabilities"][1]["scale"]["mode"] = json!("native");
    assert!(ToolchainProfile::from_json_slice(&serde_json::to_vec(&fixture.profile).unwrap()).is_err());
}

#[test]
fn profiles_and_inventories_refuse_ambiguous_ids_references_and_unknown_fields() {
    for case in ["duplicate_dependency", "unknown_dependency", "optional_dependency_in_core", "unknown_field", "unknown_version"] {
        let mut fixture = Fixture::new();
        match case {
            "duplicate_dependency" => { let duplicate = fixture.profile["dependencies"][0].clone(); fixture.profile["dependencies"].as_array_mut().unwrap().push(duplicate); },
            "unknown_dependency" => fixture.profile["capabilities"][0]["dependency_ids"] = json!(["absent"]),
            "optional_dependency_in_core" => fixture.profile["capabilities"][0]["dependency_ids"] = json!(["weights"]),
            "unknown_field" => fixture.profile["dependencies"][0]["execute"] = json!(true),
            "unknown_version" => fixture.profile["schema"] = json!("aniflow.toolchain-profile/v2"),
            _ => unreachable!(),
        }
        assert!(ToolchainProfile::from_json_slice(&serde_json::to_vec(&fixture.profile).unwrap()).is_err(), "{case}");
    }
    let mut fixture = Fixture::new();
    let encoded = serde_json::to_string(&fixture.profile).unwrap().replacen(
        "\"schema\":", "\"schema\":\"aniflow.toolchain-profile/v1\",\"schema\":", 1
    );
    assert!(ToolchainProfile::from_json_slice(encoded.as_bytes()).is_err());
    let duplicate = fixture.inventory["artifacts"][0].clone();
    fixture.inventory["artifacts"].as_array_mut().unwrap().push(duplicate);
    assert!(ToolchainInventory::from_json_slice(&serde_json::to_vec(&fixture.inventory).unwrap()).is_err());
    fixture.inventory["artifacts"].as_array_mut().unwrap().pop();
    fixture.inventory["hardware"] = json!([{"name":"vulkan","available":true,"features":[],"provenance":"fixture","execute":true}]);
    assert!(ToolchainInventory::from_json_slice(&serde_json::to_vec(&fixture.inventory).unwrap()).is_err());
}

#[test]
fn inspection_reports_are_deterministic_and_reject_unknown_selection_or_inventory_references() {
    let fixture = Fixture::new();
    assert_eq!(fixture.inspect(&[]), fixture.inspect(&["inspect"]));
    let profile = ToolchainProfile::from_json_slice(&serde_json::to_vec(&fixture.profile).unwrap()).unwrap();
    let mut inventory = fixture.inventory.clone();
    inventory["artifacts"][0]["dependency_id"] = json!("undeclared");
    let inventory = ToolchainInventory::from_json_slice(&serde_json::to_vec(&inventory).unwrap()).unwrap();
    assert!(inspect_profile(&profile, &inventory, &[]).is_err());
    let inventory = ToolchainInventory::from_json_slice(&serde_json::to_vec(&fixture.inventory).unwrap()).unwrap();
    assert!(inspect_profile(&profile, &inventory, &["unknown".to_owned()]).is_err());
    assert!(inspect_profile(&profile, &inventory, &["inspect".to_owned(), "inspect".to_owned()]).is_err());
}

#[test]
fn packaged_profile_and_inventory_templates_parse_without_inspecting_or_claiming_installation() {
    let profile = ToolchainProfile::from_json_slice(include_bytes!("../profiles/music-video-v1.json")).unwrap();
    assert!(!profile.default_capabilities.is_empty());
    for bytes in [
        include_bytes!("../profiles/inventory-linux.example.json").as_slice(),
        include_bytes!("../profiles/inventory-macos.example.json").as_slice(),
    ] {
        let inventory = ToolchainInventory::from_json_slice(bytes).unwrap();
        assert!(!inventory.platform.os.is_empty());
    }
    // Parsing validates declarations only. Placeholder paths are never read by
    // this test, and parsing alone cannot supply an inspection report.
}

#[test]
fn cli_plan_preserves_diagnostics_while_doctor_exits_not_ready_without_launching() {
    let fixture = Fixture::new();
    let profile = fixture.directory.join("profile.json");
    let inventory = fixture.directory.join("inventory.json");
    fs::write(&profile, serde_json::to_vec(&fixture.profile).unwrap()).unwrap();
    fs::write(&inventory, serde_json::to_vec(&fixture.inventory).unwrap()).unwrap();
    let before = file_tree(&fixture.directory);
    let invoke = |command: &str| Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "toolchain", command, "--profile"])
        .arg(&profile).arg("--inventory").arg(&inventory)
        .args(["--capability", "upscale"]).output().unwrap();
    let plan = invoke("plan");
    assert!(plan.status.success(), "{}", String::from_utf8_lossy(&plan.stderr));
    let planned: Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(planned["command"], "toolchain_plan");
    assert_eq!(planned["result"]["ready"], false);
    let doctor = invoke("doctor");
    assert!(!doctor.status.success());
    assert!(doctor.stdout.is_empty());
    let diagnosed: Value = serde_json::from_slice(&doctor.stderr).unwrap();
    assert_eq!(diagnosed["command"], "toolchain_doctor");
    assert_eq!(diagnosed["result"], planned["result"]);
    assert_eq!(file_tree(&fixture.directory), before);
}
