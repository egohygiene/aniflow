//! Read-only completion projection over existing fixed-profile v3 acceptance.
//! No downstream provider selection, execution, upload or publication authority.
use std::io::Read;
use std::path::{Path,PathBuf};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use crate::provider::{canonical_sha256,decode_json};
use crate::temporal::{RationalTime,TimeRange};
use crate::video_assembly::{AssemblyReport,AssemblyStageKind};
use crate::{Error,ErrorCategory,Result};

pub const FINAL_VIDEO_HANDOFF_V1: &str = "aniflow.final-video-handoff/v1";
const MAXIMUM_DOCUMENT_BYTES: u64 = 67_108_864;
#[cfg(test)]
#[path="final_video_remux_tests.rs"]
mod remux_tests;
fn invalid(message: &str) -> Error { Error::new(ErrorCategory::State,message) }
fn temporal(error: anyhow::Error) -> Error { Error::from_anyhow(ErrorCategory::Media,error) }

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum FinalVideoVariant { CleanArchivalMaster, DestinationCopy }

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalAudioProperties {
    pub codec: String,
    pub profile: String,
    pub sample_rate_hz: u32,
    pub channels: u64,
    pub channel_layout: String,
    pub interval: TimeRange,
    pub packet_configuration_timing_sha256: String,
}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalVideoArtifact {
    pub artifact_id: String,
    pub variant: FinalVideoVariant,
    /// Private local locator; public projection omits it altogether.
    pub locator: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub frame_rate: RationalTime,
    pub video_interval: TimeRange,
    pub video_duration: RationalTime,
    pub video_codec: String,
    pub pixel_format: String,
    pub color_primaries: String,
    pub color_transfer: String,
    pub color_matrix: String,
    pub color_range: String,
    pub sample_aspect_ratio: String,
    pub orientation_degrees: u16,
    pub audio: Option<FinalAudioProperties>,
    pub metadata: Option<crate::delivery_metadata::MetadataPublicSummary>,
    pub producer_lock_sha256: String,
    pub stage_invocation_sha256: String,
    pub stage_checkpoint_sha256: String,
    pub private_report_sha256: String,
}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalSourceIdentity {
    pub id: String,
    pub artifact_type: String,
    pub sha256: String,
    pub byte_count: u64,
    pub file_count: u64,
}

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalVideoHandoff {
    pub schema: String,
    pub completion_id: String,
    pub plan_sha256: String,
    pub run_manifest_sha256: String,
    pub delivery_acceptance_sha256: String,
    pub source_lineage: Vec<FinalSourceIdentity>,
    pub artifacts: Vec<FinalVideoArtifact>,
    pub creative_approval: String,
    pub publication_authority: String,
    pub proof_state: String,
}

impl FinalVideoHandoff {
    /// Explicit allowlist of public fields. No locator, lineage, private report,
    /// transcript, removed tag or implicit master bytes enter this summary.
    #[must_use]
    pub fn public_summary(&self) -> serde_json::Value {
        serde_json::json!({"schema":"aniflow.final-video-public-summary/v1","completion_id":self.completion_id,
            "artifacts":self.artifacts.iter().map(|a| serde_json::json!({"artifact_id":a.artifact_id,"variant":a.variant,
                "sha256":a.sha256,"byte_count":a.byte_count,"width":a.width,"height":a.height,"frame_rate":a.frame_rate,
                "video_duration":a.video_duration,"metadata":a.metadata})).collect::<Vec<_>>(),
            "creative_approval":self.creative_approval,"publication_authority":self.publication_authority,"proof_state":self.proof_state})
    }
}

fn canonical(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|v| matches!(v,std::path::Component::ParentDir|std::path::Component::CurDir))
        || path.canonicalize().ok().as_deref() != Some(path) { return Err(invalid("completion locators must be canonical without traversal or symlinks")); }
    Ok(())
}

