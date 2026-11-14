//! Synthetic source/refusal cases, authored uncompiled/unrun.
use super::*;
fn request() -> Request {
    let configuration=serde_json::from_value(serde_json::json!({"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)}})).unwrap();
    Request {
        schema: "aniflow.resumable-final-video-audio-qc-request/v1".into(),
        qc: FinalAudioQcRequest {
            schema: "aniflow.final-audio-qc-request/v1".into(),
            source_run_directory: "/synthetic/absent-native".into(),
            artifact_id: "delivery".into(),
            configuration,
            provider_executable: "/synthetic/provider".into(),
            maximum_pcm_bytes: 268435456,
        },
    }
}
#[test]
fn explicit_wrapper_refuses_implicit_upgrade_and_transform_or_overwrite_ports() {
    let r = request();
    r.validate().unwrap();
    let mut old = r.clone();
    old.schema = "aniflow.final-audio-qc-request/v1".into();
    assert!(old.validate().is_err());
    for key in ["adopt_partial", "overwrite", "normalize", "resample"] {
        let mut value = serde_json::to_value(&r).unwrap();
        value[key] = serde_json::json!(true);
        assert!(Request::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
#[test]
fn absent_native_producer_creates_no_capture_decode_or_native_run() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap().join("qc");
    let r = request();
    assert!(plan(&r, &CancellationToken::new()).is_err());
    assert!(run(&r, &root, &CancellationToken::new(), |_| {}).is_err());
    assert!(!root.exists());
}
#[test]
fn legacy_partial_state_is_preserved_without_inferred_completion_or_rewrite() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    std::fs::write(root.join("intent.json"), b"legacy").unwrap();
    std::fs::write(root.join("decoded.wav"), b"partial native bytes").unwrap();
    assert!(!recognized(&root).unwrap());
    assert!(resume(&root, &CancellationToken::new(), |_| {}).is_err());
    assert!(status(&root, &CancellationToken::new()).is_err());
    assert_eq!(
        std::fs::read(root.join("decoded.wav")).unwrap(),
        b"partial native bytes"
    );
    assert!(!root.join("qc.json").exists());
}
#[cfg(unix)]
#[test]
fn symbolic_resume_marker_never_selects_a_different_private_evidence_root() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    std::fs::write(root.join("other.json"), b"{}").unwrap();
    std::os::unix::fs::symlink(root.join("other.json"), root.join("resume-intent.json")).unwrap();
    assert!(recognized(&root).is_err());
}
#[test]
fn changed_container_bytes_or_audio_payload_always_change_the_bound_native_subject() {
    let a = super::super::tests::completion();
    let original = canonical_sha256(&a).unwrap();
    for audio in [false, true] {
        let mut b = a.clone();
        if audio {
            b.artifacts[0]
                .audio
                .as_mut()
                .unwrap()
                .packet_configuration_timing_sha256 = "5".repeat(64);
        } else {
            b.artifacts[0].sha256 = "6".repeat(64);
        }
        assert_ne!(canonical_sha256(&b).unwrap(), original);
    }
}
#[test]
fn complete_decoded_extent_keeps_rational_offset_and_rejects_audio_gaps() {
    let doc: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../tests/fixtures/temporal/synchronized-audio-video.json"
    ))
    .unwrap();
    let selection = serde_json::from_value(doc["selection"].clone()).unwrap();
    let observations = doc["observations"]
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
    let mut inspection = crate::temporal::from_probe_documents(
        &"a".repeat(64),
        1,
        &selection,
        &doc["inventory"],
        &observations,
    )
    .unwrap();
    let (stream, range, count) = source_signal::extent(&inspection).unwrap();
    assert_eq!(count, 8008);
    assert_eq!(
        range,
        inspection.timeline(stream).unwrap().source_time.unwrap()
    );
    inspection
        .timelines
        .iter_mut()
        .find(|t| t.stream_index == stream)
        .unwrap()
        .frames[0]
        .source_time
        .start = range
        .start
        .checked_add(crate::temporal::RationalTime::new(1, 48000).unwrap())
        .unwrap();
    assert!(source_signal::extent(&inspection).is_err());
}
