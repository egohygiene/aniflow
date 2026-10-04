//! Synthetic observed-probe boundaries. Authored; execution deferred #64.

#[test]
fn probe_documents_parse_without_opening_or_launching_the_named_tool() {
    use aniflow::toolchain::{ToolchainProbeConfiguration, ToolchainProbeReport};
    use serde_json::{Value, json};

    let mut configuration: Value = serde_json::from_slice(include_bytes!(
        "../docs/contracts/examples/toolchain-probe-configuration-v1.example.json"
    )).unwrap();
    // Use a host-native absolute spelling while retaining the synthetic pin.
    // Parsing declarations must not inspect this nonexistent executable.
    configuration["tools"][0]["path"] = json!(std::env::temp_dir()
        .join("aniflow-probe-document-only-do-not-execute"));
    let parsed = ToolchainProbeConfiguration::from_json_slice(
        &serde_json::to_vec(&configuration).unwrap()
    ).unwrap();
    let mut report: Value = serde_json::from_slice(include_bytes!(
        "../docs/contracts/examples/toolchain-probe-report-v1.example.json"
    )).unwrap();
    report["configuration"] = configuration;
    report["request_sha256"] = json!(parsed.sha256().unwrap());
    ToolchainProbeReport::from_json_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
}

#[cfg(any(target_os = "macos", all(target_os = "linux", not(target_env = "uclibc"))))]
mod process_cases {

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::CancellationToken;
use aniflow::toolchain::{
    ToolchainInventory, ToolchainProbeConfiguration, ToolchainProbeReport, ToolchainProfile,
    inspect_profile, probe_tools,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

// Probe targets are authored synthetic source interpreted by an explicitly
// resolved trusted Python 3.10+ prerequisite. The fixture rejects every argv
// outside bounded diagnostics; it never opens media, resolves a model, contacts
// a network, installs or registers a provider.
const TOOL_BODY: &str = r#"import json, os, pathlib, sys, time
if sys.version_info < (3, 10):
    sys.exit('ANIFLOW_TEST_PYTHON must select a trusted Python 3.10+ interpreter')
root = pathlib.Path(__file__).resolve().parent
args = sys.argv[1:]
profile = (root / 'profile').read_text()
mode = (root / 'mode').read_text()
with (root / 'invocations.jsonl').open('a') as log:
    log.write(json.dumps({'args': args, 'home': os.environ.get('HOME'),
        'tmpdir': os.environ.get('TMPDIR'), 'lang': os.environ.get('LANG'),
        'lc_all': os.environ.get('LC_ALL')}) + '\n')
(root / 'started').write_text('started')
allowed = [['-version'], ['-hide_banner', '-h', 'full']]
if profile == 'ffmpeg':
    allowed += [['-hide_banner', flag] for flag in ['-encoders', '-decoders', '-filters', '-hwaccels']]
if args not in allowed:
    (root / 'unexpected-operation').write_text(json.dumps(args))
    sys.exit(99)
if mode == 'fast_exit':
    os._exit(0)
if mode in ['closed_pipes', 'closed_pipes_timeout'] and args == ['-version']:
    os.write(1, (profile + ' version 6.1.1 Copyright (c) synthetic fixture\n').encode())
    os.close(1)
    os.close(2)
    (root / 'pipes-closed').write_text('leader is still alive')
    time.sleep(120 if mode == 'closed_pipes_timeout' else 0.25)
    (root / 'closed-pipes-leader-finished').write_text('leader finished its work before exit')
    os._exit(0)
if mode == 'sleep':
    time.sleep(120)
if mode == 'overflow':
    os.write(1, b'x' * 131072)
    os.write(2, b'y' * 131072)
    time.sleep(120)
if mode == 'inherited_pipe':
    if os.fork() == 0:
        time.sleep(2)
        (root / 'descendant-survived').write_text('should have been terminated')
        os._exit(0)
    (root / 'descendant-created').write_text('inherited both capture pipes')
    print(profile + ' version 6.1.1 Copyright (c) synthetic fixture', flush=True)
    sys.exit(0)
if mode == 'no_evidence':
    sys.exit(0)
if args == ['-version']:
    banner = profile
    token = '6.1.1'
    if mode == 'wrong_banner':
        banner = 'ffprobe' if profile == 'ffmpeg' else 'ffmpeg'
    if mode == 'vendor_version':
        token = 'N-12345-gabcdef-vendor'
    if mode == 'short_version':
        token = '6.1'
    if mode == 'malformed_version':
        print('Usage: ' + profile + ' [options]')
    else:
        print(banner + ' version ' + token + ' Copyright (c) synthetic fixture', flush=True)
    if mode == 'mutate_after_version':
        with pathlib.Path(__file__).open('a') as executable:
            executable.write('\n# mutated after launch\n')
    if mode == 'nonzero':
        print('synthetic failure with plausible stdout', file=sys.stderr)
        sys.exit(17)
elif args == ['-hide_banner', '-encoders']:
    if mode == 'malformed_features':
        print('Encoders:\n definitely-not-a-table libx264')
    else:
        print('Encoders:\n V..... libx264 synthetic H.264 encoder\n A..... pcm_s16le synthetic PCM encoder')
elif args == ['-hide_banner', '-decoders']:
    print('Decoders:\n A..... pcm_s16le synthetic PCM decoder')
elif args == ['-hide_banner', '-filters']:
    print('Filters:\n ... scale V->V synthetic scale filter\n ... null V->V synthetic null filter')
elif args == ['-hide_banner', '-hwaccels']:
    print('Hardware acceleration methods:\nvulkan')
else:
    if mode == 'malformed_help':
        print('Unknown option help; no option inventory is available')
    elif mode == 'unknown_flag':
        print('Global options (affect whole program instead of just one file):\n-unrecognized_flag value synthetic unknown flag')
    elif profile == 'ffmpeg':
        print('Hyper fast Audio and Video encoder\nusage: ffmpeg [options]\nGlobal options (affect whole program instead of just one file):\n-y                  overwrite\n\nPer-file options (input and output):\n-codec[:<stream_spec>] <codec>  codec\n-map <map>         map\n-f <fmt>           format')
    else:
        print('Simple multimedia streams analyzer\nusage: ffprobe [options]\nMain options:\n-show_entries <entry_list>  select entries\n-show_format          show format\n-show_streams         show streams\n-of <format>          output format\n-select_streams <stream_specifier> select streams')
"#;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture_source() -> String {
    const GUIDANCE: &str = "set ANIFLOW_TEST_PYTHON to an absolute path to a trusted Python 3.10+ interpreter; no discovery, installation or silent skip is performed";
    let selected = std::env::var_os("ANIFLOW_TEST_PYTHON")
        .map_or_else(|| PathBuf::from("/usr/bin/python3"), PathBuf::from);
    assert!(selected.is_absolute(), "{GUIDANCE}: path must be absolute");
    let resolved = selected.canonicalize()
        .unwrap_or_else(|error| panic!("{GUIDANCE}: cannot resolve {}: {error}", selected.display()));
    let metadata = fs::metadata(&resolved)
        .unwrap_or_else(|error| panic!("{GUIDANCE}: cannot inspect {}: {error}", resolved.display()));
    assert!(metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        "{GUIDANCE}: interpreter must be an executable regular file");
    let interpreter = resolved.to_str().unwrap_or_else(|| panic!("{GUIDANCE}: path must be UTF-8"));
    assert!(!interpreter.chars().any(|character| character.is_whitespace() || character.is_control()),
        "{GUIDANCE}: resolved shebang path cannot contain whitespace or controls");
    let shebang = format!("#!{interpreter}\n");
    assert!(shebang.len() <= 127, "{GUIDANCE}: complete shebang line must fit 127 bytes");
    format!("{shebang}{TOOL_BODY}")
}

struct Fixture {
    root: tempfile::TempDir,
    tool: PathBuf,
    tool_sha256: String,
    configuration: Value,
}

impl Fixture {
    fn new(profile: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let tool = root.path().canonicalize().unwrap()
            .join("native ü 'quoted' $(touch SENTINEL); tool");
        let source = fixture_source();
        let tool_sha256 = digest(source.as_bytes());
        fs::write(&tool, source).unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(root.path().join("profile"), profile).unwrap();
        fs::write(root.path().join("mode"), "success").unwrap();
        let mut configuration: Value = serde_json::from_slice(include_bytes!(
            "../docs/contracts/examples/toolchain-probe-configuration-v1.example.json"
        )).unwrap();
        configuration["tools"][0]["dependency_id"] = json!(profile);
        configuration["tools"][0]["profile"] = json!(profile);
        configuration["tools"][0]["path"] = json!(tool);
        configuration["tools"][0]["expected_sha256"] = json!(tool_sha256);
        Self { root, tool, tool_sha256, configuration }
    }