fn read_document(path: &Path, raw_sha256: Option<&str>) -> Result<Vec<u8>> {
    canonical(path)?;
    let before = crate::frame_io::identity(path,MAXIMUM_DOCUMENT_BYTES)?;
    let mut bytes = Vec::new();
    crate::audio_inspection::process::open_regular(path).map_err(|_| invalid("cannot safely open completion evidence"))?
        .take(MAXIMUM_DOCUMENT_BYTES+1).read_to_end(&mut bytes).map_err(|_| invalid("cannot read bounded completion evidence"))?;
    let actual = (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64);
    if actual != before || raw_sha256.is_some_and(|expected| expected != actual.0)
        || crate::frame_io::identity(path,MAXIMUM_DOCUMENT_BYTES)? != actual { return Err(invalid("completion evidence changed or differs from accepted bytes")); }
    Ok(bytes)
}

fn output<'a>(manifest: &'a crate::PipelineV3RunManifest,id: &str) -> Result<&'a crate::ArtifactEvidence> {
    let mut values = manifest.payload.outputs.iter().filter(|v| v.id == id);
    let value = values.next().ok_or_else(|| invalid("required completion output is missing"))?;
    if values.next().is_some() { return Err(invalid("completion output identity is duplicated")); }
    value.validate()?; Ok(value)
}

fn owned(root: &Path,relative: &str) -> Result<PathBuf> {
    let path = root.join(relative); canonical(&path)?;
    if !path.starts_with(root) { return Err(invalid("completion evidence escaped its workspace")); }
    Ok(path)
}

