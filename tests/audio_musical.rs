#![cfg(unix)]

//! Synthetic configured executables prove adapter contracts, not musical accuracy.
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::audio_analysis::{AudioAnalysis, AudioObservationKind};
use aniflow::audio_inspection::{
    AudioInspectionConfiguration, AudioInspectionRequest, AudioToolPin,
};
use aniflow::audio_musical::{self, MusicalAnalysisConfiguration, MusicalAnalysisRequest};
use aniflow::audio_stem::StemSelection;
use aniflow::{
    CancellationToken, PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunState, status_v3,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

const INSPECTION_TOOL: &str = r#"#!/usr/bin/python3
import hashlib, json, pathlib, sys, wave
name = pathlib.Path(__file__).name
args = sys.argv[1:]
if args == ["-version"]:
    print(name + " version 6.1.1 synthetic")
    raise SystemExit(0)
with wave.open(args[args.index("-i") + 1], "rb") as audio:
    rate, channels, frames = audio.getframerate(), audio.getnchannels(), audio.getnframes()
    pcm = audio.readframes(frames)
if name == "ffprobe":
    print(json.dumps({"streams": [{"index": 0, "codec_type": "audio", "codec_name": "pcm_s16le", "sample_fmt": "s16",
        "sample_rate": str(rate), "channels": channels, "bits_per_sample": 16, "time_base": "1/" + str(rate),
        "duration_ts": frames, "bit_rate": str(rate * channels * 16)}], "format": {"format_name": "wav", "nb_streams": 1}}))
else:
    print("SHA256=" + hashlib.sha256(pcm).hexdigest())
"#;

// This executable impersonates the isolated interpreter ABI only. Its observed
// metadata and musical candidates are explicitly synthetic conformance evidence.
const SYNTHETIC_PYTHON: &str = r#"#!/usr/bin/python3
import json, pathlib, sys, time, wave
root = pathlib.Path(__file__).parent
args = sys.argv[1:]
assert args[:2] == ['-I', '-B']
assert pathlib.Path(args[2]) == root / 'adapter.py'
if args[3:] == ['--probe']:
    print((root / 'probe.json').read_text())
    raise SystemExit(0)
assert args[3] == '--input' and len(args) == 5
(root / 'musical-started').write_text('yes')
with (root / 'launches').open('a') as log:
    log.write(json.dumps(args) + '\n')
mode = (root / 'mode').read_text()
if mode == 'sleep': time.sleep(120)
if mode == 'nonzero': raise SystemExit(23)
if mode == 'flood':
    print('x' * 100000)
    raise SystemExit(0)
if mode == 'malformed':
    print('{not-json')
    raise SystemExit(0)
with wave.open(args[4], 'rb') as audio:
    rate, channels, frames = audio.getframerate(), audio.getnchannels(), audio.getnframes()
value = {
    'schema': 'aniflow.audio-musical-observation/v1',
    'sample_rate_hz': rate, 'channels': channels, 'sample_frames': frames,
    'duration_seconds': frames / rate, 'downmix': 'arithmetic_average',
    'bpm': {'value': 120.0, 'ticks_seconds': [index / 2 for index in range(1, 16)],
            'raw_confidence': 2.5, 'estimates': [120.0, 60.0], 'bpm_intervals': [0.5]},
    'key_profiles': [
        {'profile': 'krumhansl', 'key': 'C', 'scale': 'major', 'raw_strength': 0.75},
        {'profile': 'temperley', 'key': 'A', 'scale': 'minor', 'raw_strength': 0.60}]}
if mode == 'nonfinite': value['bpm']['value'] = float('nan')
if mode == 'out_of_order': value['bpm']['ticks_seconds'] = [1.0, 0.5]
if mode == 'key_only':
    value['bpm'] = {'value': 0.0, 'ticks_seconds': [], 'raw_confidence': 0.0,
                    'estimates': [], 'bpm_intervals': []}
print(json.dumps(value))
"#;

struct Fixture {
    root: TempDir,
    input: PathBuf,
    tools: AudioInspectionConfiguration,
    settings: MusicalAnalysisConfiguration,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("musical café 雪 ")
            .tempdir()
            .unwrap();
        let pin = |name: &str| {
            let path = root.path().join(name);
            executable(&path, INSPECTION_TOOL);
            AudioToolPin {
                executable: path,
                version: "6.1.1".into(),
                sha256: digest(INSPECTION_TOOL.as_bytes()),
            }
        };
        let tools = AudioInspectionConfiguration {
            schema: "aniflow.audio-inspection.configuration/v1".into(),
            ffmpeg: pin("ffmpeg"),
            ffprobe: pin("ffprobe"),
            tool_timeout_milliseconds: 5000,
            maximum_tool_output_bytes: 65536,
        };
        let python = root.path().join("python");
        executable(&python, SYNTHETIC_PYTHON);
        let adapter = root.path().join("adapter.py");
        let adapter_bytes = b"# Synthetic adapter identity; fixture interpreter supplies JSON.\n";
        fs::write(&adapter, adapter_bytes).unwrap();
        let runtime_digest = digest(b"synthetic dependency inventory, not installed Essentia");
        let probe = json!({
            "schema": "aniflow.audio-musical-probe/v1", "python_version": "3.11.0",
            "essentia_version": "2.1b6.dev1389", "essentia_runtime_version": "2.1-beta6-dev",
            "essentia_git_sha": "v2.1_beta5-1389-g36ec3d92", "numpy_version": "2.3.5",
            "pyyaml_version": "6.0.3", "six_version": "1.17.0", "runtime_sha256": runtime_digest,
            "runtime_file_count": 1, "runtime_byte_count": 64,
            "license": {"essentia_expression": "AGPL-3.0-only",
                "essentia_metadata_sha256": digest(b"synthetic Essentia license metadata"),
                "numpy_metadata_sha256": digest(b"synthetic NumPy license metadata")}
        });
        fs::write(
            root.path().join("probe.json"),
            serde_json::to_vec(&probe).unwrap(),
        )
        .unwrap();
        let settings = MusicalAnalysisConfiguration::from_json_slice(&serde_json::to_vec(&json!({
            "schema": "aniflow.audio-musical.configuration/v1",
            "python": {"executable": python, "version": "3.11.0", "sha256": digest(SYNTHETIC_PYTHON.as_bytes())},
            "adapter": {"path": adapter, "sha256": digest(adapter_bytes)},
            "runtime": {"sha256": runtime_digest, "file_count": 1, "byte_count": 64},
            "tool_timeout_milliseconds": 5000, "maximum_tool_output_bytes": 65536
        })).unwrap()).unwrap();
        let input = root.path().join("input source.wav");
        write_pcm(&input, 44100, 352800, 1, false);
        fs::write(root.path().join("mode"), "ok").unwrap();
        Self {
            root,
            input,
            tools,
            settings,
        }
    }

    fn request(&self) -> MusicalAnalysisRequest {
        MusicalAnalysisRequest::new(
            AudioInspectionRequest::new(
                &self.input,
                self.tools.clone(),
                env!("CARGO_BIN_EXE_aniflow"),
            ),
            self.settings.clone(),
        )
    }

    fn mode(&self, mode: &str) {
        fs::write(self.root.path().join("mode"), mode).unwrap();
    }
    fn launches(&self) -> Vec<u8> {
        fs::read(self.root.path().join("launches")).unwrap_or_default()
    }
    fn run(&self) -> PipelineV3RunOutcome {
        audio_musical::run(
            self.request(),
            Some(self.root.path().join("runs")),
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap()
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn executable(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn write_pcm(path: &Path, rate: u32, frames: u32, channels: u16, silent: bool) {
    let size = frames * u32::from(channels) * 2;
    let mut bytes = Vec::with_capacity(size as usize + 44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(size + 36).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    for _ in 0..frames {
        for channel in 0..channels {
            let sample = if silent {
                0_i16
            } else if channel == 0 {
                1200
            } else {
                -1200
            };
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
    }
    fs::write(path, bytes).unwrap();
}

fn artifact(outcome: &PipelineV3RunOutcome, id: &str) -> PathBuf {
    outcome
        .outputs
        .iter()
        .find(|output| output.id == id)
        .unwrap()
        .path
        .clone()
}

fn normalized(outcome: &PipelineV3RunOutcome) -> AudioAnalysis {
    AudioAnalysis::from_json_slice(&fs::read(artifact(outcome, "analysis")).unwrap()).unwrap()
}

fn musical(outcome: &PipelineV3RunOutcome) -> Value {
    let bytes = fs::read(artifact(outcome, "musical")).unwrap();
    audio_musical::AudioMusicalAnalysis::from_json_slice(&bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn musical_plan_run_preserves_candidates_and_exact_resume() {
    let fixture = Fixture::new();
    let source = fs::read(&fixture.input).unwrap();
    let plan = audio_musical::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.stages.len(), 2);
    assert!(fixture.launches().is_empty());
    assert!(!fixture.root.path().join("runs").exists());
    let outcome = fixture.run();
    assert_eq!(outcome.executed_stages.len(), 2);
    let report = musical(&outcome);
    assert_eq!(report["schema"], "aniflow.audio-musical-analysis/v1");
    assert_eq!(report["source"]["artifact"]["sha256"], digest(&source));
    assert_eq!(report["provenance"], "heuristic");
    assert_eq!(report["confidence"]["kind"], "unavailable");
    assert_eq!(
        report["probe"]["runtime_sha256"],
        digest(b"synthetic dependency inventory, not installed Essentia")
    );
    assert_eq!(report["result"]["key_disagreement"], true);
    let tempos = report["result"]["tempo_candidates"].as_array().unwrap();
    assert_eq!(tempos.len(), 3);
    assert!(tempos.iter().any(|candidate| candidate["value"] == 60.0));
    assert_eq!(
        report["result"]["key_candidates"].as_array().unwrap().len(),
        2
    );
    assert_eq!(report["result"]["beats"][0]["source_frame"], 22050);
    let analysis = normalized(&outcome);
    assert_eq!(
        analysis
            .observations
            .iter()
            .filter(|item| item.kind == AudioObservationKind::Tempo)
            .count(),
        3
    );
    assert_eq!(
        analysis
            .observations
            .iter()
            .filter(|item| item.kind == AudioObservationKind::Key)
            .count(),
        2
    );
    assert_eq!(
        analysis
            .timelines
            .iter()
            .map(|timeline| timeline.events.len())
            .sum::<usize>(),
        15
    );
    for output in &outcome.outputs {
        assert_eq!(digest(&fs::read(&output.path).unwrap()), output.sha256);
    }
    let launches = fixture.launches();
    let resumed = audio_musical::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert!(resumed.executed_stages.is_empty());
    assert_eq!(resumed.reused_stages, outcome.executed_stages);
    assert_eq!(fixture.launches(), launches);
    assert_eq!(fs::read(&fixture.input).unwrap(), source);
}

#[test]
fn absent_tempo_does_not_discard_independent_key_candidates() {
    let fixture = Fixture::new();
    fixture.mode("key_only");
    let outcome = fixture.run();
    let report = musical(&outcome);
    assert_eq!(report["result"]["tempo_status"]["status"], "unavailable");
    assert_eq!(report["result"]["beats_status"]["status"], "unavailable");
    assert_eq!(report["result"]["key_status"]["status"], "estimated");
    assert!(
        report["result"]["tempo_candidates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        report["result"]["key_candidates"].as_array().unwrap().len(),
        2
    );
    let analysis = normalized(&outcome);
    assert!(
        !analysis
            .observations
            .iter()
            .any(|item| item.kind == AudioObservationKind::Tempo)
    );
    assert_eq!(
        analysis
            .observations
            .iter()
            .filter(|item| item.kind == AudioObservationKind::Key)
            .count(),
        2
    );
    assert!(analysis.timelines.is_empty());
}

#[test]
fn unsupported_short_silent_and_cancelled_downmix_are_explicitly_unavailable() {
    for (rate, frames, channels, silent, reason) in [
        (48000, 384000, 1, false, "unsupported_sample_rate"),
        (44100, 352799, 1, false, "insufficient_duration"),
        (44100, 352800, 1, true, "silent_downmix"),
        (44100, 352800, 2, false, "silent_downmix"),
    ] {
        let fixture = Fixture::new();
        write_pcm(&fixture.input, rate, frames, channels, silent);
        let source = fs::read(&fixture.input).unwrap();
        let outcome = fixture.run();
        let report = musical(&outcome);
        assert_eq!(
            report["result"],
            json!({"status": "unavailable", "reason": reason})
        );
        assert_eq!(report["commands"].as_array().unwrap().len(), 2);
        assert!(
            fixture.launches().is_empty(),
            "must skip estimator for {reason}"
        );
        assert!(
            !normalized(&outcome)
                .observations
                .iter()
                .any(|item| matches!(
                    item.kind,
                    AudioObservationKind::Tempo | AudioObservationKind::Key
                ))
        );
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn stale_missing_or_changed_dependency_identity_cannot_plan_or_resume() {
    let mut fixture = Fixture::new();
    let outcome = fixture.run();
    let launches = fixture.launches();
    let plan_bytes = fs::read(outcome.run_directory.join("plan/plan.json")).unwrap();
    let original_adapter = fs::read(&fixture.settings.adapter.path).unwrap();
    fs::write(
        &fixture.settings.adapter.path,
        b"# changed adapter identity\n",
    )
    .unwrap();
    let mut progress = false;
    assert!(
        audio_musical::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| progress = true
        )
        .is_err()
    );
    assert!(!progress);
    // Even explicitly repinning new adapter bytes changes the accepted plan.
    fixture.settings.adapter.sha256 = digest(b"# changed adapter identity\n");
    assert!(
        audio_musical::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| progress = true
        )
        .is_err()
    );
    assert!(!progress);
    fs::write(&fixture.settings.adapter.path, &original_adapter).unwrap();
    fixture.settings.adapter.sha256 = digest(&original_adapter);
    let probe_path = fixture.root.path().join("probe.json");
    let original_probe = fs::read(&probe_path).unwrap();
    let mut changed_probe: Value = serde_json::from_slice(&original_probe).unwrap();
    changed_probe["runtime_sha256"] = digest(b"changed synthetic dependency").into();
    fs::write(&probe_path, serde_json::to_vec(&changed_probe).unwrap()).unwrap();
    assert!(audio_musical::plan(&fixture.request(), &CancellationToken::default()).is_err());
    assert!(
        audio_musical::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| progress = true
        )
        .is_err()
    );
    fs::write(&probe_path, original_probe).unwrap();
    fs::remove_file(&fixture.settings.python.executable).unwrap();
    assert!(audio_musical::plan(&fixture.request(), &CancellationToken::default()).is_err());
    assert_eq!(fixture.launches(), launches);
    assert_eq!(
        fs::read(outcome.run_directory.join("plan/plan.json")).unwrap(),
        plan_bytes
    );
}

#[test]
fn malformed_nonfinite_unordered_and_bounded_process_failures_never_publish() {
    for mode in [
        "malformed",
        "nonfinite",
        "out_of_order",
        "flood",
        "sleep",
        "nonzero",
    ] {
        let fixture = Fixture::new();
        let source = fs::read(&fixture.input).unwrap();
        fixture.mode(mode);
        let mut run = None;
        let error = audio_musical::run(
            fixture.request(),
            Some(fixture.root.path().join("runs")),
            &CancellationToken::default(),
            |progress| {
                if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
                    run = Some(run_directory.clone());
                }
            },
        )
        .unwrap_err();
        let directory = run
            .unwrap_or_else(|| panic!("{mode}: expected durable run after preflight: {error:?}"));
        assert!(
            fixture.root.path().join("musical-started").exists(),
            "{mode}: expected estimator-stage failure: {error:?}"
        );
        assert_ne!(
            status_v3(&directory).unwrap().payload.state,
            PipelineV3RunState::Complete
        );
        assert!(
            !directory
                .join("artifacts/audio-musical/musical.json")
                .exists(),
            "{mode}: partial musical report published"
        );
        assert!(
            !directory
                .join("artifacts/audio-musical/analysis.json")
                .exists(),
            "{mode}: partial normalized report published"
        );
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn cancellation_retains_technical_checkpoint_and_resume_reruns_only_musical() {
    let fixture = Fixture::new();
    fixture.mode("sleep");
    let cancellation = CancellationToken::default();
    let token = cancellation.clone();
    let marker = fixture.root.path().join("musical-started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            marker.exists(),
            "musical child did not reach cancellation marker"
        );
        token.cancel();
    });
    let mut run = None;
    let result = audio_musical::run(
        fixture.request(),
        Some(fixture.root.path().join("runs")),
        &cancellation,
        |progress| {
            if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
                run = Some(run_directory.clone());
            }
        },
    );
    cancel.join().unwrap();
    assert!(result.is_err());
    let directory = run.unwrap();
    assert_eq!(
        status_v3(&directory).unwrap().payload.state,
        PipelineV3RunState::Cancelled
    );
    fixture.mode("ok");
    let resumed = audio_musical::resume(
        fixture.request(),
        directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed.reused_stages, ["inspect_audio"]);
    assert_eq!(resumed.executed_stages.len(), 1);
    musical(&resumed);
}

#[test]
fn canonical_cli_selects_musical_plan_and_structured_dependency_refusal() {
    let fixture = Fixture::new();
    let tools = fixture.root.path().join("tools.json");
    let settings = fixture.root.path().join("settings.json");
    fs::write(&tools, serde_json::to_vec(&fixture.tools).unwrap()).unwrap();
    fs::write(&settings, serde_json::to_vec(&fixture.settings).unwrap()).unwrap();
    let invoke = |command: &str| {
        Command::new(env!("CARGO_BIN_EXE_aniflow"))
            .args([
                "--output",
                "json",
                "audio",
                command,
                "--analysis",
                "musical",
                "--input",
            ])
            .arg(&fixture.input)
            .arg("--configuration")
            .arg(&tools)
            .arg("--musical-configuration")
            .arg(&settings)
            .output()
            .unwrap()
    };
    let output = invoke("plan");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["command"], "audio_plan");
    assert!(fixture.launches().is_empty());
    fs::remove_file(&fixture.settings.adapter.path).unwrap();
    let output = invoke("analyze");
    assert!(!output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["command"], "audio_analyze");
    assert!(!envelope["error"].is_null());
    assert!(fixture.launches().is_empty());
}

#[test]
fn musical_unavailability_preserves_accepted_provider_neutral_stem_scope() {
    let fixture = Fixture::new();
    let separation = TempDir::new().unwrap();
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/smoke-audio-stem.py"))
        .arg("--aniflow")
        .arg(env!("CARGO_BIN_EXE_aniflow"))
        .arg("--prepare-only")
        .arg(separation.path())
        .args(["--provider", "generic", "--rate", "44100"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let descriptor: Value = serde_json::from_slice(&output.stdout).unwrap();
    let mix = PathBuf::from(descriptor["mix"].as_str().unwrap());
    let original = fs::read(&mix).unwrap();
    let stem = descriptor["stems"][0].as_str().unwrap();
    let selection = StemSelection::new(
        descriptor["separation_run"].as_str().unwrap(),
        descriptor["stage"].as_str().unwrap(),
        stem,
    );
    let inspection = AudioInspectionRequest::new(
        mix.clone(),
        fixture.tools.clone(),
        env!("CARGO_BIN_EXE_aniflow"),
    )
    .with_stem_selection(selection);
    let outcome = audio_musical::run(
        MusicalAnalysisRequest::new(inspection, fixture.settings.clone()),
        Some(fixture.root.path().join("runs")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(outcome.executed_stages.len(), 3);
    let report = musical(&outcome);
    assert_eq!(report["result"]["reason"], "insufficient_duration");
    assert_eq!(report["source"]["stem"]["id"], stem);
    assert_eq!(report["scope"]["stem_id"], stem);
    assert_eq!(
        report["source"]["stem"]["original_mix"]["sha256"],
        digest(&original)
    );
    let analysis = normalized(&outcome);
    assert_eq!(analysis.source.stem.unwrap().id, stem);
    assert!(
        analysis
            .artifacts
            .iter()
            .any(|artifact| artifact.id == "stem_lineage")
    );
    assert!(fixture.launches().is_empty());
    assert_eq!(fs::read(mix).unwrap(), original);
}
