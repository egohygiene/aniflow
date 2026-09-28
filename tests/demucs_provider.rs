#![cfg(unix)]

//! The real provider ABI is exercised with an explicitly synthetic Demucs
//! executable. These tests establish orchestration and refusal behavior, not
//! real-model quality, speed, or release qualification.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use aniflow::{
    CancellationToken, HostResources, PipelineInputBinding, PipelinePlanningContext,
    PipelineV3Configuration, PipelineV3ResumeRequest, PipelineV3RunProgress, PipelineV3RunRequest,
    PipelineV3RunState, PipelineV3StageState, PipelineV3Workspace, ProviderExecutionFailureCode,
    ProviderExecutionLimits, ProviderExecutionOutcome, ProviderExecutionReport, ProviderManifest,
    ProviderRegistrationDocument, ProviderRegistry, SideEffect, load_stage_checkpoint, plan_v3,
    resume_v3, run_v3, run_v3_with_progress_and_cancellation, status_v3,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;
use walkdir::WalkDir;

const FAKE_PYTHON: &str = r#"#!/usr/bin/python3
import json
import os
import pathlib
import struct
import sys
import tempfile
import time
import wave

root = pathlib.Path(__file__).parent
mode = (root / "mode.txt").read_text().strip()
arguments = sys.argv[1:]
if arguments[:3] == ["-I", "-B", "-c"]:
    print(json.dumps({"demucs_version": "4.0.1", "python_version": "3.11.0"}))
    raise SystemExit(0)
assert arguments[:4] == ["-I", "-B", "-m", "demucs.separate"], arguments
expected = ["--name", "htdemucs_6s", "--two-stems", "vocals", "--device", "cpu",
            "--shifts", "0", "--jobs", "0", "--repo"]
assert arguments[4:15] == expected, arguments
assert arguments[16] == "--out" and arguments[18] == "--", arguments
assert len(arguments) == 20, arguments
cache = pathlib.Path(arguments[15])
assert (cache / "htdemucs_6s.yaml").is_file()
assert (cache / "5c90dfd2-34c22ccb.th").is_file()
source = pathlib.Path(arguments[-1])
temporary_root = pathlib.Path(tempfile.gettempdir()).resolve()
assert source.parent in temporary_root.parents, (source, temporary_root)
assert all(pathlib.Path(os.environ[name]).resolve() == temporary_root for name in ("TMPDIR", "TMP", "TEMP"))
assert os.environ["TORCH_HUB_OFFLINE"] == "1"
assert os.environ["HF_HUB_OFFLINE"] == "1"
assert os.environ["PATH"] == str(root)
assert "PYTHONPATH" not in os.environ and "PYTHONHOME" not in os.environ
with (root / "launches.jsonl").open("a") as log:
    log.write(json.dumps(arguments) + "\n")
if mode == "nonzero":
    raise SystemExit(23)
if mode == "sleep":
    # Demucs creates named temporary decoder files; interruption must clean
    # them with the owning runtime's candidate directory, even without unlink.
    with tempfile.NamedTemporaryFile(delete=False) as scratch:
        scratch.write(b"synthetic interrupted decoder scratch")
    time.sleep(30)
output = pathlib.Path(arguments[17]) / "htdemucs_6s" / source.stem
output.mkdir(parents=True)
with wave.open(str(source), "rb") as audio:
    duration = audio.getnframes() / audio.getframerate()
for stem in ["vocals", "no_vocals"]:
    if mode == "partial" and stem == "no_vocals":
        continue
    target = output / (stem + ".wav")
    if mode == "zero":
        target.write_bytes(b"")
        continue
    if mode == "corrupt":
        target.write_bytes(b"RIFF synthetic but not a valid WAV")
        continue
    frames = int(duration * 44100)
    if mode == "duration":
        frames += 44100
    with wave.open(str(target), "wb") as audio:
        audio.setnchannels(2)
        audio.setsampwidth(2)
        audio.setframerate(22050 if mode == "wrong_rate" else 44100)
        audio.writeframes(struct.pack("<hh", 123, -123) * frames)
    if mode == "truncated":
        target.write_bytes(target.read_bytes()[:-100])
