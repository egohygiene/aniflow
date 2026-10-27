//! Synthetic assembly regressions authored for deferred #64 execution.
//!
//! These fixtures exercise declarations, immutable planning and compatibility
//! identity. They do not qualify FFmpeg, actual muxing, AAC preservation or
//! perceptual quality. Private media regressions cover fixed argv and complete
//! output observations; no native result is inferred from these fixtures.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use aniflow::temporal::{self, StreamSelection};
use aniflow::upscayl_recovery::UpscaylOutputIndex;
use aniflow::video_assembly::{
    self, AssemblyConfiguration, AssemblyOperation, AssemblyRequest, AssemblyStageKind,
};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::CancellationToken;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn caller_settings() -> Value {
    json!({
        "schema":"aniflow.video-assembly-configuration/v1",
        "ffmpeg":{"executable":"/synthetic/ffmpeg", "version":"8.0.1", "sha256":"a".repeat(64)},
        "ffprobe":{"executable":"/synthetic/ffprobe", "version":"8.0.1", "sha256":"b".repeat(64)}
    })
}

fn configuration() -> AssemblyConfiguration {
    AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&caller_settings()).unwrap()).unwrap()
}

fn operation(stage: AssemblyStageKind) -> AssemblyOperation {
    AssemblyOperation {
        schema:"aniflow.video-assembly-operation/v1".to_owned(), stage,
        settings:configuration(), source_sha256:"c".repeat(64), source_byte_count:128,
        source_selection:StreamSelection::default(), frame_index_sha256:"d".repeat(64),
        index_schema:RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), frame_count:3, width:2, height:2,
    }
}

#[test]
fn caller_defaults_materialize_without_tool_discovery_and_preserve_operation_identity() {
    let settings = configuration();
    assert_eq!(settings.timeout_milliseconds, 21_600_000);
    assert_eq!(settings.maximum_capture_bytes, 67_108_864);
    assert_eq!(settings.delivery_crf, 18);
    assert_eq!(settings.preset, "slow");
    let explicit = AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(settings, explicit);
    let first = operation(AssemblyStageKind::Master);
    let mut second = first.clone();
    second.settings = explicit;
    assert_eq!(first.provider_configuration().unwrap(), second.provider_configuration().unwrap());
}

#[test]
fn caller_refuses_open_argv_unpinned_versions_aliases_and_invalid_bounds() {
    let original = serde_json::to_value(configuration()).unwrap();
    for (pointer, value) in [
        ("/ffmpeg/version", json!("8.0.2")),
        ("/ffprobe/version", json!("8.0.1-custom")),
        ("/ffmpeg/executable", json!("ffmpeg")),
        ("/ffmpeg/executable", json!("/synthetic/../ffmpeg")),
        ("/ffmpeg/executable", json!("/synthetic/ffmpeg\n")),
        ("/ffprobe/executable", json!("/synthetic/ffmpeg")),
        ("/ffprobe/sha256", json!("B".repeat(64))),
        ("/timeout_milliseconds", json!(0)),
        ("/timeout_milliseconds", json!(21_600_001)),
        ("/maximum_capture_bytes", json!(1023)),
        ("/maximum_capture_bytes", json!(67_108_865)),
        ("/maximum_source_bytes", json!(0)),
        ("/maximum_output_bytes", json!(1023)),
        ("/maximum_frame_bytes", json!(268_435_457_u64)),
        ("/maximum_total_frame_bytes", json!(1_099_511_627_777_u64)),
        ("/maximum_pixels", json!(134_217_729_u64)),
        ("/maximum_working_bytes", json!(1_048_575)),
        ("/minimum_free_bytes", json!(1_099_511_627_777_u64)),
        ("/delivery_crf", json!(52)),
        ("/preset", json!("placebo")),
    ] {
        let mut candidate = original.clone();
        *candidate.pointer_mut(pointer).unwrap() = value;
        assert!(AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&candidate).unwrap()).is_err(), "{pointer}");
    }
    for field in ["arguments", "audio_codec", "normalize_timestamps", "master_container", "download"] {
        let mut candidate = original.clone();
        candidate[field] = json!(true);
        assert!(AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&candidate).unwrap()).is_err(), "{field}");
    }
    let mut candidate = original;
    candidate["ffmpeg"]["arguments"] = json!(["-shortest"]);
    assert!(AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&candidate).unwrap()).is_err());
}

