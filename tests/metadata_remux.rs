//! Synthetic authored regressions only; no native tool/model/media invocation.
use aniflow::{CancellationToken,delivery_metadata::{mp4,MetadataInventory}};
use std::collections::BTreeMap;
fn atom(kind:[u8;4],payload:Vec<u8>)->Vec<u8> {
    let mut v=((payload.len()+8) as u32).to_be_bytes().to_vec();v.extend(kind);v.extend(payload);v
}
fn subject(extra:Vec<u8>)->Vec<u8> {
    let mut value=atom(*b"ftyp",b"isom0000".to_vec());value.extend(atom(*b"moov",extra));value.extend(atom(*b"mdat",vec![0;4]));value
}
fn mdta(key:&str,value:&str)->Vec<u8> {
    let mut handler=vec![0;8];handler.extend(b"mdta");handler.extend([0;13]);
    let mut keys=vec![0;4];keys.extend(1u32.to_be_bytes());keys.extend(((key.len()+8) as u32).to_be_bytes());keys.extend(b"mdta");keys.extend(key.as_bytes());
    let mut data=1u32.to_be_bytes().to_vec();data.extend([0;4]);data.extend(value.as_bytes());
    let mut meta=vec![0;4];meta.extend(atom(*b"hdlr",handler));meta.extend(atom(*b"keys",keys));meta.extend(atom(*b"ilst",atom(1u32.to_be_bytes(),atom(*b"data",data))));
    atom(*b"udta",atom(*b"meta",meta))
}
fn inspect(bytes:&[u8])->aniflow::Result<mp4::Mp4MetadataObservation> {
    let root=tempfile::tempdir().unwrap();let path=root.path().canonicalize().unwrap().join("shape.mp4");std::fs::write(&path,bytes).unwrap();
    mp4::inspect(&path,2_000_000,&CancellationToken::default())
}
#[test]
fn exact_unicode_mdta_matches_visible_fields_without_exposing_them_in_public_summary() {
    let observation=inspect(&subject(mdta("artist","音楽 — operator credit"))).unwrap();
    let tags=MetadataInventory {global:BTreeMap::from([("artist".into(),"音楽 — operator credit".into()),("major_brand".into(),"isom".into())]),streams:BTreeMap::new(),chapters:vec![],programs:vec![]};
    mp4::agrees(&observation,&tags).unwrap();
    assert!(observation.excluded_scope.contains("signatures"));
    let mut changed=tags;changed.global.insert("artist".into(),"tampered".into());assert!(mp4::agrees(&observation,&changed).is_err());
}
#[test]
fn unknown_uuid_and_nonzero_padding_cannot_claim_inspected_clean_scope() {
    for extra in [atom(*b"uuid",vec![42;16]),atom(*b"free",vec![1;16])] {
        let observation=inspect(&subject(extra)).unwrap();assert_eq!(observation.unsupported_boxes,1);
        let tags=MetadataInventory {global:BTreeMap::new(),streams:BTreeMap::new(),chapters:vec![],programs:vec![]};assert!(mp4::agrees(&observation,&tags).is_err());
    }
}
#[test]
fn truncated_open_ended_and_over_budget_boxes_are_refused() {
    let mut truncated=subject(mdta("title","release"));truncated.pop();assert!(inspect(&truncated).is_err());
    let mut unbounded=subject(vec![]);unbounded[0..4].fill(0);assert!(inspect(&unbounded).is_err());
    assert!(inspect(&subject(atom(*b"free",vec![0;1_048_577]))).is_err());
}
#[test]
fn duplicate_meta_and_cancelled_inspection_never_become_accepted_receipts() {
    let mut duplicate=mdta("title","one");duplicate.extend(mdta("title","two"));assert!(inspect(&subject(duplicate)).is_err());
    let root=tempfile::tempdir().unwrap();let path=root.path().canonicalize().unwrap().join("shape.mp4");std::fs::write(&path,subject(vec![])).unwrap();
    let token=CancellationToken::default();token.cancel();assert!(mp4::inspect(&path,4096,&token).is_err());
}
#[test]
fn remux_operation_requires_selected_policy_and_exact_settings_schema_pin() {
    use aniflow::delivery_metadata::remux::RemuxOperation;
    let settings=aniflow::video_assembly::AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&serde_json::json!({
        "schema":"aniflow.video-assembly-configuration/v1","ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},
        "ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},
        "delivery_metadata":{"schema":"aniflow.delivery-metadata-policy/v1","mode":"minimal","approved":{}}})).unwrap()).unwrap();
    let mut operation=RemuxOperation {schema:"aniflow.metadata-remux-operation/v1".into(),settings,settings_schema_sha256:aniflow::video_assembly::configuration_schema_reference().sha256,
        source_sha256:"c".repeat(64),source_byte_count:100,selection:aniflow::temporal::StreamSelection{video_stream:Some(0),audio_stream:None,no_audio:true,discard_streams:vec![],audio_extent:Default::default()},subtitle_preservation:None};
    operation.validate().unwrap();operation.settings_schema_sha256="d".repeat(64);assert!(operation.validate().is_err());
    operation.settings_schema_sha256=aniflow::video_assembly::configuration_schema_reference().sha256;operation.settings.delivery_metadata=None;assert!(operation.validate().is_err());
}
