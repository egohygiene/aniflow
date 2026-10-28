//! Source extraction contracts authored for deferred #64 execution.
//!
//! Fake executable bytes make accidental plan-time launch observable. These
//! cases cover source policy and existing v3 authority, not native extraction,
//! color accuracy, perceptual quality or backend qualification.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use aniflow::frame_extraction::{self, FrameExtractionConfiguration,
    FrameExtractionOperation, FrameExtractionRequest};
use aniflow::{CancellationToken, ComponentInventory, PipelineInputBinding,
    PipelinePlanningContext, PipelineV3Configuration, PipelineV3Plan,
    PipelineV3RunRequest, PipelineV3Workspace, ProviderRegistration, ProviderRegistry};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn caller_settings() -> Value {
    json!({
        "schema":"aniflow.frame-extraction-configuration/v1",
        "ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},
        "ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},
        "source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]}
    })
}

fn configuration() -> FrameExtractionConfiguration {
    FrameExtractionConfiguration::from_json_slice(&serde_json::to_vec(&caller_settings()).unwrap()).unwrap()
}

#[test]
fn explicit_stream_selection_and_closed_native_policy_refuse_ambiguous_or_conflicting_intent() {
    let original = serde_json::to_value(configuration()).unwrap();
    for (pointer, replacement) in [
        ("/ffmpeg/version", json!("8.0.2")),
        ("/ffmpeg/executable", json!("ffmpeg")),
        ("/ffprobe/executable", json!("/synthetic/ffmpeg")),
        ("/ffmpeg/sha256", json!("unknown")),
        ("/source_selection/video_stream", Value::Null),
        ("/source_selection/audio_stream", json!(1)),
        ("/source_selection/no_audio", json!(false)),
        ("/source_selection/discard_streams", json!([0])),
        ("/source_selection/discard_streams", json!([2,2])),
        ("/maximum_frames", json!(100001)),
        ("/maximum_capture_bytes", json!(65535)),
        ("/maximum_pixels", json!(33554433)),
        ("/maximum_frame_bytes", json!(67108865)),
        ("/maximum_working_bytes", json!(1048575)),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(FrameExtractionConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err(), "{pointer}");
    }
    let mut absent = caller_settings();
    absent.as_object_mut().unwrap().remove("source_selection");
    assert!(FrameExtractionConfiguration::from_json_slice(&serde_json::to_vec(&absent).unwrap()).is_err());
    for field in ["arguments", "filters", "normalize_timestamps", "autorotate", "download"] {
        let mut value = original.clone();
        value[field] = json!(true);
        assert!(FrameExtractionConfiguration::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err(), "{field}");
    }
    let mut selected_audio = original;
    selected_audio["source_selection"] = json!({"video_stream":0,"audio_stream":1,"no_audio":false,"discard_streams":[2]});
    assert!(FrameExtractionConfiguration::from_json_slice(&serde_json::to_vec(&selected_audio).unwrap()).is_ok());
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    request: FrameExtractionRequest,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap().join("extraction 音 with spaces");
    fs::create_dir(&root).unwrap();
    let source_video = root.join("source video.bin");
    fs::write(&source_video, b"synthetic source bytes; no observed media support").unwrap();
    let mut settings = configuration();
    for (name, pin) in [("ffmpeg", &mut settings.ffmpeg), ("ffprobe", &mut settings.ffprobe)] {
        pin.executable = root.join(format!("forbidden {name}"));
        let marker = root.join("launched");
        let literal = format!("'{}'", marker.display().to_string().replace('\'', "'\\''"));
        let script = format!("#!/bin/sh\nprintf launched > {literal}\nexit 99\n");
        fs::write(&pin.executable, &script).unwrap();
        fs::set_permissions(&pin.executable, fs::Permissions::from_mode(0o700)).unwrap();
        pin.sha256 = sha256(script.as_bytes());
    }
    let provider = PathBuf::from(env!("CARGO_BIN_EXE_aniflow")).canonicalize().unwrap();
    let request = FrameExtractionRequest::new(source_video, settings, provider);
    Fixture { _directory:directory, root, request }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    WalkDir::new(root).into_iter().map(|entry| entry.unwrap()).map(|entry| {
        let path = entry.path().strip_prefix(root).unwrap().to_path_buf();
        let digest = if entry.file_type().is_file() { sha256(&fs::read(entry.path()).unwrap()) }
            else { "directory".to_owned() };
        (path, digest)
    }).collect()
}

fn operation(fixture: &Fixture) -> FrameExtractionOperation {
    let source = fs::read(&fixture.request.source_video).unwrap();
    FrameExtractionOperation {
        schema:"aniflow.frame-extraction-operation/v1".to_owned(),
        settings:fixture.request.configuration.clone(), source_sha256:sha256(&source),
        source_byte_count:source.len() as u64,
    }
}

