#![cfg(unix)]

//! Synthetic configured executables prove adapter contracts, not midi accuracy.
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::audio_analysis::AudioAnalysis;
use aniflow::audio_inspection::{
    AudioInspectionConfiguration, AudioInspectionRequest, AudioToolPin,
};
use aniflow::audio_midi::{self, MidiConfiguration, MidiRequest};
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
// metadata and midi candidates are explicitly synthetic conformance evidence.
const SYNTHETIC_PYTHON: &str = r#"#!/usr/bin/python3
import hashlib, json, os, pathlib, sys, time, wave
root = pathlib.Path(__file__).parent
args = sys.argv[1:]
assert args[:2] == ['-I', '-B']
assert pathlib.Path(args[2]).parent == pathlib.Path.cwd()
assert pathlib.Path(args[2]).read_bytes() == (root / 'adapter.py').read_bytes()
assert 'PYTHONPATH' not in os.environ and 'HOME' not in os.environ
assert os.environ['TMPDIR'] == str(pathlib.Path.cwd())
if args[3:] == ['--probe']:
    print((root / 'probe.json').read_text())
    raise SystemExit(0)
assert args[3] == '--input' and args[5] == '--model' and len(args) == 7
assert pathlib.Path(args[4]).name == 'source.wav'
assert pathlib.Path(args[6]).name == 'model.onnx'
assert pathlib.Path(args[6]).read_bytes() == (root / 'model.onnx').read_bytes()
(root / 'midi-started').write_text('yes')
with (root / 'launches').open('a') as log: log.write(json.dumps(args) + '\n')
mode = (root / 'mode').read_text()
if mode == 'sleep': time.sleep(120)
if mode == 'nonzero': raise SystemExit(23)
if mode == 'flood':
    print('x' * 100000)
    raise SystemExit(0)
if mode == 'malformed':
    print('{not-json')
    raise SystemExit(0)
if mode == 'empty_stdout': raise SystemExit(0)
with wave.open(args[4], 'rb') as audio:
    rate, channels, frames = audio.getframerate(), audio.getnchannels(), audio.getnframes()
notes = [{'start_microseconds': 0, 'end_microseconds': 500000, 'pitch': 60, 'velocity': 64, 'activation': 0.5},
         {'start_microseconds': 500000, 'end_microseconds': 900000, 'pitch': 64, 'velocity': 64, 'activation': 0.5}]
if mode == 'polyphonic': notes[1]['start_microseconds'] = 0
if mode == 'empty': notes = []
if mode == 'nonfinite': notes[0]['activation'] = float('nan')
if mode == 'out_of_order': notes.reverse()
if mode == 'out_of_range': notes[0]['pitch'] = 109
if mode == 'source_mismatch': frames += 1
if mode == 'bad_velocity': notes[0]['velocity'] = 127
if mode == 'same_pitch_overlap':
    notes[1]['start_microseconds'] = 200000
    notes[1]['pitch'] = 60
if mode == 'mutate_model': pathlib.Path(args[6]).write_bytes(b'x')
if mode == 'mutate_source': pathlib.Path(args[4]).write_bytes(b'x')
if mode == 'undeclared_file': pathlib.Path('unexpected').write_text('x')
value = {'schema': 'aniflow.audio-midi-observation/v1', 'sample_rate_hz': rate,
         'channels': channels, 'sample_frames': frames, 'duration_microseconds': frames * 1000000 // rate,
         'notes': notes}
print(json.dumps(value))
"#;