if mode == "transient_temp":
    # Hold four runtime-visible paths across many 10 ms watcher intervals:
    # source snapshot + two candidate WAVs + one decoder scratch file. This
    # catches a wrapper that mistakes three final outputs for the staging cap.
    with tempfile.NamedTemporaryFile() as scratch:
        scratch.write(b"synthetic decoder scratch")
        scratch.flush()
        time.sleep(0.2)
"#;

struct Fixture {
    _temporary: TempDir,
    root: PathBuf,
    source: PathBuf,
    source_before: Vec<u8>,
}

impl Fixture {
    fn new(mode: &str) -> Self {
        let temporary = tempfile::Builder::new()
            .prefix("aniflow-demucs-")
            .tempdir()
            .expect("synthetic fixture root should be created");
        let root = temporary.path().join("synthetic media café 雪");
        fs::create_dir(&root).expect("Unicode fixture directory should be created");
        write_executable(&root.join("python stub"), FAKE_PYTHON);
        write_executable(
            &root.join("ffmpeg"),
            "#!/usr/bin/python3\nprint(\"ffmpeg version 7.1 synthetic\")\n",
        );
        write_executable(
            &root.join("ffprobe"),
            "#!/usr/bin/python3\nprint(\"ffprobe version 7.1 synthetic\")\n",
        );
        fs::write(root.join("mode.txt"), mode).expect("stub mode should be written");
        let cache = root.join("cached model assets");
        fs::create_dir(&cache).expect("synthetic model cache should be created");
        fs::write(cache.join("htdemucs_6s.yaml"), "models: ['5c90dfd2']\n")
            .expect("synthetic model bag should be written");
        fs::write(
            cache.join("5c90dfd2-34c22ccb.th"),
            b"synthetic checkpoint bytes; never load in PyTorch",
        )
        .expect("synthetic model checkpoint should be written");
        let source = root.join("source track é 音.wav");
        let source_before = synthetic_wav();
        fs::write(&source, &source_before).expect("synthetic source WAV should be written");
        Self {
            _temporary: temporary,
            root,
            source,
            source_before,
        }
    }

    fn bundle(&self) -> PathBuf {
        self.root.join("prepared provider bundle")
    }

    fn prepare(&self) -> Output {
        Command::new("python3")
            .arg(provider_root().join("workflow.py"))
            .arg("prepare")
            .arg("--python")
            .arg(self.root.join("python stub"))
            .arg("--ffmpeg")
            .arg(self.root.join("ffmpeg"))
            .arg("--ffprobe")
            .arg(self.root.join("ffprobe"))
            .arg("--model-repository")
            .arg(self.root.join("cached model assets"))
            .arg("--output-directory")
            .arg(self.bundle())
            .output()
            .expect("local preparation helper should launch")
    }

