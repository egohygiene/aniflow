//! A bounded, original-clock PNG excerpt using the existing native checkpoints.
//! Preview acceptance covers only the selected frames. It creates neither a
//! complete Upscayl index nor a delivery, and cannot authorize full-run reuse.
use super::*;
use serde::Deserialize;
use crate::frame_selection::{InclusiveFrameRange, SelectedFrameRange};
use crate::upscale_target::{UpscaleTargetDecision, UpscaleTargetPolicy, UpscaleTargetResources};

pub const PREVIEW_CONFIGURATION_SCHEMA_V1: &str = "aniflow.upscayl-preview-configuration/v1";
pub const PREVIEW_PLAN_SCHEMA_V1: &str = "aniflow.upscayl-preview-plan/v1";
pub const PREVIEW_OUTCOME_SCHEMA_V1: &str = "aniflow.upscayl-preview-outcome/v1";
const PLAN_PREFIX: &str = "upscayl-preview-";
const MAXIMUM_PREVIEW_FRAMES: u64 = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewConfiguration {
    pub schema: String,
    pub settings: UpscaylRecoveryConfiguration,
    pub target_resolution: UpscaleTargetPolicy,
}

impl PreviewConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("Upscayl preview configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes,"Upscayl preview configuration")?;
        value.validate_intent()?;
        Ok(value)
    }

    fn validate_intent(&self) -> Result<()> {
        if self.schema != PREVIEW_CONFIGURATION_SCHEMA_V1 { return Err(invalid("unsupported Upscayl preview configuration schema")); }
        // A no-change target is refused before neural settings or pins matter.
        self.target_resolution.validate()
    }
}

#[derive(Debug, Clone)]
pub struct PreviewRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: PreviewConfiguration,
    pub range: InclusiveFrameRange,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}

impl PreviewRequest {
    #[must_use]
    pub fn new(source_video: impl Into<PathBuf>, frames_directory: impl Into<PathBuf>, frame_index: impl Into<PathBuf>,
        configuration: PreviewConfiguration, range: InclusiveFrameRange, provider_executable: impl Into<PathBuf>) -> Self {
        let maximum_artifact_bytes = configuration.settings.maximum_working_bytes;
        Self { source_video:source_video.into(),frames_directory:frames_directory.into(),frame_index:frame_index.into(),
            configuration,range,provider_executable:provider_executable.into(),execution_limits:crate::ProviderExecutionLimits {
                timeout:std::time::Duration::from_secs(21_600),termination_grace_period:std::time::Duration::from_millis(2_000),
                maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,
                maximum_artifact_files:MAXIMUM_PREVIEW_FRAMES+3,maximum_artifact_bytes,
            } }
    }
}

/// A PNG excerpt is not evidence that the original encoded source will pass
/// the recipe's separate native stream, audio, color and timing admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewScope { SelectedIndexedPngsOnly }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewPlan {
    pub schema: String,
    pub scope: PreviewScope,
    pub selected: SelectedFrameRange,
    pub source_frame_count: u64,
    pub target_resolution: UpscaleTargetDecision,
    pub resources: UpscaleTargetResources,
    pub required_components: Vec<String>,
    pub pipeline: crate::PipelineV3Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewOutcome {
    pub schema: String,
    pub scope: PreviewScope,
    pub selected: SelectedFrameRange,
    pub source_frame_count: u64,
    pub target_resolution: UpscaleTargetDecision,
    pub resources: UpscaleTargetResources,
    pub required_components: Vec<String>,
    pub runtime: crate::PipelineV3RunOutcome,
}

struct PreparedPreview {
    view: PreviewPlan,
    settings: UpscaylRecoveryConfiguration,
    registry: crate::ProviderRegistry,
    bindings: Vec<crate::PipelineInputBinding>,
    frame_weights: BTreeMap<String,u64>,
    required_free_bytes: u64,
}