struct Fixture {
    root: TempDir,
    input: PathBuf,
    tools: AudioInspectionConfiguration,
    settings: MidiConfiguration,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("midi café 雪 ")
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
        let runtime_digest = digest(b"synthetic dependency inventory, not installed Basic Pitch");
        let probe = json!({
            "schema": "aniflow.audio-midi-probe/v1", "python_version": "3.11.0",
            "basic_pitch_version": "0.4.0", "basic_pitch_revision": "9991303bba609a3b93089d13ec80d1d495083596",
            "onnxruntime_version": "1.20.1", "numpy_version": "1.26.4", "librosa_version": "0.10.2.post1",
            "scipy_version": "1.13.1", "pretty_midi_version": "0.2.10", "runtime_sha256": runtime_digest,
            "runtime_file_count": 1, "runtime_byte_count": 64,
            "license": {"basic_pitch_expression": "Apache-2.0", "basic_pitch_metadata_sha256": digest(b"synthetic Basic Pitch metadata"),
                "onnxruntime_expression": "MIT", "onnxruntime_metadata_sha256": digest(b"synthetic ONNX Runtime metadata")}
        });
        fs::write(
            root.path().join("probe.json"),
            serde_json::to_vec(&probe).unwrap(),
        )
        .unwrap();
        let model = root.path().join("model.onnx");
        let model_bytes = vec![0x55_u8; 230444];
        fs::write(&model, &model_bytes).unwrap();
        let settings = MidiConfiguration::from_json_slice(&serde_json::to_vec(&json!({
            "schema": "aniflow.audio-midi.configuration/v1",
            "python": {"executable": python, "version": "3.11.0", "sha256": digest(SYNTHETIC_PYTHON.as_bytes())},
            "adapter": {"path": adapter, "sha256": digest(adapter_bytes)},
            "runtime": {"sha256": runtime_digest, "file_count": 1, "byte_count": 64},
            "model": {"path": model, "sha256": digest(&model_bytes), "byte_size": model_bytes.len(),
                "model_id": "basic-pitch-onnx-icassp-2022", "revision": "9991303bba609a3b93089d13ec80d1d495083596"},
            "tool_timeout_milliseconds": 5000, "maximum_tool_output_bytes": 65536
        })).unwrap()).unwrap();
        let input = root.path().join("input source.wav");
        write_pcm(&input, 22050, 22050, 1, false);
        fs::write(root.path().join("mode"), "ok").unwrap();
        Self {
            root,
            input,
            tools,
            settings,
        }
    }

    fn request(&self) -> MidiRequest {
        MidiRequest::new(
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
        audio_midi::run(
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

fn midi(outcome: &PipelineV3RunOutcome) -> Value {
    let bytes = fs::read(artifact(outcome, "midi")).unwrap();
    audio_midi::AudioMidiReport::from_json_slice(&bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn candidate_plan_run_preserves_source_provenance_and_exact_resume() {
    let fixture = Fixture::new();
    let source = fs::read(&fixture.input).unwrap();
    let plan = audio_midi::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.stages.len(), 2);
    assert!(fixture.launches().is_empty());
    assert!(!fixture.root.path().join("runs").exists());
    let outcome = fixture.run();
    assert_eq!(outcome.executed_stages.len(), 2);
    let report = midi(&outcome);
    assert_eq!(report["schema"], "aniflow.audio-midi/v1");
    assert_eq!(report["source"]["artifact"]["sha256"], digest(&source));
    assert_eq!(report["provenance"], "probabilistic");
    assert_eq!(report["confidence"]["kind"], "unavailable");
    assert_eq!(report["result"]["status"], "candidate");
    assert_eq!(
        report["result"]["observation"]["notes"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(report["result"]["observation"]["notes"][0]["pitch"], 60);
    assert_eq!(report["method"]["instrument"], "unavailable");
    assert_eq!(report["method"]["pitch_bends"], false);
    assert_eq!(report["commands"].as_array().unwrap().len(), 3);
    let analysis = normalized(&outcome);
    assert_eq!(analysis.source.artifact.sha256, digest(&source));
    assert!(
        analysis
            .semantic_artifacts
            .iter()
            .any(|item| item.kind
                == aniflow::audio_analysis::AudioSemanticArtifactKind::CandidateMidi)
    );
    for output in &outcome.outputs {
        assert_eq!(digest(&fs::read(&output.path).unwrap()), output.sha256);
    }
    let launches = fixture.launches();
    let resumed = audio_midi::resume(
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
fn polyphonic_and_empty_observations_preserve_uncertainty() {
    for mode in ["polyphonic", "empty"] {
        let fixture = Fixture::new();
        fixture.mode(mode);
        let outcome = fixture.run();
        let report = midi(&outcome);
        assert_eq!(report["result"]["status"], "candidate");
        let notes = report["result"]["observation"]["notes"].as_array().unwrap();
        if mode == "polyphonic" {
            assert_eq!(notes.len(), 2);
            assert_eq!(notes[0]["start"], notes[1]["start"]);
            assert_ne!(notes[0]["pitch"], notes[1]["pitch"]);
        } else {
            assert!(notes.is_empty());
            assert!(normalized(&outcome).timelines.is_empty());
            assert_eq!(
                normalized(&outcome).status,
                aniflow::audio_analysis::AudioAnalysisStatus::Partial
            );
        }
        assert_eq!(report["confidence"]["kind"], "unavailable");
    }
}

#[test]
fn unsupported_rate_channels_duration_and_silence_are_explicitly_unavailable() {
    for (rate, frames, channels, silent, reason) in [
        (16000, 16000, 1, false, "unsupported_sample_rate"),
        (22050, 22050, 2, false, "unsupported_channels"),
        (22050, 2646001, 1, false, "duration_limit"),
        (22050, 22050, 1, true, "silent_input"),
    ] {
        let fixture = Fixture::new();
        write_pcm(&fixture.input, rate, frames, channels, silent);
        let source = fs::read(&fixture.input).unwrap();
        let outcome = fixture.run();
        let report = midi(&outcome);
        assert_eq!(
            report["result"],
            json!({"status": "unavailable", "reason": reason})
        );
        assert_eq!(report["commands"].as_array().unwrap().len(), 2);
        assert!(report["raw_observation"].is_null());
        assert!(
            fixture.launches().is_empty(),
            "must skip estimator for {reason}"
        );
        assert!(normalized(&outcome).semantic_artifacts.is_empty());
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn stale_or_reconfigured_dependencies_cannot_reuse_accepted_checkpoints() {
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
        audio_midi::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| progress = true
        )
        .is_err()
    );
    assert!(!progress);
    fixture.settings.adapter.sha256 = digest(b"# changed adapter identity\n");
    assert!(
        audio_midi::resume(
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
    let original_model = fs::read(&fixture.settings.model.path).unwrap();
    let mut changed_model = original_model.clone();
    changed_model[0] ^= 1;
    fs::write(&fixture.settings.model.path, &changed_model).unwrap();
    let failed = audio_midi::plan(&fixture.request(), &CancellationToken::default()).unwrap_err();
    assert!(
        failed
            .preflight
            .unwrap()
            .diagnostics
            .iter()
            .any(|item| item.code == audio_midi::AudioMidiDiagnosticCode::ModelDigestMismatch)
    );
    fixture.settings.model.sha256 = digest(&changed_model);
    assert!(
        audio_midi::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| progress = true
        )
        .is_err()
    );
    assert!(!progress);
    fs::write(&fixture.settings.model.path, &original_model).unwrap();
    fixture.settings.model.sha256 = digest(&original_model);
    let probe_path = fixture.root.path().join("probe.json");
    let original_probe = fs::read(&probe_path).unwrap();
    let mut changed_probe: Value = serde_json::from_slice(&original_probe).unwrap();
    changed_probe["runtime_sha256"] = digest(b"changed synthetic dependency").into();
    fs::write(&probe_path, serde_json::to_vec(&changed_probe).unwrap()).unwrap();
    let failed = audio_midi::plan(&fixture.request(), &CancellationToken::default()).unwrap_err();
    assert!(
        failed
            .preflight
            .unwrap()
            .diagnostics
            .iter()
            .any(|item| item.code == audio_midi::AudioMidiDiagnosticCode::RuntimeMismatch)
    );
    assert!(
        audio_midi::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| progress = true
        )
        .is_err()
    );
    fs::write(&probe_path, original_probe).unwrap();
    fs::remove_file(&fixture.settings.python.executable).unwrap();
    assert!(audio_midi::plan(&fixture.request(), &CancellationToken::default()).is_err());
    assert_eq!(fixture.launches(), launches);
    assert_eq!(
        fs::read(outcome.run_directory.join("plan/plan.json")).unwrap(),
        plan_bytes
    );
}

#[test]
fn malformed_notes_process_failure_and_staged_mutation_never_publish() {
    for mode in [
        "malformed",
        "empty_stdout",
        "nonfinite",
        "out_of_order",
        "out_of_range",
        "source_mismatch",
        "bad_velocity",
        "same_pitch_overlap",
        "flood",
        "sleep",
        "nonzero",
        "mutate_model",
        "mutate_source",
        "undeclared_file",
    ] {
        let fixture = Fixture::new();
        let source = fs::read(&fixture.input).unwrap();
        fixture.mode(mode);
        let mut run = None;
        let error = audio_midi::run(
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
            fixture.root.path().join("midi-started").exists(),
            "{mode}: expected estimator-stage failure: {error:?}"
        );
        assert_ne!(
            status_v3(&directory).unwrap().payload.state,
            PipelineV3RunState::Complete
        );
        assert!(
            !directory.join("artifacts/audio-midi/midi.json").exists(),
            "{mode}: partial report published"
        );
        assert!(
            !directory
                .join("artifacts/audio-midi/analysis.json")
                .exists(),
            "{mode}: partial normalized report published"
        );
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn cancellation_retains_inspection_checkpoint_and_resume_reruns_only_midi() {
    let fixture = Fixture::new();
    fixture.mode("sleep");
    let cancellation = CancellationToken::default();
    let token = cancellation.clone();
    let marker = fixture.root.path().join("midi-started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            marker.exists(),
            "MIDI child did not reach cancellation marker"
        );
        token.cancel();
    });
    let mut run = None;
    let result = audio_midi::run(
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
    let resumed = audio_midi::resume(
        fixture.request(),
        directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed.reused_stages, ["inspect_audio"]);
    assert_eq!(resumed.executed_stages.len(), 1);
    midi(&resumed);
}

#[test]
fn preflight_reports_missing_symlink_and_cancelled_dependencies_without_runs() {
    let fixture = Fixture::new();
    let cancellation = CancellationToken::default();
    cancellation.cancel();
    let result = audio_midi::preflight(&fixture.settings, &fixture.tools, &cancellation).unwrap();
    assert!(!result.is_ready());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.code == audio_midi::AudioMidiDiagnosticCode::Cancelled)
    );
    fs::remove_file(&fixture.settings.model.path).unwrap();
    let result = audio_midi::preflight(
        &fixture.settings,
        &fixture.tools,
        &CancellationToken::default(),
    )
    .unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.code == audio_midi::AudioMidiDiagnosticCode::MissingModel)
    );
    std::os::unix::fs::symlink(&fixture.settings.adapter.path, &fixture.settings.model.path)
        .unwrap();
    let result = audio_midi::preflight(
        &fixture.settings,
        &fixture.tools,
        &CancellationToken::default(),
    )
    .unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.code == audio_midi::AudioMidiDiagnosticCode::InvalidModel)
    );
    assert!(fixture.launches().is_empty());
    assert!(!fixture.root.path().join("runs").exists());
}

#[test]
fn midi_preserves_provider_neutral_selected_stem_scope() {
    let fixture = Fixture::new();
    let separation = TempDir::new().unwrap();
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/smoke-audio-stem.py"))
        .arg("--aniflow")
        .arg(env!("CARGO_BIN_EXE_aniflow"))
        .arg("--prepare-only")
        .arg(separation.path())
        .args(["--provider", "generic", "--rate", "22050"])
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
    let outcome = audio_midi::run(
        MidiRequest::new(inspection, fixture.settings.clone()),
        Some(fixture.root.path().join("runs")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(outcome.executed_stages.len(), 3);
    let report = midi(&outcome);
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
    assert_eq!(fs::read(mix).unwrap(), original);
}
