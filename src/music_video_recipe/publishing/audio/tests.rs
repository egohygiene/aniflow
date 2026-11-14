//! Synthetic fixed-order and refusal cases; no native run or review is forged.
use super::*;
fn request() -> AudioPublishingRequest {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.audio-publishing-request/v1","subject":{"kind":"accepted_pcm","file":"/synthetic/original.wav","signal_run_directory":"/synthetic/accepted-signal"},"provider_executable":"/synthetic/provider","export":null,"audio_qc":null,
        "profile":{"schema":"aniflow.destination-profile/v1","id":"synthetic-audio","revision":"1","destination":"soundcloud","route":"manual","evidence":{"source_urls":["https://help.soundcloud.com/hc/synthetic-fixture"],"checked_on":"2026-11-09","valid_through":"2026-11-09","checked_by":"synthetic supplied requirements; not live policy"},"maximum_file_bytes":268435456,"maximum_duration":{"numerator":600,"denominator":1},"require_loudness_evidence":false,"requirements":{"kind":"pcm","sample_rates_hz":[44100,48000,96000],"channels":[1,2],"encodings":["pcm16","pcm24","float32"]}},
        "evaluated_on":"2026-11-09","package":{"public_metadata":{"title":"Synthetic audio","creator":"Synthetic operator","release_id":"synthetic-release","version":"1","credits":"fixture only","description":null,"artwork_references":[],"provenance_reference":null},"metadata_scope_policy":"acknowledge_inherited_or_uninspected_scope","maximum_package_bytes":536870912,"minimum_free_bytes":0},"retention":null})).unwrap()
}
fn tools() -> crate::audio_inspection::AudioInspectionConfiguration {
    serde_json::from_value(serde_json::json!({"schema":crate::audio_inspection::AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1,"ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},"tool_timeout_milliseconds":1000,"maximum_tool_output_bytes":4096})).unwrap()
}
fn export() -> LosslessConfiguration {
    LosslessConfiguration {
        schema: "aniflow.lossless-audio-configuration/v1".into(),
        format: native_export::LosslessFormat::Flac,
        tools: tools(),
        metadata_policy: native_export::ExportMetadataPolicy::RemoveInheritedDescriptiveMetadata,
        maximum_output_bytes: 268435456,
        maximum_working_bytes: 1073741824,
        minimum_free_bytes: 0,
    }
}
fn qc() -> AudioPublishingQc {
    AudioPublishingQc {
        configuration: tools(),
        maximum_pcm_bytes: 268435456,
        maximum_working_bytes: 1073741824,
        minimum_free_bytes: 0,
    }
}
#[test]
fn audio_order_places_every_byte_change_before_final_qc_review_and_package() {
    let mut r = request();
    r.validate().unwrap();
    assert_eq!(
        phases(&r),
        [
            PublishingPhase::DestinationPreflight,
            PublishingPhase::FinalReview,
            PublishingPhase::ManualPackage
        ]
    );
    r.export = Some(export());
    r.audio_qc = Some(qc());
    r.retention = Some(PublishingRetention {
        maximum_bundle_bytes: 1073741824,
        minimum_free_bytes: 0,
    });
    r.validate().unwrap();
    assert_eq!(
        phases(&r),
        [
            PublishingPhase::DestinationExport,
            PublishingPhase::FinalAudioQc,
            PublishingPhase::DestinationPreflight,
            PublishingPhase::FinalReview,
            PublishingPhase::ManualPackage,
            PublishingPhase::RetentionHandoff
        ]
    );
}
#[test]
fn required_final_loudness_cannot_skip_qc_or_change_native_profile_implicitly() {
    let mut r = request();
    r.profile.require_loudness_evidence = true;
    assert!(r.validate().is_err());
    r.audio_qc = Some(qc());
    r.validate().unwrap();
    r.audio_qc.as_mut().unwrap().maximum_pcm_bytes = 0;
    assert!(r.validate().is_err());
    r.audio_qc = Some(qc());
    r.audio_qc.as_mut().unwrap().configuration.ffmpeg.version = "9.0.0".into();
    assert!(r.validate().is_err());
}
#[test]
fn existing_lossless_subject_is_selected_directly_and_never_reexported_implicitly() {
    let mut r = request();
    r.subject = FinalLosslessSubject::AcceptedAudioExport {
        run_directory: "/synthetic/native-flac".into(),
    };
    r.audio_qc = Some(qc());
    r.validate().unwrap();
    r.export = Some(export());
    assert!(r.validate().is_err());
    assert!(export_request(&r).is_err());
    r.subject = FinalLosslessSubject::AcceptedPcm {
        file: "/synthetic/original.wav".into(),
        signal_run_directory: "/synthetic/signal".into(),
    };
    r.validate().unwrap();
    assert_eq!(
        export_request(&r).unwrap().configuration.format,
        native_export::LosslessFormat::Flac
    );
}
#[test]
fn audio_entry_refuses_video_routes_hidden_transform_ports_and_invalid_public_metadata() {
    let original = request();
    let mut r = original.clone();
    r.profile.destination = destination::Destination::Tiktok;
    assert!(r.validate().is_err());
    let mut r = original.clone();
    r.package.public_metadata.title.clear();
    assert!(r.validate().is_err());
    for field in ["normalize", "resample", "auto_review", "upload", "seal"] {
        let mut value = serde_json::to_value(&original).unwrap();
        value[field] = serde_json::json!(true);
        assert!(
            AudioPublishingRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err()
        );
    }
    assert!(PublishingRequest::from_json_slice(&serde_json::to_vec(&original).unwrap()).is_err());
}
#[test]
fn absent_or_partial_predecessors_create_no_native_child_review_package_or_seal() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut r = request();
    r.subject = FinalLosslessSubject::AcceptedPcm {
        file: root.join("original.wav"),
        signal_run_directory: root.join("absent-signal"),
    };
    let original = b"synthetic; not native accepted audio";
    std::fs::write(root.join("original.wav"), original).unwrap();
    let output = root.join("no-publishing");
    assert!(plan(&r, &CancellationToken::default()).is_err());
    assert!(run(r, &output, &CancellationToken::default(), |_, _| {}).is_err());
    assert!(!output.exists());
    assert!(status(&root, &CancellationToken::default()).is_err());
    assert_eq!(std::fs::read(root.join("original.wav")).unwrap(), original);
    assert!(!root.join("posting-review.json").exists());
    assert!(!root.join("package").exists());
    assert!(!root.join("retention-handoff.json").exists());
}
#[test]
fn supplied_audio_rejection_is_bound_to_the_whole_final_subject_and_context() {
    let plan = posting::PostingPlan {
        schema: "aniflow.posting-plan/v1".into(),
        request_sha256: "a".repeat(64),
        review_context_sha256: "b".repeat(64),
        review_subject_sha256: "c".repeat(64),
        media_filename: "audio.flac".into(),
        projected_package_bytes: 1000,
        metadata_scope: "synthetic".into(),
        publication_authority: "none".into(),
    };
    let mut supplied = ManualReview {
        schema: "aniflow.manual-review/v1".into(),
        context_sha256: plan.review_context_sha256.clone(),
        subject_sha256: plan.review_subject_sha256.clone(),
        decision: ReviewDecision::Reject,
        reviewer: "synthetic declaration".into(),
        notes: "fixture only; no actual review".into(),
    };
    require_review(&plan, &supplied).unwrap();
    supplied.subject_sha256 = "d".repeat(64);
    assert!(require_review(&plan, &supplied).is_err());
    supplied.subject_sha256 = plan.review_subject_sha256.clone();
    supplied.context_sha256 = "e".repeat(64);
    assert!(require_review(&plan, &supplied).is_err());
    let steps = [
        step(
            PublishingPhase::FinalReview,
            PublishingStepState::Rejected,
            None,
            None,
        ),
        step(
            PublishingPhase::ManualPackage,
            PublishingStepState::Pending,
            None,
            None,
        ),
    ];
    assert_eq!(
        super::super::readback::advise(&steps, Some(&plan)).action,
        "retain_rejection_start_new_intent"
    );
}

#[test]
fn audio_publishing_v2_selects_resumable_native_44100_qc_without_reinterpreting_v1() {
    let mut old = request();
    old.audio_qc = Some(qc());
    let mut new = old.clone();
    new.schema = "aniflow.audio-publishing-request/v2".into();
    new.validate().unwrap();
    assert_eq!(
        qc_request(&old, old.subject.clone()).unwrap().schema,
        "aniflow.final-lossless-audio-qc-request/v1"
    );
    let selected = qc_request(&new, new.subject.clone()).unwrap();
    assert_eq!(
        selected.schema,
        "aniflow.final-lossless-audio-qc-request/v2"
    );
    resumable_request(selected).validate().unwrap();
    assert_eq!(schema(&old, "intent"), "aniflow.audio-publishing-intent/v1");
    assert_eq!(
        schema(&new, "qc-binding"),
        "aniflow.audio-publishing-qc-binding/v2"
    );
    assert_eq!(phases(&old), phases(&new));
}
#[test]
fn audio_v2_cannot_skip_required_loudness_or_supply_automatic_review_or_recovery_override() {
    let mut r = request();
    r.schema = "aniflow.audio-publishing-request/v2".into();
    r.profile.require_loudness_evidence = true;
    assert!(r.validate().is_err());
    r.audio_qc = Some(qc());
    r.validate().unwrap();
    for field in ["adopt_partial", "replace_request", "auto_review", "upload"] {
        let mut value = serde_json::to_value(&r).unwrap();
        value[field] = serde_json::json!(true);
        assert!(
            AudioPublishingRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err()
        );
    }
}
