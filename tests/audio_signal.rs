#![cfg(unix)]

//! Fake pinned tools establish orchestration/refusal semantics, not meter accuracy.
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::audio_analysis::AudioAnalysis;
use aniflow::audio_inspection::{
    self, AudioInspectionConfiguration, AudioInspectionRequest, AudioToolPin,
};
use aniflow::audio_signal::{
    self, AudioSignalMeasurements, SignalAnalysisConfiguration, SignalAnalysisRequest,
};
use aniflow::{CancellationToken, PipelineV3RunProgress, PipelineV3RunState, status_v3};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

const TOOL: &str = r#"#!/usr/bin/python3
import hashlib, json, pathlib, sys, time, wave
root = pathlib.Path(__file__).parent
tool = pathlib.Path(__file__).name
mode = (root / "mode").read_text()
args = sys.argv[1:]
if args == ["-version"]:
    print(tool + " version " + ("9.9.9" if mode == "wrong_version" else "6.1.1") + " synthetic")
    raise SystemExit(0)
source = pathlib.Path(args[args.index("-i") + 1])
with wave.open(str(source), "rb") as audio:
    rate, channels, frames = audio.getframerate(), audio.getnchannels(), audio.getnframes()
    pcm = audio.readframes(frames)
with (root / "launches").open("a") as log:
    log.write(json.dumps(args) + "\n")
if tool == "ffprobe":
    print(json.dumps({"streams": [{"index": 0, "codec_type": "audio", "codec_name": "pcm_s16le",
        "sample_fmt": "s16", "sample_rate": str(rate), "channels": channels, "bits_per_sample": 16,
        "time_base": "1/" + str(rate), "duration_ts": frames, "bit_rate": str(rate * channels * 16)}],
        "format": {"format_name": "wav", "nb_streams": 1}}))
    raise SystemExit(0)
if not any("ebur128=" in arg for arg in args):
    print("SHA256=" + hashlib.sha256(pcm).hexdigest())
    raise SystemExit(0)
