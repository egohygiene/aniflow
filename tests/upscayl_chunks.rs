//! Synthetic bounded-chunk regressions authored for deferred #64 execution.
//!
//! Substitute scripts assert the native directory-batch interface. Fabricated
//! framehash observations are not native PNG decoding, NCNN/GPU qualification or
//! evidence that real media has been processed successfully.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use aniflow::temporal::{self, StreamSelection};
use aniflow::upscayl_recovery::{self, UpscaylChunkCollectOperation, UpscaylRecoveryConfiguration, UpscaylRecoveryRequest};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::{CancellationToken, ComponentInventory, HostResources, PipelineInputBinding,
    PipelinePlanningContext, PipelineV3Configuration, PipelineV3ProgressState,
    PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunRequest, ProviderRegistration,
    ProviderRegistry, SideEffect, resolve_pipeline_v3, run_v3_with_progress_and_cancellation};
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

    fn set_policy(&mut self, frames: u32, timeout: u64) {
        let mut value = serde_json::to_value(&self.settings).unwrap();
        value["execution"] = json!({"mode":"bounded_chunk","frames_per_chunk":frames,
            "timeout_milliseconds":timeout});
        self.settings = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    }

    fn set_processor(&mut self, body: &str) {
        let script = format!(r##"#!/bin/sh
[ "$#" -eq 20 ] || exit 80
input=""; output_directory=""; model_directory=""; model=""; native=""; scale=""; tile=""; gpu=""; threads=""; format=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -i) input="$2";; -o) output_directory="$2";; -m) model_directory="$2";; -n) model="$2";;
    -z) native="$2";; -s) scale="$2";; -t) tile="$2";; -g) gpu="$2";;
    -j) threads="$2";; -f) format="$2";; *) exit 81;;
  esac
  shift 2
done
[ "$input" = "input" ] && [ "$output_directory" = "output" ] || exit 82
[ -d "$input" ] && [ -d "$output_directory" ] || exit 87
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

    fn run(&self) -> PipelineV3RunOutcome {
        upscayl_recovery::run(self.request(), Some(self.root.join("runs")),
            &CancellationToken::default(), |_| {}).unwrap()
    }
}

fn good_batch() -> String {
    format!("for input_frame in \"$input\"/*.png; do\n  output=\"$output_directory/${{input_frame##*/}}\"\n  {}\ndone", write_png_command(&png(2, 2, [99, 2, 3])))
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
            "maximum_frame_bytes":1024,"maximum_total_frame_bytes":16777216,"maximum_pixels":1
        },
        "upscayl":{"executable":processor,"version":"1.0.0","sha256":sha256(b"#!/bin/sh\nexit 99\n")},
        "model":{"name":"fixture_model","native_scale":4,
            "param":{"path":param,"sha256":sha256(b"synthetic model parameter bytes")},
            "weights":{"path":weights,"sha256":sha256(b"synthetic model weight bytes")}},
        "execution":{"mode":"bounded_chunk","frames_per_chunk":2,"timeout_milliseconds":2000},
        "output_scale":2,"maximum_output_frame_bytes":1024,"maximum_output_pixels":16,
        "minimum_free_bytes":1048576
    })).unwrap()).unwrap();
    let mut fixture = Fixture { _directory:directory, root:root.clone(), source,
        frames:frames_directory, index_path, index, calls:root.join("processor calls.txt"), settings };
    fixture.set_processor(&good_batch());
    fixture
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
fn policy_is_explicit_bounded_and_default_per_frame_keeps_its_serialization() {
    let fixture = make_fixture(3);
    for execution in [
        json!({"mode":"bounded_chunk","frames_per_chunk":1}),
        json!({"mode":"bounded_chunk","frames_per_chunk":257}),
        json!({"mode":"bounded_chunk","frames_per_chunk":2,"timeout_milliseconds":0}),
        json!({"mode":"bounded_chunk","frames_per_chunk":2,"timeout_milliseconds":21600001_u64}),
        json!({"mode":"bounded_chunk","frames_per_chunk":2,"maximum_tool_output_bytes":1023}),
        json!({"mode":"bounded_chunk","frames_per_chunk":2,"maximum_tool_output_bytes":67108865_u64}),
        json!({"mode":"bounded_chunk","frames_per_chunk":2,"unexpected":true}),
        json!({"mode":"native_batch"}),
    ] {
        let mut value = serde_json::to_value(&fixture.settings).unwrap();
        value["execution"] = execution;
        assert!(UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut omitted = serde_json::to_value(&fixture.settings).unwrap();
    omitted.as_object_mut().unwrap().remove("execution");
    let legacy = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&omitted).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&legacy).unwrap(), omitted,
        "default execution must not silently rewrite the existing effective settings");
    let mut explicit = omitted.clone();
    explicit["execution"] = json!({"mode":"per_frame"});
    let explicit = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&explicit).unwrap()).unwrap();
    assert_eq!(legacy, explicit);
    let mut default_timeout = omitted;
    default_timeout["execution"] = json!({"mode":"bounded_chunk","frames_per_chunk":64});
    let settings = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&default_timeout).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&settings).unwrap()["execution"]["timeout_milliseconds"], 1800000);
    assert_eq!(serde_json::to_value(&settings).unwrap()["execution"]["maximum_tool_output_bytes"], 16777216);
    assert_eq!(settings.import, fixture.settings.import,
        "native batch capture and timeout defaults must not rewrite decoder configuration");
    assert_eq!(settings.import.tool_timeout_milliseconds, 2000,
        "native batch timeout must not widen the decoder timeout");
}