    fn prepared(mode: &str) -> Self {
        let fixture = Self::new(mode);
        let output = fixture.prepare();
        assert!(
            output.status.success(),
            "synthetic provider should prepare: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture
    }

    fn configuration(&self) -> PipelineV3Configuration {
        PipelineV3Configuration::load(self.bundle().join("pipeline.yml"))
            .expect("prepared pipeline should satisfy Pipeline v3")
    }

    fn bindings(&self) -> Vec<PipelineInputBinding> {
        vec![PipelineInputBinding::new(
            &self.configuration().inputs[0].id,
            &self.source,
        )]
    }

    fn registry(&self) -> ProviderRegistry {
        let registration =
            ProviderRegistrationDocument::load(self.bundle().join("registration.json"))
                .expect("prepared registration should load")
                .into_registration()
                .expect("prepared registration should materialize");
        let mut registry = ProviderRegistry::new();
        registry
            .register(registration)
            .expect("registration should be unique");
        registry
    }

    fn request(&self) -> PipelineV3RunRequest {
        let plan = plan_v3(
            self.bundle().join("pipeline.yml"),
            &self.bindings(),
            &[self.bundle().join("registration.json")],
            &PipelinePlanningContext {
                host: HostResources {
                    cpu_threads: 2,
                    memory_mib: 16_384,
                    storage_mib: 16_384,
                    gpu_available: false,
                    network_available: false,
                },
                allowed_side_effects: vec![
                    SideEffect::FilesystemRead,
                    SideEffect::FilesystemWrite,
                    SideEffect::EnvironmentRead,
                    SideEffect::Subprocess,
                    SideEffect::Ai,
                ],
                offline: true,
            },
        )
        .expect("prepared synthetic provider should resolve offline");
        PipelineV3RunRequest::new(plan, self.bindings(), self.registry())
            .with_output_directory(self.root.join("isolated runs"))
    }

    fn launch_count(&self) -> usize {
        fs::read_to_string(self.root.join("launches.jsonl"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn assert_source_unchanged(&self) {
        assert_eq!(fs::read(&self.source).unwrap(), self.source_before);
    }

    fn workflow_command(&self, action: &str) -> Command {
        let mut command = Command::new("python3");
        command
            .arg(provider_root().join("workflow.py"))
            .arg(action)
            .arg("--registration")
            .arg(self.bundle().join("registration.json"))
            .arg("--input")
            .arg(&self.source)
            .arg("--aniflow")
            .arg(env!("CARGO_BIN_EXE_aniflow"))
            .arg("--timeout-seconds")
            .arg("10");
        command
    }
}

#[test]
fn demucs_success_checkpoints_all_outputs_and_resumes_without_relaunch() {
    let fixture = Fixture::prepared("success");
    let manifest = ProviderManifest::from_json_slice(
        &fs::read(fixture.bundle().join("manifest.json")).unwrap(),
    )
    .expect("prepared manifest should validate");
    assert_eq!(manifest.capabilities.len(), 1);
    assert_eq!(
        manifest.capabilities[0]
            .outputs
            .iter()
            .map(|port| port.name.as_str())
            .collect::<Vec<_>>(),
        ["vocals", "accompaniment", "evidence"]
    );
    let configuration = fixture.configuration();
    assert_eq!(configuration.stages.len(), 1);
    assert_eq!(configuration.stages[0].outputs.len(), 3);
    let stage_id = &configuration.stages[0].id;
    let outcome = run_v3(fixture.request()).expect("synthetic separation should run");
    assert_eq!(outcome.executed_stages, std::slice::from_ref(stage_id));
    assert_eq!(outcome.outputs.len(), 3);
    assert_eq!(fixture.launch_count(), 1);
    for output in &outcome.outputs {
        assert!(output.path.starts_with(&outcome.run_directory));
        let bytes = fs::read(&output.path).expect("accepted artifact should be readable");
        assert!(!bytes.is_empty());
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), output.sha256);
        if output
            .path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let evidence: Value = serde_json::from_slice(&bytes).expect("evidence should be JSON");
            assert_eq!(evidence["schema"], "aniflow.demucs-separation/v1");
            assert_eq!(evidence["accepted"], true);
            assert_eq!(
                evidence["source"]["sha256"],
                format!("{:x}", Sha256::digest(&fixture.source_before))
            );
            assert_eq!(evidence["source"]["sample_frames"], 4410);
            assert_eq!(evidence["offline"]["network_sandbox"], false);
            assert_eq!(
                evidence["validation"]["perceptual_separation_quality"],
                "not_tested"
            );
            let stems = evidence["outputs"]
                .as_array()
                .expect("evidence should identify both stems");
            assert_eq!(stems.len(), 2);
            for stem in stems {
                let artifact = outcome
                    .outputs
                    .iter()
                    .find(|candidate| {
                        stem["artifact_id"].as_str() == Some(candidate.artifact.as_str())
                    })
                    .expect("evidence must refer to an actual published output");
                assert_eq!(stem["sha256"], artifact.sha256);
                assert_eq!(stem["sample_rate_hz"], 44_100);
                assert_eq!(stem["channels"], 2);
                assert_eq!(stem["sample_frames"], 4410);
            }
        } else {
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..12], b"WAVE");
        }
    }
    let before_status = snapshot(&outcome.run_directory);
    let status = status_v3(&outcome.run_directory).expect("status should read a valid run");
    assert_eq!(snapshot(&outcome.run_directory), before_status);
    assert_eq!(status.payload.state, PipelineV3RunState::Complete);
    assert_eq!(
        status.payload.stages[0].state,
        PipelineV3StageState::Complete
    );
    let checkpoint = load_stage_checkpoint(
        &PipelineV3Workspace::open_read_only(&outcome.run_directory).unwrap(),
        status.payload.stages[0]
            .checkpoint
            .as_ref()
            .expect("stage should have checkpoint"),
    )
    .expect("checkpoint should validate");
    assert_eq!(checkpoint.payload.outputs.len(), 3);
    assert_eq!(checkpoint.payload.validations.len(), 3);
    let output_bytes = outcome
        .outputs
        .iter()
        .map(|output| fs::read(&output.path).unwrap())
        .collect::<Vec<_>>();
    let resumed = resume_v3(PipelineV3ResumeRequest::new(
        &outcome.run_directory,
        fixture.bindings(),
        fixture.registry(),
    ))
    .expect("compatible local checkpoint should resume");
    assert!(resumed.executed_stages.is_empty());
    assert_eq!(resumed.reused_stages, std::slice::from_ref(stage_id));
    assert_eq!(fixture.launch_count(), 1);
    assert_eq!(
        resumed
            .outputs
            .iter()
            .map(|output| fs::read(&output.path).unwrap())
            .collect::<Vec<_>>(),
        output_bytes
    );
    fixture.assert_source_unchanged();
}

