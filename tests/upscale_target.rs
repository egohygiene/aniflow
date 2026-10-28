//! Pure target-policy regressions authored for deferred #64 execution.
//!
//! Geometry here is synthetic declaration data, not native probe evidence or
//! perceptual quality. Native rotation/SAR/color admission remains separate.
use aniflow::upscale_target::{
    self, UpscaleTargetDecision, UpscaleTargetInput, UpscaleTargetMode,
    UpscaleTargetPolicy, UpscaleTargetStatus,
};
use aniflow::upscayl_recovery::UpscaylRecoveryConfiguration;
use serde_json::json;

fn input(width: u32, height: u32) -> UpscaleTargetInput {
    UpscaleTargetInput {
        source_sha256:"a".repeat(64), source_byte_count:128,
        frame_index_sha256:"b".repeat(64), frames_sha256:"c".repeat(64),
        frames_byte_count:4096, frame_count:12, width, height,
        frame_profile:"rgb8_sdr_bt709_square_unrotated".to_owned(),
    }
}

fn policy(width: u32, height: u32, mode: UpscaleTargetMode) -> UpscaleTargetPolicy {
    UpscaleTargetPolicy { schema:"aniflow.upscale-target-policy/v1".to_owned(), width, height, mode, force_reprocess:false }
}

fn neural_template() -> UpscaylRecoveryConfiguration {
    UpscaylRecoveryConfiguration::from_json_slice(&serde_json::to_vec(&json!({
        "schema":"aniflow.upscayl-recovery-configuration/v1",
        "import":{"schema":"aniflow.frame-import-configuration/v1",
            "ffmpeg":{"executable":"/absent/ffmpeg","version":"8.0.1","sha256":"d".repeat(64)}},
        "upscayl":{"executable":"/absent/upscayl","version":"1.0.0","sha256":"e".repeat(64)},
        "model":{"name":"fixture_model","native_scale":4,
            "param":{"path":"/absent/model.param","sha256":"f".repeat(64)},
            "weights":{"path":"/absent/model.bin","sha256":"1".repeat(64)}},
        "output_scale":4
    })).unwrap()).unwrap()
}

#[test]
fn portrait_and_landscape_uhd_select_exact_integer_output_without_changing_model_native_scale() {
    for (width, height, target_width, target_height) in [(1920,1080,3840,2160),(1080,1920,2160,3840)] {
        let input = input(width,height);
        let policy = policy(target_width,target_height,UpscaleTargetMode::Exact);
        let decision = upscale_target::decide(&input,Some(&policy)).unwrap();
        assert_eq!(decision.status,UpscaleTargetStatus::Execute);
        assert_eq!(decision.output_scale,Some(2));
        assert_eq!((decision.effective_width,decision.effective_height),(target_width,target_height));
        assert_eq!(decision.input,input);
        assert!(decision.unchanged_input_sha256.is_none());
        let template = neural_template();
        let resolved = decision.resolved_configuration(&template).unwrap().unwrap();
        assert_eq!(resolved.output_scale,2);
        assert_eq!(resolved.model.native_scale,4);
        let mut expected = template.clone(); expected.output_scale = 2;
        assert_eq!(resolved,expected,"only requested output scale may be replaced");
        let resources = decision.resources(&template).unwrap();
        assert_eq!(resources.effective_pixels_per_frame,8_294_400);
        assert_eq!(resources.native_intermediate_pixels_per_frame,Some(33_177_600));
        assert_eq!(resources.effective_rgb24_bytes_total,8_294_400 * 3 * 12);
        assert!(!resources.memory_limit_enforced);
    }
}

#[test]
fn exact_and_above_target_preserve_current_input_without_neural_files_or_downscaling() {
    let target = policy(3840,2160,UpscaleTargetMode::Exact);
    for (width,height,status) in [(3840,2160,UpscaleTargetStatus::AlreadyTarget),(7680,4320,UpscaleTargetStatus::AboveTarget)] {
        let input = input(width,height);
        let decision = upscale_target::decide(&input,Some(&target)).unwrap();
        assert_eq!(decision.status,status);
        assert_eq!((decision.effective_width,decision.effective_height),(width,height));
        assert_eq!(decision.unchanged_input_sha256.as_deref(),Some(decision.input_sha256.as_str()));
        assert!(decision.output_scale.is_none());
        let template = neural_template();
        assert!(decision.resolved_configuration(&template).unwrap().is_none());
        let resources = decision.resources(&template).unwrap();
        assert!(resources.native_intermediate_pixels_per_frame.is_none());
        assert!(resources.maximum_output_png_bytes.is_none());
        assert!(resources.maximum_working_bytes.is_none());
        assert!(!resources.memory_limit_enforced);
    }
    let disabled = upscale_target::decide(&input(1920,1080),None).unwrap();
    assert_eq!(disabled.status,UpscaleTargetStatus::Disabled);
    assert!(disabled.policy.is_none());
    assert!(disabled.policy_sha256.is_none());
    assert_eq!(disabled.unchanged_input_sha256,Some(disabled.input_sha256));
}

