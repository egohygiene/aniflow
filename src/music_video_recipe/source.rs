//! Source entry for the same fixed recipe coordinator. The extraction child
//! owns its acceptance; the indexed tail is derived only from accepted bytes.
use super::*;
use crate::frame_extraction::FrameExtractionRequest;

impl SourceMusicVideoConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 131_072 { return Err(invalid("source music-video recipe configuration exceeds 128 KiB")); }
        let value: Self = decode_json(bytes,"source music-video recipe configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != SOURCE_MUSIC_VIDEO_CONFIGURATION_SCHEMA_V1 { return Err(invalid("unsupported source music-video recipe schema")); }
        self.extraction.validate()?;
        self.recipe.validate()
    }
}

impl SourceMusicVideoIntent {
    pub fn validate(&self) -> Result<()> {
        self.payload.configuration.validate()?;
        crate::provider::require_sha256(&self.payload.initial_plan_sha256,"source recipe extraction plan")?;
        if self.schema != SOURCE_MUSIC_VIDEO_INTENT_SCHEMA_V1
            || self.payload.initial_inputs.len() != 1
            || self.payload.initial_inputs[0].id != "source_video"
            || canonical_sha256(&self.payload)? != self.intent_sha256 {
            return Err(state("source recipe intent is malformed or differs from its immutable digest"));
        }
        Ok(())
    }
    pub(super) fn request(&self) -> SourceMusicVideoRequest {
        SourceMusicVideoRequest::new(&self.payload.source_video,self.payload.configuration.clone(),&self.payload.provider_executable)
    }
}

/// Inspect the bounded immutable entry contract without launching native tools.
/// Unknown schemas are refused rather than guessed from directory contents.
pub fn entry_kind(run_directory: impl AsRef<Path>) -> Result<MusicVideoEntryKind> {
    let root = workspace::root(run_directory.as_ref())?;
    let document: serde_json::Value = workspace::read_document(&root.join("intent.json"))?;
    match document.get("schema").and_then(serde_json::Value::as_str) {
        Some(MUSIC_VIDEO_INTENT_SCHEMA_V1) => {
            let intent: MusicVideoIntent = serde_json::from_value(document).map_err(|_| state("invalid indexed recipe intent"))?;
            intent.validate()?;
            Ok(MusicVideoEntryKind::Indexed)
        },
        Some(SOURCE_MUSIC_VIDEO_INTENT_SCHEMA_V1) => {
            let intent: SourceMusicVideoIntent = serde_json::from_value(document).map_err(|_| state("invalid source recipe intent"))?;
            intent.validate()?;
            Ok(MusicVideoEntryKind::Source)
        },
        _ => Err(state("unsupported music-video intent entry schema")),
    }
}

fn extraction_request(request: &SourceMusicVideoRequest) -> FrameExtractionRequest {
    FrameExtractionRequest::new(&request.source_video,request.configuration.extraction.clone(),&request.provider_executable)
}

fn components(request: &SourceMusicVideoRequest, token: &CancellationToken) -> Result<Vec<MusicVideoComponent>> {
    // Target mode cannot decide whether neural files are needed until accepted
    // extracted PNGs supply geometry. Their exact declared pins remain frozen
    // in configuration, and the existing indexed planner checks them on execute.
    let upscale = match &request.configuration.recipe.upscale {
        MusicVideoUpscale::Upscayl { configuration } => Some(configuration),
        MusicVideoUpscale::Disabled {..} | MusicVideoUpscale::TargetResolution {..} => None,
    };
    let mut values = components_for_configuration(&request.configuration.recipe,&request.provider_executable,upscale,token)?;
    for (id,pin) in [("extraction-ffmpeg",&request.configuration.extraction.ffmpeg),("extraction-ffprobe",&request.configuration.extraction.ffprobe)] {
        cancelled(token)?;
        verify_pin(pin,id).map_err(|_| Error::new(ErrorCategory::Dependency,"selected source extraction tool is unavailable or changed"))?;
        let observed = identity(&pin.executable,134_217_728)?;
        if observed.0 != pin.sha256 { return Err(invalid("source extraction tool changed during recipe admission")); }
        values.push(MusicVideoComponent { id:id.to_owned(),path:pin.executable.clone(),sha256:observed.0,byte_count:observed.1 });
    }
    Ok(values)
}