#[test]
fn missing_local_model_assets_refuse_preparation_before_separation() {
    let fixture = Fixture::new("success");
    fs::remove_file(
        fixture
            .root
            .join("cached model assets/5c90dfd2-34c22ccb.th"),
    )
    .unwrap();
    let output = fixture.prepare();
    assert!(
        !output.status.success(),
        "missing checkpoint must refuse preparation"
    );
    assert_eq!(fixture.launch_count(), 0);
    assert!(
        !fixture.bundle().exists(),
        "failed preflight must not publish a bundle"
    );
    fixture.assert_source_unchanged();
}

#[test]
fn invalid_demucs_outputs_and_nonzero_exit_never_complete_or_checkpoint() {
    for mode in [
        "partial",
        "zero",
        "corrupt",
        "truncated",
        "duration",
        "wrong_rate",
        "nonzero",
    ] {
        let fixture = Fixture::prepared(mode);
        let run = failed_run(&fixture, fixture.request(), &CancellationToken::default());
        assert_failed_without_checkpoint(&run, PipelineV3RunState::Failed);
        let report = execution_report(&run);
        assert_eq!(
            report.payload.outcome,
            ProviderExecutionOutcome::Failed,
            "mode {mode}"
        );
        assert_eq!(
            report.payload.failure.unwrap().code,
            ProviderExecutionFailureCode::ExitFailure,
            "mode {mode}"
        );
        assert_eq!(fixture.launch_count(), 1, "mode {mode}");
        fixture.assert_source_unchanged();
    }
}

#[test]
fn changed_model_after_preparation_refuses_before_demucs_launch() {
    let fixture = Fixture::prepared("success");
    fs::write(
        fixture
            .root
            .join("cached model assets/5c90dfd2-34c22ccb.th"),
        b"changed",
    )
    .unwrap();
    let run = failed_run(&fixture, fixture.request(), &CancellationToken::default());
    assert_failed_without_checkpoint(&run, PipelineV3RunState::Failed);
    assert_eq!(fixture.launch_count(), 0);
    fixture.assert_source_unchanged();
}