fn bounded_selection(selected: &SelectedFrameRange, execution: UpscaylExecutionPolicy) -> Result<()> {
    let duration = selected.source_time.duration().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let ten_seconds = crate::temporal::RationalTime::new(10,1).map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    if selected.frame_count == 0 || selected.frame_count > MAXIMUM_PREVIEW_FRAMES || duration > ten_seconds {
        return Err(invalid("Upscayl preview selects at most 256 frames and ten exact source seconds"));
    }
    match execution {
        UpscaylExecutionPolicy::PerFrame if selected.end_ordinal_exclusive > UPSCAYL_MAXIMUM_FRAMES as u64 =>
            Err(invalid("per-frame preview retains original ordinals and requires selected frames within the first 1024 source frames")),
        UpscaylExecutionPolicy::BoundedChunk { frames_per_chunk, .. }
            if frames_per_chunk == 0 || selected.start_ordinal % u64::from(frames_per_chunk) != 0 =>
            Err(invalid("chunk preview must start on the configured original chunk boundary; execution mode and ordinals are never changed")),
        _ => Ok(()),
    }
}

fn prepare(request: &PreviewRequest, cancellation: &CancellationToken) -> Result<PreparedPreview> {
    check_cancelled(cancellation)?;
    request.configuration.validate_intent()?;
    let (index,index_identity) = frame_import::read_index(&request.frame_index)?;
    let selection = crate::frame_selection::plan_from_rife_index(&index,&[request.range])?;
    let selected = selection.ranges.into_iter().next().ok_or_else(|| invalid("preview range is missing"))?;
    bounded_selection(&selected,request.configuration.settings.execution)?;
    // Observation bounds belong to PNG input admission, not neural tools.
    let target_input = crate::upscale_target::observe_selected_input(&request.frames_directory,&request.frame_index,
        &request.range,request.configuration.settings.import.maximum_frame_bytes,
        request.configuration.settings.import.maximum_total_frame_bytes,cancellation)?;
    if target_input.frame_index_sha256 != index_identity.0 || target_input.frame_count != selected.frame_count
        || (target_input.width,target_input.height) != (index.width,index.height) {
        return Err(invalid("preview input index changed during target observation"));
    }
    let target_resolution = crate::upscale_target::decide(&target_input,Some(&request.configuration.target_resolution))?;
    let settings = target_resolution.resolved_configuration(&request.configuration.settings)?
        .ok_or_else(|| invalid("preview target leaves the current frames unchanged; no neural preview run is needed"))?;
    request.execution_limits.validate()?;
    let source = identity(&request.source_video,settings.import.maximum_source_bytes)?;
    if source != (index.temporal_inspection.source_sha256.clone(),index.temporal_inspection.source_size_bytes) {
        return Err(invalid("preview source bytes differ from the complete original frame index"));
    }
    let resources = target_resolution.resources(&settings)?;
    let component_bytes = verify_components(&settings,cancellation)?;
    let timeline = index.temporal_inspection.video_timeline().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let index_digest = canonical_sha256(&index)?;
    let mut specs = Vec::with_capacity(selected.frame_count as usize);
    let mut bindings = vec![crate::PipelineInputBinding::new("source_video",&request.source_video),crate::PipelineInputBinding::new("source_index",&request.frame_index)];
    for frame in &index.frames[selected.start_ordinal as usize..selected.end_ordinal_exclusive as usize] {
        check_cancelled(cancellation)?;
        let path = request.frames_directory.join(&frame.filename);
        let observed = identity(&path,settings.import.maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(invalid("selected PNG changed during preview planning")); }
        specs.push(UpscaylFrameSpec { source_sha256:source.0.clone(),source_byte_count:source.1,frame_index_sha256:index_digest.clone(),
            ordinal:frame.ordinal,filename:frame.filename.clone(),source_time:timeline.frames[frame.ordinal as usize].source_time,
            input_sha256:observed.0,input_byte_count:observed.1,input_width:index.width,input_height:index.height });
        bindings.push(crate::PipelineInputBinding::new(format!("source_frame_{:06}",frame.ordinal),path));
    }
    let mut entries: Vec<_> = specs.iter().map(|frame| crate::pipeline_v3::DirectoryContentEntry {
        relative_path:frame.filename.clone(),kind:crate::PipelineInputKind::File,byte_count:frame.input_byte_count,
        content_sha256:Some(frame.input_sha256.clone()),
    }).collect();
    entries.sort_by(|left,right| left.relative_path.cmp(&right.relative_path));
    if canonical_sha256(&entries)? != target_input.frames_sha256
        || specs.iter().map(|frame| frame.input_byte_count).sum::<u64>() != target_input.frames_byte_count {
        return Err(invalid("selected frame identities changed after the target-resolution observation"));
    }
    let name = format!("{PLAN_PREFIX}{}",canonical_sha256(&(&request.configuration,&selected))?);
    let (authored,registry,frame_weights) = graph(&name,&settings,&specs,&request.provider_executable)?;
    let mut context = crate::audio_inspection::context();
    context.allowed_side_effects.push(crate::SideEffect::Gpu);
    let pipeline = crate::resolve_pipeline_v3(&authored,&bindings,&registry,&context)
        .map_err(|failure| Error::new(failure.category(),failure.message))?;
    let index_input = pipeline.payload.inputs.iter().find(|input| input.id == "source_index").ok_or_else(|| invalid("preview index binding is absent"))?;
    if (index_input.content_sha256.clone(),index_input.byte_count) != index_identity {
        return Err(invalid("preview index changed while resolving its input identity"));
    }
    super::preflight_plan(&pipeline,&registry,cancellation)?;
    if canonical_json_bytes(&pipeline)?.len() as u64 > MAXIMUM_PLAN_BYTES { return Err(invalid("preview plan exceeds 64 MiB")); }
    let count = selected.frame_count;
    let batch = settings.execution.frames_per_chunk().unwrap_or(1) as u64;
    let output_bound = count.checked_mul(settings.maximum_output_frame_bytes).ok_or_else(|| invalid("preview output byte bound overflow"))?;
    let metadata_bound = (count+1).checked_pow(2).and_then(|value| value.checked_mul(16_384)).ok_or_else(|| invalid("preview metadata byte bound overflow"))?;
    let required_free_bytes = output_bound.checked_mul(3)
        .and_then(|value| value.checked_add(source.1.checked_mul(2)?))
        .and_then(|value| value.checked_add(target_input.frames_byte_count.checked_mul(2)?))
        .and_then(|value| value.checked_add(component_bytes.checked_mul(2)?))
        .and_then(|value| value.checked_add(metadata_bound))
        .and_then(|value| value.checked_add(batch.checked_mul(settings.import.maximum_frame_bytes+settings.maximum_output_frame_bytes)?))
        .and_then(|value| value.checked_add(4*UPSCAYL_MAXIMUM_REPORT_BYTES))
        .ok_or_else(|| invalid("preview working-data bound overflow"))?;
    if required_free_bytes > settings.maximum_working_bytes { return Err(invalid("bounded preview inputs, candidates, accepted copies and scratch exceed maximum_working_bytes")); }
    let maximum_stage_frames = batch.min(count);
    if request.execution_limits.maximum_artifact_files < maximum_stage_frames+2
        || request.execution_limits.maximum_artifact_bytes < maximum_stage_frames*settings.maximum_output_frame_bytes+UPSCAYL_MAXIMUM_CHUNK_REPORT_BYTES {
        return Err(invalid("preview provider artifact limits cannot hold the selected native checkpoint"));
    }
    check_cancelled(cancellation)?;
    Ok(PreparedPreview { view:PreviewPlan { schema:PREVIEW_PLAN_SCHEMA_V1.to_owned(),scope:PreviewScope::SelectedIndexedPngsOnly,selected,source_frame_count:index.frames.len() as u64,
        target_resolution,resources,required_components:["upscayl","upscayl-ffmpeg","upscayl-param","upscayl-weights"].into_iter().map(str::to_owned).collect(),pipeline },
        settings,registry,bindings,frame_weights,required_free_bytes })
}