fn pending_phase(configuration: &MusicVideoConfiguration, phase: SourceMusicVideoPhase) -> SourceMusicVideoPhasePlan {
    if phase == SourceMusicVideoPhase::Upscale {
        if let MusicVideoUpscale::Disabled { reason } = &configuration.upscale {
            return SourceMusicVideoPhasePlan { phase,state:MusicVideoPhaseState::Disabled,reason:Some(reason.clone()),plan:None };
        }
    }
    SourceMusicVideoPhasePlan { phase,state:MusicVideoPhaseState::Pending,
        reason:Some("awaiting accepted extracted source frames; geometry and target decision are not yet observed".to_owned()),plan:None }
}

/// Bind existing source/component identities and the exact extraction plan.
/// All later media geometry, target decisions and plans remain pending.
pub fn plan_source(request: &SourceMusicVideoRequest, token: &CancellationToken) -> Result<SourceMusicVideoPlan> {
    cancelled(token)?;
    request.configuration.validate()?;
    let first = crate::frame_extraction::plan(&extraction_request(request),token)?;
    let payload = SourceMusicVideoIntentPayload {
        configuration:request.configuration.clone(),source_video:request.source_video.clone(),provider_executable:request.provider_executable.clone(),
        components:components(request,token)?,initial_inputs:first.payload.inputs.clone(),initial_plan_sha256:first.plan_sha256.clone(),
    };
    let intent = SourceMusicVideoIntent { schema:SOURCE_MUSIC_VIDEO_INTENT_SCHEMA_V1.to_owned(),intent_sha256:canonical_sha256(&payload)?,payload };
    intent.validate()?;
    cancelled(token)?;
    let mut phases = vec![SourceMusicVideoPhasePlan { phase:SourceMusicVideoPhase::ExtractFrames,state:MusicVideoPhaseState::Ready,reason:None,plan:Some(first) }];
    phases.extend([SourceMusicVideoPhase::PrepareFrames,SourceMusicVideoPhase::Upscale,SourceMusicVideoPhase::Assembly]
        .into_iter().map(|phase| pending_phase(&request.configuration.recipe,phase)));
    Ok(SourceMusicVideoPlan { schema:SOURCE_MUSIC_VIDEO_PLAN_SCHEMA_V1.to_owned(),intent,output_width:None,output_height:None,target_resolution:None,phases })
}

pub(super) fn require_source_intent(request: &SourceMusicVideoRequest, intent: &SourceMusicVideoIntent, token: &CancellationToken) -> Result<SourceMusicVideoPlan> {
    let current = plan_source(request,token)?;
    if current.intent != *intent { return Err(state("source recipe input, configuration, selected component pins or provider changed; start a new recipe run")); }
    Ok(current)
}

fn extraction_plan(initial: &SourceMusicVideoPlan) -> Result<&crate::PipelineV3Plan> {
    initial.phases.first().filter(|phase| phase.phase == SourceMusicVideoPhase::ExtractFrames)
        .and_then(|phase| phase.plan.as_ref()).ok_or_else(|| state("source recipe has no exact extraction plan"))
}

fn phase(phase: MusicVideoPhase) -> SourceMusicVideoPhase { match phase {
    MusicVideoPhase::PrepareFrames => SourceMusicVideoPhase::PrepareFrames,
    MusicVideoPhase::Upscale => SourceMusicVideoPhase::Upscale,
    MusicVideoPhase::Assembly => SourceMusicVideoPhase::Assembly,
} }
fn phase_status(value: MusicVideoPhaseStatus) -> SourceMusicVideoPhaseStatus {
    SourceMusicVideoPhaseStatus { phase:phase(value.phase),state:value.state,reason:value.reason,run_directory:value.run_directory,
        plan_sha256:value.plan_sha256,manifest:value.manifest }
}

fn pending_status(configuration: &MusicVideoConfiguration, phase: SourceMusicVideoPhase) -> SourceMusicVideoPhaseStatus {
    let value = pending_phase(configuration,phase);
    SourceMusicVideoPhaseStatus { phase,state:value.state,reason:value.reason,run_directory:None,plan_sha256:None,manifest:None }
}

fn require_no_tail(root: &Path) -> Result<()> {
    for phase in [MusicVideoPhase::PrepareFrames,MusicVideoPhase::Upscale,MusicVideoPhase::Assembly] {
        if workspace::exists(&workspace::child(root,phase))? {
            return Err(state("source recipe has a later child without accepted extraction; preserve divergent children for explicit reconciliation"));
        }
    }
    Ok(())
}

