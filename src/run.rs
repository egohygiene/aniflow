use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::command;
use crate::media::{self, MediaInspection};
use crate::pipeline::{
    CommandProcessor, FrameProcessor, FrameValidation, Pipeline, RenderflowHandoff, SubtitleMode,
    expand_argument, sanitize_identifier,
};
use crate::state::{ArtifactRecord, RunManifest, StageRecord, StageStatus};
use crate::workspace::RunWorkspace;
use crate::{ProgressState, RunOperation, RunOutcome, RunProgress};

#[derive(Debug, Serialize)]
struct DeliveryManifest {
    schema_version: u32,
    aniflow_version: String,
    run_id: String,
    pipeline_name: String,
    created_at: DateTime<Utc>,
    source_sha256: String,
    master: DeliveryArtifact,
    media: MediaInspection,
    renderflow_directory: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct DeliveryArtifact {
    path: PathBuf,
    sha256: String,
}

pub fn start(
    input: &Path,
    pipeline_path: &Path,
    output_parent: Option<&Path>,
    progress: &mut dyn FnMut(&RunProgress),
) -> Result<RunOutcome> {
    let source = input
        .canonicalize()
        .with_context(|| format!("failed to resolve input {}", input.display()))?;
    let original_pipeline_path = pipeline_path
        .canonicalize()
        .with_context(|| format!("failed to resolve pipeline {}", pipeline_path.display()))?;
    let mut pipeline = Pipeline::load(&original_pipeline_path)?;
    require_pipeline_commands(&pipeline)?;
    let inspection = media::inspect(&source)?;
    let workspace = RunWorkspace::create(output_parent, &pipeline.name)?;

    snapshot_pipeline_inputs(&mut pipeline, &original_pipeline_path, &workspace)?;
    let pipeline_yaml = serde_yaml::to_string(&pipeline)?;
    fs::write(workspace.pipeline_copy(), pipeline_yaml).with_context(|| {
        format!(
            "failed to snapshot pipeline at {}",
            workspace.pipeline_copy().display()
        )
    })?;

    let run_id = workspace
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("aniflow-run")
        .to_owned();
    let stages = pipeline
        .stage_names(inspection.has_audio)
        .into_iter()
        .map(|name| {
            (
                name,
                StageRecord {
                    status: StageStatus::Pending,
                    started_at: None,
                    completed_at: None,
                    message: None,
                },
            )
        })
        .collect();
    let now = Utc::now();
    let source_sha256 = sha256_file(&source)?;
    let mut manifest = RunManifest {
        schema_version: 2,
        aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        run_id,
        pipeline_name: pipeline.name.clone(),
        pipeline_file: workspace.pipeline_copy(),
        source_file: source,
        source_sha256,
        created_at: now,
        updated_at: now,
        inspection,
        stages,
        artifacts: BTreeMap::new(),
    };
    manifest.save(&workspace.manifest())?;

    progress(&RunProgress::Started {
        operation: RunOperation::Run,
        run_directory: workspace.root.clone(),
        pipeline_name: pipeline.name.clone(),
    });

    execute_pipeline(&workspace, &pipeline, &mut manifest, progress)
}

pub fn resume(run_directory: &Path, progress: &mut dyn FnMut(&RunProgress)) -> Result<RunOutcome> {
    let workspace = RunWorkspace::open(run_directory)?;
    let pipeline = Pipeline::load(&workspace.pipeline_copy())?;
    require_pipeline_commands(&pipeline)?;
    let mut manifest = RunManifest::load(&workspace.manifest())?;

    if !manifest.source_file.is_file() {
        bail!(
            "the original source is unavailable: {}",
            manifest.source_file.display()
        );
    }
    if sha256_file(&manifest.source_file)? != manifest.source_sha256 {
        bail!("the source checksum changed; refusing to resume with different media");
    }

    progress(&RunProgress::Started {
        operation: RunOperation::Resume,
        run_directory: workspace.root.clone(),
        pipeline_name: pipeline.name.clone(),
    });

    execute_pipeline(&workspace, &pipeline, &mut manifest, progress)
}

fn require_pipeline_commands(pipeline: &Pipeline) -> Result<()> {
    for executable in pipeline.required_commands() {
        if matches!(executable.as_str(), "ffmpeg" | "ffprobe") {
            command::require_executable(&executable)?;
        } else {
            command::require_available(&executable)?;
        }
    }
    Ok(())
}

fn execute_pipeline(
    workspace: &RunWorkspace,
    pipeline: &Pipeline,
    manifest: &mut RunManifest,
    progress: &mut dyn FnMut(&RunProgress),
) -> Result<RunOutcome> {
    let source = manifest.source_file.clone();
    let inspection = manifest.inspection.clone();

    run_stage(workspace, manifest, "inspect", progress, || {
        let destination = workspace.metadata().join("source.json");
        fs::write(&destination, serde_json::to_string_pretty(&inspection)?)
            .with_context(|| format!("failed to write {}", destination.display()))?;
        let tools = pipeline
            .required_commands()
            .into_iter()
            .map(|executable| {
                let summary = command::executable_summary(&executable)?;
                Ok((executable, summary))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let tools_destination = workspace.metadata().join("tools.json");
        fs::write(&tools_destination, serde_json::to_string_pretty(&tools)?)
            .with_context(|| format!("failed to write {}", tools_destination.display()))?;
        Ok(format!(
            "{}x{} at {:.6} fps",
            inspection.width, inspection.height, inspection.frames_per_second
        ))
    })?;

    run_stage(workspace, manifest, "extract", progress, || {
        extract_media(workspace, &source, &inspection)
    })?;

    let mut frame_directory = workspace.source_frames();
    for (index, processor) in pipeline.enabled_frame_processors().enumerate() {
        let stage = format!(
            "frame_{:02}_{}",
            index + 1,
            sanitize_identifier(processor.id())
        );
        let input_directory = frame_directory.clone();
        let output_directory = workspace.frame_stage(index, processor.id());
        fs::create_dir_all(&output_directory)?;
        run_stage(workspace, manifest, &stage, progress, || {
            if processor.is_batch() {
                process_frame_batch(
                    workspace,
                    processor,
                    &input_directory,
                    &output_directory,
                    &stage,
                )
            } else {
                process_frames(
                    workspace,
                    processor,
                    &input_directory,
                    &output_directory,
                    &stage,
                )
            }
        })?;
        frame_directory = output_directory;
    }

    run_stage(workspace, manifest, "validate_frames", progress, || {
        validate_frames(
            &workspace.source_frames(),
            &frame_directory,
            &pipeline.validation,
        )
    })?;
    run_stage(workspace, manifest, "assemble_video", progress, || {
        assemble_video(workspace, pipeline, &inspection, &frame_directory)
    })?;

    let mut current_video = workspace.video().join("assembled.mp4");
    if inspection.has_audio {
        let mut current_audio = workspace.audio().join("source.wav");
        for (index, processor) in pipeline.enabled_audio_processors().enumerate() {
            let stage = format!(
                "audio_{:02}_{}",
                index + 1,
                sanitize_identifier(&processor.id)
            );
            let extension = processor.output_extension.as_deref().unwrap_or("wav");
            let output = workspace.audio_stage_file(index, &processor.id, extension);
            let input = current_audio.clone();
            run_stage(workspace, manifest, &stage, progress, || {
                process_media_command(workspace, processor, &input, &output, &stage)
            })?;
            current_audio = output;
        }

        let video_input = current_video.clone();
        run_stage(workspace, manifest, "restore_audio", progress, || {
            restore_audio(workspace, &video_input, &current_audio)
        })?;
        current_video = workspace.video().join("with-audio.mp4");
    }

    if let Some(subtitles) = pipeline
        .subtitles
        .as_ref()
        .filter(|subtitles| subtitles.enabled)
    {
        let subtitle_input = current_video.clone();
        run_stage(workspace, manifest, "subtitles", progress, || {
            apply_subtitles(workspace, subtitles, &subtitle_input)
        })?;
        current_video = workspace.video().join("with-subtitles.mp4");
    }

    for (index, processor) in pipeline.enabled_video_processors().enumerate() {
        let stage = format!(
            "video_{:02}_{}",
            index + 1,
            sanitize_identifier(&processor.id)
        );
        let extension = processor.output_extension.as_deref().unwrap_or("mp4");
        let output = workspace.video_stage_file(index, &processor.id, extension);
        let input = current_video.clone();
        run_stage(workspace, manifest, &stage, progress, || {
            process_media_command(workspace, processor, &input, &output, &stage)
        })?;
        current_video = output;
    }

    let final_source = current_video;
    run_stage(workspace, manifest, "finalize", progress, || {
        let output = workspace.root.join(&pipeline.output.file);
        let output_parent = output
            .parent()
            .context("configured output does not have a parent directory")?;
        fs::create_dir_all(output_parent)?;
        fs::copy(&final_source, &output).with_context(|| {
            format!(
                "failed to copy {} to {}",
                final_source.display(),
                output.display()
            )
        })?;
        let checksum = sha256_file(&output)?;
        Ok(format!("{} ({checksum})", output.display()))
    })?;

    let final_output = workspace.root.join(&pipeline.output.file);
    let final_checksum = sha256_file(&final_output)?;
    manifest.artifacts.insert(
        "master_video".to_owned(),
        ArtifactRecord {
            path: final_output.clone(),
            sha256: Some(final_checksum.clone()),
        },
    );
    manifest.save(&workspace.manifest())?;

    if let Some(renderflow) = pipeline
        .renderflow
        .as_ref()
        .filter(|renderflow| renderflow.enabled)
    {
        run_stage(workspace, manifest, "renderflow", progress, || {
            handoff_to_renderflow(workspace, renderflow, &final_output)
        })?;
    }

    let delivery = DeliveryManifest {
        schema_version: 1,
        aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
        run_id: manifest.run_id.clone(),
        pipeline_name: manifest.pipeline_name.clone(),
        created_at: Utc::now(),
        source_sha256: manifest.source_sha256.clone(),
        master: DeliveryArtifact {
            path: pipeline.output.file.clone(),
            sha256: final_checksum,
        },
        media: manifest.inspection.clone(),
        renderflow_directory: pipeline
            .renderflow
            .as_ref()
            .filter(|renderflow| renderflow.enabled)
            .map(|_| PathBuf::from("renderflow")),
    };
    run_stage(workspace, manifest, "delivery", progress, || {
        fs::write(
            workspace.delivery_manifest(),
            serde_json::to_string_pretty(&delivery)?,
        )
        .with_context(|| {
            format!(
                "failed to write delivery manifest {}",
                workspace.delivery_manifest().display()
            )
        })?;
        Ok(workspace.delivery_manifest().display().to_string())
    })?;

    let delivery_path = workspace.delivery_manifest();
    manifest.artifacts.insert(
        "delivery_manifest".to_owned(),
        ArtifactRecord {
            path: delivery_path.clone(),
            sha256: Some(sha256_file(&delivery_path)?),
        },
    );
    manifest.save(&workspace.manifest())?;

    Ok(RunOutcome {
        run_directory: workspace.root.clone(),
        output: final_output,
        delivery_manifest: delivery_path,
        run_manifest: workspace.manifest(),
    })
}

fn run_stage<F>(
    workspace: &RunWorkspace,
    manifest: &mut RunManifest,
    stage: &str,
    progress: &mut dyn FnMut(&RunProgress),
    operation: F,
) -> Result<()>
where
    F: FnOnce() -> Result<String>,
{
    if workspace.stage_marker(stage).is_file() {
        manifest.stage_complete(stage, Some("reused completion checkpoint".to_owned()));
        manifest.save(&workspace.manifest())?;
        progress(&RunProgress::Stage {
            name: stage.to_owned(),
            state: ProgressState::Cached,
        });
        return Ok(());
    }

    progress(&RunProgress::Stage {
        name: stage.to_owned(),
        state: ProgressState::Running,
    });
    manifest.stage_running(stage);
    manifest.save(&workspace.manifest())?;

    match operation() {
        Ok(message) => {
            fs::write(workspace.stage_marker(stage), format!("{message}\n"))?;
            manifest.stage_complete(stage, Some(message));
            manifest.save(&workspace.manifest())?;
            progress(&RunProgress::Stage {
                name: stage.to_owned(),
                state: ProgressState::Complete,
            });
            Ok(())
        }
        Err(error) => {
            manifest.stage_failed(stage, format!("{error:#}"));
            manifest.save(&workspace.manifest())?;
            progress(&RunProgress::Stage {
                name: stage.to_owned(),
                state: ProgressState::Failed,
            });
            Err(error)
        }
    }
}

fn extract_media(
    workspace: &RunWorkspace,
    source: &Path,
    inspection: &MediaInspection,
) -> Result<String> {
    let frame_pattern = workspace.source_frames().join("frame-%08d.png");
    let frame_arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-y"),
        OsString::from("-i"),
        source.as_os_str().to_owned(),
        OsString::from("-map"),
        OsString::from("0:v:0"),
        OsString::from("-fps_mode"),
        OsString::from("passthrough"),
        frame_pattern.as_os_str().to_owned(),
    ];
    command::run_logged(
        "ffmpeg",
        frame_arguments,
        &workspace.stage_log("extract-frames"),
    )?;

    if inspection.has_audio {
        let audio_output = workspace.audio().join("source.wav");
        let audio_arguments = vec![
            OsString::from("-hide_banner"),
            OsString::from("-y"),
            OsString::from("-i"),
            source.as_os_str().to_owned(),
            OsString::from("-map"),
            OsString::from("0:a:0"),
            OsString::from("-vn"),
            OsString::from("-c:a"),
            OsString::from("pcm_s24le"),
            audio_output.as_os_str().to_owned(),
        ];
        command::run_logged(
            "ffmpeg",
            audio_arguments,
            &workspace.stage_log("extract-audio"),
        )?;
    }

    let frames = image_files(&workspace.source_frames())?;
    if frames.is_empty() {
        bail!("frame extraction completed without producing frames");
    }
    Ok(format!(
        "{} frames and source timing extracted",
        frames.len()
    ))
}

fn process_frames(
    workspace: &RunWorkspace,
    processor: &FrameProcessor,
    input_directory: &Path,
    output_directory: &Path,
    stage: &str,
) -> Result<String> {
    let frames = Arc::new(image_files(input_directory)?);
    let next_index = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(Mutex::new(Vec::<String>::new()));
    let concurrency = processor
        .concurrency()
        .context("per-frame processor did not define concurrency")?
        .min(frames.len().max(1));

    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            let frames = Arc::clone(&frames);
            let next_index = Arc::clone(&next_index);
            let failures = Arc::clone(&failures);

            scope.spawn(move || {
                loop {
                    let index = next_index.fetch_add(1, Ordering::Relaxed);
                    let Some(input) = frames.get(index) else {
                        break;
                    };
                    let Some(frame_name) = input.file_name().and_then(|name| name.to_str()) else {
                        failures
                            .lock()
                            .expect("failure lock poisoned")
                            .push(format!("invalid UTF-8 frame name: {}", input.display()));
                        continue;
                    };
                    let output = output_directory.join(frame_name);
                    if output.is_file() && png_dimensions(&output).is_ok() {
                        continue;
                    }
                    let arguments = processor
                        .arguments(input, &output)
                        .iter()
                        .map(|argument| {
                            expand_argument(argument, input, &output, frame_name, &workspace.root)
                        })
                        .collect::<Vec<_>>();
                    let result = Command::new(processor.command()).args(&arguments).output();
                    match result {
                        Ok(command_output)
                            if command_output.status.success() && output.is_file() => {}
                        Ok(command_output) => {
                            let stderr = String::from_utf8_lossy(&command_output.stderr);
                            failures
                                .lock()
                                .expect("failure lock poisoned")
                                .push(format!("{frame_name}: {}", stderr.trim()));
                        }
                        Err(error) => {
                            failures
                                .lock()
                                .expect("failure lock poisoned")
                                .push(format!("{frame_name}: {error}"));
                        }
                    }
                }
            });
        }
    });

    let failures = failures.lock().expect("failure lock poisoned");
    if !failures.is_empty() {
        let failure_log = workspace.stage_log(&format!("{stage}-failures"));
        fs::write(&failure_log, failures.join("\n"))?;
        bail!(
            "{} frame(s) failed; see {}",
            failures.len(),
            failure_log.display()
        );
    }

    let completed = image_files(output_directory)?.len();
    Ok(format!(
        "{completed} frames processed with {}",
        processor.command()
    ))
}