/// Build only the selected native stages. No collection/output-index stage is
/// authored, so the ordinary v3 final outputs describe this excerpt alone.
fn graph(name: &str, settings: &UpscaylRecoveryConfiguration, specs: &[UpscaylFrameSpec], executable: &Path)
    -> Result<(crate::PipelineV3Configuration,crate::ProviderRegistry,BTreeMap<String,u64>)> {
    let mut inputs = vec![json!({"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}),
        json!({"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"})];
    for frame in specs { inputs.push(json!({"id":format!("source_frame_{:06}",frame.ordinal),"artifact_type":"image/png","artifact_role":"intermediate","stream_role":"video"})); }
    let size = settings.execution.frames_per_chunk().unwrap_or(1) as usize;
    if size == 0 { return Err(invalid("preview chunk size cannot be zero")); }
    let mut stages = Vec::new();
    let mut outputs = Vec::new();
    let mut registry = crate::ProviderRegistry::new();
    let mut weights = BTreeMap::new();
    for chunk in specs.chunks(size) {
        let ordinal = chunk[0].ordinal;
        let (stage_id,frame_id,evidence_id,capability,port,kind,frame_path,evidence_path,manifest,configuration,implementation) = if settings.execution.is_per_frame() {
            let operation = UpscaylFrameOperation { schema:UPSCAYL_FRAME_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),frame:chunk[0].clone() };
            (format!("upscale_frame_{ordinal:06}"),format!("frame_{ordinal:06}"),format!("evidence_{ordinal:06}"),UPSCAYL_FRAME_CAPABILITY_ID,"frame","file",
                format!("artifacts/preview/frames/{ordinal:06}/frame.png"),format!("artifacts/preview/frames/{ordinal:06}/frame.json"),operation.manifest()?,operation.provider_configuration()?,"aniflow-upscayl-frame-v1")
        } else {
            let chunk_ordinal = ordinal / size as u64;
            let operation = UpscaylChunkOperation { schema:UPSCAYL_CHUNK_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),chunk_ordinal,frames:chunk.to_vec() };
            (format!("upscale_chunk_{chunk_ordinal:06}"),format!("chunk_frames_{chunk_ordinal:06}"),format!("chunk_evidence_{chunk_ordinal:06}"),UPSCAYL_CHUNK_CAPABILITY_ID,"frames","directory",
                format!("artifacts/preview/chunks/{chunk_ordinal:06}/frames"),format!("artifacts/preview/chunks/{chunk_ordinal:06}/chunk.json"),operation.manifest()?,operation.provider_configuration()?,"aniflow-upscayl-chunk-v1")
        };
        registry.register(crate::ProviderRegistration::new(&stage_id,manifest,configuration,executable,implementation,inventory(settings))?)?;
        let frame_integrity = format!("{frame_id}_integrity");
        let evidence_integrity = format!("{evidence_id}_integrity");
        let input_ids: Vec<_> = chunk.iter().map(|frame| format!("source_frame_{:06}",frame.ordinal)).collect();
        stages.push(json!({"id":stage_id,"depends_on":[],"capability":{"id":capability,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":stage_id},"fallbacks":[]},
            "inputs":[{"port":port,"artifacts":input_ids}],"outputs":[{"port":port,"artifacts":[{"id":frame_id,"relative_path":frame_path,"kind":kind}]},
                {"port":"evidence","artifacts":[{"id":evidence_id,"relative_path":evidence_path,"kind":"file"}]}],
            "validations":[{"id":frame_integrity,"artifact":frame_id,"contract":"aniflow.validation/artifact-integrity/v1"},{"id":evidence_integrity,"artifact":evidence_id,"contract":"aniflow.validation/artifact-integrity/v1"}]}));
        outputs.push(json!({"id":frame_id,"artifact":frame_id,"required_validations":[frame_integrity]}));
        outputs.push(json!({"id":evidence_id,"artifact":evidence_id,"required_validations":[evidence_integrity]}));
        weights.insert(stage_id,chunk.len() as u64);
    }
    let authored = json!({"schema":"aniflow.pipeline/v3","name":name,"inputs":inputs,"stages":stages,"outputs":outputs});
    let configuration = crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&authored)?)
        .map_err(|failure| Error::new(failure.category(),failure.message))?;
    Ok((configuration,registry,weights))
}

