//! Synthetic registration preparation coverage. Authored; execution deferred #64.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use aniflow::audio_inspection::AudioInspectionProviderConfiguration;
use aniflow::toolchain::{
    AudioInspectionRegistrationRequest, ToolchainPreflightConfiguration,
    ToolchainRegistrationPreparation, prepare_audio_inspection_registration, preflight_toolchain,
};
use aniflow::{
    HostResources, PipelineInputBinding, PipelinePlanningContext, PipelineV3Configuration,
    PipelineV3Plan, ProviderRegistrationDocument, ProviderRegistry, SideEffect,
    bind_toolchain_preflight, resolve_pipeline_v3,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn synthetic_wave() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&38_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8000_u32.to_le_bytes());
    bytes.extend_from_slice(&16000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes
}

fn file_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = walkdir::WalkDir::new(root).follow_links(false).into_iter()
        .map(|entry| entry.unwrap()).filter(|entry| entry.file_type().is_file())
        .map(|entry| (entry.path().strip_prefix(root).unwrap().to_owned(), fs::read(entry.path()).unwrap()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

struct Fixture {
    root: tempfile::TempDir,
    directory: PathBuf,
    adapter: PathBuf,
    ffmpeg: PathBuf,
    source: PathBuf,
    request: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        let directory = root_path.join("reviewable registration");
        fs::create_dir_all(directory.join("bin")).unwrap();
        let executable = |path: &Path, name: &str| {
            // Preparation, registration loading and planning must only read
            // these files. Any accidental launch leaves an observable marker.
            let bytes = format!("#!/bin/sh\n# synthetic {name}\n: > \"$0.launched\"\nexit 97\n");
            fs::write(path, &bytes).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            digest(bytes.as_bytes())
        };
        let adapter_locator = "bin/native ü 'literal' $(touch SENTINEL); provider";
        let adapter = directory.join(adapter_locator);
        let adapter_sha256 = executable(&adapter, "adapter");
        let ffmpeg = root_path.join("ffmpeg ü 'literal' $(touch SENTINEL); tool");
        let ffprobe = root_path.join("ffprobe ü 'literal' $(touch SENTINEL); tool");
        let ffmpeg_sha256 = executable(&ffmpeg, "ffmpeg");
        let ffprobe_sha256 = executable(&ffprobe, "ffprobe");
        let source = root_path.join("synthetic source.wav");
        let wave = synthetic_wave();
        fs::write(&source, &wave).unwrap();
        let dependencies = ["local-ffmpeg", "local-ffprobe"].map(|id| json!({
            "id":id, "kind":"tool", "optional":false, "version_requirement":"=6.1.1",
            "required_flags":[], "required_features":[], "suggested_locators":[],
        }));
        let artifacts = [
            ("local-ffmpeg", &ffmpeg, &ffmpeg_sha256),
            ("local-ffprobe", &ffprobe, &ffprobe_sha256),
        ].map(|(id, path, sha256)| json!({
            "dependency_id":id, "path":path, "expected_sha256":sha256, "maximum_bytes":4096,
            "observation":{
                "executable_sha256":sha256, "version":"6.1.1", "package_revision":"synthetic-only",
                "flags":[], "features":[], "provenance":"supplied synthetic identity; never probed",
            },
        }));
        let platform = json!({"os":std::env::consts::OS, "arch":std::env::consts::ARCH});
        let request = json!({
            "schema":"aniflow.toolchain.audio-inspection-registration/v1",
            "registration_directory":directory,
            "adapter":{
                "relative_path":adapter_locator, "implementation_id":"aniflow-audio-inspection-v2",
                "expected_sha256":adapter_sha256, "maximum_bytes":4096,
            },
            "preflight":{
                "schema":"aniflow.toolchain.preflight/v1",
                "profile":{
                    "schema":"aniflow.toolchain-profile/v1", "id":"registration-fixture",
                    "default_capabilities":["inspect"], "dependencies":dependencies,
                    "capabilities":[{"id":"inspect", "optional":false,
                        "dependency_ids":["local-ffmpeg", "local-ffprobe"],
                        "platforms":[platform.clone()], "effective_settings":{},
                        "side_effects":["filesystem_read", "filesystem_write", "environment_read", "subprocess"]}],
                },
                "inventory":{
                    "schema":"aniflow.toolchain-inventory/v1", "platform":platform,
                    "artifacts":artifacts, "hardware":[],
                },
                "bindings":[{"stage_id":"inspect_audio", "registration_id":"audio-inspection-native",
                    "capability_ids":["inspect"], "ffmpeg_dependency_id":"local-ffmpeg",
                    "ffprobe_dependency_id":"local-ffprobe"}],
            },
            "source":{"id":"source_audio", "sha256":digest(&wave), "byte_size":wave.len()},
            "tool_timeout_milliseconds":2000, "maximum_tool_output_bytes":4096,
        });
        Self { root, directory, adapter, ffmpeg, source, request }
    }

    fn configuration(&self) -> AudioInspectionRegistrationRequest {
        AudioInspectionRegistrationRequest::from_json_slice(&serde_json::to_vec(&self.request).unwrap()).unwrap()
    }

    fn prepare(&self) -> ToolchainRegistrationPreparation {
        prepare_audio_inspection_registration(&self.configuration()).unwrap()
    }

    fn write_reviewed_files(&self, result: &ToolchainRegistrationPreparation) {
        assert!(result.ready, "{result:?}");
        for file in &result.files {
            // The caller explicitly materializes returned inert documents;
            // preparation itself is not allowed to create any of these files.
            fs::write(self.directory.join(&file.relative_path), serde_json::to_vec(&file.content).unwrap()).unwrap();
        }
    }

    fn registry(&self) -> ProviderRegistry {
        let registration = ProviderRegistrationDocument::load(self.directory.join("registration.json"))
            .unwrap().into_registration().unwrap();
        let mut registry = ProviderRegistry::new();
        registry.register(registration).unwrap();
        registry
    }

    fn plan(&self, registry: &ProviderRegistry) -> PipelineV3Plan {
        let pipeline = PipelineV3Configuration::from_yaml_slice(include_bytes!(
            "../providers/audio-inspection/pipeline.yml"
        )).unwrap();
        let context = PipelinePlanningContext {
            host:HostResources { cpu_threads:1, memory_mib:0, storage_mib:0,
                gpu_available:false, network_available:false },
            allowed_side_effects:vec![SideEffect::FilesystemRead, SideEffect::FilesystemWrite,
                SideEffect::EnvironmentRead, SideEffect::Subprocess], offline:true,
        };
        resolve_pipeline_v3(&pipeline, &[PipelineInputBinding::new("source_audio", &self.source)], registry, &context).unwrap()
    }
}

fn assert_refused(value: &Value) {
    if let Ok(request) = AudioInspectionRegistrationRequest::from_json_slice(&serde_json::to_vec(value).unwrap()) {
        if let Ok(result) = prepare_audio_inspection_registration(&request) {
            assert!(!result.ready, "unexpected ready preparation: {result:?}");
            assert!(!result.native_qualification);
            assert!(result.files.is_empty(), "refused preparation exposed usable files");
            assert!(!result.diagnostics.is_empty());
        }
    }
}

#[test]
fn preparation_is_inert_deterministic_and_loads_into_existing_explicit_pipeline() {
    let fixture = Fixture::new();
    let before = file_tree(fixture.root.path());
    let request = fixture.configuration();
    let result = fixture.prepare();
    assert!(result.ready, "{result:?}");
    assert!(!result.native_qualification);
    assert_eq!(result.request_sha256, digest(&request.canonical_json_bytes().unwrap()));
    assert_eq!(serde_json::to_value(&result).unwrap(), serde_json::to_value(fixture.prepare()).unwrap());
    assert_eq!(result.files.iter().map(|file| file.relative_path.as_str()).collect::<Vec<_>>(),
        ["configuration.json", "manifest.json", "preflight.json", "registration.json"]);
    for file in &result.files {
        assert_eq!(file.sha256, digest(&serde_json::to_vec(&file.content).unwrap()));
    }
    assert_eq!(file_tree(fixture.root.path()), before);
    assert!(!fixture.directory.join("registration.json").exists());

    fixture.write_reviewed_files(&result);
    let documents_written = file_tree(fixture.root.path());
    let registry = fixture.registry();
    let registration = registry.registration("audio-inspection-native").unwrap();
    assert_eq!(registration.executable(), fixture.adapter.as_path());
    let typed: AudioInspectionProviderConfiguration = serde_json::from_value(
        serde_json::to_value(&registration.configuration().values).unwrap()
    ).unwrap();
    assert_eq!(typed.settings.ffmpeg.executable, fixture.ffmpeg);
    assert_eq!(typed.settings.tool_timeout_milliseconds, 2000);
    assert_eq!(typed.settings.maximum_tool_output_bytes, 4096);
    for id in ["ffmpeg", "ffprobe"] {
        let component = registration.components().tools.iter().find(|tool| tool.id == id).unwrap();
        let requirement = registration.capability().requirements.tools.iter().find(|tool| tool.id == id).unwrap();
        assert_eq!(requirement.version_requirement, "=6.1.1");
        assert_eq!(requirement.sha256, component.sha256);
        assert!(component.sha256.is_some());
    }
    let preflight = ToolchainPreflightConfiguration::load(fixture.directory.join("preflight.json")).unwrap();
    let plan = fixture.plan(&registry);
    assert_eq!(Some(&plan.payload.stages[0].provider_lock.payload.implementation), result.adapter.as_ref());
    let bound = bind_toolchain_preflight(plan, &registry, &preflight).unwrap();
    assert_eq!(bound.payload.toolchain_preflight_sha256, Some(preflight.configuration_sha256().unwrap()));
    let report = preflight_toolchain(&bound, &registry, &preflight).unwrap();
    assert!(report.ready, "{report:?}");
    assert!(!report.native_qualification);
    assert_eq!(file_tree(fixture.root.path()), documents_written);
}

#[test]
fn preparation_does_not_read_media_or_discover_unselected_dependencies() {
    let mut fixture = Fixture::new();
    fs::remove_file(&fixture.source).unwrap();
    fixture.request["preflight"]["profile"]["dependencies"].as_array_mut().unwrap().push(json!({
        "id":"optional-upscayl", "kind":"tool", "optional":true, "version_requirement":"=2.17.0",
        "required_flags":[], "required_features":[], "suggested_locators":[],
    }));
    let platform = fixture.request["preflight"]["inventory"]["platform"].clone();
    fixture.request["preflight"]["profile"]["capabilities"].as_array_mut().unwrap().push(json!({
        "id":"upscale", "optional":true, "dependency_ids":["optional-upscayl"],
        "platforms":[platform], "effective_settings":{},
        "side_effects":["filesystem_read", "filesystem_write", "subprocess"],
    }));
    let before = file_tree(fixture.root.path());
    let result = fixture.prepare();
    assert!(result.ready, "{result:?}");
    assert!(!result.native_qualification);
    assert_eq!(result.inspection.selected_capabilities, ["inspect"]);
    assert!(result.inspection.facts.iter().all(|fact| fact.capability_id == "inspect"
        && fact.dependency_id.as_deref() != Some("optional-upscayl")));
    assert_eq!(file_tree(fixture.root.path()), before);
}

#[test]
fn missing_stale_and_incompatible_identity_never_produces_registration_files() {
    for case in ["changed_tool", "missing_tool", "wrong_pin", "stale_observation", "wrong_version",
        "missing_observation", "wrong_platform", "changed_adapter", "wrong_adapter_pin", "adapter_limit"] {
        let mut fixture = Fixture::new();
        match case {
            "changed_tool" => fs::write(&fixture.ffmpeg, b"changed synthetic executable").unwrap(),
            "missing_tool" => fs::remove_file(&fixture.ffmpeg).unwrap(),
            "wrong_pin" => fixture.request["preflight"]["inventory"]["artifacts"][0]["expected_sha256"] = json!("a".repeat(64)),
            "stale_observation" => fixture.request["preflight"]["inventory"]["artifacts"][0]["observation"]["executable_sha256"] = json!("a".repeat(64)),
            "wrong_version" => fixture.request["preflight"]["inventory"]["artifacts"][0]["observation"]["version"] = json!("6.2.0"),
            "missing_observation" => { fixture.request["preflight"]["inventory"]["artifacts"][0].as_object_mut().unwrap().remove("observation"); },
            "wrong_platform" => fixture.request["preflight"]["inventory"]["platform"]["os"] = json!("unsupported-host"),
            "changed_adapter" => fs::write(&fixture.adapter, b"changed synthetic provider").unwrap(),
            "wrong_adapter_pin" => fixture.request["adapter"]["expected_sha256"] = json!("a".repeat(64)),
            "adapter_limit" => fixture.request["adapter"]["maximum_bytes"] = json!(1),
            _ => unreachable!(),
        }
        let before = file_tree(fixture.root.path());
        assert_refused(&fixture.request);
        assert_eq!(file_tree(fixture.root.path()), before, "{case}");
    }
}

#[test]
fn profile_cannot_expand_or_understate_the_typed_adapter_authority() {
    let fixture = Fixture::new();
    for case in ["missing_effect", "extra_effect", "settings", "backend", "scale", "same_tool", "extra_binding"] {
        let mut request = fixture.request.clone();
        match case {
            "missing_effect" => request["preflight"]["profile"]["capabilities"][0]["side_effects"] = json!(["filesystem_read"]),
            "extra_effect" => request["preflight"]["profile"]["capabilities"][0]["side_effects"].as_array_mut().unwrap().push(json!("network")),
            "settings" => request["preflight"]["profile"]["capabilities"][0]["effective_settings"] = json!({"unapplied":true}),
            "backend" => request["preflight"]["profile"]["capabilities"][0]["backend"] = json!({"name":"vulkan", "required_features":[]}),
            "scale" => request["preflight"]["profile"]["capabilities"][0]["scale"] = json!({"native":2, "requested":2, "mode":"native"}),
            "same_tool" => request["preflight"]["bindings"][0]["ffprobe_dependency_id"] = json!("local-ffmpeg"),
            "extra_binding" => {
                let mut binding = request["preflight"]["bindings"][0].clone();
                binding["stage_id"] = json!("another_stage");
                request["preflight"]["bindings"].as_array_mut().unwrap().push(binding);
            },
            _ => unreachable!(),
        }
        let before = file_tree(fixture.root.path());
        assert_refused(&request);
        assert_eq!(file_tree(fixture.root.path()), before, "{case}");
    }
}

#[test]
fn adapter_locator_is_confined_and_rejects_symlinks_and_nonexecutables() {
    for locator in ["../escape", "/absolute/provider", "bin/../provider", "bin\\provider", "C:/provider"] {
        let fixture = Fixture::new();
        let mut request = fixture.request.clone();
        request["adapter"]["relative_path"] = json!(locator);
        assert_refused(&request);
    }
    for locator in ["manifest.json", "Manifest.JSON", "registration.json/provider", "CONFIGURATION.JSON/provider"] {
        let fixture = Fixture::new();
        let mut request = fixture.request.clone();
        request["adapter"]["relative_path"] = json!(locator);
        assert!(AudioInspectionRegistrationRequest::from_json_slice(&serde_json::to_vec(&request).unwrap()).is_err(),
            "adapter must not overlap emitted documents on a case-insensitive filesystem: {locator}");
    }
    for case in ["leaf_symlink", "parent_symlink", "root_symlink", "not_executable", "directory"] {
        let mut fixture = Fixture::new();
        match case {
            "leaf_symlink" => {
                fs::remove_file(&fixture.adapter).unwrap();
                symlink(&fixture.ffmpeg, &fixture.adapter).unwrap();
                fixture.request["adapter"]["expected_sha256"] = json!(digest(&fs::read(&fixture.ffmpeg).unwrap()));
            },
            "parent_symlink" => {
                let actual = fixture.directory.join("real-bin");
                fs::rename(fixture.directory.join("bin"), &actual).unwrap();
                symlink(actual, fixture.directory.join("bin")).unwrap();
            },
            "root_symlink" => {
                let alias = fixture.root.path().join("registration-alias");
                symlink(&fixture.directory, &alias).unwrap();
                fixture.request["registration_directory"] = json!(alias);
            },
            "not_executable" => fs::set_permissions(&fixture.adapter, fs::Permissions::from_mode(0o644)).unwrap(),
            "directory" => {
                fs::remove_file(&fixture.adapter).unwrap();
                fs::create_dir(&fixture.adapter).unwrap();
            },
            _ => unreachable!(),
        }
        let before = file_tree(fixture.root.path());
        assert_refused(&fixture.request);
        assert_eq!(file_tree(fixture.root.path()), before, "{case}");
    }
}

#[test]
fn changed_adapter_cannot_reuse_prepared_request_or_bound_plan() {
    let fixture = Fixture::new();
    let result = fixture.prepare();
    fixture.write_reviewed_files(&result);
    let registry = fixture.registry();
    let preflight = ToolchainPreflightConfiguration::load(fixture.directory.join("preflight.json")).unwrap();
    let bound = bind_toolchain_preflight(fixture.plan(&registry), &registry, &preflight).unwrap();
    fs::write(&fixture.adapter, b"changed synthetic provider after preparation").unwrap();
    let before = file_tree(fixture.root.path());
    assert_refused(&fixture.request);
    let report = preflight_toolchain(&bound, &registry, &preflight).unwrap();
    assert!(!report.ready);
    assert!(!report.diagnostics.is_empty());
    assert!(!report.native_qualification);
    assert_eq!(file_tree(fixture.root.path()), before);
}

#[test]
fn request_json_is_closed_bounded_and_requires_explicit_identity() {
    let fixture = Fixture::new();
    let bytes = serde_json::to_vec(&fixture.request).unwrap();
    let original = String::from_utf8(bytes.clone()).unwrap();
    let duplicate = format!("{{\"schema\":\"aniflow.toolchain.audio-inspection-registration/v1\",{}", &original[1..]);
    assert!(AudioInspectionRegistrationRequest::from_json_slice(duplicate.as_bytes()).is_err());
    let mut oversized = bytes;
    oversized.resize(1_048_577, b' ');
    assert!(AudioInspectionRegistrationRequest::from_json_slice(&oversized).is_err());
    for case in ["execute", "install", "download", "arguments", "missing_pin", "unknown_version", "source_id", "timeout"] {
        let mut request = fixture.request.clone();
        match case {
            "execute" | "install" | "download" => request[case] = json!(true),
            "arguments" => request["adapter"]["arguments"] = json!(["--execute"]),
            "missing_pin" => { request["adapter"].as_object_mut().unwrap().remove("expected_sha256"); },
            "unknown_version" => request["schema"] = json!("aniflow.toolchain.audio-inspection-registration/v2"),
            "source_id" => request["source"]["id"] = json!("different_source"),
            "timeout" => request["tool_timeout_milliseconds"] = json!(120001),
            _ => unreachable!(),
        }
        assert!(AudioInspectionRegistrationRequest::from_json_slice(&serde_json::to_vec(&request).unwrap()).is_err(), "{case}");
    }
}

#[test]
fn cli_returns_reviewable_json_and_retains_dependency_refusal_without_writing() {
    let fixture = Fixture::new();
    let configuration = fixture.root.path().join("prepare-registration.json");
    fs::write(&configuration, serde_json::to_vec(&fixture.request).unwrap()).unwrap();
    let invoke = || Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "toolchain", "prepare-registration", "--configuration"])
        .arg(&configuration).output().unwrap();
    let before = file_tree(fixture.root.path());
    let prepared = invoke();
    assert!(prepared.status.success(), "{}", String::from_utf8_lossy(&prepared.stderr));
    assert!(prepared.stderr.is_empty());
    let success: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(success["command"], "toolchain_prepare_registration");
    assert_eq!(success["result"]["ready"], true);
    assert_eq!(success["result"]["native_qualification"], false);
    assert_eq!(success["result"]["files"].as_array().unwrap().len(), 4);
    assert_eq!(file_tree(fixture.root.path()), before);

    fs::write(&fixture.adapter, b"changed synthetic provider").unwrap();
    let before_refusal = file_tree(fixture.root.path());
    let refused = invoke();
    assert!(!refused.status.success());
    assert!(refused.stdout.is_empty());
    let failure: Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(failure["command"], "toolchain_prepare_registration");
    assert_eq!(failure["error"]["category"], "dependency");
    assert_eq!(failure["result"]["ready"], false);
    assert_eq!(failure["result"]["native_qualification"], false);
    assert_eq!(failure["result"]["files"], json!([]));
    assert!(!failure["result"]["diagnostics"].as_array().unwrap().is_empty());
    assert_eq!(file_tree(fixture.root.path()), before_refusal);
}
