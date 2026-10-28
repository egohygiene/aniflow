//! One fixed indexed-frame recipe over existing child Pipeline v3 runs.
//! The immutable intent locates children; only their existing acceptance
//! records can authorize a handoff. No recipe completion file is written.
mod types;
mod workspace;

use std::path::{Path, PathBuf};
use crate::audio_inspection::AudioToolPin;
use crate::audio_inspection::process::verify_pin;
use crate::frame_import::{FrameImportConfiguration, FrameImportRequest};
use crate::frame_repair::allenk::AllenkRepairRequest;
use crate::frame_io::identity;
use crate::provider::{canonical_sha256, decode_json};
use crate::upscayl_recovery::UpscaylRecoveryRequest;
use crate::video_assembly::AssemblyRequest;
use crate::{CancellationToken, Error, ErrorCategory, Result};
pub use types::*;

const MAXIMUM_DOCUMENT_BYTES: u64 = 67_108_864;
fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input,message) }
fn state(message: impl Into<String>) -> Error { Error::new(ErrorCategory::State,message) }
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { return Err(Error::new(ErrorCategory::Execution,"music-video recipe cancelled")); }
    Ok(())
}

impl MusicVideoConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("music-video recipe configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes,"music-video recipe configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != MUSIC_VIDEO_CONFIGURATION_SCHEMA_V1 { return Err(invalid("unsupported music-video recipe schema")); }
        let reason = |reason: &str| -> Result<()> {
            if reason.trim().is_empty() || reason.len() > 256 || reason.chars().any(char::is_control) {
                return Err(invalid("a disabled step requires a printable reason of 1..256 bytes"));
            }
            Ok(())
        };
        match &self.repair {
            MusicVideoRepair::Disabled { reason: why, import } => { reason(why)?; import.validate()?; },
            MusicVideoRepair::Allenk { configuration, ranges } => {
                configuration.validate()?;
                if ranges.is_empty() { return Err(invalid("named repair requires explicit nonempty frame ranges")); }
            },
        }
        match &self.upscale {
            MusicVideoUpscale::Disabled { reason: why } => reason(why)?,
            MusicVideoUpscale::Upscayl { configuration } => configuration.validate()?,
        }
        self.assembly.validate()
    }
    fn import(&self) -> &FrameImportConfiguration { match &self.repair {
        MusicVideoRepair::Disabled { import,.. } => import,
        MusicVideoRepair::Allenk { configuration,.. } => &configuration.import,
    } }
}

impl MusicVideoIntent {
    pub fn validate(&self) -> Result<()> {
        self.payload.configuration.validate()?;
        if self.schema != MUSIC_VIDEO_INTENT_SCHEMA_V1 || canonical_sha256(&self.payload)? != self.intent_sha256 {
            return Err(state("recipe intent is malformed or differs from its immutable digest"));
        }
        Ok(())
    }
    fn request(&self) -> MusicVideoRequest { MusicVideoRequest::new(&self.payload.source_video,&self.payload.frames_directory,&self.payload.frame_index,
        self.payload.configuration.clone(),&self.payload.provider_executable) }
}

fn import_request(request: &MusicVideoRequest) -> FrameImportRequest {
    FrameImportRequest::new(&request.source_video,&request.frames_directory,&request.frame_index,request.configuration.import().clone(),&request.provider_executable)
}
fn repair_request(request: &MusicVideoRequest) -> Result<AllenkRepairRequest> {
    let MusicVideoRepair::Allenk { configuration,ranges } = &request.configuration.repair else { return Err(invalid("named repair is disabled")); };
    Ok(AllenkRepairRequest::new(&request.source_video,&request.frames_directory,&request.frame_index,configuration.clone(),ranges.clone(),&request.provider_executable))
}
fn initial_plan(request: &MusicVideoRequest, token: &CancellationToken) -> Result<crate::PipelineV3Plan> {
    match &request.configuration.repair {
        MusicVideoRepair::Disabled {..} => crate::frame_import::plan(&import_request(request),token),
        MusicVideoRepair::Allenk {..} => crate::frame_repair::allenk::plan(&repair_request(request)?,token),
    }
}

