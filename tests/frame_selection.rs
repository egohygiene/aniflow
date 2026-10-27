//! Synthetic frame-selection preview coverage for #36; execution deferred to #64.
//!
//! These fixtures are declared clocks and frame identities, not decoded media.
use std::process::Command;

use aniflow::frame_selection::{InclusiveFrameRange, plan_from_rife_index};
use aniflow::temporal::{self, RationalTime, StreamSelection};
use aniflow::video_interpolation::{RIFE_FRAME_INDEX_SCHEMA_V1, RifeFrame, RifeFrameIndex};
use serde_json::{Value, json};

/// Produce a bounded synthetic fractional-rate source with explicit frame names.
fn frame_index(frame_count: u64) -> RifeFrameIndex {
    let frames: Vec<Value> = (0..frame_count)
        .map(|ordinal| {
            json!({
                "stream_index": 7,
                "pts": ordinal * 1001,
                "pkt_dts": ordinal * 1001,
                "pkt_duration": 1001,
                "key_frame": if ordinal == 0 { 1 } else { 0 }
            })
        })
        .collect();
    let packets: Vec<Value> = (0..frame_count)
        .map(|ordinal| {
            json!({
                "stream_index": 7,
                "pts": ordinal * 1001,
                "dts": ordinal * 1001,
                "duration": 1001
            })
        })
        .collect();
    let inventory = json!({
        "streams": [{
            "index": 7,
            "codec_type": "video",
            "codec_name": "h264",
            "time_base": "1/30000",
            "start_pts": 0,
            "duration_ts": frame_count * 1001,
            "r_frame_rate": "30000/1001",
            "avg_frame_rate": "30000/1001",
            "width": 16,
            "height": 16,
            "pix_fmt": "yuv420p",
            "disposition": { "attached_pic": 0 }
        }]
    });
    let inspection = temporal::from_probe_documents(
        &"a".repeat(64),
        64,
        &StreamSelection::default(),
        &inventory,
        &[(7, json!({ "frames": frames }), json!({ "packets": packets }))],
    )
    .expect("synthetic exact clock should be admitted");
    RifeFrameIndex {
        schema: RIFE_FRAME_INDEX_SCHEMA_V1.to_owned(),
        temporal_inspection: inspection,
        width: 16,
        height: 16,
        frame_profile: "rgb8_sdr_bt709_square_unrotated".to_owned(),
        frames: (0..frame_count)
            .map(|ordinal| RifeFrame {
                ordinal,
                filename: format!("frame_{ordinal:08}.png"),
                sha256: "b".repeat(64),
            })
            .collect(),
        cut_before_ordinals: Vec::new(),
    }
}

fn range(first: u64, last: u64) -> InclusiveFrameRange {
    InclusiveFrameRange { first, last }
}

#[test]
fn two_299_frame_ranges_keep_exact_spans_and_the_untouched_gap() {
    let index = frame_index(1000);
    let plan = plan_from_rife_index(&index, &[range(1, 299), range(600, 898)]).unwrap();
    assert_eq!(plan.schema, "aniflow.frame-selection-plan/v1");
    assert_eq!(plan.source_frame_count, 1000);
    assert_eq!(plan.selected_frame_count, 598);
    assert_eq!(plan.unselected_frame_count, 402);
    assert_eq!(plan.ranges.len(), 2);
    let first = &plan.ranges[0];
    assert_eq!((first.first_frame, first.last_frame), (1, 299));
    assert_eq!((first.start_ordinal, first.end_ordinal_exclusive), (0, 299));
    assert_eq!(first.frame_count, 299);
    assert_eq!(first.source_time.start, RationalTime::ZERO);
    assert_eq!(first.source_time.end, RationalTime::new(299 * 1001, 30_000).unwrap());
    let second = &plan.ranges[1];
    assert_eq!((second.first_frame, second.last_frame), (600, 898));
    assert_eq!((second.start_ordinal, second.end_ordinal_exclusive), (599, 898));
    assert_eq!(second.frame_count, 299);
    assert_eq!(second.source_time.start, RationalTime::new(599 * 1001, 30_000).unwrap());
    assert_eq!(second.source_time.end, RationalTime::new(898 * 1001, 30_000).unwrap());
}

#[test]
fn inclusive_endpoints_and_adjacent_ranges_do_not_lose_or_double_count_frames() {
    let index = frame_index(4);
    let plan = plan_from_rife_index(&index, &[range(1, 1), range(2, 3), range(4, 4)]).unwrap();
    assert_eq!(plan.selected_frame_count, 4);
    assert_eq!(plan.unselected_frame_count, 0);
    assert_eq!(plan.ranges[0].frame_count, 1);
    assert_eq!(plan.ranges[2].source_time.end, RationalTime::new(4004, 30_000).unwrap());
    for pair in plan.ranges.windows(2) {
        assert_eq!(pair[0].end_ordinal_exclusive, pair[1].start_ordinal);
        assert_eq!(pair[0].source_time.end, pair[1].source_time.start);
    }
    let single = plan_from_rife_index(&frame_index(1), &[range(1, 1)]).unwrap();
    assert_eq!(single.selected_frame_count, 1);
    assert_eq!(single.unselected_frame_count, 0);
    assert_eq!(single.ranges[0].end_ordinal_exclusive, 1);
}

#[test]
fn sorting_changes_no_intent_and_does_not_mutate_declared_inputs() {
    let index = frame_index(10);
    let input_bytes = serde_json::to_vec(&index).unwrap();
    let unsorted = [range(8, 9), range(1, 2), range(4, 5)];
    let actual = plan_from_rife_index(&index, &unsorted).unwrap();
    let ordered = plan_from_rife_index(&index, &[range(1, 2), range(4, 5), range(8, 9)]).unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), serde_json::to_value(ordered).unwrap());
    assert_eq!(unsorted[0].first, 8);
    assert_eq!(serde_json::to_vec(&index).unwrap(), input_bytes);
}

