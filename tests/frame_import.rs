//! Synthetic import-contract regressions for #36; execution is deferred to #64.
//!
//! The fake decoder exercises process/evidence admission, not real PNG decoding
//! or FFmpeg compatibility. Native decoder qualification remains separate.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

use aniflow::frame_import::{FrameImportConfiguration, execute_invocation};
use aniflow::temporal::{self, StreamSelection};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::{ArtifactKind, ArtifactRole, ProviderInvocationArtifactBinding, ProviderInvocationRequest, StreamRole};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 { crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1))); }
    }
    !crc
}

fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut payload = kind.to_vec();
    payload.extend_from_slice(data);
    output.extend_from_slice(&crc32(&payload).to_be_bytes());
}

fn png(color: [u8; 3]) -> Vec<u8> {
    let mut output = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = Vec::new();
    header.extend_from_slice(&1_u32.to_be_bytes());
    header.extend_from_slice(&1_u32.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut output, b"IHDR", &header);
    let scanline = [0, color[0], color[1], color[2]];
    let mut data = vec![0x78, 0x01, 0x01, 4, 0, 0xfb, 0xff];
    data.extend_from_slice(&scanline);
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in scanline { a = (a + u32::from(byte)) % 65_521; b = (b + a) % 65_521; }
    data.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut output, b"IDAT", &data);
    chunk(&mut output, b"IEND", &[]);
    output
}

const FRAMEHASH: &str = "#format: frame checksums\n#version: 2\n#hash: SHA256\n#software: synthetic fixture\n#tb 0: 1/25\n#media_type 0: video\n#codec_id 0: rawvideo\n#dimensions 0: 1x1\n#sar 0: 0/1\n#stream#, dts,        pts, duration,     size, hash\n0, 0, 0, 1, 3, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";

fn decoder_script(body: &str) -> String {
    format!("#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then printf 'ffmpeg version 1.0.0\\n'; exit 0; fi\n{body}\n")
}

fn successful_decoder() -> String {
    decoder_script(&format!("cat <<'FRAMEHASH'\n{FRAMEHASH}FRAMEHASH"))
}

struct Fixture {
    _directory: tempfile::TempDir,
    request: ProviderInvocationRequest,
    configuration: FrameImportConfiguration,
    index: RifeFrameIndex,
}

impl Fixture {
    fn input(&self, port: &str) -> &Path {
        &self.request.inputs.iter().find(|value| value.port == port).unwrap().path
    }
    fn output(&self, port: &str) -> &Path {
        &self.request.outputs.iter().find(|value| value.port == port).unwrap().path
    }
    fn write_index(&self) { fs::write(self.input("index"), serde_json::to_vec(&self.index).unwrap()).unwrap(); }
    fn repin_decoder(&mut self, script: &str) {
        fs::write(&self.configuration.ffmpeg.executable, script).unwrap();
        self.configuration.ffmpeg.sha256 = sha256(script.as_bytes());
        self.request.configuration = self.configuration.provider_configuration().unwrap();
    }
}

fn make_fixture(script: &str) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("frames with spaces 音");
    fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    let source_bytes = b"synthetic source identity, not a decodable video";
    fs::write(&source, source_bytes).unwrap();
    let frames_directory = root.join("input frames");
    fs::create_dir(&frames_directory).unwrap();
    let mut frames = Vec::new();
    for ordinal in 0..2_u64 {
        let filename = format!("frame_{ordinal:08}.png");
        let bytes = png([ordinal as u8, 2, 3]);
        fs::write(frames_directory.join(&filename), &bytes).unwrap();
        frames.push(RifeFrame { ordinal, filename, sha256: sha256(&bytes) });
    }
    let inspection = temporal::from_probe_documents(
        &sha256(source_bytes), source_bytes.len() as u64, &StreamSelection::default(),
        &json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
            "time_base":"1/24","start_pts":0,"duration_ts":2,"r_frame_rate":"24/1",
            "avg_frame_rate":"24/1","width":1,"height":1,"pix_fmt":"yuv420p",
            "disposition":{"attached_pic":0}}]}),
        &[(0, json!({"frames":[
            {"stream_index":0,"pts":0,"pkt_dts":0,"pkt_duration":1,"key_frame":1},
            {"stream_index":0,"pts":1,"pkt_dts":1,"pkt_duration":1,"key_frame":0}]}),
            json!({"packets":[
                {"stream_index":0,"pts":0,"dts":0,"duration":1},
                {"stream_index":0,"pts":1,"dts":1,"duration":1}]}))],
    ).unwrap();
    let index = RifeFrameIndex {
        schema: RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection: inspection,
        width: 1, height: 1, frame_profile: "rgb8_sdr_bt709_square_unrotated".to_owned(),
        frames, cut_before_ordinals: Vec::new(),
    };
    let index_path = root.join("index.json");
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let tool = root.join("synthetic ffmpeg");
    fs::write(&tool, script).unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let configuration = FrameImportConfiguration::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.frame-import-configuration/v1",
        "ffmpeg":{"executable":tool,"version":"1.0.0","sha256":sha256(script.as_bytes())},
        "tool_timeout_milliseconds":1000,
        "maximum_source_bytes":1048576,"maximum_frame_bytes":1024,
        "maximum_total_frame_bytes":1048576,"maximum_pixels":1
    })).unwrap()).unwrap();
    let output = root.join("output");
    fs::create_dir(&output).unwrap();
    let request = ProviderInvocationRequest::new(
        "import_frames", "b".repeat(64), configuration.provider_configuration().unwrap(),
        vec![
            ProviderInvocationArtifactBinding::new("source", "source_video", "video/*",
                ArtifactRole::TemporalSource, Some(StreamRole::Video), ArtifactKind::File, source).unwrap(),
            ProviderInvocationArtifactBinding::new("frames", "source_frames", "application/vnd.aniflow.rife-frames",
                ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::Directory, frames_directory).unwrap(),
            ProviderInvocationArtifactBinding::new("index", "source_index", "application/vnd.aniflow.rife-frame-index+json",
                ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, index_path).unwrap(),
        ],
        vec![
            ProviderInvocationArtifactBinding::new("frames", "imported_frames", "application/vnd.aniflow.rife-frames",
                ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::Directory, output.join("frames")).unwrap(),
            ProviderInvocationArtifactBinding::new("evidence", "import_evidence", "application/vnd.aniflow.frame-import-report+json",
                ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, output.join("import.json")).unwrap(),
        ],
    ).unwrap();
    Fixture { _directory: directory, request, configuration, index }
}

