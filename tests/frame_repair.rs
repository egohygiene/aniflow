//! Synthetic selective-repair contract coverage, authored for later #64 execution.
//!
//! These substitute tools exercise argv, identity and failure admission. They do
//! not prove PNG decompression, native tool compatibility, visual repair or an
//! accepted standalone historical import. The public plan owns import → repair.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use aniflow::frame_repair::{
    FrameRepairConfiguration, FrameRepairOperation, FrameRepairRequest, execute_invocation,
};
use aniflow::frame_selection::InclusiveFrameRange;
use aniflow::temporal::{self, StreamSelection};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::{ArtifactKind, ArtifactRole, ProviderInvocationArtifactBinding, ProviderInvocationRequest, StreamRole};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

// Integration fixtures follow the existing pipeline tests' canonical JSON
// convention; this does not expose an internal serialization helper publicly.
fn canonical_sha256(value: &impl Serialize) -> String {
    fn canonicalize(value: Value) -> Value {
        match value {
            Value::Object(values) => Value::Object(values.into_iter()
                .map(|(key, value)| (key, canonicalize(value))).collect()),
            Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
            scalar => scalar,
        }
    }
    sha256(&serde_json::to_vec(&canonicalize(serde_json::to_value(value).unwrap())).unwrap())
}

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

const FRAMEHASH: &str = "#format: frame checksums\n#version: 2\n#hash: SHA256\n#software: synthetic fixture\n#tb 0: 1/25\n#media_type 0: video\n#codec_id 0: rawvideo\n#dimensions 0: 1x1\n#sar 0: 0/1\n#stream#, dts, pts, duration, size, hash\n0, 0, 0, 1, 3, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";

fn decoder_script(output: &str) -> String {
    format!("#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then printf 'ffmpeg version 1.0.0\\n'; exit 0; fi\ncat <<'FRAMEHASH'\n{output}\nFRAMEHASH\n")
}

// Shell text is confined to synthetic fixtures; production passes literal argv.
fn shell_literal(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

fn write_executable(path: &Path, script: &str) {
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn write_png_command(bytes: &[u8]) -> String {
    let octal = bytes.iter().map(|byte| format!("\\{byte:03o}")).collect::<String>();
    format!("printf '{octal}' > \"$2\"")
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    calls: PathBuf,
    index: RifeFrameIndex,
    operation: FrameRepairOperation,
    request: ProviderInvocationRequest,
}

impl Fixture {
    fn input(&self, port: &str) -> &Path {
        &self.request.inputs.iter().find(|value| value.port == port).unwrap().path
    }
    fn output(&self, port: &str) -> &Path {
        &self.request.outputs.iter().find(|value| value.port == port).unwrap().path
    }
    fn update_operation(&mut self) {
        self.request.configuration = self.operation.provider_configuration().unwrap();
    }
    fn set_processor(&mut self, body: &str) {
        let script = format!(
            "#!/bin/sh\n[ \"$#\" -eq 2 ] || exit 80\n[ -f \"$1\" ] || exit 81\nprintf 'call\\n' >> {}\n{body}\n",
            shell_literal(&self.calls),
        );
        let tool = &mut self.operation.settings.processor.tool;
        write_executable(&tool.executable, &script);
        tool.sha256 = sha256(script.as_bytes());
        self.update_operation();
    }
    fn report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.output("evidence")).unwrap()).unwrap()
    }
    fn import_report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.input("import")).unwrap()).unwrap()
    }
    fn write_import_report(&self, report: &Value) {
        fs::write(self.input("import"), serde_json::to_vec(report).unwrap()).unwrap();
    }
    fn rewrite_import(&self) {
        let settings = &self.operation.settings.import;
        let observations: Vec<_> = self.index.frames.iter().map(|frame| {
            let bytes = fs::read(self.input("frames").join(&frame.filename)).unwrap();
            json!({"ordinal":frame.ordinal,"filename":frame.filename,"sha256":frame.sha256,
                "byte_count":bytes.len(),"decoded_rgb24_sha256":"a".repeat(64),"decoded_byte_count":3})
        }).collect();
        let report = json!({
            "schema":"aniflow.frame-import-report/v1", "stage_id":"import_frames",
            "provider_lock_sha256":"b".repeat(64),
            "configuration_sha256":settings.provider_configuration().unwrap().effective_configuration_sha256,
            "source_sha256":self.index.temporal_inspection.source_sha256,
            "source_byte_count":self.index.temporal_inspection.source_size_bytes,
            "frame_index_sha256":canonical_sha256(&self.index),
            "index":self.index, "historical_processing":"unknown",
            "source_relationship":"declared_not_verified", "decode_method":"ffmpeg_png_rgb24_framehash_sha256",
            "decoder":{"id":"ffmpeg","version":settings.ffmpeg.version,"sha256":settings.ffmpeg.sha256},
            "frames":observations,
        });
        self.write_import_report(&report);
    }
    fn library_request(&self) -> FrameRepairRequest {
        FrameRepairRequest::new(
            &self.source, self.input("frames"), self.input("index"),
            self.operation.settings.clone(), self.operation.ranges.clone(),
            self.operation.settings.processor.tool.executable.clone(),
        )
    }
    fn assert_no_completed_report(&self) {
        assert!(!self.output("evidence").exists());
    }
}