fn components(request: &MusicVideoRequest, token: &CancellationToken) -> Result<Vec<MusicVideoComponent>> {
    let mut values = Vec::new();
    let mut tool = |id: &str, pin: &AudioToolPin| -> Result<()> {
        cancelled(token)?;
        verify_pin(pin,id).map_err(|_| Error::new(ErrorCategory::Dependency,format!("selected recipe tool {id} is unavailable or changed")))?;
        let observed = identity(&pin.executable,134_217_728)?;
        if observed.0 != pin.sha256 { return Err(invalid("selected recipe tool changed during admission")); }
        values.push(MusicVideoComponent { id:id.to_owned(),path:pin.executable.clone(),sha256:observed.0,byte_count:observed.1 });
        Ok(())
    };
    tool("import-ffmpeg",&request.configuration.import().ffmpeg)?;
    if let MusicVideoRepair::Allenk { configuration,.. } = &request.configuration.repair { tool("allenk",&configuration.tool)?; }
    if let MusicVideoUpscale::Upscayl { configuration } = &request.configuration.upscale {
        tool("upscayl",&configuration.upscayl)?;
        tool("upscayl-ffmpeg",&configuration.import.ffmpeg)?;
    }
    tool("assembly-ffmpeg",&request.configuration.assembly.ffmpeg)?;
    tool("assembly-ffprobe",&request.configuration.assembly.ffprobe)?;
    if let MusicVideoUpscale::Upscayl { configuration } = &request.configuration.upscale {
        for (id,file) in [("upscayl-param",&configuration.model.param),("upscayl-weights",&configuration.model.weights)] {
            cancelled(token)?;
            let observed = identity(&file.path,1_073_741_824)?;
            if observed.0 != file.sha256 || observed.1 == 0 { return Err(Error::new(ErrorCategory::Dependency,"selected recipe model is empty or changed")); }
            values.push(MusicVideoComponent { id:id.to_owned(),path:file.path.clone(),sha256:observed.0,byte_count:observed.1 });
        }
    }
    cancelled(token)?;
    let observed = identity(&request.provider_executable,1_073_741_824)?;
    values.push(MusicVideoComponent { id:"aniflow-provider".to_owned(),path:request.provider_executable.clone(),sha256:observed.0,byte_count:observed.1 });
    Ok(values)
}

