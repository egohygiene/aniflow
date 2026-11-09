//! Synthetic projection checks only, authored/uncompiled/unrun. No native run
//! or acceptance record is fabricated; project helper is not observe/recheck.
use super::*;
use crate::delivery_metadata::{self as metadata,remux,mp4};
use serde_json::{Value,json};
fn fixture()->(remux::RemuxReport,crate::ArtifactEvidence) {
    let document:Value=serde_json::from_slice(include_bytes!("../tests/fixtures/temporal/explicit-no-audio.json")).unwrap();
    let observations=document["observations"].as_array().unwrap().iter().map(|o|(o["stream_index"].as_u64().unwrap() as u32,o["frames"].clone(),o["packets"].clone())).collect::<Vec<_>>();
    let source=crate::temporal::from_probe_documents(&"a".repeat(64),100,&serde_json::from_value(document["selection"].clone()).unwrap(),&document["inventory"],&observations).unwrap();
    let settings=crate::video_assembly::AssemblyConfiguration::from_json_slice(&serde_json::to_vec(&json!({"schema":"aniflow.video-assembly-configuration/v1",
        "ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"b".repeat(64)},"ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"c".repeat(64)},
        "delivery_metadata":{"schema":"aniflow.delivery-metadata-policy/v1","mode":"minimal","approved":{}}})).unwrap()).unwrap();
    let policy=settings.delivery_metadata.as_ref().unwrap();let tags=metadata::inventory(&json!({"format":{},"streams":[]})).unwrap();
    let mut receipt=metadata::verify(policy,tags.clone(),tags,&"a".repeat(64),&"a".repeat(64),0,None).unwrap();receipt.public_summary.status=metadata::MetadataInspectionStatus::CleanWithinScope;
    let report=remux::RemuxReport {schema:"aniflow.metadata-remux-report/v1".into(),provider_lock_sha256:"d".repeat(64),
        operation:remux::RemuxOperation {schema:"aniflow.metadata-remux-operation/v1".into(),settings,settings_schema_sha256:crate::video_assembly::configuration_schema_reference().sha256,
            source_sha256:source.source_sha256.clone(),source_byte_count:100,selection:source.selection_intent.clone(),subtitle_preservation:None},
        media:remux::MetadataRemuxMedia {schema:"aniflow.metadata-remux-media/v1".into(),source:source.clone(),candidate:source,video_packets_sha256:"e".repeat(64),audio_packets_sha256:None,metadata:receipt,
            container:mp4::Mp4MetadataObservation {schema:"aniflow.mp4-metadata-observation/v1".into(),source_sha256:"a".repeat(64),box_count:3,mdta:Default::default(),unsupported_boxes:0,inspected_scope:"synthetic".into(),excluded_scope:"coded signal and signatures".into()},
            full_decode_observed:true,native_qualified:false,subtitle:None,playback:Some(remux::MetadataRemuxPlayback {video_codec:"h264".into(),pixel_format:"yuv420p".into(),color_primaries:"bt709".into(),color_transfer:"bt709".into(),color_matrix:"bt709".into(),color_range:"tv".into(),sample_aspect_ratio:"1/1".into(),orientation_degrees:0,faststart:true,audio:None})}};
    let artifact=crate::ArtifactEvidence {id:"metadata_delivery".into(),port:"media".into(),relative_path:Some("artifacts/metadata-remux/delivery.mp4".into()),artifact_type:"video/mp4".into(),artifact_role:crate::ArtifactRole::CandidateMaster,
        stream_role:Some(crate::StreamRole::Video),kind:crate::ArtifactKind::File,file_count:1,byte_count:100,sha256:"a".repeat(64)};(report,artifact)
}
fn project(report:&remux::RemuxReport,artifact:&crate::ArtifactEvidence)->Result<FinalVideoArtifact> {
    project_remux_properties(report,artifact,PathBuf::from("/synthetic/delivery.mp4"),&"b".repeat(64),&"c".repeat(64),&"d".repeat(64),&"e".repeat(64))
}
#[test]
fn native_remux_projection_is_one_destination_copy_with_preserved_exact_clock() {
    let (report,artifact)=fixture();let projected=project(&report,&artifact).unwrap();assert_eq!(projected.variant,FinalVideoVariant::DestinationCopy);
    assert_eq!(projected.artifact_id,"metadata_delivery");assert_eq!(projected.video_duration,report.media.candidate.video_timeline().unwrap().source_time.unwrap().duration().unwrap());
    assert_eq!(projected.metadata.unwrap().status,metadata::MetadataInspectionStatus::CleanWithinScope);
}
#[test]
fn square_pixel_spellings_bridge_assembly_and_native_probe_without_admitting_anamorphic() {
    let (mut report,artifact)=fixture();report.media.playback.as_mut().unwrap().sample_aspect_ratio="1:1".into();assert!(project(&report,&artifact).is_ok());
    report.media.playback.as_mut().unwrap().sample_aspect_ratio="4:3".into();assert!(project(&report,&artifact).is_err());assert!(square_pixels("1/1"));assert!(square_pixels("1:1"));assert!(!square_pixels("unknown"));
}
#[test]
fn old_missing_playback_and_inconsistent_audio_facts_refuse_projection() {
    let (mut report,artifact)=fixture();let playback=report.media.playback.take();assert!(project(&report,&artifact).is_err());
    report.media.playback=playback;report.media.audio_packets_sha256=Some("f".repeat(64));assert!(project(&report,&artifact).is_err());
}
#[test]
fn unsupported_metadata_and_changed_artifact_bytes_cannot_be_projected_as_current() {
    let (mut report,mut artifact)=fixture();report.media.metadata.public_summary.status=metadata::MetadataInspectionStatus::Unsupported;assert!(project(&report,&artifact).is_err());
    report.media.metadata.public_summary.status=metadata::MetadataInspectionStatus::CleanWithinScope;artifact.sha256="f".repeat(64);assert!(project(&report,&artifact).is_err());
}
