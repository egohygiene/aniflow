use super::*;
#[test]
fn sparse_batches_keep_original_ordinals_and_never_include_selected_reuse() {
    let missing = vec![1, 3, 5601, 5602, 99999];
    assert_eq!(
        batches(&missing, 2).unwrap(),
        vec![vec![1, 3], vec![5601, 5602], vec![99999]]
    );
    assert!(batches(&[], 128).unwrap().is_empty());
}
#[test]
fn unordered_duplicate_unbounded_or_oversized_missing_work_refuses() {
    for (values, size) in [
        (vec![2, 1], 2),
        (vec![2, 2], 2),
        (vec![100000], 2),
        (vec![1], 0),
        (vec![1], 129),
    ] {
        assert!(batches(&values, size).is_err());
    }
}
#[test]
fn readback_refuses_incomplete_parent_and_creates_no_child_or_completion() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    assert!(status(&root, &CancellationToken::default()).is_err());
    assert!(completion(&root, &CancellationToken::default()).is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
}
#[test]
fn recovered_slots_distinguish_files_directories_and_missing_state() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = root.join("intent.json");
    assert!(!present(&path, false).unwrap());
    std::fs::write(&path, b"{}").unwrap();
    assert!(present(&path, false).unwrap());
    assert!(present(&path, true).is_err());
    assert!(present(&root, true).unwrap());
    assert!(present(&root, false).is_err());
}
#[test]
fn recovered_assembly_has_its_own_operation_schema_and_keeps_v1_meaning() {
    let mut op:crate::video_assembly::AssemblyOperation=serde_json::from_value(serde_json::json!({"schema":"aniflow.video-assembly-operation/v2","stage":"master",
        "settings":{"schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)}},
        "source_sha256":"c".repeat(64),"source_byte_count":1000,"source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]},
        "frame_index_sha256":"d".repeat(64),"index_schema":carrier::SCHEMA,"frame_count":2,"width":4,"height":4})).unwrap();
    op.validate().unwrap();
    assert_eq!(
        op.provider_configuration()
            .unwrap()
            .configuration_schema
            .version,
        "2.0.0"
    );
    op.schema = crate::video_assembly::ASSEMBLY_OPERATION_SCHEMA_V1.into();
    assert!(op.validate().is_err());
    op.index_schema = crate::video_interpolation::RIFE_FRAME_INDEX_SCHEMA_V1.into();
    op.validate().unwrap();
    assert_eq!(
        op.provider_configuration().unwrap().configuration_schema,
        crate::video_assembly::operation_schema_reference()
    );
}
