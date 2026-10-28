//! Synthetic recipe regressions authored for deferred #64 execution.
//!
//! Files and substitute executables exercise admission and existing runtime
//! ownership. They do not qualify image repair, Upscayl, media assembly, visual
//! quality or native support. No fixture report can grant child acceptance.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use aniflow::music_video_recipe::{
    self, MusicVideoConfiguration, MusicVideoIntent, MusicVideoPhase, MusicVideoPhaseState, MusicVideoRequest,
};
use aniflow::temporal::{self, StreamSelection};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::CancellationToken;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn caller_settings() -> Value {
    let ffmpeg = json!({"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)});
    json!({
        "schema":"aniflow.music-video-recipe/v1",
        "repair":{"mode":"disabled","reason":"Frames need no watermark repair",
            "import":{"schema":"aniflow.frame-import-configuration/v1","ffmpeg":ffmpeg.clone()}},
        "upscale":{"mode":"disabled","reason":"The indexed dimensions are sufficient"},
        "assembly":{"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":ffmpeg,
            "ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)}}
    })
}

fn parse(value: &Value) -> aniflow::Result<MusicVideoConfiguration> {
    MusicVideoConfiguration::from_json_slice(&serde_json::to_vec(value).unwrap())
}

#[test]
fn settings_require_explicit_optional_decisions_and_mandatory_assembly() {
    let original = caller_settings();
    let settings = parse(&original).unwrap();
    assert_eq!(settings, parse(&serde_json::to_value(&settings).unwrap()).unwrap());
    for field in ["repair", "upscale", "assembly"] {
        let mut candidate = original.clone(); candidate.as_object_mut().unwrap().remove(field);
        assert!(parse(&candidate).is_err(), "missing {field}");
    }
    for operation in ["repair", "upscale"] {
        for reason in [json!(""), json!("   "), json!("skip\nwithout a durable explanation"), Value::Null] {
            let mut candidate = original.clone(); candidate[operation]["reason"] = reason;
            assert!(parse(&candidate).is_err(), "{operation} reason");
        }
        let mut candidate = original.clone(); candidate[operation].as_object_mut().unwrap().remove("reason");
        assert!(parse(&candidate).is_err());
    }
    let mut candidate = original; candidate["assembly"] = json!({"mode":"disabled","reason":"skip"});
    assert!(parse(&candidate).is_err());
}

#[test]
fn settings_reject_generic_steps_fallbacks_and_ignored_configuration() {
    let original = caller_settings();
    for field in ["steps", "providers", "fallback", "arguments", "import", "complete"] {
        let mut candidate = original.clone(); candidate[field] = json!({});
        assert!(parse(&candidate).is_err(), "{field}");
    }
    for (operation, mode) in [("repair", "generic"), ("repair", "automatic"), ("upscale", "automatic")] {
        let mut candidate = original.clone(); candidate[operation]["mode"] = json!(mode);
        assert!(parse(&candidate).is_err());
    }
    for operation in ["repair", "upscale"] {
        let mut candidate = original.clone(); candidate[operation]["configuration"] = json!({});
        assert!(parse(&candidate).is_err(), "disabled {operation} must not silently ignore settings");
    }
    let mut candidate = original; candidate["assembly"]["ffmpeg"]["version"] = json!("8.0.2");
    assert!(parse(&candidate).is_err());
}

fn png(color: u8) -> Vec<u8> {
    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind); output.extend_from_slice(data);
        let mut crc = u32::MAX;
        for byte in kind.iter().chain(data) {
            crc ^= u32::from(*byte);
            for _ in 0..8 { crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1)); }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
    }
    let mut output = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = 2_u32.to_be_bytes().to_vec(); header.extend_from_slice(&2_u32.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]); chunk(&mut output, b"IHDR", &header);
    let scanlines = [0, color, color, color, color, color, color, 0, color, color, color, color, color, color];
    let mut compressed = vec![0x78, 0x01, 0x01, 14, 0, 0xf1, 0xff];
    compressed.extend_from_slice(&scanlines);
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in scanlines { a = (a + u32::from(byte)) % 65_521; b = (b + a) % 65_521; }
    compressed.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut output, b"IDAT", &compressed); chunk(&mut output, b"IEND", &[]); output
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    frames: PathBuf,
    index_path: PathBuf,
    settings: Value,
}