fn extraction_status(root: &Path, expected: &crate::PipelineV3Plan) -> Result<SourceMusicVideoPhaseStatus> {
    let child = workspace::extraction_child(root);
    if !workspace::exists(&child)? {
        return Ok(SourceMusicVideoPhaseStatus { phase:SourceMusicVideoPhase::ExtractFrames,state:MusicVideoPhaseState::Ready,reason:None,
            run_directory:None,plan_sha256:Some(expected.plan_sha256.clone()),manifest:None });
    }
    let retained = match workspace::retained_plan(&child) {
        Ok(plan) => plan,
        Err(error) => return Ok(SourceMusicVideoPhaseStatus { phase:SourceMusicVideoPhase::ExtractFrames,state:MusicVideoPhaseState::IncompleteInitialization,
            reason:Some(error.message().to_owned()),run_directory:Some(child),plan_sha256:None,manifest:None }),
    };
    if retained != *expected { return Err(state("source extraction child differs from the exact source recipe intent")); }
    let manifest = crate::status_v3(&child).map_err(|error| state(format!("source extraction child has no valid v3 authority: {}; preserve it for reconciliation",error.message())))?;
    let phase_state = match manifest.payload.state {
        crate::PipelineV3RunState::Complete => MusicVideoPhaseState::Complete,
        crate::PipelineV3RunState::Running => MusicVideoPhaseState::Running,
        crate::PipelineV3RunState::Cancelled => MusicVideoPhaseState::Cancelled,
        crate::PipelineV3RunState::Failed | crate::PipelineV3RunState::Interrupted => MusicVideoPhaseState::Failed,
    };
    Ok(SourceMusicVideoPhaseStatus { phase:SourceMusicVideoPhase::ExtractFrames,state:phase_state,reason:manifest.payload.diagnostic.clone(),
        run_directory:Some(child),plan_sha256:Some(expected.plan_sha256.clone()),manifest:Some(manifest) })
}

fn resolved_status(extracted: SourceMusicVideoPhaseStatus, tail: MusicVideoStatus, initial: &MusicVideoPlan) -> SourceMusicVideoStatus {
    let mut phases = vec![extracted];
    phases.extend(tail.phases.into_iter().map(phase_status));
    SourceMusicVideoStatus { schema:SOURCE_MUSIC_VIDEO_STATUS_SCHEMA_V1.to_owned(),run_directory:tail.run_directory,intent_sha256:tail.intent_sha256,
        complete:tail.complete,output_width:Some(initial.output_width),output_height:Some(initial.output_height),
        target_resolution:tail.target_resolution,phases,outputs:tail.outputs }
}

fn observe_source(request: &SourceMusicVideoRequest, root: &Path, intent: &SourceMusicVideoIntent, initial: &SourceMusicVideoPlan, token: &CancellationToken) -> Result<SourceMusicVideoStatus> {
    workspace::validate_source_layout(root,intent)?;
    let expected = extraction_plan(initial)?;
    let extracted = extraction_status(root,expected)?;
    let mut phases = vec![extracted.clone()];
    if extracted.state != MusicVideoPhaseState::Complete {
        require_no_tail(root)?;
        phases.extend([SourceMusicVideoPhase::PrepareFrames,SourceMusicVideoPhase::Upscale,SourceMusicVideoPhase::Assembly]
            .into_iter().map(|phase| pending_status(&request.configuration.recipe,phase)));
        return Ok(SourceMusicVideoStatus { schema:SOURCE_MUSIC_VIDEO_STATUS_SCHEMA_V1.to_owned(),run_directory:root.to_path_buf(),intent_sha256:intent.intent_sha256.clone(),
            complete:false,output_width:None,output_height:None,target_resolution:None,phases,outputs:Vec::new() });
    }
    let manifest = extracted.manifest.as_ref().ok_or_else(|| state("complete extraction has no accepted manifest"))?;
    let (indexed,indexed_plan) = source_handoff::indexed(request,&workspace::extraction_child(root),expected,manifest,token)?;
    let tail = observe_tail(&indexed,root,&intent.intent_sha256,&indexed_plan,token)?;
    if extraction_status(root,expected)? != extracted { return Err(state("accepted extraction changed during source recipe status")); }
    Ok(resolved_status(extracted,tail,&indexed_plan))
}

/// Read only the exact source intent and current child acceptance. No probe,
/// decoder, writer reclamation or incomplete-state repair runs here.
pub fn status_source(run_directory: impl AsRef<Path>) -> Result<SourceMusicVideoStatus> {
    let root = workspace::root(run_directory.as_ref())?;
    let intent = workspace::read_source_intent(&root)?;
    let request = intent.request();
    let token = CancellationToken::default();
    let initial = require_source_intent(&request,&intent,&token)?;
    observe_source(&request,&root,&intent,&initial,&token)
}

