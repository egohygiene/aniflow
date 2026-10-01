//! Authored regression coverage for #32; execution intentionally deferred.
use std::path::PathBuf;

use aniflow::temporal::{
    self, ArtifactStreamRelation, RationalTime, StreamSelection, TemporalCode, TemporalInspection,
};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal")
        .join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn parse(value: &Value) -> aniflow::Result<TemporalInspection> {
    let observations = value["observations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            (
                o["stream_index"].as_u64().unwrap() as u32,
                o["frames"].clone(),
                o["packets"].clone(),
            )
        })
        .collect::<Vec<_>>();
    temporal::from_probe_documents(
        value["source_sha256"].as_str().unwrap(),
        value["source_size_bytes"].as_u64().unwrap(),
        &serde_json::from_value(value["selection"].clone()).unwrap(),
        &value["inventory"],
        &observations,
    )
}

#[test]
fn generator_first_protocol_corpus_has_exact_supported_or_typed_refused_outcomes() {
    let index = fixture("index");
    for id in index["fixtures"].as_array().unwrap() {
        let value = fixture(id.as_str().unwrap());
        let result = parse(&value);
        let codes: Vec<Value> = match value["expected"]["outcome"].as_str().unwrap() {
            "parser_error" => vec![
                serde_json::to_value(
                    result
                        .expect_err("malformed protocol must fail")
                        .temporal_diagnostic()
                        .expect("typed temporal reason")
                        .code,
                )
                .unwrap(),
            ],
            outcome => {
                let report = result.expect("well-formed observation must remain inspectable");
                assert_eq!(
                    report.processing.supported,
                    outcome == "supported",
                    "{id}: {:?}",
                    report.processing.diagnostics
                );
                assert_eq!(
                    report.streams.iter().map(|s| s.index).collect::<Vec<_>>(),
                    {
                        let mut v = report.streams.iter().map(|s| s.index).collect::<Vec<_>>();
                        v.sort_unstable();
                        v
                    }
                );
                report
                    .processing
                    .diagnostics
                    .iter()
                    .map(|d| serde_json::to_value(d.code).unwrap())
                    .collect()
            }
        };
        for expected in value["expected"]["codes"].as_array().unwrap() {
            assert!(
                codes.contains(expected),
                "{id}: expected {expected}, got {codes:?}"
            );
        }
    }
}

#[test]
fn rational_time_is_reduced_signed_and_exact_without_float_arithmetic() {
    let period = RationalTime::new(1001, 24000).unwrap();
    assert_eq!(
        period.checked_mul(24000).unwrap(),
        RationalTime::new(1001, 1).unwrap()
    );
    assert_eq!(
        RationalTime::decimal("-0.125").unwrap(),
        RationalTime::new(-1, 8).unwrap()
    );
    assert_eq!(period.reciprocal().unwrap().to_string(), "24000/1001");
    assert_eq!(
        RationalTime::new(2, 4).unwrap(),
        RationalTime::new(1, 2).unwrap()
    );
    assert_eq!(RationalTime::new(0, 99).unwrap(), RationalTime::ZERO);
}

#[test]
fn rational_wire_values_reject_invalid_or_noncanonical_representations() {
    for raw in [
        r#"{"numerator":1,"denominator":0}"#,
        r#"{"numerator":2,"denominator":4}"#,
        r#"{"numerator":0,"denominator":2}"#,
        r#"{"numerator":1,"denominator":1,"extra":true}"#,
    ] {
        assert!(serde_json::from_str::<RationalTime>(raw).is_err());
    }
    assert!(
        RationalTime::new(i64::MAX, 1)
            .unwrap()
            .checked_mul(2)
            .is_err()
    );
    for raw in ["NaN", "inf", "1e4", "-", "1/-2", "0/0"] {
        assert!(RationalTime::parse(raw).is_err() || RationalTime::decimal(raw).is_err());
    }
}

#[test]
fn sample_and_seek_projection_have_explicit_integer_bounds() {
    let time = RationalTime::new(1001, 24000).unwrap();
    assert_eq!(time.sample_boundary_ceil(48000).unwrap(), 2002);
    assert_eq!(time.sample_boundary_ceil(44100).unwrap(), 1840);
    assert_eq!(time.ffmpeg_microseconds().unwrap(), "0.041708");
    assert!(
        RationalTime::new(-1, 10)
            .unwrap()
            .ffmpeg_microseconds()
            .is_err()
    );
}

#[test]
fn decode_order_and_presentation_order_are_distinct() {
    let report = parse(&fixture("reordered-packet-pts-monotonic-dts")).unwrap();
    assert!(report.processing.supported);
    let timeline = report.video_timeline().unwrap();
    assert_eq!(timeline.packets[0].decode_ticks, Some(-2002));
    assert!(timeline.packets[1].presentation_ticks > timeline.packets[2].presentation_ticks);
    assert!(
        timeline
            .frames
            .windows(2)
            .all(|p| p[0].source_time.end == p[1].source_time.start)
    );
}