impl Fixture {
    fn request(&self) -> MusicVideoRequest {
        MusicVideoRequest::new(&self.source, &self.frames, &self.index_path, parse(&self.settings).unwrap(),
            PathBuf::from(env!("CARGO_BIN_EXE_aniflow")))
    }

    fn tool(&self, name: &str, version: &str) -> Value {
        let executable = self.root.join(name);
        // Substitute scripts only: a read-only plan must never launch one.
        let text = format!("#!/bin/sh\nprintf launched > '{}'/launched\nexit 99\n", self.root.display());
        fs::write(&executable, &text).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        json!({"executable":executable,"version":version,"sha256":sha256(text.as_bytes())})
    }

    fn select_repair(&mut self) {
        let import = self.settings["repair"]["import"].clone();
        self.settings["repair"] = json!({"mode":"allenk","configuration":{
            "schema":"aniflow.allenk-frame-repair-configuration/v1","import":import,
            "tool":self.tool("allenk", "0.3.2"),"watermark_profile":"current_v2"
        },"ranges":[{"first":2,"last":2}]});
    }

    fn select_upscale(&mut self) {
        let import = if self.settings["repair"]["mode"] == "disabled" { self.settings["repair"]["import"].clone() }
            else { self.settings["repair"]["configuration"]["import"].clone() };
        let param = self.root.join("fixture.param"); let weights = self.root.join("fixture.bin");
        fs::write(&param, b"synthetic parameter bytes").unwrap();
        fs::write(&weights, b"synthetic weights bytes").unwrap();
        self.settings["upscale"] = json!({"mode":"upscayl","configuration":{
            "schema":"aniflow.upscayl-recovery-configuration/v1","import":import,
            "upscayl":self.tool("upscayl", "1.0.0"),"output_scale":2,
            "model":{"name":"fixture_model","native_scale":4,
                "param":{"path":param,"sha256":sha256(b"synthetic parameter bytes")},
                "weights":{"path":weights,"sha256":sha256(b"synthetic weights bytes")}},
            "execution":{"mode":"bounded_chunk","frames_per_chunk":2,"timeout_milliseconds":2000}
        }});
    }
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("recipe 音 with spaces"); fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    let bytes = b"synthetic original source identity; not a native media file";
    fs::write(&source, bytes).unwrap();
    let frames = root.join("original frames"); fs::create_dir(&frames).unwrap();
    let entries = (0..3).map(|ordinal| {
        let filename = format!("frame_{ordinal:08}.png"); let bytes = png(ordinal as u8 + 1);
        fs::write(frames.join(&filename), &bytes).unwrap();
        RifeFrame { ordinal, filename, sha256:sha256(&bytes) }
    }).collect();
    let inspection = temporal::from_probe_documents(&sha256(bytes), bytes.len() as u64,
        &StreamSelection::default(), &json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
            "time_base":"1/24","start_pts":0,"duration_ts":3,"r_frame_rate":"24/1","avg_frame_rate":"24/1",
            "width":2,"height":2,"pix_fmt":"yuv420p","disposition":{"attached_pic":0}}]}),
        &[(0, json!({"frames":(0..3).map(|ordinal| json!({"stream_index":0,"pts":ordinal,"pkt_dts":ordinal,
            "pkt_duration":1,"key_frame":if ordinal == 0 {1} else {0}})).collect::<Vec<_>>()}),
            json!({"packets":(0..3).map(|ordinal| json!({"stream_index":0,"pts":ordinal,"dts":ordinal,"duration":1})).collect::<Vec<_>>()}))],
    ).unwrap();
    let index = RifeFrameIndex { schema:RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection:inspection,
        width:2, height:2, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(), frames:entries, cut_before_ordinals:vec![2] };
    let index_path = root.join("frame index.json"); fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let mut fixture = Fixture { _directory:directory, root, source, frames, index_path, settings:caller_settings() };
    let ffmpeg = fixture.tool("ffmpeg", "8.0.1"); let ffprobe = fixture.tool("ffprobe", "8.0.1");
    fixture.settings["repair"]["import"]["ffmpeg"] = ffmpeg.clone();
    fixture.settings["assembly"]["ffmpeg"] = ffmpeg; fixture.settings["assembly"]["ffprobe"] = ffprobe;
    fixture
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    WalkDir::new(root).into_iter().map(|entry| entry.unwrap()).map(|entry| {
        let path = entry.path().strip_prefix(root).unwrap().to_path_buf();
        let identity = if entry.file_type().is_file() { sha256(&fs::read(entry.path()).unwrap()) }
            else { "directory".to_owned() };
        (path, identity)
    }).collect()
}