    fn mode(&self, mode: &str) {
        fs::write(self.root.path().join("mode"), mode).unwrap();
    }

    fn request(&self) -> ToolchainProbeConfiguration {
        ToolchainProbeConfiguration::from_json_slice(
            &serde_json::to_vec(&self.configuration).unwrap()
        ).unwrap()
    }

    fn probe(&self) -> Value {
        self.probe_with(&CancellationToken::default())
    }

    fn probe_with(&self, token: &CancellationToken) -> Value {
        serde_json::to_value(probe_tools(&self.request(), token).unwrap()).unwrap()
    }

    fn invocations(&self) -> Vec<Value> {
        let path = self.root.path().join("invocations.jsonl");
        if !path.exists() {
            return Vec::new();
        }
        fs::read_to_string(path).unwrap().lines()
            .map(|line| serde_json::from_str(line).unwrap()).collect()
    }
}

fn command<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["tools"][0]["commands"].as_array().unwrap().iter()
        .find(|command| command["command_id"] == id).unwrap()
}

fn no_observation(report: &Value) {
    assert_eq!(report["complete"], false, "{report}");
    assert_eq!(report["native_qualification"], false);
    assert!(report["tools"][0].get("observation").is_none(), "{report}");
}

#[test]
fn explicit_literal_executables_receive_only_fixed_diagnostic_arguments() {
    for profile in ["ffmpeg", "ffprobe"] {
        let fixture = Fixture::new(profile);
        let before = fs::read(&fixture.tool).unwrap();
        let report = fixture.probe();
        assert_eq!(report["complete"], true, "{report}");
        assert_eq!(report["native_qualification"], false);
        assert_eq!(report["tools"][0]["status"], "installed");
        assert_eq!(report["tools"][0]["before_sha256"], digest(&before));
        assert_eq!(report["tools"][0]["after_sha256"], digest(&before));
        let expected = if profile == "ffmpeg" {
            json!([["-version"], ["-hide_banner", "-encoders"],
                ["-hide_banner", "-decoders"], ["-hide_banner", "-filters"],
                ["-hide_banner", "-hwaccels"], ["-hide_banner", "-h", "full"]])
        } else {
            json!([["-version"], ["-hide_banner", "-h", "full"]])
        };
        let invocations = fixture.invocations();
        assert_eq!(json!(invocations.iter().map(|row| row["args"].clone()).collect::<Vec<_>>()), expected);
        assert_eq!(json!(report["tools"][0]["commands"].as_array().unwrap().iter()
            .map(|row| row["arguments"].clone()).collect::<Vec<_>>()), expected);
        for invocation in invocations {
            assert_eq!(invocation["lang"], "C");
            assert_eq!(invocation["lc_all"], "C");
            assert!(invocation["home"].as_str().is_some_and(|path| path.starts_with('/')));
            assert!(invocation["tmpdir"].as_str().is_some_and(|path| path.starts_with('/')));
        }
        assert_eq!(fs::read(&fixture.tool).unwrap(), before);
        assert!(!fixture.root.path().join("SENTINEL").exists());
        assert!(!fixture.root.path().join("unexpected-operation").exists());
        assert_eq!(report["tools"][0]["observation"]["executable_sha256"], digest(&before));
    }
}

