//! Reuse a spatial output only through its existing accepted v3 authority.
use super::*;

fn accepted(request: &MusicVideoRequest) -> Result<(PathBuf, crate::PipelineV3RunManifest)> {
    let root = workspace::root(
        request
            .prior_upscale_run
            .as_ref()
            .ok_or_else(|| invalid("prior Upscayl run is required"))?,
    )?;
    if !matches!(
        request.configuration.repair,
        MusicVideoRepair::Disabled { .. }
    ) {
        return Err(invalid(
            "prior spatial input requires repair disabled; spatial repair is not supported by the original-clock importer",
        ));
    }
    let (index, _) = crate::video_assembly::read_index(&request.frame_index)?;
    if !matches!(
        index,
        crate::video_assembly::AssemblyFrameIndex::Upscaled(_)
    ) {
        return Err(invalid(
            "prior Upscayl input requires its spatial output index",
        ));
    }
    let manifest = crate::status_v3(&root)?;
    if manifest.payload.state != crate::PipelineV3RunState::Complete
        || manifest.payload.delivery.is_none()
    {
        return Err(state("prior Upscayl run lacks complete current acceptance"));
    }
    if output_path(&root, output(&manifest, "upscaled_frames")?)? != request.frames_directory
        || output_path(&root, output(&manifest, "upscaled_index")?)? != request.frame_index
    {
        return Err(state(
            "prior input paths must be the exact accepted Upscayl output ports",
        ));
    }
    Ok((root, manifest))
}

pub(super) fn binding(request: &MusicVideoRequest) -> Result<Option<PriorUpscaleInput>> {
    if request.prior_upscale_run.is_none() {
        return Ok(None);
    }
    let (run_directory, manifest) = accepted(request)?;
    Ok(Some(PriorUpscaleInput {
        run_directory,
        manifest_sha256: canonical_sha256(&manifest)?,
    }))
}

pub(super) fn initial_plan(
    request: &MusicVideoRequest,
    token: &CancellationToken,
) -> Result<crate::PipelineV3Plan> {
    let (root, before) = accepted(request)?;
    let result = crate::video_assembly::plan(
        &AssemblyRequest::new(
            &request.source_video,
            &request.frames_directory,
            &request.frame_index,
            request.configuration.assembly.clone(),
            &request.provider_executable,
        ),
        token,
    )?;
    if crate::status_v3(root)? != before {
        return Err(state("prior acceptance changed during recipe admission"));
    }
    Ok(result)
}

pub(super) fn handoff(
    request: &MusicVideoRequest,
    token: &CancellationToken,
) -> Result<(FrameHandoff, crate::PipelineV3RunManifest)> {
    let (root, manifest) = accepted(request)?;
    let mut frames = super::handoff(request, MusicVideoPhase::Upscale, &root, &manifest, token)?;
    frames.target_resolution = target::prepared(request, &frames, token)?;
    if crate::status_v3(root)? != manifest {
        return Err(state("prior acceptance changed during target handoff"));
    }
    Ok((frames, manifest))
}