fn process_frame_batch(
    workspace: &RunWorkspace,
    processor: &FrameProcessor,
    input_directory: &Path,
    output_directory: &Path,
    stage: &str,
) -> Result<String> {
    let input_frames = image_files(input_directory)?;
    let existing_outputs = image_files(output_directory)?;
    if !input_frames.is_empty()
        && input_frames.len() == existing_outputs.len()
        && input_frames
            .iter()
            .zip(&existing_outputs)
            .all(|(input, output)| {
                input.file_name() == output.file_name() && png_dimensions(output).is_ok()
            })
    {
        return Ok(format!(
            "{} frames reused from complete native batch output",
            existing_outputs.len()
        ));
    }

    let arguments = processor.arguments(input_directory, output_directory);
    let command_output =
        command::run_logged(processor.command(), arguments, &workspace.stage_log(stage))?;

    if matches!(processor, FrameProcessor::GeminiWatermarkRemover { .. }) {
        let records = compact_gwr_batch_records(&command_output.stdout);
        if !records.is_empty() {
            fs::write(
                workspace.metadata().join(format!("{stage}.jsonl")),
                format!("{}\n", records.join("\n")),
            )?;
            fs::write(
                workspace.stage_log(stage),
                format!(
                    "{} completed in native batch mode; {} compact records written to metadata/{stage}.jsonl\n",
                    processor.command(),
                    records.len()
                ),
            )?;
        }
    }

    let output_frames = image_files(output_directory)?;
    let completed = output_frames.len();
    if completed != input_frames.len() {
        bail!(
            "batch processor `{}` produced {completed} frames; expected {}",
            processor.id(),
            input_frames.len()
        );
    }
    for (input, output) in input_frames.iter().zip(&output_frames) {
        if input.file_name() != output.file_name() {
            bail!(
                "batch processor `{}` changed frame ordering or names",
                processor.id()
            );
        }
        png_dimensions(output)?;
    }
    Ok(format!(
        "{completed} frames processed in one {} batch",
        processor.command()
    ))
}