#[test]
fn runtime_artifact_bounds_include_demucs_temporary_files() {
    let fixture = Fixture::prepared("transient_temp");
    let request = fixture
        .request()
        .with_execution_limits(ProviderExecutionLimits {
            maximum_artifact_files: 3,
            termination_grace_period: Duration::from_millis(100),
            ..ProviderExecutionLimits::default()
        });
    let run = failed_run(&fixture, request, &CancellationToken::default());
    assert_failed_without_checkpoint(&run, PipelineV3RunState::Failed);
    assert_eq!(
        execution_report(&run).payload.outcome,
        ProviderExecutionOutcome::ArtifactLimitExceeded
    );
    assert_eq!(fixture.launch_count(), 1);
    fixture.assert_source_unchanged();
}

#[test]
fn management_workflow_delegates_run_resume_and_refuses_stale_cache_reuse() {
    let fixture = Fixture::prepared("transient_temp");
    let output = fixture
        .workflow_command("run")
        .arg("--output-directory")
        .arg(fixture.root.join("managed runs"))
        .args([
            "--host-cpu-threads",
            "2",
            "--host-memory-mib",
            "16384",
            "--host-storage-mib",
            "16384",
        ])
        .output()
        .expect("workflow should delegate to the actual aniflow CLI");
    assert!(
        output.status.success(),
        "workflow run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("workflow should preserve machine stdout");
    let run = PathBuf::from(
        envelope["result"]["run_directory"]
            .as_str()
            .expect("run should expose its workspace"),
    );
    assert_eq!(fixture.launch_count(), 1);
    let output = fixture
        .workflow_command("resume")
        .arg("--run-directory")
        .arg(&run)
        .output()
        .expect("workflow should delegate compatible resume");
    assert!(
        output.status.success(),
        "workflow resume failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("resume should preserve machine stdout");
    assert_eq!(envelope["result"]["executed_stages"], serde_json::json!([]));
    assert_eq!(fixture.launch_count(), 1);

    // Run-local reuse does not invoke the provider, so the management wrapper
    // must recheck external dependencies before dispatching resume to Rust.
    let before_refusal = snapshot(&run);
    fs::write(
        fixture
            .root
            .join("cached model assets/5c90dfd2-34c22ccb.th"),
        b"changed model",
    )
    .unwrap();
    let output = fixture
        .workflow_command("resume")
        .arg("--run-directory")
        .arg(&run)
        .output()
        .expect("stale cache should receive an inspectable refusal");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("digest changed"));
    assert!(output.stdout.is_empty());
    assert_eq!(snapshot(&run), before_refusal);
    assert_eq!(fixture.launch_count(), 1);
    fixture.assert_source_unchanged();
}

#[test]
fn timeout_and_active_cancellation_preserve_failure_evidence_without_checkpoint() {
    let timed = Fixture::prepared("sleep");
    let request = timed
        .request()
        .with_execution_limits(ProviderExecutionLimits {
            timeout: Duration::from_secs(2),
            termination_grace_period: Duration::from_millis(100),
            ..ProviderExecutionLimits::default()
        });
    let run = failed_run(&timed, request, &CancellationToken::default());
    assert_failed_without_checkpoint(&run, PipelineV3RunState::Failed);
    assert_eq!(
        execution_report(&run).payload.failure.unwrap().code,
        ProviderExecutionFailureCode::Timeout
    );
    assert_eq!(timed.launch_count(), 1);
    timed.assert_source_unchanged();
    recover_interrupted_run(&timed, &run);

    let cancelled = Fixture::prepared("sleep");
    let cancellation = CancellationToken::default();
    let thread_token = cancellation.clone();
    let launches = cancelled.root.join("launches.jsonl");
    let watcher = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !launches.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        thread_token.cancel();
    });
    let request = cancelled
        .request()
        .with_execution_limits(ProviderExecutionLimits {
            timeout: Duration::from_secs(8),
            termination_grace_period: Duration::from_millis(100),
            ..ProviderExecutionLimits::default()
        });
    let run = failed_run(&cancelled, request, &cancellation);
    watcher.join().expect("cancellation watcher should finish");
    assert_failed_without_checkpoint(&run, PipelineV3RunState::Cancelled);
    assert_eq!(
        execution_report(&run).payload.failure.unwrap().code,
        ProviderExecutionFailureCode::Cancelled
    );
    assert_eq!(cancelled.launch_count(), 1);
    cancelled.assert_source_unchanged();
    recover_interrupted_run(&cancelled, &run);
}

