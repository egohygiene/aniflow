//! Synthetic media setup coverage, authored but unrun under #64.
//! Fixture executables only expose an accidental-launch marker; no real media,
//! model, native implementation or hardware qualification is represented.
#![cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
))]

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use aniflow::frame_repair::allenk::AllenkRepairConfiguration;
use aniflow::toolchain::{MediaSetupPreparation, MediaSetupRequest, prepare_media_setup};
use aniflow::upscayl_recovery::UpscaylRecoveryConfiguration;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const ALLENK_REVISION: &str = "7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e";
const UPSCAYL_REVISION: &str = "0beb39028a0ddd83250e845b4c3333c0675e3b97";
const FFMPEG_FLAGS: &[&str] = &["-version", "-hide_banner", "-nostdin", "-v", "-xerror", "-err_detect",
    "-threads", "-protocol_whitelist", "-f", "-pattern_type", "-c:v", "-i", "-map", "-an", "-sn", "-dn",
    "-pix_fmt", "-format_version", "-hash"];
const FFMPEG_FEATURES: &[&str] = &["decoder:png", "encoder:rawvideo", "demuxer:image2", "muxer:framehash",
    "protocol:file", "protocol:pipe", "pixel_format:rgb24", "hash:sha256"];
const ALLENK_FLAGS: &[&str] = &["--version", "--input", "--output", "--remove", "--no-banner", "--threshold",
    "--no-legacy", "--legacy"];
