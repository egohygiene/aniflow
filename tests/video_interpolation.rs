//! Synthetic RIFE planning coverage authored under #114; execution deferred #64.
use std::path::PathBuf;

use aniflow::audio_inspection::AudioToolPin;
use aniflow::temporal::{self, RationalTime};
use aniflow::video_interpolation::*;
use serde_json::Value;

fn configuration() -> RifeConfiguration {
    RifeConfiguration {
        schema: RIFE_CONFIGURATION_SCHEMA_V1.to_owned(),
        enabled: true,
        host: RifeHost {
            operating_system: "macos".to_owned(),
            architecture: "aarch64".to_owned(),
            total_memory_mib: 18_432,
            available_memory_mib: 12_288,
        },
        settings: RifeSettings {
            tool: AudioToolPin {
                executable: PathBuf::from("/synthetic/rife-ncnn-vulkan"),
                version: "2022.10.29".to_owned(),
                sha256: "a".repeat(64),
            },
            model: RifeModelPin {
                directory: PathBuf::from("/synthetic/rife-v4.6"),
                param_sha256: "b".repeat(64), param_bytes: 1024,
                bin_sha256: "c".repeat(64), bin_bytes: 4096,
            },
            preset: RifePreset::Auto, device_id: -1,
            tool_timeout_milliseconds: 120_000,
            maximum_tool_output_bytes: 1_048_576,
            maximum_frame_bytes: 67_108_864,
            maximum_artifact_bytes: 68_719_476_736,
        },
    }
}

fn index(name: &str, width: u32, height: u32) -> RifeFrameIndex {
    let mut value: Value = serde_json::from_slice(&std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal").join(format!("{name}.json")),
    ).unwrap()).unwrap();
    value["inventory"]["streams"][0]["width"] = width.into();
    value["inventory"]["streams"][0]["height"] = height.into();
    let observations = value["observations"].as_array().unwrap().iter().map(|item| {
        (item["stream_index"].as_u64().unwrap() as u32, item["frames"].clone(), item["packets"].clone())
    }).collect::<Vec<_>>();
    let inspection = temporal::from_probe_documents(
        value["source_sha256"].as_str().unwrap(), value["source_size_bytes"].as_u64().unwrap(),
        &serde_json::from_value(value["selection"].clone()).unwrap(), &value["inventory"], &observations,
    ).unwrap();
    RifeFrameIndex {
        schema: RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(),
        frames: (0..inspection.video_timeline().unwrap().frames.len()).map(|ordinal| RifeFrame {
            ordinal: ordinal as u64, filename: format!("frame_{ordinal:08}.png"), sha256: "d".repeat(64),
        }).collect(),
        temporal_inspection: inspection, width, height,
        frame_profile: "rgb8_sdr_bt709_square_unrotated".to_owned(),
        cut_before_ordinals: vec![],
    }
}

#[test]
fn exact_fractional_clock_and_explicit_cut_endpoint_holds() {
    let mut index = index("cfr-30000-1001", 1080, 1920);
    index.cut_before_ordinals = vec![2];
    let plan = plan(&configuration(), &index).unwrap();
    assert_eq!(plan.output_frame_count, 8);
    assert_eq!(plan.output_frame_rate, RationalTime::new(60_000, 1001).unwrap());
    assert_eq!(plan.duration, RationalTime::new(1001, 7500).unwrap());
    assert_eq!(output_frame(&plan, 1).unwrap(), (0, 1, RifeFrameOrigin::InterpolatedMidpoint));
    assert_eq!(output_frame(&plan, 3).unwrap(), (1, 1, RifeFrameOrigin::CutHold));
    assert_eq!(output_frame(&plan, 7).unwrap(), (3, 3, RifeFrameOrigin::EndpointHold));
    assert!(output_frame(&plan, 8).is_err());
    assert!(!plan.native_qualified);
}

#[test]
fn macbook_auto_is_serial_and_refuses_4k_before_execution() {
    for (width, height) in [(1080, 1920), (1920, 1080)] {
        let result = plan(&configuration(), &index("cfr-integer-30", width, height)).unwrap();
        let resources = result.resources.unwrap();
        assert_eq!(resources.preset, RifePreset::Conservative);
        assert_eq!(resources.maximum_active_pairs, 1);
        assert_eq!(resources.threads, "1:1:1");
        assert_eq!(resources.padded_width * resources.padded_height, 2_088_960);
        assert!(!resources.memory_limit_enforced);
    }
    assert!(plan(&configuration(), &index("cfr-integer-30", 2160, 3840)).is_err());
}

#[test]
fn presets_use_memory_headroom_and_never_add_concurrency() {
    let index = index("cfr-integer-30", 3840, 2160);
    let mut config = configuration();
    config.settings.preset = RifePreset::Balanced;
    assert!(plan(&config, &index).is_err());
    config.host.total_memory_mib = 36_864;
    config.host.available_memory_mib = 24_576;
    assert_eq!(plan(&config, &index).unwrap().resources.unwrap().maximum_active_pairs, 1);
    config.host.available_memory_mib = 4096;
    assert!(plan(&config, &index).is_err());
    config.host.operating_system = "linux".to_owned();
    config.host.architecture = "x86_64".to_owned();
    config.host.available_memory_mib = 24_576;
    config.settings.preset = RifePreset::Auto;
    assert_eq!(plan(&config, &index).unwrap().resources.unwrap().preset, RifePreset::Balanced);
}