#[test]
fn import_configuration_refuses_unknown_fields_unbounded_limits_and_relative_tools() {
    let fixture = make_fixture(&successful_decoder());
    let original = serde_json::to_value(&fixture.configuration).unwrap();
    for (field, value) in [
        ("maximum_pixels", json!(8_400_001)), ("maximum_pixels", json!(0)),
        ("maximum_frame_bytes", json!(67_108_865_u64)),
        ("maximum_source_bytes", json!(1_099_511_627_777_u64)),
        ("maximum_total_frame_bytes", json!(0)),
        ("tool_timeout_milliseconds", json!(120_001)), ("unknown", json!(true)),
    ] {
        let mut altered = original.clone();
        altered[field] = value;
        assert!(FrameImportConfiguration::from_json_slice(&serde_json::to_vec(&altered).unwrap()).is_err());
    }
    let mut altered = original;
    altered["ffmpeg"]["executable"] = json!("ffmpeg");
    assert!(FrameImportConfiguration::from_json_slice(&serde_json::to_vec(&altered).unwrap()).is_err());
}

#[test]
fn synthetic_import_preserves_all_png_bytes_and_distinguishes_observation_from_history() {
    let fixture = make_fixture(&successful_decoder());
    let originals: Vec<_> = fixture.index.frames.iter().map(|frame|
        fs::read(fixture.input("frames").join(&frame.filename)).unwrap()).collect();
    let source_before = fs::read(fixture.input("source")).unwrap();
    let index_before = fs::read(fixture.input("index")).unwrap();
    execute_invocation(&fixture.request).unwrap();
    let report: Value = serde_json::from_slice(&fs::read(fixture.output("evidence")).unwrap()).unwrap();
    assert_eq!(report["schema"], "aniflow.frame-import-report/v1");
    assert_eq!(report["historical_processing"], "unknown");
    assert_eq!(report["source_relationship"], "declared_not_verified");
    assert_eq!(report["provider_lock_sha256"], "b".repeat(64));
    assert_eq!(report["source_sha256"], sha256(&source_before));
    assert_eq!(report["frames"].as_array().unwrap().len(), 2);
    assert_eq!(report["index"], serde_json::to_value(&fixture.index).unwrap());
    for (ordinal, (frame, bytes)) in fixture.index.frames.iter().zip(originals).enumerate() {
        assert_eq!(fs::read(fixture.input("frames").join(&frame.filename)).unwrap(), bytes);
        assert_eq!(fs::read(fixture.output("frames").join(&frame.filename)).unwrap(), bytes);
        assert_eq!(report["frames"][ordinal]["decoded_byte_count"], 3);
        assert_eq!(report["frames"][ordinal]["byte_count"], bytes.len());
        assert_eq!(report["frames"][ordinal]["sha256"], sha256(&bytes));
    }
    assert_eq!(fs::read(fixture.input("source")).unwrap(), source_before);
    assert_eq!(fs::read(fixture.input("index")).unwrap(), index_before);
}

#[test]
fn import_requires_exact_complete_membership_including_unselected_frames() {
    for missing in [true, false] {
        let fixture = make_fixture(&successful_decoder());
        if missing {
            fs::remove_file(fixture.input("frames").join(&fixture.index.frames[1].filename)).unwrap();
        } else {
            fs::write(fixture.input("frames").join("extra.png"), png([3, 2, 1])).unwrap();
        }
        assert!(execute_invocation(&fixture.request).is_err());
        assert!(!fixture.output("evidence").exists());
    }
}

