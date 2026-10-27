//! Synthetic named-repair regressions authored for deferred #64 execution.
//!
//! Substitute executables exercise argv, identity, ownership and checkpoints.
//! Fabricated framehash text is not native PNG decode, allenk support, visual
//! quality, embedded-asset provenance or host/runtime qualification evidence.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use aniflow::frame_repair::allenk::{
    self, AllenkRepairConfiguration, AllenkRepairOperation, AllenkRepairRequest,
};
use aniflow::frame_selection::InclusiveFrameRange;
use aniflow::temporal::{self, StreamSelection};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use aniflow::{
    CancellationToken, ComponentInventory, HostResources, PipelineInputBinding,
    PipelinePlanningContext, PipelineV3Configuration, PipelineV3Plan, PipelineV3ProgressState,
    PipelineV3ResumeRequest, PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunRequest,
    ProviderRegistration, ProviderRegistry, SideEffect, resolve_pipeline_v3,
    resume_v3_with_progress_and_cancellation, run_v3_with_progress_and_cancellation,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 { crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1)); }
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

const DECODER: &str = r##"#!/bin/sh
if [ "$1" = "-version" ]; then printf "ffmpeg version 1.0.0\n"; exit 0; fi
cat <<"FRAMEHASH"
#format: frame checksums
#version: 2
#hash: SHA256
#software: synthetic fixture
#tb 0: 1/25
#media_type 0: video
#codec_id 0: rawvideo
#dimensions 0: 1x1
#sar 0: 0/1
#stream#, dts, pts, duration, size, hash
0, 0, 0, 1, 3, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
FRAMEHASH
"##;

// Shell interpolation is restricted to synthetic tools. Production uses argv.
fn shell_literal(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

fn write_executable(path: &Path, script: &str) {
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn replacement_command() -> String {
    let octal = png([99, 2, 3]).iter().map(|byte| format!("\\{byte:03o}")).collect::<String>();
    format!("printf '{octal}' > \"$output\"")
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    frames: PathBuf,
    index_path: PathBuf,
    index: RifeFrameIndex,
    calls: PathBuf,
    versions: PathBuf,
    settings: AllenkRepairConfiguration,
    ranges: Vec<InclusiveFrameRange>,
}

impl Fixture {
    fn request(&self) -> AllenkRepairRequest {
        AllenkRepairRequest::new(&self.source, &self.frames, &self.index_path,
            self.settings.clone(), self.ranges.clone(), PathBuf::from(env!("CARGO_BIN_EXE_aniflow")))
    }

    fn operation(&self) -> AllenkRepairOperation {
        AllenkRepairOperation {
            schema:"aniflow.allenk-frame-repair-provider-configuration/v1".to_owned(),
            settings:self.settings.clone(), ranges:self.ranges.clone(),
        }
    }

    fn set_processor(&mut self, version: &str, body: &str) {
        let script = format!(r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  [ "$#" -eq 2 ] && [ "$2" = "--no-banner" ] || exit 79
  printf "probe\n" >> {versions}
  printf "%s\n" "{version}"
  exit 0
fi
[ "$#" -eq 9 ] || exit 80
[ "$1" = "--input" ] && [ "$3" = "--output" ] || exit 81
[ "$5" = "--remove" ] && [ "$6" = "--no-banner" ] && [ "$8" = "--threshold" ] || exit 82
input="$2"; output="$4"; threshold="$9"
case "$7" in --no-legacy) profile="current_v2";; --legacy) profile="legacy_v1";; *) exit 83;; esac
case "$input" in /*/input.png) ;; *) exit 84;; esac
case "$output" in /*/output.png) ;; *) exit 85;; esac
[ -f "$input" ] && [ ! -e "$output" ] || exit 86
printf "%s:%s\n" "$profile" "$threshold" >> {calls}
{body}
"##, versions=shell_literal(&self.versions), calls=shell_literal(&self.calls));
        write_executable(&self.settings.tool.executable, &script);
        self.settings.tool.sha256 = sha256(script.as_bytes());
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(&self.calls).map(|text| text.lines().map(str::to_owned).collect()).unwrap_or_default()
    }

    fn run(&self) -> PipelineV3RunOutcome {
        allenk::run(self.request(), Some(self.root.join("runs")), &CancellationToken::default(), |_| {}).unwrap()
    }

    fn registry(&self, plan: &PipelineV3Plan) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        for stage in &plan.payload.stages {
            let (manifest, configuration) = if stage.id == "import_frames" {
                (self.settings.import.manifest().unwrap(), self.settings.import.provider_configuration().unwrap())
            } else {
                (self.operation().manifest().unwrap(), self.operation().provider_configuration().unwrap())
            };
            let lock = &stage.provider_lock.payload;
            registry.register(ProviderRegistration::new(&lock.registration_id, manifest, configuration,
                PathBuf::from(env!("CARGO_BIN_EXE_aniflow")), &lock.implementation.id,
                ComponentInventory { tools:lock.tools.clone(), codecs:lock.codecs.clone(), models:lock.models.clone() },
            ).unwrap()).unwrap();
        }
        registry
    }

    fn bindings(&self) -> Vec<PipelineInputBinding> {
        vec![PipelineInputBinding::new("source_video", &self.source),
            PipelineInputBinding::new("source_frames", &self.frames),
            PipelineInputBinding::new("source_index", &self.index_path)]
    }
}