#[test]
fn stage_source_index_geometry_audio_selection_and_tool_pins_change_compatibility_identity() {
    let master = operation(AssemblyStageKind::Master);
    let original = master.provider_configuration().unwrap().effective_configuration_sha256;
    let mut variants = Vec::new();
    let mut candidate = master.clone(); candidate.stage = AssemblyStageKind::Delivery; variants.push(candidate);
    let mut candidate = master.clone(); candidate.source_sha256 = "e".repeat(64); variants.push(candidate);
    let mut candidate = master.clone(); candidate.frame_index_sha256 = "e".repeat(64); variants.push(candidate);
    let mut candidate = master.clone(); candidate.width = 4; variants.push(candidate);
    let mut candidate = master.clone(); candidate.frame_count = 4; variants.push(candidate);
    let mut candidate = master.clone(); candidate.source_selection.video_stream = Some(0); variants.push(candidate);
    let mut candidate = master.clone(); candidate.source_selection.no_audio = true; variants.push(candidate);
    let mut candidate = master.clone(); candidate.settings.ffmpeg.sha256 = "e".repeat(64); variants.push(candidate);
    let mut candidate = master.clone(); candidate.settings.ffprobe.sha256 = "e".repeat(64); variants.push(candidate);
    let mut candidate = master; candidate.settings.delivery_crf = 19; variants.push(candidate);
    for candidate in variants {
        assert_ne!(original, candidate.provider_configuration().unwrap().effective_configuration_sha256);
    }
}

#[test]
fn operation_refuses_unknown_index_profiles_and_geometry_outside_delivery_contract() {
    let original = serde_json::to_value(operation(AssemblyStageKind::Master)).unwrap();
    for (field, value) in [
        ("schema", json!("aniflow.video-assembly-operation/v2")),
        ("index_schema", json!("aniflow.guessed-frame-index/v1")),
        ("source_byte_count", json!(0)),
        ("frame_count", json!(0)),
        ("frame_count", json!(100001)),
        ("width", json!(1)),
        ("width", json!(3)),
        ("height", json!(0)),
        ("height", json!(3)),
        ("source_sha256", json!("unknown")),
    ] {
        let mut candidate = original.clone();
        candidate[field] = value;
        let parsed: AssemblyOperation = serde_json::from_value(candidate).unwrap();
        assert!(parsed.validate().is_err(), "{field}");
    }
    let mut candidate = original;
    candidate["historical_processing"] = json!("verified");
    assert!(serde_json::from_value::<AssemblyOperation>(candidate).is_err());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 { crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1)); }
    }
    !crc
}

fn png(width: u32, height: u32, color: u8) -> Vec<u8> {
    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(data);
        let mut payload = kind.to_vec(); payload.extend_from_slice(data);
        output.extend_from_slice(&crc32(&payload).to_be_bytes());
    }
    let mut output = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = width.to_be_bytes().to_vec(); header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]); chunk(&mut output, b"IHDR", &header);
    let mut scanlines = Vec::new();
    for _ in 0..height { scanlines.push(0); scanlines.extend(vec![color; width as usize * 3]); }
    let length = u16::try_from(scanlines.len()).unwrap();
    let mut compressed = vec![0x78, 0x01, 0x01];
    compressed.extend_from_slice(&length.to_le_bytes()); compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(&scanlines);
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in scanlines { a = (a + u32::from(byte)) % 65_521; b = (b + a) % 65_521; }
    compressed.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut output, b"IDAT", &compressed); chunk(&mut output, b"IEND", &[]); output
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    request: AssemblyRequest,
    index: RifeFrameIndex,
}