/// Read-only admission. Downstream file digests do not exist until an accepted
/// predecessor supplies them, so those child plans remain explicitly pending.
pub fn plan(request: &MusicVideoRequest, token: &CancellationToken) -> Result<MusicVideoPlan> {
    cancelled(token)?;
    request.configuration.validate()?;
    let first = initial_plan(request,token)?;
    let (index,index_identity) = crate::frame_import::read_index(&request.frame_index)?;
    if !first.payload.inputs.iter().any(|input| input.id == "source_index" && input.content_sha256 == index_identity.0 && input.byte_count == index_identity.1)
        || !first.payload.inputs.iter().any(|input| input.id == "source_video" && input.content_sha256 == index.temporal_inspection.source_sha256 && input.byte_count == index.temporal_inspection.source_size_bytes) {
        return Err(invalid("recipe initial index or temporal source changed during plan admission"));
    }
    let (width,height) = match &request.configuration.upscale {
        MusicVideoUpscale::Disabled {..} => (index.width,index.height),
        MusicVideoUpscale::Upscayl { configuration } => {
            let count = index.frames.len();
            let supported = match configuration.execution {
                crate::upscayl_recovery::UpscaylExecutionPolicy::PerFrame => count <= crate::upscayl_recovery::UPSCAYL_MAXIMUM_FRAMES,
                crate::upscayl_recovery::UpscaylExecutionPolicy::BoundedChunk { frames_per_chunk,.. } => count <= crate::upscayl_recovery::UPSCAYL_MAXIMUM_CHUNK_FRAMES
                    && count.div_ceil(frames_per_chunk as usize) <= crate::upscayl_recovery::UPSCAYL_MAXIMUM_CHUNKS,
            };
            if !supported { return Err(invalid("recipe frame count exceeds the selected Upscayl per-frame or bounded-chunk policy")); }
            crate::frame_import::admit_geometry(&configuration.import,&index)?;
            configuration.output_dimensions(index.width,index.height)?
        },
    };
    if width < 2 || height < 2 || width % 2 != 0 || height % 2 != 0
        || u64::from(width) * u64::from(height) > request.configuration.assembly.maximum_pixels {
        return Err(invalid("recipe's final geometry is outside the selected H264 assembly profile"));
    }
    if index.temporal_inspection.source_size_bytes > request.configuration.assembly.maximum_source_bytes {
        return Err(invalid("recipe source exceeds the selected assembly source bound"));
    }
    let payload = MusicVideoIntentPayload {
        configuration:request.configuration.clone(),source_video:request.source_video.clone(),frames_directory:request.frames_directory.clone(),
        frame_index:request.frame_index.clone(),provider_executable:request.provider_executable.clone(),components:components(request,token)?,
        initial_inputs:first.payload.inputs.clone(),initial_plan_sha256:first.plan_sha256.clone(),
    };
    let intent = MusicVideoIntent { schema:MUSIC_VIDEO_INTENT_SCHEMA_V1.to_owned(),intent_sha256:canonical_sha256(&payload)?,payload };
    let upscale = match &request.configuration.upscale {
        MusicVideoUpscale::Disabled { reason } => MusicVideoPhasePlan { phase:MusicVideoPhase::Upscale,state:MusicVideoPhaseState::Disabled,reason:Some(reason.clone()),plan:None },
        MusicVideoUpscale::Upscayl {..} => MusicVideoPhasePlan { phase:MusicVideoPhase::Upscale,state:MusicVideoPhaseState::Pending,reason:Some("awaiting accepted prepared frames".to_owned()),plan:None },
    };
    Ok(MusicVideoPlan { schema:MUSIC_VIDEO_PLAN_SCHEMA_V1.to_owned(),intent,output_width:width,output_height:height,phases:vec![
        MusicVideoPhasePlan { phase:MusicVideoPhase::PrepareFrames,state:MusicVideoPhaseState::Ready,reason:None,plan:Some(first) },upscale,
        MusicVideoPhasePlan { phase:MusicVideoPhase::Assembly,state:MusicVideoPhaseState::Pending,reason:Some("awaiting accepted processed frames".to_owned()),plan:None },
    ] })
}

fn require_intent(request: &MusicVideoRequest, expected: &MusicVideoIntent, token: &CancellationToken) -> Result<MusicVideoPlan> {
    let current = plan(request,token)?;
    if current.intent != *expected { return Err(state("recipe source, frames, configuration, tool/model pins or provider identity changed; start a new recipe run")); }
    Ok(current)
}

struct FrameHandoff {
    frames: PathBuf,
    index: PathBuf,
    index_sha256: String,
    directory: crate::PipelineInputIdentity,
    frame_hashes: Vec<String>,
    source_sha256: String,
}

fn output<'a>(manifest: &'a crate::PipelineV3RunManifest, id: &str) -> Result<&'a crate::ArtifactEvidence> {
    let mut matches = manifest.payload.outputs.iter().filter(|output| output.id == id);
    let value = matches.next().ok_or_else(|| state(format!("accepted child is missing its {id} output")))?;
    if matches.next().is_some() { return Err(state("accepted child repeats an output identity")); }
    Ok(value)
}

fn output_path(root: &Path, evidence: &crate::ArtifactEvidence) -> Result<PathBuf> {
    evidence.validate()?;
    let relative = evidence.relative_path.as_ref().ok_or_else(|| state("accepted child output has no owned workspace locator"))?;
    let path = root.join(relative);
    workspace::require_existing_canonical(&path)?;
    Ok(path)
}