#[test]
fn matching_file_digest_cannot_hide_corrupt_png_structure() {
    let mut fixture = make_fixture(&successful_decoder());
    let path = fixture.input("frames").join(&fixture.index.frames[0].filename);
    let mut corrupted = fs::read(&path).unwrap();
    corrupted[29] ^= 1;
    fs::write(&path, &corrupted).unwrap();
    fixture.index.frames[0].sha256 = sha256(&corrupted);
    fixture.write_index();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
}

#[test]
fn successful_process_cannot_hide_wrong_decoded_size_missing_rows_or_extra_frames() {
    for output in [
        FRAMEHASH.replace(", 3, ", ", 2, "),
        FRAMEHASH.lines().filter(|line| line.starts_with('#')).collect::<Vec<_>>().join("\n"),
        format!("{FRAMEHASH}0, 1, 1, 1, 3, {}\n", "a".repeat(64)),
        FRAMEHASH.replace("#hash: SHA256", "#hash: MD5"),
        FRAMEHASH.replace(&"a".repeat(64), &"z".repeat(64)),
    ] {
        let script = decoder_script(&format!("cat <<'FRAMEHASH'\n{output}\nFRAMEHASH"));
        let fixture = make_fixture(&script);
        assert!(execute_invocation(&fixture.request).is_err());
        assert!(!fixture.output("evidence").exists());
    }
    let fixture = make_fixture(&decoder_script("exit 9"));
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
}

#[test]
fn source_identity_must_match_the_declared_temporal_mapping() {
    let fixture = make_fixture(&successful_decoder());
    fs::write(fixture.input("source"), b"different source video identity").unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());

    let mut fixture = make_fixture(&successful_decoder());
    fixture.index.temporal_inspection.source_size_bytes += 1;
    fixture.write_index();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
}

#[test]
fn completed_destination_is_never_overwritten() {
    let fixture = make_fixture(&successful_decoder());
    execute_invocation(&fixture.request).unwrap();
    let evidence = fs::read(fixture.output("evidence")).unwrap();
    let frame = fs::read(fixture.output("frames").join(&fixture.index.frames[0].filename)).unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert_eq!(fs::read(fixture.output("evidence")).unwrap(), evidence);
    assert_eq!(fs::read(fixture.output("frames").join(&fixture.index.frames[0].filename)).unwrap(), frame);
}

#[test]
fn changed_original_during_decode_prevents_completion() {
    let mut fixture = make_fixture(&successful_decoder());
    // Shell quoting is solely fixture authoring; the production adapter uses argv.
    let source = fixture.input("source").to_str().unwrap().replace('\'', "'\\''");
    let script = decoder_script(&format!(
        "printf 'changed during decode' > '{source}'\ncat <<'FRAMEHASH'\n{FRAMEHASH}FRAMEHASH"
    ));
    fixture.repin_decoder(&script);
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
}

#[test]
fn symlink_members_and_symlinked_frame_roots_are_not_imported() {
    let fixture = make_fixture(&successful_decoder());
    let original = fixture.input("frames").join(&fixture.index.frames[0].filename);
    let elsewhere = fixture.input("frames").parent().unwrap().join("retained.png");
    fs::rename(&original, &elsewhere).unwrap();
    symlink(&elsewhere, &original).unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());

    let mut fixture = make_fixture(&successful_decoder());
    let link = fixture.input("frames").with_extension("link");
    symlink(fixture.input("frames"), &link).unwrap();
    fixture.request.inputs.iter_mut().find(|value| value.port == "frames").unwrap().path = link;
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
}

#[test]
fn decoder_pin_and_exact_version_are_required_before_acceptance() {
    let fixture = make_fixture(&successful_decoder());
    fs::write(&fixture.configuration.ffmpeg.executable, decoder_script("exit 0")).unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
    let mut fixture = make_fixture(&successful_decoder());
    fixture.configuration.ffmpeg.version = "2.0.0".to_owned();
    fixture.request.configuration = fixture.configuration.provider_configuration().unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.output("evidence").exists());
}

#[test]
fn library_plan_binds_three_inputs_without_launching_a_decoder_or_creating_outputs() {
    let fixture = make_fixture(&decoder_script("exit 99"));
    let request = aniflow::frame_import::FrameImportRequest::new(
        fixture.input("source"), fixture.input("frames"), fixture.input("index"),
        fixture.configuration.clone(), fixture.configuration.ffmpeg.executable.clone(),
    );
    let cancellation = aniflow::CancellationToken::default();
    let plan = aniflow::frame_import::plan(&request, &cancellation).unwrap();
    assert_eq!(plan.payload.inputs.len(), 3);
    assert!(plan.payload.inputs.iter().any(|input|
        input.id == "source_video" && input.content_sha256 == fixture.index.temporal_inspection.source_sha256));
    assert!(!fixture.output("frames").exists());
    assert!(!fixture.output("evidence").exists());
    cancellation.cancel();
    assert!(aniflow::frame_import::plan(&request, &cancellation).is_err());
    assert!(!fixture.output("frames").exists());
}