fn make_fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("named repair 音 with spaces");
    fs::create_dir(&root).unwrap();
    let source = root.join("source video.bin");
    let source_bytes = b"synthetic declared video identity, not native media";
    fs::write(&source, source_bytes).unwrap();
    let frame_root = root.join("original frames");
    fs::create_dir(&frame_root).unwrap();
    let frames = (0..3).map(|ordinal| {
        let filename = format!("frame_{ordinal:08}.png");
        let bytes = png([ordinal as u8 + 1, 2, 3]);
        fs::write(frame_root.join(&filename), &bytes).unwrap();
        RifeFrame { ordinal, filename, sha256:sha256(&bytes) }
    }).collect();
    let inspection = temporal::from_probe_documents(&sha256(source_bytes), source_bytes.len() as u64,
        &StreamSelection::default(), &json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
            "time_base":"1/24","start_pts":0,"duration_ts":3,"r_frame_rate":"24/1","avg_frame_rate":"24/1",
            "width":1,"height":1,"pix_fmt":"yuv420p","disposition":{"attached_pic":0}}]}),
        &[(0, json!({"frames":(0..3).map(|ordinal| json!({"stream_index":0,"pts":ordinal,"pkt_dts":ordinal,
            "pkt_duration":1,"key_frame":if ordinal == 0 {1} else {0}})).collect::<Vec<_>>()}),
            json!({"packets":(0..3).map(|ordinal| json!({"stream_index":0,"pts":ordinal,"dts":ordinal,
                "duration":1})).collect::<Vec<_>>()}))],
    ).unwrap();
    let index = RifeFrameIndex { schema:RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(), temporal_inspection:inspection,
        width:1, height:1, frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(), frames, cut_before_ordinals:vec![2] };
    let index_path = root.join("source index.json");
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let decoder = root.join("synthetic ffmpeg");
    write_executable(&decoder, DECODER);
    let processor = root.join("synthetic allenk");
    write_executable(&processor, "#!/bin/sh\nexit 99\n");
    let settings = AllenkRepairConfiguration::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.allenk-frame-repair-configuration/v1",
        "import":{"schema":"aniflow.frame-import-configuration/v1",
            "ffmpeg":{"executable":decoder,"version":"1.0.0","sha256":sha256(DECODER.as_bytes())},
            "tool_timeout_milliseconds":2000,"maximum_source_bytes":1048576,"maximum_frame_bytes":1024,
            "maximum_total_frame_bytes":1048576,"maximum_pixels":1},
        "tool":{"executable":processor,"version":"0.3.2","sha256":sha256(b"#!/bin/sh\nexit 99\n")},
        "watermark_profile":"current_v2"
    })).unwrap()).unwrap();
    let mut fixture = Fixture { _directory:directory, root:root.clone(), source, frames:frame_root, index_path,
        index, calls:root.join("repair calls.txt"), versions:root.join("version calls.txt"), settings,
        ranges:vec![InclusiveFrameRange {first:2,last:2}] };
    fixture.set_processor("0.3.2", &replacement_command());
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