fn handoff(request: &MusicVideoRequest, phase: MusicVideoPhase, child: &Path, manifest: &crate::PipelineV3RunManifest) -> Result<FrameHandoff> {
    if manifest.payload.state != crate::PipelineV3RunState::Complete || manifest.payload.delivery.is_none() { return Err(state("only a complete accepted child can supply the next frame set")); }
    let (frames_id,index_id) = match phase {
        MusicVideoPhase::PrepareFrames => match &request.configuration.repair {
            MusicVideoRepair::Disabled {..} => ("imported_frames",None),
            MusicVideoRepair::Allenk {..} => ("repaired_frames",Some("repaired_index")),
        },
        MusicVideoPhase::Upscale => ("upscaled_frames",Some("upscaled_index")),
        MusicVideoPhase::Assembly => return Err(state("delivery cannot be adopted as a frame set")),
    };
    let frames_evidence = output(manifest,frames_id)?;
    let frames = output_path(child,frames_evidence)?;
    let (index,index_sha256) = if let Some(id) = index_id {
        let evidence = output(manifest,id)?;
        (output_path(child,evidence)?,evidence.sha256.clone())
    } else {
        let retained = workspace::retained_plan(child)?;
        let original = retained.payload.inputs.iter().find(|input| input.id == "source_index").ok_or_else(|| state("import child lost its original index binding"))?;
        (request.frame_index.clone(),original.content_sha256.clone())
    };
    let (observed_index,observed_identity) = crate::video_assembly::read_index(&index)?;
    if observed_identity.0 != index_sha256 { return Err(state("accepted frame index changed during handoff")); }
    let declaration = crate::AuthoredPipelineInput { id:"source_frames".to_owned(),artifact_type:"application/vnd.aniflow.assembly-frames".to_owned(),artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video) };
    let directory = crate::pipeline_v3::hash_pipeline_input(&declaration,&frames).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if directory.file_count != frames_evidence.file_count || directory.byte_count != frames_evidence.byte_count {
        return Err(state("accepted frame directory changed during handoff"));
    }
    // v3 output and planning directories use distinct digest contracts. Bridge
    // them only while the same accepted child is verified before and after.
    if crate::status_v3(child)? != *manifest { return Err(state("accepted child changed while observing its next input identities")); }
    Ok(FrameHandoff { frames,index,index_sha256,directory,frame_hashes:observed_index.frames().iter().map(|frame| frame.sha256.clone()).collect(),source_sha256:observed_index.temporal().source_sha256.clone() })
}

fn upscale_request(request: &MusicVideoRequest, handoff: &FrameHandoff) -> Result<UpscaylRecoveryRequest> {
    let MusicVideoUpscale::Upscayl { configuration } = &request.configuration.upscale else { return Err(invalid("upscale is disabled")); };
    Ok(UpscaylRecoveryRequest::new(&request.source_video,&handoff.frames,&handoff.index,configuration.clone(),&request.provider_executable))
}
fn assembly_request(request: &MusicVideoRequest, handoff: &FrameHandoff) -> AssemblyRequest {
    AssemblyRequest::new(&request.source_video,&handoff.frames,&handoff.index,request.configuration.assembly.clone(),&request.provider_executable)
}

fn downstream_plan(request: &MusicVideoRequest, phase: MusicVideoPhase, handoff: &FrameHandoff, token: &CancellationToken) -> Result<crate::PipelineV3Plan> {
    let plan = match phase {
        MusicVideoPhase::Upscale => crate::upscayl_recovery::plan(&upscale_request(request,handoff)?,token)?,
        MusicVideoPhase::Assembly => crate::video_assembly::plan(&assembly_request(request,handoff),token)?,
        MusicVideoPhase::PrepareFrames => return Err(invalid("initial frames are not a downstream phase")),
    };
    let index = plan.payload.inputs.iter().find(|input| input.id == "source_index").ok_or_else(|| state("downstream child lost its explicit frame index"))?;
    if index.content_sha256 != handoff.index_sha256 { return Err(state("downstream plan is not bound to the accepted index bytes")); }
    if !plan.payload.inputs.iter().any(|input| input.id == "source_video" && input.content_sha256 == handoff.source_sha256) {
        return Err(state("downstream plan lost the accepted original temporal source"));
    }
    if let Some(directory) = plan.payload.inputs.iter().find(|input| input.id == "source_frames") {
        if directory.content_sha256 != handoff.directory.content_sha256 || directory.file_count != handoff.directory.file_count || directory.byte_count != handoff.directory.byte_count {
            return Err(state("downstream plan is not bound to the accepted frame directory"));
        }
    } else {
        // Upscayl expands the accepted complete set into per-frame/chunk inputs.
        let frames: Vec<_> = plan.payload.inputs.iter().filter(|input| input.id.starts_with("source_frame_")).collect();
        if frames.len() != handoff.frame_hashes.len() { return Err(state("downstream frame input set differs from the accepted index")); }
        for (position,expected) in handoff.frame_hashes.iter().enumerate() {
            if !frames.iter().any(|input| input.id == format!("source_frame_{position:06}") && &input.content_sha256 == expected) {
                return Err(state("downstream frame bytes differ from the accepted index mapping"));
            }
        }
    }
    Ok(plan)
}