fn project(root: &Path,plan: &crate::PipelineV3Plan,manifest: &crate::PipelineV3RunManifest,kind: AssemblyStageKind) -> Result<FinalVideoArtifact> {
    let id = kind.artifact_id();
    let report_id = if kind == AssemblyStageKind::Master { "master_evidence" } else { "delivery_evidence" };
    let artifact = output(manifest,id)?;
    if artifact.kind != crate::ArtifactKind::File || artifact.byte_count == 0 { return Err(invalid("completion video must be an accepted nonempty file")); }
    let locator = owned(root,artifact.relative_path.as_deref().ok_or_else(|| invalid("completion video has no owned locator"))?)?;
    let report_port = output(manifest,report_id)?;
    let bytes = read_document(&owned(root,report_port.relative_path.as_deref().ok_or_else(|| invalid("completion report locator is missing"))?)?,Some(&report_port.sha256))?;
    if bytes.len() as u64 != report_port.byte_count { return Err(invalid("completion report size differs from accepted evidence")); }
    let report: AssemblyReport = decode_json(&bytes,"accepted assembly report")?;
    let stage = plan.payload.stages.iter().find(|s| s.id == kind.stage_id()).ok_or_else(|| invalid("completion producer is missing"))?;
    let stage_state = manifest.payload.stages.iter().find(|s| s.stage_id == stage.id).ok_or_else(|| invalid("completion producer state is missing"))?;
    let reference = stage_state.checkpoint.as_ref().ok_or_else(|| invalid("completion producer lacks an accepted checkpoint"))?;
    let checkpoint: crate::StageCheckpoint = decode_json(&read_document(&owned(root,&reference.relative_path)?,None)?,"accepted completion checkpoint")?;
    checkpoint.validate()?;
    if checkpoint.checkpoint_sha256 != reference.checkpoint_sha256 || checkpoint.payload.plan_sha256 != plan.plan_sha256
        || checkpoint.payload.provider_lock_sha256 != stage.provider_lock.lock_sha256 { return Err(invalid("completion checkpoint differs from producer authority")); }
    let expected_capability = kind.capability_id();
    let expected_contract=if report.media.derived_video_clock.is_some(){crate::validation::DERIVED_TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1}else{crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1};
    if stage.capability.id != expected_capability || stage.capability.version != "1.0.0"
        || !stage.validations.iter().any(|v|v.artifact==id&&v.temporal.as_ref().is_some_and(|t|t.derived_video_clock==report.media.derived_video_clock))
        || stage.provider_lock.payload.provider.id != crate::video_assembly::ASSEMBLY_PROVIDER_ID
        || !stage.validations.iter().any(|v| v.artifact == id && v.contract == expected_contract)
        || !checkpoint.payload.validations.iter().any(|v| v.artifact_id == id && v.artifact_sha256 == artifact.sha256 && v.accepted && v.contract == expected_contract) {
        return Err(invalid("completion requires the fixed assembly producer and accepted native temporal gate"));
    }
    if report.schema != crate::video_assembly::ASSEMBLY_REPORT_SCHEMA_V1 || report.stage != kind || report.stage_id != stage.id
        || report.provider_lock_sha256 != stage.provider_lock.lock_sha256
        || report.configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256
        || report.output_sha256 != artifact.sha256 || report.output_byte_count != artifact.byte_count
        || report.media.temporal_inspection.source_sha256 != artifact.sha256
        || report.media.temporal_inspection.source_size_bytes != artifact.byte_count
        || report.media.kind != kind || report.media.frame_count != report.frame_count
        || report.media.width != report.width || report.media.height != report.height
        || report.media.profile != "aniflow.ffmpeg-8.0.1-rgb8-cfr-aac-assembly/v1"
        || report.media.method != "native_integer_timeline_rgb24_framehash_aac_packet_sha256_mp4_atoms" {
        return Err(invalid("completion report is not bound to the accepted final video and producer"));
    }
    for (input,digest) in [("source_video",&report.source_sha256),("source_index",&report.frame_index_sha256)] {
        if !plan.payload.inputs.iter().any(|v| v.id == input && &v.content_sha256 == digest) { return Err(invalid("completion report lost its immutable source lineage")); }
    }
    let temporal_report = &report.media.temporal_inspection;
    temporal_report.require_processing().map_err(temporal)?;
    let video = temporal_report.selected.as_ref().and_then(|v| v.video).and_then(|i| temporal_report.stream(i)).ok_or_else(|| invalid("completion video stream identity is missing"))?;
    if video.width != Some(u64::from(report.width)) || video.height != Some(u64::from(report.height))
        || video.codec.as_ref() != Some(&report.media.video_codec) || video.pixel_format.as_ref() != Some(&report.media.pixel_format) {
        return Err(invalid("completion geometry/codec differs from observed final stream"));
    }
    let timeline = temporal_report.video_timeline().map_err(temporal)?;
    let period = timeline.frame_period.ok_or_else(|| invalid("completion video lacks an exact frame rate"))?;
    let interval = timeline.source_time.ok_or_else(|| invalid("completion video lacks an exact interval"))?;
    if timeline.frames.len() as u64 != report.frame_count { return Err(invalid("completion frame count differs from accepted temporal evidence")); }
    let has_audio = temporal_report.selected.as_ref().is_some_and(|s| s.audio.is_some());
    if has_audio != report.media.audio_properties.is_some() { return Err(invalid("completion audio properties are missing; older audio reports require a newly qualified run")); }
    if let Some(audio) = &report.media.audio_properties {
        let stream = temporal_report.selected.as_ref().and_then(|v| v.audio).and_then(|i| temporal_report.stream(i));
        let interval = temporal_report.selected.as_ref().and_then(|v| v.audio).and_then(|i| temporal_report.timeline(i)).and_then(|v| v.source_time);
        if audio.codec != "aac" || audio.profile != "LC" || audio.channels == 0
            || stream.and_then(|v| v.sample_rate_hz) != Some(audio.sample_rate_hz) || interval != Some(audio.interval)
            || Some(&audio.packet_configuration_timing_sha256) != report.media.output_audio_sha256.as_ref()
            || report.media.source_audio_sha256 != report.media.output_audio_sha256 { return Err(invalid("completion audio packet preservation evidence differs")); }
    }
    let metadata = report.media.metadata.as_ref().map(|v| v.public_summary.clone());
    if let Some(receipt) = &report.media.metadata {
        if receipt.candidate_sha256 != artifact.sha256 || receipt.source_sha256 != report.source_sha256
            || receipt.public_summary.status != crate::delivery_metadata::MetadataInspectionStatus::Unsupported {
            return Err(invalid("completion metadata receipt failed or refers to different bytes"));
        }
    }
    let (codec,pixel,matrix,range,variant) = match kind {
        AssemblyStageKind::Master => ("ffv1","bgr0","rgb","pc",FinalVideoVariant::CleanArchivalMaster),
        AssemblyStageKind::Delivery => ("h264","yuv420p","bt709","tv",FinalVideoVariant::DestinationCopy),
    };
    if report.media.video_codec != codec || report.media.pixel_format != pixel || !report.media.faststart { return Err(invalid("completion media profile differs from the fixed accepted profile")); }
    Ok(FinalVideoArtifact { artifact_id:id.to_owned(),variant,locator,sha256:artifact.sha256.clone(),byte_count:artifact.byte_count,
        width:report.width,height:report.height,frame_count:report.frame_count,frame_rate:period.reciprocal().map_err(temporal)?,
        video_interval:interval,video_duration:interval.duration().map_err(temporal)?,video_codec:codec.to_owned(),pixel_format:pixel.to_owned(),
        color_primaries:"bt709".to_owned(),color_transfer:"bt709".to_owned(),color_matrix:matrix.to_owned(),color_range:range.to_owned(),
        sample_aspect_ratio:"1/1".to_owned(),orientation_degrees:0,audio:report.media.audio_properties,metadata,
        producer_lock_sha256:stage.provider_lock.lock_sha256.clone(),stage_invocation_sha256:checkpoint.payload.stage_invocation_sha256,
        stage_checkpoint_sha256:reference.checkpoint_sha256.clone(),private_report_sha256:report_port.sha256.clone() })
}

