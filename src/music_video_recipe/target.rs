//! Target policy adaptation for the fixed recipe. Existing accepted child
//! artifacts remain the authority; these decisions never create checkpoints.
use super::*;
use crate::upscale_target::{self, UpscaleTargetStatus};
use crate::upscayl_recovery::UpscaylRecoveryConfiguration;

fn observe(request: &MusicVideoRequest, frames: &Path, index: &Path,
    input_phase: MusicVideoTargetInputPhase, token: &CancellationToken) -> Result<Option<MusicVideoTargetResolution>> {
    let MusicVideoUpscale::TargetResolution { policy, configuration } = &request.configuration.upscale else { return Ok(None); };
    let bounds = request.configuration.import();
    let input = upscale_target::observe_input(frames,index,bounds.maximum_frame_bytes,bounds.maximum_total_frame_bytes,token)?;
    let decision = upscale_target::decide(&input,Some(policy))?;
    let resources = decision.resources(configuration)?;
    let required_components = if decision.status == UpscaleTargetStatus::Execute {
        ["upscayl","upscayl-ffmpeg","upscayl-param","upscayl-weights"].into_iter().map(str::to_owned).collect()
    } else { Vec::new() };
    Ok(Some(MusicVideoTargetResolution { input_phase,decision,resources,required_components }))
}

pub(super) fn initial(request: &MusicVideoRequest, first: &crate::PipelineV3Plan,
    token: &CancellationToken) -> Result<Option<MusicVideoTargetResolution>> {
    let result = observe(request,&request.frames_directory,&request.frame_index,MusicVideoTargetInputPhase::OriginalInput,token)?;
    if let Some(resolution) = &result {
        let input = &resolution.decision.input;
        for (id,digest,bytes) in [
            ("source_video",&input.source_sha256,input.source_byte_count),
            ("source_index",&input.frame_index_sha256,first.payload.inputs.iter().find(|value| value.id == "source_index")
                .ok_or_else(|| state("recipe initial plan has no source index"))?.byte_count),
            ("source_frames",&input.frames_sha256,input.frames_byte_count),
        ] {
            if !first.payload.inputs.iter().any(|value| value.id == id && &value.content_sha256 == digest && value.byte_count == bytes) {
                return Err(state("target observation differs from the exact initial recipe input bytes"));
            }
        }
        if !first.payload.inputs.iter().any(|value| value.id == "source_frames" && value.file_count == input.frame_count) {
            return Err(state("target observation differs from the initial complete frame set"));
        }
    }
    Ok(result)
}

pub(super) fn prepared(request: &MusicVideoRequest, frames: &FrameHandoff,
    token: &CancellationToken) -> Result<Option<MusicVideoTargetResolution>> {
    let result = observe(request,&frames.frames,&frames.index,MusicVideoTargetInputPhase::PreparedFrames,token)?;
    if let Some(resolution) = &result {
        let input = &resolution.decision.input;
        if input.source_sha256 != frames.source_sha256 || input.frame_index_sha256 != frames.index_sha256
            || input.frames_sha256 != frames.directory.content_sha256 || input.frames_byte_count != frames.directory.byte_count
            || input.frame_count != frames.directory.file_count {
            return Err(state("target observation differs from the accepted prepared-frame handoff"));
        }
    }
    Ok(result)
}

pub(super) fn configuration(request: &MusicVideoRequest, resolution: Option<&MusicVideoTargetResolution>) -> Result<Option<UpscaylRecoveryConfiguration>> {
    match &request.configuration.upscale {
        MusicVideoUpscale::Disabled {..} => Ok(None),
        MusicVideoUpscale::Upscayl { configuration } => Ok(Some(configuration.clone())),
        MusicVideoUpscale::TargetResolution { policy, configuration } => {
            let observed = resolution.ok_or_else(|| state("target-resolution mode needs an observed current input decision"))?;
            if observed.decision.policy.as_ref() != Some(policy) { return Err(state("target decision differs from the selected recipe policy")); }
            observed.decision.resolved_configuration(configuration)
        },
    }
}

pub(super) fn bypass(resolution: &MusicVideoTargetResolution) -> Option<MusicVideoPhaseState> {
    match resolution.decision.status {
        UpscaleTargetStatus::AlreadyTarget => Some(MusicVideoPhaseState::AlreadyTarget),
        UpscaleTargetStatus::AboveTarget => Some(MusicVideoPhaseState::AboveTarget),
        UpscaleTargetStatus::Execute | UpscaleTargetStatus::Disabled => None,
    }
}

/// Preparation may change selected pixels, but the recipe's current repair
/// profile cannot change geometry. Require its actual decision to agree with
/// the initially disclosed operation while binding the new accepted identities.
pub(super) fn require_same_operation(initial: Option<&MusicVideoTargetResolution>, current: Option<&MusicVideoTargetResolution>) -> Result<()> {
    match (initial,current) {
        (None,None) => Ok(()),
        (Some(initial),Some(current)) => {
            let before = &initial.decision;
            let after = &current.decision;
            if current.input_phase != MusicVideoTargetInputPhase::PreparedFrames
                || before.policy != after.policy || before.status != after.status
                || before.effective_width != after.effective_width || before.effective_height != after.effective_height
                || before.output_scale != after.output_scale || before.input.width != after.input.width
                || before.input.height != after.input.height || before.input.frame_count != after.input.frame_count
                || before.input.source_sha256 != after.input.source_sha256 {
                return Err(state("accepted preparation changed the disclosed target operation; start a new compatible recipe"));
            }
            Ok(())
        },
        _ => Err(state("recipe target decision is missing or unexpectedly present")),
    }
}