#[test]
fn executable_changes_before_and_after_launch_never_publish_stale_observations() {
    let fixture = Fixture::new("ffmpeg");
    let mut changed = fs::read(&fixture.tool).unwrap();
    changed.extend_from_slice(b"\n# changed before probing\n");
    fs::write(&fixture.tool, changed).unwrap();
    let report = fixture.probe();
    no_observation(&report);
    assert_eq!(report["tools"][0]["status"], "incompatible");
    assert!(fixture.invocations().is_empty());
    assert!(report["tools"][0]["commands"].as_array().unwrap().is_empty());

    let fixture = Fixture::new("ffmpeg");
    fixture.mode("mutate_after_version");
    let report = fixture.probe();
    no_observation(&report);
    assert_eq!(report["tools"][0]["status"], "incompatible");
    assert_eq!(fixture.invocations().len(), 1);
    assert_eq!(command(&report, "version")["outcome"], "succeeded");
    assert_ne!(report["tools"][0]["before_sha256"], report["tools"][0]["after_sha256"]);
}

#[test]
fn configuration_rejects_caller_commands_unknown_profiles_and_duplicate_references() {
    let fixture = Fixture::new("ffmpeg");
    for case in ["argv", "unknown_profile", "duplicate", "relative", "reversed_timeout"] {
        let mut configuration = fixture.configuration.clone();
        match case {
            "argv" => configuration["tools"][0]["arguments"] = json!(["-i", "media.wav"]),
            "unknown_profile" => configuration["tools"][0]["profile"] = json!("arbitrary"),
            "duplicate" => {
                let duplicate = configuration["tools"][0].clone();
                configuration["tools"].as_array_mut().unwrap().push(duplicate);
            },
            "relative" => configuration["tools"][0]["path"] = json!("ffmpeg"),
            "reversed_timeout" => configuration["limits"]["total_timeout_milliseconds"] = json!(1),
            _ => unreachable!(),
        }
        assert!(ToolchainProbeConfiguration::from_json_slice(
            &serde_json::to_vec(&configuration).unwrap()
        ).is_err(), "{case}");
    }
    assert!(fixture.invocations().is_empty());
}