/// Generic v3 entry points recognize only this exact excerpt topology. Native
/// frame/chunk preflight continues to own the component-file observations.
pub(super) fn preflight_plan(plan: &crate::PipelineV3Plan, registry: &crate::ProviderRegistry) -> Result<()> {
    let Some(suffix) = plan.payload.pipeline_name.strip_prefix(PLAN_PREFIX) else { return Ok(()); };
    require_sha256(suffix,"preview intent identity")?;
    let mut settings = None;
    let mut executable = None;
    let mut specs = Vec::new();
    for stage in &plan.payload.stages {
        let registration = registry.registration(&stage.provider_lock.payload.registration_id).ok_or_else(|| invalid("preview native registration is missing"))?;
        let values = serde_json::to_value(&registration.configuration().values).map_err(|error| invalid(error.to_string()))?;
        let (candidate,frames) = match stage.capability.id.as_str() {
            UPSCAYL_FRAME_CAPABILITY_ID => {
                let operation: UpscaylFrameOperation = serde_json::from_value(values).map_err(|_| invalid("invalid preview frame operation"))?;
                operation.validate()?;
                (operation.settings,vec![operation.frame])
            }
            UPSCAYL_CHUNK_CAPABILITY_ID => {
                let operation: UpscaylChunkOperation = serde_json::from_value(values).map_err(|_| invalid("invalid preview chunk operation"))?;
                operation.validate()?;
                (operation.settings,operation.frames)
            }
            _ => return Err(invalid("preview plans contain only native selected-frame or chunk stages, never a collector")),
        };
        if settings.as_ref().is_some_and(|previous| previous != &candidate)
            || executable.as_ref().is_some_and(|previous: &PathBuf| previous != registration.executable()) {
            return Err(invalid("preview stages must use identical effective settings and provider executable"));
        }
        settings = Some(candidate);
        executable = Some(registration.executable().to_path_buf());
        specs.extend(frames);
    }
    let settings = settings.ok_or_else(|| invalid("preview plan has no selected native stages"))?;
    let executable = executable.ok_or_else(|| invalid("preview provider is missing"))?;
    specs.sort_by_key(|frame| frame.ordinal);
    let first = specs.first().ok_or_else(|| invalid("preview has no selected frames"))?;
    let last = specs.last().ok_or_else(|| invalid("preview has no selected frames"))?;
    let period = first.source_time.duration().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let mut filenames = BTreeSet::new();
    for (offset,frame) in specs.iter().enumerate() {
        let expected_start = period.checked_mul(frame.ordinal).map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
        if frame.ordinal != first.ordinal+offset as u64 || frame.source_sha256 != first.source_sha256
            || frame.source_byte_count != first.source_byte_count || frame.frame_index_sha256 != first.frame_index_sha256
            || (frame.input_width,frame.input_height) != (first.input_width,first.input_height)
            || !filenames.insert(frame.filename.clone()) || frame.source_time.start != expected_start
            || frame.source_time.duration().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))? != period
            || (offset > 0 && specs[offset-1].source_time.end != frame.source_time.start) {
            return Err(invalid("preview stages must preserve one contiguous original source mapping"));
        }
        let id = format!("source_frame_{:06}",frame.ordinal);
        if !plan.payload.inputs.iter().any(|input| input.id == id && input.kind == crate::PipelineInputKind::File
            && input.byte_count == frame.input_byte_count && input.content_sha256 == frame.input_sha256) {
            return Err(invalid("preview source PNG identity differs from its native operation"));
        }
    }
    if !plan.payload.inputs.iter().any(|input| input.id == "source_video" && input.kind == crate::PipelineInputKind::File
        && input.byte_count == first.source_byte_count && input.content_sha256 == first.source_sha256)
        || !plan.payload.inputs.iter().any(|input| input.id == "source_index" && input.kind == crate::PipelineInputKind::File
            && input.byte_count > 0 && input.byte_count <= crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES) {
        return Err(invalid("preview requires the exact source video and complete original frame-index file bindings"));
    }
    let selected = SelectedFrameRange { first_frame:first.ordinal+1,last_frame:last.ordinal+1,start_ordinal:first.ordinal,
        end_ordinal_exclusive:last.ordinal+1,frame_count:specs.len() as u64,first_filename:first.filename.clone(),last_filename:last.filename.clone(),
        source_time:crate::temporal::TimeRange { start:first.source_time.start,end:last.source_time.end } };
    bounded_selection(&selected,settings.execution)?;
    let (expected,expected_registry,_) = graph(&plan.payload.pipeline_name,&settings,&specs,&executable)?;
    if expected.configuration_sha256().map_err(|failure| Error::new(failure.category(),failure.message))? != plan.payload.configuration_sha256 {
        return Err(invalid("preview plan differs from its fixed native graph, input roles, outputs or acceptance gates"));
    }
    for stage in &plan.payload.stages {
        let actual = registry.registration(&stage.provider_lock.payload.registration_id).ok_or_else(|| invalid("preview registration is missing"))?;
        let expected = expected_registry.registration(&stage.id).ok_or_else(|| invalid("unexpected preview stage"))?;
        if actual.configuration() != expected.configuration() || actual.manifest() != expected.manifest()
            || actual.components() != expected.components() || actual.implementation_id() != expected.implementation_id()
            || actual.executable() != expected.executable() {
            return Err(invalid("preview requires the exact existing native provider registration"));
        }
    }
    Ok(())
}