fn compact_gwr_batch_records(stdout: &[u8]) -> Vec<String> {
    let decoded_stdout = String::from_utf8_lossy(stdout);

    let Some(line) = decoded_stdout.lines().rev().find(|line| {
        let line = line.trim_start();
        line.starts_with('[') || line.starts_with('{')
    }) else {
        return Vec::new();
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Vec::new();
    };

    let values = value
        .as_array()
        .map_or_else(|| vec![&value], |items| items.iter().collect());
    let mut records = values
        .into_iter()
        .filter_map(|item| {
            let frame_name = Path::new(item["input"].as_str()?).file_name()?.to_str()?;
            compact_gwr_value(frame_name, &item["meta"])
        })
        .collect::<Vec<_>>();
    records.sort();
    records
}

fn compact_gwr_value(frame_name: &str, meta: &serde_json::Value) -> Option<String> {
    serde_json::to_string(&serde_json::json!({
        "frame": frame_name,
        "applied": meta["applied"],
        "decision_tier": meta["decisionTier"],
        "quality_status": meta["qualityStatus"],
        "retry_recommended": meta["retryRecommended"],
        "position": meta["position"],
    }))
    .ok()
}

fn validate_frames(
    source_directory: &Path,
    output_directory: &Path,
    validation: &FrameValidation,
) -> Result<String> {
    let source_frames = image_files(source_directory)?;
    let output_frames = image_files(output_directory)?;
    if source_frames.is_empty() {
        bail!("source frame directory is empty");
    }
    if source_frames.len() != output_frames.len() {
        bail!(
            "frame count mismatch: expected {}, found {}",
            source_frames.len(),
            output_frames.len()
        );
    }

    let mut expected_dimensions = None;
    for (expected, actual) in source_frames.iter().zip(&output_frames) {
        if expected.file_name() != actual.file_name() {
            bail!(
                "frame ordering mismatch: {} does not correspond to {}",
                expected.display(),
                actual.display()
            );
        }
        let size = fs::metadata(actual)?.len();
        if size < validation.minimum_frame_bytes {
            bail!(
                "frame {} is suspiciously small: {size} bytes",
                actual.display()
            );
        }
        if validation.require_uniform_dimensions {
            let dimensions = png_dimensions(actual)?;
            if let Some(expected) = expected_dimensions {
                if dimensions != expected {
                    bail!(
                        "frame dimension discontinuity: {} is {}x{}, expected {}x{}",
                        actual.display(),
                        dimensions.0,
                        dimensions.1,
                        expected.0,
                        expected.1
                    );
                }
            } else {
                expected_dimensions = Some(dimensions);
            }
        }
    }

    let dimension_message = expected_dimensions
        .map(|(width, height)| format!(", uniform {width}x{height}"))
        .unwrap_or_default();
    Ok(format!(
        "{} ordered frames validated{dimension_message}",
        output_frames.len()
    ))
}