fn recover_interrupted_run(fixture: &Fixture, run: &Path) {
    fs::write(fixture.root.join("mode.txt"), "success").unwrap();
    let resumed = resume_v3(PipelineV3ResumeRequest::new(
        run,
        fixture.bindings(),
        fixture.registry(),
    ))
    .expect("an interrupted synthetic separation should restart and complete");
    assert_eq!(resumed.executed_stages.len(), 1);
    assert!(resumed.reused_stages.is_empty());
    assert_eq!(resumed.outputs.len(), 3);
    assert_eq!(fixture.launch_count(), 2);
    assert_eq!(
        status_v3(run).unwrap().payload.state,
        PipelineV3RunState::Complete
    );
    fixture.assert_source_unchanged();
}

fn provider_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("providers/demucs")
}

fn failed_run(
    fixture: &Fixture,
    request: PipelineV3RunRequest,
    cancellation: &CancellationToken,
) -> PathBuf {
    let mut run_directory = None;
    run_v3_with_progress_and_cancellation(request, cancellation, |progress| {
        if let PipelineV3RunProgress::Started {
            run_directory: directory,
            ..
        } = progress
        {
            run_directory = Some(directory.clone());
        }
    })
    .expect_err("synthetic refusal must never complete a run");
    fixture.assert_source_unchanged();
    run_directory.expect("failed execution should expose its isolated run")
}

fn assert_failed_without_checkpoint(run: &Path, state: PipelineV3RunState) {
    let status = status_v3(run).expect("failure evidence should remain inspectable");
    assert_eq!(status.payload.state, state);
    assert!(status.payload.stages[0].checkpoint.is_none());
    assert!(
        fs::read_dir(run.join("state/checkpoints"))
            .unwrap()
            .next()
            .is_none()
    );
    for entry in WalkDir::new(run.join("work")) {
        let entry = entry.expect("failed workspace should remain traversable");
        assert!(
            entry.file_type().is_dir(),
            "partial artifact survived failure: {}",
            entry.path().display()
        );
        assert!(
            !entry.file_name().to_string_lossy().starts_with(".demucs-"),
            "private provider staging survived failure"
        );
    }
}

fn execution_report(run: &Path) -> ProviderExecutionReport {
    let paths = fs::read_dir(run.join("providers"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".report.json"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    ProviderExecutionReport::from_json_slice(&fs::read(&paths[0]).unwrap())
        .expect("execution evidence should validate")
}

fn write_executable(path: &Path, text: &str) {
    fs::write(path, text).expect("synthetic executable should be written");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .expect("synthetic executable should be runnable");
}

fn synthetic_wav() -> Vec<u8> {
    // A 100 ms, stereo, PCM16 wave is generated here: no checked-in or user media.
    let sample_rate = 44_100_u32;
    let data_size = sample_rate / 10 * 4;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for frame in 0..sample_rate / 10 {
        let sample = if frame % 2 == 0 { 256_i16 } else { -256_i16 };
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes.extend_from_slice(&(-sample).to_le_bytes());
    }
    bytes
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .map(|entry| {
            let entry = entry.expect("workspace should be traversable");
            let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
            let bytes = if entry.file_type().is_dir() {
                None
            } else {
                Some(fs::read(entry.path()).unwrap())
            };
            (relative, bytes)
        })
        .collect()
}