fn registry(fixture: &Fixture, plan: &PipelineV3Plan) -> ProviderRegistry {
    let operation = operation(fixture);
    let lock = &plan.payload.stages[0].provider_lock.payload;
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderRegistration::new(&lock.registration_id, operation.manifest().unwrap(),
        operation.provider_configuration().unwrap(), fixture.request.provider_executable.clone(), &lock.implementation.id,
        ComponentInventory { tools:lock.tools.clone(), codecs:lock.codecs.clone(), models:lock.models.clone() },
    ).unwrap()).unwrap();
    registry
}

#[test]
fn plan_hashes_source_and_pins_without_native_probe_or_fabricated_frame_geometry() {
    let fixture = fixture();
    let before = snapshot(&fixture.root);
    let plan = frame_extraction::plan(&fixture.request, &CancellationToken::default()).unwrap();
    assert_eq!(snapshot(&fixture.root), before);
    assert!(!fixture.root.join("launched").exists());
    assert_eq!(plan.payload.inputs.len(), 1);
    assert_eq!(plan.payload.inputs[0].id, "source_video");
    assert_eq!(plan.payload.inputs[0].content_sha256, operation(&fixture).source_sha256);
    assert_eq!(plan.payload.stages.len(), 1);
    let stage = &plan.payload.stages[0];
    assert_eq!(stage.id, "extract_frames");
    assert_eq!(stage.capability.id, "aniflow/extract-frames");
    assert!(stage.depends_on.is_empty());
    assert_eq!(stage.provider_lock.payload.tools.len(), 2);
    assert_eq!(stage.provider_lock.payload.effective_configuration_sha256,
        operation(&fixture).provider_configuration().unwrap().effective_configuration_sha256);
    for id in ["extracted_frames", "extracted_index", "extraction_evidence"] {
        assert!(stage.outputs.iter().flat_map(|binding| &binding.artifacts).any(|artifact| artifact.id == id));
        let output = plan.payload.outputs.iter().find(|output| output.artifact == id).unwrap();
        assert_eq!(output.required_validations.len(), 1);
        assert!(stage.validations.iter().any(|validation| validation.id == output.required_validations[0]
            && validation.artifact == id && validation.contract == aniflow::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1));
    }
    let effective = operation(&fixture).provider_configuration().unwrap();
    for pending in ["width", "height", "frame_count", "temporal_inspection"] {
        assert!(!effective.values.contains_key(pending));
    }
    assert_eq!(plan, frame_extraction::plan(&fixture.request, &CancellationToken::default()).unwrap());
}

#[test]
fn defaulted_and_materialized_settings_have_the_same_compatibility_identity() {
    let fixture = fixture();
    let original = operation(&fixture);
    let explicit = FrameExtractionConfiguration::from_json_slice(
        &serde_json::to_vec(&original.settings).unwrap()).unwrap();
    assert_eq!(explicit.maximum_pixels, 8_400_000);
    let mut materialized = original.clone();
    materialized.settings = explicit;
    assert_eq!(original.provider_configuration().unwrap(), materialized.provider_configuration().unwrap());
    let mut guessed = serde_json::to_value(original).unwrap();
    guessed["width"] = json!(3840);
    assert!(serde_json::from_value::<FrameExtractionOperation>(guessed).is_err());
}

#[test]
fn changed_source_selected_stream_bounds_or_replaced_pins_invalidate_retained_intent() {
    for changed in ["source", "stream", "bound", "tool"] {
        let mut fixture = fixture();
        let original = frame_extraction::plan(&fixture.request, &CancellationToken::default()).unwrap();
        let workspace = PipelineV3Workspace::create_at(fixture.root.join("retained run")).unwrap();
        workspace.publish_plan_bytes(&original.canonical_json_bytes().unwrap()).unwrap();
        match changed {
            "source" => fs::write(&fixture.request.source_video, b"different source identity").unwrap(),
            "stream" => fixture.request.configuration.source_selection.video_stream = Some(3),
            "bound" => fixture.request.configuration.maximum_frames = 10,
            "tool" => {
                let pin = &mut fixture.request.configuration.ffmpeg;
                let mut bytes = fs::read(&pin.executable).unwrap();
                bytes.extend_from_slice(b"# changed pinned build\n");
                fs::write(&pin.executable, &bytes).unwrap();
                pin.sha256 = sha256(&bytes);
            }
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.root);
        let fresh = frame_extraction::plan(&fixture.request, &CancellationToken::default()).unwrap();
        assert_ne!(original.plan_sha256, fresh.plan_sha256, "{changed}");
        let mut progressed = false;
        let error = frame_extraction::resume(fixture.request.clone(), workspace.root(),
            &CancellationToken::default(), |_| progressed = true).unwrap_err();
        assert!(error.message().contains("changed"), "{changed}: {error}");
        assert!(!progressed);
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
    }
}