fn make_fixture() -> Fixture { make_fixture_with_count(3) }

fn make_fixture_with_count(frame_count: u64) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("manual repair 音 with spaces");
    fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    let source_bytes = b"synthetic source identity, not native media";
    fs::write(&source, source_bytes).unwrap();
    let frames_directory = root.join("imported frames");
    fs::create_dir(&frames_directory).unwrap();
    let frames: Vec<_> = (0..frame_count).map(|ordinal| {
        let filename = format!("frame_{ordinal:08}.png");
        let bytes = png([(ordinal % 255) as u8 + 1, 2, 3]);
        fs::write(frames_directory.join(&filename), &bytes).unwrap();
        RifeFrame { ordinal, filename, sha256: sha256(&bytes) }
    }).collect();
    let inspection = temporal::from_probe_documents(
        &sha256(source_bytes), source_bytes.len() as u64, &StreamSelection::default(),
        &json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
            "time_base":"1/24","start_pts":0,"duration_ts":frame_count,"r_frame_rate":"24/1",
            "avg_frame_rate":"24/1","width":1,"height":1,"pix_fmt":"yuv420p",
            "disposition":{"attached_pic":0}}]}),
        &[(0, json!({"frames":(0..frame_count).map(|ordinal| json!({
            "stream_index":0,"pts":ordinal,"pkt_dts":ordinal,"pkt_duration":1,
            "key_frame":if ordinal == 0 {1} else {0}
        })).collect::<Vec<_>>()}), json!({"packets":(0..frame_count).map(|ordinal| json!({
            "stream_index":0,"pts":ordinal,"dts":ordinal,"duration":1
        })).collect::<Vec<_>>()}))],
    ).unwrap();
    let index = RifeFrameIndex {
        schema: RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection: inspection,
        width:1, height:1, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(),
        frames, cut_before_ordinals:vec![2],
    };
    let index_path = root.join("source index.json");
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let decoder_path = root.join("synthetic ffmpeg");
    let decoder = decoder_script(FRAMEHASH);
    write_executable(&decoder_path, &decoder);
    let processor_path = root.join("synthetic processor");
    let processor = "#!/bin/sh\ncp \"$1\" \"$2\"\n";
    write_executable(&processor_path, processor);
    let settings = FrameRepairConfiguration::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.frame-repair-configuration/v1",
        "import": {
            "schema":"aniflow.frame-import-configuration/v1",
            "ffmpeg":{"executable":decoder_path,"version":"1.0.0","sha256":sha256(decoder.as_bytes())},
            "tool_timeout_milliseconds":1000,"maximum_source_bytes":1048576,
            "maximum_frame_bytes":1024,"maximum_total_frame_bytes":1048576,"maximum_pixels":1
        },
        "processor":{"profile":"synthetic_manual_repair_v1",
            "tool":{"executable":processor_path,"version":"1.0.0","sha256":sha256(processor.as_bytes())},
            "arguments":["{input}","{output}"]}
    })).unwrap()).unwrap();
    let operation = FrameRepairOperation {
        schema:"aniflow.frame-repair-provider-configuration/v1".to_owned(), settings,
        ranges:vec![InclusiveFrameRange { first:2, last:2 }],
    };
    let output = root.join("output");
    fs::create_dir(&output).unwrap();
    let request = ProviderInvocationRequest::new(
        "repair_frames", "c".repeat(64), operation.provider_configuration().unwrap(),
        vec![
            ProviderInvocationArtifactBinding::new("frames", "imported_frames", "application/vnd.aniflow.rife-frames",
                ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::Directory, frames_directory).unwrap(),
            ProviderInvocationArtifactBinding::new("import", "import_evidence", "application/vnd.aniflow.frame-import-report+json",
                ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, root.join("import.json")).unwrap(),
            ProviderInvocationArtifactBinding::new("index", "source_index", "application/vnd.aniflow.rife-frame-index+json",
                ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, index_path).unwrap(),
        ],
        vec![
            ProviderInvocationArtifactBinding::new("frames", "repaired_frames", "application/vnd.aniflow.rife-frames",
                ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::Directory, output.join("frames")).unwrap(),
            ProviderInvocationArtifactBinding::new("index", "repaired_index", "application/vnd.aniflow.rife-frame-index+json",
                ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, output.join("index.json")).unwrap(),
            ProviderInvocationArtifactBinding::new("evidence", "repair_evidence", "application/vnd.aniflow.frame-repair-report+json",
                ArtifactRole::ValidationEvidence, Some(StreamRole::TimedMetadata), ArtifactKind::File, output.join("repair.json")).unwrap(),
        ],
    ).unwrap();
    let calls = root.join("processor calls.txt");
    let mut fixture = Fixture { _directory:directory, root, source, calls, index, operation, request };
    fixture.set_processor("cp \"$1\" \"$2\"");
    fixture.rewrite_import();
    fixture
}

