//! An accepted extraction supplies the existing indexed recipe in memory.
//! Its child acceptance remains authoritative throughout the handoff.
use super::*;

fn require_accepted(child: &Path, expected: &crate::PipelineV3Plan, manifest: &crate::PipelineV3RunManifest) -> Result<()> {
    if manifest.payload.state != crate::PipelineV3RunState::Complete || manifest.payload.delivery.is_none()
        || manifest.payload.plan_sha256 != expected.plan_sha256 {
        return Err(state("source recipe requires complete extraction acceptance for its exact child plan"));
    }
    if workspace::retained_plan(child)? != *expected || crate::status_v3(child)? != *manifest {
        return Err(state("accepted extraction changed during its indexed-frame handoff"));
    }
    Ok(())
}

fn extracted_output<'a>(manifest: &'a crate::PipelineV3RunManifest, id: &str,
    relative: &str, kind: crate::ArtifactKind) -> Result<&'a crate::ArtifactEvidence> {
    let evidence = output(manifest,id)?;
    if evidence.port != "final" || evidence.relative_path.as_deref() != Some(relative) || evidence.kind != kind {
        return Err(state("accepted extraction output differs from its fixed source profile"));
    }
    Ok(evidence)
}

/// The mutating caller retains the accepted extraction writer until the entire
/// recipe tail finishes. Read-only status instead brackets every observation
/// with the existing v3 authority and never claims a writer or probes media.
pub(super) fn indexed(request: &SourceMusicVideoRequest, child: &Path, expected: &crate::PipelineV3Plan,
    manifest: &crate::PipelineV3RunManifest, token: &CancellationToken) -> Result<(MusicVideoRequest,MusicVideoPlan)> {
    cancelled(token)?;
    require_accepted(child,expected,manifest)?;
    if expected.payload.stages.len() != 1 || expected.payload.inputs.len() != 1 || manifest.payload.outputs.len() != 3 {
        return Err(state("source recipe extraction differs from its single-stage source profile"));
    }
    let stage = &expected.payload.stages[0];
    let source = &expected.payload.inputs[0];
    if stage.id != "extract_frames" || stage.capability.id != crate::frame_extraction::FRAME_EXTRACTION_CAPABILITY_ID
        || source.id != "source_video" || source.kind != crate::PipelineInputKind::File || source.file_count != 1 {
        return Err(state("source recipe extraction lost its original source binding"));
    }
    let frames_evidence = extracted_output(manifest,"extracted_frames","artifacts/frame-extraction/frames",crate::ArtifactKind::Directory)?;
    let index_evidence = extracted_output(manifest,"extracted_index","artifacts/frame-extraction/index.json",crate::ArtifactKind::File)?;
    let report_evidence = extracted_output(manifest,"extraction_evidence","artifacts/frame-extraction/extraction.json",crate::ArtifactKind::File)?;
    let frames = output_path(child,frames_evidence)?;
    let index_path = output_path(child,index_evidence)?;
    let report_path = output_path(child,report_evidence)?;
    // This parser accepts only an original RifeFrameIndex, never a repaired or
    // upscaled index that would silently replace the original temporal clock.
    let (index,index_identity) = crate::frame_import::read_index(&index_path)?;
    if index_identity != (index_evidence.sha256.clone(),index_evidence.byte_count)
        || index.temporal_inspection.source_sha256 != source.content_sha256
        || index.temporal_inspection.source_size_bytes != source.byte_count
        || index.temporal_inspection.selection_intent != request.configuration.extraction.source_selection {
        return Err(state("accepted extraction index differs from its source, selection or accepted bytes"));
    }
    let declaration = crate::AuthoredPipelineInput { id:"source_frames".to_owned(),artifact_type:"application/vnd.aniflow.rife-frames".to_owned(),
        artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video) };
    let directory = crate::pipeline_v3::hash_pipeline_input(&declaration,&frames).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if directory.file_count != frames_evidence.file_count || directory.byte_count != frames_evidence.byte_count
        || directory.file_count != index.frames.len() as u64 {
        return Err(state("accepted extraction directory differs from its complete index or output evidence"));
    }
    let report: crate::frame_extraction::FrameExtractionReport = workspace::read_document(&report_path)?;
    if identity(&report_path,crate::frame_extraction::FRAME_EXTRACTION_MAXIMUM_REPORT_BYTES)? != (report_evidence.sha256.clone(),report_evidence.byte_count)
        || report.schema != crate::frame_extraction::FRAME_EXTRACTION_REPORT_SCHEMA_V1
        || report.stage_id != stage.id || report.provider_lock_sha256 != stage.provider_lock.lock_sha256
        || report.configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256
        || report.source_sha256 != source.content_sha256 || report.source_byte_count != source.byte_count
        || report.frame_index_sha256 != index_identity.0 || report.frames_sha256 != directory.content_sha256
        || report.frames_byte_count != directory.byte_count || report.frame_count != directory.file_count
        || report.width != index.width || report.height != index.height
        || report.media.temporal_inspection != index.temporal_inspection
        || report.media.width != index.width || report.media.height != index.height || report.media.frame_count != directory.file_count {
        return Err(state("extraction evidence differs from its accepted source, index or complete frame directory"));
    }
    // Output and input directories use different digest contracts. The report
    // binds the planning digest, while v3 status re-verifies output acceptance.
    require_accepted(child,expected,manifest)?;
    cancelled(token)?;
    let indexed = MusicVideoRequest::new(&request.source_video,frames,index_path,request.configuration.recipe.clone(),&request.provider_executable);
    let planned = plan(&indexed,token)?;
    let inputs = &planned.intent.payload.initial_inputs;
    if !inputs.iter().any(|input| input.id == "source_video" && input.kind == crate::PipelineInputKind::File
        && input.content_sha256 == source.content_sha256 && input.byte_count == source.byte_count && input.file_count == source.file_count)
        || !inputs.iter().any(|input| input.id == "source_index" && input.kind == crate::PipelineInputKind::File
            && input.content_sha256 == index_identity.0 && input.byte_count == index_identity.1 && input.file_count == 1)
        || !inputs.iter().any(|input| input.id == "source_frames" && input.kind == crate::PipelineInputKind::Directory
            && input.content_sha256 == directory.content_sha256 && input.byte_count == directory.byte_count && input.file_count == directory.file_count) {
        return Err(state("indexed preparation plan does not bind the accepted extraction's complete inputs"));
    }
    require_accepted(child,expected,manifest)?;
    cancelled(token)?;
    Ok((indexed,planned))
}