fn skipped(request: &MusicVideoRequest, phase: MusicVideoPhase) -> Option<String> {
    if phase == MusicVideoPhase::Upscale {
        if let MusicVideoUpscale::Disabled { reason } = &request.configuration.upscale { return Some(reason.clone()); }
    }
    None
}

fn expected_plan(request: &MusicVideoRequest, phase: MusicVideoPhase, initial: &MusicVideoPlan, frames: Option<&FrameHandoff>, token: &CancellationToken) -> Result<crate::PipelineV3Plan> {
    if phase == MusicVideoPhase::PrepareFrames {
        return initial.phases[0].plan.clone().ok_or_else(|| state("recipe initial child plan is unavailable"));
    }
    downstream_plan(request,phase,frames.ok_or_else(|| state("downstream child requires accepted predecessor frames"))?,token)
}

fn current_outputs(child: &Path, plan: &crate::PipelineV3Plan, manifest: &crate::PipelineV3RunManifest) -> Result<Vec<crate::PipelineV3Output>> {
    plan.payload.outputs.iter().map(|declaration| {
        // Final run-manifest output evidence is keyed by the public output id;
        // its producer artifact id remains separately available in the plan.
        let evidence = output(manifest,&declaration.id)?;
        Ok(crate::PipelineV3Output { id:declaration.id.clone(),artifact:declaration.artifact.clone(),path:output_path(child,evidence)?,sha256:evidence.sha256.clone() })
    }).collect()
}

fn observe(request: &MusicVideoRequest, root: &Path, intent: &MusicVideoIntent, initial: &MusicVideoPlan, token: &CancellationToken) -> Result<MusicVideoStatus> {
    workspace::validate_layout(root,intent)?;
    let mut phases = Vec::new();
    let mut frames = None;
    let mut blocked = false;
    let mut outputs = Vec::new();
    for phase in [MusicVideoPhase::PrepareFrames,MusicVideoPhase::Upscale,MusicVideoPhase::Assembly] {
        cancelled(token)?;
        if let Some(reason) = skipped(request,phase) {
            phases.push(MusicVideoPhaseStatus { phase,state:MusicVideoPhaseState::Disabled,reason:Some(reason),run_directory:None,plan_sha256:None,manifest:None });
            continue;
        }
        let child = workspace::child(root,phase);
        let exists = workspace::exists(&child)?;
        if blocked {
            if exists { return Err(state("a later recipe child exists without a currently accepted predecessor; preserve the divergent children for explicit reconciliation")); }
            phases.push(MusicVideoPhaseStatus { phase,state:MusicVideoPhaseState::Pending,reason:Some("awaiting accepted predecessor outputs".to_owned()),run_directory:None,plan_sha256:None,manifest:None });
            continue;
        }
        let expected = expected_plan(request,phase,initial,frames.as_ref(),token)?;
        if !exists {
            phases.push(MusicVideoPhaseStatus { phase,state:MusicVideoPhaseState::Ready,reason:None,run_directory:None,plan_sha256:Some(expected.plan_sha256),manifest:None });
            blocked = true;
            continue;
        }
        let retained = match workspace::retained_plan(&child) {
            Ok(plan) => plan,
            Err(error) => {
                phases.push(MusicVideoPhaseStatus { phase,state:MusicVideoPhaseState::IncompleteInitialization,reason:Some(error.message().to_owned()),run_directory:Some(child),plan_sha256:None,manifest:None });
                blocked = true;
                continue;
            },
        };
        if retained != expected { return Err(state("recipe child plan differs from the exact current intent and accepted predecessor outputs; loose or stale runs cannot be adopted")); }
        let manifest = crate::status_v3(&child).map_err(|error| state(format!("recipe child {} has no valid initialized v3 authority: {}; preserve it for explicit reconciliation",phase.id(),error.message())))?;
        let phase_state = match manifest.payload.state {
            crate::PipelineV3RunState::Complete => MusicVideoPhaseState::Complete,
            crate::PipelineV3RunState::Running => MusicVideoPhaseState::Running,
            crate::PipelineV3RunState::Cancelled => MusicVideoPhaseState::Cancelled,
            crate::PipelineV3RunState::Failed | crate::PipelineV3RunState::Interrupted => MusicVideoPhaseState::Failed,
        };
        if phase_state == MusicVideoPhaseState::Complete {
            if phase == MusicVideoPhase::Assembly { outputs = current_outputs(&child,&expected,&manifest)?; }
            else { frames = Some(handoff(request,phase,&child,&manifest)?); }
        } else { blocked = true; }
        phases.push(MusicVideoPhaseStatus { phase,state:phase_state,reason:manifest.payload.diagnostic.clone(),run_directory:Some(child),plan_sha256:Some(expected.plan_sha256),manifest:Some(manifest) });
    }
    let complete = phases.last().is_some_and(|phase| phase.state == MusicVideoPhaseState::Complete);
    Ok(MusicVideoStatus { schema:MUSIC_VIDEO_STATUS_SCHEMA_V1.to_owned(),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256.clone(),complete,phases,outputs })
}