fn png_dimensions(path: &Path) -> Result<(u32, u32)> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut header = [0_u8; 24];
    file.read_exact(&mut header)
        .with_context(|| format!("failed to read PNG header {}", path.display()))?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        bail!("frame is not a valid PNG: {}", path.display());
    }
    let width = u32::from_be_bytes(header[16..20].try_into()?);
    let height = u32::from_be_bytes(header[20..24].try_into()?);
    if width == 0 || height == 0 {
        bail!("frame has invalid dimensions: {}", path.display());
    }
    Ok((width, height))
}

fn assemble_video(
    workspace: &RunWorkspace,
    pipeline: &Pipeline,
    inspection: &MediaInspection,
    frames: &Path,
) -> Result<String> {
    let frame_pattern = frames.join("frame-%08d.png");
    let output = workspace.video().join("assembled.mp4");
    let arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-y"),
        OsString::from("-framerate"),
        OsString::from(format!("{:.10}", inspection.frames_per_second)),
        OsString::from("-i"),
        frame_pattern.as_os_str().to_owned(),
        OsString::from("-c:v"),
        OsString::from(pipeline.output.video_codec.as_str()),
        OsString::from("-crf"),
        OsString::from(pipeline.output.crf.to_string()),
        OsString::from("-preset"),
        OsString::from(pipeline.output.preset.as_str()),
        OsString::from("-pix_fmt"),
        OsString::from(pipeline.output.pixel_format.as_str()),
        OsString::from("-movflags"),
        OsString::from("+faststart"),
        output.as_os_str().to_owned(),
    ];
    command::run_logged("ffmpeg", arguments, &workspace.stage_log("assemble-video"))?;
    Ok(output.display().to_string())
}

