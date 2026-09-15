#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use aniflow::{ProviderExecutionOutcome, ProviderExecutionReport, RunRequest};
use tempfile::TempDir;

#[test]
fn pipeline_v2_processor_families_share_the_provider_runtime() {
    let fixture = PipelineFixture::new();
    let pipeline = fixture.write_pipeline("all-processors", false);

    let outcome = aniflow::run(
        RunRequest::new(&fixture.source, pipeline).with_output_directory(&fixture.runs),
    )
    .expect("pipeline v2 provider fixture should complete");

    assert!(outcome.output.is_file());
    for stage in [
        "frame_01_external",
        "frame_02_gemini",
        "frame_03_upscayl",
        "audio_01_audio",
        "video_01_video",
    ] {
        assert!(
            outcome
                .run_directory
                .join(format!("providers/{stage}/provider-lock.json"))
                .is_file(),
            "{stage} should retain a provider lock"
        );
    }
    for report in [
        "providers/frame_01_external/reports/frame-00000001-png.json",
        "providers/frame_02_gemini/reports/batch.json",
        "providers/frame_03_upscayl/reports/batch.json",
        "providers/audio_01_audio/reports/artifact.json",
        "providers/video_01_video/reports/artifact.json",
    ] {
        let report = fs::read(outcome.run_directory.join(report))
            .expect("processor execution report should be retained");
        let report = ProviderExecutionReport::from_json_slice(&report)
            .expect("processor execution report should validate");
        assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Succeeded);
    }
    assert!(
        outcome
            .run_directory
            .join("metadata/frame_02_gemini.jsonl")
            .is_file()
    );
}

#[test]
fn semantically_invalid_frame_never_completes_the_processor_stage() {
    let fixture = PipelineFixture::new();
    let pipeline = fixture.write_pipeline("invalid-frame", true);

    let error = aniflow::run(
        RunRequest::new(&fixture.source, pipeline).with_output_directory(&fixture.runs),
    )
    .expect_err("invalid frame output should fail the stage");

    assert!(error.to_string().contains("not a valid PNG"));
    let run_directory = fs::read_dir(&fixture.runs)
        .expect("failed run should retain its workspace")
        .next()
        .expect("failed run directory should exist")
        .expect("failed run directory should be readable")
        .path();
    assert!(
        !run_directory
            .join("state/frame_01_external.complete")
            .exists()
    );
    assert!(
        !run_directory
            .join("frames/stages/01-external/frame-00000001.png")
            .exists()
    );
    let report =
        fs::read(run_directory.join("providers/frame_01_external/reports/frame-00000001-png.json"))
            .expect("provider report should survive processor-specific validation failure");
    let report = ProviderExecutionReport::from_json_slice(&report)
        .expect("provider report should remain self-validating");
    assert_eq!(report.payload.outcome, ProviderExecutionOutcome::Succeeded);
}

struct PipelineFixture {
    _temporary: TempDir,
    root: PathBuf,
    source: PathBuf,
    provider: PathBuf,
    runs: PathBuf,
}

impl PipelineFixture {
    fn new() -> Self {
        let temporary = TempDir::new().expect("temporary directory should be created");
        let root = temporary.path().to_path_buf();
        let source = root.join("source.mp4");
        generate_source(&source);
        let provider = write_provider(&root);
        let runs = root.join("runs");
        Self {
            _temporary: temporary,
            root,
            source,
            provider,
            runs,
        }
    }

    fn write_pipeline(&self, name: &str, invalid: bool) -> PathBuf {
        let path = self.root.join(format!("{name}.yml"));
        let external_mode = if invalid { "invalid" } else { "copy" };
        let extra_processors = if invalid {
            String::new()
        } else {
            format!(
                r#"
  - kind: gemini_watermark_remover
    id: gemini
    command: {command:?}
    json: true
  - kind: upscayl_ncnn
    id: upscayl
    command: {command:?}
    model: fixture
    scale: 2

audio_processors:
  - id: audio
    command: {command:?}
    arguments: ["copy", "{{input}}", "{{output}}"]
    output_extension: wav

video_processors:
  - id: video
    command: {command:?}
    arguments: ["copy", "{{input}}", "{{output}}"]
    output_extension: mp4
"#,
                command = self.provider.to_string_lossy(),
            )
        };
        let yaml = format!(
            r#"version: 2
name: {name}
frame_processors:
  - kind: external
    id: external
    command: {command:?}
    arguments: ["{external_mode}", "{{input}}", "{{output}}"]
    concurrency: 2
{extra_processors}
validation:
  require_uniform_dimensions: true
  minimum_frame_bytes: 24

renderflow:
  enabled: false

output:
  file: output/master.mp4
"#,
            command = self.provider.to_string_lossy(),
        );
        fs::write(&path, yaml).expect("fixture pipeline should be written");
        path
    }
}

fn write_provider(directory: &Path) -> PathBuf {
    let path = directory.join("pipeline provider");
    fs::write(
        &path,
        r#"#!/bin/sh
case "$1" in
  copy)
    cp "$2" "$3"
    ;;
  invalid)
    printf "this is definitely not a png" > "$3"
    ;;
  remove)
    input="$2"
    output="$4"
    mkdir -p "$output"
    cp "$input"/*.png "$output"/
    printf '[{"input":"%s/frame-00000001.png","meta":{"applied":false,"qualityStatus":"clean"}}]\n' "$input"
    ;;
  -i)
    input="$2"
    output="$4"
    mkdir -p "$output"
    cp "$input"/*.png "$output"/
    ;;
  *)
    exit 64
    ;;
esac
"#,
    )
    .expect("fixture provider should be written");
    let mut permissions = fs::metadata(&path)
        .expect("fixture provider metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("fixture provider should be executable");
    path
}

fn generate_source(destination: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x64:rate=2:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(destination)
        .status()
        .expect("ffmpeg should execute");
    assert!(status.success(), "ffmpeg should create the fixture media");
}
