use super::*;
use crate::frame_io::ensure_canonical_directory;

struct Prepared {
    view: Plan,
    operation: Option<Operation>,
    registry: Option<crate::ProviderRegistry>,
    bindings: Vec<crate::PipelineInputBinding>,
}

fn prepare(request: &Request, token: &CancellationToken) -> Result<Prepared> {
    cancelled(token)?;
    request.validate()?;
    let source = source(request, token)?;
    let decision = decide(&source.assessment.evaluation, &request.configuration)?;
    let mut view = Plan {
        schema: "aniflow.audio-gain-plan/v1".into(),
        request: request.clone(),
        source: source.clone(),
        decision: decision.clone(),
        native_plan: None,
    };
    if decision.state != DecisionState::Ready {
        return Ok(Prepared {
            view,
            operation: None,
            registry: None,
            bindings: Vec::new(),
        });
    }
    let operation = Operation {
        schema: SCHEMA.into(),
        request: request.clone(),
        source: source.clone(),
        components: components(request)?,
        gain_milli_db: decision
            .gain_milli_db
            .ok_or_else(|| invalid("ready gain is missing"))?,
    };
    operation.validate()?;
    let value = serde_json::json!({"schema":"aniflow.pipeline/v3","name":"selected-fixed-audio-gain","inputs":[{"id":"gain_source","artifact_type":"audio/wav","artifact_role":"temporal_source","stream_role":"audio"}],"stages":[{"id":"audio_gain","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"audio_gain"},"fallbacks":[]},"inputs":[{"port":"source","artifacts":["gain_source"]}],"outputs":[{"port":"media","artifacts":[{"id":"gain_audio","relative_path":"artifacts/gain/audio.wav","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"gain_evidence","relative_path":"artifacts/gain/private.json","kind":"file"}]}],"validations":[{"id":"gain_file_integrity","artifact":"gain_audio","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"gain_report_integrity","artifact":"gain_evidence","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],"outputs":[{"id":"gain_audio","artifact":"gain_audio","required_validations":["gain_file_integrity"]},{"id":"gain_evidence","artifact":"gain_evidence","required_validations":["gain_report_integrity"]}]});
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?)
        .map_err(|e| invalid(&e.message))?;
    let mut registry = crate::ProviderRegistry::new();
    registry.register(crate::ProviderRegistration::new(
        "audio_gain",
        operation.manifest()?,
        operation.configuration()?,
        request.provider_executable.clone(),
        "aniflow-fixed-audio-gain-v1",
        operation.inventory(),
    )?)?;
    let bindings = vec![crate::PipelineInputBinding::new(
        "gain_source",
        &source.pcm.locator,
    )];
    let native_plan = crate::resolve_pipeline_v3(
        &pipeline,
        &bindings,
        &registry,
        &crate::audio_inspection::context(),
    )
    .map_err(|e| invalid(&e.message))?;
    if !native_plan.payload.inputs.iter().any(|i| {
        i.id == "gain_source"
            && i.content_sha256 == source.pcm.sha256
            && i.byte_count == source.pcm.byte_count
    }) || source != super::source(request, token)?
        || components(request)? != operation.components
    {
        return Err(invalid(
            "gain source/owner/components changed during native planning",
        ));
    }
    view.native_plan = Some(native_plan);
    cancelled(token)?;
    Ok(Prepared {
        view,
        operation: Some(operation),
        registry: Some(registry),
        bindings,
    })
}

