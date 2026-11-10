//! Synthetic relationship/refusal cases only; no claimed native acceptance.
use super::*;
pub(super) fn completion()->FinalVideoHandoff {
    serde_json::from_value(serde_json::json!({
        "schema":"aniflow.final-video-handoff/v1","completion_id":"a".repeat(64),"plan_sha256":"b".repeat(64),
        "run_manifest_sha256":"c".repeat(64),"delivery_acceptance_sha256":"d".repeat(64),"source_lineage":[],
        "creative_approval":"not_performed","publication_authority":"none","proof_state":"not_asserted",
        "artifacts":[{"artifact_id":"delivery","variant":"destination_copy","locator":"/synthetic/delivery.mp4","sha256":"e".repeat(64),"byte_count":1000,
            "width":1920,"height":1080,"frame_count":30,"frame_rate":{"numerator":30,"denominator":1},
            "video_interval":{"start":{"numerator":0,"denominator":1},"end":{"numerator":1,"denominator":1}},"video_duration":{"numerator":1,"denominator":1},
            "video_codec":"h264","pixel_format":"yuv420p","color_primaries":"bt709","color_transfer":"bt709","color_matrix":"bt709","color_range":"tv","sample_aspect_ratio":"1:1","orientation_degrees":0,
            "audio":{"codec":"aac","profile":"LC","sample_rate_hz":48000,"channels":2,"channel_layout":"stereo",
                "interval":{"start":{"numerator":0,"denominator":1},"end":{"numerator":1,"denominator":1}},"packet_configuration_timing_sha256":"f".repeat(64)},
            "metadata":null,"producer_lock_sha256":"1".repeat(64),"stage_invocation_sha256":"2".repeat(64),"stage_checkpoint_sha256":"3".repeat(64),"private_report_sha256":"4".repeat(64)}]
    })).unwrap()
}
#[test]
fn final_qc_refuses_missing_audio_and_unknown_artifact_instead_of_inventing_silence() {
    let mut subject=completion();assert!(selected(&subject,"delivery").is_ok());assert!(selected(&subject,"other").is_err());
    subject.artifacts[0].audio=None;assert!(selected(&subject,"delivery").is_err());
}
#[test]
fn unsupported_rate_channels_and_soft_subtitles_stay_explicit() {
    let subject=completion();
    for (rate,channels) in [(44_100,2),(48_000,6)] {
        let mut changed=subject.clone();let audio=changed.artifacts[0].audio.as_mut().unwrap();audio.sample_rate_hz=rate;audio.channels=channels;
        assert!(selected(&changed,"delivery").is_err());
    }
    let mut changed=subject;changed.artifacts[0].variant=FinalVideoVariant::SubtitleMuxedCandidate;assert!(selected(&changed,"delivery").is_err());
}
#[test]
fn entire_file_and_native_audio_packet_identity_are_both_part_of_qc_subject() {
    let subject=completion();let before=canonical_sha256(&selected(&subject,"delivery").unwrap()).unwrap();
    let mut metadata_change=subject.clone();metadata_change.artifacts[0].sha256="5".repeat(64);
    assert_ne!(before,canonical_sha256(&selected(&metadata_change,"delivery").unwrap()).unwrap());
    let mut packet_change=subject;packet_change.artifacts[0].audio.as_mut().unwrap().packet_configuration_timing_sha256="6".repeat(64);
    assert_ne!(before,canonical_sha256(&selected(&packet_change,"delivery").unwrap()).unwrap());
}
#[test]
fn incomplete_root_and_unpinned_receipt_are_never_adopted() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();
    assert!(observe(&root,&"a".repeat(64),&CancellationToken::new()).is_err());
    assert!(observe(&root,"",&CancellationToken::new()).is_err());
}
