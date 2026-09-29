#![cfg(unix)]

//! Synthetic native tools prove reviewed-lyrics orchestration, never alignment quality.
use aniflow::audio_stem::StemSelection;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::audio_alignment::{self, AlignmentConfiguration, AlignmentRequest};
use aniflow::audio_analysis::AudioAnalysis;
use aniflow::audio_inspection::{
    AudioInspectionConfiguration, AudioInspectionRequest, AudioToolPin,
};
use aniflow::timed_text::{ConversionOptions, TextProvenance, TimedTextFormat};
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

const FEATURE_PARAMS: &str = "-lowerf 130\n-upperf 6800\n-nfilt 25\n-transform dct\n-lifter 22\n-feat 1s_c_d_dd\n-svspec 0-12/13-25/26-38\n-agc none\n-cmn batch\n-varnorm no\n-model ptm\n-remove_noise yes\n";
const NOISE_DICTIONARY: &str = "<s> SIL\n</s> SIL\n<sil> SIL\n[NOISE] +NSN+\n[SPEECH] +SPN+\n";
const SYNTHETIC_ALIGNER: &str = r#"#!/usr/bin/python3
import json, pathlib, sys, time, os
root = pathlib.Path(__FIXTURE_ROOT__)
args = sys.argv[1:]
mode = (root / "mode").read_text()
(root / "alignment-started").write_text("yes")
with (root / "launches").open("a") as log:
    log.write(json.dumps(args) + "\n")
assert len(args) == 21
assert args[:2] == ["-hmm", str(pathlib.Path.cwd() / "model")]
assert args[2:4] == ["-dict", str(pathlib.Path.cwd() / "dictionary.dict")]
assert args[4:6] == ["-lm", str(pathlib.Path.cwd() / "disabled.lm")]
assert not pathlib.Path(args[5]).exists()
assert args[6:18] == ["-samprate", "16000", "-frate", "100", "-phone_align", "no", "-state_align", "no", "-fsgusealtpron", "no", "-loglevel", "ERROR"]
assert args[18] == "align"
assert args[20] == "hello synthetic world"
assert os.environ.get("PATH") == "/usr/bin:/bin"
assert os.environ.get("HOME") is None
assert sorted(path.name for path in pathlib.Path(args[1]).iterdir()) == ["feat.params", "mdef", "means", "noisedict", "sendump", "transition_matrices", "variances"]
if mode == "sleep": time.sleep(120)
if mode == "nonzero": raise SystemExit(23)
if mode == "flood":
    print("x" * 100000)
    raise SystemExit(0)
if mode == "malformed":
    print("{not-json")
    raise SystemExit(0)
if mode == "changed_lyrics":
    path = root / "reviewed.json"
    path.write_bytes(path.read_bytes() + b"\n")
if mode == "changed_model":
    path = root / "model" / "means"
    path.write_bytes(b"changed model bytes")
if mode == "undeclared_file":
    pathlib.Path("unexpected").write_text("no")
value = {"b": 0.0, "d": 2.0, "p": 1.0, "t": "hello synthetic world", "w": [
    {"b": 0.0, "d": 0.4, "p": 1.0, "t": "hello"},
    {"b": 0.5, "d": 0.4, "p": 1.0, "t": "synthetic"},
    {"b": 1.0, "d": 0.4, "p": 1.0, "t": "world"}]}
if mode == "partial":
    value["w"].pop(1)
    value["t"] = "hello world"
if mode == "out_of_range": value["w"][-1]["d"] = 10.0
if mode == "unknown_word":
    value["w"][-1]["t"] = "intruder"
    value["t"] = "hello synthetic intruder"
if mode == "out_of_order": value["w"].reverse()
print(json.dumps(value))
"#;