fn process_media_command(
    workspace: &RunWorkspace,
    processor: &CommandProcessor,
    input: &Path,
    output: &Path,
    stage: &str,
) -> Result<String> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let arguments = processor
        .arguments
        .iter()
        .map(|argument| expand_argument(argument, input, output, "", &workspace.root))
        .collect::<Vec<_>>();
    command::run_logged(&processor.command, arguments, &workspace.stage_log(stage))?;
    if !output.is_file() {
        bail!(
            "processor `{}` succeeded but did not create {}",
            processor.id,
            output.display()
        );
    }
    Ok(output.display().to_string())
}

fn restore_audio(workspace: &RunWorkspace, video: &Path, audio: &Path) -> Result<String> {
    let output = workspace.video().join("with-audio.mp4");
    let arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-y"),
        OsString::from("-i"),
        video.as_os_str().to_owned(),
        OsString::from("-i"),
        audio.as_os_str().to_owned(),
        OsString::from("-map"),
        OsString::from("0:v:0"),
        OsString::from("-map"),
        OsString::from("1:a:0"),
        OsString::from("-c:v"),
        OsString::from("copy"),
        OsString::from("-c:a"),
        OsString::from("aac"),
        OsString::from("-b:a"),
        OsString::from("320k"),
        OsString::from("-shortest"),
        OsString::from("-movflags"),
        OsString::from("+faststart"),
        output.as_os_str().to_owned(),
    ];
    command::run_logged("ffmpeg", arguments, &workspace.stage_log("restore-audio"))?;
    Ok(output.display().to_string())
}

