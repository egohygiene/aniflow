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
use aniflow::{
    CancellationToken, PipelineV3Configuration, PipelineV3Plan, PipelineV3RunProgress,
    PipelineV3RunState, PipelineV3Workspace, ProviderManifest, status_v3,
};
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
if any("astats=" in arg for arg in args):
    (root / "signal-peak-started").write_text("yes")
    count = 4 * (frames + rate // 10)
    if mode == "astats_bad_count":
        count -= 1
    for line in ["Overall", "Peak level dB: -28.700000", "Number of samples: " + str(count),
                 "Number of NaNs: 0.000000", "Number of Infs: 0.000000"]:
        print("[Parsed_astats_2 @ 0x1234] " + line, file=sys.stderr)
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
    assert_eq!(report.schema, "aniflow.audio-signal-measurements/v1");
    assert_eq!(report.provider.version, "1.0.0");
    let encoded = report.canonical_json_bytes().unwrap();
    assert_eq!(
        AudioSignalMeasurements::from_json_slice(&encoded).unwrap(),
        report
    );
}

#[test]
fn published_v2_signal_fixture_is_distinct_from_historical_v1() {
    let document =
        include_bytes!("../docs/contracts/examples/audio-signal-measurements-v2.example.json");
    let report = AudioSignalMeasurements::from_json_slice(document).unwrap();
    assert_eq!(report.schema, "aniflow.audio-signal-measurements/v2");
    assert_eq!(report.provider.version, "2.0.0");
    assert_eq!(
        AudioSignalMeasurements::from_json_slice(&report.canonical_json_bytes().unwrap()).unwrap(),
        report
    );
    let mut mislabeled: Value = serde_json::from_slice(document).unwrap();
    mislabeled["schema"] = "aniflow.audio-signal-measurements/v1".into();
    assert!(
        AudioSignalMeasurements::from_json_slice(&serde_json::to_vec(&mislabeled).unwrap())
            .is_err()
    );
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_digest(value: &impl serde::Serialize) -> String {
    // serde_json's default map representation orders every object key, matching
    // the public aniflow.canonical-json/v1 contract used by locks and plans.
    digest(&serde_json::to_vec(&serde_json::to_value(value).unwrap()).unwrap())
}

fn synthetic_legacy_signal_plan(mut plan: PipelineV3Plan) -> PipelineV3Plan {
    let stage = plan
        .payload
        .stages
        .iter_mut()
        .find(|stage| stage.id == "measure_audio")
        .unwrap();
    assert_eq!(stage.provider_lock.payload.provider.version, "2.0.0");
    assert_eq!(stage.capability.version, "2.0.0");
    assert_eq!(
        stage.provider_lock.payload.implementation.id,
        "aniflow-audio-signal-v2"
    );
    stage.capability_requirement.version_requirement = "=1.0.0".to_owned();
    stage.capability.version = "1.0.0".to_owned();
    let lock = &mut stage.provider_lock;
    lock.payload.provider.version = "1.0.0".to_owned();
    lock.payload.capability.version = "1.0.0".to_owned();
    lock.payload.implementation.id = "aniflow-audio-signal-v1".to_owned();
    lock.payload.implementation.executable_sha256 =
        digest(b"synthetic legacy native implementation");
    let mut manifest = ProviderManifest::from_json_slice(include_bytes!(
        "../providers/audio-signal/manifest.json"
    ))
    .unwrap();
    manifest.provider.version = "1.0.0".to_owned();
    manifest.capabilities[0] = stage.capability.clone();
    manifest.validate().unwrap();
    lock.payload.manifest_sha256 = canonical_digest(&manifest);
    lock.lock_sha256 = canonical_digest(&lock.payload);
    let mut authored = PipelineV3Configuration::from_yaml_slice(include_bytes!(
        "../providers/audio-signal/pipeline.yml"
    ))
    .unwrap();
    authored
        .stages
        .iter_mut()
        .find(|stage| stage.id == "measure_audio")
        .unwrap()
        .capability
        .version_requirement = "=1.0.0".to_owned();
    plan.payload.configuration_sha256 = authored.configuration_sha256().unwrap();
    plan.plan_sha256 = canonical_digest(&plan.payload);
    plan.validate().unwrap();
    plan
}

struct Fixture {
    root: TempDir,
    input: PathBuf,
    tools: AudioInspectionConfiguration,
    settings: SignalAnalysisConfiguration,
}

impl Fixture {
    fn new() -> Self {
        Self::with_pcm(8000, 1, 25600)
    }

    fn with_pcm(rate: u32, channels: u16, frames: u32) -> Self {
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
            tool_timeout_milliseconds: 5000,
            maximum_tool_output_bytes: 65536,
        };
        let settings = SignalAnalysisConfiguration::from_json_slice(
            br#"{"schema":"aniflow.audio-signal.configuration/v1"}"#,
        )
        .unwrap();
        let input = root.path().join("synthetic.wav");
        // Exact PCM16 square waves; the second channel has half the amplitude.
        let data_bytes = frames * u32::from(channels) * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for frame in 0..frames {
            for channel in 0..channels {
                let amplitude = 1200_i16 / (i16::try_from(channel).unwrap() + 1);
                wav.extend_from_slice(
                    &(if frame % 2 == 0 {
                        amplitude
                    } else {
                        -amplitude
                    })
                    .to_le_bytes(),
                );
            }
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

    // This fixture represents coherent v1 authority, not a corrupted digest.
    // It deliberately does not claim execution by a historical released binary.
    let workspace = PipelineV3Workspace::open_read_only(&outcome.run_directory).unwrap();
    let current_plan =
        PipelineV3Plan::from_json_slice(&fs::read(workspace.plan()).unwrap()).unwrap();
    let legacy_plan = synthetic_legacy_signal_plan(current_plan.clone());
    let legacy_bytes = legacy_plan.canonical_json_bytes().unwrap();
    assert_ne!(legacy_plan.plan_sha256, current_plan.plan_sha256);
    PipelineV3Plan::from_json_slice(&legacy_bytes).unwrap();
    fs::write(workspace.plan(), &legacy_bytes).unwrap();
    let mut emitted_progress = false;
    let failure = audio_signal::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| emitted_progress = true,
    )
    .unwrap_err();
    assert!(
        failure
            .error
            .to_string()
            .contains("differ from the saved run"),
        "legacy provider authority was not refused by the identity guard: {failure:?}"
    );
    assert!(!emitted_progress);
    assert_eq!(fs::read(workspace.plan()).unwrap(), legacy_bytes);
    assert_eq!(
        fs::read(fixture.root.path().join("launches")).unwrap(),
        launches
    );
    assert_eq!(fs::read(&fixture.input).unwrap(), source);
}

#[test]
fn synthetic_high_rate_pipeline_publishes_versioned_explicit_interpolation_evidence() {
    for rate in [88200, 96000, 176400, 192000] {
        let fixture = Fixture::with_pcm(rate, 2, rate * 2 / 5);
        let source = fs::read(&fixture.input).unwrap();
        let outcome = fixture.run();
        assert_eq!(outcome.executed_stages, ["inspect_audio", "measure_audio"]);
        let signal = outcome
            .outputs
            .iter()
            .find(|output| output.id == "signal")
            .unwrap();
        let bytes = fs::read(&signal.path).unwrap();
        let report = AudioSignalMeasurements::from_json_slice(&bytes).unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report.schema, "aniflow.audio-signal-measurements/v2");
        assert_eq!(report.provider.version, "2.0.0");
        assert_eq!(value["source"]["artifact"]["sha256"], digest(&source));
        assert_eq!(value["source"]["sample_rate_hz"], rate);
        assert_eq!(value["source"]["channels"], 2);
        assert_eq!(value["true_peak"]["value"]["kind"], "measured");
        assert_eq!(value["true_peak"]["value"]["value"], -28.7);
        assert_eq!(value["method"]["true_peak_target_sample_rate_hz"], rate * 4);
        assert_eq!(value["method"]["true_peak_padding_frames"], rate / 10);
        assert_eq!(
            value["method"]["true_peak_algorithm"]["kind"],
            "swr_4x_astats"
        );
        assert_eq!(
            value["method"]["true_peak_algorithm"]["oversampling_factor"],
            4
        );
        let arguments = value["commands"][1]["arguments"].as_array().unwrap();
        let filter = arguments
            .iter()
            .filter_map(Value::as_str)
            .find(|value| value.contains("astats="))
            .unwrap();
        assert!(filter.starts_with(&format!(
            "apad=pad_len={},aresample={}:resampler=swr:",
            rate / 10,
            rate * 4
        )));
        assert!(filter.contains("filter_size=64:phase_shift=10:linear_interp=0:exact_rational=1:cutoff=1:filter_type=kaiser:kaiser_beta=9:dither_method=0:async=0"));
        let analysis = outcome
            .outputs
            .iter()
            .find(|output| output.id == "analysis")
            .unwrap();
        let analysis = AudioAnalysis::from_json_slice(&fs::read(&analysis.path).unwrap()).unwrap();
        let capability = analysis
            .capabilities
            .iter()
            .find(|capability| capability.capability.id == "aniflow/audio-signal-measurements")
            .unwrap();
        assert_eq!(capability.capability.version, "1.0.0");
        assert!(fixture.root.path().join("signal-peak-started").exists());
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn malformed_high_rate_sample_count_never_publishes_signal_evidence() {
    let fixture = Fixture::with_pcm(192000, 2, 76800);
    let source = fs::read(&fixture.input).unwrap();
    fixture.mode("astats_bad_count");
    let mut run = None;
    let error = audio_signal::run(
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
    assert!(
        fixture.root.path().join("signal-peak-started").exists(),
        "high-rate pass was not reached: {error:?}"
    );
    let directory =
        run.unwrap_or_else(|| panic!("durable high-rate failure workspace missing: {error:?}"));
    assert_ne!(
        status_v3(&directory).unwrap().payload.state,
        PipelineV3RunState::Complete
    );
    assert!(
        !directory
            .join("artifacts/audio-signal/signal.json")
            .exists()
    );
    assert!(
        !directory
            .join("artifacts/audio-signal/analysis.json")
            .exists()
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
        // The same budget covers Python startup and preflight. Leave room for
        // concurrent test load; the measurement-only 120s sleep still exceeds it.
        fixture.tools.tool_timeout_milliseconds = 5000;
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
        let error = result.unwrap_err();
        let directory = run
            .unwrap_or_else(|| panic!("mode {mode}: durable run should exist; failure: {error:?}"));
        assert!(
            fixture.root.path().join("signal-started").exists(),
            "mode {mode}: failure must occur during measurement; failure: {error:?}"
        );
        let manifest = status_v3(directory).unwrap();
        assert_ne!(
            manifest.payload.state,
            PipelineV3RunState::Complete,
            "mode {mode}: failed measurement cannot complete; failure: {error:?}"
        );
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