impl Fixture {
    fn write_index(&self) {
        fs::write(&self.request.frame_index, serde_json::to_vec(&self.index).unwrap()).unwrap();
    }
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("assembly 音 with spaces");
    fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    let source_bytes = b"synthetic source identity; no native media claim";
    fs::write(&source, source_bytes).unwrap();
    let frames_directory = root.join("declared frames"); fs::create_dir(&frames_directory).unwrap();
    let frames = (0..3).map(|ordinal| {
        let filename = format!("frame_{ordinal:08}.png");
        let bytes = png(2, 2, ordinal as u8 + 1);
        fs::write(frames_directory.join(&filename), &bytes).unwrap();
        RifeFrame { ordinal, filename, sha256:sha256(&bytes) }
    }).collect();
    let inspection = temporal::from_probe_documents(&sha256(source_bytes), source_bytes.len() as u64,
        &StreamSelection::default(), &json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
            "time_base":"1/24","start_pts":0,"duration_ts":3,"r_frame_rate":"24/1","avg_frame_rate":"24/1",
            "width":2,"height":2,"pix_fmt":"yuv420p","disposition":{"attached_pic":0}}]}),
        &[(0, json!({"frames":(0..3).map(|ordinal| json!({"stream_index":0,"pts":ordinal,"pkt_dts":ordinal,
            "pkt_duration":1,"key_frame":if ordinal == 0 {1} else {0}})).collect::<Vec<_>>()}),
            json!({"packets":(0..3).map(|ordinal| json!({"stream_index":0,"pts":ordinal,"dts":ordinal,
                "duration":1})).collect::<Vec<_>>()}))],
    ).unwrap();
    let index = RifeFrameIndex { schema:RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection:inspection,
        width:2, height:2, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(), frames, cut_before_ordinals:vec![2] };
    let mut settings = configuration();
    for (name, pin) in [("ffmpeg", &mut settings.ffmpeg), ("ffprobe", &mut settings.ffprobe)] {
        pin.executable = root.join(format!("forbidden {name}"));
        // A plan accidentally launching a tool fails loudly and leaves evidence.
        let text = format!("#!/bin/sh\nprintf launched > '{}'/launched\nexit 99\n", root.display());
        fs::write(&pin.executable, &text).unwrap();
        fs::set_permissions(&pin.executable, fs::Permissions::from_mode(0o700)).unwrap();
        pin.sha256 = sha256(text.as_bytes());
    }
    let index_path = root.join("frame index.json");
    let request = AssemblyRequest::new(source, frames_directory, index_path, settings,
        PathBuf::from(env!("CARGO_BIN_EXE_aniflow")));
    let result = Fixture { _directory:directory, root, request, index };
    result.write_index(); result
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
fn plan_is_read_only_and_binds_master_then_delivery_to_existing_validation_owners() {
    let fixture = fixture();
    let before = snapshot(&fixture.root);
    let plan = video_assembly::plan(&fixture.request, &CancellationToken::default()).unwrap();
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
    assert_eq!(plan.payload.inputs.len(), 3);
    assert!(plan.payload.inputs.iter().any(|input| input.id == "source_video"
        && input.content_sha256 == fixture.index.temporal_inspection.source_sha256));
    assert_eq!(plan.payload.stages.len(), 2);
    let master = &plan.payload.stages[0];
    let delivery = &plan.payload.stages[1];
    assert_eq!(master.id, "assemble_master");
    assert_eq!(master.capability.id, "aniflow/assemble-master");
    assert!(master.depends_on.is_empty());
    assert_eq!(delivery.id, "encode_delivery");
    assert_eq!(delivery.capability.id, "aniflow/encode-delivery");
    assert_eq!(delivery.depends_on, ["assemble_master"]);
    assert!(delivery.inputs.iter().any(|binding| binding.port == "master" && binding.artifacts == ["lossless_master"]));
    assert!(!delivery.inputs.iter().any(|binding| binding.artifacts.contains(&"source_frames".to_owned())));
    for (stage, id, path) in [(master, "lossless_master", "artifacts/assembly/master/master.mp4"),
        (delivery, "h264_delivery", "artifacts/assembly/delivery/delivery.mp4")] {
        assert!(stage.outputs.iter().any(|binding| binding.port == "media"
            && binding.artifacts.iter().any(|artifact| artifact.id == id && artifact.relative_path == path)));
        assert_eq!(stage.validation_providers.len(), 1);
        assert!(stage.validations.iter().any(|validation| validation.artifact == id
            && validation.contract == aniflow::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1));
        assert_eq!(stage.provider_lock.payload.tools.len(), 2);
        for (name, pin) in [("ffmpeg", &fixture.request.configuration.ffmpeg), ("ffprobe", &fixture.request.configuration.ffprobe)] {
            assert!(stage.provider_lock.payload.tools.iter().any(|tool|
                tool.id == name && tool.version == "8.0.1" && tool.sha256.as_deref() == Some(pin.sha256.as_str())));
        }
    }
    assert_eq!(plan.payload.outputs.len(), 4);
    assert!(plan.payload.outputs.iter().any(|output| output.id == "lossless_master"
        && output.required_validations == ["lossless_master_temporal"]));
    assert!(plan.payload.outputs.iter().any(|output| output.id == "h264_delivery"
        && output.required_validations == ["h264_delivery_temporal"]));
    assert_eq!(plan, video_assembly::plan(&fixture.request, &CancellationToken::default()).unwrap());
}