pub fn plan(request: &PreviewRequest, cancellation: &CancellationToken) -> Result<PreviewPlan> {
    Ok(prepare(request,cancellation)?.view)
}

fn output_parent(request: &PreviewRequest, settings: &UpscaylRecoveryConfiguration, directory: Option<PathBuf>) -> Result<PathBuf> {
    let import = FrameImportRequest::new(&request.source_video,&request.frames_directory,&request.frame_index,settings.import.clone(),&request.provider_executable);
    let parent = frame_import::output_parent(&import,directory)?;
    for input in [&settings.upscayl.executable,&settings.model.param.path,&settings.model.weights.path] {
        if parent.starts_with(input) { return Err(invalid("preview output parent overlaps an input component")); }
    }
    Ok(parent)
}

fn outcome(view: PreviewPlan, runtime: crate::PipelineV3RunOutcome) -> PreviewOutcome {
    PreviewOutcome { schema:PREVIEW_OUTCOME_SCHEMA_V1.to_owned(),scope:view.scope,selected:view.selected,source_frame_count:view.source_frame_count,target_resolution:view.target_resolution,
        resources:view.resources,required_components:view.required_components,runtime }
}

pub fn run<F>(request: PreviewRequest, output_directory: Option<PathBuf>, cancellation: &CancellationToken, on_progress: F) -> Result<PreviewOutcome>
where F: FnMut(&UpscaylRecoveryProgress) {
    let prepared = prepare(&request,cancellation)?;
    let parent = output_parent(&request,&prepared.settings,output_directory)?;
    require_storage(&parent,prepared.required_free_bytes,prepared.settings.minimum_free_bytes)?;
    let execution = crate::PipelineV3RunRequest::new(prepared.view.pipeline.clone(),prepared.bindings,prepared.registry)
        .with_output_directory(parent).with_execution_limits(request.execution_limits);
    let runtime = crate::run_v3_with_progress_and_cancellation(execution,cancellation,progress_observer(prepared.frame_weights,on_progress))?;
    Ok(outcome(prepared.view,runtime))
}