peak_only = any("apad=" in arg for arg in args)
if not peak_only:
    (root / "signal-started").write_text("yes")
    if mode == "sleep":
        time.sleep(120)
    if mode == "nonzero":
        raise SystemExit(23)
    if mode == "flood":
        print("x" * 100000)
        raise SystemExit(0)
    if mode == "missing":
        raise SystemExit(0)
    for index in range(frames // (rate // 10)):
        pts = index * (rate // 10) + (1 if mode == "bad_timestamp" else 0)
        print(f"frame:{index} pts:{pts} pts_time:{pts / rate}")
        print("lavfi.r128.S=" + ("nan" if mode == "nonfinite" else "-28.700"))
value = "nan" if mode == "bad_summary" else "-28.7"
print(f"""[Parsed_ebur128_0] Summary:

  Integrated loudness:
    I: {value} LUFS
    Threshold: -38.7 LUFS

  Loudness range:
    LRA: 0.0 LU
    Threshold: -48.7 LUFS
    LRA low: -28.7 LUFS
    LRA high: -28.7 LUFS

  True peak:
    Peak: -28.7 dBFS
""", file=sys.stderr)
"#;

#[test]
fn published_signal_fixture_parses_with_semantic_validation() {
    let document =
        include_bytes!("../docs/contracts/examples/audio-signal-measurements-v1.example.json");
    let report = AudioSignalMeasurements::from_json_slice(document).unwrap();
    let encoded = report.canonical_json_bytes().unwrap();
    assert_eq!(
        AudioSignalMeasurements::from_json_slice(&encoded).unwrap(),
        report
    );
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Fixture {
    root: TempDir,
    input: PathBuf,
    tools: AudioInspectionConfiguration,
    settings: SignalAnalysisConfiguration,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("signal ü fixture ")
            .tempdir()
            .unwrap();
        fs::write(root.path().join("mode"), "ok").unwrap();
        let pin = |name: &str| {
            let executable = root.path().join(name);
            fs::write(&executable, TOOL).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            AudioToolPin {
                executable,
                version: "6.1.1".to_owned(),
                sha256: digest(TOOL.as_bytes()),
            }
        };
        let tools = AudioInspectionConfiguration {
            schema: "aniflow.audio-inspection.configuration/v1".to_owned(),
            ffmpeg: pin("ffmpeg"),
            ffprobe: pin("ffprobe"),
            tool_timeout_milliseconds: 2000,
            maximum_tool_output_bytes: 65536,
        };
        let settings = SignalAnalysisConfiguration::from_json_slice(
            br#"{"schema":"aniflow.audio-signal.configuration/v1"}"#,
        )
        .unwrap();
        let input = root.path().join("synthetic.wav");
        // Exact 3.2s mono PCM16 at8kHz; alternating square-wave samples.
        let data_bytes = 25600_u32 * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8000_u32.to_le_bytes());
        wav.extend_from_slice(&16000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for frame in 0..25600 {
            wav.extend_from_slice(
                &(if frame % 2 == 0 { 1200_i16 } else { -1200_i16 }).to_le_bytes(),
            );
        }
        fs::write(&input, wav).unwrap();
        Self {
            root,
            input,
            tools,
            settings,
        }
    }
    fn inspection(&self) -> AudioInspectionRequest {
        AudioInspectionRequest::new(
            &self.input,
            self.tools.clone(),
            env!("CARGO_BIN_EXE_aniflow"),
        )
    }
    fn request(&self) -> SignalAnalysisRequest {
        SignalAnalysisRequest::new(self.inspection(), self.settings.clone())
    }
    fn mode(&self, mode: &str) {
        fs::write(self.root.path().join("mode"), mode).unwrap();
    }
    fn run(&self) -> aniflow::PipelineV3RunOutcome {
        audio_signal::run(
            self.request(),
            Some(self.root.path().join("runs")),
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap()
    }
}

#[test]
fn signal_plan_run_evidence_and_exact_two_stage_resume() {
    let fixture = Fixture::new();
    let source = fs::read(&fixture.input).unwrap();
    audio_signal::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert!(!fixture.root.path().join("runs").exists());
    assert!(!fixture.root.path().join("launches").exists());
    let outcome = fixture.run();
    assert_eq!(outcome.executed_stages, ["inspect_audio", "measure_audio"]);
    let signal = outcome
        .outputs
        .iter()
        .find(|output| output.id == "signal")
        .unwrap();
    let report = fs::read(&signal.path).unwrap();
    AudioSignalMeasurements::from_json_slice(&report).unwrap();
    let value: Value = serde_json::from_slice(&report).unwrap();
    assert_eq!(value["source"]["artifact"]["sha256"], digest(&source));
    assert_eq!(value["source"]["frame_count"], 25600);
    assert_eq!(value["short_term"].as_array().unwrap().len(), 3);
    for (pointer, replacement) in [
        ("/source/frame_count", serde_json::json!(1)),
        ("/integrated_loudness/value/value", serde_json::json!(-70)),
        ("/true_peak/value/value", serde_json::json!(-120)),
        ("/channels/0/rms_ratio", serde_json::json!(0.99)),
        ("/short_term/0/range/start", serde_json::json!(1)),
        ("/method/short_term_window_frames", serde_json::json!(1)),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            AudioSignalMeasurements::from_json_slice(&serde_json::to_vec(&invalid).unwrap())
                .is_err(),
            "accepted {pointer}"
        );
    }
    let analysis = outcome
        .outputs
        .iter()
        .find(|output| output.id == "analysis")
        .unwrap();
    let analysis = AudioAnalysis::from_json_slice(&fs::read(&analysis.path).unwrap()).unwrap();
    assert!(
        analysis
            .capabilities
            .iter()
            .any(|capability| capability.capability.id == "aniflow/audio-signal-measurements")
    );
    for output in &outcome.outputs {
        assert_eq!(digest(&fs::read(&output.path).unwrap()), output.sha256);
    }
    let launches = fs::read(fixture.root.path().join("launches")).unwrap();
    let resumed = audio_signal::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed.reused_stages, ["inspect_audio", "measure_audio"]);
    assert!(resumed.executed_stages.is_empty());
    assert_eq!(
        fs::read(fixture.root.path().join("launches")).unwrap(),
        launches
    );
    assert_eq!(fs::read(&fixture.input).unwrap(), source);
}

#[test]
fn changed_settings_tools_and_profile_cannot_reuse_signal_authority() {
    let mut fixture = Fixture::new();
    let outcome = fixture.run();
    fixture.settings.silence_threshold_pcm += 1;
    assert!(
        audio_signal::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
    fixture.settings.silence_threshold_pcm -= 1;
    fs::write(
        &fixture.tools.ffmpeg.executable,
        format!("{TOOL}\n# changed\n"),
    )
    .unwrap();
    assert!(
        audio_signal::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .unwrap_err()
        .preflight
        .is_some()
    );
    fs::write(&fixture.tools.ffmpeg.executable, TOOL).unwrap();
    let technical = audio_inspection::run(
        fixture.inspection(),
        Some(fixture.root.path().join("technical")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert!(
        audio_signal::resume(
            fixture.request(),
            &technical.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
}

#[test]
fn malformed_nonfinite_missing_and_bounded_tool_failures_never_complete() {
    for mode in [
        "nonfinite",
        "bad_summary",
        "bad_timestamp",
        "missing",
        "flood",
        "sleep",
        "nonzero",
    ] {
        let mut fixture = Fixture::new();
        fixture.mode(mode);
        if mode == "sleep" {
            fixture.tools.tool_timeout_milliseconds = 250;
        }
        let mut run = None;
        let result = audio_signal::run(
            fixture.request(),
            Some(fixture.root.path().join("runs")),
            &CancellationToken::default(),
            |progress| {
                if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
                    run = Some(run_directory.clone());
                }
            },
        );
        assert!(result.is_err(), "accepted {mode}");
        let manifest = status_v3(run.expect("durable run should exist")).unwrap();
        assert_ne!(manifest.payload.state, PipelineV3RunState::Complete);
    }
}

#[test]
fn cancellation_during_signal_stage_retains_a_recoverable_checkpoint() {
    let fixture = Fixture::new();
    fixture.mode("sleep");
    let cancellation = CancellationToken::default();
    let token = cancellation.clone();
    let marker = fixture.root.path().join("signal-started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        token.cancel();
    });
    let mut run = None;
    let result = audio_signal::run(
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
    let resumed = audio_signal::resume(
        fixture.request(),
        directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed.reused_stages, ["inspect_audio"]);
    assert_eq!(resumed.executed_stages, ["measure_audio"]);
}

#[test]
fn canonical_signal_cli_requires_selection_settings_and_structured_preflight() {
    let fixture = Fixture::new();
    let tools = fixture.root.path().join("tools.json");
    let settings = fixture.root.path().join("settings.json");
    fs::write(&tools, serde_json::to_vec(&fixture.tools).unwrap()).unwrap();
    fs::write(&settings, serde_json::to_vec(&fixture.settings).unwrap()).unwrap();
    fixture.mode("wrong_version");
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args([
            "--output",
            "json",
            "audio",
            "analyze",
            "--analysis",
            "signal",
            "--input",
        ])
        .arg(&fixture.input)
        .arg("--configuration")
        .arg(&tools)
        .arg("--signal-configuration")
        .arg(&settings)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "audio_analyze");
    assert!(error.to_string().contains("tool_version_mismatch"));
    let missing = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["audio", "plan", "--analysis", "signal", "--input"])
        .arg(&fixture.input)
        .arg("--configuration")
        .arg(&tools)
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("--signal-configuration")
    );
}
