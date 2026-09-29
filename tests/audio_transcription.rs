#![cfg(unix)]

//! Synthetic whisper/model identities prove orchestration, never recognition quality.
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
use aniflow::audio_stem::StemSelection;
use aniflow::audio_transcription::{self, TranscriptionConfiguration, TranscriptionRequest};
use aniflow::timed_text::{
    self, ConversionLossKind, ConversionOptions, ImportContext, TextProvenance, TimedTextFormat,
};
use aniflow::{
    CancellationToken, PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunState, SideEffect,
    status_v3,
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

const SYNTHETIC_WHISPER: &str = r#"#!/usr/bin/python3
import json, pathlib, sys, time
root = pathlib.Path(__FIXTURE_ROOT__)
args = sys.argv[1:]
mode = (root / 'mode').read_text()
if args == ['--version']:
    print('whisper.cpp version: ' + ('9.9.9' if mode == 'wrong_version' else '1.8.7'))
    raise SystemExit(0)
(root / 'transcription-started').write_text('yes')
with (root / 'launches').open('a') as log:
    log.write(json.dumps(args) + '\n')
assert args[args.index('--output-file') + 1] == '-'
assert '--output-json' in args and '--no-gpu' in args
assert '--no-flash-attn' in args and '--no-fallback' in args
assert args[args.index('--language') + 1] == 'en'
assert args[args.index('--threads') + 1] == '1'
assert args[args.index('--processors') + 1] == '1'
assert '--output-json-full' not in args
if mode == 'sleep': time.sleep(120)
if mode == 'nonzero': raise SystemExit(23)
if mode == 'flood':
    print('x' * 100000)
    raise SystemExit(0)
if mode == 'malformed':
    print('{not-json')
    raise SystemExit(0)
value = {
    'systeminfo': 'WHISPER : COREML = 0 | OPENVINO = 0 | CPU : synthetic |',
    'model': {'type': 'tiny', 'multilingual': False, 'vocab': 51864,
        'audio': {'ctx': 1500, 'state': 384, 'head': 6, 'layer': 4},
        'text': {'ctx': 448, 'state': 384, 'head': 6, 'layer': 4}, 'mels': 80, 'ftype': 1},
    'params': {'model': args[args.index('--model') + 1], 'language': 'en', 'translate': False},
    'result': {'language': 'en'},
    'transcription': [
        {'timestamps': {'from': '00:00:00,000', 'to': '00:00:00,500'},
         'offsets': {'from': 0, 'to': 500}, 'text': ' synthetic hello'},
        {'timestamps': {'from': '00:00:00,700', 'to': '00:00:01,200'},
         'offsets': {'from': 700, 'to': 1200}, 'text': ' café synthetic words'}]}
if mode == 'empty': value['transcription'] = []
if mode == 'out_of_range':
    value['transcription'][1]['timestamps']['to'] = '00:00:10,000'
    value['transcription'][1]['offsets']['to'] = 10000
if mode == 'mismatched_clock': value['transcription'][0]['offsets']['to'] = 510
if mode == 'out_of_order': value['transcription'].reverse()
if mode == 'wrong_model': value['params']['model'] = '/synthetic/unbound/model.bin'
print(json.dumps(value))
"#;

struct Fixture {
    root: TempDir,
    input: PathBuf,
    tools: AudioInspectionConfiguration,
    settings: TranscriptionConfiguration,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("transcription café 雪 ")
            .tempdir()
            .unwrap();
        let directory = root.path().canonicalize().unwrap();
        let pin = |name: &str| {
            let path = directory.join(name);
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
        let whisper = directory.join("synthetic-whisper");
        let script = SYNTHETIC_WHISPER.replace(
            "__FIXTURE_ROOT__",
            &serde_json::to_string(&directory).unwrap(),
        );
        executable(&whisper, &script);
        let model = directory.join("synthetic-tiny.en.bin");
        let model_bytes = b"Synthetic model identity for ABI conformance; not inference weights.\n";
        fs::write(&model, model_bytes).unwrap();
        let settings = TranscriptionConfiguration::from_json_slice(&serde_json::to_vec(&json!({
            "schema": "aniflow.audio-transcription.configuration/v1",
            "whisper": {"executable": whisper, "version": "1.8.7", "sha256": digest(script.as_bytes())},
            "model": {"path": model, "sha256": digest(model_bytes), "byte_size": model_bytes.len(), "model_id": "tiny.en", "revision": "synthetic-fixture-v1"},
            "language": "en", "threads": 1, "tool_timeout_milliseconds": 5000, "maximum_tool_output_bytes": 65536
        })).unwrap()).unwrap();
        let input = directory.join("source.wav");
        write_pcm(&input, 16000, 32000);
        fs::write(directory.join("mode"), "ok").unwrap();
        Self {
            root,
            input,
            tools,
            settings,
        }
    }

    fn request(&self) -> TranscriptionRequest {
        TranscriptionRequest::new(
            AudioInspectionRequest::new(
                &self.input,
                self.tools.clone(),
                env!("CARGO_BIN_EXE_aniflow"),
            )
            .with_execution_limits(audio_transcription::default_execution_limits()),
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
        audio_transcription::run(
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

fn write_pcm(path: &Path, rate: u32, frames: u32) {
    let size = frames * 2;
    let mut bytes = Vec::with_capacity(size as usize + 44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(size + 36).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    for frame in 0..frames {
        bytes
            .extend_from_slice(&(if frame % 64 < 32 { 1200_i16 } else { -1200_i16 }).to_le_bytes());
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

fn transcription(outcome: &PipelineV3RunOutcome) -> audio_transcription::AudioTranscriptionReport {
    audio_transcription::AudioTranscriptionReport::from_json_slice(
        &fs::read(artifact(outcome, "transcription")).unwrap(),
    )
    .unwrap()
}

#[test]
fn observed_segments_preserve_source_evidence_export_authority_and_exact_resume() {
    let fixture = Fixture::new();
    let source = fs::read(&fixture.input).unwrap();
    let plan =
        audio_transcription::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.stages.len(), 2);
    assert!(plan.payload.policy.offline);
    assert!(
        plan.payload
            .policy
            .allowed_side_effects
            .contains(&SideEffect::Ai)
    );
    assert!(
        !plan
            .payload
            .policy
            .allowed_side_effects
            .contains(&SideEffect::Network)
    );
    assert!(fixture.launches().is_empty());
    assert!(!fixture.root.path().join("runs").exists());
    let outcome = fixture.run();
    assert_eq!(outcome.executed_stages.len(), 2);
    let report = transcription(&outcome);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["schema"], "aniflow.audio-transcription/v1");
    assert_eq!(value["source"]["artifact"]["sha256"], digest(&source));
    assert_eq!(value["result"]["status"], "observed");
    assert_eq!(value["result"]["observation"]["detected_language"], "en");
    assert_eq!(
        value["result"]["observation"]["segments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        value["result"]["observation"]["segments"][0]["end"],
        json!({"numerator":1,"denominator":2})
    );
    assert_eq!(value["word_timing"]["status"], "unavailable");
    assert_eq!(value["confidence"]["kind"], "unavailable");
    assert_eq!(value["provenance"], "probabilistic");
    assert_eq!(
        value["settings"]["model"]["sha256"],
        fixture.settings.model.sha256
    );
    assert_eq!(value["scope"]["channels"], json!([0]));
    let document = report.timed_text.as_ref().unwrap();
    assert!(matches!(
        document.provenance,
        TextProvenance::ObservedTranscript { .. }
    ));
    assert_eq!(document.cues[0].text, " synthetic hello");
    let export = timed_text::encode(
        document,
        TimedTextFormat::Webvtt,
        &ConversionOptions {
            allow_losses: vec![ConversionLossKind::Language],
        },
    )
    .unwrap();
    assert!(matches!(
        export.document.provenance,
        TextProvenance::ObservedTranscript { .. }
    ));
    assert!(
        export
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::Language)
    );
    assert_eq!(
        timed_text::decode(
            &export.bytes,
            TimedTextFormat::Webvtt,
            &ImportContext::default()
        )
        .unwrap()
        .document
        .cues
        .len(),
        2
    );
    let report_path = artifact(&outcome, "transcription");
    let original_report = fs::read(&report_path).unwrap();
    let exported = audio_transcription::export_transcript_file(
        &report_path,
        TimedTextFormat::Webvtt,
        &fixture.root.path().join("export"),
        &ConversionOptions {
            allow_losses: vec![ConversionLossKind::Language],
        },
    )
    .unwrap();
    assert_eq!(exported.transcription_report.id, "transcription_report");
    assert_eq!(
        exported.transcription_report.sha256,
        digest(&original_report)
    );
    assert_eq!(
        exported.transcription_report.byte_size,
        original_report.len() as u64
    );
    assert_ne!(
        exported.transcription_report.sha256,
        exported.conversion.report.input.sha256
    );
    assert_eq!(
        exported.conversion.report.output.sha256,
        digest(&fs::read(exported.conversion.payload_path).unwrap())
    );
    let exported_document = timed_text::decode(
        &fs::read(exported.conversion.output_document_path).unwrap(),
        TimedTextFormat::Json,
        &ImportContext::default(),
    )
    .unwrap()
    .document;
    assert!(matches!(
        exported_document.provenance,
        TextProvenance::ObservedTranscript { .. }
    ));
    assert_eq!(fs::read(report_path).unwrap(), original_report);
    let analysis = normalized(&outcome);
    assert_eq!(analysis.source, report.source);
    assert!(
        analysis.semantic_artifacts.iter().any(|item| item.kind
            == aniflow::audio_analysis::AudioSemanticArtifactKind::ObservedTranscript)
    );
    for output in &outcome.outputs {
        assert_eq!(digest(&fs::read(&output.path).unwrap()), output.sha256);
    }
    let launches = fixture.launches();
    let resumed = audio_transcription::resume(
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

    // Coherent digest rewrites exercise policy validation, not merely a stale hash.
    let mut missing_ai = plan.clone();
    missing_ai
        .payload
        .policy
        .allowed_side_effects
        .retain(|effect| *effect != SideEffect::Ai);
    missing_ai.plan_sha256 =
        digest(&serde_json::to_vec(&serde_json::to_value(&missing_ai.payload).unwrap()).unwrap());
    assert!(missing_ai.validate().is_err());
    let mut offline_network = plan;
    offline_network
        .payload
        .policy
        .allowed_side_effects
        .push(SideEffect::Network);
    offline_network
        .payload
        .policy
        .allowed_side_effects
        .sort_unstable();
    offline_network.plan_sha256 = digest(
        &serde_json::to_vec(&serde_json::to_value(&offline_network.payload).unwrap()).unwrap(),
    );
    assert!(offline_network.validate().is_err());
}

#[test]
fn empty_observation_and_unsupported_source_never_invent_cues() {
    let fixture = Fixture::new();
    fixture.mode("empty");
    let outcome = fixture.run();
    let report = transcription(&outcome);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["result"]["status"], "empty");
    assert!(
        value["result"]["observation"]["segments"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(report.timed_text.is_none());
    assert!(normalized(&outcome).semantic_artifacts.is_empty());
    let export = fixture.root.path().join("empty-export");
    assert!(
        audio_transcription::export_transcript_file(
            &artifact(&outcome, "transcription"),
            TimedTextFormat::Json,
            &export,
            &ConversionOptions::default()
        )
        .is_err()
    );
    assert!(!export.exists());
    assert!(!fixture.launches().is_empty());
    let fixture = Fixture::new();
    write_pcm(&fixture.input, 8000, 16000);
    let source = fs::read(&fixture.input).unwrap();
    let report = transcription(&fixture.run());
    assert_eq!(
        serde_json::to_value(&report).unwrap()["result"]["reason"],
        "unsupported_sample_rate"
    );
    assert!(report.timed_text.is_none());
    assert!(fixture.launches().is_empty());
    assert_eq!(fs::read(&fixture.input).unwrap(), source);
    let fixture = Fixture::new();
    let mut silent = fs::read(&fixture.input).unwrap();
    silent[44..].fill(0);
    fs::write(&fixture.input, &silent).unwrap();
    let report = transcription(&fixture.run());
    assert_eq!(
        serde_json::to_value(&report).unwrap()["result"]["reason"],
        "silent_input"
    );
    assert!(report.timed_text.is_none());
    assert!(fixture.launches().is_empty());
    assert_eq!(fs::read(&fixture.input).unwrap(), silent);
}

#[test]
fn stale_missing_and_re_pinned_dependencies_cannot_reuse_prior_authority() {
    let mut fixture = Fixture::new();
    let outcome = fixture.run();
    let launches = fixture.launches();
    let model = fs::read(&fixture.settings.model.path).unwrap();
    let original_pin = fixture.settings.model.clone();
    fs::write(&fixture.settings.model.path, b"changed synthetic model").unwrap();
    let mut emitted_progress = false;
    let failure = audio_transcription::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| emitted_progress = true,
    )
    .unwrap_err();
    assert!(failure.preflight.is_some());
    assert!(!emitted_progress);
    fixture.settings.model.sha256 = digest(b"changed synthetic model");
    fixture.settings.model.byte_size = b"changed synthetic model".len() as u64;
    assert!(
        audio_transcription::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| emitted_progress = true
        )
        .is_err()
    );
    assert!(!emitted_progress);
    fs::write(&fixture.settings.model.path, &model).unwrap();
    fixture.settings.model = original_pin;
    fs::remove_file(&fixture.settings.model.path).unwrap();
    let preflight = audio_transcription::preflight(
        &fixture.settings,
        &fixture.tools,
        &CancellationToken::default(),
    )
    .unwrap();
    assert!(!preflight.ready);
    assert!(
        serde_json::to_value(&preflight).unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "missing_model")
    );
    fs::write(&fixture.settings.model.path, model).unwrap();
    let executable_bytes = fs::read(&fixture.settings.whisper.executable).unwrap();
    fs::write(
        &fixture.settings.whisper.executable,
        b"changed synthetic executable",
    )
    .unwrap();
    assert!(
        audio_transcription::plan(&fixture.request(), &CancellationToken::default())
            .unwrap_err()
            .preflight
            .is_some()
    );
    fs::write(&fixture.settings.whisper.executable, executable_bytes).unwrap();
    fixture.mode("wrong_version");
    assert!(
        audio_transcription::plan(&fixture.request(), &CancellationToken::default())
            .unwrap_err()
            .preflight
            .is_some()
    );
    fs::remove_file(&fixture.settings.whisper.executable).unwrap();
    assert!(
        audio_transcription::plan(&fixture.request(), &CancellationToken::default())
            .unwrap_err()
            .preflight
            .is_some()
    );
    assert_eq!(fixture.launches(), launches);
}

#[test]
fn malformed_hallucinated_or_bounded_tool_failures_never_publish_transcripts() {
    for mode in [
        "malformed",
        "out_of_range",
        "mismatched_clock",
        "out_of_order",
        "wrong_model",
        "flood",
        "sleep",
        "nonzero",
    ] {
        let fixture = Fixture::new();
        let source = fs::read(&fixture.input).unwrap();
        fixture.mode(mode);
        let mut run = None;
        let failure = audio_transcription::run(
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
        let directory =
            run.unwrap_or_else(|| panic!("{mode}: expected durable run, got {failure:?}"));
        assert!(
            fixture.root.path().join("transcription-started").exists(),
            "{mode}: must fail at inference, got {failure:?}"
        );
        assert_ne!(
            status_v3(&directory).unwrap().payload.state,
            PipelineV3RunState::Complete
        );
        assert!(
            !directory
                .join("artifacts/audio-transcription/transcription.json")
                .exists()
        );
        assert!(
            !directory
                .join("artifacts/audio-transcription/analysis.json")
                .exists()
        );
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn cancellation_preserves_recoverable_technical_stage_without_complete_transcript() {
    let fixture = Fixture::new();
    fixture.mode("sleep");
    let cancellation = CancellationToken::default();
    let token = cancellation.clone();
    let marker = fixture.root.path().join("transcription-started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            marker.exists(),
            "inference did not reach cancellation marker"
        );
        token.cancel();
    });
    let mut run = None;
    let result = audio_transcription::run(
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
    let resumed = audio_transcription::resume(
        fixture.request(),
        directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed.reused_stages, ["inspect_audio"]);
    assert_eq!(resumed.executed_stages.len(), 1);
    transcription(&resumed);
}

#[test]
fn canonical_cli_reports_plan_and_typed_missing_model_without_execution() {
    let fixture = Fixture::new();
    let tools = fixture.root.path().join("tools.json");
    let settings = fixture.root.path().join("transcription.json");
    fs::write(&tools, serde_json::to_vec(&fixture.tools).unwrap()).unwrap();
    fs::write(&settings, serde_json::to_vec(&fixture.settings).unwrap()).unwrap();
    let plan = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args([
            "--output",
            "json",
            "audio",
            "plan",
            "--analysis",
            "transcription",
            "--input",
        ])
        .arg(&fixture.input)
        .arg("--configuration")
        .arg(&tools)
        .arg("--transcription-configuration")
        .arg(&settings)
        .output()
        .unwrap();
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let value: Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(value["command"], "audio_plan");
    assert!(fixture.launches().is_empty());
    fs::remove_file(&fixture.settings.model.path).unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "audio", "transcribe", "--input"])
        .arg(&fixture.input)
        .arg("--configuration")
        .arg(&tools)
        .arg("--transcription-configuration")
        .arg(&settings)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let value: Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(value["command"], "audio_transcribe");
    assert!(value.to_string().contains("missing_model"));
    assert!(fixture.launches().is_empty());
}

#[test]
fn unsupported_stereo_stem_keeps_verified_provider_neutral_lineage() {
    let fixture = Fixture::new();
    let separation = TempDir::new().unwrap();
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/smoke-audio-stem.py"))
        .arg("--aniflow")
        .arg(env!("CARGO_BIN_EXE_aniflow"))
        .arg("--prepare-only")
        .arg(separation.path())
        .args(["--provider", "generic", "--rate", "16000"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let descriptor: Value = serde_json::from_slice(&output.stdout).unwrap();
    let input = PathBuf::from(descriptor["mix"].as_str().unwrap());
    let original = fs::read(&input).unwrap();
    let stem = descriptor["stems"][0].as_str().unwrap();
    let selection = StemSelection::new(
        descriptor["separation_run"].as_str().unwrap(),
        descriptor["stage"].as_str().unwrap(),
        stem,
    );
    let inspection =
        AudioInspectionRequest::new(&input, fixture.tools.clone(), env!("CARGO_BIN_EXE_aniflow"))
            .with_execution_limits(audio_transcription::default_execution_limits())
            .with_stem_selection(selection);
    let outcome = audio_transcription::run(
        TranscriptionRequest::new(inspection, fixture.settings.clone()),
        Some(fixture.root.path().join("runs")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(outcome.executed_stages.len(), 3);
    let report = transcription(&outcome);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["result"]["reason"], "unsupported_channels");
    assert_eq!(value["source"]["stem"]["id"], stem);
    assert_eq!(value["scope"]["stem_id"], stem);
    assert_eq!(value["scope"]["channels"], json!([0, 1]));
    assert_eq!(
        value["source"]["stem"]["original_mix"]["sha256"],
        digest(&original)
    );
    assert_eq!(normalized(&outcome).source.stem.unwrap().id, stem);
    assert!(fixture.launches().is_empty());
    assert_eq!(fs::read(&input).unwrap(), original);
}