#[test]
fn repeated_reports_preserve_input_identity_and_capture_bytes_separately_from_elapsed_time() {
    let fixture = Fixture::new("ffprobe");
    let stable = |mut report: Value| {
        for tool in report["tools"].as_array_mut().unwrap() {
            for command in tool["commands"].as_array_mut().unwrap() {
                command.as_object_mut().unwrap().remove("duration_milliseconds");
            }
        }
        report
    };
    let first = fixture.probe();
    let second = fixture.probe();
    let parsed = ToolchainProbeReport::from_json_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
    let canonical = parsed.canonical_json_bytes().unwrap();
    assert_eq!(ToolchainProbeReport::from_json_slice(&canonical).unwrap()
        .canonical_json_bytes().unwrap(), canonical);
    assert_eq!(stable(first), stable(second));
}

#[test]
fn retained_report_rejects_stale_identity_and_claims_not_supported_by_capture_bytes() {
    ToolchainProbeConfiguration::from_json_slice(include_bytes!(
        "../docs/contracts/examples/toolchain-probe-configuration-v1.example.json"
    )).unwrap();
    ToolchainProbeReport::from_json_slice(include_bytes!(
        "../docs/contracts/examples/toolchain-probe-report-v1.example.json"
    )).unwrap();
    let fixture = Fixture::new("ffprobe");
    let report = fixture.probe();
    ToolchainProbeReport::from_json_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    for case in ["capture", "pin", "request", "version", "flags", "native"] {
        let mut altered = report.clone();
        match case {
            "capture" => altered["tools"][0]["commands"][0]["stdout"]["retained_hex"] = json!("00"),
            "pin" => altered["tools"][0]["after_sha256"] = json!("a".repeat(64)),
            "request" => altered["request_sha256"] = json!("a".repeat(64)),
            "version" => altered["tools"][0]["observation"]["version"] = json!("99.0.0"),
            "flags" => altered["tools"][0]["observation"]["flags"] = json!(["-arbitrary_unobserved_flag"]),
            "native" => altered["native_qualification"] = json!(true),
            _ => unreachable!(),
        }
        assert!(ToolchainProbeReport::from_json_slice(
            &serde_json::to_vec(&altered).unwrap()
        ).is_err(), "{case}");
    }
}

