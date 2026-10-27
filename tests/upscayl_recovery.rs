//! Synthetic recovery regressions authored for deferred #64 execution.
//!
//! Substitute executables exercise argument, artifact and checkpoint contracts.
//! Their framehash output is fabricated fixture evidence, not PNG decompression,
//! native Upscayl/NCNN support, GPU safety or visual-quality qualification.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use aniflow::temporal::{self, StreamSelection};
use aniflow::upscayl_recovery::{
    self, UpscaylCollectOperation, UpscaylFrameOperation, UpscaylFrameSpec, UpscaylRecoveryConfiguration,
    UpscaylRecoveryRequest, execute_frame_invocation,
};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::{
    ArtifactKind, ArtifactRole, CancellationToken, ComponentInventory, HostResources,
    PipelineInputBinding, PipelinePlanningContext, PipelineV3Configuration, PipelineV3Plan,
    PipelineV3ProgressState, PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunRequest,
    PipelineV3ResumeRequest, ProviderInvocationArtifactBinding, ProviderInvocationRequest,
    ProviderRegistration, ProviderRegistry, SideEffect, StreamRole, resolve_pipeline_v3,
    resume_v3_with_progress_and_cancellation, run_v3_with_progress_and_cancellation,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

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
        for _ in 0..8 { crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1)); }
    }
    !crc
}

fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(bytes);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(bytes);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// Small complete RGB PNG with a single stored DEFLATE block; no native tool.
fn png(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
    let mut output = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut output, b"IHDR", &header);
    let mut scanlines = Vec::new();
    for _ in 0..height {
        scanlines.push(0);
        for _ in 0..width { scanlines.extend_from_slice(&color); }
    }
    let length = u16::try_from(scanlines.len()).unwrap();
    let mut compressed = vec![0x78, 0x01, 0x01];
    compressed.extend_from_slice(&length.to_le_bytes());
    compressed.extend_from_slice(&(!length).to_le_bytes());
    compressed.extend_from_slice(&scanlines);
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in scanlines {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    compressed.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut output, b"IDAT", &compressed);
    chunk(&mut output, b"IEND", &[]);
    output
}

fn write_executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

// Shell interpolation is confined to substitute executable fixtures. Production
// supplies a direct argv and never constructs a shell command.
fn shell_literal(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

fn write_png_command(bytes: &[u8]) -> String {
    let octal = bytes.iter().map(|byte| format!("\\{byte:03o}")).collect::<String>();
    format!("printf '{octal}' > \"$output\"")
}

const DECODER: &str = r##"#!/bin/sh
if [ "$1" = "-version" ]; then printf "ffmpeg version 1.0.0\n"; exit 0; fi
input=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-i" ]; then input="$2"; break; fi
  shift
done
[ -n "$input" ] || exit 70
set -- $(/usr/bin/od -An -tu1 -j16 -N8 "$input")
[ "$#" -eq 8 ] || exit 71
width=$(( $1 * 16777216 + $2 * 65536 + $3 * 256 + $4 ))
height=$(( $5 * 16777216 + $6 * 65536 + $7 * 256 + $8 ))
bytes=$(( width * height * 3 ))
printf "#format: frame checksums\n#version: 2\n#hash: SHA256\n#software: synthetic fixture\n#tb 0: 1/25\n#media_type 0: video\n#codec_id 0: rawvideo\n#dimensions 0: %sx%s\n#sar 0: 0/1\n#stream#, dts, pts, duration, size, hash\n0, 0, 0, 1, %s, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n" "$width" "$height" "$bytes"
"##;

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    frames: PathBuf,
    index_path: PathBuf,
    index: RifeFrameIndex,
    calls: PathBuf,
    settings: UpscaylRecoveryConfiguration,
}

impl Fixture {
    fn request(&self) -> UpscaylRecoveryRequest {
        UpscaylRecoveryRequest::new(&self.source, &self.frames, &self.index_path,
            self.settings.clone(), PathBuf::from(env!("CARGO_BIN_EXE_aniflow")))
    }