#[test]
fn exact_refuses_fractional_targets_while_minimum_reports_smallest_supported_overshoot() {
    let input = input(1280,720);
    assert!(upscale_target::decide(&input,Some(&policy(1920,1080,UpscaleTargetMode::Exact))).is_err());
    let minimum = upscale_target::decide(&input,Some(&policy(1920,1080,UpscaleTargetMode::Minimum))).unwrap();
    assert_eq!(minimum.output_scale,Some(2));
    assert_eq!((minimum.effective_width,minimum.effective_height),(2560,1440));
    for (width,height,scale) in [(3840,2160,3),(5120,2880,4)] {
        let exact = upscale_target::decide(&input,Some(&policy(width,height,UpscaleTargetMode::Exact))).unwrap();
        assert_eq!(exact.output_scale,Some(scale));
        assert_eq!((exact.effective_width,exact.effective_height),(width,height));
    }
    assert!(upscale_target::decide(&input,Some(&policy(7680,4320,UpscaleTargetMode::Minimum))).is_err());
}

#[test]
fn both_oriented_dimensions_and_closed_frame_profile_are_required() {
    let target = policy(3840,2160,UpscaleTargetMode::Minimum);
    for (width,height) in [(3840,1080),(7680,1080),(2160,3840),(1920,1200)] {
        assert!(upscale_target::decide(&input(width,height),Some(&target)).is_err(),"{width}x{height}");
    }
    for profile in ["rgb8_sdr_bt709_square_rotated_90","rgb8_sdr_bt709_non_square_unrotated","unknown"] {
        let mut input = input(1920,1080); input.frame_profile = profile.to_owned();
        assert!(upscale_target::decide(&input,Some(&target)).is_err(),"{profile}");
    }
    let mut input = input(1920,1080); input.width = 0;
    assert!(upscale_target::decide(&input,Some(&target)).is_err());
}

#[test]
fn force_is_explicit_and_cannot_invent_one_times_processing_or_hidden_downscale() {
    let input = input(3840,2160);
    let mut target = policy(3840,2160,UpscaleTargetMode::Exact); target.force_reprocess = true;
    assert!(upscale_target::decide(&input,Some(&target)).is_err());
    target.mode = UpscaleTargetMode::Minimum;
    let forced = upscale_target::decide(&input,Some(&target)).unwrap();
    assert_eq!(forced.status,UpscaleTargetStatus::Execute);
    assert_eq!(forced.output_scale,Some(2));
    assert_eq!((forced.effective_width,forced.effective_height),(7680,4320));
    assert!(forced.unchanged_input_sha256.is_none());
}

#[test]
fn policy_is_closed_and_decisions_bind_changed_inputs_and_configuration() {
    let target = policy(3840,2160,UpscaleTargetMode::Exact);
    let value = serde_json::to_value(&target).unwrap();
    for field in ["fit","crop","letterbox","filename","quality","fallback"] {
        let mut candidate = value.clone(); candidate[field] = json!(true);
        assert!(UpscaleTargetPolicy::from_json_slice(&serde_json::to_vec(&candidate).unwrap()).is_err(),"{field}");
    }
    let input = input(1920,1080);
    let decision = upscale_target::decide(&input,Some(&target)).unwrap();
    decision.validate().unwrap();
    let mut changed = input.clone(); changed.frames_sha256 = "2".repeat(64);
    let replacement = upscale_target::decide(&changed,Some(&target)).unwrap();
    assert_ne!(decision.input_sha256,replacement.input_sha256);
    let mut alternate = target.clone(); alternate.mode = UpscaleTargetMode::Minimum;
    assert_ne!(decision.policy_sha256,upscale_target::decide(&input,Some(&alternate)).unwrap().policy_sha256);
    let mut forged = serde_json::to_value(&decision).unwrap(); forged["effective_width"] = json!(7680);
    assert!(serde_json::from_value::<UpscaleTargetDecision>(forged).unwrap().validate().is_err());
}