#[test]
fn fixed_recipe_plans_only_real_initial_bytes_and_keeps_downstream_plans_pending() {
    for (repair, upscale) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut fixture = fixture();
        if repair { fixture.select_repair(); }
        if upscale { fixture.select_upscale(); }
        let before = snapshot(&fixture.root);
        let plan = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap();
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
        assert_eq!((plan.output_width, plan.output_height), if upscale { (4, 4) } else { (2, 2) });
        assert_eq!(plan.phases.iter().map(|phase| phase.phase).collect::<Vec<_>>(),
            [MusicVideoPhase::PrepareFrames, MusicVideoPhase::Upscale, MusicVideoPhase::Assembly]);
        assert_eq!(plan.phases[0].state, MusicVideoPhaseState::Ready);
        assert_eq!(plan.phases[1].state, if upscale { MusicVideoPhaseState::Pending } else { MusicVideoPhaseState::Disabled });
        assert_eq!(plan.phases[2].state, MusicVideoPhaseState::Pending);
        let first = plan.phases[0].plan.as_ref().unwrap();
        assert_eq!(first.payload.stages.len(), if repair { 2 } else { 1 });
        assert_eq!(first.plan_sha256, plan.intent.payload.initial_plan_sha256);
        assert_eq!(first.payload.inputs, plan.intent.payload.initial_inputs);
        assert!(plan.phases[1].plan.is_none());
        assert!(plan.phases[2].plan.is_none());
        assert!(plan.intent.payload.components.iter().any(|component|
            component.path == fixture.root.join("ffprobe")), "even pending assembly pins are observed up front");
        if upscale {
            assert!(plan.intent.payload.components.iter().any(|component| component.path == fixture.root.join("fixture.bin")));
        }
    }
}

#[test]
fn downstream_settings_skip_reasons_and_exact_component_bytes_bind_intent_identity() {
    let mut fixture = fixture();
    let first = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    fixture.settings["upscale"]["reason"] = json!("Keep the original indexed resolution");
    let reason = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_ne!(first.intent.intent_sha256, reason.intent.intent_sha256);
    assert_eq!(first.intent.payload.initial_plan_sha256, reason.intent.payload.initial_plan_sha256);
    fixture.settings["assembly"]["delivery_crf"] = json!(19);
    let settings = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_ne!(reason.intent.intent_sha256, settings.intent.intent_sha256);
    assert_eq!(reason.intent.payload.initial_plan_sha256, settings.intent.payload.initial_plan_sha256);
    let executable = fixture.root.join("ffprobe");
    let mut bytes = fs::read(&executable).unwrap(); bytes.extend_from_slice(b"# changed build identity\n");
    fs::write(executable, &bytes).unwrap();
    fixture.settings["assembly"]["ffprobe"]["sha256"] = json!(sha256(&bytes));
    let component = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_ne!(settings.intent.intent_sha256, component.intent.intent_sha256);
    assert_eq!(settings.intent.payload.initial_plan_sha256, component.intent.payload.initial_plan_sha256);
    assert!(!fixture.root.join("launched").exists());
}

