use super::*;
fn intent() -> ImportIntent {
    let settings = ImportSettings {
        decoder: FrameImportConfiguration {
            schema: frame_import::FRAME_IMPORT_CONFIGURATION_SCHEMA_V1.into(),
            ffmpeg: crate::audio_inspection::AudioToolPin {
                executable: "/synthetic/ffmpeg".into(),
                version: "8.0.1".into(),
                sha256: "a".repeat(64),
            },
            tool_timeout_milliseconds: 120_000,
            maximum_source_bytes: 1_048_576,
            maximum_frame_bytes: 1_048_576,
            maximum_total_frame_bytes: 16_777_216,
            maximum_pixels: 1_048_576,
        },
        output_width: 4,
        output_height: 4,
        minimum_free_bytes: 0,
    };
    let range = crate::temporal::TimeRange::new(
        crate::temporal::RationalTime::new(5, 30).unwrap(),
        crate::temporal::RationalTime::new(6, 30).unwrap(),
    )
    .unwrap();
    ImportIntent {
        settings,
        source_sha256: "b".repeat(64),
        source_bytes: 1000,
        source_index_sha256: "c".repeat(64),
        source_index_bytes: 2000,
        source_width: 2,
        source_height: 2,
        original_frame_count: 100,
        frames: vec![HistoricalFrame {
            ordinal: 5,
            filename: "frame_00000005.png".into(),
            sha256: "d".repeat(64),
            byte_count: 100,
            source_time: range,
        }],
    }
}
fn declaration(p: &ImportReviewPlan) -> ManualReview {
    ManualReview {
        schema: "aniflow.manual-review/v1".into(),
        context_sha256: p.context_sha256.clone(),
        subject_sha256: p.subject_sha256.clone(),
        decision: ReviewDecision::Approve,
        reviewer: "synthetic-reviewer".into(),
        notes: "Synthetic import decision only; no creative/native qualification".into(),
    }
}
#[test]
fn subset_keeps_original_ordinal_time_and_unknown_history() {
    let p = review(intent()).unwrap();
    assert_eq!(p.intent.frames[0].ordinal, 5);
    assert_eq!(p.intent.original_frame_count, 100);
    assert!(p.historical_processing.starts_with("unknown"));
    assert_ne!(
        SUBSET_SCHEMA,
        crate::upscayl_recovery::UPSCAYL_OUTPUT_INDEX_SCHEMA_V1
    );
}
#[test]
fn selected_pixels_decoder_geometry_and_clock_bind_the_import_decision() {
    let original = review(intent()).unwrap();
    let accepted = declaration(&original);
    let mut changed = intent();
    changed.settings.output_width = 8;
    changed.settings.output_height = 8;
    assert!(
        ImportOperation {
            schema: OPERATION_SCHEMA.into(),
            intent: changed,
            declaration: accepted.clone()
        }
        .validate()
        .is_err()
    );
    let mut changed = intent();
    changed.frames[0].sha256 = "e".repeat(64);
    assert!(
        ImportOperation {
            schema: OPERATION_SCHEMA.into(),
            intent: changed,
            declaration: accepted
        }
        .validate()
        .is_err()
    );
}
#[test]
fn rejection_and_missing_or_stale_decision_cannot_import() {
    let p = review(intent()).unwrap();
    let mut d = declaration(&p);
    d.decision = ReviewDecision::Reject;
    assert!(
        ImportOperation {
            schema: OPERATION_SCHEMA.into(),
            intent: p.intent.clone(),
            declaration: d
        }
        .validate()
        .is_err()
    );
    let mut d = declaration(&p);
    d.subject_sha256 = "0".repeat(64);
    assert!(
        ImportOperation {
            schema: OPERATION_SCHEMA.into(),
            intent: p.intent,
            declaration: d
        }
        .validate()
        .is_err()
    );
}
#[test]
fn duplicate_or_unordered_ordinals_and_unsafe_names_refuse() {
    let mut i = intent();
    i.frames.push(i.frames[0].clone());
    assert!(i.validate().is_err());
    let mut i = intent();
    i.frames[0].filename = "../frame.png".into();
    assert!(i.validate().is_err());
    let mut i = intent();
    i.frames[0].ordinal = 100;
    assert!(i.validate().is_err());
}
#[test]
fn scale_pixel_frame_count_and_byte_bounds_are_explicit() {
    let mut i = intent();
    i.settings.output_width = 5;
    assert!(i.validate().is_err());
    let mut i = intent();
    i.settings.decoder.maximum_pixels = 1;
    assert!(i.validate().is_err());
    let mut i = intent();
    i.frames[0].byte_count = i.settings.decoder.maximum_frame_bytes + 1;
    assert!(i.validate().is_err());
    let mut i = intent();
    i.frames = vec![i.frames[0].clone(); MAX_FRAMES + 1];
    assert!(i.validate().is_err());
}
#[test]
fn importer_manifest_has_no_upscayl_model_or_ai_execution_claim() {
    let p = review(intent()).unwrap();
    let op = ImportOperation {
        schema: OPERATION_SCHEMA.into(),
        declaration: declaration(&p),
        intent: p.intent,
    };
    let manifest = op.manifest().unwrap();
    let text = String::from_utf8(canonical_json_bytes(&manifest).unwrap()).unwrap();
    assert!(text.contains(PROVIDER_ID));
    assert!(!text.contains("model_weights"));
    assert!(!text.contains("\"ai\""));
    assert!(!text.contains(crate::upscayl_recovery::UPSCAYL_FRAME_PROVIDER_ID));
}
