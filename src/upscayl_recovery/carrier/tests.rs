//! Synthetic relationship/refusal cases, authored uncompiled/unrun.
use super::*;
fn index()->RifeFrameIndex {
    let document:serde_json::Value=serde_json::from_slice(include_bytes!("../../../tests/fixtures/temporal/explicit-no-audio.json")).unwrap();
    let observations=document["observations"].as_array().unwrap().iter().map(|v|(v["stream_index"].as_u64().unwrap() as u32,v["frames"].clone(),v["packets"].clone())).collect::<Vec<_>>();
    let temporal=crate::temporal::from_probe_documents(&"a".repeat(64),100,&serde_json::from_value(document["selection"].clone()).unwrap(),&document["inventory"],&observations).unwrap();
    let count=temporal.video_timeline().unwrap().frames.len();
    RifeFrameIndex {schema:crate::video_interpolation::RIFE_FRAME_INDEX_SCHEMA_V1.into(),temporal_inspection:temporal,width:1920,height:1080,frame_profile:"rgb8_sdr_bt709_square_unrotated".into(),
        frames:(0..count).map(|i|RifeFrame {ordinal:i as u64,filename:format!("frame_{i:08}.png"),sha256:"b".repeat(64)}).collect(),cut_before_ordinals:vec![]}
}
fn frame(index:&RifeFrameIndex,ordinal:usize)->SelectedSpatialFrame {
    SelectedSpatialFrame {ordinal:ordinal as u64,filename:index.frames[ordinal].filename.clone(),sha256:"c".repeat(64),byte_count:100,decoded_rgb24_sha256:"d".repeat(64),
        source_time:index.temporal_inspection.video_timeline().unwrap().frames[ordinal].source_time,input_file:format!("/synthetic/frame-{ordinal}.png").into(),origin_index:0}
}
fn request()->CarrierRequest {CarrierRequest {schema:"aniflow.spatial-carrier-request/v1".into(),source_video:"/synthetic/source.mp4".into(),source_index:"/synthetic/index.json".into(),sources:vec![],output_width:3840,output_height:2160,
    maximum_source_bytes:1_048_576,maximum_frame_bytes:1_048_576,maximum_total_frame_bytes:16_777_216,maximum_working_bytes:134_217_728,minimum_free_bytes:0}}
#[test]
fn coverage_exposes_missing_ordinals_without_substituting_or_renumbering() {
    let index=index();let frames=(0..index.frames.len()).map(|i|frame(&index,i)).collect::<Vec<_>>();assert!(coverage(&index,&frames).unwrap().is_empty());
    assert_eq!(coverage(&index,&frames[1..]).unwrap(),vec![0]);
}
#[test]
fn duplicate_renamed_or_out_of_range_frames_cannot_complete_a_carrier() {
    let index=index();let original=frame(&index,0);assert!(coverage(&index,&[original.clone(),original.clone()]).is_err());
    let mut changed=original.clone();changed.filename="other.png".into();assert!(coverage(&index,&[changed]).is_err());
    let mut changed=original;changed.ordinal=index.frames.len() as u64;assert!(coverage(&index,&[changed]).is_err());
}
#[test]
fn exact_source_intervals_cannot_be_shifted_by_one_tick() {
    let index=index();let mut changed=frame(&index,0);changed.source_time.start=changed.source_time.start.checked_add(crate::temporal::RationalTime::new(1,1000).unwrap()).unwrap();
    assert!(coverage(&index,&[changed]).is_err());
}
#[test]
fn finite_storage_and_strict_selected_ordinals_are_required() {
    let mut request=request();request.validate().unwrap();request.maximum_frame_bytes=0;assert!(request.validate().is_err());
    assert!(ordinals(&[],128).is_err());assert!(ordinals(&[4,4],128).is_err());assert!(ordinals(&[4,3],128).is_err());
    let mut value=serde_json::to_value(self::request()).unwrap();value["adopt_unvalidated"]=serde_json::json!(true);assert!(CarrierRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn loose_reports_have_no_native_import_authority() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();std::fs::write(root.join("report.json"),b"{}").unwrap();
    let selected=SpatialSelection::Historical {run_directory:root.clone(),ordinals:vec![0]};
    assert!(origins::append(&selected,&request(),&index(),&("e".repeat(64),100),&mut Vec::new(),&mut Vec::new(),&CancellationToken::default()).is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(),1);
}
#[test]
fn missing_completion_marker_is_not_adopted_and_observation_writes_nothing() {
    let temp=tempfile::tempdir().unwrap();let root=temp.path().canonicalize().unwrap();assert!(observe(&root,&"a".repeat(64),&CancellationToken::default()).is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
}