pub fn resume<F>(request: PreviewRequest, run_directory: impl AsRef<Path>, cancellation: &CancellationToken, on_progress: F) -> Result<PreviewOutcome>
where F: FnMut(&UpscaylRecoveryProgress) {
    let prepared = prepare(&request,cancellation)?;
    let root = run_directory.as_ref().canonicalize().map_err(|_| invalid("cannot resolve the preview run directory"))?;
    let workspace = crate::PipelineV3Workspace::open_read_only(&root)?;
    let plan_path = workspace.plan();
    let observed = identity(&plan_path,MAXIMUM_PLAN_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(&plan_path).map_err(|_| invalid("cannot open retained preview plan"))?.take(MAXIMUM_PLAN_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read retained preview plan"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(invalid("retained preview plan changed while reading")); }
    let retained = crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if retained != prepared.view.pipeline { return Err(invalid("preview range, target, source, settings or exact component identity changed; start a new preview run")); }
    require_storage(&root,(prepared.frame_weights.len() as u64+1)*65_536,prepared.settings.minimum_free_bytes)?;
    let execution = crate::PipelineV3ResumeRequest::new(root,prepared.bindings,prepared.registry).with_execution_limits(request.execution_limits);
    let runtime = crate::resume_v3_with_progress_and_cancellation(execution,cancellation,progress_observer(prepared.frame_weights,on_progress))?;
    Ok(outcome(prepared.view,runtime))
}

#[cfg(test)]
mod tests {
    // Authored for deferred #64 execution. These exercise pure admission and
    // generic planning with inert bytes; they do not qualify native Upscayl.
    use super::*;
    use crate::temporal::{RationalTime,TimeRange};

    fn time(ticks: u64, rate: u64) -> RationalTime { RationalTime::new(ticks as i64,rate).unwrap() }

    fn selection(start: u64, count: u64, rate: u64) -> SelectedFrameRange {
        SelectedFrameRange { first_frame:start+1,last_frame:start+count,start_ordinal:start,end_ordinal_exclusive:start+count,
            frame_count:count,first_filename:format!("frame_{start:06}.png"),last_filename:format!("frame_{:06}.png",start+count-1),
            source_time:TimeRange::new(time(start,rate),time(start+count,rate)).unwrap() }
    }

    fn settings(execution: UpscaylExecutionPolicy) -> UpscaylRecoveryConfiguration {
        let mut settings: UpscaylRecoveryConfiguration = serde_json::from_value(json!({
            "schema":UPSCAYL_RECOVERY_CONFIGURATION_SCHEMA_V1,
            "import":{"schema":"aniflow.frame-import-configuration/v1","ffmpeg":{"executable":"/fixture/ffmpeg","version":"1.0.0","sha256":"a".repeat(64)}},
            "upscayl":{"executable":"/fixture/upscayl","version":"1.0.0","sha256":"b".repeat(64)},
            "model":{"name":"fixture_model","native_scale":4,"param":{"path":"/fixture/model.param","sha256":"c".repeat(64)},"weights":{"path":"/fixture/model.bin","sha256":"d".repeat(64)}},
            "output_scale":2,"tta":true,
        })).unwrap();
        settings.execution = execution;
        settings
    }

    fn specs(start: u64, count: u64) -> Vec<UpscaylFrameSpec> {
        (start..start+count).map(|ordinal| UpscaylFrameSpec {
            source_sha256:format!("{:x}",Sha256::digest(b"source")),source_byte_count:6,frame_index_sha256:"e".repeat(64),
            ordinal,filename:format!("frame_{ordinal:06}.png"),source_time:TimeRange::new(time(ordinal,24),time(ordinal+1,24)).unwrap(),
            input_sha256:format!("{:x}",Sha256::digest(b"frame")),input_byte_count:5,input_width:16,input_height:9,
        }).collect()
    }

    #[test]
    fn preview_bounds_use_exact_duration_and_original_chunk_grid() {
        assert!(bounded_selection(&selection(0,240,24),UpscaylExecutionPolicy::PerFrame).is_ok());
        assert!(bounded_selection(&selection(0,241,24),UpscaylExecutionPolicy::PerFrame).is_err());
        assert!(bounded_selection(&selection(0,257,30),UpscaylExecutionPolicy::PerFrame).is_err());
        assert!(bounded_selection(&selection(1024,1,24),UpscaylExecutionPolicy::PerFrame).is_err());
        let chunk = UpscaylExecutionPolicy::BoundedChunk { frames_per_chunk:16,timeout_milliseconds:120_000,maximum_tool_output_bytes:65_536 };
        assert!(bounded_selection(&selection(5120,17,24),chunk).is_ok());
        assert!(bounded_selection(&selection(5121,16,24),chunk).is_err());
    }

    #[test]
    fn excerpt_graph_preserves_original_settings_and_has_no_full_collection() {
        let settings = settings(UpscaylExecutionPolicy::BoundedChunk { frames_per_chunk:16,timeout_milliseconds:120_000,maximum_tool_output_bytes:65_536 });
        let frames = specs(5120,17);
        let (pipeline,registry,weights) = graph(&format!("{PLAN_PREFIX}{}","f".repeat(64)),&settings,&frames,Path::new("/fixture/provider")).unwrap();
        assert_eq!(weights.values().sum::<u64>(),17);
        assert_eq!(pipeline.stages.len(),2);
        assert!(pipeline.stages.iter().all(|stage| stage.capability.id == UPSCAYL_CHUNK_CAPABILITY_ID && stage.depends_on.is_empty()));
        assert!(pipeline.outputs.iter().all(|output| !matches!(output.id.as_str(),"upscaled_frames"|"upscaled_index"|"delivery")));
        let operation: UpscaylChunkOperation = serde_json::from_value(serde_json::to_value(&registry.registration("upscale_chunk_000320").unwrap().configuration().values).unwrap()).unwrap();
        assert_eq!(operation.settings,settings);
        assert_eq!(operation.frames,frames[..16]);
        assert_eq!(operation.frames[0].source_time.start,time(5120,24));
    }

    fn resolve_fixture(configuration: &crate::PipelineV3Configuration, registry: &crate::ProviderRegistry,
        frames: &[UpscaylFrameSpec], changed_source: bool) -> crate::PipelineV3Plan {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap();
        let source = directory.join("source");
        let index = directory.join("index");
        std::fs::write(&source,if changed_source { b"mutate" } else { b"source" }).unwrap();
        std::fs::write(&index,b"complete original index fixture").unwrap();
        let mut bindings = vec![crate::PipelineInputBinding::new("source_video",source),crate::PipelineInputBinding::new("source_index",index)];
        for frame in frames {
            let path = directory.join(&frame.filename);
            std::fs::write(&path,b"frame").unwrap();
            bindings.push(crate::PipelineInputBinding::new(format!("source_frame_{:06}",frame.ordinal),path));
        }
        let mut context = crate::audio_inspection::context();
        context.allowed_side_effects.push(crate::SideEffect::Gpu);
        crate::resolve_pipeline_v3(configuration,&bindings,registry,&context).unwrap()
    }

    #[test]
    fn generic_preview_preflight_refuses_full_output_alias_and_source_drift() {
        let settings = settings(UpscaylExecutionPolicy::PerFrame);
        let frames = specs(3,2);
        let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
        let (mut pipeline,registry,_) = graph(&format!("{PLAN_PREFIX}{}","f".repeat(64)),&settings,&frames,&executable).unwrap();
        assert!(preflight_plan(&resolve_fixture(&pipeline,&registry,&frames,false),&registry).is_ok());
        assert!(preflight_plan(&resolve_fixture(&pipeline,&registry,&frames,true),&registry).is_err());
        pipeline.outputs.push(crate::pipeline_v3::FinalOutputRequirement { id:"upscaled_frames".to_owned(),artifact:"frame_000003".to_owned(),required_validations:vec!["frame_000003_integrity".to_owned()] });
        assert!(preflight_plan(&resolve_fixture(&pipeline,&registry,&frames,false),&registry).is_err());
    }

    #[test]
    fn preview_preflight_does_not_accept_rebased_source_times() {
        let settings = settings(UpscaylExecutionPolicy::PerFrame);
        let mut frames = specs(3,2);
        for (offset,frame) in frames.iter_mut().enumerate() {
            frame.source_time = TimeRange::new(time(offset as u64,24),time(offset as u64+1,24)).unwrap();
        }
        let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
        let (pipeline,registry,_) = graph(&format!("{PLAN_PREFIX}{}","f".repeat(64)),&settings,&frames,&executable).unwrap();
        assert!(preflight_plan(&resolve_fixture(&pipeline,&registry,&frames,false),&registry).is_err());
    }
}
