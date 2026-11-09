//! Synthetic request/order/refusal cases; no actual media, reviews or native
//! completion records are manufactured. Authored, uncompiled and unrun.
use super::*;
fn request()->PublishingRequest {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.publishing-request/v1","video_run_directory":"/synthetic/accepted-source","artifact_id":"delivery","provider_executable":"/synthetic/aniflow","export":null,"audio_qc":null,
        "profile":{"schema":"aniflow.destination-profile/v1","id":"synthetic-manual-video","revision":"1","destination":"tiktok","route":"manual",
            "evidence":{"source_urls":["https://developers.tiktok.com/doc/synthetic-fixture"],"checked_on":"2026-11-09","valid_through":"2026-11-09","checked_by":"synthetic declaration; not live policy"},
            "maximum_file_bytes":1048576,"maximum_duration":{"numerator":600,"denominator":1},"require_loudness_evidence":false,
            "requirements":{"kind":"video","minimum_axis":2,"maximum_axis":8192,"required_width":1080,"required_height":1920,"minimum_frame_rate":{"numerator":1,"denominator":1},"maximum_frame_rate":{"numerator":120,"denominator":1},"audio_required":false,"maximum_audio_channels":2,"require_zero_origin":true,"require_matching_audio_extent":false}},
        "evaluated_on":"2026-11-09","package":{"public_metadata":{"title":"Synthetic song","creator":"Synthetic operator","release_id":"synthetic-release","version":"1","credits":"fixture only","description":null,"artwork_references":[],"provenance_reference":null},"metadata_scope_policy":"require_clean_within_declared_scope","maximum_package_bytes":67108864,"minimum_free_bytes":0},"retention":null})).unwrap()
}
fn tools()->crate::video_assembly::AssemblyConfiguration {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},"delivery_metadata":{"schema":"aniflow.delivery-metadata-policy/v1","mode":"minimal","approved":{}}})).unwrap()
}
fn export(request:&PublishingRequest)->video::VideoExportConfiguration {
    video::VideoExportConfiguration {schema:"aniflow.destination-video-configuration/v1".into(),tools:tools(),geometry:video::GeometryRule::ExactAspect {width:1080,height:1920},allow_upscale:false,profile:request.profile.clone(),evaluated_on:request.evaluated_on}
}
fn posting_plan()->posting::PostingPlan {
    posting::PostingPlan {schema:"aniflow.posting-plan/v1".into(),request_sha256:"a".repeat(64),review_context_sha256:"b".repeat(64),review_subject_sha256:"c".repeat(64),media_filename:"delivery.mp4".into(),projected_package_bytes:1000,metadata_scope:"fixture only".into(),publication_authority:"none".into()}
}
#[test]
fn fixed_order_places_all_byte_changes_before_qc_review_package_and_retention() {
    let mut request=request();request.validate().unwrap();
    assert_eq!(phases(&request),[PublishingPhase::DestinationPreflight,PublishingPhase::FinalReview,PublishingPhase::ManualPackage]);
    request.export=Some(export(&request));request.audio_qc=Some(PublishingAudioQc {configuration:tools(),maximum_pcm_bytes:1_048_576});request.retention=Some(PublishingRetention {maximum_bundle_bytes:134_217_728,minimum_free_bytes:0});request.validate().unwrap();
    assert_eq!(phases(&request),[PublishingPhase::DestinationExport,PublishingPhase::FinalAudioQc,PublishingPhase::DestinationPreflight,PublishingPhase::FinalReview,PublishingPhase::ManualPackage,PublishingPhase::RetentionHandoff]);
}
#[test]
fn requested_loudness_cannot_be_satisfied_by_skipping_qc_or_a_zero_pcm_budget() {
    let mut request=request();request.profile.require_loudness_evidence=true;assert!(request.validate().is_err());
    request.audio_qc=Some(PublishingAudioQc {configuration:tools(),maximum_pcm_bytes:0});assert!(request.validate().is_err());
    request.audio_qc.as_mut().unwrap().maximum_pcm_bytes=1_048_576;request.validate().unwrap();
}
#[test]
fn export_and_preflight_cannot_disagree_about_profile_or_evaluation_date() {
    let mut request=request();request.export=Some(export(&request));request.validate().unwrap();
    request.export.as_mut().unwrap().profile.maximum_file_bytes+=1;assert!(request.validate().is_err());
    request.export=Some(export(&request));request.export.as_mut().unwrap().evaluated_on="2026-11-08".parse().unwrap();assert!(request.validate().is_err());
}
#[test]
fn supplied_rejection_is_valid_evidence_but_changed_subject_or_context_is_not() {
    let plan=posting_plan();let mut declaration=ManualReview {schema:"aniflow.manual-review/v1".into(),context_sha256:plan.review_context_sha256.clone(),subject_sha256:plan.review_subject_sha256.clone(),decision:ReviewDecision::Reject,reviewer:"synthetic declaration".into(),notes:"fixture; no actual approval".into()};
    require_review(&plan,&declaration).unwrap();
    declaration.subject_sha256="d".repeat(64);assert!(require_review(&plan,&declaration).is_err());declaration.subject_sha256=plan.review_subject_sha256.clone();
    declaration.context_sha256="e".repeat(64);assert!(require_review(&plan,&declaration).is_err());
}
#[test]
fn advice_cannot_offer_review_bindings_before_matching_preflight_or_bypass_rejection() {
    let plan=posting_plan();
    let blocked=[step(PublishingPhase::DestinationPreflight,PublishingStepState::Blocked,None,None),step(PublishingPhase::FinalReview,PublishingStepState::Pending,None,None)];
    let advice=readback::advise(&blocked,Some(&plan));assert!(advice.review_context_sha256.is_none());assert_eq!(advice.action,"change_media_or_requirements_in_new_intent");
    let review=[step(PublishingPhase::FinalReview,PublishingStepState::AwaitingReview,None,None)];
    assert_eq!(readback::advise(&review,Some(&plan)).review_subject_sha256,Some(plan.review_subject_sha256));
    let rejected=[step(PublishingPhase::FinalReview,PublishingStepState::Rejected,None,None)];assert_eq!(readback::advise(&rejected,None).action,"retain_rejection_start_new_intent");
}
#[test]
fn empty_or_partial_parent_is_never_adopted_and_readback_writes_nothing() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();
    assert!(status(&root,&CancellationToken::default()).is_err());assert_eq!(std::fs::read_dir(&root).unwrap().count(),0);
    std::fs::create_dir(root.join("children")).unwrap();std::fs::write(root.join("publishing-intent.json"),b"{}").unwrap();
    assert!(status(&root,&CancellationToken::default()).is_err());assert_eq!(std::fs::read_dir(&root).unwrap().count(),2);
}
#[test]
fn retention_handoff_cannot_invent_a_package_or_write_a_seal() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();let mut request=request();request.retention=Some(PublishingRetention {maximum_bundle_bytes:134_217_728,minimum_free_bytes:0});
    assert!(readback::retention(&root,&request,&CancellationToken::default()).is_err());assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
}
#[test]
fn invalid_public_metadata_is_refused_before_native_work_and_retention_uses_owner_bounds() {
    let mut request=request();request.package.public_metadata.title.clear();assert!(request.validate().is_err());
    let mut request=self::request();request.retention=Some(PublishingRetention {maximum_bundle_bytes:1_099_511_627_777,minimum_free_bytes:0});assert!(request.validate().is_err());
}
