//! Pure refusal fixtures; execution coverage is authored in audio_signal tests.
use super::*;
fn request() -> ResumableQcRequest {
    serde_json::from_value(serde_json::json!({"schema":"aniflow.resumable-final-audio-qc-request/v1","qc":{
        "schema":"aniflow.final-lossless-audio-qc-request/v2","subject":{"kind":"accepted_pcm","file":"/synthetic/source.wav","signal_run_directory":"/synthetic/absent-native"},
        "configuration":{"schema":"aniflow.audio-inspection.configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},"tool_timeout_milliseconds":1000,"maximum_tool_output_bytes":4096},
        "provider_executable":"/synthetic/provider","maximum_pcm_bytes":1048576,"maximum_working_bytes":1073741824,"minimum_free_bytes":0}})).unwrap()
}
#[test]
fn explicit_resumable_lane_refuses_legacy_meter_selection_and_hidden_retry_transforms() {
    let original = request();
    original.validate().unwrap();
    let mut old = original.clone();
    old.qc.schema = "aniflow.final-lossless-audio-qc-request/v1".into();
    assert!(old.validate().is_err());
    for field in ["adopt_partial", "overwrite", "resample", "normalize"] {
        let mut value = serde_json::to_value(&original).unwrap();
        value[field] = serde_json::json!(true);
        assert!(ResumableQcRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
#[test]
fn unknown_legacy_partial_roots_are_retained_and_never_promoted_or_overwritten() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(root.join("intent.json"), b"legacy partial intent").unwrap();
    std::fs::write(root.join("decoded.wav"), b"retained partial bytes").unwrap();
    assert!(!recognized(&root).unwrap());
    assert!(resume(&root, &CancellationToken::default(), |_| {}).is_err());
    assert!(status(&root, &CancellationToken::default()).is_err());
    assert_eq!(
        std::fs::read(root.join("decoded.wav")).unwrap(),
        b"retained partial bytes"
    );
    assert!(!root.join("resume-intent.json").exists());
    assert!(!root.join("qc.json").exists());
}
#[cfg(unix)]
#[test]
fn symbolic_transition_marker_cannot_select_the_resumable_observer() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(root.join("other.json"), b"{}").unwrap();
    std::os::unix::fs::symlink(root.join("other.json"), root.join("resume-intent.json")).unwrap();
    assert!(recognized(&root).is_err());
    assert!(
        super::super::observe(&root, &local::hash(b"{}"), &CancellationToken::default()).is_err()
    );
    assert_eq!(std::fs::read(root.join("other.json")).unwrap(), b"{}");
}
