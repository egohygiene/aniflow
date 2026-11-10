//! Synthetic mapping/refusal only; no native accepted origin is manufactured.
use super::*;
use crate::upscayl_recovery::carrier::*;
pub(crate) fn fixture()->(tempfile::TempDir,RifeConfiguration,CompleteSpatialCarrier) {
    let (root,request)=super::super::provider::runtime_fixture();let original=super::super::provider::read_index(&request.frame_index).unwrap().0;
    let frames=original.frames.iter().map(|f|SelectedSpatialFrame {ordinal:f.ordinal,filename:f.filename.clone(),sha256:"e".repeat(64),byte_count:100,decoded_rgb24_sha256:"f".repeat(64),source_time:original.temporal_inspection.video_timeline().unwrap().frames[f.ordinal as usize].source_time,input_file:format!("/synthetic/old/{}",f.filename).into(),origin_index:0}).collect::<Vec<_>>();
    let selection=SpatialSelectionPlan {schema:"aniflow.spatial-selection-plan/v1".into(),request:CarrierRequest {schema:"aniflow.spatial-carrier-request/v1".into(),source_video:"/synthetic/source.mp4".into(),source_index:request.frame_index, sources:vec![],output_width:original.width*2,output_height:original.height*2,maximum_source_bytes:1048576,maximum_frame_bytes:1048576,maximum_total_frame_bytes:16777216,maximum_working_bytes:1073741824,minimum_free_bytes:0},source_index_sha256:"a".repeat(64),source_index_bytes:100,source_index:original,
        origins:vec![SpatialOrigin {kind:SpatialOriginKind::ReviewedHistoricalImport,run_directory:"/synthetic/import".into(),stage_id:"import".into(),plan_sha256:"b".repeat(64),checkpoint_sha256:"c".repeat(64),provider_lock_sha256:"d".repeat(64),report_sha256:"e".repeat(64),import_decision_sha256:Some("f".repeat(64)),processing_history:"unknown historical tool/model/execution".into()}],total_bytes:frames.len() as u64*100,frames,missing:vec![],complete:true};
    (root,request.configuration,CompleteSpatialCarrier {schema:SCHEMA.into(),selection,frames_directory:"/synthetic/carrier/frames".into(),creative_approval:"not_performed; individual import declarations do not approve complete movie".into(),native_qualified:false})
}
fn output(configuration:&RifeConfiguration,carrier:&CompleteSpatialCarrier)->RifeOutputIndex {
    let plan=plan(configuration,carrier).unwrap();let mut frames=super::super::provider::output_frame_template(&plan).unwrap();for f in &mut frames {f.sha256=if f.origin==RifeFrameOrigin::InterpolatedMidpoint {"9".repeat(64)}else{carrier.selection.frames[f.source_left_ordinal as usize].sha256.clone()};}RifeOutputIndex {schema:OUTPUT_SCHEMA.into(),plan,frames}
}
#[test]
fn recovered_plan_preserves_original_clock_and_unknown_history_with_actual_derived_geometry() {
    let (_root,c,carrier)=fixture();let before=carrier.clone();let p=plan(&c,&carrier).unwrap();let original=super::super::plan(&c,&carrier.selection.source_index).unwrap();assert_eq!(carrier,before);assert_eq!(p.duration,original.duration);assert_eq!(p.output_frame_rate,original.output_frame_rate);assert_eq!(p.frame_index_sha256,canonical_sha256(&carrier).unwrap());assert!(carrier.selection.origins[0].processing_history.contains("unknown"));
}
#[test]
fn held_frames_bind_recovered_pixels_instead_of_original_source_pngs() {
    let (_root,c,carrier)=fixture();let mut out=output(&c,&carrier);validate_output(&out,&c,&carrier).unwrap();assert!(out.validate_against(&c,&carrier.selection.source_index).is_err());out.frames[0].sha256=carrier.selection.source_index.frames[0].sha256.clone();assert!(validate_output(&out,&c,&carrier).is_err());
}
#[test]
fn incomplete_or_reordered_spatial_coverage_cannot_enter_rife() {
    let (_root,c,mut carrier)=fixture();carrier.selection.complete=false;assert!(plan(&c,&carrier).is_err());carrier.selection.complete=true;carrier.selection.frames.remove(0);assert!(plan(&c,&carrier).is_err());
}
#[test]
fn recovered_rife_assembly_needs_v2_and_never_adopts_loose_native_evidence() {
    let (_root,c,carrier)=fixture();let input=RecoveredInput {binding:RecoveredBinding {carrier_directory:"/synthetic/carrier".into(),carrier_sha256:"a".repeat(64)},carrier:carrier.clone()};
    let mut assembly=crate::video_assembly::rife::RifeAssemblyIndex {schema:crate::video_assembly::rife::RIFE_ASSEMBLY_INDEX_SCHEMA_V2.into(),accepted_run:"/synthetic/rife".into(),accepted_manifest_sha256:"b".repeat(64),source_index_sha256:"a".repeat(64),configuration:c.clone(),source_index:carrier.selection.source_index.clone(),output_index:output(&c,&carrier),spatial_input:None,recovered_input:Some(input)};
    assembly.clock().unwrap();assert_eq!(assembly.dimensions(),(carrier.selection.request.output_width,carrier.selection.request.output_height));assert!(assembly.validate_binding().is_err());assembly.schema=crate::video_assembly::rife::RIFE_ASSEMBLY_INDEX_SCHEMA_V1.into();assert!(assembly.clock().is_err());
}
#[test]
fn unknown_request_fields_and_absent_carrier_refuse_without_creating_native_work() {
    let (_fixture,c,_carrier)=fixture();let root=tempfile::tempdir().unwrap();let root=root.path().canonicalize().unwrap();let request=RecoveredRifeRequest {schema:"aniflow.recovered-rife-request/v1".into(),source:RecoveredBinding {carrier_directory:root.clone(),carrier_sha256:"a".repeat(64)},configuration:c,provider_executable:"/synthetic/provider".into()};request.validate().unwrap();
    assert!(runtime::plan(&request,&CancellationToken::default()).is_err());let out=root.join("no-run");assert!(runtime::run(request.clone(),&out,&CancellationToken::default(),|_|{}).is_err());assert!(!out.exists());let mut value=serde_json::to_value(request).unwrap();value["adopt_files"]=serde_json::json!(true);assert!(RecoveredRifeRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn original_spatial_wrapper_and_recovered_wrapper_are_distinct_contracts() {
    let (_root,c,_carrier)=fixture();let wrapper=Wrapper {schema:"aniflow.recovered-rife-configuration/v1".into(),configuration:c,recovered:RecoveredBinding {carrier_directory:"/synthetic/carrier".into(),carrier_sha256:"a".repeat(64)}};
    let config=wrapper.provider_configuration().unwrap();assert_eq!(config.provider.id,PROVIDER_ID);assert_ne!(config.provider.id,crate::video_interpolation::spatial::PROVIDER_ID);assert!(serde_json::from_value::<crate::video_interpolation::spatial::Wrapper>(serde_json::to_value(wrapper).unwrap()).is_err());
}
