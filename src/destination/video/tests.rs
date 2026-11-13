//! Pure synthetic operation fixtures; no native media or platform acceptance.
use super::*;
use crate::final_video::*;
use crate::temporal::{RationalTime, TimeRange};
pub(crate) fn operation() -> VideoExportOperation {
    use crate::destination::*;
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    let tools:crate::video_assembly::AssemblyConfiguration=serde_json::from_value(serde_json::json!({"schema":"aniflow.video-assembly-configuration/v1",
        "ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},
        "delivery_metadata":{"schema":"aniflow.delivery-metadata-policy/v1","mode":"minimal","approved":{}}})).unwrap();
    let profile = DestinationProfile {
        schema: "aniflow.destination-profile/v1".into(),
        id: "synthetic-video".into(),
        revision: "1".into(),
        destination: Destination::Tiktok,
        route: DeliveryRoute::Manual,
        evidence: RequirementEvidence {
            source_urls: vec!["https://developers.tiktok.com/doc/synthetic-fixture".into()],
            checked_on: date,
            valid_through: date,
            checked_by: "synthetic declaration".into(),
        },
        maximum_file_bytes: 1_048_576,
        maximum_duration: RationalTime::new(600, 1).unwrap(),
        require_loudness_evidence: false,
        requirements: MediaRequirements::Video {
            minimum_axis: 2,
            maximum_axis: 8192,
            required_width: Some(2),
            required_height: Some(2),
            minimum_frame_rate: RationalTime::new(1, 1).unwrap(),
            maximum_frame_rate: RationalTime::new(120, 1).unwrap(),
            audio_required: false,
            maximum_audio_channels: 2,
            require_zero_origin: true,
            require_matching_audio_extent: false,
        },
    };
    let configuration = VideoExportConfiguration {
        schema: "aniflow.destination-video-configuration/v1".into(),
        tools,
        geometry: GeometryRule::ExactAspect {
            width: 2,
            height: 2,
        },
        allow_upscale: false,
        profile,
        evaluated_on: date,
    };
    let interval = TimeRange::new(RationalTime::ZERO, RationalTime::new(2, 30).unwrap()).unwrap();
    let artifact = FinalVideoArtifact {
        artifact_id: "source_video".into(),
        variant: FinalVideoVariant::DestinationCopy,
        locator: "/synthetic/source/video.mp4".into(),
        sha256: "c".repeat(64),
        byte_count: 1000,
        width: 2,
        height: 2,
        frame_count: 2,
        frame_rate: RationalTime::new(30, 1).unwrap(),
        video_interval: interval,
        video_duration: interval.duration().unwrap(),
        video_codec: "h264".into(),
        pixel_format: "yuv420p".into(),
        color_primaries: "bt709".into(),
        color_transfer: "bt709".into(),
        color_matrix: "bt709".into(),
        color_range: "tv".into(),
        sample_aspect_ratio: "1/1".into(),
        orientation_degrees: 0,
        audio: None,
        metadata: None,
        producer_lock_sha256: "d".repeat(64),
        stage_invocation_sha256: "e".repeat(64),
        stage_checkpoint_sha256: "f".repeat(64),
        private_report_sha256: "1".repeat(64),
    };
    let source = FinalVideoHandoff {
        schema: FINAL_VIDEO_HANDOFF_V1.into(),
        completion_id: "2".repeat(64),
        plan_sha256: "3".repeat(64),
        run_manifest_sha256: "4".repeat(64),
        delivery_acceptance_sha256: "5".repeat(64),
        source_lineage: Vec::new(),
        artifacts: vec![artifact.clone()],
        creative_approval: "not_performed".into(),
        publication_authority: "none".into(),
        proof_state: "not_asserted".into(),
    };
    let request = VideoExportRequest {
        schema: "aniflow.destination-video-request/v1".into(),
        source_run_directory: "/synthetic/source".into(),
        artifact_id: "source_video".into(),
        configuration,
        provider_executable: "/synthetic/aniflow".into(),
    };
    let payload = VideoExportPayload {
        request,
        source,
        artifact,
        geometry: ResolvedGeometry {
            width: 2,
            height: 2,
            scaled_width: 2,
            scaled_height: 2,
            pad_left: 0,
            pad_top: 0,
            pad_right: 0,
            pad_bottom: 0,
        },
    };
    VideoExportOperation {
        schema: OPERATION_SCHEMA.into(),
        intent: VideoExportIntent {
            schema: "aniflow.destination-video-intent/v1".into(),
            intent_sha256: canonical_sha256(&payload).unwrap(),
            payload,
            quality_approval: "not_performed".into(),
            platform_authority: "none".into(),
        },
    }
}
#[test]
fn operation_rejects_unbound_source_geometry_and_stale_intent() {
    let op = operation();
    op.validate().unwrap();
    let mut changed = op.clone();
    changed.intent.payload.geometry.width = 4;
    assert!(changed.validate().is_err());
    let mut changed = op;
    changed.intent.payload.artifact.sha256 = "0".repeat(64);
    changed.intent.intent_sha256 = canonical_sha256(&changed.intent.payload).unwrap();
    assert!(changed.validate().is_err());
}
#[test]
fn unsupported_subtitle_source_never_silently_loses_tracks() {
    let mut op = operation();
    op.intent.payload.artifact.variant = FinalVideoVariant::SubtitleMuxedCandidate;
    op.intent.payload.source.artifacts = vec![op.intent.payload.artifact.clone()];
    op.intent.intent_sha256 = canonical_sha256(&op.intent.payload).unwrap();
    assert!(op.validate().is_err());
}
#[test]
fn output_limit_uses_selected_profile_and_declared_byte_bounds() {
    let mut op = operation();
    assert_eq!(op.output_limit(), 1_048_576);
    op.intent
        .payload
        .request
        .configuration
        .tools
        .maximum_output_bytes = 4096;
    assert_eq!(op.output_limit(), 4096);
    op.intent
        .payload
        .request
        .configuration
        .tools
        .maximum_working_bytes = 1_048_576;
    assert!(op.working_bytes().is_err());
}
#[test]
fn provider_is_closed_lossy_video_with_no_model_or_audio_transform_permission() {
    let op = operation();
    let text = String::from_utf8(canonical_json_bytes(&op.manifest().unwrap()).unwrap()).unwrap();
    assert!(text.contains("\"fidelity\":\"lossy\""));
    assert!(text.contains("\"models\":[]"));
    assert!(!text.contains("\"ai\""));
    assert!(!text.contains("normalization"));
}
#[test]
fn supplied_profile_expiry_prevents_native_planning_admission() {
    let mut op = operation();
    op.intent.payload.request.configuration.evaluated_on =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
    op.intent.intent_sha256 = canonical_sha256(&op.intent.payload).unwrap();
    assert!(op.validate().is_err());
}