/// Observe the accepted fixed assembly run, including a recipe's assembly child.
/// Rehashes existing authority/artifacts; creates no lock, state or media output.
pub fn observe_assembly(run_directory: impl AsRef<Path>) -> Result<FinalVideoHandoff> {
    let root = run_directory.as_ref(); canonical(root)?;
    let workspace = crate::PipelineV3Workspace::open_read_only(root)?;
    let plan = crate::PipelineV3Plan::from_json_slice(&read_document(&workspace.plan(),None)?).map_err(|_| invalid("completion plan is malformed"))?;
    if plan.payload.pipeline_name != "verified-processed-frame-assembly"
        || plan.payload.stages.len() != 2 || plan.payload.outputs.len() != 4 { return Err(invalid("completion supports only the accepted fixed automated assembly profile")); }
    let before = crate::status_v3(root)?;
    if before.payload.state != crate::PipelineV3RunState::Complete || before.payload.plan_sha256 != plan.plan_sha256 { return Err(invalid("completion requires the complete accepted current plan")); }
    let reference = before.payload.delivery.as_ref().ok_or_else(|| invalid("completion lacks delivery acceptance"))?;
    let acceptance: crate::validation::AcceptanceRecord = decode_json(&read_document(&owned(root,&reference.relative_path)?,None)?,"delivery acceptance")?;
    acceptance.validate()?;
    if acceptance.sha256()? != reference.sha256 || acceptance.plan_sha256 != plan.plan_sha256
        || acceptance.layer != crate::validation::ValidationLayer::Delivery { return Err(invalid("completion delivery authority differs from the existing acceptance record")); }
    let artifacts = [AssemblyStageKind::Master,AssemblyStageKind::Delivery].into_iter().map(|kind| project(root,&plan,&before,kind)).collect::<Result<Vec<_>>>()?;
    let identities: Vec<_> = artifacts.iter().map(|a| (&a.artifact_id,&a.sha256,a.byte_count)).collect();
    let completion_id = canonical_sha256(&(FINAL_VIDEO_HANDOFF_V1,&plan.plan_sha256,&reference.sha256,identities))?;
    let source_lineage = plan.payload.inputs.iter().map(|v| FinalSourceIdentity { id:v.id.clone(),artifact_type:v.artifact_type.clone(),sha256:v.content_sha256.clone(),byte_count:v.byte_count,file_count:v.file_count }).collect();
    if crate::status_v3(root)? != before { return Err(invalid("completion authority or final bytes changed during observation")); }
    Ok(FinalVideoHandoff { schema:FINAL_VIDEO_HANDOFF_V1.to_owned(),completion_id,plan_sha256:plan.plan_sha256,
        run_manifest_sha256:before.manifest_sha256,delivery_acceptance_sha256:reference.sha256.clone(),source_lineage,artifacts,
        creative_approval:"not_recorded_by_fixed_automated_profile".to_owned(),publication_authority:"none".to_owned(),proof_state:"not_asserted".to_owned() })
}