struct Fixture {
    root: TempDir,
    input: PathBuf,
    lyrics: PathBuf,
    tools: AudioInspectionConfiguration,
    settings: AlignmentConfiguration,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("alignment café 雪 ")
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
        let tool = directory.join("synthetic-pocketsphinx");
        let script = SYNTHETIC_ALIGNER.replace(
            "__FIXTURE_ROOT__",
            &serde_json::to_string(&directory).unwrap(),
        );
        executable(&tool, &script);
        let model = directory.join("model");
        fs::create_dir(&model).unwrap();
        let files: Vec<Value> = audio_alignment::AUDIO_ALIGNMENT_MODEL_FILES
            .iter()
            .map(|name| {
                let bytes = match *name {
                    "feat.params" => FEATURE_PARAMS.as_bytes(),
                    "noisedict" => NOISE_DICTIONARY.as_bytes(),
                    _ => b"Synthetic model identity; not inference weights.\n".as_slice(),
                };
                fs::write(model.join(name), bytes).unwrap();
                json!({"name":name,"sha256":digest(bytes),"byte_size":bytes.len()})
            })
            .collect();
        // An adjacent asset is deliberately not staged or interpreted.
        fs::write(model.join("untracked-resource"), b"must remain untouched").unwrap();
        let dictionary = directory.join("dictionary.dict");
        let dictionary_bytes = b"hello HH AH L OW\nsynthetic S IH N TH EH T IH K\nworld W ER L D\n";
        fs::write(&dictionary, dictionary_bytes).unwrap();
        let settings = AlignmentConfiguration::from_json_slice(&serde_json::to_vec(&json!({
            "schema":"aniflow.audio-alignment.configuration/v1",
            "pocketsphinx":{"executable":tool,"version":"5.1.1","sha256":digest(script.as_bytes())},
            "model":{"directory":model,"model_id":"en-us","revision":"synthetic-fixture-v1","files":files},
            "dictionary":{"path":dictionary,"sha256":digest(dictionary_bytes),"byte_size":dictionary_bytes.len()},
            "language":"en","tool_timeout_milliseconds":1000,"maximum_tool_output_bytes":65536
        })).unwrap()).unwrap();
        let input = directory.join("source.wav");
        write_pcm(&input, 16000, 32000);
        let lyrics = directory.join("reviewed.json");
        let mut document: Value = serde_json::from_slice(include_bytes!(
            "../docs/contracts/examples/timed-text-v1.example.json"
        ))
        .unwrap();
        document["language"] = json!("en");
        document["audio_source"] = Value::Null;
        document["metadata"] = json!({"revision":"review-r1"});
        document["cues"][0]["text"] = json!("Hello, synthetic!");
        document["cues"][1]["text"] = json!("World.");
        for cue in document["cues"].as_array_mut().unwrap() {
            cue["timing"] = json!({"kind":"untimed"});
        }
        fs::write(&lyrics, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        fs::write(directory.join("mode"), "ok").unwrap();
        Self {
            root,
            input,
            lyrics,
            tools,
            settings,
        }
    }
    fn request(&self) -> AlignmentRequest {
        AlignmentRequest::new(
            AudioInspectionRequest::new(
                &self.input,
                self.tools.clone(),
                env!("CARGO_BIN_EXE_aniflow"),
            )
            .with_execution_limits(audio_alignment::default_execution_limits()),
            self.settings.clone(),
            &self.lyrics,
        )
    }
    fn mode(&self, value: &str) {
        fs::write(self.root.path().join("mode"), value).unwrap();
    }
    fn launches(&self) -> Vec<u8> {
        fs::read(self.root.path().join("launches")).unwrap_or_default()
    }
    fn run(&self) -> PipelineV3RunOutcome {
        audio_alignment::run(
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
fn report(outcome: &PipelineV3RunOutcome) -> audio_alignment::AudioAlignmentReport {
    audio_alignment::AudioAlignmentReport::from_json_slice(
        &fs::read(artifact(outcome, "alignment")).unwrap(),
    )
    .unwrap()
}
#[test]
fn candidate_alignment_preserves_reviewed_text_authority_and_exact_resume() {
    let fixture = Fixture::new();
    let source = fs::read(&fixture.input).unwrap();
    let lyrics = fs::read(&fixture.lyrics).unwrap();
    let plan = audio_alignment::plan(&fixture.request(), &CancellationToken::default()).unwrap();
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
    let outcome = fixture.run();
    let report = report(&outcome);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["result"]["status"], "candidate");
    assert_eq!(
        value["result"]["observation"]["words"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        value["result"]["observation"]["words"][0]["timing"]["end"],
        json!({"numerator":2,"denominator":5})
    );
    assert_eq!(value["confidence"]["kind"], "unavailable");
    assert_eq!(value["method"]["timing_reviewed"], false);
    assert_eq!(report.reviewed_lyrics.artifact.sha256, digest(&lyrics));
    assert_eq!(
        report.reviewed_lyrics.document.metadata["revision"],
        "review-r1"
    );
    let document = report.timed_text.as_ref().unwrap();
    assert_eq!(document.cues[0].text, "Hello, synthetic!");
    assert_eq!(document.cues[1].text, "World.");
    assert!(matches!(
        document.provenance,
        TextProvenance::ReviewedLyrics { .. }
    ));
    assert_eq!(
        document.provenance,
        report.reviewed_lyrics.document.provenance
    );
    let normalized =
        AudioAnalysis::from_json_slice(&fs::read(artifact(&outcome, "analysis")).unwrap()).unwrap();
    assert_eq!(normalized.source, report.source);
    assert_eq!(normalized.semantic_artifacts.len(), 1);
    assert_eq!(
        normalized.semantic_artifacts[0].artifact_id,
        "reviewed_lyrics"
    );
    let TextProvenance::ReviewedLyrics { authority, .. } =
        &report.reviewed_lyrics.document.provenance
    else {
        panic!("reviewed provenance missing");
    };
    assert_eq!(
        normalized.semantic_artifacts[0].authority.as_ref(),
        Some(authority)
    );
    assert!(
        normalized
            .semantic_artifacts
            .iter()
            .all(|artifact| artifact.artifact_id != "alignment")
    );
    let export = audio_alignment::export_alignment_file(
        &artifact(&outcome, "alignment"),
        TimedTextFormat::Json,
        &fixture.root.path().join("export"),
        &ConversionOptions::default(),
    )
    .unwrap();
    assert!(export.conversion.payload_path.is_file());
    let launches = fixture.launches();
    let resumed = audio_alignment::resume(
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
    assert_eq!(fs::read(&fixture.lyrics).unwrap(), lyrics);
    assert_eq!(
        fs::read(fixture.settings.model.directory.join("untracked-resource")).unwrap(),
        b"must remain untouched"
    );
}
#[test]
fn partial_alignment_retains_unmatched_reviewed_words_without_inventing_timing() {
    let fixture = Fixture::new();
    fixture.mode("partial");
    let outcome = fixture.run();
    let report = report(&outcome);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        value["result"]["observation"]["words"][1]["timing"]["status"],
        "unmatched"
    );
    assert_eq!(
        value["result"]["observation"]["cues"][0]["timing"]["status"],
        "partial"
    );
    assert_eq!(report.timed_text.unwrap().cues[0].text, "Hello, synthetic!");
    let normalized =
        AudioAnalysis::from_json_slice(&fs::read(artifact(&outcome, "analysis")).unwrap()).unwrap();
    assert_eq!(
        normalized.status,
        aniflow::audio_analysis::AudioAnalysisStatus::Partial
    );
    assert_eq!(
        normalized
            .capabilities
            .iter()
            .find(|value| value.capability.id == audio_alignment::AUDIO_ALIGNMENT_CAPABILITY_ID)
            .unwrap()
            .status,
        aniflow::audio_analysis::AudioAnalysisStatus::Partial
    );
}
#[test]
fn unsupported_source_is_explicit_without_launching_alignment() {
    for mode in ["rate", "silent"] {
        let fixture = Fixture::new();
        if mode == "rate" {
            write_pcm(&fixture.input, 8000, 16000);
        } else {
            let mut bytes = fs::read(&fixture.input).unwrap();
            bytes[44..].fill(0);
            fs::write(&fixture.input, bytes).unwrap();
        }
        let report = report(&fixture.run());
        assert!(matches!(
            report.result,
            audio_alignment::AlignmentResult::Unavailable { .. }
        ));
        assert!(report.timed_text.is_none());
        assert!(fixture.launches().is_empty());
    }
}
#[test]
fn dependency_and_review_changes_refuse_resume_before_launch() {
    let mut fixture = Fixture::new();
    let outcome = fixture.run();
    let launches = fixture.launches();
    let original_lyrics = fs::read(&fixture.lyrics).unwrap();
    fs::write(
        &fixture.lyrics,
        [original_lyrics.as_slice(), b"\n"].concat(),
    )
    .unwrap();
    assert!(
        audio_alignment::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
    fs::write(&fixture.lyrics, original_lyrics).unwrap();
    let path = fixture.settings.model.directory.join("means");
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"changed").unwrap();
    let failed = audio_alignment::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap_err();
    assert!(failed.preflight.is_some());
    let pin = fixture
        .settings
        .model
        .files
        .iter_mut()
        .find(|file| file.name == "means")
        .unwrap();
    pin.sha256 = digest(b"changed");
    pin.byte_size = 7;
    assert!(
        audio_alignment::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
    fs::write(&path, original).unwrap();
    assert_eq!(fixture.launches(), launches);
    let fixture = Fixture::new();
    fs::remove_file(&fixture.settings.dictionary.path).unwrap();
    let preflight = audio_alignment::preflight(
        &fixture.settings,
        &fixture.tools,
        &CancellationToken::default(),
    )
    .unwrap();
    assert!(
        preflight
            .diagnostics
            .iter()
            .any(|item| item.code
                == audio_alignment::AudioAlignmentDiagnosticCode::MissingDictionary)
    );
    let fixture = Fixture::new();
    fs::remove_file(&fixture.settings.pocketsphinx.executable).unwrap();
    assert!(
        !audio_alignment::preflight(
            &fixture.settings,
            &fixture.tools,
            &CancellationToken::default()
        )
        .unwrap()
        .ready
    );
}
#[test]
fn malformed_bounded_and_mutating_tools_never_publish_alignment() {
    for mode in [
        "malformed",
        "out_of_range",
        "unknown_word",
        "out_of_order",
        "flood",
        "sleep",
        "nonzero",
        "changed_lyrics",
        "changed_model",
        "undeclared_file",
    ] {
        let fixture = Fixture::new();
        fixture.mode(mode);
        let source = fs::read(&fixture.input).unwrap();
        let mut run = None;
        let failure = audio_alignment::run(
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
        let directory = run.unwrap_or_else(|| panic!("{mode}: expected durable run: {failure:?}"));
        assert!(
            fixture.root.path().join("alignment-started").exists(),
            "{mode}: {failure:?}"
        );
        assert_ne!(
            status_v3(&directory).unwrap().payload.state,
            PipelineV3RunState::Complete
        );
        assert!(
            !directory
                .join("artifacts/audio-alignment/alignment.json")
                .exists()
        );
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}
#[test]
fn cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment() {
    let fixture = Fixture::new();
    fixture.mode("sleep");
    let cancellation = CancellationToken::default();
    let token = cancellation.clone();
    let marker = fixture.root.path().join("alignment-started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(marker.exists());
        token.cancel();
    });
    let mut run = None;
    let result = audio_alignment::run(
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
    let run = run.unwrap();
    assert_eq!(
        status_v3(&run).unwrap().payload.state,
        PipelineV3RunState::Cancelled
    );
    fixture.mode("ok");
    let resumed = audio_alignment::resume(
        fixture.request(),
        run,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(resumed.reused_stages, ["inspect_audio"]);
    assert_eq!(resumed.executed_stages.len(), 1);
    report(&resumed);
}
#[test]
fn reviewed_authority_is_required_before_planning_and_tool_launch() {
    for kind in ["unreviewed", "observed_transcript"] {
        let fixture = Fixture::new();
        let mut value: Value = serde_json::from_slice(&fs::read(&fixture.lyrics).unwrap()).unwrap();
        value["provenance"] = if kind == "unreviewed" {
            json!({"kind":kind})
        } else {
            json!({"kind":kind,"producer":"synthetic"})
        };
        fs::write(&fixture.lyrics, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(audio_alignment::plan(&fixture.request(), &CancellationToken::default()).is_err());
        assert!(fixture.launches().is_empty());
    }
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
            .with_execution_limits(audio_alignment::default_execution_limits())
            .with_stem_selection(selection);
    let outcome = audio_alignment::run(
        AlignmentRequest::new(inspection, fixture.settings.clone(), &fixture.lyrics),
        Some(fixture.root.path().join("runs")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(outcome.executed_stages.len(), 3);
    let report = report(&outcome);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["result"]["reason"], "unsupported_channels");
    assert_eq!(value["source"]["stem"]["id"], stem);
    assert_eq!(value["scope"]["stem_id"], stem);
    assert_eq!(value["scope"]["channels"], json!([0, 1]));
    assert_eq!(
        value["source"]["stem"]["original_mix"]["sha256"],
        digest(&original)
    );
    assert_eq!(
        AudioAnalysis::from_json_slice(&fs::read(artifact(&outcome, "analysis")).unwrap())
            .unwrap()
            .source
            .stem
            .unwrap()
            .id,
        stem
    );
    assert!(fixture.launches().is_empty());
    assert_eq!(fs::read(&input).unwrap(), original);
}