#[test]
fn selected_downstream_pins_geometry_and_ranges_are_admitted_before_any_child_starts() {
    for mutation in ["probe", "upscayl", "weights", "geometry", "range", "source", "index_profile"] {
        let mut fixture = fixture(); fixture.select_repair(); fixture.select_upscale();
        match mutation {
            "probe" => fs::write(fixture.root.join("ffprobe"), b"changed selected binary").unwrap(),
            "upscayl" => fs::write(fixture.root.join("upscayl"), b"changed selected binary").unwrap(),
            "weights" => fs::write(fixture.root.join("fixture.bin"), b"changed selected model").unwrap(),
            "geometry" => fixture.settings["assembly"]["maximum_pixels"] = json!(4),
            "range" => fixture.settings["repair"]["ranges"] = json!([{"first":4,"last":4}]),
            "source" => fs::write(&fixture.source, b"changed original source").unwrap(),
            "index_profile" => {
                let mut index: Value = serde_json::from_slice(&fs::read(&fixture.index_path).unwrap()).unwrap();
                index["schema"] = json!("aniflow.upscayl-output-index/v1");
                fs::write(&fixture.index_path, serde_json::to_vec(&index).unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.root);
        assert!(music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).is_err(), "{mutation}");
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
    }
}

#[test]
fn cancellation_does_not_create_a_recipe_or_probe_selected_tools() {
    let mut fixture = fixture(); fixture.select_repair(); fixture.select_upscale();
    let before = snapshot(&fixture.root);
    let cancellation = CancellationToken::default(); cancellation.cancel();
    assert!(music_video_recipe::plan(&fixture.request(), &cancellation).is_err());
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
}

#[test]
fn immutable_intent_rejects_changed_payload_and_has_no_completion_authority_field() {
    let fixture = fixture();
    let intent = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap().intent;
    intent.validate().unwrap();
    let original = serde_json::to_value(&intent).unwrap();
    for (pointer, value) in [
        ("/payload/configuration/upscale/reason", json!("a different decision")),
        ("/payload/configuration/assembly/delivery_crf", json!(20)),
        ("/payload/initial_plan_sha256", json!("e".repeat(64))),
        ("/payload/components/0/sha256", json!("f".repeat(64))),
        ("/payload/source_video", json!("/another/source.mp4")),
    ] {
        let mut changed = original.clone(); *changed.pointer_mut(pointer).unwrap() = value;
        let changed: MusicVideoIntent = serde_json::from_value(changed).unwrap();
        assert!(changed.validate().is_err(), "{pointer}");
    }
    for field in ["complete", "accepted", "phases"] {
        let mut changed = original.clone(); changed[field] = json!(true);
        assert!(serde_json::from_value::<MusicVideoIntent>(changed).is_err(), "{field}");
    }
    assert!(!fixture.root.join("launched").exists());
}

#[test]
fn read_only_status_never_promotes_loose_child_outputs_into_an_accepted_handoff() {
    let fixture = fixture();
    let plan = music_video_recipe::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    let root = fixture.root.join("retained recipe");
    fs::create_dir(&root).unwrap(); fs::create_dir(root.join("children")).unwrap();
    fs::write(root.join("intent.json"), serde_json::to_vec(&plan.intent).unwrap()).unwrap();
    let before = snapshot(&fixture.root);
    let initial = music_video_recipe::status(&root).unwrap();
    assert!(!initial.complete);
    assert!(initial.outputs.is_empty());
    assert_eq!(initial.phases[0].state, MusicVideoPhaseState::Ready);
    assert_eq!(initial.phases[2].state, MusicVideoPhaseState::Pending);
    assert_eq!(snapshot(&fixture.root), before);

    let mut changed_settings = fixture.settings.clone();
    changed_settings["assembly"]["delivery_crf"] = json!(20);
    let mut changed_request = fixture.request(); changed_request.configuration = parse(&changed_settings).unwrap();
    let mut emitted_progress = false;
    assert!(music_video_recipe::resume(changed_request, &root, &CancellationToken::default(),
        |_| emitted_progress = true).is_err());
    assert!(!emitted_progress);
    assert_eq!(snapshot(&fixture.root), before, "changed intent must refuse before a writer or child transition");

    // A crash can leave a child directory. These loose files deliberately lack
    // v3 plan/checkpoint/validation/delivery authority and must never advance it.
    let child = root.join("children/prepare_frames"); fs::create_dir(&child).unwrap();
    fs::write(child.join("repaired_index.json"), fs::read(&fixture.index_path).unwrap()).unwrap();
    fs::write(child.join("complete.json"), br#"{"complete":true,"accepted":true}"#).unwrap();
    let before = snapshot(&fixture.root);
    let observed = music_video_recipe::status(&root).unwrap();
    assert!(!observed.complete);
    assert!(observed.outputs.is_empty());
    assert_eq!(observed.phases[0].state, MusicVideoPhaseState::IncompleteInitialization);
    assert_eq!(observed.phases[2].state, MusicVideoPhaseState::Pending);
    assert!(observed.phases[0].manifest.is_none());
    assert!(observed.phases[2].run_directory.is_none());
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
}