#[test]
fn successful_exit_without_matching_complete_evidence_is_not_installation() {
    for mode in ["wrong_banner", "malformed_version", "malformed_features", "malformed_help", "no_evidence"] {
        let fixture = Fixture::new("ffmpeg");
        fixture.mode(mode);
        let report = fixture.probe();
        no_observation(&report);
        assert_ne!(report["tools"][0]["status"], "installed", "{mode}: {report}");
        assert!(report["tools"][0]["commands"].as_array().unwrap().iter()
            .all(|command| command["outcome"] == "succeeded"));
        assert!(!report["tools"][0]["diagnostics"].as_array().unwrap().is_empty());
    }
}

#[test]
fn numeric_versions_are_explicitly_normalized_and_vendor_tokens_are_not_invented() {
    let fixture = Fixture::new("ffprobe");
    fixture.mode("short_version");
    let report = fixture.probe();
    assert_eq!(report["tools"][0]["version"], json!({
        "raw_token":"6.1", "normalized_semver":"6.1.0", "normalization":"append_patch_zero"
    }));
    assert_eq!(report["tools"][0]["observation"]["version"], "6.1.0");

    let fixture = Fixture::new("ffprobe");
    fixture.mode("vendor_version");
    let report = fixture.probe();
    assert_eq!(report["tools"][0]["version"]["raw_token"], "N-12345-gabcdef-vendor");
    assert!(report["tools"][0]["version"].get("normalized_semver").is_none());
    assert!(report["tools"][0]["observation"].get("version").is_none());
    assert_eq!(report["native_qualification"], false);
}

#[test]
fn process_failure_timeout_and_capture_overflow_retain_separate_failed_facts() {
    for (mode, outcome) in [("nonzero", "failed"), ("sleep", "timed_out"), ("overflow", "output_limit")] {
        let mut fixture = Fixture::new("ffmpeg");
        fixture.mode(mode);
        fixture.configuration["limits"]["timeout_milliseconds"] = json!(500);
        fixture.configuration["limits"]["maximum_stdout_bytes"] = json!(1024);
        fixture.configuration["limits"]["maximum_stderr_bytes"] = json!(1024);
        let start = Instant::now();
        let report = fixture.probe();
        assert!(start.elapsed() < Duration::from_secs(10), "{mode}");
        no_observation(&report);
        let result = command(&report, "version");
        assert_eq!(result["outcome"], outcome, "{report}");
        assert_eq!(fixture.invocations().len(), 1);
        for stream in ["stdout", "stderr"] {
            assert!(result[stream]["retained_hex"].as_str().unwrap().len() <= 2048);
        }
        if mode == "nonzero" {
            assert_eq!(result["exit_code"], 17);
            assert!(result["stdout"]["total_bytes"].as_u64().unwrap() > 0);
            assert!(result["stderr"]["total_bytes"].as_u64().unwrap() > 0);
        }
        if mode == "overflow" {
            assert!(result["stdout"]["truncated"] == true || result["stderr"]["truncated"] == true);
        }
    }
}