fn apply_subtitles(
    workspace: &RunWorkspace,
    subtitles: &crate::pipeline::Subtitles,
    input: &Path,
) -> Result<String> {
    if !subtitles.source.is_file() {
        bail!(
            "subtitle source is unavailable: {}",
            subtitles.source.display()
        );
    }

    let output = workspace.video().join("with-subtitles.mp4");
    let mut arguments = vec![
        OsString::from("-hide_banner"),
        OsString::from("-y"),
        OsString::from("-i"),
        input.as_os_str().to_owned(),
    ];

    match subtitles.mode {
        SubtitleMode::Burn => {
            arguments.extend([
                OsString::from("-vf"),
                OsString::from(format!(
                    "subtitles=filename='{}'",
                    escape_subtitle_filter_path(&subtitles.source)
                )),
                OsString::from("-c:a"),
                OsString::from("copy"),
            ]);
        }
        SubtitleMode::Mux => {
            arguments.extend([
                OsString::from("-i"),
                subtitles.source.as_os_str().to_owned(),
                OsString::from("-map"),
                OsString::from("0:v:0"),
                OsString::from("-map"),
                OsString::from("0:a:0?"),
                OsString::from("-map"),
                OsString::from("1:0"),
                OsString::from("-c:v"),
                OsString::from("copy"),
                OsString::from("-c:a"),
                OsString::from("copy"),
                OsString::from("-c:s"),
                OsString::from("mov_text"),
            ]);
        }
    }
    arguments.push(output.as_os_str().to_owned());
    command::run_logged("ffmpeg", arguments, &workspace.stage_log("subtitles"))?;
    Ok(output.display().to_string())
}