#[test]
fn disabled_stage_requires_no_native_tools_or_memory_admission() {
    let mut config = configuration();
    config.enabled = false;
    config.host.total_memory_mib = 0;
    config.host.available_memory_mib = 0;
    let plan = plan(&config, &index("cfr-integer-30", 2160, 3840)).unwrap();
    assert_eq!(plan.output_frame_count, plan.source_frame_count);
    assert_eq!(plan.source_frame_rate, plan.output_frame_rate);
    assert!(plan.resources.is_none());
    assert!(output_frame(&plan, 0).is_err());
}

#[test]
fn stale_or_contradictory_index_and_unsupported_profile_are_refused() {
    let config = configuration();
    for name in ["vfr-visible-refusal", "positive-origin", "negative-origin"] {
        assert!(plan(&config, &index(name, 16, 16)).is_err());
    }
    let mut frame_index = index("cfr-integer-30", 16, 16);
    frame_index.cut_before_ordinals = vec![2, 2];
    assert!(plan(&config, &frame_index).is_err());
    frame_index.cut_before_ordinals.clear();
    frame_index.frames[0].filename = "../escape.png".to_owned();
    assert!(plan(&config, &frame_index).is_err());
    frame_index.frames[0].filename = "frame_00000000.png".to_owned();
    frame_index.width = 32;
    assert!(plan(&config, &frame_index).is_err());
}

#[test]
fn configured_tool_model_and_host_changes_change_plan_identity() {
    let index = index("cfr-integer-30", 16, 16);
    let config = configuration();
    let baseline = plan(&config, &index).unwrap();
    let mut changed = config.clone();
    changed.settings.model.bin_sha256 = "e".repeat(64);
    assert_ne!(baseline.configuration_sha256, plan(&changed, &index).unwrap().configuration_sha256);
    let mut changed_index = index;
    changed_index.frames[1].sha256 = "e".repeat(64);
    assert_ne!(baseline.frame_index_sha256, plan(&config, &changed_index).unwrap().frame_index_sha256);
    let provider = RifeProviderConfiguration { schema: RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(), configuration: config };
    provider.provider_configuration().unwrap().validate().unwrap();
    provider.manifest().unwrap().validate().unwrap();
}

#[test]
fn single_frame_has_an_explicit_same_duration_endpoint_hold() {
    let mut source = index("cfr-integer-30", 16, 16);
    source.frames.truncate(1);
    let timeline = &mut source.temporal_inspection.timelines[0];
    timeline.frames.truncate(1);
    timeline.packets.truncate(1);
    timeline.source_time.as_mut().unwrap().end = timeline.frames[0].source_time.end;
    source.temporal_inspection.streams[0].duration_ticks = Some(1);
    source.temporal_inspection.processing.frame_count = Some(1);
    let plan = plan(&configuration(), &source).unwrap();
    assert_eq!(plan.output_frame_count, 2);
    assert_eq!(plan.duration, RationalTime::new(1, 30).unwrap());
    assert_eq!(output_frame(&plan, 1).unwrap(), (0, 0, RifeFrameOrigin::EndpointHold));
}

#[test]
fn imported_output_evidence_cannot_change_clock_mapping_or_copied_bytes() {
    let config = configuration();
    let source = index("cfr-integer-30", 16, 16);
    let plan = plan(&config, &source).unwrap();
    let frames = (0..plan.output_frame_count).map(|ordinal| {
        let (left, right, origin) = output_frame(&plan, ordinal).unwrap();
        RifeOutputFrame {
            ordinal, filename: format!("frame_{ordinal:08}.png"),
            sha256: if origin == RifeFrameOrigin::InterpolatedMidpoint { "e".repeat(64) } else { source.frames[left as usize].sha256.clone() },
            source_left_ordinal: left, source_right_ordinal: right, origin,
        }
    }).collect();
    let output = RifeOutputIndex { schema: RIFE_OUTPUT_INDEX_SCHEMA_V1.to_owned(), plan, frames };
    output.validate_against(&config, &source).unwrap();
    let mut changed = output.clone();
    changed.frames[0].sha256 = "e".repeat(64);
    assert!(changed.validate_against(&config, &source).is_err());
    changed = output.clone();
    changed.frames[1].source_right_ordinal = 3;
    assert!(changed.validate_against(&config, &source).is_err());
    changed = output;
    changed.plan.output_frame_rate = RationalTime::new(59, 1).unwrap();
    assert!(changed.validate_against(&config, &source).is_err());
}

#[test]
fn machine_cli_plans_without_tools_or_model_files_and_refuses_oversized_mac_input() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let configuration_path = root.join("configuration.json");
    let index_path = root.join("index.json");
    let config = configuration();
    let source = index("cfr-integer-30", 1080, 1920);
    let config_bytes = serde_json::to_vec(&config).unwrap();
    let index_bytes = serde_json::to_vec(&source).unwrap();
    std::fs::write(&configuration_path, &config_bytes).unwrap();
    std::fs::write(&index_path, &index_bytes).unwrap();
    let run = || std::process::Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["video", "interpolation-plan", "--configuration"])
        .arg(&configuration_path).arg("--frame-index").arg(&index_path)
        .args(["--output", "json"]).output().unwrap();
    let output = run();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let envelope: aniflow::MachineEnvelope<RifePlan> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope.command, aniflow::CommandName::VideoInterpolationPlan);
    let aniflow::MachineOutcome::Success { result } = envelope.outcome else { panic!("plan success expected") };
    assert_eq!(result.output_frame_count, 8);
    assert_eq!(result.resources.unwrap().maximum_active_pairs, 1);
    assert_eq!(std::fs::read(&configuration_path).unwrap(), config_bytes);
    assert_eq!(std::fs::read(&index_path).unwrap(), index_bytes);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    std::fs::write(&index_path, serde_json::to_vec(&index("cfr-integer-30", 2160, 3840)).unwrap()).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "video_interpolation_plan");
    assert_eq!(error["status"], "error");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
}