#[test]
fn missing_or_stale_native_pins_and_source_aliases_refuse_before_any_workspace_write() {
    for changed in ["tool_bytes", "tool_missing", "source_alias", "source_limit"] {
        let mut fixture = fixture();
        match changed {
            "tool_bytes" => fs::write(&fixture.request.configuration.ffmpeg.executable, b"unapproved tool bytes").unwrap(),
            "tool_missing" => fs::remove_file(&fixture.request.configuration.ffprobe.executable).unwrap(),
            "source_alias" => fixture.request.source_video = fixture.request.configuration.ffmpeg.executable.clone(),
            "source_limit" => fixture.request.configuration.maximum_source_bytes = 1,
            _ => unreachable!(),
        }
        let before = snapshot(&fixture.root);
        assert!(frame_extraction::plan(&fixture.request, &CancellationToken::default()).is_err(), "{changed}");
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("launched").exists());
    }
}

fn authored(plan: &PipelineV3Plan) -> PipelineV3Configuration {
    let stage = &plan.payload.stages[0];
    let inputs: Vec<_> = plan.payload.inputs.iter().map(|input| json!({
        "id":input.id,"artifact_type":input.artifact_type,"artifact_role":input.artifact_role,"stream_role":input.stream_role
    })).collect();
    let outputs: Vec<_> = stage.outputs.iter().map(|binding| json!({
        "port":binding.port,"artifacts":binding.artifacts.iter().map(|artifact| json!({
            "id":artifact.id,"relative_path":artifact.relative_path,"kind":artifact.kind
        })).collect::<Vec<_>>()
    })).collect();
    PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.pipeline/v3","name":plan.payload.pipeline_name,"inputs":inputs,
        "stages":[{"id":stage.id,"depends_on":stage.depends_on,"capability":stage.capability_requirement,
            "provider":stage.provider_selection,"inputs":stage.inputs,"outputs":outputs,"validations":stage.validations}],
        "outputs":plan.payload.outputs
    })).unwrap()).unwrap()
}

#[test]
fn generic_v3_cannot_redirect_outputs_omit_evidence_or_change_fixed_stage_authority() {
    for changed in ["destination", "evidence", "stage", "validation"] {
        let fixture = fixture();
        let original = frame_extraction::plan(&fixture.request, &CancellationToken::default()).unwrap();
        let registry = registry(&fixture, &original);
        let mut pipeline = authored(&original);
        let bindings = vec![PipelineInputBinding::new("source_video", &fixture.request.source_video)];
        let context = PipelinePlanningContext { host:original.payload.policy.host.clone(),
            allowed_side_effects:original.payload.policy.allowed_side_effects.clone(), offline:original.payload.policy.offline };
        // Prove reconstruction preserves the original semantic intent before
        // intentionally changing one boundary. Human prose is not hashed.
        assert_eq!(pipeline.configuration_sha256().unwrap(), original.payload.configuration_sha256);
        assert_eq!(aniflow::resolve_pipeline_v3(&pipeline, &bindings, &registry, &context).unwrap(), original);
        match changed {
            "destination" => pipeline.stages[0].outputs[0].artifacts[0].relative_path = "artifacts/other/frames".to_owned(),
            "evidence" => pipeline.outputs.retain(|output| output.artifact != "extraction_evidence"),
            "stage" => pipeline.stages[0].id = "other_extract".to_owned(),
            "validation" => {
                let old = pipeline.stages[0].validations[0].id.clone();
                pipeline.stages[0].validations[0].id = "other_integrity".to_owned();
                for output in &mut pipeline.outputs {
                    for id in &mut output.required_validations {
                        if *id == old { *id = "other_integrity".to_owned(); }
                    }
                }
            }
            _ => unreachable!(),
        }
        // Resolving a coherent graph proves this is not merely a stale digest.
        let altered = aniflow::resolve_pipeline_v3(&pipeline, &bindings, &registry, &context).unwrap();
        let output = fixture.root.join("forbidden runs");
        let before = snapshot(&fixture.root);
        let mut progressed = false;
        let error = aniflow::run_v3_with_progress_and_cancellation(
            PipelineV3RunRequest::new(altered, bindings, registry).with_output_directory(&output),
            &CancellationToken::default(), |_| progressed = true).unwrap_err();
        assert!(error.message().contains("frame extraction"), "{changed}: {error}");
        assert!(!progressed);
        assert!(!output.exists());
        assert_eq!(snapshot(&fixture.root), before);
    }
}

#[test]
fn cancellation_and_output_overlap_do_not_launch_tools_or_create_run_state() {
    let fixture = fixture();
    let cancelled = CancellationToken::default(); cancelled.cancel();
    let before = snapshot(&fixture.root);
    assert!(frame_extraction::plan(&fixture.request, &cancelled).is_err());
    let output = fixture.request.source_video.join("runs");
    let mut progressed = false;
    assert!(frame_extraction::run(fixture.request.clone(), Some(output.clone()),
        &CancellationToken::default(), |_| progressed = true).is_err());
    assert!(!progressed);
    assert!(!output.exists());
    assert_eq!(snapshot(&fixture.root), before);
}
