//! Pure comparison/refusal fixtures; never fabricated native acceptance.
use super::*;
use crate::final_video::*;
use std::path::PathBuf;
fn time(n:i64)->RationalTime {RationalTime::new(n,1).unwrap()}
fn range(end:i64)->TimeRange {TimeRange::new(RationalTime::ZERO,time(end)).unwrap()}
pub(super) fn request()->PreflightRequest {
    PreflightRequest {schema:"aniflow.destination-preflight-request/v1".into(),input:MediaInput::AcceptedVideo {run_directory:PathBuf::from("/synthetic/accepted"),artifact_id:"delivery".into()},
        evaluated_on:"2026-10-02".parse().unwrap(),profile:DestinationProfile {schema:"aniflow.destination-profile/v1".into(),id:"synthetic_portrait".into(),revision:"shape.v1".into(),destination:Destination::Tiktok,route:DeliveryRoute::Manual,
            evidence:RequirementEvidence {source_urls:vec!["https://developers.tiktok.com/doc/content-posting-api-media-transfer-guide".into()],checked_on:"2026-10-02".parse().unwrap(),valid_through:"2026-10-02".parse().unwrap(),checked_by:"synthetic declaration; not a current policy review".into()},
            maximum_file_bytes:1_000_000,maximum_duration:time(2),require_loudness_evidence:false,
            requirements:MediaRequirements::Video {minimum_axis:360,maximum_axis:4096,required_width:Some(1080),required_height:Some(1920),minimum_frame_rate:time(23),maximum_frame_rate:time(60),audio_required:false,maximum_audio_channels:2,require_zero_origin:true,require_matching_audio_extent:false}}}
}
pub(super) fn facts(request:&PreflightRequest)->MediaObservation {
    let a=FinalVideoArtifact {artifact_id:"delivery".into(),variant:FinalVideoVariant::DestinationCopy,locator:PathBuf::from("/synthetic/delivery.mp4"),sha256:"a".repeat(64),byte_count:100_000,
        width:1080,height:1920,frame_count:24,frame_rate:time(24),video_interval:range(1),video_duration:time(1),video_codec:"h264".into(),pixel_format:"yuv420p".into(),
        color_primaries:"bt709".into(),color_transfer:"bt709".into(),color_matrix:"bt709".into(),color_range:"tv".into(),sample_aspect_ratio:"1:1".into(),orientation_degrees:0,audio:None,metadata:None,
        producer_lock_sha256:"b".repeat(64),stage_invocation_sha256:"c".repeat(64),stage_checkpoint_sha256:"d".repeat(64),private_report_sha256:"e".repeat(64)};
    let file=MediaFile {locator:a.locator.clone(),sha256:a.sha256.clone(),byte_count:a.byte_count};
    let completion=FinalVideoHandoff {schema:FINAL_VIDEO_HANDOFF_V1.into(),completion_id:"f".repeat(64),plan_sha256:"b".repeat(64),run_manifest_sha256:"c".repeat(64),delivery_acceptance_sha256:"d".repeat(64),
        source_lineage:vec![],artifacts:vec![a],creative_approval:"not_performed".into(),publication_authority:"none".into(),proof_state:"not_asserted".into()};
    MediaObservation {input:request.input.clone(),file,facts:MediaFacts::Video {completion,artifact_id:"delivery".into()}}
}
#[test]
fn pure_profile_match_never_becomes_live_policy_or_content_approval() {
    let r=request();r.validate().unwrap();let m=facts(&r);
    let report=evaluate(&r,m.clone()).unwrap();assert_eq!(report.payload.disposition,PreflightDisposition::MatchesSuppliedProfile);
    assert_eq!(report,evaluate(&r,m).unwrap());assert_eq!(report.payload.creative_approval,"not_performed");assert_eq!(report.payload.publication_authority,"none");
    assert!(report.payload.requirement_authority.contains("not independently verified"));
}
#[test]
fn full_audio_tail_and_geometry_are_not_silently_trimmed_or_reframed() {
    let r=request();let mut m=facts(&r);
    let MediaFacts::Video {completion,..}=&mut m.facts else {panic!("fixture");};
    let a=&mut completion.artifacts[0];a.width=1920;a.height=1080;
    a.audio=Some(FinalAudioProperties {codec:"aac".into(),profile:"LC".into(),sample_rate_hz:48_000,channels:2,channel_layout:"stereo".into(),interval:range(3),packet_configuration_timing_sha256:"9".repeat(64)});
    let report=evaluate(&r,m).unwrap();assert_eq!(report.payload.disposition,PreflightDisposition::NeedsChanges);
    for id in ["complete_duration","video_geometry"] {assert!(report.payload.checks.iter().any(|c|c.id==id&&c.state==CheckState::NeedsChanges));}
}
#[test]
fn stale_requirement_dates_and_unobserved_loudness_remain_missing() {
    let mut r=request();r.evaluated_on="2026-11-08".parse().unwrap();r.profile.require_loudness_evidence=true;
    let report=evaluate(&r,facts(&r)).unwrap();assert_eq!(report.payload.disposition,PreflightDisposition::MissingEvidence);
    for id in ["requirement_dates","loudness_evidence"] {assert!(report.payload.checks.iter().any(|c|c.id==id&&c.state==CheckState::MissingEvidence));}
}
#[test]
fn closed_requirements_refuse_wrong_destination_unknown_fields_and_bad_bounds() {
    let original=request();let mut wrong=original.clone();wrong.profile.destination=Destination::Soundcloud;assert!(wrong.validate().is_err());
    let mut wrong=original.clone();wrong.profile.maximum_file_bytes=0;assert!(wrong.validate().is_err());
    let mut value=serde_json::to_value(&original).unwrap();value["profile"]["auto_trim"]=serde_json::json!(true);
    assert!(PreflightRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn fabricated_saved_observation_cannot_replace_current_native_acceptance() {
    let directory=tempfile::tempdir().unwrap();let root=directory.path().canonicalize().unwrap();let mut r=request();
    r.input=MediaInput::AcceptedVideo {run_directory:root.clone(),artifact_id:"delivery".into()};
    let report=evaluate(&r,facts(&r)).unwrap();assert!(recheck(&report,&CancellationToken::default()).is_err());
    let mut forged=report;forged.preflight_sha256="0".repeat(64);assert!(recheck(&forged,&CancellationToken::default()).is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
}
#[test]
fn cancelled_preflight_refuses_before_source_observation() {
    let r=request();let token=CancellationToken::default();token.cancel();
    assert!(preflight(&r,&token).is_err());
}