fn report(outcome: &PipelineV3RunOutcome) -> Value {
    outcome.outputs.iter().filter(|output| output.path.is_file()).filter_map(|output| {
        serde_json::from_slice::<Value>(&fs::read(&output.path).unwrap()).ok()
    }).find(|value| value["schema"] == "aniflow.allenk-frame-repair-report/v1").unwrap()
}

#[test]
fn named_policy_is_closed_and_planning_does_not_probe_or_process() {
    let fixture = make_fixture();
    let original = serde_json::to_value(&fixture.settings).unwrap();
    for (field, value) in [("watermark_profile", json!("automatic")), ("detection_threshold_percent", json!(101)),
        ("arguments", json!(["--force"])), ("denoise", json!("ai")), ("unknown", json!(true))] {
        let mut candidate = original.clone();
        candidate[field] = value;
        assert!(AllenkRepairConfiguration::from_json_slice(&serde_json::to_vec(&candidate).unwrap()).is_err());
    }
    let before = snapshot(&fixture.root);
    let plan = allenk::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert_eq!(plan.payload.stages.len(), 2);
    assert_eq!(plan.payload.stages[1].capability.id, "aniflow/frame-repair-allenk");
    assert_eq!(plan.payload.stages[1].depends_on, ["import_frames"]);
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.versions.exists());
    assert!(fixture.calls().is_empty());
}

#[test]
fn named_profiles_use_fixed_argv_and_preserve_unselected_bytes_and_source_clock() {
    for (profile, percent, expected) in [("current_v2",25,"current_v2:0.25"),("legacy_v1",75,"legacy_v1:0.75")] {
        let mut fixture = make_fixture();
        let mut value = serde_json::to_value(&fixture.settings).unwrap();
        value["watermark_profile"] = json!(profile);
        value["detection_threshold_percent"] = json!(percent);
        fixture.settings = AllenkRepairConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        let originals = snapshot(&fixture.frames);
        let outcome = fixture.run();
        assert_eq!(fixture.calls(), [expected]);
        assert_eq!(snapshot(&fixture.frames), originals);
        let evidence = report(&outcome);
        assert_eq!(evidence["watermark_profile"], profile);
        assert_eq!(evidence["observed_processor_version"], "0.3.2");
        assert_eq!(evidence["profile_assets_evidence"], "embedded_in_pinned_executable");
        assert_eq!(evidence["denoising"], "disabled");
        assert_eq!(evidence["host_runtime_evidence"], "unobserved");
        assert_eq!(evidence["backend_qualification"], "unqualified");
        let repair = &evidence["repair"];
        assert_eq!(repair["changed_frame_count"], 1);
        assert_eq!(repair["historical_processing"], "unknown");
        assert_eq!(repair["source_relationship"], "declared_not_verified");
        assert_eq!(repair["output_index"]["temporal_inspection"], serde_json::to_value(&fixture.index.temporal_inspection).unwrap());
        let repaired_frames = &outcome.outputs.iter().find(|output| output.id == "repaired_frames").unwrap().path;
        for ordinal in [0, 2] {
            assert_eq!(repair["frames"][ordinal]["bytes_changed"], false);
            assert_eq!(repair["frames"][ordinal]["input_sha256"], repair["frames"][ordinal]["output_sha256"]);
            let filename = &fixture.index.frames[ordinal].filename;
            assert_eq!(fs::read(repaired_frames.join(filename)).unwrap(), fs::read(fixture.frames.join(filename)).unwrap());
        }
    }
}

#[test]
fn wrong_native_version_and_skipped_selection_never_accept_a_repair() {
    for (version, body) in [("0.3.1", replacement_command()), ("0.3.2", "exit 1".to_owned())] {
        let mut fixture = make_fixture();
        fixture.set_processor(version, &body);
        let originals = snapshot(&fixture.frames);
        let mut completed = Vec::new();
        let result = allenk::run(fixture.request(), Some(fixture.root.join("runs")), &CancellationToken::default(), |event| {
            if let PipelineV3RunProgress::Stage {stage_id,state:PipelineV3ProgressState::Complete} = event {
                completed.push(stage_id.clone());
            }
        });
        assert!(result.is_err());
        assert_eq!(completed, ["import_frames"]);
        assert_eq!(fs::read_to_string(&fixture.versions).unwrap(), "probe\n");
        assert_eq!(fixture.calls().len(), usize::from(version == "0.3.2"));
        assert_eq!(snapshot(&fixture.frames), originals);
    }
}