/// Read-only identity and v3 acceptance inspection. It launches no media tools
/// and does not repair missing initialization or reclaim writer locks.
pub fn status(run_directory: impl AsRef<Path>) -> Result<MusicVideoStatus> {
    let root = workspace::root(run_directory.as_ref())?;
    let intent = workspace::read_intent(&root)?;
    let request = intent.request();
    let token = CancellationToken::default();
    let initial = require_intent(&request,&intent,&token)?;
    observe(&request,&root,&intent,&initial,&token)
}

fn run_child<F>(request: &MusicVideoRequest, phase: MusicVideoPhase, frames: Option<&FrameHandoff>, child: &Path,
    expected: &crate::PipelineV3Plan, resume: bool, token: &CancellationToken, progress: &mut F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&MusicVideoProgress) {
    let mut report = |event: &crate::PipelineV3RunProgress| progress(&MusicVideoProgress::Child {
        phase:phase.id().to_owned(),run_directory:child.to_path_buf(),event:event.clone(),
    });
    match phase {
        MusicVideoPhase::PrepareFrames => match &request.configuration.repair {
            MusicVideoRepair::Disabled {..} => {
                let child_request = import_request(request);
                if resume { crate::frame_import::resume(child_request,child,token,report) }
                else { crate::frame_import::run_at(child_request,child,expected,token,report) }
            },
            MusicVideoRepair::Allenk {..} => {
                let child_request = repair_request(request)?;
                if resume { crate::frame_repair::allenk::resume(child_request,child,token,report) }
                else { crate::frame_repair::allenk::run_at(child_request,child,expected,token,report) }
            },
        },
        MusicVideoPhase::Upscale => {
            let child_request = upscale_request(request,frames.ok_or_else(|| state("upscale has no accepted frames"))?)?;
            let on_progress = |event: &crate::upscayl_recovery::UpscaylRecoveryProgress| report(&event.runtime);
            if resume { crate::upscayl_recovery::resume(child_request,child,token,on_progress) }
            else { crate::upscayl_recovery::run_at(child_request,child,expected,token,on_progress) }
        },
        MusicVideoPhase::Assembly => {
            let child_request = assembly_request(request,frames.ok_or_else(|| state("assembly has no accepted frames"))?);
            if resume { crate::video_assembly::resume(child_request,child,token,report) }
            else { crate::video_assembly::run_at(child_request,child,expected,token,report) }
        },
    }
}

fn outcome(status: MusicVideoStatus) -> Result<MusicVideoOutcome> {
    if !status.complete { return Err(state("recipe has not produced an accepted final assembly")); }
    Ok(MusicVideoOutcome { schema:MUSIC_VIDEO_OUTCOME_SCHEMA_V1.to_owned(),run_directory:status.run_directory,intent_sha256:status.intent_sha256,phases:status.phases,outputs:status.outputs })
}