fn handoff_to_renderflow(
    workspace: &RunWorkspace,
    renderflow: &RenderflowHandoff,
    input: &Path,
) -> Result<String> {
    fs::create_dir_all(workspace.renderflow())?;
    let arguments = renderflow
        .arguments
        .iter()
        .map(|argument| {
            expand_argument(
                argument,
                input,
                &workspace.renderflow(),
                "",
                &workspace.root,
            )
        })
        .collect::<Vec<_>>();
    command::run_logged(
        &renderflow.command,
        arguments,
        &workspace.stage_log("renderflow"),
    )?;
    Ok(workspace.renderflow().display().to_string())
}

fn snapshot_pipeline_inputs(
    pipeline: &mut Pipeline,
    original_pipeline_path: &Path,
    workspace: &RunWorkspace,
) -> Result<()> {
    let Some(subtitles) = pipeline
        .subtitles
        .as_mut()
        .filter(|subtitles| subtitles.enabled)
    else {
        return Ok(());
    };

    let original_directory = original_pipeline_path
        .parent()
        .context("pipeline path does not have a parent directory")?;
    let source = if subtitles.source.is_absolute() {
        subtitles.source.clone()
    } else {
        original_directory.join(&subtitles.source)
    };
    if !source.is_file() {
        bail!("subtitle source does not exist: {}", source.display());
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("srt");
    let snapshot = workspace.subtitles().join(format!("source.{extension}"));
    fs::copy(&source, &snapshot).with_context(|| {
        format!(
            "failed to snapshot subtitles from {} to {}",
            source.display(),
            snapshot.display()
        )
    })?;
    subtitles.source = snapshot
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", snapshot.display()))?;
    Ok(())
}

fn image_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = WalkDir::new(directory)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn sha256_file(path: &Path) -> Result<String> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn escape_subtitle_filter_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{compact_gwr_batch_records, png_dimensions};

    #[test]
    fn reads_png_dimensions() {
        let directory = tempdir().expect("temporary directory should be created");
        let path = directory.path().join("frame.png");
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(1920_u32.to_be_bytes());
        bytes.extend(1080_u32.to_be_bytes());
        fs::write(&path, bytes).expect("PNG fixture should be written");

        assert_eq!(
            png_dimensions(&path).expect("dimensions should parse"),
            (1920, 1080)
        );
    }

    #[test]
    fn compacts_gwr_batch_json_output() {
        let output = br#"[{"input":"/frames/frame-00000002.png","meta":{"applied":true,"qualityStatus":"clean"}},{"input":"/frames/frame-00000001.png","meta":{"applied":false,"qualityStatus":"review"}}]"#;
        let records = compact_gwr_batch_records(output);

        assert_eq!(records.len(), 2);
        assert!(
            records
                .iter()
                .any(|record| record.contains("frame-00000001.png"))
        );
        assert!(
            records
                .iter()
                .any(|record| record.contains("frame-00000002.png"))
        );
    }
}