#[test]
fn upscayl_spatial_view_uses_actual_output_geometry_without_rewriting_source_history() {
    let fixture = fixture();
    let original = fixture.index.clone();
    let frames = original.frames.iter().map(|frame| {
        let bytes = png(4, 4, frame.ordinal as u8 + 11);
        fs::write(fixture.request.frames_directory.join(&frame.filename), &bytes).unwrap();
        RifeFrame { ordinal:frame.ordinal, filename:frame.filename.clone(), sha256:sha256(&bytes) }
    }).collect();
    let spatial = UpscaylOutputIndex { schema:"aniflow.upscayl-output-index/v1".to_owned(), source_index:original.clone(),
        output_width:4, output_height:4, output_profile:original.frame_profile.clone(), frames };
    let bytes = serde_json::to_vec(&spatial).unwrap();
    fs::write(&fixture.request.frame_index, &bytes).unwrap();
    let before = snapshot(&fixture.root);
    let plan = video_assembly::plan(&fixture.request, &CancellationToken::default()).unwrap();
    assert_eq!(snapshot(&fixture.root), before);
    assert_eq!(spatial.source_index, original);
    assert_eq!(spatial.source_index.width, 2);
    assert_eq!(spatial.source_index.temporal_inspection, fixture.index.temporal_inspection);
    for (stage, kind) in plan.payload.stages.iter().zip([AssemblyStageKind::Master, AssemblyStageKind::Delivery]) {
        let expected = AssemblyOperation {
            schema:"aniflow.video-assembly-operation/v1".to_owned(), stage:kind,
            settings:fixture.request.configuration.clone(), source_sha256:original.temporal_inspection.source_sha256.clone(),
            source_byte_count:original.temporal_inspection.source_size_bytes,
            source_selection:original.temporal_inspection.selection_intent.clone(),
            frame_index_sha256:sha256(&bytes), index_schema:spatial.schema.clone(),
            frame_count:3, width:4, height:4,
        };
        assert_eq!(stage.provider_lock.payload.effective_configuration_sha256,
            expected.provider_configuration().unwrap().effective_configuration_sha256);
    }
}

#[test]
fn stale_source_frame_or_tool_bytes_refuse_planning_without_creating_outputs() {
    for target in ["source", "frame", "tool", "missing", "extra"] {
        let fixture = fixture();
        match target {
            "source" => fs::write(&fixture.request.source_video, b"changed source").unwrap(),
            "frame" => fs::write(fixture.request.frames_directory.join(&fixture.index.frames[0].filename), png(2, 2, 99)).unwrap(),
            "tool" => fs::write(&fixture.request.configuration.ffmpeg.executable, b"changed binary identity").unwrap(),
            "missing" => fs::remove_file(fixture.request.frames_directory.join(&fixture.index.frames[0].filename)).unwrap(),
            "extra" => fs::write(fixture.request.frames_directory.join("undeclared.png"), png(2, 2, 1)).unwrap(),
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.root);
        assert!(video_assembly::plan(&fixture.request, &CancellationToken::default()).is_err(), "{target}");
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
    }
}