    fn set_processor(&mut self, body: &str) {
        let script = format!(r##"#!/bin/sh
[ "$#" -eq 20 ] || exit 80
input=""; output=""; model_directory=""; model=""; native=""; scale=""; tile=""; gpu=""; threads=""; format=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -i) input="$2";; -o) output="$2";; -m) model_directory="$2";; -n) model="$2";;
    -z) native="$2";; -s) scale="$2";; -t) tile="$2";; -g) gpu="$2";;
    -j) threads="$2";; -f) format="$2";; *) exit 81;;
  esac
  shift 2
done
[ "$input" = "input.png" ] && [ "$output" = "output.png" ] || exit 82
[ "$model_directory" = "models" ] && [ "$model" = "fixture_model" ] || exit 83
[ "$native" = "4" ] && [ "$scale" = "2" ] && [ "$tile" = "32" ] || exit 84
[ "$gpu" = "0" ] && [ "$threads" = "1:1:1" ] && [ "$format" = "png" ] || exit 85
[ -f "$model_directory/$model.param" ] && [ -f "$model_directory/$model.bin" ] || exit 86
printf "%s\n" "$native:$scale:$tile:$gpu:$threads" >> {calls}
{body}
"##, calls=shell_literal(&self.calls));
        write_executable(&self.settings.upscayl.executable, &script);
        self.settings.upscayl.sha256 = sha256(script.as_bytes());
    }

    fn launch_count(&self) -> usize {
        fs::read_to_string(&self.calls).map(|text| text.lines().count()).unwrap_or(0)
    }

    fn frame_operation(&self, ordinal: usize) -> UpscaylFrameOperation {
        let frame = &self.index.frames[ordinal];
        let timeline = self.index.temporal_inspection.video_timeline().unwrap();
        UpscaylFrameOperation {
            schema:"aniflow.upscayl-frame-operation/v1".to_owned(),
            settings:self.settings.clone(),
            frame:UpscaylFrameSpec {
                source_sha256:self.index.temporal_inspection.source_sha256.clone(),
                source_byte_count:self.index.temporal_inspection.source_size_bytes,
                frame_index_sha256:canonical_sha256(&self.index), ordinal:frame.ordinal,
                filename:frame.filename.clone(), source_time:timeline.frames[ordinal].source_time,
                input_sha256:frame.sha256.clone(),
                input_byte_count:fs::metadata(self.frames.join(&frame.filename)).unwrap().len(),
                input_width:1, input_height:1,
            },
        }
    }

    fn invocation(&self, ordinal: usize) -> ProviderInvocationRequest {
        let parent = self.root.join(format!("native outputs {ordinal}"));
        fs::create_dir(&parent).unwrap();
        ProviderInvocationRequest::new(
            format!("upscale_frame_{ordinal:06}"), "c".repeat(64),
            self.frame_operation(ordinal).provider_configuration().unwrap(),
            vec![ProviderInvocationArtifactBinding::new(
                "frame", format!("source_frame_{ordinal:06}"), "image/png", ArtifactRole::Intermediate,
                Some(StreamRole::Video), ArtifactKind::File,
                self.frames.join(&self.index.frames[ordinal].filename),
            ).unwrap()],
            vec![
                ProviderInvocationArtifactBinding::new("frame", format!("frame_{ordinal:06}"), "image/png",
                    ArtifactRole::Intermediate, Some(StreamRole::Video), ArtifactKind::File, parent.join("frame.png")).unwrap(),
                ProviderInvocationArtifactBinding::new("evidence", format!("evidence_{ordinal:06}"),
                    "application/vnd.aniflow.upscayl-frame-report+json", ArtifactRole::ValidationEvidence,
                    Some(StreamRole::TimedMetadata), ArtifactKind::File, parent.join("frame.json")).unwrap(),
            ],
        ).unwrap()
    }

    fn run(&self) -> PipelineV3RunOutcome {
        upscayl_recovery::run(self.request(), Some(self.root.join("runs")),
            &CancellationToken::default(), |_| {}).unwrap()
    }
}

