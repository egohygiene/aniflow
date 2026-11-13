use super::*;
fn request() -> Request {
    Request {
        schema: "aniflow.deflicker-comparison-request/v1".into(),
        video_run_directory: "/synthetic/absent-native".into(),
        artifact_id: "delivery".into(),
        first_frame: 3,
        frame_count: 6,
        configuration: None,
        provider_executable: None,
    }
}
fn operation() -> Operation {
    let artifact:FinalVideoArtifact=serde_json::from_value(serde_json::json!({"artifact_id":"delivery","variant":"destination_copy","locator":"/synthetic/a video.mp4","sha256":"a".repeat(64),"byte_count":1000,"width":2,"height":2,"frame_count":30,"frame_rate":{"numerator":30,"denominator":1},"video_interval":{"start":{"numerator":0,"denominator":1},"end":{"numerator":1,"denominator":1}},"video_duration":{"numerator":1,"denominator":1},"video_codec":"h264","pixel_format":"yuv420p","color_primaries":"bt709","color_transfer":"bt709","color_matrix":"bt709","color_range":"tv","sample_aspect_ratio":"1/1","orientation_degrees":0,"audio":null,"metadata":null,"producer_lock_sha256":"b".repeat(64),"stage_invocation_sha256":"c".repeat(64),"stage_checkpoint_sha256":"d".repeat(64),"private_report_sha256":"e".repeat(64)})).unwrap();
    let source:FinalVideoHandoff=serde_json::from_value(serde_json::json!({"schema":"aniflow.final-video-handoff/v1","completion_id":"1".repeat(64),"plan_sha256":"2".repeat(64),"run_manifest_sha256":"3".repeat(64),"delivery_acceptance_sha256":"4".repeat(64),"source_lineage":[],"artifacts":[artifact],"creative_approval":"not_performed","publication_authority":"none","proof_state":"not_asserted"})).unwrap();
    Operation {
        schema: "aniflow.deflicker-comparison-operation/v1".into(),
        request: request(),
        artifact: source.artifacts[0].clone(),
        source,
        renderer_sha256: None,
        working_bytes: 0,
    }
}
fn configuration() -> Configuration {
    Configuration {tools:serde_json::from_value(serde_json::json!({"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)}})).unwrap(),window:5,mode:Mode::Mean,spatial_polish:None}
}
#[test]
fn omitted_selection_requires_no_tool_or_model_identity() {
    let op = operation();
    op.validate().unwrap();
    assert_eq!(admission(&op.request, &op.artifact).unwrap(), (None, 0));
}
#[test]
fn arbitrary_filters_automatic_review_and_composition_changes_are_not_options() {
    let r = request();
    for key in ["filter", "stabilize", "crop", "auto_review", "upload"] {
        let mut v = serde_json::to_value(&r).unwrap();
        v[key] = serde_json::json!(true);
        assert!(Request::from_json_slice(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
#[test]
fn enabled_window_and_complete_excerpt_bounds_are_explicit() {
    let mut r = request();
    r.configuration = Some(configuration());
    assert!(r.validate().is_err());
    r.provider_executable = Some("/synthetic/provider".into());
    r.validate().unwrap();
    r.configuration.as_mut().unwrap().window = 4;
    assert!(r.validate().is_err());
    let mut op = operation();
    op.request.first_frame = 29;
    assert!(op.validate().is_err());
}
#[test]
fn native_filter_order_keeps_closed_original_frame_selection() {
    let mut op = operation();
    op.request.configuration = Some(configuration());
    let before = native::filter(&op, false).unwrap();
    let after = native::filter(&op, true).unwrap();
    assert!(before.starts_with("trim=start_frame=3:end_frame=9,setpts=PTS-STARTPTS,"));
    assert!(!before.contains("deflicker"));
    assert!(after.contains("deflicker=size=5:mode=am"));
    assert!(!after.contains("fps="));
    assert!(!after.contains("crop="));
}
#[test]
fn unknown_partial_roots_and_missing_source_never_create_a_preview_or_review() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let output = root.join("no-comparison");
    assert!(run(&request(), &output, &CancellationToken::new(), |_| {}).is_err());
    assert!(!output.exists());
    std::fs::write(root.join("comparison-intent.json"), b"{}").unwrap();
    assert!(resume(&root, &CancellationToken::new(), |_| {}).is_err());
    assert_eq!(
        std::fs::read(root.join("comparison-intent.json")).unwrap(),
        b"{}"
    );
    assert!(!root.join("review.json").exists());
}

#[test]
fn complete_rgb_decode_requires_every_frame_on_the_exact_original_cadence() {
    let op = operation();
    let headers = "#format: frame checksums\n#version: 2\n#hash: SHA256\n#media_type 0: video\n#codec_id 0: rawvideo\n#dimensions 0: 2x2\n#tb 0: 1/30\n";
    let rows = (0..6)
        .map(|n| format!("0, {n}, {n}, 1, 12, {}\n", "a".repeat(64)))
        .collect::<String>();
    let valid = format!("{headers}{rows}");
    assert_eq!(
        native::parse_hashes(valid.as_bytes(), &op).unwrap().len(),
        6
    );
    assert!(
        native::parse_hashes(
            valid.replace("0, 2, 2, 1, 12", "0, 3, 3, 1, 12").as_bytes(),
            &op
        )
        .is_err()
    );
    let mut fewer = valid.lines().collect::<Vec<_>>();
    fewer.pop();
    assert!(native::parse_hashes(fewer.join("\n").as_bytes(), &op).is_err());
}

fn polish() -> crate::spatial_polish::Profile {
    crate::spatial_polish::Profile::from_json_slice(
        br#"{
        "schema":"aniflow.spatial-polish/v1",
        "denoise":{"strength_hundredths":120,"patch_odd":3,"search_odd":7},
        "deband":{"threshold_thousandths":20,"radius":16},
        "sharpen":{"amount_hundredths":50,"kernel_odd":5}
    }"#,
    )
    .unwrap()
}

#[test]
fn omitted_polish_preserves_legacy_configuration_and_exact_filter() {
    let mut op = operation();
    op.request.configuration = Some(configuration());
    let old = serde_json::to_value(op.request.configuration.as_ref().unwrap()).unwrap();
    assert!(old.get("spatial_polish").is_none());
    let decoded: Configuration = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), old);
    assert_eq!(
        native::filter(&op, true).unwrap(),
        "trim=start_frame=3:end_frame=9,setpts=PTS-STARTPTS,deflicker=size=5:mode=am,scale=in_range=tv:out_range=full:in_color_matrix=bt709:out_color_matrix=bt709,format=pix_fmts=rgb24,setsar=1"
    );
}

#[test]
fn spatial_polish_is_after_deflicker_and_never_changes_the_control() {
    let mut op = operation();
    op.request.configuration = Some(configuration());
    op.request.provider_executable = Some("/synthetic/provider".into());
    let control = native::filter(&op, false).unwrap();
    let old_context = op.context().unwrap();
    let profile = polish();
    let expected = profile.filters().unwrap().join(",");
    op.request.configuration.as_mut().unwrap().spatial_polish = Some(profile);
    op.request.validate().unwrap();
    let after = native::filter(&op, true).unwrap();
    assert!(after.contains(&format!("deflicker=size=5:mode=am,{expected},scale=")));
    assert_eq!(native::filter(&op, false).unwrap(), control);
    assert_ne!(op.context().unwrap(), old_context);
    op.request
        .configuration
        .as_mut()
        .unwrap()
        .spatial_polish
        .as_mut()
        .unwrap()
        .sharpen
        .as_mut()
        .unwrap()
        .amount_hundredths = 0;
    assert!(op.request.validate().is_err());
    assert!(native::filter(&op, true).is_err());
}

#[test]
fn existing_review_guard_rejects_a_declaration_from_the_prior_profile() {
    let mut op = operation();
    op.request.configuration = Some(configuration());
    let context = op.context().unwrap();
    let report = Report {
        schema: "aniflow.deflicker-comparison-report/v1".into(),
        operation: op.clone(),
        provider_lock_sha256: "a".repeat(64),
        frames: Vec::new(),
        comparison_sha256: "b".repeat(64),
        visual_approval: "not_performed".into(),
        runtime_qualified: false,
        audio_scope: "silent_visual_excerpt; original_audio_unchanged_unassessed".into(),
    };
    let mut plan = Plan {
        schema: "aniflow.deflicker-comparison-plan/v1".into(),
        operation: op,
        context_sha256: context.clone(),
        native_plan: None,
        bypassed: false,
    };
    // A synthetic rejection tests the binding only; no actual review is created.
    let declaration = ManualReview {
        schema: "aniflow.manual-review/v1".into(),
        context_sha256: context,
        subject_sha256: report.comparison_sha256.clone(),
        decision: crate::music_video_recipe::manual::ReviewDecision::Reject,
        reviewer: "synthetic-only".into(),
        notes: "Synthetic binding fixture; no media review performed".into(),
    };
    runtime::require_review(&plan, &report, &declaration).unwrap();
    plan.operation
        .request
        .configuration
        .as_mut()
        .unwrap()
        .spatial_polish = Some(polish());
    plan.context_sha256 = plan.operation.context().unwrap();
    assert!(runtime::require_review(&plan, &report, &declaration).is_err());
}