fn advance<F>(request: MusicVideoRequest, root: PathBuf, intent: MusicVideoIntent, token: &CancellationToken, mut progress: F) -> Result<MusicVideoOutcome>
where F: FnMut(&MusicVideoProgress) {
    progress(&MusicVideoProgress::Started { run_directory:root.clone() });
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(&root,"writer.lock")?;
    let initial = require_intent(&request,&intent,token)?;
    let observed = observe(&request,&root,&intent,&initial,token)?;
    if observed.phases.iter().any(|phase| phase.state == MusicVideoPhaseState::IncompleteInitialization) {
        return Err(state("recipe child initialization is incomplete; preserve the named child and reconcile it explicitly before resuming"));
    }
    if !observed.complete {
        let (index,index_identity) = crate::frame_import::read_index(&request.frame_index)?;
        if !intent.payload.initial_inputs.iter().any(|input| input.id == "source_index" && input.content_sha256 == index_identity.0 && input.byte_count == index_identity.1) {
            return Err(state("recipe original index changed before native source admission"));
        }
        // The same assembly observer refuses unsupported native source profiles
        // before import/repair/upscale. This observation grants no checkpoint.
        crate::video_assembly::admit_source(&request.configuration.assembly,&request.source_video,&index.temporal_inspection,&root,token)?;
    }
    let mut frames = None;
    let mut accepted_writers = Vec::new();
    for phase in [MusicVideoPhase::PrepareFrames,MusicVideoPhase::Upscale,MusicVideoPhase::Assembly] {
        if skipped(&request,phase).is_some() { continue; }
        cancelled(token)?;
        if workspace::read_intent(&root)? != intent { return Err(state("recipe intent changed during execution")); }
        workspace::validate_layout(&root,&intent)?;
        let initial = require_intent(&request,&intent,token)?;
        let expected = expected_plan(&request,phase,&initial,frames.as_ref(),token)?;
        let child = workspace::child(&root,phase);
        let exists = workspace::exists(&child)?;
        let complete = if exists {
            if workspace::retained_plan(&child)? != expected { return Err(state("existing child differs from the accepted handoff; a new child cannot replace it")); }
            crate::status_v3(&child)?.payload.state == crate::PipelineV3RunState::Complete
        } else { false };
        if !complete { run_child(&request,phase,frames.as_ref(),&child,&expected,exists,token,&mut progress)?; }
        // Block cooperating direct child writers until this recipe's handoffs
        // finish. A stale lock is preserved and never silently reclaimed.
        accepted_writers.push(crate::cache_v3::ExclusiveWriter::acquire(&child,"writer.lock")?);
        if workspace::retained_plan(&child)? != expected { return Err(state("child plan changed before acceptance handoff")); }
        let manifest = crate::status_v3(&child)?;
        if manifest.payload.state != crate::PipelineV3RunState::Complete { return Err(state("child returned without complete verified v3 acceptance")); }
        if phase != MusicVideoPhase::Assembly { frames = Some(handoff(&request,phase,&child,&manifest)?); }
    }
    let initial = require_intent(&request,&intent,token)?;
    let finished = observe(&request,&root,&intent,&initial,token)?;
    let result = outcome(finished);
    drop(accepted_writers);
    result
}

/// Create an isolated recipe root and run the selected fixed child sequence.
pub fn run<F>(request: MusicVideoRequest, output_directory: Option<PathBuf>, token: &CancellationToken, progress: F) -> Result<MusicVideoOutcome>
where F: FnMut(&MusicVideoProgress) {
    let admitted = plan(&request,token)?;
    cancelled(token)?;
    let root = workspace::create(&request,output_directory,&admitted.intent)?;
    advance(request,root,admitted.intent,token,progress)
}

/// Resume only the exact intent and fixed child slots. Changed bytes or pins,
/// unknown child runs and incomplete initialization are explicit refusals.
pub fn resume<F>(request: MusicVideoRequest, run_directory: impl AsRef<Path>, token: &CancellationToken, progress: F) -> Result<MusicVideoOutcome>
where F: FnMut(&MusicVideoProgress) {
    let root = workspace::root(run_directory.as_ref())?;
    let intent = workspace::read_intent(&root)?;
    require_intent(&request,&intent,token)?;
    workspace::validate_layout(&root,&intent)?;
    advance(request,root,intent,token,progress)
}