fn make_fixture(frame_count: usize) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("recovery 音 with spaces");
    fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    let source_bytes = b"synthetic declared source identity, not native media";
    fs::write(&source, source_bytes).unwrap();
    let frames_directory = root.join("source frames");
    fs::create_dir(&frames_directory).unwrap();
    let frames = (0..frame_count).map(|ordinal| {
        let filename = format!("frame_{ordinal:08}.png");
        let bytes = png(1, 1, [(ordinal % 255) as u8 + 1, 2, 3]);
        fs::write(frames_directory.join(&filename), &bytes).unwrap();
        RifeFrame { ordinal:ordinal as u64, filename, sha256:sha256(&bytes) }
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
        schema:RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection:inspection,
        width:1, height:1, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(),
        frames, cut_before_ordinals:if frame_count > 2 {vec![2]} else {Vec::new()},
    };
    let index_path = root.join("source index.json");
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let decoder = root.join("synthetic ffmpeg");
    write_executable(&decoder, DECODER);
    let processor = root.join("synthetic upscayl");
    write_executable(&processor, "#!/bin/sh\nexit 99\n");
    let param = root.join("original model.param");
    let weights = root.join("original model.bin");
    fs::write(&param, b"synthetic model parameter bytes").unwrap();
    fs::write(&weights, b"synthetic model weight bytes").unwrap();
    let settings = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.upscayl-recovery-configuration/v1",
        "import":{
            "schema":"aniflow.frame-import-configuration/v1",
            "ffmpeg":{"executable":decoder,"version":"1.0.0","sha256":sha256(DECODER.as_bytes())},
            "tool_timeout_milliseconds":2000,"maximum_source_bytes":1048576,
            "maximum_frame_bytes":1024,"maximum_total_frame_bytes":1048576,"maximum_pixels":1
        },
        "upscayl":{"executable":processor,"version":"1.0.0","sha256":sha256(b"#!/bin/sh\nexit 99\n")},
        "model":{"name":"fixture_model","native_scale":4,
            "param":{"path":param,"sha256":sha256(b"synthetic model parameter bytes")},
            "weights":{"path":weights,"sha256":sha256(b"synthetic model weight bytes")}},
        "output_scale":2,"maximum_output_frame_bytes":1024,"maximum_output_pixels":16,
        "minimum_free_bytes":1048576
    })).unwrap()).unwrap();
    let mut fixture = Fixture { _directory:directory, root:root.clone(), source,
        frames:frames_directory, index_path, index, calls:root.join("processor calls.txt"), settings };
    fixture.set_processor(&write_png_command(&png(2, 2, [99, 2, 3])));
    fixture
}

fn output_path<'a>(invocation: &'a ProviderInvocationRequest, port: &str) -> &'a Path {
    &invocation.outputs.iter().find(|output| output.port == port).unwrap().path
}

fn stage_frame_path(run: &Path, plan: &PipelineV3Plan, ordinal: usize) -> PathBuf {
    let stage = &plan.payload.stages[ordinal];
    let frame = stage.outputs.iter().find(|binding| binding.port == "frame").unwrap();
    run.join(&frame.artifacts[0].relative_path)
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    WalkDir::new(root).min_depth(1).into_iter().map(|entry| {
        let entry = entry.unwrap();
        assert!(!entry.file_type().is_symlink());
        let relative = entry.path().strip_prefix(root).unwrap().to_path_buf();
        let bytes = entry.file_type().is_file().then(|| fs::read(entry.path()).unwrap());
        (relative, bytes)
    }).collect()
}

fn output_document(outcome: &PipelineV3RunOutcome, schema: &str) -> Value {
    outcome.outputs.iter().filter(|output| output.path.is_file()).filter_map(|output| {
        serde_json::from_slice::<Value>(&fs::read(&output.path).unwrap()).ok()
    }).find(|document| document["schema"] == schema).expect("typed public output must exist")
}

