//! Public preparation and current acceptance readback. No custom execution journal.
use super::*;
use crate::frame_io::ensure_canonical_directory;

struct Ready {
    operation: MovieOperation,
    preview: PreviewReport,
    plan: crate::PipelineV3Plan,
    registry: crate::ProviderRegistry,
    bindings: Vec<crate::PipelineInputBinding>,
}
fn locate(root: &Path, artifact: &crate::ArtifactEvidence) -> Result<PathBuf> {
    let relative = artifact.relative_path.as_deref().ok_or_else(|| invalid("preview output locator missing"))?;
    let rel = Path::new(relative);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(invalid("preview output locator must be a confined relative path"));
    }
    let path = root.join(rel);
    if path.canonicalize().ok().as_deref() != Some(path.as_path()) { return Err(invalid("preview output is not canonical and confined")); }
    Ok(path)
}
fn select<'a>(manifest: &'a crate::PipelineV3RunManifest, id: &str) -> Result<&'a crate::ArtifactEvidence> {
    let selected = manifest.payload.outputs.iter().filter(|a| a.id == id).collect::<Vec<_>>();
    if selected.len() != 1 { return Err(invalid("preview output is absent or ambiguous")); }
    Ok(selected[0])
}
fn capture(request: &MovieRequest, token: &CancellationToken) -> Result<(MovieOperation, PreviewReport)> {
    cancelled(token)?;
    request.validate()?;
    // Refuse oversized/incorrect selected report before the upstream parser.
    let before = super::super::status(&request.preview_run_directory)?;
    let report = select(&before, "preview_report")?;
    if report.sha256 != request.expected_preview_report_sha256 || report.byte_count == 0 || report.byte_count > MAXIMUM_PREVIEW_REPORT_BYTES {
        return Err(invalid("movie requires the exact known accepted preview report within 512 KiB"));
    }
    let frames = select(&before, "preview_frames")?;
    let complete = super::super::observe(&request.preview_run_directory, token)?;
    if complete.run_manifest_sha256 != before.manifest_sha256 || complete.report_sha256 != report.sha256
        || locate(&request.preview_run_directory, frames)? != complete.frames_directory {
        return Err(invalid("selected accepted PNG preview changed during movie preparation"));
    }
    let source = super::super::runtime::inputs(&complete.report.operation, token)?;
    for frame in &complete.report.frames {
        cancelled(token)?;
        if crate::frame_io::inspect_png(&complete.frames_directory.join(&frame.frame.filename), source.index.width, source.index.height,
            request.settings.maximum_frame_bytes)?.0 != frame.frame.sha256 {
            return Err(invalid("accepted PNG exceeds the movie frame admission or changed"));
        }
    }
    let tools = crate::video_assembly::observe_components(&request.settings)?;
    let renderer = identity(&request.provider_executable, crate::video_assembly::MAXIMUM_TOOL_BYTES)?;
    let mut operation = MovieOperation {
        schema: OPERATION.into(), request: request.clone(),
        source: MovieSource {
            preview_plan_sha256: complete.plan_sha256, preview_run_manifest_sha256: complete.run_manifest_sha256,
            report_path: locate(&request.preview_run_directory, report)?, report_sha256: complete.report_sha256,
            report_byte_count: report.byte_count, frames_directory: complete.frames_directory,
            frames_sha256: frames.sha256.clone(), frames_byte_count: frames.byte_count,
            source_index_path: complete.report.operation.frame_index.clone(), source_index_sha256: source.index_identity.0,
            source_index_byte_count: source.index_identity.1, width: source.index.width, height: source.index.height,
            frame_count: complete.report.frames.len() as u64, frame_rate: complete.report.full_source_plan.output_frame_rate,
            first_original_output_ordinal: complete.report.frames.first().ok_or_else(|| invalid("empty preview cannot become a movie"))?.frame.ordinal,
            original_window: complete.report.window,
        }, renderer_sha256: renderer.0, renderer_byte_count: renderer.1,
        tool_byte_counts: [tools[0].1, tools[1].1], working_bytes: 0,
    };
    operation.working_bytes = operation.required_working_bytes()?;
    operation.validate()?;
    if frames.file_count != operation.source.frame_count
        || super::super::status(&request.preview_run_directory)? != before {
        return Err(invalid("accepted preview membership or current native receipt changed"));
    }
    cancelled(token)?;
    Ok((operation, complete.report))
}
fn input_bindings(plan: &crate::PipelineV3Plan, op: &MovieOperation) -> Result<()> {
    let s = &op.source;
    for (id, sha, bytes, files) in [
        ("preview_frames", &s.frames_sha256, s.frames_byte_count, s.frame_count),
        ("source_preview", &s.report_sha256, s.report_byte_count, 1),
        ("source_index", &s.source_index_sha256, s.source_index_byte_count, 1),
    ] {
        if !plan.payload.inputs.iter().any(|i| i.id == id && i.content_sha256 == *sha && i.byte_count == bytes && i.file_count == files) {
            return Err(invalid("movie inputs differ from the exact accepted preview/index bytes"));
        }
    }
    Ok(())
}
fn prepare(request: &MovieRequest, token: &CancellationToken) -> Result<Ready> {
    let (operation, preview) = capture(request, token)?;
    let v = serde_json::json!({"schema":"aniflow.pipeline/v3","name":STAGE,
        "inputs":[{"id":"preview_frames","artifact_type":"application/vnd.aniflow.rife-preview-frames","artifact_role":"intermediate","stream_role":"video"},{"id":"source_preview","artifact_type":"application/vnd.aniflow.rife-preview-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"},{"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"}],
        "stages":[{"id":STAGE,"depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"rife-preview-movie"},"fallbacks":[]},
            "inputs":[{"port":"frames","artifacts":["preview_frames"]},{"port":"preview","artifacts":["source_preview"]},{"port":"index","artifacts":["source_index"]}],
            "outputs":[{"port":"media","artifacts":[{"id":"preview_movie","relative_path":"artifacts/rife-preview-movie/preview.mp4","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"movie_report","relative_path":"artifacts/rife-preview-movie/report.json","kind":"file"}]}],
            "validations":[{"id":"movie_integrity","artifact":"preview_movie","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"movie_report_integrity","artifact":"movie_report","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],
        "outputs":[{"id":"preview_movie","artifact":"preview_movie","required_validations":["movie_integrity"]},{"id":"movie_report","artifact":"movie_report","required_validations":["movie_report_integrity"]}]});
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&v)?).map_err(|e| invalid(e.message))?;
    let mut registry = crate::ProviderRegistry::new();
    registry.register(crate::ProviderRegistration::new("rife-preview-movie", operation.manifest()?, operation.configuration()?,
        request.provider_executable.clone(), "aniflow-rife-preview-movie-v1", crate::video_assembly::component_inventory(&request.settings))?)?;
    let s = &operation.source;
    let bindings = vec![crate::PipelineInputBinding::new("preview_frames", &s.frames_directory),
        crate::PipelineInputBinding::new("source_preview", &s.report_path), crate::PipelineInputBinding::new("source_index", &s.source_index_path)];
    let plan = crate::resolve_pipeline_v3(&pipeline, &bindings, &registry, &crate::audio_inspection::context()).map_err(|e| invalid(e.message))?;
    input_bindings(&plan, &operation)?;
    if capture(request, token)?.0 != operation { return Err(invalid("preview source/tool binding changed during movie plan")); }
    Ok(Ready { operation, preview, plan, registry, bindings })
}
pub fn plan(request: &MovieRequest, token: &CancellationToken) -> Result<crate::PipelineV3Plan> { Ok(prepare(request, token)?.plan) }
fn retained(root: &Path) -> Result<crate::PipelineV3Plan> {
    ensure_canonical_directory(root)?;
    let workspace = crate::PipelineV3Workspace::open_read_only(root)?;
    let v: serde_json::Value = local::read(&workspace.plan())?;
    let plan = crate::PipelineV3Plan::from_json_slice(&canonical_json_bytes(&v)?).map_err(|e| invalid(e.message))?;
    if plan.payload.pipeline_name != STAGE || plan.payload.stages.len() != 1 || plan.payload.inputs.len() != 3
        || plan.payload.outputs.len() != 2 || plan.payload.stages[0].id != STAGE
        || plan.payload.stages[0].capability.id != CAPABILITY_ID || plan.payload.stages[0].provider_lock.payload.provider.id != PROVIDER_ID {
        return Err(invalid("run is not the fixed RIFE preview movie profile"));
    }
    Ok(plan)
}
fn execution_limits(op: &MovieOperation) -> crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits { timeout: std::time::Duration::from_secs(300), termination_grace_period: std::time::Duration::from_secs(2),
        maximum_stdout_bytes: 1_048_576, maximum_stderr_bytes: 1_048_576, maximum_artifact_files: 2,
        maximum_artifact_bytes: op.request.settings.maximum_output_bytes + local::DOCUMENT_BYTES }
}
fn run_ready<F: FnMut(&crate::PipelineV3RunProgress)>(ready: Ready, root: &Path, resume: bool, token: &CancellationToken, progress: F) -> Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(root)?;
    let op = &ready.operation;
    if resume {
        if retained(root)? != ready.plan { return Err(invalid("movie resume preview/source/clock/settings/tool/implementation changed; preserve existing work")); }
    }
    let original = &ready.preview.operation;
    for source in [&op.request.preview_run_directory, &op.source.frames_directory, &op.source.report_path, &op.source.source_index_path,
        &op.request.provider_executable, &op.request.settings.ffmpeg.executable, &op.request.settings.ffprobe.executable,
        &original.frames_directory, &original.configuration.settings.tool.executable, &original.configuration.settings.model.directory] {
        if root.starts_with(source) || source.starts_with(root) { return Err(invalid("movie workspace overlaps upstream evidence, original frames or tools/models")); }
    }
    crate::video_assembly::require_storage(root, op.working_bytes, op.request.settings.minimum_free_bytes)?;
    let limits = execution_limits(op);
    let result = if resume {
        crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root, ready.bindings, ready.registry).with_execution_limits(limits), token, progress)?
    } else {
        let mut request = crate::PipelineV3RunRequest::new(ready.plan, ready.bindings, ready.registry).with_execution_limits(limits);
        request.output_directory = Some(root.to_path_buf());
        crate::run_v3_with_progress_and_cancellation(request, token, progress)?
    };
    if capture(&ready.operation.request, token)?.0 != ready.operation { return Err(invalid("movie upstream source or components changed during execution; retain native workspace")); }
    Ok(result)
}
pub fn run<F: FnMut(&crate::PipelineV3RunProgress)>(request: MovieRequest, parent: &Path, token: &CancellationToken, progress: F) -> Result<crate::PipelineV3RunOutcome> {
    run_ready(prepare(&request, token)?, parent, false, token, progress)
}
pub fn resume<F: FnMut(&crate::PipelineV3RunProgress)>(request: MovieRequest, root: &Path, token: &CancellationToken, progress: F) -> Result<crate::PipelineV3RunOutcome> {
    run_ready(prepare(&request, token)?, root, true, token, progress)
}
/// Current ordinary v3 state, including incomplete runs. Does not repair or launch.
pub fn status(root: &Path) -> Result<crate::PipelineV3RunManifest> {
    let plan = retained(root)?;
    let manifest = crate::status_v3(root)?;
    if manifest.payload.plan_sha256 != plan.plan_sha256 || retained(root)? != plan { return Err(invalid("movie native status differs from retained plan")); }
    Ok(manifest)
}
/// Complete movie readback: exact ordinary acceptance plus current upstream source.
pub fn observe(root: &Path, token: &CancellationToken) -> Result<MovieCompletion> {
    cancelled(token)?;
    let plan = retained(root)?;
    let before = status(root)?;
    if before.payload.state != crate::PipelineV3RunState::Complete || before.payload.delivery.is_none() {
        return Err(invalid("movie completion requires current complete ordinary v3 acceptance"));
    }
    let media = select(&before, "preview_movie")?;
    let evidence = select(&before, "movie_report")?;
    let movie_path = locate(root, media)?;
    let report_path = locate(root, evidence)?;
    if identity(&report_path, local::DOCUMENT_BYTES)? != (evidence.sha256.clone(), evidence.byte_count) { return Err(invalid("accepted movie report bytes changed")); }
    let report: MovieReport = local::read(&report_path)?;
    let ready = prepare(&report.operation.request, token)?;
    if ready.plan != plan || report.operation != ready.operation || report.provider_lock_sha256 != plan.payload.stages[0].provider_lock.lock_sha256 {
        return Err(invalid("movie report no longer matches its complete current native source/configuration/provider plan"));
    }
    report.validate(&ready.preview)?;
    if media.sha256 != report.media.sha256 || media.byte_count != report.media.byte_count
        || identity(&movie_path, report.operation.request.settings.maximum_output_bytes)? != (media.sha256.clone(), media.byte_count) {
        return Err(invalid("movie differs from its exact accepted native output bytes"));
    }
    let reference = before.payload.stages.iter().find(|s| s.stage_id == STAGE).and_then(|s| s.checkpoint.as_ref()).ok_or_else(|| invalid("movie checkpoint missing"))?;
    let checkpoint: crate::StageCheckpoint = local::read(&root.join(&reference.relative_path))?;
    checkpoint.validate()?;
    if checkpoint.checkpoint_sha256 != reference.checkpoint_sha256 || checkpoint.payload.plan_sha256 != plan.plan_sha256
        || checkpoint.payload.provider_lock_sha256 != report.provider_lock_sha256
        || ![media, evidence].iter().all(|a| checkpoint.payload.validations.iter().any(|v| v.accepted && v.artifact_id == a.id && v.artifact_sha256 == a.sha256 && v.contract == crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1)) {
        return Err(invalid("movie lacks exact current native stage acceptance"));
    }
    let delivery = before.payload.delivery.as_ref().ok_or_else(|| invalid("movie delivery acceptance missing"))?;
    let acceptance: crate::validation::AcceptanceRecord = local::read(&root.join(&delivery.relative_path))?;
    acceptance.validate()?;
    if acceptance.sha256()? != delivery.sha256 || acceptance.plan_sha256 != plan.plan_sha256 || acceptance.layer != crate::validation::ValidationLayer::Delivery {
        return Err(invalid("movie native delivery acceptance changed"));
    }
    if capture(&report.operation.request, token)?.0 != report.operation || status(root)? != before
        || identity(&report_path, local::DOCUMENT_BYTES)?.0 != evidence.sha256
        || identity(&movie_path, report.operation.request.settings.maximum_output_bytes)? != (media.sha256.clone(), media.byte_count) {
        return Err(invalid("movie or original preview changed during current observation"));
    }
    Ok(MovieCompletion { schema: "aniflow.rife-preview-movie-completion/v1".into(), run_directory: root.to_path_buf(),
        plan_sha256: plan.plan_sha256, run_manifest_sha256: before.manifest_sha256.clone(), report_sha256: evidence.sha256.clone(), movie_path, report })
}