#[test]
fn changed_exact_tool_or_index_pins_require_a_different_plan() {
    let mut fixture = fixture();
    let baseline = video_assembly::plan(&fixture.request, &CancellationToken::default()).unwrap();
    let executable = &fixture.request.configuration.ffmpeg.executable;
    let mut bytes = fs::read(executable).unwrap(); bytes.extend_from_slice(b"# different pinned build\n");
    fs::write(executable, &bytes).unwrap();
    fixture.request.configuration.ffmpeg.sha256 = sha256(&bytes);
    let changed_tool = video_assembly::plan(&fixture.request, &CancellationToken::default()).unwrap();
    assert_ne!(baseline.plan_sha256, changed_tool.plan_sha256);
    for (original, changed) in baseline.payload.stages.iter().zip(&changed_tool.payload.stages) {
        assert_ne!(original.provider_lock.lock_sha256, changed.provider_lock.lock_sha256);
    }
    fixture.index.cut_before_ordinals = vec![1]; fixture.write_index();
    let changed_index = video_assembly::plan(&fixture.request, &CancellationToken::default()).unwrap();
    assert_ne!(changed_tool.plan_sha256, changed_index.plan_sha256);
    assert!(!fixture.root.join("launched").exists());
}

#[test]
fn incompatible_clock_profile_and_geometry_are_visible_plan_refusals() {
    for mutation in ["clock", "profile", "odd_width", "unsupported_index", "pixels", "working"] {
        let mut fixture = fixture();
        match mutation {
            "clock" => {
                // Reuse the repository's observed nonzero-origin inspection;
                // changing a field on an accepted inspection could make its
                // own canonical evidence invalid for an unrelated reason.
                let document: Value = serde_json::from_slice(include_bytes!("fixtures/temporal/positive-origin.json")).unwrap();
                let observations = document["observations"].as_array().unwrap().iter().map(|observation| (
                    observation["stream_index"].as_u64().unwrap() as u32,
                    observation["frames"].clone(), observation["packets"].clone(),
                )).collect::<Vec<_>>();
                fixture.index.temporal_inspection = temporal::from_probe_documents(
                    &fixture.index.temporal_inspection.source_sha256,
                    fixture.index.temporal_inspection.source_size_bytes,
                    &StreamSelection::default(), &document["inventory"], &observations,
                ).unwrap();
                assert!(!fixture.index.temporal_inspection.processing.supported);
                fixture.write_index();
            }
            "profile" => { fixture.index.frame_profile = "unknown_hdr".to_owned(); fixture.write_index(); }
            "odd_width" => { fixture.index.width = 3; fixture.write_index(); }
            "unsupported_index" => {
                let mut value = serde_json::to_value(&fixture.index).unwrap();
                value["schema"] = json!("aniflow.rife-output-index/v1");
                fs::write(&fixture.request.frame_index, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "pixels" => fixture.request.configuration.maximum_pixels = 3,
            "working" => fixture.request.configuration.maximum_working_bytes = 1_048_576,
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.root);
        assert!(video_assembly::plan(&fixture.request, &CancellationToken::default()).is_err(), "{mutation}");
        assert_eq!(snapshot(&fixture.root), before);
    }
}

#[test]
fn cancellation_and_output_overlap_fail_before_native_invocation_or_workspace_creation() {
    let fixture = fixture();
    let cancellation = CancellationToken::default(); cancellation.cancel();
    let before = snapshot(&fixture.root);
    assert!(video_assembly::plan(&fixture.request, &cancellation).is_err());
    let nested = fixture.request.frames_directory.join("runs");
    let mut emitted_progress = false;
    assert!(video_assembly::run(fixture.request.clone(), Some(nested.clone()), &CancellationToken::default(),
        |_| emitted_progress = true).is_err());
    assert!(!emitted_progress);
    assert!(!nested.exists());
    assert_eq!(snapshot(&fixture.root), before);
}
