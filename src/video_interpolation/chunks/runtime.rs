use super::*;
struct Ready {
    plan: PipelineV3Plan,
    registry: crate::ProviderRegistry,
    bindings: Vec<crate::PipelineInputBinding>,
    input: RecoveredInput,
}
fn prepare(r: &Request, token: &CancellationToken) -> Result<Ready> {
    r.validate()?;
    crate::destination::cancelled(token)?;
    let input = RecoveredInput::observe(&r.base.source, token)?;
    let full = recovered::plan(&r.base.configuration, &input.carrier)?;
    let parts = ranges(full.output_frame_count, r.output_frames_per_chunk)?;
    let components = super::super::runtime::observe_components(&r.base.configuration, token)?;
    let renderer = identity(&r.base.provider_executable, 134_217_728)?;
    let max = super::super::provider::frame_byte_bound_dimensions(
        input.carrier.selection.request.output_width,
        input.carrier.selection.request.output_height,
        &r.base.configuration.settings,
    )?;
    let working = max
        .checked_mul(full.output_frame_count)
        .and_then(|v| v.checked_mul(3))
        .and_then(|v| v.checked_add(input.carrier.selection.total_bytes))
        .and_then(|v| {
            v.checked_add(
                components
                    .iter()
                    .map(|c| c.byte_count)
                    .sum::<u64>()
                    .checked_mul(3)?,
            )
        })
        .and_then(|v| v.checked_add(renderer.1))
        .and_then(|v| v.checked_add(RIFE_MAXIMUM_INDEX_BYTES.checked_mul(parts.len() as u64 + 2)?))
        .and_then(|v| v.checked_add(16_777_216))
        .ok_or_else(|| invalid("chunked RIFE full output/collection admission overflow"))?;
    if working > r.base.configuration.settings.maximum_artifact_bytes {
        return Err(invalid(
            "chunked RIFE full retained outputs plus collection exceed selected artifact bound",
        ));
    }
    let mut registry = crate::ProviderRegistry::new();
    let mut stages = Vec::new();
    let mut frame_ids = Vec::new();
    let mut chunk_ids = Vec::new();
    let mut dependencies = Vec::new();
    let output = |port: &str, id: &str, path: String, kind: &str| serde_json::json!({"port":port,"artifacts":[{"id":id,"relative_path":path,"kind":kind}]});
    let validation = |id: &str, artifact: &str| serde_json::json!({"id":id,"artifact":artifact,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1});
    for (n, (first, end)) in parts.iter().copied().enumerate() {
        let stage = id(n);
        let frames = format!("chunk_{n:04}_frames");
        let index = format!("chunk_{n:04}_index");
        let op = Operation {
            schema: "aniflow.rife-chunk-operation/v1".into(),
            request: r.clone(),
            role: Role::Chunk {
                first_output: first,
                end_output: end,
            },
        };
        registry.register(crate::ProviderRegistration::new(
            &stage,
            op.manifest()?,
            op.configuration()?,
            r.base.provider_executable.clone(),
            "aniflow-rife-chunks-v1",
            super::super::runtime::component_inventory(&r.base.configuration),
        )?)?;
        stages.push(serde_json::json!({"id":stage,"depends_on":[],"capability":{"id":CHUNK_CAP,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":stage},"fallbacks":[]},"inputs":[{"port":"frames","artifacts":["source_frames"]},{"port":"index","artifacts":["source_index"]}],"outputs":[output("frames",&frames,format!("artifacts/rife/chunks/{n:04}/frames"),"directory"),output("index",&index,format!("artifacts/rife/chunks/{n:04}/index.json"),"file")],"validations":[validation(&format!("chunk_{n:04}_frames_valid"),&frames),validation(&format!("chunk_{n:04}_index_valid"),&index)]}));
        dependencies.push(stage);
        frame_ids.push(frames);
        chunk_ids.push(index);
    }
    let op = Operation {
        schema: "aniflow.rife-chunk-operation/v1".into(),
        request: r.clone(),
        role: Role::Collect,
    };
    registry.register(crate::ProviderRegistration::new(
        "collect_rife_chunks",
        op.manifest()?,
        op.configuration()?,
        r.base.provider_executable.clone(),
        "aniflow-rife-chunks-v1",
        super::super::runtime::component_inventory(&r.base.configuration),
    )?)?;
    stages.push(serde_json::json!({"id":"collect_rife_chunks","depends_on":dependencies,"capability":{"id":COLLECT_CAP,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"collect_rife_chunks"},"fallbacks":[]},"inputs":[{"port":"index","artifacts":["source_index"]},{"port":"frames","artifacts":frame_ids},{"port":"chunks","artifacts":chunk_ids}],"outputs":[output("frames","interpolated_frames","artifacts/rife/frames".into(),"directory"),output("index","interpolated_index","artifacts/rife/index.json".into(),"file")],"validations":[validation("rife_frames_integrity","interpolated_frames"),validation("rife_index_integrity","interpolated_index")]}));
    let value = serde_json::json!({"schema":"aniflow.pipeline/v3","name":PIPELINE,"inputs":[{"id":"source_frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video"},{"id":"source_index","artifact_type":recovered::INPUT_TYPE,"artifact_role":"validation_evidence","stream_role":"timed_metadata"}],"stages":stages,"outputs":[{"id":"interpolated_frames","artifact":"interpolated_frames","required_validations":["rife_frames_integrity"]},{"id":"interpolated_index","artifact":"interpolated_index","required_validations":["rife_index_integrity"]}]});
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?)
        .map_err(|e| invalid(e.message))?;
    let index = r.base.source.carrier_directory.join("carrier.json");
    let bindings = vec![
        crate::PipelineInputBinding::new("source_frames", &input.carrier.frames_directory),
        crate::PipelineInputBinding::new("source_index", &index),
    ];
    let mut context = crate::audio_inspection::context();
    context.host.memory_mib = r.base.configuration.host.available_memory_mib;
    context.host.gpu_available = r.base.configuration.settings.device_id >= 0;
    context.allowed_side_effects.push(crate::SideEffect::Ai);
    if context.host.gpu_available {
        context.allowed_side_effects.push(crate::SideEffect::Gpu);
    }
    let plan = crate::resolve_pipeline_v3(&pipeline, &bindings, &registry, &context)
        .map_err(|e| invalid(e.message))?;
    if !plan
        .payload
        .inputs
        .iter()
        .any(|i| i.id == "source_index" && i.content_sha256 == r.base.source.carrier_sha256)
        || RecoveredInput::observe(&r.base.source, token)? != input
    {
        return Err(invalid(
            "chunked RIFE carrier changed during native planning",
        ));
    }
    Ok(Ready {
        plan,
        registry,
        bindings,
        input,
    })
}
fn digest(i: &Intent) -> Result<String> {
    canonical_sha256(&(&i.schema, &i.request, &i.native_plan))
}
pub fn plan(r: &Request, token: &CancellationToken) -> Result<Intent> {
    let p = prepare(r, token)?;
    let mut i = Intent {
        schema: "aniflow.chunked-recovered-rife-intent/v1".into(),
        request: r.clone(),
        native_plan: p.plan,
        intent_sha256: String::new(),
    };
    i.intent_sha256 = digest(&i)?;
    local::bytes(&i)?;
    Ok(i)
}
fn child(root: &Path) -> PathBuf {
    root.join("children/interpolate")
}
fn present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() || m.is_dir() => Ok(true),
        Ok(_) => Err(invalid("chunked RIFE symbolic/special member")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(invalid("chunked RIFE member unavailable")),
    }
}
fn retained(root: &Path, token: &CancellationToken) -> Result<Intent> {
    ensure_canonical_directory(root)?;
    let i: Intent = local::read(&root.join("chunked-rife-intent.json"))?;
    if i.schema != "aniflow.chunked-recovered-rife-intent/v1"
        || i.intent_sha256 != digest(&i)?
        || plan(&i.request, token)? != i
    {
        return Err(invalid(
            "chunked RIFE current source/tools/models/provider/settings differ; retain prior native work",
        ));
    }
    local::entries(root, &["chunked-rife-intent.json"], &["children"], true)?;
    local::entries(
        &root.join("children"),
        &[],
        &if present(&child(root))? {
            vec!["interpolate"]
        } else {
            vec![]
        },
        false,
    )?;
    Ok(i)
}
pub fn status(root: &Path, token: &CancellationToken) -> Result<Status> {
    let i = retained(root, token)?;
    let mut completed = 0;
    let mut failed = 0;
    let count = i.native_plan.payload.stages.len() as u64 - 1;
    let native = if present(&child(root))? {
        require_native_plan(&child(root), &i.native_plan)?;
        let m = crate::status_v3(&child(root))?;
        if m.payload.plan_sha256 != i.native_plan.plan_sha256 {
            return Err(invalid("chunked RIFE native plan/manifest differs"));
        }
        for s in &m.payload.stages {
            if s.stage_id.starts_with("rife_chunk_") {
                if s.state == crate::PipelineV3StageState::Complete && !s.rerun_required {
                    completed += 1;
                }
                if matches!(
                    s.state,
                    crate::PipelineV3StageState::Failed | crate::PipelineV3StageState::Cancelled
                ) {
                    failed += 1;
                }
            }
        }
        Some(m)
    } else {
        None
    };
    let complete = native.as_ref().is_some_and(|m| {
        m.payload.state == crate::PipelineV3RunState::Complete && m.payload.delivery.is_some()
    });
    if retained(root, token)? != i {
        return Err(invalid("chunked RIFE changed during status"));
    }
    Ok(Status {
        schema: "aniflow.chunked-recovered-rife-status/v1".into(),
        run_directory: root.to_path_buf(),
        native_run_directory: child(root),
        intent_sha256: i.intent_sha256,
        completed_chunks: completed,
        pending_chunks: count.saturating_sub(completed),
        failed_chunks: failed,
        complete,
        native,
    })
}
fn require_native_plan(root: &Path, p: &PipelineV3Plan) -> Result<()> {
    let w = crate::PipelineV3Workspace::open_read_only(root)?;
    let value: serde_json::Value = local::read_bounded(&w.plan(), 67_108_864)?;
    let saved = PipelineV3Plan::from_json_slice(&canonical_json_bytes(&value)?)
        .map_err(|e| invalid(e.message))?;
    if saved != *p {
        return Err(invalid(
            "chunked RIFE native child differs from frozen complete graph",
        ));
    }
    Ok(())
}
fn disjoint(root: &Path, r: &Request, input: &RecoveredInput) -> Result<()> {
    for p in [
        &r.base.source.carrier_directory,
        &input.carrier.frames_directory,
        &r.base.provider_executable,
        &r.base.configuration.settings.tool.executable,
        &r.base.configuration.settings.model.directory,
    ] {
        if root.starts_with(p) || p.starts_with(root) {
            return Err(invalid("chunked RIFE output overlaps source/components"));
        }
    }
    for origin in &input.carrier.selection.origins {
        if root.starts_with(&origin.run_directory) || origin.run_directory.starts_with(root) {
            return Err(invalid("chunked RIFE root overlaps accepted origin"));
        }
    }
    Ok(())
}
pub fn run<F: FnMut(&crate::PipelineV3RunProgress)>(
    r: &Request,
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<Status> {
    let p = plan(r, token)?;
    local::parent(root)?;
    let ready = prepare(r, token)?;
    disjoint(root, r, &ready.input)?;
    local::directory(root)?;
    local::directory(&root.join("children"))?;
    local::write(&root.join("chunked-rife-intent.json"), &local::bytes(&p)?)?;
    drive(root, token, progress)
}
pub fn resume<F: FnMut(&crate::PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<Status> {
    retained(root, token)?;
    drive(root, token, progress)
}
fn drive<F: FnMut(&crate::PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<Status> {
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let i = retained(root, token)?;
    let current = status(root, token)?;
    if current.complete {
        return Ok(current);
    }
    let ready = prepare(&i.request, token)?;
    disjoint(root, &i.request, &ready.input)?;
    let limits = super::super::runtime::RifeRunRequest::new(
        &ready.input.carrier.frames_directory,
        i.request.base.source.carrier_directory.join("carrier.json"),
        i.request.base.configuration.clone(),
        &i.request.base.provider_executable,
    )
    .execution_limits;
    if present(&child(root))? {
        crate::resume_v3_with_progress_and_cancellation(
            crate::PipelineV3ResumeRequest::new(child(root), ready.bindings, ready.registry)
                .with_execution_limits(limits),
            token,
            progress,
        )?;
    } else {
        crate::run_v3::run_v3_at_with_progress_and_cancellation(
            crate::PipelineV3RunRequest::new(ready.plan, ready.bindings, ready.registry)
                .with_output_directory(root.join("children"))
                .with_execution_limits(limits),
            &child(root),
            token,
            progress,
        )?;
    }
    status(root, token)
}
/// Generic v3 entry points cannot use a collector-only or foreign-producer graph.
pub(crate) fn preflight_plan(
    p: &PipelineV3Plan,
    registry: &crate::ProviderRegistry,
    token: &CancellationToken,
) -> Result<()> {
    if !p
        .payload
        .stages
        .iter()
        .any(|s| s.provider_lock.payload.provider.id == PROVIDER)
    {
        return Ok(());
    }
    let collectors = p
        .payload
        .stages
        .iter()
        .filter(|s| s.capability.id == COLLECT_CAP)
        .collect::<Vec<_>>();
    if collectors.len() != 1 {
        return Err(invalid("RIFE chunks require one complete-set collector"));
    }
    let stage = collectors[0];
    let registration = registry
        .registration(&stage.provider_lock.payload.registration_id)
        .ok_or_else(|| invalid("chunk collector registry missing"))?;
    let op: Operation = serde_json::from_value(
        serde_json::to_value(&registration.configuration().values)
            .map_err(|_| invalid("chunk registry encoding"))?,
    )
    .map_err(|_| invalid("closed chunk operation required"))?;
    if op.role != Role::Collect
        || op.configuration()? != *registration.configuration()
        || prepare(&op.request, token)?.plan != *p
    {
        return Err(invalid(
            "chunk collector requires every exact native range producer and current complete source/components; loose partial reports are not accepted",
        ));
    }
    Ok(())
}
/// Current actual complete native graph, used by the existing full assembly carrier.
pub(crate) fn validate_accepted(
    native_root: &Path,
    configuration: &RifeConfiguration,
    binding: &recovered::RecoveredBinding,
    output: &RifeOutputIndex,
) -> Result<PathBuf> {
    if native_root.file_name().and_then(|s| s.to_str()) != Some("interpolate")
        || native_root
            .parent()
            .and_then(|s| s.file_name())
            .and_then(|s| s.to_str())
            != Some("children")
    {
        return Err(invalid("chunked native run escaped its fixed parent"));
    }
    let root = native_root
        .parent()
        .and_then(|s| s.parent())
        .ok_or_else(|| invalid("chunked RIFE parent missing"))?;
    let token = CancellationToken::default();
    let i = retained(root, &token)?;
    if i.request.base.configuration != *configuration || i.request.base.source != *binding {
        return Err(invalid(
            "chunked RIFE binding belongs to different source/settings",
        ));
    }
    let current = status(root, &token)?;
    if !current.complete {
        return Err(invalid("chunked RIFE complete native acceptance required"));
    }
    let m = current.native.unwrap();
    let indices = m
        .payload
        .outputs
        .iter()
        .filter(|a| a.artifact_type == recovered::OUTPUT_TYPE)
        .collect::<Vec<_>>();
    let frames = m
        .payload
        .outputs
        .iter()
        .filter(|a| a.artifact_type == "application/vnd.aniflow.rife-frames")
        .collect::<Vec<_>>();
    if indices.len() != 1 || frames.len() != 1 {
        return Err(invalid("chunked RIFE final outputs absent/ambiguous"));
    }
    let path = native_root.join(
        indices[0]
            .relative_path
            .as_deref()
            .ok_or_else(|| invalid("chunked index locator missing"))?,
    );
    let actual: RifeOutputIndex = local::read_bounded(&path, RIFE_MAXIMUM_INDEX_BYTES)?;
    if actual != *output
        || identity(&path, RIFE_MAXIMUM_INDEX_BYTES)?
            != (indices[0].sha256.clone(), indices[0].byte_count)
    {
        return Err(invalid("chunked RIFE final index changed"));
    }
    let input = RecoveredInput::observe(binding, &token)?;
    recovered::validate_output(output, configuration, &input.carrier)?;
    let dir = native_root.join(
        frames[0]
            .relative_path
            .as_deref()
            .ok_or_else(|| invalid("chunked frames locator missing"))?,
    );
    native::inspect_complete(&dir, &input, configuration, output)?;
    if crate::status_v3(native_root)? != m || retained(root, &token)? != i {
        return Err(invalid(
            "chunked RIFE native/source binding changed during assembly readback",
        ));
    }
    Ok(dir)
}
pub fn bind_assembly(
    root: &Path,
    destination: &Path,
    token: &CancellationToken,
) -> Result<crate::video_assembly::rife::RifeAssemblyIndex> {
    let i = retained(root, token)?;
    let current = status(root, token)?;
    if !current.complete {
        return Err(invalid(
            "bind assembly requires complete accepted RIFE chunks",
        ));
    }
    crate::video_assembly::rife::bind_recovered_run(
        &child(root),
        i.request.base.configuration,
        &i.request.base.source,
        destination,
    )
}