const UPSCAYL_FLAGS: &[&str] = &["-i", "-o", "-m", "-n", "-z", "-s", "-t", "-g", "-j", "-f", "-x"];

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn file_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = walkdir::WalkDir::new(root).follow_links(false).into_iter()
        .map(|entry| entry.unwrap()).filter(|entry| entry.file_type().is_file())
        .map(|entry| (entry.path().strip_prefix(root).unwrap().to_owned(), fs::read(entry.path()).unwrap()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn import_options() -> Value {
    json!({"tool_timeout_milliseconds":2000, "maximum_source_bytes":1048576,
        "maximum_frame_bytes":4096, "maximum_total_frame_bytes":1048576, "maximum_pixels":4096})
}

struct Fixture {
    root: tempfile::TempDir,
    request: Value,
}

impl Fixture {
    fn new(upscale: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap();
        let executable = |name: &str| {
            let path = directory.join(format!("{name} ü 'literal' $(touch SENTINEL); tool"));
            let bytes = format!("#!/bin/sh\n# synthetic {name}\n: > \"$0.launched\"\nexit 97\n");
            fs::write(&path, &bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            (path, digest(bytes.as_bytes()))
        };
        let tool_artifact = |id: &str, version: &str, revision: &str, flags: &[&str], features: &[&str]| {
            let (path, sha256) = executable(id);
            json!({"dependency_id":id, "path":path, "expected_sha256":sha256, "maximum_bytes":4096,
                "observation":{"executable_sha256":sha256, "version":version, "package_revision":revision,
                    "flags":flags, "features":features, "provenance":"supplied synthetic fixture; never probed"}})
        };
        let model_artifact = |id: &str, extension: &str| {
            let path = directory.join(format!("model ü 'literal' $(touch SENTINEL).{extension}"));
            let bytes = format!("synthetic {id}; not native model data");
            fs::write(&path, &bytes).unwrap();
            let sha256 = digest(bytes.as_bytes());
            json!({"dependency_id":id, "path":path, "expected_sha256":sha256, "maximum_bytes":4096,
                "observation":{"executable_sha256":sha256, "package_revision":"synthetic-model-pair-v1",
                    "native_scale":4, "flags":[], "features":[], "provenance":"supplied synthetic model declaration"}})
        };
        let dependencies = vec![
            json!({"id":"decoder", "kind":"tool", "optional":false, "version_requirement":"=6.1.1",
                "required_flags":FFMPEG_FLAGS, "required_features":FFMPEG_FEATURES, "suggested_locators":[]}),
            json!({"id":"repair", "kind":"tool", "optional":true, "version_requirement":"=0.3.2",
                "package_revision":ALLENK_REVISION, "required_flags":ALLENK_FLAGS,
                "required_features":["visible-image-repair"], "suggested_locators":[]}),
            json!({"id":"upscale", "kind":"tool", "optional":true, "package_revision":UPSCAYL_REVISION,
                "required_flags":UPSCAYL_FLAGS, "required_features":["model-pair-loading","vulkan"], "suggested_locators":[]}),
            json!({"id":"param", "kind":"model", "optional":true, "native_scale":4,
                "required_flags":[], "required_features":[], "suggested_locators":[]}),
            json!({"id":"weights", "kind":"model", "optional":true, "native_scale":4,
                "required_flags":[], "required_features":[], "suggested_locators":[]}),
        ];
        let platform = json!({"os":std::env::consts::OS, "arch":std::env::consts::ARCH});
        let effects = json!(["filesystem_read","filesystem_write","environment_read","subprocess"]);
        let capabilities = vec![
            json!({"id":"core", "optional":false, "dependency_ids":["decoder"], "platforms":[platform.clone()],
                "effective_settings":{}, "side_effects":["filesystem_read"]}),
            json!({"id":"selected-repair", "optional":true, "dependency_ids":["decoder","repair"],
                "platforms":[platform.clone()], "effective_settings":{"import":import_options(),
                    "watermark_profile":"current_v2", "detection_threshold_percent":25, "maximum_working_bytes":1048576},
                "side_effects":effects}),
            json!({"id":"selected-upscale", "optional":true, "dependency_ids":["decoder","upscale","param","weights"],
                "platforms":[platform.clone()], "backend":{"name":"vulkan", "required_features":["ncnn-vulkan"]},
                "scale":{"native":4, "requested":2, "mode":"post_resize"},
                "effective_settings":{"import":import_options(), "model_name":"fixture-model-x4", "native_scale":4,
                    "output_scale":2, "tile_size":32, "gpu_id":0, "threads":"1:1:1", "tta":false,
                    "maximum_output_frame_bytes":4096, "maximum_output_pixels":65536,
                    "maximum_working_bytes":1048576, "minimum_free_bytes":0},
                "side_effects":["filesystem_read","filesystem_write","environment_read","subprocess","gpu"]}),
        ];
        let selection = if upscale {
            json!({"kind":"upscayl_recovery", "ffmpeg_dependency_id":"decoder", "processor_dependency_id":"upscale",
                "param_dependency_id":"param", "weights_dependency_id":"weights"})
        } else {
            json!({"kind":"allenk_image_repair", "ffmpeg_dependency_id":"decoder", "processor_dependency_id":"repair"})
        };
        let request = json!({"schema":"aniflow.toolchain.media-setup/v1",
            "profile":{"schema":"aniflow.toolchain-profile/v1", "id":"synthetic-media-setup",
                "default_capabilities":["core"], "dependencies":dependencies, "capabilities":capabilities},
            "inventory":{"schema":"aniflow.toolchain-inventory/v1", "platform":platform,
                "artifacts":[
                    tool_artifact("decoder","6.1.1","synthetic-ffmpeg",FFMPEG_FLAGS,FFMPEG_FEATURES),
                    tool_artifact("repair","0.3.2",ALLENK_REVISION,ALLENK_FLAGS,&["visible-image-repair"]),
                    tool_artifact("upscale","0.0.1",UPSCAYL_REVISION,UPSCAYL_FLAGS,&["model-pair-loading","vulkan"]),
                    model_artifact("param","param"), model_artifact("weights","bin")],
                "hardware":[{"name":"vulkan", "available":true, "features":["ncnn-vulkan"],
                    "provenance":"synthetic supplied backend; no device observed"}]},
            "capability_id":if upscale {"selected-upscale"} else {"selected-repair"}, "selection":selection});
        Self {root, request}
    }

    fn options(&mut self) -> &mut Value {
        let capability = self.request["capability_id"].as_str().unwrap().to_owned();
        &mut self.request["profile"]["capabilities"].as_array_mut().unwrap().iter_mut()
            .find(|value| value["id"] == capability).unwrap()["effective_settings"]
    }

    fn artifact(&mut self, id: &str) -> &mut Value {
        self.request["inventory"]["artifacts"].as_array_mut().unwrap().iter_mut()
            .find(|value| value["dependency_id"] == id).unwrap()
    }

    fn dependency(&mut self, id: &str) -> &mut Value {
        self.request["profile"]["dependencies"].as_array_mut().unwrap().iter_mut()
            .find(|value| value["id"] == id).unwrap()
    }

    fn path(&self, id: &str) -> PathBuf {
        let artifact = self.request["inventory"]["artifacts"].as_array().unwrap().iter()
            .find(|value| value["dependency_id"] == id).unwrap();
        PathBuf::from(artifact["path"].as_str().unwrap())
    }

    fn configuration(&self) -> MediaSetupRequest {
        MediaSetupRequest::from_json_slice(&serde_json::to_vec(&self.request).unwrap()).unwrap()
    }

    fn prepare(&self) -> MediaSetupPreparation { prepare_media_setup(&self.configuration()).unwrap() }

    fn assert_unready(&self) -> MediaSetupPreparation {
        let result = self.prepare();
        assert!(!result.ready, "unexpected ready preparation: {result:?}");
        assert!(!result.native_qualification);
        assert_eq!(result.host_runtime_evidence, "unobserved");
        assert!(result.files.is_empty());
        assert!(!result.diagnostics.is_empty());
        result
    }
}

#[test]
fn allenk_profiles_prepare_existing_closed_configuration_without_writes_or_launches() {
    for profile in ["current_v2", "legacy_v1"] {
        let mut fixture = Fixture::new(false);
        fixture.options()["watermark_profile"] = json!(profile);
        let before = file_tree(fixture.root.path());
        let result = fixture.prepare();
        assert!(result.ready, "{result:?}");
        assert!(!result.native_qualification);
        assert_eq!(result.host_runtime_evidence, "unobserved");
        assert_eq!(result.request_sha256, digest(&fixture.configuration().canonical_json_bytes().unwrap()));
        assert_eq!(result.files.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.relative_path, "configuration.json");
        assert_eq!(file.sha256, digest(&serde_json::to_vec(&file.content).unwrap()));
        let settings = AllenkRepairConfiguration::from_json_slice(&serde_json::to_vec(&file.content).unwrap()).unwrap();
        assert_eq!(settings.import.ffmpeg.executable, fixture.path("decoder"));
        assert_eq!(settings.tool.executable, fixture.path("repair"));
        assert_eq!(settings.tool.version, "0.3.2");
        assert_eq!(serde_json::to_value(settings.watermark_profile).unwrap(), profile);
        assert_eq!(settings.detection_threshold_percent, 25);
        assert_eq!(settings.import.maximum_source_bytes, 1048576);
        assert_eq!(serde_json::to_value(&result).unwrap(), serde_json::to_value(fixture.prepare()).unwrap());
        assert_eq!(file_tree(fixture.root.path()), before);
    }
}

#[test]
fn upscayl_preserves_separate_native_scale_requested_scale_and_model_pins() {
    for output_scale in [2, 3, 4] {
        let mut fixture = Fixture::new(true);
        fixture.options()["output_scale"] = json!(output_scale);
        fixture.request["profile"]["capabilities"][2]["scale"] = json!({"native":4,
            "requested":output_scale, "mode":if output_scale == 4 {"native"} else {"post_resize"}});
        let before = file_tree(fixture.root.path());
        let result = fixture.prepare();
        assert!(result.ready, "{result:?}");
        assert_eq!(result.files.len(), 1);
        let settings = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&result.files[0].content).unwrap()).unwrap();
        assert_eq!(settings.model.native_scale, 4);
        assert_eq!(settings.output_scale, output_scale);
        assert_eq!(settings.model.param.path, fixture.path("param"));
        assert_eq!(settings.model.weights.path, fixture.path("weights"));
        assert_eq!(settings.model.param.sha256, fixture.artifact("param")["expected_sha256"].as_str().unwrap());
        assert_eq!(settings.upscayl.version, "0.0.1", "version remains a supplied package declaration");
        assert_eq!(settings.threads, "1:1:1");
        assert_eq!(settings.gpu_id, 0);
        assert_eq!(file_tree(fixture.root.path()), before);
    }
}

#[test]
fn missing_changed_and_unbound_dependencies_retain_diagnostics_without_configuration() {
    for case in ["missing", "changed", "observation", "binding", "version", "revision"] {
        let mut fixture = Fixture::new(false);
        match case {
            "missing" => fs::remove_file(fixture.path("repair")).unwrap(),
            "changed" => fs::write(fixture.path("repair"), b"changed fixture").unwrap(),
            "observation" => { fixture.artifact("repair").as_object_mut().unwrap().remove("observation"); },
            "binding" => fixture.artifact("repair")["observation"]["executable_sha256"] = json!("0".repeat(64)),
            "version" => fixture.artifact("repair")["observation"]["version"] = json!("0.3.1"),
            "revision" => fixture.artifact("repair")["observation"]["package_revision"] = json!("different-source"),
            _ => unreachable!(),
        }
        let before = file_tree(fixture.root.path());
        fixture.assert_unready();
        assert_eq!(file_tree(fixture.root.path()), before);
    }
}

#[test]
fn permissive_profile_cannot_bypass_adapter_flags_features_or_source_interface() {
    for (upscale, id, field, value) in [
        (false, "decoder", "flags", json!([])),
        (false, "decoder", "features", json!([])),
        (false, "repair", "flags", json!([])),
        (false, "repair", "features", json!([])),
        (false, "repair", "version", json!("0.3.1")),
        (false, "repair", "package_revision", json!("wrong-revision")),
        (true, "upscale", "flags", json!([])),
        (true, "upscale", "features", json!([])),
        (true, "upscale", "package_revision", json!("wrong-revision")),
    ] {
        let mut fixture = Fixture::new(upscale);
        fixture.dependency(id)["required_flags"] = json!([]);
        fixture.dependency(id)["required_features"] = json!([]);
        fixture.dependency(id).as_object_mut().unwrap().remove("version_requirement");
        fixture.dependency(id).as_object_mut().unwrap().remove("package_revision");
        fixture.artifact(id)["observation"][field] = value;
        fixture.assert_unready();
    }
}

#[test]
fn both_model_files_require_bound_scale_and_matching_revision() {
    for case in ["missing-param", "scale", "revision", "unbound"] {
        let mut fixture = Fixture::new(true);
        match case {
            "missing-param" => fs::remove_file(fixture.path("param")).unwrap(),
            "scale" => fixture.artifact("weights")["observation"]["native_scale"] = json!(2),
            "revision" => fixture.artifact("weights")["observation"]["package_revision"] = json!("different-model-pair"),
            "unbound" => fixture.artifact("param")["observation"]["executable_sha256"] = json!("0".repeat(64)),
            _ => unreachable!(),
        }
        fixture.assert_unready();
    }
}

#[test]
fn missing_backend_and_host_mismatch_do_not_become_native_readiness() {
    for case in ["unobserved", "unavailable", "features", "host"] {
        let mut fixture = Fixture::new(true);
        match case {
            "unobserved" => fixture.request["inventory"]["hardware"] = json!([]),
            "unavailable" => fixture.request["inventory"]["hardware"][0]["available"] = json!(false),
            "features" => fixture.request["inventory"]["hardware"][0]["features"] = json!([]),
            "host" => {
                let platform = json!({"os":"synthetic-host", "arch":"synthetic-arch"});
                fixture.request["inventory"]["platform"] = platform.clone();
                fixture.request["profile"]["capabilities"][2]["platforms"] = json!([platform]);
            },
            _ => unreachable!(),
        }
        fixture.assert_unready();
    }
}

#[test]
fn unselected_optional_dependencies_are_not_observed_or_required() {
    let mut fixture = Fixture::new(false);
    for id in ["upscale", "param", "weights"] { fs::remove_file(fixture.path(id)).unwrap(); }
    fixture.request["inventory"]["hardware"] = json!([]);
    let result = fixture.prepare();
    assert!(result.ready, "{result:?}");
    assert_eq!(result.inspection.selected_capabilities, ["selected-repair"]);
    assert!(result.inspection.facts.iter().all(|fact| fact.dependency_id.as_deref()
        .is_none_or(|id| matches!(id, "decoder" | "repair"))));
}

#[test]
fn closed_request_rejects_duplicates_ambiguity_and_wrong_dependency_kinds() {
    let fixture = Fixture::new(false);
    let raw = serde_json::to_string(&fixture.request).unwrap();
    let duplicate = raw.replacen("\"capability_id\":", "\"capability_id\":\"other\",\"capability_id\":", 1);
    assert!(MediaSetupRequest::from_json_slice(duplicate.as_bytes()).is_err());
    for case in ["root-unknown", "selection-unknown", "same-tool", "extra-dependency", "wrong-kind", "wrong-capability"] {
        let mut fixture = Fixture::new(false);
        match case {
            "root-unknown" => fixture.request["install"] = json!(true),
            "selection-unknown" => fixture.request["selection"]["arguments"] = json!(["--force"]),
            "same-tool" => fixture.request["selection"]["processor_dependency_id"] = json!("decoder"),
            "extra-dependency" => fixture.request["profile"]["capabilities"][1]["dependency_ids"] = json!(["decoder","repair","upscale"]),
            "wrong-kind" => {
                fixture.dependency("repair")["kind"] = json!("model");
                fixture.dependency("repair")["required_flags"] = json!([]);
                fixture.dependency("repair")["required_features"] = json!([]);
                fixture.dependency("repair").as_object_mut().unwrap().remove("version_requirement");
            },
            "wrong-capability" => fixture.request["capability_id"] = json!("selected-upscale"),
            _ => unreachable!(),
        }
        assert!(MediaSetupRequest::from_json_slice(&serde_json::to_vec(&fixture.request).unwrap()).is_err(), "{case}");
    }
}

#[test]
fn selected_effective_settings_are_closed_explicit_and_agree_with_scale_and_effects() {
    for (upscale, case) in [(false,"missing"),(false,"unknown"),(false,"threshold"),(false,"effects"),
        (true,"scale"),(true,"backend"),(true,"threads")] {
        let mut fixture = Fixture::new(upscale);
        match case {
            "missing" => { fixture.options().as_object_mut().unwrap().remove("import"); },
            "unknown" => fixture.options()["force"] = json!(true),
            "threshold" => fixture.options()["detection_threshold_percent"] = json!(101),
            "effects" => fixture.request["profile"]["capabilities"][1]["side_effects"] = json!(["filesystem_read"]),
            "scale" => fixture.request["profile"]["capabilities"][2]["scale"]["requested"] = json!(3),
            "backend" => fixture.request["profile"]["capabilities"][2]["backend"]["name"] = json!("automatic"),
            "threads" => fixture.options()["threads"] = json!("4:4:4"),
            _ => unreachable!(),
        }
        assert!(MediaSetupRequest::from_json_slice(&serde_json::to_vec(&fixture.request).unwrap()).is_err(), "{case}");
    }
}

#[test]
fn model_names_cannot_override_the_declared_native_scale_with_x8_or_x16() {
    for name in ["fixture-x8", "fixture-8x", "fixture-x16", "fixture-16x", "fixture-x2"] {
        let mut fixture = Fixture::new(true);
        fixture.options()["model_name"] = json!(name);
        assert!(MediaSetupRequest::from_json_slice(&serde_json::to_vec(&fixture.request).unwrap()).is_err(), "{name}");
    }
}

#[test]
fn preparation_reobserves_files_and_rejects_nonexecutables_or_symlinked_tools() {
    let fixture = Fixture::new(false);
    assert!(fixture.prepare().ready);
    fs::write(fixture.path("repair"), b"different bytes after ready report").unwrap();
    fixture.assert_unready();
    let fixture = Fixture::new(false);
    fs::set_permissions(fixture.path("repair"), fs::Permissions::from_mode(0o644)).unwrap();
    fixture.assert_unready();
    let mut fixture = Fixture::new(false);
    let target = fixture.path("repair");
    let link = target.with_extension("link");
    symlink(target, &link).unwrap();
    fixture.artifact("repair")["path"] = json!(link);
    fixture.assert_unready();
    let mut fixture = Fixture::new(false);
    let target = fixture.path("repair");
    let directory_link = fixture.root.path().join("symlinked ancestor");
    symlink(fixture.root.path().canonicalize().unwrap(), &directory_link).unwrap();
    fixture.artifact("repair")["path"] = json!(directory_link.join(target.file_name().unwrap()));
    fixture.assert_unready();
    let mut fixture = Fixture::new(false);
    let decoder = fixture.path("decoder");
    let decoder_sha256 = fixture.artifact("decoder")["expected_sha256"].clone();
    fixture.artifact("repair")["path"] = json!(decoder);
    fixture.artifact("repair")["expected_sha256"] = decoder_sha256.clone();
    fixture.artifact("repair")["observation"]["executable_sha256"] = decoder_sha256;
    fixture.assert_unready();
}

#[test]
fn cli_preserves_ready_and_dependency_failure_preparations_in_machine_envelopes() {
    let fixture = Fixture::new(false);
    let request_path = fixture.root.path().join("request.json");
    fs::write(&request_path, serde_json::to_vec(&fixture.request).unwrap()).unwrap();
    let before = file_tree(fixture.root.path());
    let invoke = || Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output","json","toolchain","prepare-media","--configuration"])
        .arg(&request_path).output().unwrap();
    let output = invoke();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["command"], "toolchain_prepare_media");
    assert_eq!(envelope["status"], "success");
    assert_eq!(envelope["result"]["ready"], true);
    assert_eq!(envelope["result"]["files"].as_array().unwrap().len(), 1);
    assert_eq!(file_tree(fixture.root.path()), before);
    fs::remove_file(fixture.path("repair")).unwrap();
    let output = invoke();
    assert_eq!(output.status.code(), Some(5));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["command"], "toolchain_prepare_media");
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["category"], "dependency");
    assert_eq!(envelope["result"]["ready"], false);
    assert_eq!(envelope["result"]["files"], json!([]));
}
