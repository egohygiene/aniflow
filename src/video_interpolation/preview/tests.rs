//! Synthetic/pure source regressions, authored and uncompiled/unrun.
use super::*;
use crate::CancellationToken;
pub(super) fn fixture()->(tempfile::TempDir,PreviewRequest) {
    let (root,run)=super::super::provider::runtime_fixture();
    (root,PreviewRequest::new(run,PreviewRange {first_source_ordinal:1,end_source_ordinal:2}))
}
pub(super) fn operation(request:&PreviewRequest)->PreviewConfiguration {PreviewConfiguration {schema:CONFIGURATION.into(),configuration:request.run.configuration.clone(),range:request.range,frames_directory:request.run.frames_directory.clone(),frame_index:request.run.frame_index.clone()}}
pub(super) fn original(request:&PreviewRequest)->RifeFrameIndex {super::super::provider::read_index(&request.run.frame_index).unwrap().0}
#[test]
fn plan_bounds_only_selected_native_work_and_never_launches_the_model() {
    let (_root,request)=fixture();let planned=plan(&request,&CancellationToken::default()).unwrap();
    assert_eq!(planned.payload.pipeline_name,"bounded-rife-preview");assert_eq!(planned.payload.stages.len(),1);assert_eq!(planned.payload.stages[0].capability.id,CAPABILITY_ID);
    assert!(planned.payload.inputs.iter().any(|i|i.id=="source_frames"&&i.file_count==2));
    let (_,window,frames)=operation(&request).mapping(&original(&request)).unwrap();
    assert_eq!(window.start,RationalTime::new(1,24).unwrap());assert_eq!(window.end,RationalTime::new(1,12).unwrap());
    assert_eq!(frames.iter().map(|f|f.frame.ordinal).collect::<Vec<_>>(),vec![2,3]);assert_eq!(frames[1].frame.origin,RifeFrameOrigin::EndpointHold);
    assert_eq!(frames[0].presentation.start,window.start);assert_eq!(frames[1].presentation.end,window.end);
    let mut marker=request.run.configuration.settings.tool.executable.as_os_str().to_os_string();marker.push(".launched");assert!(!PathBuf::from(marker).exists());
}
#[test]
fn shot_cut_and_right_context_mapping_match_the_full_original_plan() {
    let (_root,request)=fixture();let mut op=operation(&request);op.range=PreviewRange {first_source_ordinal:0,end_source_ordinal:1};let mut index=original(&request);
    let (_,_,frames)=op.mapping(&index).unwrap();assert_eq!(frames[1].frame.source_right_ordinal,1);assert_eq!(frames[1].frame.origin,RifeFrameOrigin::InterpolatedMidpoint);
    index.cut_before_ordinals=vec![1];let (_,_,frames)=op.mapping(&index).unwrap();assert_eq!(frames[1].frame.source_right_ordinal,0);assert_eq!(frames[1].frame.origin,RifeFrameOrigin::CutHold);
}
#[test]
fn empty_oversized_outside_and_disabled_ranges_cannot_become_previews() {
    let (_root,request)=fixture();let index=original(&request);let mut op=operation(&request);
    for range in [PreviewRange {first_source_ordinal:1,end_source_ordinal:1},PreviewRange {first_source_ordinal:0,end_source_ordinal:121},PreviewRange {first_source_ordinal:1,end_source_ordinal:3}] {op.range=range;assert!(op.mapping(&index).is_err());}
    op=operation(&request);op.configuration.enabled=false;assert!(op.mapping(&index).is_err());
}
#[test]
fn exact_clock_bound_refuses_more_than_ten_seconds_even_with_few_frames() {
    let (_root,request)=fixture();let mut index=original(&request);
    let inventory=serde_json::json!({"streams":[{"index":0,"codec_type":"video","codec_name":"h264","time_base":"10/1","start_pts":0,"duration_ts":2,"r_frame_rate":"1/10","avg_frame_rate":"1/10","width":1,"height":1,"pix_fmt":"yuv420p","disposition":{"attached_pic":0}}]});
    let frames=serde_json::json!({"frames":[{"stream_index":0,"pts":0,"pkt_dts":0,"pkt_duration":1},{"stream_index":0,"pts":1,"pkt_dts":1,"pkt_duration":1}]});
    let packets=serde_json::json!({"packets":[{"stream_index":0,"pts":0,"dts":0,"duration":1},{"stream_index":0,"pts":1,"dts":1,"duration":1}]});
    index.temporal_inspection=crate::temporal::from_probe_documents(&"a".repeat(64),100,&crate::temporal::StreamSelection::default(),&inventory,&[(0,frames,packets)]).unwrap();
    let mut op=operation(&request);assert!(op.mapping(&index).is_ok());op.range.first_source_ordinal=0;assert!(op.mapping(&index).is_err());
}
#[test]
fn preview_report_is_not_a_full_output_index_and_rejects_changed_global_mapping() {
    let (_root,request)=fixture();let index=original(&request);let op=operation(&request);
    let mut report=report_template(&op,&index,&("a".repeat(64),100),&"b".repeat(64),&"c".repeat(64)).unwrap();
    for frame in &mut report.frames {frame.frame.sha256=index.frames[frame.frame.source_left_ordinal as usize].sha256.clone();}
    report.validate_against(&index).unwrap();assert!(RifeOutputIndex::from_json_slice(&crate::provider::canonical_json_bytes(&report).unwrap()).is_err());
    report.frames[0].frame.ordinal=0;assert!(report.validate_against(&index).is_err());
}
#[test]
fn changed_original_png_or_model_refuses_without_native_launch() {
    let (_root,request)=fixture();std::fs::write(request.run.frames_directory.join("source_00000000.png"),b"changed").unwrap();assert!(plan(&request,&CancellationToken::default()).is_err());
    let (_root,request)=fixture();std::fs::write(request.run.configuration.settings.model.directory.join("flownet.bin"),b"changed").unwrap();assert!(plan(&request,&CancellationToken::default()).is_err());
}
#[test]
fn cancelled_admission_and_loose_report_never_create_acceptance_or_repair_files() {
    let (root,request)=fixture();let token=CancellationToken::default();token.cancel();assert!(plan(&request,&token).is_err());
    let loose=root.path().canonicalize().unwrap().join("loose");std::fs::create_dir(&loose).unwrap();std::fs::write(loose.join("report.json"),b"{}").unwrap();
    assert!(observe(&loose,&CancellationToken::default()).is_err());assert!(status(&loose).is_err());assert_eq!(std::fs::read_dir(loose).unwrap().count(),1);
}
