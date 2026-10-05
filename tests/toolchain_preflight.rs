//! Synthetic preflight guards. Authored only; all execution is deferred #64.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use aniflow::audio_analysis::AudioArtifactReference;
use aniflow::audio_inspection::{
    AudioInspectionConfiguration, AudioInspectionProviderConfiguration, AudioToolPin,
};
use aniflow::cache_v3::CachePolicy;
use aniflow::toolchain::{
    ToolchainPreflightConfiguration, ToolchainPreflightReport, preflight_toolchain,
};
use aniflow::{
    CancellationToken, ComponentIdentity, ComponentInventory, HostResources, PipelineInputBinding,
    PipelinePlanningContext, PipelineV3Configuration, PipelineV3Plan, PipelineV3ResumeRequest,
    PipelineV3RunManifest, PipelineV3RunProgress, PipelineV3RunRequest, PipelineV3StageState,
    PipelineV3Workspace, ProviderManifest, ProviderRegistration, ProviderRegistry, SideEffect,
    StageRunRecord, append_run_manifest, bind_toolchain_preflight,
    resolve_pipeline_v3, resume_v3, run_v3_with_progress_and_cancellation,
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

struct Fixture {
    root: tempfile::TempDir,
    source: PathBuf,
    source_identity: AudioArtifactReference,
    provider: PathBuf,
    settings: AudioInspectionConfiguration,
    preflight: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap();
        // These executable bytes are only hashed. Every test that reaches a
        // run/resume API must refuse before any provider or tool can launch.
        let executable = |name: &str| {
            let path = directory.join(format!("{name} ü 'literal' $(touch SENTINEL); tool"));
            let bytes = format!("#!/bin/sh\n# synthetic {name}\n: > \"$0.launched\"\nexit 97\n");
            fs::write(&path, &bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            AudioToolPin { executable: path, version: "6.1.1".to_owned(), sha256: digest(bytes.as_bytes()) }
        };
        let settings = AudioInspectionConfiguration {
            schema: "aniflow.audio-inspection.configuration/v1".to_owned(),
            ffmpeg: executable("ffmpeg"), ffprobe: executable("ffprobe"),
            tool_timeout_milliseconds: 2000, maximum_tool_output_bytes: 4096,
        };
        let provider = executable("typed-provider").executable;
        let source = directory.join("synthetic source.wav");
        let bytes = synthetic_wave();
        fs::write(&source, &bytes).unwrap();
        let source_identity = AudioArtifactReference {
            id: "source_audio".to_owned(), sha256: digest(&bytes), byte_size: bytes.len() as u64,
        };
        let dependencies = ["local-ffmpeg", "local-ffprobe"].map(|id| json!({
            "id":id, "kind":"tool", "optional":false, "version_requirement":"=6.1.1",
            "required_flags":[], "required_features":[], "suggested_locators":[],
        }));
        let artifacts = [("local-ffmpeg", &settings.ffmpeg), ("local-ffprobe", &settings.ffprobe)]
            .map(|(id, pin)| json!({
                "dependency_id":id, "path":pin.executable, "expected_sha256":pin.sha256,
                "maximum_bytes":4096, "observation":{
                    "executable_sha256":pin.sha256, "version":pin.version,
                    "package_revision":"synthetic-build-only", "flags":[], "features":[],
                    "provenance":"caller supplied synthetic identity; no native query executed",
                },
            }));
        let platform = json!({"os":std::env::consts::OS, "arch":std::env::consts::ARCH});
        let preflight = json!({
            "schema":"aniflow.toolchain.preflight/v1",
            "profile":{
                "schema":"aniflow.toolchain-profile/v1", "id":"typed-audio-fixture",
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
        });
        Self { root, source, source_identity, provider, settings, preflight }
    }

    fn configuration(&self) -> ToolchainPreflightConfiguration {
        configuration(&self.preflight)
    }

    fn tools(&self) -> Vec<ComponentIdentity> {
        [("ffmpeg", &self.settings.ffmpeg), ("ffprobe", &self.settings.ffprobe)]
            .into_iter().map(|(id, pin)| ComponentIdentity {
                id:id.to_owned(), version:pin.version.clone(), sha256:Some(pin.sha256.clone()),
            }).collect()
    }

    fn registry_with(&self, settings: AudioInspectionConfiguration, tools: Vec<ComponentIdentity>) -> ProviderRegistry {
        let wrapper = AudioInspectionProviderConfiguration {
            schema:"aniflow.audio-inspection.provider-configuration/v2".to_owned(),
            settings, source:self.source_identity.clone(),
        }.provider_configuration().unwrap();
        let mut manifest = ProviderManifest::from_json_slice(include_bytes!(
            "../providers/audio-inspection/manifest.json"
        )).unwrap();
        for requirement in &mut manifest.capabilities[0].requirements.tools {
            let tool = tools.iter().find(|tool| tool.id == requirement.id).unwrap();
            requirement.version_requirement = format!("={}", tool.version);
            requirement.sha256 = tool.sha256.clone();
        }
        let registration = ProviderRegistration::new(
            "audio-inspection-native", manifest, wrapper, &self.provider,
            "aniflow-audio-inspection-v2", ComponentInventory { tools, codecs:vec![], models:vec![] },
        ).unwrap();
        let mut registry = ProviderRegistry::new();
        registry.register(registration).unwrap();
        registry
    }

    fn registry(&self) -> ProviderRegistry {
        self.registry_with(self.settings.clone(), self.tools())
    }

    fn inputs(&self) -> Vec<PipelineInputBinding> {
        vec![PipelineInputBinding::new("source_audio", &self.source)]
    }

    fn plan_with(&self, registry: &ProviderRegistry) -> PipelineV3Plan {
        let pipeline = PipelineV3Configuration::from_yaml_slice(include_bytes!(
            "../providers/audio-inspection/pipeline.yml"
        )).unwrap();
        let context = PipelinePlanningContext {
            host:HostResources { cpu_threads:1, memory_mib:0, storage_mib:0,
                gpu_available:false, network_available:false },
            allowed_side_effects:vec![SideEffect::FilesystemRead, SideEffect::FilesystemWrite,
                SideEffect::EnvironmentRead, SideEffect::Subprocess], offline:true,
        };
        resolve_pipeline_v3(&pipeline, &self.inputs(), registry, &context).unwrap()
    }

    fn bound_plan(&self, registry: &ProviderRegistry) -> PipelineV3Plan {
        bind_toolchain_preflight(self.plan_with(registry), registry, &self.configuration()).unwrap()
    }

    fn cache(&self) -> CachePolicy {
        CachePolicy::new(self.root.path().join("cache-must-remain-absent"), "preflight-fixture")
    }
}

fn configuration(value: &Value) -> ToolchainPreflightConfiguration {
    ToolchainPreflightConfiguration::from_json_slice(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = walkdir::WalkDir::new(root).follow_links(false).into_iter()
        .map(|entry| entry.unwrap()).filter(|entry| entry.file_type().is_file())
        .map(|entry| (entry.path().strip_prefix(root).unwrap().to_owned(), fs::read(entry.path()).unwrap()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn assert_refused(result: aniflow::Result<ToolchainPreflightReport>) {
    if let Ok(report) = result {
        assert!(!report.ready, "unexpected ready report: {report:?}");
        assert!(!report.diagnostics.is_empty());
        assert!(!report.native_qualification);
    }
}

#[test]
fn binding_is_read_only_canonical_and_part_of_the_immutable_plan_identity() {
    let legacy = PipelineV3Plan::from_json_slice(include_bytes!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    )).unwrap();
    assert!(legacy.payload.toolchain_preflight_sha256.is_none());
    assert!(serde_json::to_value(&legacy).unwrap()["payload"].get("toolchain_preflight_sha256").is_none());
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let plan = fixture.plan_with(&registry);
    let original = plan.canonical_json_bytes().unwrap();
    assert!(serde_json::to_value(&plan).unwrap()["payload"].get("toolchain_preflight_sha256").is_none());
    let before = tree(fixture.root.path());
    let config = fixture.configuration();
    let report = preflight_toolchain(&plan, &registry, &config).unwrap();
    assert!(report.ready, "{report:?}");
    assert!(!report.native_qualification);
    assert_eq!(report.inspections.len(), 1);
    let bound = bind_toolchain_preflight(plan.clone(), &registry, &config).unwrap();
    assert_eq!(bound.payload.toolchain_preflight_sha256, Some(config.configuration_sha256().unwrap()));
    assert_ne!(bound.plan_sha256, plan.plan_sha256);
    assert_eq!(plan.canonical_json_bytes().unwrap(), original);
    assert_eq!(bind_toolchain_preflight(plan, &registry, &config).unwrap(), bound);
    assert_eq!(tree(fixture.root.path()), before);

    let mut forged = serde_json::to_value(&bound).unwrap();
    forged["payload"]["toolchain_preflight_sha256"] = json!("a".repeat(64));
    assert!(PipelineV3Plan::from_json_slice(&serde_json::to_vec(&forged).unwrap()).is_err());
    let mut changed = fixture.preflight.clone();
    changed["profile"]["id"] = json!("different-guard-identity");
    let mismatched = preflight_toolchain(&bound, &registry, &configuration(&changed)).unwrap();
    assert!(!mismatched.ready);
    assert!(!mismatched.diagnostics.is_empty());
    assert!(bind_toolchain_preflight(bound, &registry, &configuration(&changed)).is_err());
}

#[test]
fn registration_wrapper_and_actual_locked_tool_identity_must_all_agree() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let plan = fixture.plan_with(&registry);
    let mut changed = fixture.preflight.clone();
    changed["bindings"][0]["registration_id"] = json!("unselected-registration");
    assert_refused(preflight_toolchain(&plan, &registry, &configuration(&changed)));

    let mut settings = fixture.settings.clone();
    settings.maximum_tool_output_bytes = 8192;
    let changed_registry = fixture.registry_with(settings, fixture.tools());
    assert_refused(preflight_toolchain(&plan, &changed_registry, &fixture.configuration()));

    for field in ["version", "sha256"] {
        let mut tools = fixture.tools();
        if field == "version" { tools[0].version = "6.1.2".to_owned(); }
        else { tools[0].sha256 = Some("a".repeat(64)); }
        let changed_registry = fixture.registry_with(fixture.settings.clone(), tools);
        let changed_plan = fixture.plan_with(&changed_registry);
        assert_refused(preflight_toolchain(&changed_plan, &changed_registry, &fixture.configuration()));
    }
}

#[test]
fn inventory_tool_paths_pins_observations_and_current_bytes_are_rechecked() {
    for case in ["path", "pin", "observed_version", "changed_file", "missing_file"] {
        let fixture = Fixture::new();
        let registry = fixture.registry();
        let plan = if matches!(case, "changed_file" | "missing_file") {
            fixture.bound_plan(&registry)
        } else {
            fixture.plan_with(&registry)
        };
        let mut changed = fixture.preflight.clone();
        match case {
            "path" => {
                let sibling = fixture.root.path().join("same bytes at a different path");
                fs::copy(&fixture.settings.ffmpeg.executable, &sibling).unwrap();
                changed["inventory"]["artifacts"][0]["path"] = json!(sibling);
            },
            "pin" => changed["inventory"]["artifacts"][0]["expected_sha256"] = json!("a".repeat(64)),
            "observed_version" => changed["inventory"]["artifacts"][0]["observation"]["version"] = json!("6.1.2"),
            "changed_file" => fs::write(&fixture.settings.ffmpeg.executable, b"changed synthetic executable bytes").unwrap(),
            "missing_file" => fs::remove_file(&fixture.settings.ffmpeg.executable).unwrap(),
            _ => unreachable!(),
        }
        let before = tree(fixture.root.path());
        assert_refused(preflight_toolchain(&plan, &registry, &configuration(&changed)));
        assert_eq!(tree(fixture.root.path()), before, "{case}");
    }
}

#[test]
fn profile_authority_cannot_silently_expand_the_typed_adapter() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let plan = fixture.plan_with(&registry);
    for field in ["backend", "scale", "settings", "side_effects"] {
        let mut changed = fixture.preflight.clone();
        let capability = &mut changed["profile"]["capabilities"][0];
        match field {
            "backend" => capability["backend"] = json!({"name":"vulkan", "required_features":[]}),
            "scale" => capability["scale"] = json!({"native":2, "requested":2, "mode":"native"}),
            "settings" => capability["effective_settings"] = json!({"unapplied_setting":true}),
            "side_effects" => capability["side_effects"] = json!(["filesystem_read"]),
            _ => unreachable!(),
        }
        match ToolchainPreflightConfiguration::from_json_slice(&serde_json::to_vec(&changed).unwrap()) {
            Ok(configuration) => assert_refused(preflight_toolchain(&plan, &registry, &configuration)),
            Err(_) => {},
        }
    }
}

#[test]
fn guarded_run_refuses_before_missing_input_workspace_or_cache_access() {
    for case in ["omitted", "different_configuration", "changed_tool"] {
        let fixture = Fixture::new();
        let registry = fixture.registry();
        let plan = fixture.bound_plan(&registry);
        let mut config = fixture.preflight.clone();
        if case == "different_configuration" { config["profile"]["id"] = json!("different"); }
        if case == "changed_tool" { fs::write(&fixture.settings.ffmpeg.executable, b"changed tool").unwrap(); }
        fs::remove_file(&fixture.source).unwrap();
        let before = tree(fixture.root.path());
        let runs = fixture.root.path().join("runs-must-remain-absent");
        let cache = fixture.cache();
        let mut request = PipelineV3RunRequest::new(plan, fixture.inputs(), registry)
            .with_output_directory(&runs).with_cache(cache.clone());
        if case != "omitted" { request = request.with_toolchain_preflight(configuration(&config)); }
        let mut started = false;
        let error = run_v3_with_progress_and_cancellation(request, &CancellationToken::default(), |event| {
            if matches!(event, PipelineV3RunProgress::Started { .. }) { started = true; }
        }).unwrap_err();
        assert!(error.message().contains("toolchain preflight"), "{case}: {error}");
        assert!(!started);
        assert!(!runs.exists());
        assert!(!cache.root.exists());
        assert_eq!(tree(fixture.root.path()), before);
    }
}

#[test]
fn guarded_resume_requires_the_same_live_authority_before_inputs_or_checkpoint_reuse() {
    for case in ["omitted", "different_configuration", "changed_tool"] {
        let fixture = Fixture::new();
        let registry = fixture.registry();
        let plan = fixture.bound_plan(&registry);
        let workspace = PipelineV3Workspace::create_at(fixture.root.path().join("resume")).unwrap();
        workspace.publish_plan_bytes(&plan.canonical_json_bytes().unwrap()).unwrap();
        // No successful provider run is fabricated. The guard must refuse
        // before even looking for the absent manifest/checkpoint or source.
        let mut config = fixture.preflight.clone();
        if case == "different_configuration" { config["profile"]["id"] = json!("different"); }
        if case == "changed_tool" { fs::write(&fixture.settings.ffprobe.executable, b"changed tool").unwrap(); }
        fs::remove_file(&fixture.source).unwrap();
        let before = tree(fixture.root.path());
        let cache = fixture.cache();
        let mut request = PipelineV3ResumeRequest::new(workspace.root(), fixture.inputs(), registry)
            .with_cache(cache.clone());
        if case != "omitted" { request = request.with_toolchain_preflight(configuration(&config)); }
        let error = resume_v3(request).unwrap_err();
        assert!(error.message().contains("toolchain preflight"), "{case}: {error}");
        assert!(!cache.root.exists());
        assert_eq!(tree(fixture.root.path()), before);
    }
}

#[test]
fn removing_a_guard_and_rehashing_the_plan_cannot_replace_persisted_run_authority() {
    let fixture = Fixture::new();
    let registry = fixture.registry();
    let original = fixture.bound_plan(&registry);
    let mut stripped = serde_json::to_value(&original).unwrap();
    stripped["payload"].as_object_mut().unwrap().remove("toolchain_preflight_sha256");
    // serde_json::Value object keys are recursively sorted in this crate; this
    // deliberately creates a valid new plan, rather than a bad-digest decoy.
    stripped["plan_sha256"] = json!(digest(&serde_json::to_vec(&stripped["payload"]).unwrap()));
    let stripped = PipelineV3Plan::from_json_slice(&serde_json::to_vec(&stripped).unwrap()).unwrap();
    assert!(stripped.payload.toolchain_preflight_sha256.is_none());
    assert_ne!(stripped.plan_sha256, original.plan_sha256);
    let workspace = PipelineV3Workspace::create_at(fixture.root.path().join("guard-stripped")).unwrap();
    workspace.publish_plan_bytes(&stripped.canonical_json_bytes().unwrap()).unwrap();
    let stages = original.payload.stages.iter().map(|stage| StageRunRecord {
        rerun_required:false, excluded_checkpoints:vec![], stage_id:stage.id.clone(),
        state:PipelineV3StageState::Pending, checkpoint:None, compatibility:None, message:None,
    }).collect();
    let manifest = PipelineV3RunManifest::new(workspace.run_id(), original.plan_sha256, stages).unwrap();
    append_run_manifest(&workspace, &manifest).unwrap();
    fs::remove_file(&fixture.source).unwrap();
    let before = tree(fixture.root.path());
    let error = resume_v3(PipelineV3ResumeRequest::new(workspace.root(), fixture.inputs(), registry)).unwrap_err();
    assert_eq!(error.category(), aniflow::ErrorCategory::State);
    assert!(error.message().contains("manifest references a different immutable plan"), "{error}");
    assert_eq!(tree(fixture.root.path()), before);
}

#[test]
fn preflight_json_is_closed_bounded_and_rejects_ambiguous_bindings() {
    let fixture = Fixture::new();
    let bytes = serde_json::to_vec(&fixture.preflight).unwrap();
    let original = String::from_utf8(bytes.clone()).unwrap();
    let duplicate = format!("{{\"schema\":\"aniflow.toolchain.preflight/v1\",{}", &original[1..]);
    assert!(ToolchainPreflightConfiguration::from_json_slice(duplicate.as_bytes()).is_err());
    let mut too_large = bytes;
    too_large.resize(1_048_577, b' ');
    assert!(ToolchainPreflightConfiguration::from_json_slice(&too_large).is_err());
    for case in ["unknown_root", "unknown_binding", "duplicate_stage", "empty_capabilities", "same_dependency"] {
        let mut changed = fixture.preflight.clone();
        match case {
            "unknown_root" => changed["execute"] = json!(true),
            "unknown_binding" => changed["bindings"][0]["arguments"] = json!(["-i", "media.wav"]),
            "duplicate_stage" => {
                let duplicate = changed["bindings"][0].clone();
                changed["bindings"].as_array_mut().unwrap().push(duplicate);
            },
            "empty_capabilities" => changed["bindings"][0]["capability_ids"] = json!([]),
            "same_dependency" => changed["bindings"][0]["ffprobe_dependency_id"] = json!("local-ffmpeg"),
            _ => unreachable!(),
        }
        assert!(ToolchainPreflightConfiguration::from_json_slice(
            &serde_json::to_vec(&changed).unwrap()
        ).is_err(), "{case}");
    }
}