pub fn plan(request: &Request, token: &CancellationToken) -> Result<Plan> {
    Ok(prepare(request, token)?.view)
}
fn limits(configuration: &Configuration) -> crate::ProviderExecutionLimits {
    crate::ProviderExecutionLimits {
        timeout: std::time::Duration::from_millis(
            configuration
                .tools
                .tool_timeout_milliseconds
                .saturating_mul(6)
                .saturating_add(30_000),
        ),
        termination_grace_period: std::time::Duration::from_secs(2),
        maximum_stdout_bytes: 1_048_576,
        maximum_stderr_bytes: 1_048_576,
        maximum_artifact_files: 16,
        maximum_artifact_bytes: configuration.maximum_output_bytes.saturating_add(4_194_304),
    }
}
fn disjoint(path: &Path, operation: &Operation) -> Result<()> {
    let mut originals = vec![
        operation.source.pcm.locator.as_path(),
        operation.source.assessment.subject.file.locator.as_path(),
    ];
    match &operation.request.check.input {
        MediaInput::AcceptedVideoWithQc {
            run_directory,
            qc_directory,
            ..
        } => originals.extend([run_directory.as_path(), qc_directory.as_path()]),
        MediaInput::AcceptedAudioWithQc {
            subject,
            qc_directory,
            ..
        } => originals.extend([subject.run_directory(), qc_directory.as_path()]),
        _ => return Err(invalid("unsupported gain source")),
    }
    originals.extend(operation.components.iter().map(|c| c.path.as_path()));
    if originals
        .iter()
        .any(|source| path.starts_with(source) || source.starts_with(path))
    {
        return Err(invalid("gain output overlaps source/evidence/component"));
    }
    Ok(())
}
pub fn run<F: FnMut(&crate::PipelineV3RunProgress)>(
    request: &Request,
    parent: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(parent)?;
    let ready = prepare(request, token)?;
    let operation = ready.operation.as_ref().ok_or_else(|| {
        invalid("gain is not ready; inspect the explicit decision without creating a candidate")
    })?;
    disjoint(parent, operation)?;
    crate::video_assembly::require_storage(
        parent,
        operation.working_bytes()?,
        request.configuration.minimum_free_bytes,
    )?;
    let execution = crate::PipelineV3RunRequest::new(
        ready
            .view
            .native_plan
            .ok_or_else(|| invalid("gain native plan missing"))?,
        ready.bindings,
        ready
            .registry
            .ok_or_else(|| invalid("gain provider missing"))?,
    )
    .with_output_directory(parent)
    .with_execution_limits(limits(&request.configuration));
    let result = crate::run_v3_with_progress_and_cancellation(execution, token, progress)?;
    if source(request, token)? != operation.source || components(request)? != operation.components {
        return Err(invalid(
            "gain source/components changed during execution; retain run",
        ));
    }
    Ok(result)
}
fn retained(root: &Path) -> Result<crate::PipelineV3Plan> {
    ensure_canonical_directory(root)?;
    let workspace = crate::PipelineV3Workspace::open_read_only(root)?;
    let value = local::read::<serde_json::Value>(&workspace.plan())?;
    let plan = crate::PipelineV3Plan::from_json_slice(&canonical_json_bytes(&value)?)
        .map_err(|e| invalid(&e.message))?;
    if plan.payload.stages.len() != 1
        || plan.payload.stages[0].id != "audio_gain"
        || plan.payload.stages[0].provider_lock.payload.provider.id != PROVIDER_ID
        || plan.payload.stages[0].capability.id != CAPABILITY_ID
        || plan.payload.inputs.len() != 1
        || plan.payload.outputs.len() != 2
    {
        return Err(invalid("run is not the fixed audio gain profile"));
    }
    Ok(plan)
}
pub fn resume<F: FnMut(&crate::PipelineV3RunProgress)>(
    request: &Request,
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<crate::PipelineV3RunOutcome> {
    let ready = prepare(request, token)?;
    let operation = ready
        .operation
        .as_ref()
        .ok_or_else(|| invalid("gain no longer ready"))?;
    disjoint(root, operation)?;
    if ready.view.native_plan.as_ref() != Some(&retained(root)?) {
        return Err(invalid(
            "gain source/policy/settings/components changed; no stale resume",
        ));
    }
    crate::video_assembly::require_storage(
        root,
        operation.working_bytes()?,
        request.configuration.minimum_free_bytes,
    )?;
    let result = crate::resume_v3_with_progress_and_cancellation(
        crate::PipelineV3ResumeRequest::new(
            root,
            ready.bindings,
            ready
                .registry
                .ok_or_else(|| invalid("gain provider missing"))?,
        )
        .with_execution_limits(limits(&request.configuration)),
        token,
        progress,
    )?;
    if source(request, token)? != operation.source || components(request)? != operation.components {
        return Err(invalid("gain source/components changed during resume"));
    }
    Ok(result)
}

pub fn observe(root: &Path, token: &CancellationToken) -> Result<Completion> {
    cancelled(token)?;
    let plan = retained(root)?;
    let before = crate::status_v3(root)?;
    if before.payload.state != crate::PipelineV3RunState::Complete
        || before.payload.delivery.is_none()
        || before.payload.plan_sha256 != plan.plan_sha256
    {
        return Err(invalid(
            "gain observation requires complete current native v3 acceptance",
        ));
    }
    let select = |id: &str| -> Result<&crate::ArtifactEvidence> {
        let v = before
            .payload
            .outputs
            .iter()
            .filter(|a| a.id == id)
            .collect::<Vec<_>>();
        if v.len() != 1 {
            return Err(invalid("gain output missing/ambiguous"));
        }
        Ok(v[0])
    };
    let audio = select("gain_audio")?;
    let evidence = select("gain_evidence")?;
    let locate = |artifact: &crate::ArtifactEvidence, maximum: u64| -> Result<PathBuf> {
        let p = root.join(
            artifact
                .relative_path
                .as_deref()
                .ok_or_else(|| invalid("gain artifact locator missing"))?,
        );
        if !p.starts_with(root)
            || identity(&p, maximum)? != (artifact.sha256.clone(), artifact.byte_count)
        {
            return Err(invalid("gain artifact changed"));
        }
        Ok(p)
    };
    let path = locate(audio, MEDIA_BYTES)?;
    let report_path = locate(evidence, local::DOCUMENT_BYTES)?;
    let report: Report = local::read(&report_path)?;
    report.operation.validate()?;
    let operation = &report.operation;
    let stage = &plan.payload.stages[0];
    let wave = crate::audio_inspection::wav::inspect_native(&path)?;
    native::require_candidate(&path, operation)?;
    if report.schema != "aniflow.audio-gain-report/v1"
        || report.provider_lock_sha256 != stage.provider_lock.lock_sha256
        || operation.configuration()?.effective_configuration_sha256
            != stage.provider_lock.payload.effective_configuration_sha256
        || !plan.payload.inputs.iter().any(|i| {
            i.id == "gain_source"
                && i.content_sha256 == operation.source.pcm.sha256
                && i.byte_count == operation.source.pcm.byte_count
        })
        || report.output_sha256 != audio.sha256
        || report.output_bytes != audio.byte_count
        || report.output_pcm_sha256 != wave.pcm_sha256
        || report.sample_rate_hz != wave.sample_rate_hz
        || report.channels != wave.channels
        || report.frame_count != wave.frame_count
        || report.output_policy_status != "requires_fresh_exact_file_qc"
        || report.runtime_qualified
        || report.creative_approval != "not_performed"
        || report.publication_authority != "none"
        || source(&operation.request, token)? != operation.source
        || components(&operation.request)? != operation.components
    {
        return Err(invalid(
            "gain report no longer binds exact current source/policy/native samples/evidence",
        ));
    }
    if crate::status_v3(root)? != before
        || identity(&report_path, local::DOCUMENT_BYTES)?
            != (evidence.sha256.clone(), evidence.byte_count)
    {
        return Err(invalid("gain evidence changed during observation"));
    }
    cancelled(token)?;
    Ok(Completion {
        schema: "aniflow.audio-gain-completion/v1".into(),
        file: MediaFile {
            locator: path,
            sha256: audio.sha256.clone(),
            byte_count: audio.byte_count,
        },
        report,
    })
}