/// Project one already accepted metadata-only destination copy. Older reports
/// without observed playback facts refuse rather than inherit guessed fields.
pub fn observe_remux(run_directory:impl AsRef<Path>)->Result<FinalVideoHandoff> {
    use crate::delivery_metadata::remux;
    let root=run_directory.as_ref();canonical(root)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let plan=crate::PipelineV3Plan::from_json_slice(&read_document(&workspace.plan(),None)?).map_err(|_|invalid("remux completion plan malformed"))?;
    if plan.payload.pipeline_name!="metadata-only-remux"||plan.payload.stages.len()!=1||plan.payload.outputs.len()!=2{return Err(invalid("completion remux must be the exact fixed native profile"));}
    let before=crate::status_v3(root)?;let report=remux::observe_accepted_report(root)?;
    let stage=&plan.payload.stages[0];let media=output(&before,"metadata_delivery")?;let evidence=output(&before,"metadata_evidence")?;
    let locator=owned(root,media.relative_path.as_deref().ok_or_else(||invalid("remux destination locator missing"))?)?;
    let bytes=read_document(&owned(root,evidence.relative_path.as_deref().ok_or_else(||invalid("remux evidence locator missing"))?)?,Some(&evidence.sha256))?;
    let current:remux::RemuxReport=decode_json(&bytes,"accepted remux report")?;
    if current!=report||bytes.len() as u64!=evidence.byte_count||before.payload.plan_sha256!=plan.plan_sha256
        ||media.kind!=crate::ArtifactKind::File||media.byte_count==0||stage.id!="metadata_remux"||stage.capability.id!=remux::CAPABILITY_ID
        ||stage.capability.version!="1.0.0"||stage.provider_lock.payload.provider.id!=remux::PROVIDER_ID
        ||stage.provider_lock.payload.manifest_sha256!=canonical_sha256(&report.operation.manifest()?)? {return Err(invalid("remux completion differs from current native producer"));}
    let reference=before.payload.stages.iter().find(|s|s.stage_id==stage.id).and_then(|s|s.checkpoint.as_ref()).ok_or_else(||invalid("remux accepted checkpoint missing"))?;
    let checkpoint:crate::StageCheckpoint=decode_json(&read_document(&owned(root,&reference.relative_path)?,None)?,"remux checkpoint")?;checkpoint.validate()?;
    let contract=crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1;
    if checkpoint.checkpoint_sha256!=reference.checkpoint_sha256||checkpoint.payload.plan_sha256!=plan.plan_sha256
        ||checkpoint.payload.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||!stage.validations.iter().any(|v|v.artifact=="metadata_delivery"&&v.contract==contract)
        ||!checkpoint.payload.validations.iter().any(|v|v.artifact_id=="metadata_delivery"&&v.artifact_sha256==media.sha256&&v.contract==contract&&v.accepted){return Err(invalid("remux lacks exact accepted native temporal validation"));}
    let delivery=before.payload.delivery.as_ref().ok_or_else(||invalid("remux delivery acceptance missing"))?;
    let acceptance:crate::validation::AcceptanceRecord=decode_json(&read_document(&owned(root,&delivery.relative_path)?,None)?,"remux delivery acceptance")?;acceptance.validate()?;
    if acceptance.sha256()?!=delivery.sha256||acceptance.plan_sha256!=plan.plan_sha256||acceptance.layer!=crate::validation::ValidationLayer::Delivery{return Err(invalid("remux delivery acceptance changed"));}
    let artifact=project_remux_properties(&report,media,locator,&stage.provider_lock.lock_sha256,&checkpoint.payload.stage_invocation_sha256,&reference.checkpoint_sha256,&evidence.sha256)?;
    let completion_id=canonical_sha256(&(FINAL_VIDEO_HANDOFF_V1,&plan.plan_sha256,&delivery.sha256,vec![(&artifact.artifact_id,&artifact.sha256,artifact.byte_count)]))?;
    let source_lineage=plan.payload.inputs.iter().map(|i|FinalSourceIdentity {id:i.id.clone(),artifact_type:i.artifact_type.clone(),sha256:i.content_sha256.clone(),byte_count:i.byte_count,file_count:i.file_count}).collect();
    if crate::status_v3(root)?!=before||remux::observe_accepted_report(root)?!=report{return Err(invalid("remux native authority changed during completion observation"));}
    Ok(FinalVideoHandoff {schema:FINAL_VIDEO_HANDOFF_V1.into(),completion_id,plan_sha256:plan.plan_sha256,run_manifest_sha256:before.manifest_sha256,
        delivery_acceptance_sha256:delivery.sha256.clone(),source_lineage,artifacts:vec![artifact],creative_approval:"not_recorded_by_fixed_automated_profile".into(),publication_authority:"none".into(),proof_state:"not_asserted".into()})
}