#[test]
fn full_video_plan_uses_bounded_independent_chunks_and_short_final_chunk() {
    let mut fixture = make_fixture(5602);
    fixture.set_policy(64, 1800000);
    let before = snapshot(&fixture.root);
    let plan = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.stages.len(), 89);
    for (ordinal, stage) in plan.payload.stages[..88].iter().enumerate() {
        assert_eq!(stage.id, format!("upscale_chunk_{ordinal:06}"));
        assert!(stage.depends_on.is_empty(), "chunks must be independent recovery units");
        let inputs = stage.inputs.iter().find(|binding| binding.port == "frames").unwrap();
        assert_eq!(inputs.artifacts.len(), if ordinal == 87 {34} else {64});
        let outputs = stage.outputs.iter().find(|binding| binding.port == "frames").unwrap();
        assert_eq!(outputs.artifacts.len(), 1, "a chunk publishes one immutable directory");
    }
    let collector = &plan.payload.stages[88];
    assert_eq!(collector.id, "collect_upscaled_chunks");
    assert_eq!(collector.depends_on.len(), 88);
    assert_eq!(fixture.launch_count(), 0);
    assert_eq!(snapshot(&fixture.root), before, "planning creates no workspace or native output");
    assert!(fixture.request().execution_limits.maximum_artifact_files >= 5604,
        "the default collector budget must permit the declared complete video");
}

#[test]
fn chunk_stage_cap_and_legacy_per_frame_cap_are_separate() {
    let mut fixture = make_fixture(2049);
    assert!(upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).is_err(),
        "2-frame chunks would need 1025 stages");
    fixture.set_policy(64, 2000);
    assert_eq!(upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap().payload.stages.len(), 34);
    let mut value = serde_json::to_value(&fixture.settings).unwrap();
    value.as_object_mut().unwrap().remove("execution");
    fixture.settings = UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).is_err(),
        "the chunk profile does not quietly raise the per-frame stage cap");
    assert_eq!(fixture.launch_count(), 0);
}