#[test]
fn offsets_remain_exact_observations_and_processing_never_silently_rebases_them() {
    for (id, expected) in [
        ("positive-origin", RationalTime::new(5005, 24000).unwrap()),
        ("negative-origin", RationalTime::new(-2002, 24000).unwrap()),
    ] {
        let report = parse(&fixture(id)).unwrap();
        assert_eq!(
            report.video_timeline().unwrap().source_time.unwrap().start,
            expected
        );
        assert!(report.require_processing().is_err());
    }
}

#[test]
fn explicit_stream_choice_is_independent_of_inventory_order() {
    let mut value = fixture("explicit-second-audio");
    let before = parse(&value).unwrap();
    value["inventory"]["streams"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let after = parse(&value).unwrap();
    assert_eq!(before, after);
    assert_eq!(after.selected.unwrap().audio, Some(11));
}

#[test]
fn cached_supported_flag_cannot_override_vfr_refusal() {
    let mut report = parse(&fixture("vfr-visible-refusal")).unwrap();
    report.processing.supported = true;
    report.processing.diagnostics.clear();
    assert!(report.require_processing().is_err());
}

#[test]
fn reconstruction_requires_each_exact_frame_interval_and_cardinality() {
    let source = parse(&fixture("cfr-24000-1001")).unwrap();
    temporal::validate_reconstruction(&source, &source, false).unwrap();
    let mut dropped = source.clone();
    dropped.timelines[0].frames.pop();
    assert!(temporal::validate_reconstruction(&source, &dropped, false).is_err());
    let mut retimed = source.clone();
    retimed.timelines[0].frames[1].source_time.start = RationalTime::new(1, 10).unwrap();
    assert!(temporal::validate_reconstruction(&source, &retimed, false).is_err());
    let mut reordered = source.clone();
    reordered.timelines[0].frames.swap(1, 2);
    assert!(temporal::validate_reconstruction(&source, &reordered, false).is_err());
}

#[test]
fn source_frame_binding_is_exact_read_only_and_workspace_confined() {
    let source = parse(&fixture("cfr-24000-1001")).unwrap();
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("frame-00000002.png");
    std::fs::write(&file, b"protocol-only image placeholder").unwrap();
    let before = std::fs::read(&file).unwrap();
    let binding = temporal::bind_artifact(
        root.path(),
        &file,
        &source,
        ArtifactStreamRelation::Frame(1),
    )
    .unwrap();
    assert_eq!(
        binding.source_streams[0].source_time,
        source.video_timeline().unwrap().frames[1].source_time
    );
    assert_eq!(binding.source_streams[0].frame_ordinal, Some(1));
    assert_eq!(binding.source_sha256, source.source_sha256);
    assert_eq!(std::fs::read(&file).unwrap(), before);
    let elsewhere = tempfile::NamedTempFile::new().unwrap();
    assert!(
        temporal::bind_artifact(
            root.path(),
            elsewhere.path(),
            &source,
            ArtifactStreamRelation::Frame(0)
        )
        .is_err()
    );
    assert!(
        temporal::bind_artifact(
            root.path(),
            &file,
            &source,
            ArtifactStreamRelation::Frame(99)
        )
        .is_err()
    );
}

#[test]
fn missing_or_conflicting_selection_stays_machine_readable() {
    let value = fixture("ambiguous-multi-video");
    let report = parse(&value).unwrap();
    assert_eq!(
        report.processing.diagnostics[0].code,
        TemporalCode::AmbiguousVideo
    );
    let mut changed = fixture("explicit-no-audio");
    changed["selection"]["audio_stream"] = Value::from(8);
    changed["observations"] = Value::Array(Vec::new());
    let report = parse(&changed).unwrap();
    assert_eq!(
        report.processing.diagnostics[0].code,
        TemporalCode::InvalidSelection
    );
    let _: StreamSelection = serde_json::from_value(value["selection"].clone()).unwrap();
}

#[test]
fn frozen_selection_and_unique_stream_identities_are_rechecked() {
    let mut selected = parse(&fixture("explicit-second-video")).unwrap();
    selected.selection_intent.video_stream = Some(7);
    assert!(selected.require_processing().is_err());
    let mut duplicate = parse(&fixture("cfr-24000-1001")).unwrap();
    duplicate.streams.push(duplicate.streams[0].clone());
    assert!(duplicate.require_processing().is_err());
    let mut duplicate = parse(&fixture("cfr-24000-1001")).unwrap();
    duplicate.timelines.push(duplicate.timelines[0].clone());
    assert!(duplicate.require_processing().is_err());
}