#[test]
fn resume_reuses_accepted_import_after_cancellation_and_complete_repair_after_success() {
    let fixture = make_fixture();
    let token = CancellationToken::default();
    let mut run_directory = None;
    let result = allenk::run(fixture.request(), Some(fixture.root.join("runs")), &token, |event| match event {
        PipelineV3RunProgress::Started {run_directory:started,..} => run_directory = Some(started.clone()),
        PipelineV3RunProgress::Stage {stage_id,state:PipelineV3ProgressState::Complete} if stage_id == "import_frames" => token.cancel(),
        _ => {}
    });
    assert!(result.is_err());
    assert!(fixture.calls().is_empty());
    let outcome = allenk::resume(fixture.request(), run_directory.unwrap(), &CancellationToken::default(), |_| {}).unwrap();
    assert_eq!(outcome.reused_stages, ["import_frames"]);
    assert_eq!(outcome.executed_stages, ["repair_frames"]);
    assert_eq!(fixture.calls().len(), 1);
    let versions = fs::read(&fixture.versions).unwrap();
    let completed = allenk::resume(fixture.request(), &outcome.run_directory, &CancellationToken::default(), |_| {}).unwrap();
    assert_eq!(completed.reused_stages, ["import_frames", "repair_frames"]);
    assert!(completed.executed_stages.is_empty());
    assert_eq!(fixture.calls().len(), 1);
    assert_eq!(fs::read(&fixture.versions).unwrap(), versions, "accepted-stage reuse launches no native version probe");
}

#[test]
fn failed_repair_retries_all_selected_frames_with_the_same_accepted_import() {
    let mut fixture = make_fixture();
    fixture.ranges = vec![InclusiveFrameRange {first:1,last:2}];
    let marker = fixture.root.join("inject failure");
    fs::write(&marker, b"enabled").unwrap();
    let body = format!("if [ -f {marker} ] && [ \"$(wc -l < {calls})\" -eq 2 ]; then exit 1; fi\n{replacement}",
        marker=shell_literal(&marker), calls=shell_literal(&fixture.calls), replacement=replacement_command());
    fixture.set_processor("0.3.2", &body);
    let mut run_directory = None;
    assert!(allenk::run(fixture.request(), Some(fixture.root.join("runs")), &CancellationToken::default(), |event| {
        if let PipelineV3RunProgress::Started {run_directory:started,..} = event { run_directory = Some(started.clone()); }
    }).is_err());
    assert_eq!(fixture.calls().len(), 2);
    fs::remove_file(marker).unwrap();
    let outcome = allenk::resume(fixture.request(), run_directory.unwrap(), &CancellationToken::default(), |_| {}).unwrap();
    assert_eq!(outcome.reused_stages, ["import_frames"]);
    assert_eq!(outcome.executed_stages, ["repair_frames"]);
    assert_eq!(fixture.calls().len(), 4, "an unaccepted repaired frame is not an independent checkpoint");
}

#[test]
fn changed_settings_require_a_new_run_before_progress_or_mutation() {
    let mut fixture = make_fixture();
    let outcome = fixture.run();
    let before = snapshot(&outcome.run_directory);
    fixture.settings.detection_threshold_percent = 50;
    let mut progressed = false;
    assert!(allenk::resume(fixture.request(), &outcome.run_directory, &CancellationToken::default(), |_| progressed = true).is_err());
    assert!(!progressed);
    assert_eq!(fixture.calls().len(), 1);
    assert_eq!(snapshot(&outcome.run_directory), before);
}

#[test]
fn generic_resume_rechecks_native_and_decoder_pins_even_when_every_stage_was_accepted() {
    for decoder in [false, true] {
        let fixture = make_fixture();
        let plan = allenk::plan(&fixture.request(), &CancellationToken::default()).unwrap();
        let registry = fixture.registry(&plan);
        let outcome = fixture.run();
        let before = snapshot(&outcome.run_directory);
        let path = if decoder {&fixture.settings.import.ffmpeg.executable} else {&fixture.settings.tool.executable};
        let mut changed = fs::read(path).unwrap();
        changed.extend_from_slice(b"\n# changed after registration\n");
        fs::write(path, changed).unwrap();
        let mut progressed = false;
        assert!(resume_v3_with_progress_and_cancellation(
            PipelineV3ResumeRequest::new(&outcome.run_directory, fixture.bindings(), registry)
                .with_execution_limits(fixture.request().execution_limits),
            &CancellationToken::default(), |_| progressed = true,
        ).is_err());
        assert!(!progressed);
        assert_eq!(fixture.calls().len(), 1);
        assert_eq!(snapshot(&outcome.run_directory), before);
    }
}