fn project_remux_properties(report:&crate::delivery_metadata::remux::RemuxReport,artifact:&crate::ArtifactEvidence,locator:PathBuf,lock:&str,invocation:&str,checkpoint:&str,private_report:&str)->Result<FinalVideoArtifact> {
    let media=&report.media;let properties=media.playback.as_ref().ok_or_else(||invalid("remux completion needs observed playback facts; legacy reports cannot be upgraded by inference"))?;
    let candidate=&media.candidate;candidate.require_processing().map_err(temporal)?;
    let selected=candidate.selected.as_ref().ok_or_else(||invalid("remux selected streams missing"))?;
    let video=selected.video.and_then(|i|candidate.stream(i)).ok_or_else(||invalid("remux video stream missing"))?;
    let timeline=candidate.video_timeline().map_err(temporal)?;let interval=timeline.source_time.ok_or_else(||invalid("remux video interval missing"))?;
    if media.schema!="aniflow.metadata-remux-media/v1"||candidate.source_sha256!=artifact.sha256||candidate.source_size_bytes!=artifact.byte_count
        ||properties.video_codec!="h264"||video.codec.as_deref()!=Some(properties.video_codec.as_str())||video.pixel_format.as_deref()!=Some(properties.pixel_format.as_str())
        ||properties.color_primaries!="bt709"||properties.color_transfer!="bt709"||!matches!(properties.color_matrix.as_str(),"bt709"|"gbr")
        ||!matches!(properties.color_range.as_str(),"tv"|"pc")||properties.sample_aspect_ratio!="1/1"||properties.orientation_degrees!=0||!properties.faststart
        ||selected.audio.is_some()!=properties.audio.is_some()||!media.full_decode_observed||media.native_qualified
        ||media.metadata.public_summary.status!=crate::delivery_metadata::MetadataInspectionStatus::CleanWithinScope {return Err(invalid("remux playback facts do not match accepted current native profile"));}
    crate::temporal::validate_reconstruction(&media.source,candidate,false).map_err(temporal)?;
    crate::provider::require_sha256(&media.video_packets_sha256,"remux video packets")?;
    if let Some(audio)=&properties.audio {
        let stream=selected.audio.and_then(|i|candidate.stream(i));let range=selected.audio.and_then(|i|candidate.timeline(i)).and_then(|t|t.source_time);
        if audio.codec!="aac"||audio.profile!="LC"||audio.channels==0||stream.and_then(|s|s.sample_rate_hz)!=Some(audio.sample_rate_hz)
            ||range!=Some(audio.interval)||media.audio_packets_sha256.as_deref()!=Some(audio.packet_configuration_timing_sha256.as_str()){return Err(invalid("remux original-audio packet/clock facts differ"));}
    }else if media.audio_packets_sha256.is_some(){return Err(invalid("remux has unselected audio packet evidence"));}
    Ok(FinalVideoArtifact {artifact_id:artifact.id.clone(),variant:FinalVideoVariant::DestinationCopy,locator,sha256:artifact.sha256.clone(),byte_count:artifact.byte_count,
        width:u32::try_from(video.width.ok_or_else(||invalid("remux width missing"))?).map_err(|_|invalid("remux width overflow"))?,height:u32::try_from(video.height.ok_or_else(||invalid("remux height missing"))?).map_err(|_|invalid("remux height overflow"))?,
        frame_count:timeline.frames.len() as u64,frame_rate:timeline.frame_period.ok_or_else(||invalid("remux exact CFR missing"))?.reciprocal().map_err(temporal)?,video_interval:interval,video_duration:interval.duration().map_err(temporal)?,
        video_codec:properties.video_codec.clone(),pixel_format:properties.pixel_format.clone(),color_primaries:properties.color_primaries.clone(),color_transfer:properties.color_transfer.clone(),color_matrix:properties.color_matrix.clone(),color_range:properties.color_range.clone(),
        sample_aspect_ratio:properties.sample_aspect_ratio.clone(),orientation_degrees:properties.orientation_degrees,audio:properties.audio.clone(),metadata:Some(media.metadata.public_summary.clone()),
        producer_lock_sha256:lock.into(),stage_invocation_sha256:invocation.into(),stage_checkpoint_sha256:checkpoint.into(),private_report_sha256:private_report.into()})
}

