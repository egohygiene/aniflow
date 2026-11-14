//! Authored synthetic selection/manifest regressions; uncompiled/unrun.
use aniflow::frame_extraction::references::*;
use aniflow::temporal::{RationalTime, TimeRange};
fn policy(interval: u64, cap: u64) -> SamplingPolicy {
    SamplingPolicy {
        schema: "aniflow.reference-sampling/v1".into(),
        every_n_frames: interval,
        maximum_samples: cap,
        include_first: true,
        include_last: true,
        overflow: "reject".into(),
    }
}
#[test]
fn periodic_first_last_are_ordered_deduplicated_positions() {
    let selection = policy(3, 5).select(10).unwrap();
    assert_eq!(
        selection
            .iter()
            .map(|s| s.source_ordinal)
            .collect::<Vec<_>>(),
        [0, 3, 6, 9]
    );
    assert_eq!(selection[0].reasons, ["first", "periodic"]);
    assert_eq!(selection[3].reasons, ["periodic", "last"]);
    assert_eq!(
        policy(3, 5)
            .select(9)
            .unwrap()
            .last()
            .unwrap()
            .source_ordinal,
        8
    );
}
#[test]
fn a_single_frame_has_all_reasons_and_zero_or_overflow_is_explicit() {
    let one = policy(10, 1).select(1).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].reasons, ["first", "periodic", "last"]);
    for (interval, cap, count) in [
        (1, 1, 2),
        (1, 256, 100_000),
        (0, 256, 10),
        (10, 257, 10),
        (10, 2, 0),
        (10, 2, 100_001),
    ] {
        assert!(policy(interval, cap).select(count).is_err());
    }
}
fn manifest() -> ReferenceManifest {
    let period = RationalTime::new(1, 30).unwrap();
    let policy = policy(30, 3);
    let images = policy
        .select(60)
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(i, p)| ReferenceImage {
            sample_ordinal: i as u64,
            source_ordinal: p.source_ordinal,
            source_time: TimeRange::new(
                period.checked_mul(p.source_ordinal).unwrap(),
                period.checked_mul(p.source_ordinal + 1).unwrap(),
            )
            .unwrap(),
            reasons: p.reasons,
            filename: format!("frame-{i:08}.png"),
            sha256: "b".repeat(64),
            byte_count: 100,
            decoded_rgb24_sha256: "c".repeat(64),
        })
        .collect();
    ReferenceManifest {
        schema: "aniflow.reference-manifest/v1".into(),
        source_sha256: "a".repeat(64),
        source_byte_count: 1000,
        source_frames: 60,
        source_period: period,
        width: 2,
        height: 2,
        policy,
        ffmpeg: aniflow::audio_inspection::AudioToolObservation {
            id: "ffmpeg".into(),
            version: "8.0.1".into(),
            sha256: "d".repeat(64),
        },
        ffprobe: aniflow::audio_inspection::AudioToolObservation {
            id: "ffprobe".into(),
            version: "8.0.1".into(),
            sha256: "e".repeat(64),
        },
        images,
        conversion: "explicit_bt709_source_to_full_range_rgb24".into(),
        native_qualified: false,
    }
}
#[test]
fn exact_manifest_round_trip_and_local_review_links_keep_static_duplicates() {
    let value = manifest();
    value.validate().unwrap();
    assert_eq!(value.images[0].sha256, value.images[1].sha256);
    let decoded: ReferenceManifest =
        serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(value, decoded);
    let markdown = value.markdown().unwrap();
    assert!(markdown.contains("frames/frame-00000002.png"));
    assert!(markdown.contains("59"));
    assert!(!markdown.contains("/private/"));
}
#[test]
fn shifted_time_renamed_image_and_reordered_sample_invalidate_manifest() {
    let original = manifest();
    let mut value = original.clone();
    value.images[1].source_time = value.images[0].source_time;
    assert!(value.validate().is_err());
    let mut value = original.clone();
    value.images[1].filename = "../escape.png".into();
    assert!(value.validate().is_err());
    let mut value = original;
    value.images.swap(0, 1);
    assert!(value.validate().is_err());
}