#[test]
fn cancellation_prevents_launch_or_terminates_the_active_probe_without_observation() {
    let fixture = Fixture::new("ffmpeg");
    let token = CancellationToken::default();
    token.cancel();
    no_observation(&fixture.probe_with(&token));
    assert!(fixture.invocations().is_empty());

    let fixture = Fixture::new("ffmpeg");
    fixture.mode("sleep");
    let token = CancellationToken::default();
    let cancellation = token.clone();
    let started = fixture.root.path().join("started");
    let canceller = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !started.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        cancellation.cancel();
    });
    let report = fixture.probe_with(&token);
    canceller.join().unwrap();
    no_observation(&report);
    assert_eq!(command(&report, "version")["outcome"], "cancelled");
    assert_eq!(fixture.invocations().len(), 1);
}

#[test]
fn rapid_leader_exits_preserve_successful_process_facts_without_inventing_evidence() {
    let fixture = Fixture::new("ffprobe");
    fixture.mode("fast_exit");
    // This public-API case exercises promptly exiting children. A private
    // observer test must force exit before observer registration; scheduling
    // here deliberately makes no claim to force that ordering.
    for _ in 0..8 {
        let report = fixture.probe();
        no_observation(&report);
        let commands = report["tools"][0]["commands"].as_array().unwrap();
        assert_eq!(commands.len(), 2, "{report}");
        for result in commands {
            assert_eq!(result["outcome"], "succeeded", "{report}");
            assert_eq!(result["exit_code"], 0);
            assert!(result.get("signal").is_none());
            for stream in ["stdout", "stderr"] {
                assert_eq!(result[stream]["total_bytes"], 0);
                assert_eq!(result[stream]["truncated"], false);
            }
        }
    }
    assert_eq!(fixture.invocations().len(), 16);
}

#[test]
fn closed_capture_pipes_neither_kill_a_live_leader_nor_turn_it_into_a_success() {
    let fixture = Fixture::new("ffprobe");
    fixture.mode("closed_pipes");
    let report = fixture.probe();
    assert_eq!(report["complete"], true, "{report}");
    assert!(fixture.root.path().join("pipes-closed").exists());
    assert!(fixture.root.path().join("closed-pipes-leader-finished").exists(),
        "the still-running leader must finish after closing its capture pipes");
    assert_eq!(command(&report, "version")["outcome"], "succeeded");
    assert_eq!(command(&report, "version")["exit_code"], 0);
    assert!(command(&report, "version").get("signal").is_none());

    let mut fixture = Fixture::new("ffprobe");
    fixture.mode("closed_pipes_timeout");
    fixture.configuration["limits"]["timeout_milliseconds"] = json!(1000);
    let started = Instant::now();
    let report = fixture.probe();
    assert!(started.elapsed() < Duration::from_secs(10));
    no_observation(&report);
    assert!(fixture.root.path().join("pipes-closed").exists());
    assert!(!fixture.root.path().join("closed-pipes-leader-finished").exists());
    let result = command(&report, "version");
    assert_eq!(result["outcome"], "timed_out", "{report}");
    assert_eq!(result["stdout"]["truncated"], false);
    assert_eq!(result["stderr"]["truncated"], false);
    assert_eq!(fixture.invocations().len(), 1);
}