/// Consumer-side freshness check before packaging/retry. Changed bytes or
/// acceptance produce a new binding or refusal; old handoffs cannot be replayed.
pub fn recheck(expected: &FinalVideoHandoff,run_directory: impl AsRef<Path>) -> Result<()> {
    if observe(run_directory)? != *expected { return Err(invalid("completion handoff is stale; observe the current accepted result")); }
    Ok(())
}

/// Accept either an assembly run or a complete source/indexed recipe. Parent
/// recipe acceptance is checked before and after projecting its assembly child.
pub fn observe(run_directory: impl AsRef<Path>) -> Result<FinalVideoHandoff> {
    use crate::music_video_recipe as recipe;
    let root = run_directory.as_ref(); canonical(root)?;
    let has_intent = root.join("intent.json").try_exists().map_err(|_| invalid("cannot inspect completion entry"))?;
    if !has_intent {
        let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
        let plan=crate::PipelineV3Plan::from_json_slice(&read_document(&workspace.plan(),None)?).map_err(|_|invalid("completion plan malformed"))?;
        return match plan.payload.pipeline_name.as_str() {"verified-processed-frame-assembly"=>observe_assembly(root),"metadata-only-remux"=>observe_remux(root),_=>Err(invalid("completion supports only fixed accepted assembly or metadata-remux profiles"))};
    }
    match recipe::entry_kind(root)? {
        recipe::MusicVideoEntryKind::Indexed => {
            let before = recipe::status(root)?;
            if !before.complete { return Err(invalid("partial recipe cannot emit completion evidence")); }
            let phase = before.phases.iter().find(|p| p.phase == recipe::MusicVideoPhase::Assembly).ok_or_else(|| invalid("recipe assembly phase is missing"))?;
            let child = phase.run_directory.as_ref().ok_or_else(|| invalid("recipe assembly run is missing"))?;
            let result = observe_assembly(child)?;
            if recipe::status(root)? != before { return Err(invalid("recipe changed during completion observation")); }
            Ok(result)
        },
        recipe::MusicVideoEntryKind::Source => {
            let before = recipe::status_source(root)?;
            if !before.complete { return Err(invalid("partial source recipe cannot emit completion evidence")); }
            let phase = before.phases.iter().find(|p| p.phase == recipe::SourceMusicVideoPhase::Assembly).ok_or_else(|| invalid("source recipe assembly phase is missing"))?;
            let child = phase.run_directory.as_ref().ok_or_else(|| invalid("source recipe assembly run is missing"))?;
            let result = observe_assembly(child)?;
            if recipe::status_source(root)? != before { return Err(invalid("source recipe changed during completion observation")); }
            Ok(result)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound_evidence_rejects_mutation_instead_of_accepting_a_previous_digest() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().canonicalize().unwrap().join("evidence.json");
        std::fs::write(&file,b"original accepted evidence").unwrap();
        let digest = format!("{:x}",Sha256::digest(b"original accepted evidence"));
        assert_eq!(read_document(&file,Some(&digest)).unwrap(),b"original accepted evidence");
        std::fs::write(&file,b"mutated accepted evidence").unwrap();
        assert!(read_document(&file,Some(&digest)).is_err());
    }
    #[test]
    fn loose_artifacts_and_missing_acceptance_never_emit_completion_or_create_state() {
        let directory = tempfile::tempdir().unwrap(); let root = directory.path().canonicalize().unwrap();
        std::fs::write(root.join("master.mp4"),b"unaccepted placeholder").unwrap();
        assert!(observe(&root).is_err());
        assert!(!root.join("writer.lock").exists());
        assert!(!root.join("intent.json").exists());
        assert_eq!(std::fs::read(root.join("master.mp4")).unwrap(),b"unaccepted placeholder");
    }
    #[test]
    fn public_summary_omits_locators_lineage_and_private_report_references() {
        let interval = TimeRange::new(RationalTime::ZERO,RationalTime::new(1,1).unwrap()).unwrap();
        let artifact = FinalVideoArtifact { artifact_id:"h264_delivery".to_owned(),variant:FinalVideoVariant::DestinationCopy,
            locator:PathBuf::from("/private/production/personal-title.mp4"),sha256:"a".repeat(64),byte_count:100,width:2,height:2,
            frame_count:24,frame_rate:RationalTime::new(24,1).unwrap(),video_interval:interval,video_duration:RationalTime::new(1,1).unwrap(),
            video_codec:"h264".to_owned(),pixel_format:"yuv420p".to_owned(),color_primaries:"bt709".to_owned(),color_transfer:"bt709".to_owned(),
            color_matrix:"bt709".to_owned(),color_range:"tv".to_owned(),sample_aspect_ratio:"1/1".to_owned(),orientation_degrees:0,audio:None,metadata:None,
            producer_lock_sha256:"b".repeat(64),stage_invocation_sha256:"c".repeat(64),stage_checkpoint_sha256:"d".repeat(64),private_report_sha256:"e".repeat(64) };
        let value = FinalVideoHandoff { schema:FINAL_VIDEO_HANDOFF_V1.to_owned(),completion_id:"f".repeat(64),plan_sha256:"a".repeat(64),
            run_manifest_sha256:"b".repeat(64),delivery_acceptance_sha256:"c".repeat(64),source_lineage:vec![],artifacts:vec![artifact],
            creative_approval:"not_recorded_by_fixed_automated_profile".to_owned(),publication_authority:"none".to_owned(),proof_state:"not_asserted".to_owned() };
        let public = serde_json::to_string(&value.public_summary()).unwrap();
        for forbidden in ["locator","/private/","personal-title","source_lineage","private_report_sha256","producer_lock_sha256"] { assert!(!public.contains(forbidden)); }
        assert!(public.contains("h264_delivery"));
    }
    #[cfg(unix)]
    #[test]
    fn completion_evidence_cannot_follow_symlinks_or_escape_its_workspace() {
        let directory = tempfile::tempdir().unwrap(); let root = directory.path().canonicalize().unwrap();
        let outside = root.join("outside.json"); std::fs::write(&outside,b"private source").unwrap();
        let workspace = root.join("run"); std::fs::create_dir(&workspace).unwrap();
        std::os::unix::fs::symlink(&outside,workspace.join("linked.json")).unwrap();
        assert!(read_document(&workspace.join("linked.json"),None).is_err());
        assert!(owned(&workspace,"../outside.json").is_err());
    }
}