#[test]
fn invalid_and_overlapping_ranges_fail_instead_of_being_clamped_or_deduplicated() {
    let index = frame_index(1000);
    for ranges in [
        Vec::new(),
        vec![range(0, 1)],
        vec![range(4, 3)],
        vec![range(1, 1001)],
        vec![range(1001, 1001)],
        vec![range(1, u64::MAX)],
        vec![range(u64::MAX, u64::MAX)],
        vec![range(1, 299), range(1, 299)],
        vec![range(1, 299), range(299, 400)],
        vec![range(1, 299), range(20, 30)],
    ] {
        assert!(plan_from_rife_index(&index, &ranges).is_err());
    }
    let many_ranges: Vec<_> = (1..=1025).map(|frame| range(frame, frame)).collect();
    assert!(plan_from_rife_index(&frame_index(1025), &many_ranges).is_err());
}

#[test]
fn external_299_to_240_truncation_and_missing_mapping_are_refused() {
    let mut index = frame_index(299);
    index.frames.truncate(240);
    assert!(plan_from_rife_index(&index, &[range(1, 240)]).is_err());

    let mut index = frame_index(299);
    index.frames.remove(150);
    assert!(plan_from_rife_index(&index, &[range(1, 100)]).is_err());
}

#[test]
fn malformed_mapping_is_rejected_even_when_it_is_outside_selected_frames() {
    let index = frame_index(10);
    let selection = [range(1, 2)];
    let mut duplicate = index.clone();
    duplicate.frames[9].filename = duplicate.frames[8].filename.clone();
    assert!(plan_from_rife_index(&duplicate, &selection).is_err());
    let mut missing = index.clone();
    missing.frames[9].filename.clear();
    assert!(plan_from_rife_index(&missing, &selection).is_err());
    let mut wrong_ordinal = index.clone();
    wrong_ordinal.frames[9].ordinal = 8;
    assert!(plan_from_rife_index(&wrong_ordinal, &selection).is_err());
    let mut unsafe_name = index.clone();
    unsafe_name.frames[9].filename = "../outside.png".to_owned();
    assert!(plan_from_rife_index(&unsafe_name, &selection).is_err());
    let mut invalid_digest = index;
    invalid_digest.frames[9].sha256 = "not-a-digest".to_owned();
    assert!(plan_from_rife_index(&invalid_digest, &selection).is_err());
}

#[test]
fn exact_timing_is_recomputed_instead_of_trusting_saved_support_flags() {
    let mut index = frame_index(10);
    index.temporal_inspection.timelines[0].frames[5].source_time.start = RationalTime::ZERO;
    assert!(index.temporal_inspection.processing.supported);
    assert!(plan_from_rife_index(&index, &[range(1, 2)]).is_err());
}

#[test]
fn plan_identity_binds_source_and_selected_or_untouched_frame_declarations() {
    let index = frame_index(10);
    let ranges = [range(1, 2)];
    let baseline = plan_from_rife_index(&index, &ranges).unwrap();
    let mut changed_source = index.clone();
    changed_source.temporal_inspection.source_sha256 = "c".repeat(64);
    let changed = plan_from_rife_index(&changed_source, &ranges).unwrap();
    assert_ne!(changed.source_sha256, baseline.source_sha256);
    assert_ne!(changed.frame_index_sha256, baseline.frame_index_sha256);
    for ordinal in [0, 9] {
        let mut changed_frame = index.clone();
        changed_frame.frames[ordinal].sha256 = "d".repeat(64);
        let changed = plan_from_rife_index(&changed_frame, &ranges).unwrap();
        assert_ne!(changed.frame_index_sha256, baseline.frame_index_sha256);
    }
}

#[test]
fn inclusive_range_parser_requires_two_unsigned_decimal_frame_numbers() {
    let parsed: InclusiveFrameRange = "12:310".parse().unwrap();
    assert_eq!((parsed.first, parsed.last), (12, 310));
    for text in ["", "1", "1:", ":2", "1:2:3", "1..2", "1.0:2", "-1:2", "+1:2", "1: 2", " 1:2", "１:２", "18446744073709551616:2"] {
        assert!(text.parse::<InclusiveFrameRange>().is_err(), "unexpected range: {text}");
    }
}

#[test]
fn machine_preview_uses_declared_index_only_and_keeps_input_bytes_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let index_path = root.join("index with spaces.json");
    let index_bytes = serde_json::to_vec(&frame_index(10)).unwrap();
    std::fs::write(&index_path, &index_bytes).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .current_dir(&root)
        .env("PATH", "")
        .args(["--output", "json", "video", "frame-selection-plan", "--frame-index"])
        .arg(&index_path)
        .args(["--range", "8:9", "--range", "1:2"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["command"], "video_frame_selection_plan");
    assert_eq!(envelope["status"], "success");
    assert_eq!(envelope["result"]["selected_frame_count"], 4);
    assert_eq!(envelope["result"]["unselected_frame_count"], 6);
    assert_eq!(envelope["result"]["ranges"][0]["first_frame"], 1);
    assert_eq!(std::fs::read(&index_path).unwrap(), index_bytes);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);

    let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .current_dir(&root)
        .env("PATH", "")
        .args(["--output", "json", "video", "frame-selection-plan", "--frame-index"])
        .arg(&index_path)
        .args(["--range", "1:299"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    // Preserve the current product's machine-error stream until the separate
    // observability migration explicitly revises it.
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["command"], "video_frame_selection_plan");
    assert_eq!(error["status"], "error");
    assert_eq!(std::fs::read(&index_path).unwrap(), index_bytes);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
}
