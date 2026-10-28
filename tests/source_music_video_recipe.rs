//! Synthetic source-entry recipe regressions authored for deferred #64 execution.
//!
//! Substitute tools make accidental plan-time native execution observable. These
//! cases exercise public admission and authority contracts; they do not qualify
//! extraction, image repair, Upscayl, assembly or visual quality.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use aniflow::frame_extraction::{self, FrameExtractionRequest};
use aniflow::music_video_recipe::{self, MusicVideoConfiguration, MusicVideoPhase,
    MusicVideoPhaseState, MusicVideoRequest, SourceMusicVideoConfiguration,
    SourceMusicVideoIntent, SourceMusicVideoPhase, SourceMusicVideoRequest};
use aniflow::CancellationToken;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn caller_settings() -> Value {
    let ffmpeg = json!({"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)});
    let ffprobe = json!({"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)});
    json!({
        "schema":"aniflow.source-music-video-recipe/v1",
        "extraction":{"schema":"aniflow.frame-extraction-configuration/v1",
            "ffmpeg":ffmpeg.clone(),"ffprobe":ffprobe.clone(),
            "source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]}},
        "recipe":{"schema":"aniflow.music-video-recipe/v1",
            "repair":{"mode":"disabled","reason":"No watermark repair requested",
                "import":{"schema":"aniflow.frame-import-configuration/v1","ffmpeg":ffmpeg.clone()}},
            "upscale":{"mode":"disabled","reason":"Preserve extracted dimensions"},
            "assembly":{"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":ffmpeg,"ffprobe":ffprobe}}
    })
}

fn parse(value: &Value) -> aniflow::Result<SourceMusicVideoConfiguration> {
    SourceMusicVideoConfiguration::from_json_slice(&serde_json::to_vec(value).unwrap())
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    settings: Value,
}

impl Fixture {
    fn request(&self) -> SourceMusicVideoRequest {
        SourceMusicVideoRequest::new(&self.source, parse(&self.settings).unwrap(),
            PathBuf::from(env!("CARGO_BIN_EXE_aniflow")).canonicalize().unwrap())
    }

    fn tool(&self, name: &str) -> Value {
        let executable = self.root.join(name);
        let marker = self.root.join("launched");
        let literal = format!("'{}'", marker.display().to_string().replace('\'', "'\\''"));
        let script = format!("#!/bin/sh\nprintf launched > {literal}\nexit 99\n");
        fs::write(&executable, &script).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        json!({"executable":executable,"version":"8.0.1","sha256":sha256(script.as_bytes())})
    }

    fn select_target_with_absent_neural_files(&mut self) {
        let import = self.settings["recipe"]["repair"]["import"].clone();
        self.settings["recipe"]["upscale"] = json!({"mode":"target_resolution",
            "policy":{"schema":"aniflow.upscale-target-policy/v1","width":3840,"height":2160,"mode":"exact"},
            "configuration":{"schema":"aniflow.upscayl-recovery-configuration/v1","import":import,
                "upscayl":{"executable":self.root.join("absent upscayl"),"version":"1.0.0","sha256":"c".repeat(64)},
                "output_scale":2,"model":{"name":"fixture_model","native_scale":4,
                    "param":{"path":self.root.join("absent model.param"),"sha256":"d".repeat(64)},
                    "weights":{"path":self.root.join("absent model.bin"),"sha256":"e".repeat(64)}},
                "execution":{"mode":"bounded_chunk","frames_per_chunk":2,"timeout_milliseconds":2000}}});
    }

    fn retain_intent(&self) -> PathBuf {
        let plan = music_video_recipe::plan_source(&self.request(), &CancellationToken::default()).unwrap();
        let root = self.root.join("retained source recipe");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("children")).unwrap();
        fs::write(root.join("intent.json"), serde_json::to_vec(&plan.intent).unwrap()).unwrap();
        root
    }
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("source recipe 音 with spaces");
    fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    fs::write(&source, b"synthetic original source bytes; not a native media file").unwrap();
    let mut fixture = Fixture { _directory:directory, root, source, settings:caller_settings() };
    let ffmpeg = fixture.tool("ffmpeg");
    let ffprobe = fixture.tool("extraction ffprobe");
    let assembly_ffprobe = fixture.tool("assembly ffprobe");
    fixture.settings["extraction"]["ffmpeg"] = ffmpeg.clone();
    fixture.settings["extraction"]["ffprobe"] = ffprobe;
    fixture.settings["recipe"]["repair"]["import"]["ffmpeg"] = ffmpeg.clone();
    fixture.settings["recipe"]["assembly"]["ffmpeg"] = ffmpeg;
    fixture.settings["recipe"]["assembly"]["ffprobe"] = assembly_ffprobe;
    fixture
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    WalkDir::new(root).into_iter().map(|entry| entry.unwrap()).map(|entry| {
        let path = entry.path().strip_prefix(root).unwrap().to_path_buf();
        let digest = if entry.file_type().is_file() { sha256(&fs::read(entry.path()).unwrap()) }
            else { "directory".to_owned() };
        (path, digest)
    }).collect()
}

#[test]
fn source_configuration_is_closed_and_preserves_the_indexed_configuration_boundary() {
    let original = caller_settings();
    let settings = parse(&original).unwrap();
    assert_eq!(settings, parse(&serde_json::to_value(&settings).unwrap()).unwrap());
    for field in ["extraction", "recipe"] {
        let mut candidate = original.clone(); candidate.as_object_mut().unwrap().remove(field);
        assert!(parse(&candidate).is_err(), "missing {field}");
    }
    for field in ["frames_directory", "frame_index", "steps", "repair", "upscale", "assembly", "complete"] {
        let mut candidate = original.clone(); candidate[field] = json!({});
        assert!(parse(&candidate).is_err(), "mixed or unknown {field}");
    }
    let mut candidate = original.clone();
    candidate["extraction"].as_object_mut().unwrap().remove("source_selection");
    assert!(parse(&candidate).is_err());
    let mut candidate = original.clone();
    candidate["recipe"].as_object_mut().unwrap().remove("assembly");
    assert!(parse(&candidate).is_err());
    let indexed_bytes = serde_json::to_vec(&original["recipe"]).unwrap();
    assert!(MusicVideoConfiguration::from_json_slice(&indexed_bytes).is_ok());
    assert!(SourceMusicVideoConfiguration::from_json_slice(&indexed_bytes).is_err());
    assert!(MusicVideoConfiguration::from_json_slice(&serde_json::to_vec(&original).unwrap()).is_err());
    // The original indexed constructor and phase enum remain available, with no
    // new required source-entry field or extraction phase in their contract.
    let indexed = MusicVideoRequest::new("/synthetic/source", "/synthetic/frames", "/synthetic/index.json",
        settings.recipe, "/synthetic/provider");
    assert_eq!(indexed.frames_directory, PathBuf::from("/synthetic/frames"));
    assert_eq!(serde_json::to_value(MusicVideoPhase::PrepareFrames).unwrap(), json!("prepare_frames"));
    assert!(serde_json::from_value::<MusicVideoPhase>(json!("extract_frames")).is_err());
}

#[test]
fn source_configuration_has_a_bounded_document_without_a_new_settings_owner() {
    let original = caller_settings();
    let mut bytes = vec![b' '; 131_072];
    bytes.extend_from_slice(&serde_json::to_vec(&original).unwrap());
    assert!(SourceMusicVideoConfiguration::from_json_slice(&bytes).is_err());
    for (pointer, replacement) in [
        ("/extraction/source_selection/no_audio", json!(false)),
        ("/extraction/ffmpeg/version", json!("8.0.2")),
        ("/recipe/repair/reason", json!("   ")),
        ("/recipe/assembly/ffmpeg/version", json!("8.0.2")),
    ] {
        let mut candidate = original.clone();
        *candidate.pointer_mut(pointer).unwrap() = replacement;
        assert!(parse(&candidate).is_err(), "nested contract {pointer}");
    }
}

#[test]
fn source_plan_uses_only_exact_extraction_inputs_without_native_probe_or_guessed_geometry() {
    let fixture = fixture();
    let request = fixture.request();
    let before = snapshot(&fixture.root);
    let plan = music_video_recipe::plan_source(&request, &CancellationToken::default()).unwrap();
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
    assert_eq!(plan.schema, "aniflow.source-music-video-plan/v1");
    assert_eq!((plan.output_width, plan.output_height), (None, None));
    assert!(plan.target_resolution.is_none());
    assert_eq!(plan.phases.iter().map(|phase| phase.phase).collect::<Vec<_>>(),
        [SourceMusicVideoPhase::ExtractFrames, SourceMusicVideoPhase::PrepareFrames,
            SourceMusicVideoPhase::Upscale, SourceMusicVideoPhase::Assembly]);
    assert_eq!(plan.phases[0].state, MusicVideoPhaseState::Ready);
    assert_eq!(plan.phases[1].state, MusicVideoPhaseState::Pending);
    assert_eq!(plan.phases[2].state, MusicVideoPhaseState::Disabled);
    assert_eq!(plan.phases[3].state, MusicVideoPhaseState::Pending);
    assert!(plan.phases[1..].iter().all(|phase| phase.plan.is_none()));
    let extraction_request = FrameExtractionRequest::new(&request.source_video,
        request.configuration.extraction.clone(), &request.provider_executable);
    let extraction = frame_extraction::plan(&extraction_request, &CancellationToken::default()).unwrap();
    assert_eq!(plan.phases[0].plan.as_ref(), Some(&extraction));
    assert_eq!(plan.intent.payload.initial_plan_sha256, extraction.plan_sha256);
    assert_eq!(plan.intent.payload.initial_inputs, extraction.payload.inputs);
    assert_eq!(plan.intent.payload.initial_inputs.len(), 1);
    assert_eq!(plan.intent.payload.initial_inputs[0].id, "source_video");
    assert!(plan.intent.payload.components.iter().any(|component| component.path == fixture.root.join("assembly ffprobe")),
        "pending mandatory assembly dependencies are observed up front");
    let serialized = serde_json::to_value(&plan).unwrap();
    for field in ["output_width", "output_height", "target_resolution"] {
        assert_eq!(serialized.get(field), Some(&Value::Null), "{field} must explicitly remain unresolved");
    }
    assert!(serialized["intent"]["payload"].get("frames_directory").is_none());
    assert!(serialized["intent"]["payload"].get("frame_index").is_none());
    assert_eq!(plan, music_video_recipe::plan_source(&request, &CancellationToken::default()).unwrap());
}

#[test]
fn unresolved_target_defers_neural_files_without_claiming_a_bypass() {
    let mut fixture = fixture();
    fixture.select_target_with_absent_neural_files();
    let before = snapshot(&fixture.root);
    let plan = music_video_recipe::plan_source(&fixture.request(), &CancellationToken::default()).unwrap();
    assert!(plan.target_resolution.is_none());
    assert_eq!((plan.output_width, plan.output_height), (None, None));
    assert_eq!(plan.phases[2].state, MusicVideoPhaseState::Pending);
    assert!(plan.phases[2].plan.is_none());
    for filename in ["absent upscayl", "absent model.param", "absent model.bin"] {
        assert!(!plan.intent.payload.components.iter().any(|component| component.path == fixture.root.join(filename)));
    }
    assert_eq!(snapshot(&fixture.root), before);
    let first_intent = plan.intent.intent_sha256;
    fixture.settings["recipe"]["upscale"]["configuration"]["model"]["weights"]["sha256"] = json!("f".repeat(64));
    let changed = music_video_recipe::plan_source(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_ne!(first_intent, changed.intent.intent_sha256, "deferred files' configured pins still bind intent");
    assert!(!fixture.root.join("launched").exists());
}

#[test]
fn mandatory_and_unconditionally_selected_dependencies_cannot_be_deferred() {
    for dependency in ["assembly", "unconditional_upscale"] {
        let mut fixture = fixture();
        if dependency == "assembly" {
            fs::remove_file(fixture.root.join("assembly ffprobe")).unwrap();
        } else {
            fixture.select_target_with_absent_neural_files();
            let configuration = fixture.settings["recipe"]["upscale"]["configuration"].clone();
            fixture.settings["recipe"]["upscale"] = json!({"mode":"upscayl","configuration":configuration});
        }
        let before = snapshot(&fixture.root);
        assert!(music_video_recipe::plan_source(&fixture.request(), &CancellationToken::default()).is_err(), "{dependency}");
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
    }
}

#[test]
fn immutable_source_intent_cannot_gain_frame_locators_or_completion_authority() {
    let fixture = fixture();
    let intent = music_video_recipe::plan_source(&fixture.request(), &CancellationToken::default()).unwrap().intent;
    intent.validate().unwrap();
    let original = serde_json::to_value(&intent).unwrap();
    for (pointer, replacement) in [
        ("/payload/configuration/recipe/upscale/reason", json!("Changed optional decision")),
        ("/payload/configuration/extraction/maximum_frames", json!(10)),
        ("/payload/initial_plan_sha256", json!("d".repeat(64))),
        ("/payload/components/0/sha256", json!("f".repeat(64))),
        ("/payload/source_video", json!("/synthetic/another-source.mp4")),
    ] {
        let mut candidate = original.clone();
        *candidate.pointer_mut(pointer).unwrap() = replacement;
        let changed: SourceMusicVideoIntent = serde_json::from_value(candidate).unwrap();
        assert!(changed.validate().is_err(), "{pointer}");
    }
    for field in ["complete", "accepted", "phases"] {
        let mut candidate = original.clone(); candidate[field] = json!(true);
        assert!(serde_json::from_value::<SourceMusicVideoIntent>(candidate).is_err(), "{field}");
    }
    for field in ["frames_directory", "frame_index", "target_resolution"] {
        let mut candidate = original.clone(); candidate["payload"][field] = json!("/synthetic/guessed");
        assert!(serde_json::from_value::<SourceMusicVideoIntent>(candidate).is_err(), "{field}");
    }
}

#[test]
fn changed_source_tool_or_settings_refuse_resume_before_writing_or_progress() {
    for changed in ["source", "extraction_tool", "assembly_tool", "extraction_settings", "recipe_settings"] {
        let mut fixture = fixture();
        let root = fixture.retain_intent();
        match changed {
            "source" => fs::write(&fixture.source, b"changed source identity").unwrap(),
            "extraction_tool" | "assembly_tool" => {
                let (filename, pointer) = if changed == "extraction_tool" {
                    ("extraction ffprobe", "/extraction/ffprobe/sha256")
                } else { ("assembly ffprobe", "/recipe/assembly/ffprobe/sha256") };
                let path = fixture.root.join(filename);
                let mut bytes = fs::read(&path).unwrap(); bytes.extend_from_slice(b"# replaced build\n");
                fs::write(path, &bytes).unwrap();
                *fixture.settings.pointer_mut(pointer).unwrap() = json!(sha256(&bytes));
            },
            "extraction_settings" => fixture.settings["extraction"]["maximum_frames"] = json!(10),
            "recipe_settings" => fixture.settings["recipe"]["assembly"]["delivery_crf"] = json!(19),
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.root);
        let mut progressed = false;
        assert!(music_video_recipe::resume_source(fixture.request(), &root, &CancellationToken::default(),
            |_| progressed = true).is_err(), "{changed}");
        assert!(!progressed, "{changed}");
        assert_eq!(snapshot(&fixture.root), before, "{changed}");
        assert!(!fixture.root.join("launched").exists());
    }
}

#[test]
fn read_only_status_keeps_extraction_pending_without_adopting_loose_outputs() {
    let mut fixture = fixture();
    fixture.select_target_with_absent_neural_files();
    let root = fixture.retain_intent();
    let before = snapshot(&fixture.root);
    let initial = music_video_recipe::status_source(&root).unwrap();
    assert!(!initial.complete);
    assert!(initial.outputs.is_empty());
    assert_eq!((initial.output_width, initial.output_height), (None, None));
    assert!(initial.target_resolution.is_none());
    assert_eq!(initial.phases[0].state, MusicVideoPhaseState::Ready);
    assert!(initial.phases[1..].iter().all(|phase| phase.state == MusicVideoPhaseState::Pending));
    assert_eq!(snapshot(&fixture.root), before);
    let child = root.join("children/extract_frames");
    fs::create_dir(&child).unwrap();
    fs::create_dir(child.join("extracted_frames")).unwrap();
    fs::write(child.join("extracted_index.json"), br#"{"schema":"aniflow.rife-frame-index/v1","width":3840,"height":2160}"#).unwrap();
    fs::write(child.join("extraction_report.json"), br#"{"complete":true,"accepted":true}"#).unwrap();
    let before = snapshot(&fixture.root);
    let observed = music_video_recipe::status_source(&root).unwrap();
    assert!(!observed.complete);
    assert!(observed.outputs.is_empty());
    assert_eq!((observed.output_width, observed.output_height), (None, None));
    assert!(observed.target_resolution.is_none());
    assert_eq!(observed.phases[0].state, MusicVideoPhaseState::IncompleteInitialization);
    assert!(observed.phases[0].manifest.is_none());
    assert!(observed.phases[1..].iter().all(|phase| phase.state == MusicVideoPhaseState::Pending
        && phase.manifest.is_none() && phase.run_directory.is_none()));
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
}

#[test]
fn unknown_or_premature_tail_children_cannot_become_an_accepted_source_handoff() {
    for slot in ["unknown", "prepare_frames", "assembly"] {
        let fixture = fixture();
        let root = fixture.retain_intent();
        fs::create_dir(root.join("children").join(slot)).unwrap();
        let before = snapshot(&fixture.root);
        assert!(music_video_recipe::status_source(&root).is_err(), "{slot}");
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
    }
}

#[test]
fn cancellation_before_source_admission_creates_no_recipe_or_native_process() {
    let fixture = fixture();
    let before = snapshot(&fixture.root);
    let cancellation = CancellationToken::default(); cancellation.cancel();
    assert!(music_video_recipe::plan_source(&fixture.request(), &cancellation).is_err());
    let mut progressed = false;
    assert!(music_video_recipe::run_source(fixture.request(), Some(fixture.root.join("runs")),
        &cancellation, |_| progressed = true).is_err());
    assert!(!progressed);
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
}