fn outcome(value: SourceMusicVideoStatus) -> Result<SourceMusicVideoOutcome> {
    if !value.complete { return Err(state("source recipe has not produced accepted final assembly")); }
    Ok(SourceMusicVideoOutcome { schema:SOURCE_MUSIC_VIDEO_OUTCOME_SCHEMA_V1.to_owned(),run_directory:value.run_directory,intent_sha256:value.intent_sha256,
        output_width:value.output_width.ok_or_else(|| state("complete source recipe has no observed output width"))?,
        output_height:value.output_height.ok_or_else(|| state("complete source recipe has no observed output height"))?,
        target_resolution:value.target_resolution,phases:value.phases,outputs:value.outputs })
}

fn advance_source<F>(request: SourceMusicVideoRequest, root: PathBuf, intent: SourceMusicVideoIntent, token: &CancellationToken, mut progress: F) -> Result<SourceMusicVideoOutcome>
where F: FnMut(&MusicVideoProgress) {
    progress(&MusicVideoProgress::Started { run_directory:root.clone() });
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(&root,"writer.lock")?;
    let initial = require_source_intent(&request,&intent,token)?;
    RecipeAuthority::Source(&intent).require_current(&root,token)?;
    let expected = extraction_plan(&initial)?;
    let child = workspace::extraction_child(&root);
    let observed = extraction_status(&root,expected)?;
    if observed.state == MusicVideoPhaseState::IncompleteInitialization {
        return Err(state("source extraction initialization is incomplete; preserve its directory and reconcile explicitly before resuming"));
    }
    if observed.state != MusicVideoPhaseState::Complete {
        require_no_tail(&root)?;
        let existed = workspace::exists(&child)?;
        let mut report = |event: &crate::PipelineV3RunProgress| progress(&MusicVideoProgress::Child {
            phase:"extract_frames".to_owned(),run_directory:child.clone(),event:event.clone(),
        });
        if existed { crate::frame_extraction::resume(extraction_request(&request),&child,token,&mut report)?; }
        else { crate::frame_extraction::run_at(extraction_request(&request),&child,expected,token,&mut report)?; }
    }
    // Keep accepted extraction protected until every downstream handoff and the
    // final observation completes, including an already-complete resume.
    let _extraction_writer = crate::cache_v3::ExclusiveWriter::acquire(&child,"writer.lock")?;
    RecipeAuthority::Source(&intent).require_current(&root,token)?;
    let extracted = extraction_status(&root,expected)?;
    if extracted.state != MusicVideoPhaseState::Complete { return Err(state("extraction returned without current complete acceptance")); }
    let manifest = extracted.manifest.as_ref().ok_or_else(|| state("accepted extraction has no manifest"))?;
    let (indexed,indexed_plan) = source_handoff::indexed(&request,&child,expected,manifest,token)?;
    let tail = advance_tail(&indexed,&root,&indexed_plan.intent,RecipeAuthority::Source(&intent),token,&mut progress)?;
    RecipeAuthority::Source(&intent).require_current(&root,token)?;
    if extraction_status(&root,expected)? != extracted { return Err(state("source extraction acceptance changed during downstream execution")); }
    // The shared tail observed final acceptance while holding every accepted
    // child writer. Preserve that observation rather than reopening children
    // after those guards have been released.
    outcome(resolved_status(extracted,tail,&indexed_plan))
}

pub fn run_source<F>(request: SourceMusicVideoRequest, output_directory: Option<PathBuf>, token: &CancellationToken, progress: F) -> Result<SourceMusicVideoOutcome>
where F: FnMut(&MusicVideoProgress) {
    let admitted = plan_source(&request,token)?;
    cancelled(token)?;
    let root = workspace::create_source(&request,output_directory,&admitted.intent)?;
    advance_source(request,root,admitted.intent,token,progress)
}

pub fn resume_source<F>(request: SourceMusicVideoRequest, run_directory: impl AsRef<Path>, token: &CancellationToken, progress: F) -> Result<SourceMusicVideoOutcome>
where F: FnMut(&MusicVideoProgress) {
    let root = workspace::root(run_directory.as_ref())?;
    let intent = workspace::read_source_intent(&root)?;
    require_source_intent(&request,&intent,token)?;
    workspace::validate_source_layout(&root,&intent)?;
    advance_source(request,root,intent,token,progress)
}