#[test]
fn native_scale_output_scale_and_model_filename_rules_are_explicit() {
    let fixture = make_fixture(3);
    assert_eq!(fixture.settings.output_dimensions(1, 1).unwrap(), (2, 2));
    assert_eq!(fixture.settings.model.native_scale, 4);
    assert_eq!(fixture.settings.tile_size, 32);
    assert_eq!(fixture.settings.threads, "1:1:1");
    assert_eq!(fixture.settings.model_filenames().unwrap(),
        ("fixture_model.param".to_owned(), "fixture_model.bin".to_owned()));
    let mut special = fixture.settings.clone();
    special.model.name = "realesr-animevideov3".to_owned();
    assert_eq!(special.model_filenames().unwrap(),
        ("realesr-animevideov3-x4.param".to_owned(), "realesr-animevideov3-x4.bin".to_owned()));
    let mut too_small = fixture.settings.clone();
    too_small.maximum_output_pixels = 4;
    assert!(too_small.output_dimensions(1, 1).is_err(),
        "requested 2x must not hide the native 4x intermediate geometry");
    for mutation in 0..8 {
        let mut value = serde_json::to_value(&fixture.settings).unwrap();
        match mutation {
            0 => value["model"]["native_scale"] = json!(2),
            1 => value["output_scale"] = json!(1),
            2 => value["tile_size"] = json!(0),
            3 => value["threads"] = json!("2:2:2"),
            4 => value["model"]["name"] = json!("conflicting_x2_model"),
            5 => value["model"]["param"]["sha256"] = json!("not a digest"),
            6 => value["model"]["weights"]["path"] = json!("relative.bin"),
            _ => value["unexpected"] = json!(true),
        }
        assert!(UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn planning_is_read_only_and_frame_checkpoints_are_independent() {
    let fixture = make_fixture(3);
    let before = snapshot(&fixture.root);
    let plan = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.stages.len(), 4);
    for (ordinal, stage) in plan.payload.stages[..3].iter().enumerate() {
        assert_eq!(stage.id, format!("upscale_frame_{ordinal:06}"));
        assert!(stage.depends_on.is_empty(), "one missing frame must not invalidate later good frames");
        assert_eq!(stage.inputs.len(), 1);
        assert_eq!(stage.inputs[0].artifacts.len(), 1);
    }
    assert_eq!(plan.payload.stages[3].id, "collect_upscaled_frames");
    assert_eq!(plan.payload.stages[3].depends_on.len(), 3);
    assert_eq!(fixture.launch_count(), 0);
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn admission_refuses_more_than_1024_frames_and_output_parents_inside_sources() {
    let fixture = make_fixture(1025);
    assert!(upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).is_err());
    assert_eq!(fixture.launch_count(), 0);
    let fixture = make_fixture(3);
    let nested = fixture.frames.join("forbidden runs");
    let mut progressed = false;
    assert!(upscayl_recovery::run(fixture.request(), Some(nested.clone()),
        &CancellationToken::default(), |_| progressed = true).is_err());
    assert!(!nested.exists());
    assert!(!progressed);
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn native_four_to_requested_two_uses_exact_argv_and_observes_scaled_output() {
    let fixture = make_fixture(3);
    let invocation = fixture.invocation(0);
    execute_frame_invocation(&invocation).unwrap();
    assert_eq!(fixture.launch_count(), 1);
    assert_eq!(fs::read_to_string(&fixture.calls).unwrap(), "4:2:32:0:1:1:1\n");
    let report: Value = serde_json::from_slice(&fs::read(output_path(&invocation, "evidence")).unwrap()).unwrap();
    assert_eq!(report["native_scale"], 4);
    assert_eq!(report["output_scale"], 2);
    assert_eq!(report["output_width"], 2);
    assert_eq!(report["output_height"], 2);
    assert_eq!(report["frame"], serde_json::to_value(fixture.frame_operation(0).frame).unwrap());
    assert_eq!(report["model_param_sha256"], fixture.settings.model.param.sha256);
    assert_eq!(report["model_weights_sha256"], fixture.settings.model.weights.sha256);
    assert_eq!(report["output_sha256"], sha256(&fs::read(output_path(&invocation, "frame")).unwrap()));
    assert_ne!(report["backend_qualification"], "qualified");
}

#[test]
fn zero_exit_missing_corrupt_wrong_geometry_and_extra_outputs_are_not_accepted() {
    let mut bad_crc = png(2, 2, [3, 4, 5]);
    bad_crc[29] ^= 1;
    for body in [
        "exit 0".to_owned(),
        write_png_command(&bad_crc),
        write_png_command(&png(1, 1, [3, 4, 5])),
        format!("{}\nprintf 'undeclared' > extra.png", write_png_command(&png(2, 2, [3, 4, 5]))),
        "ln -s input.png output.png".to_owned(),
    ] {
        let mut fixture = make_fixture(3);
        fixture.set_processor(&body);
        let invocation = fixture.invocation(0);
        assert!(execute_frame_invocation(&invocation).is_err());
        assert!(!output_path(&invocation, "evidence").exists());
    }
}

#[test]
fn tool_model_and_source_mutations_fail_before_processor_launch() {
    for target in 0..4 {
        let fixture = make_fixture(3);
        let invocation = fixture.invocation(0);
        let path = match target {
            0 => &fixture.settings.upscayl.executable,
            1 => &fixture.settings.model.param.path,
            2 => &fixture.settings.model.weights.path,
            _ => &invocation.inputs[0].path,
        };
        fs::write(path, b"changed after pinning").unwrap();
        assert!(execute_frame_invocation(&invocation).is_err());
        assert_eq!(fixture.launch_count(), 0);
        assert!(!output_path(&invocation, "evidence").exists());
    }
    let fixture = make_fixture(3);
    let original = fixture.frames.join(&fixture.index.frames[0].filename);
    let retained = fixture.root.join("retained source.png");
    fs::rename(&original, &retained).unwrap();
    symlink(&retained, &original).unwrap();
    assert!(upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).is_err());
}

#[test]
fn structural_png_success_does_not_replace_complete_decoded_frame_evidence() {
    let mut fixture = make_fixture(3);
    let decoder = DECODER.replace("bytes=$(( width * height * 3 ))", "bytes=1");
    write_executable(&fixture.settings.import.ffmpeg.executable, &decoder);
    fixture.settings.import.ffmpeg.sha256 = sha256(decoder.as_bytes());
    let invocation = fixture.invocation(0);
    assert!(execute_frame_invocation(&invocation).is_err());
    assert_eq!(fixture.launch_count(), 0, "input decode must complete before Upscayl starts");
    assert!(!output_path(&invocation, "evidence").exists());
}

#[test]
fn cancellation_after_good_frames_resumes_pending_frames_without_reprocessing() {
    let fixture = make_fixture(3);
    let token = CancellationToken::default();
    let mut run_directory = None;
    let mut frame_counts = Vec::new();
    let result = upscayl_recovery::run(fixture.request(), Some(fixture.root.join("runs")), &token, |progress| {
        frame_counts.push(progress.frames.clone());
        match &progress.runtime {
            PipelineV3RunProgress::Started { run_directory:started, .. } => run_directory = Some(started.clone()),
            PipelineV3RunProgress::Stage { stage_id, state:PipelineV3ProgressState::Complete }
                if stage_id == "upscale_frame_000001" => token.cancel(),
            _ => {}
        }
    });
    assert!(result.is_err());
    assert_eq!(fixture.launch_count(), 2);
    assert_eq!(frame_counts[0].estimated_remaining_milliseconds, None);
    let run_directory = run_directory.unwrap();
    let resumed = upscayl_recovery::resume(fixture.request(), &run_directory,
        &CancellationToken::default(), |progress| frame_counts.push(progress.frames.clone())).unwrap();
    assert_eq!(fixture.launch_count(), 3);
    assert!(resumed.reused_stages.contains(&"upscale_frame_000000".to_owned()));
    assert!(resumed.reused_stages.contains(&"upscale_frame_000001".to_owned()));
    assert!(resumed.executed_stages.contains(&"upscale_frame_000002".to_owned()));
    let counts = frame_counts.last().unwrap();
    assert_eq!(counts.total, 3);
    assert_eq!(counts.completed, 3);
    assert_eq!(counts.reused, 2);
    assert_eq!(counts.pending, 0);
    assert_eq!(counts.failed, 0);
    let index = output_document(&resumed, "aniflow.upscayl-output-index/v1");
    assert_eq!(index["source_index"], serde_json::to_value(&fixture.index).unwrap());
    assert_eq!(index["output_width"], 2);
    assert_eq!(index["output_height"], 2);
    assert_eq!(index["frames"].as_array().unwrap().len(), 3);
    for (ordinal, frame) in fixture.index.frames.iter().enumerate() {
        assert_eq!(index["frames"][ordinal]["filename"], frame.filename);
        assert_eq!(index["frames"][ordinal]["ordinal"], frame.ordinal);
        assert_eq!(sha256(&fs::read(fixture.frames.join(&frame.filename)).unwrap()), frame.sha256);
    }
}

#[test]
fn missing_or_corrupt_checkpoint_output_retries_only_that_frame_and_collector() {
    for remove in [false, true] {
        let fixture = make_fixture(3);
        let plan = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
        let outcome = fixture.run();
        assert_eq!(fixture.launch_count(), 3);
        let bad = stage_frame_path(&outcome.run_directory, &plan, 1);
        if remove { fs::remove_file(&bad).unwrap(); } else { fs::write(&bad, b"partial PNG").unwrap(); }
        let resumed = upscayl_recovery::resume(fixture.request(), &outcome.run_directory,
            &CancellationToken::default(), |_| {}).unwrap();
        assert_eq!(fixture.launch_count(), 4);
        assert_eq!(resumed.executed_stages, ["upscale_frame_000001", "collect_upscaled_frames"]);
        assert_eq!(resumed.reused_stages, ["upscale_frame_000000", "upscale_frame_000002"]);
    }
}

#[test]
fn changed_settings_models_binary_or_source_refuse_resume_without_mutation() {
    for change in 0..4 {
        let mut fixture = make_fixture(3);
        let outcome = fixture.run();
        let before = snapshot(&outcome.run_directory);
        match change {
            0 => fixture.settings.tile_size = 64,
            1 => {
                fs::write(&fixture.settings.model.weights.path, b"different model").unwrap();
                fixture.settings.model.weights.sha256 = sha256(b"different model");
            }
            2 => {
                let bytes = fs::read(&fixture.settings.upscayl.executable).unwrap();
                let mut updated = bytes;
                updated.extend_from_slice(b"\n# different executable identity\n");
                fs::write(&fixture.settings.upscayl.executable, &updated).unwrap();
                fixture.settings.upscayl.sha256 = sha256(&updated);
            }
            _ => fs::write(&fixture.source, b"changed original source").unwrap(),
        }
        let mut progressed = false;
        assert!(upscayl_recovery::resume(fixture.request(), &outcome.run_directory,
            &CancellationToken::default(), |_| progressed = true).is_err());
        assert!(!progressed);
        assert_eq!(fixture.launch_count(), 3);
        assert_eq!(snapshot(&outcome.run_directory), before);
    }
}

#[test]
fn generic_resume_rechecks_model_files_even_with_the_retained_registry() {
    let fixture = make_fixture(3);
    let plan = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    let outcome = fixture.run();
    let mut registry = ProviderRegistry::new();
    for (ordinal, stage) in plan.payload.stages.iter().enumerate() {
        let (manifest, configuration) = if ordinal < fixture.index.frames.len() {
            let operation = fixture.frame_operation(ordinal);
            (operation.manifest().unwrap(), operation.provider_configuration().unwrap())
        } else {
            let operation = UpscaylCollectOperation {
                schema:"aniflow.upscayl-collect-operation/v1".to_owned(),
                settings:fixture.settings.clone(), frame_index_sha256:canonical_sha256(&fixture.index),
                frame_count:fixture.index.frames.len() as u64,
            };
            (operation.manifest().unwrap(), operation.provider_configuration().unwrap())
        };
        let lock = &stage.provider_lock.payload;
        registry.register(ProviderRegistration::new(&lock.registration_id, manifest, configuration,
            PathBuf::from(env!("CARGO_BIN_EXE_aniflow")), &lock.implementation.id,
            ComponentInventory { tools:lock.tools.clone(), codecs:lock.codecs.clone(), models:lock.models.clone() },
        ).unwrap()).unwrap();
    }
    let mut bindings = vec![PipelineInputBinding::new("source_video", &fixture.source),
        PipelineInputBinding::new("source_index", &fixture.index_path)];
    bindings.extend(fixture.index.frames.iter().map(|frame| PipelineInputBinding::new(
        format!("source_frame_{:06}", frame.ordinal), fixture.frames.join(&frame.filename),
    )));
    let before = snapshot(&outcome.run_directory);
    fs::write(&fixture.settings.model.weights.path, b"model changed after registration").unwrap();
    let mut progressed = false;
    let error = resume_v3_with_progress_and_cancellation(
        PipelineV3ResumeRequest::new(&outcome.run_directory, bindings, registry)
            .with_execution_limits(fixture.request().execution_limits),
        &CancellationToken::default(), |_| progressed = true,
    ).expect_err("retained registry declarations cannot override changed model bytes");
    assert!(error.to_string().contains("model"));
    assert!(!progressed, "generic resume must recheck pins before any durable transition");
    assert_eq!(fixture.launch_count(), 3);
    assert_eq!(snapshot(&outcome.run_directory), before);
}

#[test]
fn generic_collector_refuses_loose_images_and_reports_without_frame_stage_dependencies() {
    let fixture = make_fixture(1);
    let genuine = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    let lock = &genuine.payload.stages.last().unwrap().provider_lock.payload;
    let operation = UpscaylCollectOperation {
        schema:"aniflow.upscayl-collect-operation/v1".to_owned(),
        settings:fixture.settings.clone(), frame_index_sha256:canonical_sha256(&fixture.index),
        frame_count:1,
    };
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderRegistration::new("collect_upscaled_frames", operation.manifest().unwrap(),
        operation.provider_configuration().unwrap(), PathBuf::from(env!("CARGO_BIN_EXE_aniflow")),
        &lock.implementation.id,
        ComponentInventory { tools:lock.tools.clone(), codecs:lock.codecs.clone(), models:lock.models.clone() },
    ).unwrap()).unwrap();
    let loose_png = fixture.root.join("loose output.png");
    let loose_report = fixture.root.join("loose report.json");
    fs::write(&loose_png, png(2, 2, [99, 2, 3])).unwrap();
    // Deliberately not valid provider evidence: topology admission must precede
    // report interpretation, tool execution and run-workspace creation.
    fs::write(&loose_report, b"{\"accepted\":true,\"historical_provider\":\"forged\"}").unwrap();
    let authored = json!({
        "schema":"aniflow.pipeline/v3", "name":"loose-upscayl-collector",
        "inputs":[
            {"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"},
            {"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"},
            {"id":"frame_000000","artifact_type":"image/png","artifact_role":"intermediate","stream_role":"video"},
            {"id":"evidence_000000","artifact_type":"application/vnd.aniflow.upscayl-frame-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"}
        ],
        "stages":[{
            "id":"collect_upscaled_frames", "depends_on":[],
            "capability":{"id":"aniflow/upscayl-collect","version_requirement":"=1.0.0"},
            "provider":{"primary":{"registration_id":"collect_upscaled_frames"},"fallbacks":[]},
            "inputs":[{"port":"index","artifacts":["source_index"]},
                {"port":"frames","artifacts":["frame_000000"]},
                {"port":"evidence","artifacts":["evidence_000000"]}],
            "outputs":[
                {"port":"frames","artifacts":[{"id":"upscaled_frames","relative_path":"artifacts/collection/frames","kind":"directory"}]},
                {"port":"index","artifacts":[{"id":"upscaled_index","relative_path":"artifacts/collection/index.json","kind":"file"}]},
                {"port":"evidence","artifacts":[{"id":"upscale_evidence","relative_path":"artifacts/collection/report.json","kind":"file"}]}
            ],
            "validations":[
                {"id":"frames_integrity","artifact":"upscaled_frames","contract":"aniflow.validation/artifact-integrity/v1"},
                {"id":"index_integrity","artifact":"upscaled_index","contract":"aniflow.validation/artifact-integrity/v1"},
                {"id":"report_integrity","artifact":"upscale_evidence","contract":"aniflow.validation/artifact-integrity/v1"}
            ]
        }],
        "outputs":[
            {"id":"frames","artifact":"upscaled_frames","required_validations":["frames_integrity"]},
            {"id":"index","artifact":"upscaled_index","required_validations":["index_integrity"]},
            {"id":"evidence","artifact":"upscale_evidence","required_validations":["report_integrity"]}
        ]
    });
    let bindings = vec![PipelineInputBinding::new("source_video", &fixture.source),
        PipelineInputBinding::new("source_index", &fixture.index_path),
        PipelineInputBinding::new("frame_000000", &loose_png),
        PipelineInputBinding::new("evidence_000000", &loose_report)];
    let plan = resolve_pipeline_v3(
        &PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&authored).unwrap()).unwrap(),
        &bindings, &registry, &PipelinePlanningContext {
            host:HostResources { cpu_threads:1, memory_mib:0, storage_mib:0,
                gpu_available:false, network_available:false },
            allowed_side_effects:vec![SideEffect::FilesystemRead, SideEffect::FilesystemWrite],
            offline:true,
        },
    ).unwrap();
    let before = snapshot(&fixture.root);
    let mut progressed = false;
    let error = run_v3_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, bindings, registry)
            .with_output_directory(fixture.root.join("forbidden loose-output run")),
        &CancellationToken::default(), |_| progressed = true,
    ).expect_err("external PNGs and report JSON cannot replace accepted frame-stage dependencies");
    assert!(error.to_string().contains("accepted frame-stage"));
    assert!(!progressed);
    assert_eq!(fixture.launch_count(), 0);
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn an_unowned_existing_png_or_report_does_not_become_a_reusable_checkpoint() {
    let fixture = make_fixture(3);
    let loose = fixture.root.join("manual partial outputs");
    fs::create_dir(&loose).unwrap();
    fs::write(loose.join(&fixture.index.frames[0].filename), png(2, 2, [99, 2, 3])).unwrap();
    fs::write(loose.join("frame.json"), b"{\"accepted\":true}").unwrap();
    let before = snapshot(&loose);
    let mut progressed = false;
    assert!(upscayl_recovery::resume(fixture.request(), &loose,
        &CancellationToken::default(), |_| progressed = true).is_err());
    assert!(!progressed);
    assert_eq!(fixture.launch_count(), 0);
    assert_eq!(snapshot(&loose), before);
    let outcome = fixture.run();
    assert_eq!(fixture.launch_count(), 3);
    assert!(outcome.reused_stages.is_empty());
}

#[test]
fn preexisting_assigned_outputs_are_preserved_and_never_overwritten() {
    let fixture = make_fixture(3);
    let invocation = fixture.invocation(0);
    fs::write(output_path(&invocation, "frame"), b"existing owned-path bytes").unwrap();
    assert!(execute_frame_invocation(&invocation).is_err());
    assert_eq!(fs::read(output_path(&invocation, "frame")).unwrap(), b"existing owned-path bytes");
    assert_eq!(fixture.launch_count(), 0);
    assert!(!output_path(&invocation, "evidence").exists());
}