#[test]
fn configuration_requires_exact_whole_argument_placeholders_and_finite_bounds() {
    let fixture = make_fixture();
    let original = serde_json::to_value(&fixture.operation.settings).unwrap();
    for arguments in [
        json!(["{input}"]), json!(["{input}","{output}","{input}"]),
        json!(["--input={input}","{output}"]), json!(["{input}","{output}.png"]),
        json!(["{input}","{output}","{unknown}"]),
        json!(["{input}","{output}","bad\nargument"]),
        json!(["{input}","{output}", ""]),
        json!(["{input}","{output}", "x".repeat(4097)]),
    ] {
        let mut value = original.clone();
        value["processor"]["arguments"] = arguments;
        assert!(FrameRepairConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    for (count, argument_bytes) in [(255, 1), (9, 4096)] {
        let mut arguments = vec!["{input}".to_owned(), "{output}".to_owned()];
        arguments.extend((0..count).map(|_| "x".repeat(argument_bytes)));
        let mut value = original.clone();
        value["processor"]["arguments"] = json!(arguments);
        assert!(FrameRepairConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut value = original.clone();
    value["maximum_working_bytes"] = json!(0);
    assert!(FrameRepairConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut value = original.clone();
    value["processor"]["tool"]["executable"] = json!("processor");
    assert!(FrameRepairConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut value = original;
    value["unknown"] = json!(true);
    assert!(FrameRepairConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn only_manual_selection_is_invoked_and_unselected_bytes_and_timing_are_preserved() {
    let mut fixture = make_fixture();
    let replacement = png([99, 2, 3]);
    fixture.set_processor(&write_png_command(&replacement));
    let originals: Vec<_> = fixture.index.frames.iter().map(|frame|
        fs::read(fixture.input("frames").join(&frame.filename)).unwrap()).collect();
    execute_invocation(&fixture.request).unwrap();
    assert_eq!(fs::read_to_string(&fixture.calls).unwrap().lines().count(), 1);
    let report = fixture.report();
    assert_eq!(report["selection"]["selected_frame_count"], 1);
    assert_eq!(report["selection"]["unselected_frame_count"], 2);
    assert_eq!(report["changed_frame_count"], 1);
    let output_index: RifeFrameIndex = serde_json::from_slice(&fs::read(fixture.output("index")).unwrap()).unwrap();
    let mut expected_index = fixture.index.clone();
    expected_index.frames[1].sha256 = sha256(&replacement);
    assert_eq!(output_index, expected_index);
    assert_eq!(report["output_index"], serde_json::to_value(&expected_index).unwrap());
    let timeline = fixture.index.temporal_inspection.video_timeline().unwrap();
    for (ordinal, (frame, original)) in fixture.index.frames.iter().zip(&originals).enumerate() {
        assert_eq!(fs::read(fixture.input("frames").join(&frame.filename)).unwrap(), *original);
        let output = fs::read(fixture.output("frames").join(&frame.filename)).unwrap();
        assert_eq!(output, if ordinal == 1 { replacement.clone() } else { original.clone() });
        assert_eq!(report["frames"][ordinal]["selected"], ordinal == 1);
        assert_eq!(report["frames"][ordinal]["bytes_changed"], ordinal == 1);
        assert_eq!(report["frames"][ordinal]["input_sha256"], frame.sha256);
        assert_eq!(report["frames"][ordinal]["source_time"], serde_json::to_value(&timeline.frames[ordinal].source_time).unwrap());
    }
}

#[test]
fn selected_identical_output_is_observed_without_claiming_a_visual_repair() {
    let mut fixture = make_fixture();
    // A generic tool's version is caller-declared; no invented --version protocol.
    fixture.operation.settings.processor.tool.version = "9.8.7".to_owned();
    fixture.update_operation();
    execute_invocation(&fixture.request).unwrap();
    let report = fixture.report();
    assert_eq!(report["changed_frame_count"], 0);
    assert_eq!(report["processor_declared_version"], "9.8.7");
    assert_eq!(report["model_environment_evidence"], "unobserved");
    assert_eq!(report["historical_processing"], "unknown");
    assert_eq!(report["source_relationship"], "declared_not_verified");
    assert_eq!(report["frames"][1]["selected"], true);
    assert_eq!(report["frames"][1]["bytes_changed"], false);
    assert!(report.get("repair_success").is_none());
}

#[test]
fn missing_extra_corrupt_and_wrong_geometry_outputs_do_not_complete() {
    let mut wrong_geometry = png([4, 5, 6]);
    wrong_geometry[16..20].copy_from_slice(&2_u32.to_be_bytes());
    let checksum = crc32(&wrong_geometry[12..29]);
    wrong_geometry[29..33].copy_from_slice(&checksum.to_be_bytes());
    for body in [
        "exit 0".to_owned(),
        "printf 'not a PNG' > \"$2\"".to_owned(),
        "cp \"$1\" \"$2\"; cp \"$1\" \"${2}.extra\"".to_owned(),
        write_png_command(&wrong_geometry),
        "exit 9".to_owned(),
    ] {
        let mut fixture = make_fixture();
        fixture.set_processor(&body);
        assert!(execute_invocation(&fixture.request).is_err());
        fixture.assert_no_completed_report();
    }
}

#[test]
fn valid_png_structure_does_not_replace_required_decoder_evidence() {
    let mut fixture = make_fixture();
    let decoder = decoder_script(&FRAMEHASH.replace(", 3, ", ", 2, "));
    let tool = &mut fixture.operation.settings.import.ffmpeg;
    write_executable(&tool.executable, &decoder);
    tool.sha256 = sha256(decoder.as_bytes());
    fixture.update_operation();
    fixture.rewrite_import();
    assert!(execute_invocation(&fixture.request).is_err());
    fixture.assert_no_completed_report();
}

#[test]
fn import_observations_must_bind_complete_index_counts_bytes_and_history() {
    for mutation in 0..11 {
        let fixture = make_fixture();
        let mut report = fixture.import_report();
        match mutation {
            0 => report["frames"].as_array_mut().unwrap().pop().map(|_| ()).unwrap(),
            1 => report["frames"][0]["byte_count"] = json!(1),
            2 => report["frames"][0]["decoded_byte_count"] = json!(6),
            3 => report["frame_index_sha256"] = json!("d".repeat(64)),
            4 => report["source_byte_count"] = json!(0),
            5 => report["historical_processing"] = json!("known"),
            6 => report["configuration_sha256"] = json!("e".repeat(64)),
            7 => report["decoder"]["id"] = json!("unbound-decoder"),
            8 => report["decoder"]["version"] = json!("9.8.7"),
            9 => report["decoder"]["sha256"] = json!("f".repeat(64)),
            _ => report["frames"][0]["decoded_rgb24_sha256"] = json!("invalid"),
        }
        fixture.write_import_report(&report);
        assert!(execute_invocation(&fixture.request).is_err());
        assert!(!fixture.calls.exists());
        fixture.assert_no_completed_report();
    }
}

#[test]
fn a_299_frame_timeline_cannot_be_silently_reduced_to_240_files() {
    for truncate_index in [false, true] {
        let mut fixture = make_fixture_with_count(299);
        assert_eq!(fixture.index.frames.len(), 299);
        for frame in fixture.index.frames.iter().skip(240) {
            fs::remove_file(fixture.input("frames").join(&frame.filename)).unwrap();
        }
        if truncate_index {
            // Even a mutually consistent 240-file import/index must not erase
            // 59 declared source ordinals or retime the original 299-frame run.
            fixture.index.frames.truncate(240);
            fs::write(fixture.input("index"), serde_json::to_vec(&fixture.index).unwrap()).unwrap();
            fixture.rewrite_import();
        }
        assert!(execute_invocation(&fixture.request).is_err());
        assert!(!fixture.calls.exists());
        fixture.assert_no_completed_report();
    }
}

#[test]
fn complete_membership_and_manual_range_errors_fail_before_processor_launch() {
    for extra in [false, true] {
        let fixture = make_fixture();
        if extra { fs::write(fixture.input("frames").join("extra.png"), png([1, 1, 1])).unwrap(); }
        else { fs::remove_file(fixture.input("frames").join(&fixture.index.frames[0].filename)).unwrap(); }
        assert!(execute_invocation(&fixture.request).is_err());
        assert!(!fixture.calls.exists());
    }
    for ranges in [vec![], vec![InclusiveFrameRange { first:1, last:4 }],
        vec![InclusiveFrameRange { first:1, last:2 }, InclusiveFrameRange { first:2, last:3 }]] {
        let mut fixture = make_fixture();
        fixture.operation.ranges = ranges;
        if let Ok(configuration) = fixture.operation.provider_configuration() {
            fixture.request.configuration = configuration;
            assert!(execute_invocation(&fixture.request).is_err());
        }
        assert!(!fixture.calls.exists());
        fixture.assert_no_completed_report();
    }
}

#[test]
fn changed_tool_or_processor_mutation_of_any_bound_input_prevents_completion() {
    let fixture = make_fixture();
    fs::write(&fixture.operation.settings.processor.tool.executable, "#!/bin/sh\nexit 0\n").unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.calls.exists());
    for mutation in ["snapshot", "frames", "index", "import"] {
        let mut fixture = make_fixture();
        let target = match mutation {
            "snapshot" => "\"$1\"".to_owned(),
            "frames" => shell_literal(&fixture.input("frames").join(&fixture.index.frames[0].filename)),
            "index" => shell_literal(fixture.input("index")),
            _ => shell_literal(fixture.input("import")),
        };
        fixture.set_processor(&format!("cp \"$1\" \"$2\"\nprintf 'changed' > {target}"));
        assert!(execute_invocation(&fixture.request).is_err());
        fixture.assert_no_completed_report();
    }
}

#[test]
fn symlinked_inputs_and_outputs_and_preexisting_destinations_are_refused() {
    let fixture = make_fixture();
    let source = fixture.input("frames").join(&fixture.index.frames[0].filename);
    let retained = fixture.root.join("retained original.png");
    fs::rename(&source, &retained).unwrap();
    symlink(&retained, &source).unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert!(!fixture.calls.exists());

    let mut fixture = make_fixture();
    fixture.set_processor("ln -s \"$1\" \"$2\"");
    assert!(execute_invocation(&fixture.request).is_err());
    fixture.assert_no_completed_report();

    let fixture = make_fixture();
    execute_invocation(&fixture.request).unwrap();
    let report_before = fs::read(fixture.output("evidence")).unwrap();
    assert!(execute_invocation(&fixture.request).is_err());
    assert_eq!(fs::read(fixture.output("evidence")).unwrap(), report_before);
    assert_eq!(fs::read_to_string(&fixture.calls).unwrap().lines().count(), 1);
}

#[test]
fn library_plan_keeps_import_and_repair_ordered_without_launching_tools() {
    let fixture = make_fixture();
    let request = fixture.library_request();
    let cancellation = aniflow::CancellationToken::default();
    let plan = aniflow::frame_repair::plan(&request, &cancellation).unwrap();
    assert_eq!(plan.payload.inputs.len(), 3);
    assert_eq!(plan.payload.stages.len(), 2);
    assert_eq!(plan.payload.stages[0].id, "import_frames");
    assert_eq!(plan.payload.stages[1].id, "repair_frames");
    assert_eq!(plan.payload.stages[1].depends_on, ["import_frames"]);
    assert!(!fixture.calls.exists());
    assert!(!fixture.output("frames").exists());
    cancellation.cancel();
    assert!(aniflow::frame_repair::plan(&request, &cancellation).is_err());
}

#[test]
fn output_parent_admission_never_creates_a_run_inside_original_frames() {
    let fixture = make_fixture();
    let nested = fixture.input("frames").join("forbidden new runs");
    let mut progressed = false;
    assert!(aniflow::frame_repair::run(
        fixture.library_request(), Some(nested.clone()), &aniflow::CancellationToken::default(),
        |_| progressed = true,
    ).is_err());
    assert!(!nested.exists());
    assert!(!progressed);
    assert_eq!(fs::read_dir(fixture.input("frames")).unwrap().count(), 3);
}
