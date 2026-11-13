//! Pure/synthetic regressions, authored and uncompiled/unrun; no tool invocation.
use super::*;
fn settings() -> crate::video_assembly::AssemblyConfiguration {
    crate::video_assembly::AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&serde_json::json!({"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"b".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"c".repeat(64)}})).unwrap()).unwrap()
}
fn configuration() -> CaptionConfiguration {
    CaptionConfiguration {
        schema: "aniflow.caption-configuration/v1".into(),
        mode: CaptionMode::BurnIn,
        settings: settings(),
        font: CaptionFont {
            file: "/synthetic/face.ttf".into(),
            sha256: "a".repeat(64),
        },
        style: CaptionStyle {
            font_size: 40,
            bottom_margin: 32,
            foreground: "ffffff".into(),
            box_color: "000000".into(),
            box_opacity: 180,
        },
        preview_cue_indices: vec![],
    }
}
#[test]
fn actual_millisecond_srt_maps_to_half_open_fractional_cfr_without_float_rounding() {
    use crate::audio_analysis::AudioRationalTime as T;
    let rate = RationalTime::new(30000, 1001).unwrap();
    assert_eq!(
        frame_at(
            T {
                numerator: 1,
                denominator: 1000
            },
            rate
        )
        .unwrap(),
        1
    );
    assert_eq!(
        frame_at(
            T {
                numerator: 1001,
                denominator: 30000
            },
            rate
        )
        .unwrap(),
        1
    );
    assert_eq!(
        frame_at(
            T {
                numerator: 1001,
                denominator: 1000
            },
            rate
        )
        .unwrap(),
        30
    );
    assert!(
        frame_at(
            T {
                numerator: -1,
                denominator: 1
            },
            rate
        )
        .is_err()
    );
}
#[test]
fn requested_subset_is_bounded_and_does_not_become_full_delivery_approval() {
    let mut cfg = configuration();
    let cues = vec![
        CaptionCue {
            id: "a".into(),
            text: "one".into(),
            first_frame: 0,
            end_frame: 10,
        },
        CaptionCue {
            id: "b".into(),
            text: "two".into(),
            first_frame: 20,
            end_frame: 30,
        },
    ];
    assert_eq!(selection(&cfg, &cues).unwrap(), vec![4, 24]);
    cfg.preview_cue_indices = vec![1];
    assert_eq!(selection(&cfg, &cues).unwrap(), vec![24]);
    cfg.preview_cue_indices = vec![2];
    assert!(selection(&cfg, &cues).is_err());
    cfg.preview_cue_indices = vec![0, 0];
    assert!(cfg.validate().is_err());
}
#[test]
fn plain_authored_text_is_never_interpolated_into_native_filter_or_argv() {
    let cfg = configuration();
    let cue = CaptionCue {
        id: "cue".into(),
        text: "literal ' : %, [drawtext] \\ newline\n大丈夫".into(),
        first_frame: 3,
        end_frame: 17,
    };
    let filter =
        crate::video_assembly::media::captions::filter_parameters(&[cue.clone()], &cfg.style);
    assert!(filter.contains("textfile=cue-0000.txt:expansion=none"));
    assert!(filter.contains("gte(n\\,3)*lt(n\\,17)"));
    assert!(!filter.contains(&cue.text));
    assert!(!filter.contains("/synthetic"));
}
#[test]
fn preview_rejection_or_changed_context_or_subject_cannot_authorize_delivery() {
    let binding = PreviewBinding {
        run_directory: "/synthetic/preview".into(),
        report_sha256: "a".repeat(64),
        context_sha256: "b".repeat(64),
        subject_sha256: "c".repeat(64),
    };
    let mut declaration = ManualReview {
        schema: "aniflow.manual-review/v1".into(),
        context_sha256: binding.context_sha256.clone(),
        subject_sha256: binding.subject_sha256.clone(),
        decision: ReviewDecision::Reject,
        reviewer: "synthetic".into(),
        notes: "fixture only".into(),
    };
    assert!(validate_declaration(&binding.context_sha256, &binding, &declaration).is_err());
    declaration.decision = ReviewDecision::Approve;
    assert!(validate_declaration(&binding.context_sha256, &binding, &declaration).is_ok());
    assert!(validate_declaration(&"d".repeat(64), &binding, &declaration).is_err());
    declaration.subject_sha256 = "e".repeat(64);
    assert!(validate_declaration(&binding.context_sha256, &binding, &declaration).is_err());
}
#[test]
fn unsafe_style_and_implicit_metadata_operation_refuse() {
    let mut cfg = configuration();
    cfg.style.foreground = "ffffff:fontfile=other".into();
    assert!(cfg.validate().is_err());
    cfg = configuration();
    cfg.style.font_size = 0;
    assert!(cfg.validate().is_err());
    cfg = configuration();
    cfg.font.sha256 = "unknown".into();
    assert!(cfg.validate().is_err());
}
#[test]
fn loose_preview_report_has_no_acceptance_and_status_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    std::fs::write(root.join("private.json"), b"{}").unwrap();
    assert!(preview_status(&root, &CancellationToken::default()).is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
}