#[test]
fn generic_named_repair_refuses_loose_import_reports_without_the_exact_import_producer() {
    let fixture = make_fixture();
    let genuine = allenk::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    let lock = &genuine.payload.stages[1].provider_lock.payload;
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderRegistration::new(&lock.registration_id, fixture.operation().manifest().unwrap(),
        fixture.operation().provider_configuration().unwrap(), PathBuf::from(env!("CARGO_BIN_EXE_aniflow")),
        &lock.implementation.id, ComponentInventory {tools:lock.tools.clone(),codecs:lock.codecs.clone(),models:lock.models.clone()},
    ).unwrap()).unwrap();
    let import = fixture.root.join("loose import.json");
    fs::write(&import, b"{\"accepted\":true}").unwrap();
    let authored = json!({
        "schema":"aniflow.pipeline/v3","name":"loose-named-repair",
        "inputs":[
            {"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"},
            {"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"},
            {"id":"imported_frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video"},
            {"id":"import_evidence","artifact_type":"application/vnd.aniflow.frame-import-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"}
        ],
        "stages":[{"id":"repair_frames","depends_on":[],"capability":{"id":"aniflow/frame-repair-allenk","version_requirement":"=1.0.0"},
            "provider":{"primary":{"registration_id":lock.registration_id},"fallbacks":[]},
            "inputs":[{"port":"frames","artifacts":["imported_frames"]},{"port":"import","artifacts":["import_evidence"]},{"port":"index","artifacts":["source_index"]}],
            "outputs":[
                {"port":"frames","artifacts":[{"id":"repaired_frames","relative_path":"artifacts/repair/frames","kind":"directory"}]},
                {"port":"index","artifacts":[{"id":"repaired_index","relative_path":"artifacts/repair/index.json","kind":"file"}]},
                {"port":"evidence","artifacts":[{"id":"repair_evidence","relative_path":"artifacts/repair/report.json","kind":"file"}]}
            ],
            "validations":[{"id":"frames_integrity","artifact":"repaired_frames","contract":"aniflow.validation/artifact-integrity/v1"},
                {"id":"index_integrity","artifact":"repaired_index","contract":"aniflow.validation/artifact-integrity/v1"},
                {"id":"repair_integrity","artifact":"repair_evidence","contract":"aniflow.validation/artifact-integrity/v1"}]
        }],
        "outputs":[{"id":"frames","artifact":"repaired_frames","required_validations":["frames_integrity"]},
            {"id":"index","artifact":"repaired_index","required_validations":["index_integrity"]},
            {"id":"evidence","artifact":"repair_evidence","required_validations":["repair_integrity"]}]
    });
    let bindings = vec![PipelineInputBinding::new("source_video", &fixture.source),
        PipelineInputBinding::new("source_index", &fixture.index_path),
        PipelineInputBinding::new("imported_frames", &fixture.frames), PipelineInputBinding::new("import_evidence", import)];
    let plan = resolve_pipeline_v3(&PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&authored).unwrap()).unwrap(),
        &bindings, &registry, &PipelinePlanningContext {
            host:HostResources {cpu_threads:8,memory_mib:16*1024,storage_mib:64*1024,gpu_available:false,network_available:false},
            allowed_side_effects:vec![SideEffect::FilesystemRead,SideEffect::FilesystemWrite,SideEffect::EnvironmentRead,SideEffect::Subprocess],
            offline:true,
        }).unwrap();
    let output = fixture.root.join("loose runs");
    let mut progressed = false;
    assert!(run_v3_with_progress_and_cancellation(PipelineV3RunRequest::new(plan, bindings, registry).with_output_directory(&output),
        &CancellationToken::default(), |_| progressed = true).is_err());
    assert!(!progressed);
    assert!(!output.exists());
    assert!(fixture.calls().is_empty());
}