#[test]
fn batches_launch_once_per_chunk_and_preserve_complete_mapping_and_sources() {
    let fixture = make_fixture(5);
    let source_before = snapshot(&fixture.frames);
    let mut counts = Vec::new();
    let outcome = upscayl_recovery::run(fixture.request(), Some(fixture.root.join("runs")),
        &CancellationToken::default(), |event| counts.push(event.frames.clone())).unwrap();
    assert_eq!(fixture.launch_count(), 3);
    assert!(fs::read_to_string(&fixture.calls).unwrap().lines().all(|line| line == "4:2:32:0:1:1:1"));
    assert_eq!(snapshot(&fixture.frames), source_before);
    let index = output_document(&outcome, "aniflow.upscayl-output-index/v1");
    assert_eq!(index["source_index"], serde_json::to_value(&fixture.index).unwrap());
    assert_eq!(index["output_width"], 2);
    assert_eq!(index["output_height"], 2);
    assert_eq!(index["frames"].as_array().unwrap().len(), 5);
    let report = output_document(&outcome, "aniflow.upscayl-chunk-recovery-report/v1");
    let chunks = report["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0]["frames"].as_array().unwrap().len(), 2);
    assert_eq!(chunks[2]["frames"].as_array().unwrap().len(), 1);
    assert_eq!(chunks[2]["frames"][0]["frame"]["ordinal"], 4);
    assert_eq!(chunks[0]["native_scale"], 4);
    assert_eq!(chunks[0]["output_scale"], 2);
    assert_eq!(chunks[0]["model_param_sha256"], fixture.settings.model.param.sha256);
    assert_eq!(chunks[0]["model_weights_sha256"], fixture.settings.model.weights.sha256);
    assert_eq!(report["backend_qualification"], "unqualified");
    assert_eq!(counts.first().unwrap().estimated_remaining_milliseconds, None);
    let final_counts = counts.last().unwrap();
    assert_eq!((final_counts.total, final_counts.completed, final_counts.reused, final_counts.pending), (5, 5, 0, 0));
    assert!(counts.iter().any(|count| count.completed == 2));
    assert!(counts.iter().any(|count| count.completed == 4));
}

#[test]
fn cancellation_between_chunks_resumes_only_pending_chunks_with_weighted_progress() {
    let fixture = make_fixture(5);
    let token = CancellationToken::default();
    let mut run_directory = None;
    let result = upscayl_recovery::run(fixture.request(), Some(fixture.root.join("runs")), &token, |event| {
        match &event.runtime {
            PipelineV3RunProgress::Started {run_directory:started, ..} => run_directory = Some(started.clone()),
            PipelineV3RunProgress::Stage {stage_id, state:PipelineV3ProgressState::Complete}
                if stage_id == "upscale_chunk_000001" => token.cancel(),
            _ => {}
        }
    });
    assert!(result.is_err());
    assert_eq!(fixture.launch_count(), 2);
    let mut counts = Vec::new();
    let outcome = upscayl_recovery::resume(fixture.request(), run_directory.unwrap(),
        &CancellationToken::default(), |event| counts.push(event.frames.clone())).unwrap();
    assert_eq!(fixture.launch_count(), 3);
    assert_eq!(outcome.reused_stages, ["upscale_chunk_000000", "upscale_chunk_000001"]);
    assert_eq!(outcome.executed_stages, ["upscale_chunk_000002", "collect_upscaled_chunks"]);
    let count = counts.last().unwrap();
    assert_eq!((count.completed, count.reused, count.pending, count.failed), (5, 4, 0, 0));
    assert!(counts.iter().filter(|count| count.reused > 0 && count.completed == count.reused)
        .all(|count| count.estimated_remaining_milliseconds.is_none()), "reused chunks are not throughput samples");
}

#[test]
fn a_partial_failed_chunk_is_retried_whole_while_accepted_chunks_are_reused() {
    let mut fixture = make_fixture(5);
    let failure_flag = fixture.root.join("inject partial failure");
    fs::write(&failure_flag, b"enabled").unwrap();
    let body = format!(r#"
if [ -f {flag} ] && [ -f "$input/000002.png" ]; then
  output="$output_directory/000002.png"
  {one}
  exit 9
fi
{all}
"#, flag=shell_literal(&failure_flag), one=write_png_command(&png(2, 2, [99, 2, 3])), all=good_batch());
    fixture.set_processor(&body);
    let mut run_directory = None;
    let mut counts = Vec::new();
    let result = upscayl_recovery::run(fixture.request(), Some(fixture.root.join("runs")),
        &CancellationToken::default(), |event| {
            counts.push(event.frames.clone());
            if let PipelineV3RunProgress::Started {run_directory:started, ..} = &event.runtime {
                run_directory = Some(started.clone());
            }
        });
    assert!(result.is_err());
    assert_eq!(fixture.launch_count(), 2);
    let count = counts.last().unwrap();
    assert_eq!((count.completed, count.pending, count.failed), (2, 3, 2),
        "an unaccepted partial chunk contributes no completed frames");
    fs::remove_file(failure_flag).unwrap();
    let outcome = upscayl_recovery::resume(fixture.request(), run_directory.unwrap(),
        &CancellationToken::default(), |_| {}).unwrap();
    assert_eq!(fixture.launch_count(), 4);
    assert_eq!(outcome.reused_stages, ["upscale_chunk_000000"]);
    assert_eq!(outcome.executed_stages, ["upscale_chunk_000001", "upscale_chunk_000002", "collect_upscaled_chunks"]);
}

#[test]
fn missing_or_corrupt_member_invalidates_its_whole_chunk_and_collector_only() {
    for remove in [false, true] {
        let fixture = make_fixture(5);
        let plan = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
        let outcome = fixture.run();
        let binding = plan.payload.stages[1].outputs.iter().find(|binding| binding.port == "frames").unwrap();
        let directory = outcome.run_directory.join(&binding.artifacts[0].relative_path);
        let member = fs::read_dir(directory).unwrap().next().unwrap().unwrap().path();
        if remove { fs::remove_file(member).unwrap(); } else { fs::write(member, b"partial PNG").unwrap(); }
        let resumed = upscayl_recovery::resume(fixture.request(), &outcome.run_directory,
            &CancellationToken::default(), |_| {}).unwrap();
        assert_eq!(fixture.launch_count(), 4);
        assert_eq!(resumed.reused_stages, ["upscale_chunk_000000", "upscale_chunk_000002"]);
        assert_eq!(resumed.executed_stages, ["upscale_chunk_000001", "collect_upscaled_chunks"]);
    }
}

#[test]
fn zero_exit_missing_extra_corrupt_or_wrong_size_batch_members_never_complete() {
    let mut bad_crc = png(2, 2, [99, 2, 3]);
    bad_crc[29] ^= 1;
    for body in [
        "exit 0".to_owned(),
        format!("{}\noutput=\"$output_directory/unexpected.png\"\n{}", good_batch(), write_png_command(&png(2, 2, [99, 2, 3]))),
        format!("output=\"$output_directory/000000.png\"\n{}", write_png_command(&png(2, 2, [99, 2, 3]))),
        format!("for input_frame in \"$input\"/*.png; do output=\"$output_directory/${{input_frame##*/}}\"; {}; done", write_png_command(&bad_crc)),
        format!("for input_frame in \"$input\"/*.png; do output=\"$output_directory/${{input_frame##*/}}\"; {}; done", write_png_command(&png(1, 1, [99, 2, 3]))),
    ] {
        let mut fixture = make_fixture(2);
        fixture.set_processor(&body);
        let mut completed = Vec::new();
        let result = upscayl_recovery::run(fixture.request(), Some(fixture.root.join("runs")),
            &CancellationToken::default(), |event| {
                if let PipelineV3RunProgress::Stage {stage_id, state:PipelineV3ProgressState::Complete} = &event.runtime {
                    completed.push(stage_id.clone());
                }
            });
        assert!(result.is_err());
        assert!(completed.is_empty());
        assert_eq!(fixture.launch_count(), 1);
    }
}

#[test]
fn changing_chunk_size_or_native_timeout_requires_new_run_before_mutation() {
    for timeout_change in [false, true] {
        let mut fixture = make_fixture(5);
        let outcome = fixture.run();
        let before = snapshot(&outcome.run_directory);
        if timeout_change { fixture.set_policy(2, 4000); } else { fixture.set_policy(3, 2000); }
        let mut progressed = false;
        assert!(upscayl_recovery::resume(fixture.request(), &outcome.run_directory,
            &CancellationToken::default(), |_| progressed = true).is_err());
        assert!(!progressed);
        assert_eq!(fixture.launch_count(), 3);
        assert_eq!(snapshot(&outcome.run_directory), before);
    }
}

#[test]
fn generic_chunk_collector_refuses_loose_directories_without_accepted_chunk_producers() {
    let fixture = make_fixture(1);
    let genuine = upscayl_recovery::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    let lock = &genuine.payload.stages.last().unwrap().provider_lock.payload;
    let operation = UpscaylChunkCollectOperation {
        schema:"aniflow.upscayl-chunk-collect-operation/v1".to_owned(),
        settings:fixture.settings.clone(), frame_index_sha256:canonical_sha256(&fixture.index),
        frame_count:1,
    };
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderRegistration::new("collect_upscaled_chunks", operation.manifest().unwrap(),
        operation.provider_configuration().unwrap(), PathBuf::from(env!("CARGO_BIN_EXE_aniflow")),
        &lock.implementation.id,
        ComponentInventory { tools:lock.tools.clone(), codecs:lock.codecs.clone(), models:lock.models.clone() },
    ).unwrap()).unwrap();
    let loose_png = fixture.root.join("loose output frames");
    fs::create_dir(&loose_png).unwrap();
    let loose_report = fixture.root.join("loose report.json");
    fs::write(loose_png.join(&fixture.index.frames[0].filename), png(2, 2, [99, 2, 3])).unwrap();
    // Deliberately not valid provider evidence: topology admission must precede
    // report interpretation, tool execution and run-workspace creation.
    fs::write(&loose_report, b"{\"accepted\":true,\"historical_provider\":\"forged\"}").unwrap();
    let authored = json!({
        "schema":"aniflow.pipeline/v3", "name":"loose-upscayl-collector",
        "inputs":[
            {"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"},
            {"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"},
            {"id":"chunk_frames_000000","artifact_type":"application/vnd.aniflow.upscayl-chunk-frames","artifact_role":"intermediate","stream_role":"video"},
            {"id":"chunk_evidence_000000","artifact_type":"application/vnd.aniflow.upscayl-chunk-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"}
        ],
        "stages":[{
            "id":"collect_upscaled_chunks", "depends_on":[],
            "capability":{"id":"aniflow/upscayl-chunk-collect","version_requirement":"=1.0.0"},
            "provider":{"primary":{"registration_id":"collect_upscaled_chunks"},"fallbacks":[]},
            "inputs":[{"port":"index","artifacts":["source_index"]},
                {"port":"frames","artifacts":["chunk_frames_000000"]},
                {"port":"evidence","artifacts":["chunk_evidence_000000"]}],
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
        PipelineInputBinding::new("chunk_frames_000000", &loose_png),
        PipelineInputBinding::new("chunk_evidence_000000", &loose_report)];
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
    ).expect_err("external directories and reports cannot replace accepted chunk-stage dependencies");
    assert!(error.to_string().contains("chunk"));
    assert!(!progressed);
    assert_eq!(fixture.launch_count(), 0);
    assert_eq!(snapshot(&fixture.root), before);
}

