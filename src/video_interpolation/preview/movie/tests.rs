//! Synthetic cases authored for the deferred qualification pass. Uncompiled/unrun.
use super::*;

fn operation() -> MovieOperation {
    let settings = AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&serde_json::json!({
        "schema":"aniflow.video-assembly-configuration/v1",
        "ffmpeg":{"executable":"/tools/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},
        "ffprobe":{"executable":"/tools/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},
        "timeout_milliseconds":30000,"maximum_capture_bytes":1048576,"maximum_source_bytes":134217728,
        "maximum_output_bytes":16777216,"maximum_frame_bytes":1048576,"maximum_total_frame_bytes":134217728,
        "maximum_pixels":8294400,"maximum_working_bytes":1073741824,"minimum_free_bytes":0,"delivery_crf":18,"preset":"medium"
    })).unwrap()).unwrap();
    let mut op = MovieOperation {
        schema: OPERATION.into(),
        request: MovieRequest {
            schema: REQUEST.into(),
            preview_run_directory: "/work/accepted-preview".into(),
            expected_preview_report_sha256: "c".repeat(64),
            settings,
            provider_executable: "/tools/aniflow".into(),
        },
        source: MovieSource {
            preview_plan_sha256: "d".repeat(64),
            preview_run_manifest_sha256: "e".repeat(64),
            report_path: "/work/accepted-preview/artifacts/report.json".into(),
            report_sha256: "c".repeat(64),
            report_byte_count: 4096,
            frames_directory: "/work/accepted-preview/artifacts/frames".into(),
            frames_sha256: "f".repeat(64),
            frames_byte_count: 200,
            source_index_path: "/work/source/index.json".into(),
            source_index_sha256: "1".repeat(64),
            source_index_byte_count: 8192,
            width: 640,
            height: 360,
            frame_count: 2,
            frame_rate: RationalTime::new(48, 1).unwrap(),
            first_original_output_ordinal: 2,
            original_window: TimeRange::new(
                RationalTime::new(1, 24).unwrap(),
                RationalTime::new(1, 12).unwrap(),
            )
            .unwrap(),
        },
        renderer_sha256: "2".repeat(64),
        renderer_byte_count: 1000,
        tool_byte_counts: [1000, 1000],
        working_bytes: 0,
    };
    op.working_bytes = op.required_working_bytes().unwrap();
    op
}
fn preview() -> (tempfile::TempDir, PreviewReport) {
    let (root, request) = super::super::tests::fixture();
    let index = super::super::tests::original(&request);
    let op = super::super::tests::operation(&request);
    let mut report = super::super::report_template(
        &op,
        &index,
        &("1".repeat(64), 8192),
        &"3".repeat(64),
        &"4".repeat(64),
    )
    .unwrap();
    for f in &mut report.frames {
        f.frame.sha256 = index.frames[f.frame.source_left_ordinal as usize]
            .sha256
            .clone();
    }
    report.validate_against(&index).unwrap();
    (root, report)
}
fn media(op: &MovieOperation) -> MovieMedia {
    MovieMedia {
        sha256: "5".repeat(64),
        byte_count: 100,
        width: op.source.width,
        height: op.source.height,
        frame_count: op.source.frame_count,
        frame_rate: op.source.frame_rate,
        local_interval: TimeRange::new(RationalTime::ZERO, RationalTime::new(1, 24).unwrap())
            .unwrap(),
        video_codec: "h264".into(),
        pixel_format: "yuv420p".into(),
        color_primaries: "bt709".into(),
        color_transfer: "bt709".into(),
        color_matrix: "bt709".into(),
        color_range: "tv".into(),
        sample_aspect_ratio: "1:1".into(),
        faststart: true,
        full_decode_observed: true,
        audio_stream_count: 0,
    }
}
#[test]
fn exact_native_ports_keep_the_viewing_copy_intermediate_and_serial() {
    let op = operation();
    op.validate().unwrap();
    let v = serde_json::to_value(op.manifest().unwrap()).unwrap();
    let cap = &v["capabilities"][0];
    assert_eq!(cap["outputs"][0]["artifact_role"], "intermediate");
    assert_eq!(cap["inputs"].as_array().unwrap().len(), 3);
    assert_eq!(cap["max_concurrency"], 1);
    assert_eq!(cap["requirements"]["compute"]["gpu"], "forbidden");
    assert_eq!(cap["requirements"]["compute"]["network"], "forbidden");
    assert_ne!(
        cap["id"],
        crate::video_assembly::ASSEMBLY_DELIVERY_CAPABILITY_ID
    );
}
#[test]
fn odd_geometry_overlong_clocks_and_wrong_selected_pins_refuse() {
    let mut op = operation();
    op.source.width = 641;
    assert!(op.validate().is_err());
    let mut op = operation();
    op.source.frame_count = 242;
    assert!(op.validate().is_err());
    let mut op = operation();
    op.source.frame_rate = RationalTime::new(1, 10).unwrap();
    op.source.original_window.end = op
        .source
        .original_window
        .start
        .checked_add(RationalTime::new(20, 1).unwrap())
        .unwrap();
    assert!(op.validate().is_err());
    let mut op = operation();
    op.source.report_sha256 = "6".repeat(64);
    assert!(op.validate().is_err());
    let mut op = operation();
    op.source.original_window.end = RationalTime::new(1, 11).unwrap();
    assert!(op.validate().is_err());
}
#[test]
fn native_arguments_preserve_fractional_clock_and_argv_path_boundaries() {
    let mut op = operation();
    op.source.frame_rate = RationalTime::new(60000, 1001).unwrap();
    op.source.original_window.end = op
        .source
        .original_window
        .start
        .checked_add(RationalTime::new(1001, 30000).unwrap())
        .unwrap();
    op.validate().unwrap();
    let args = crate::video_assembly::media::rife_preview::arguments(
        &op,
        &op.request.settings,
        Path::new("/work/a % 音"),
        Path::new("/work/watch me.mp4"),
    )
    .unwrap();
    let args = args.iter().map(|s| s.to_str().unwrap()).collect::<Vec<_>>();
    assert!(args.windows(2).any(|p| p == ["-framerate", "60000/1001"]));
    assert!(
        args.windows(2)
            .any(|p| p == ["-enc_time_base:v", "1001/60000"])
    );
    assert!(args.contains(&"/work/a %% 音/frame-%08d.png"));
    assert_eq!(args.last(), Some(&"/work/watch me.mp4"));
    for forbidden in [
        "-r",
        "-ss",
        "-t",
        "-shortest",
        "-frames:v",
        "-start_at_zero",
    ] {
        assert!(!args.contains(&forbidden));
    }
    assert!(args.contains(&"-an"));
}
#[test]
fn local_zero_mapping_keeps_original_global_ordinals_and_endpoint_origin() {
    let (_root, p) = preview();
    let frames = mapping(&p).unwrap();
    assert_eq!(frames[0].local_ordinal, 0);
    assert_eq!(frames[0].local_presentation.start, RationalTime::ZERO);
    assert_eq!(frames[0].original.frame.ordinal, 2);
    assert_eq!(
        frames[0].original.presentation.start,
        RationalTime::new(1, 24).unwrap()
    );
    assert_eq!(
        frames[1].original.frame.origin,
        crate::video_interpolation::RifeFrameOrigin::EndpointHold
    );
    assert_eq!(frames[1].original.presentation.end, p.window.end);
}
#[test]
fn report_cannot_promote_native_bytes_to_visual_or_publication_approval() {
    let (_root, p) = preview();
    let op = operation();
    let mut report = MovieReport {
        schema: REPORT.into(),
        media: media(&op),
        operation: op,
        provider_lock_sha256: "7".repeat(64),
        frames: mapping(&p).unwrap(),
        clock_policy: CLOCK.into(),
        color_policy: COLOR.into(),
        audio_scope: AUDIO.into(),
        review_scope: SCOPE.into(),
        native_qualified: false,
        creative_approval: "not_performed".into(),
        publication_authority: "none".into(),
    };
    report.validate(&p).unwrap();
    report.native_qualified = true;
    assert!(report.validate(&p).is_err());
    report.native_qualified = false;
    report.creative_approval = "approved".into();
    assert!(report.validate(&p).is_err());
    report.creative_approval = "not_performed".into();
    report.publication_authority = "granted".into();
    assert!(report.validate(&p).is_err());
    report.publication_authority = "none".into();
    report.frames[0].original.frame.ordinal = 0;
    assert!(report.validate(&p).is_err());
}
#[test]
fn media_evidence_refuses_audio_and_rounded_or_truncated_picture_intervals() {
    let op = operation();
    let original = media(&op);
    original.validate(&op).unwrap();
    let mut changed = original.clone();
    changed.audio_stream_count = 1;
    assert!(changed.validate(&op).is_err());
    let mut changed = original.clone();
    changed.frame_count = 1;
    assert!(changed.validate(&op).is_err());
    let mut changed = original.clone();
    changed.frame_rate = RationalTime::new(50, 1).unwrap();
    assert!(changed.validate(&op).is_err());
    let mut changed = original;
    changed.local_interval.start = RationalTime::new(1, 24).unwrap();
    assert!(changed.validate(&op).is_err());
}
#[test]
fn finite_settings_and_complete_working_admission_are_not_silently_relaxed() {
    let op = operation();
    let mut request = op.request.clone();
    request.settings.maximum_source_bytes = 134217729;
    assert!(request.validate().is_err());
    let mut value = serde_json::to_value(&op.request).unwrap();
    value["automatic_review"] = serde_json::json!(true);
    assert!(MovieRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(MovieRequest::from_json_slice(&vec![b' '; 65537]).is_err());
    let mut changed = op.clone();
    changed.working_bytes -= 1;
    assert!(changed.validate().is_err());
    let mut changed = op;
    changed.source.frames_byte_count = u64::MAX;
    assert!(changed.required_working_bytes().is_err());
}
#[test]
fn cancellation_or_loose_reports_cannot_create_or_adopt_native_acceptance() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut request = operation().request;
    request.preview_run_directory = root.clone();
    let token = CancellationToken::default();
    token.cancel();
    assert!(plan(&request, &token).is_err());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::write(root.join("report.json"), b"{\"complete\":true}").unwrap();
    assert!(observe(&root, &CancellationToken::default()).is_err());
    assert!(status(&root).is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
}