#[test]
fn exited_parent_with_inherited_capture_pipes_cannot_leave_a_running_descendant() {
    let mut fixture = Fixture::new("ffprobe");
    fixture.mode("inherited_pipe");
    fixture.configuration["limits"]["timeout_milliseconds"] = json!(1000);
    let start = Instant::now();
    let report = fixture.probe();
    assert!(start.elapsed() < Duration::from_secs(10));
    no_observation(&report);
    assert!(fixture.root.path().join("descendant-created").exists());
    let result = command(&report, "version");
    assert_eq!(result["outcome"], "timed_out");
    assert_eq!(result["exit_code"], 0, "leader exit and capture completion are separate facts");
    assert_eq!(result["stdout"]["truncated"], true);
    assert_eq!(result["stderr"]["truncated"], true);
    // A post-return marker distinguishes group cleanup from merely abandoning
    // the leader handle or closing the parent's copies of the pipes.
    thread::sleep(Duration::from_millis(2200));
    assert!(!fixture.root.path().join("descendant-survived").exists());
}

#[test]
fn compiled_backend_lists_cannot_supply_device_readiness_or_unknown_flag_support() {
    let mut fixture = Fixture::new("ffmpeg");
    fixture.configuration["tools"][0].as_object_mut().unwrap().remove("caller_package_revision");
    let report = fixture.probe();
    assert_eq!(report["complete"], true, "{report}");
    assert!(report["tools"][0]["observation"].get("package_revision").is_none());
    assert!(report["tools"][0]["observation"]["features"].as_array().unwrap()
        .contains(&json!("hwaccel:vulkan")));
    let observation = report["tools"][0]["observation"].clone();
    let profile = ToolchainProfile::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.toolchain-profile/v1", "id":"requires-device",
        "default_capabilities":["encode"],
        "dependencies":[{"id":"ffmpeg", "kind":"tool", "optional":false,
            "required_flags":["-map"], "required_features":[], "suggested_locators":[]}],
        "capabilities":[{"id":"encode", "optional":false, "dependency_ids":["ffmpeg"],
            "platforms":[{"os":"linux", "arch":"x86_64"}],
            "backend":{"name":"vulkan", "required_features":["compute"]},
            "effective_settings":{}, "side_effects":["subprocess", "gpu"]}]
    })).unwrap()).unwrap();
    let inventory = ToolchainInventory::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.toolchain-inventory/v1", "platform":{"os":"linux", "arch":"x86_64"},
        "artifacts":[{"dependency_id":"ffmpeg", "path":fixture.tool,
            "expected_sha256":fixture.tool_sha256, "maximum_bytes":1048576,
            "observation":observation}], "hardware":[]
    })).unwrap()).unwrap();
    let inspected = serde_json::to_value(inspect_profile(&profile, &inventory, &[]).unwrap()).unwrap();
    assert_eq!(inspected["ready"], false);
    assert!(inspected["facts"].as_array().unwrap().iter().any(|fact|
        fact["check"] == "backend" && fact["status"] == "unverified"));
    assert!(inspected["facts"].as_array().unwrap().iter().any(|fact|
        fact["check"] == "package_revision" && fact["status"] == "unverified"));
    assert_eq!(inspected["native_qualification"], false);

    let fixture = Fixture::new("ffmpeg");
    fixture.mode("unknown_flag");
    let report = fixture.probe();
    let flags = report["tools"][0]["observation"]["flags"].as_array();
    assert!(flags.is_none_or(|flags| !flags.contains(&json!("-map"))));
}

#[test]
fn cli_failure_preserves_probe_evidence_without_creating_inventory_or_registration() {
    let fixture = Fixture::new("ffprobe");
    fixture.mode("nonzero");
    let configuration = fixture.root.path().join("configuration.json");
    fs::write(&configuration, serde_json::to_vec(&fixture.configuration).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "toolchain", "probe", "--configuration"])
        .arg(&configuration).output().unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let envelope: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(envelope["command"], "toolchain_probe");
    no_observation(&envelope["result"]);
    assert_eq!(command(&envelope["result"], "version")["exit_code"], 17);
    assert!(!fixture.root.path().join("inventory.json").exists());
    assert!(!fixture.root.path().join("provider-registration.json").exists());
}

}
