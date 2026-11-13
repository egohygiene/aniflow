use super::*;
struct Ready {
    plan: Plan,
    registry: Option<crate::ProviderRegistry>,
    bindings: Vec<crate::PipelineInputBinding>,
}
fn capture(r: &Request, token: &CancellationToken) -> Result<Operation> {
    r.validate()?;
    cancelled(token)?;
    let source = crate::final_video::observe(&r.video_run_directory)?;
    let artifact = source
        .artifacts
        .iter()
        .find(|a| a.artifact_id == r.artifact_id)
        .cloned()
        .ok_or_else(|| invalid("deflicker source artifact not accepted"))?;
    let (renderer_sha256, working_bytes) = admission(r, &artifact)?;
    let op = Operation {
        schema: "aniflow.deflicker-comparison-operation/v1".into(),
        request: r.clone(),
        source,
        artifact,
        renderer_sha256,
        working_bytes,
    };
    op.validate()?;
    local::bytes(&op)?;
    Ok(op)
}
fn prepare(r: &Request, token: &CancellationToken) -> Result<Ready> {
    let op = capture(r, token)?;
    let mut registry = None;
    let mut bindings = Vec::new();
    let native_plan = if let Some(c) = &r.configuration {
        let pipeline = serde_json::json!({"schema":"aniflow.pipeline/v3","name":"explicit-deflicker-comparison","inputs":[{"id":"deflicker_source","artifact_type":"video/mp4","artifact_role":"temporal_source","stream_role":"video"}],"stages":[{"id":"compare_deflicker","depends_on":[],"capability":{"id":CAPABILITY,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"deflicker"},"fallbacks":[]},"inputs":[{"port":"source","artifacts":["deflicker_source"]}],"outputs":[{"port":"comparison","artifacts":[{"id":"comparison","relative_path":"artifacts/deflicker/comparison","kind":"directory"}]},{"port":"evidence","artifacts":[{"id":"comparison_report","relative_path":"artifacts/deflicker/report.json","kind":"file"}]}],"validations":[{"id":"comparison_integrity","artifact":"comparison","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1},{"id":"report_integrity","artifact":"comparison_report","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],"outputs":[{"id":"comparison","artifact":"comparison","required_validations":["comparison_integrity"]},{"id":"comparison_report","artifact":"comparison_report","required_validations":["report_integrity"]}]});
        let pipeline =
            crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&pipeline)?)
                .map_err(|e| invalid(&e.message))?;
        let mut reg = crate::ProviderRegistry::new();
        reg.register(crate::ProviderRegistration::new(
            "deflicker",
            op.manifest()?,
            op.configuration()?,
            r.provider_executable.clone().unwrap(),
            "aniflow-deflicker-comparison-v1",
            crate::video_assembly::component_inventory(&c.tools),
        )?)?;
        bindings.push(crate::PipelineInputBinding::new(
            "deflicker_source",
            &op.artifact.locator,
        ));
        let plan = crate::resolve_pipeline_v3(
            &pipeline,
            &bindings,
            &reg,
            &crate::audio_inspection::context(),
        )
        .map_err(|e| invalid(&e.message))?;
        if !plan.payload.inputs.iter().any(|i| {
            i.id == "deflicker_source"
                && i.content_sha256 == op.artifact.sha256
                && i.byte_count == op.artifact.byte_count
        }) {
            return Err(invalid("deflicker source changed during planning"));
        }
        registry = Some(reg);
        Some(plan)
    } else {
        None
    };
    Ok(Ready {
        plan: Plan {
            schema: "aniflow.deflicker-comparison-plan/v1".into(),
            context_sha256: op.context()?,
            bypassed: r.configuration.is_none(),
            operation: op,
            native_plan,
        },
        registry,
        bindings,
    })
}
pub fn plan(r: &Request, token: &CancellationToken) -> Result<Plan> {
    Ok(prepare(r, token)?.plan)
}
fn retained(root: &Path, token: &CancellationToken) -> Result<Plan> {
    let p: Plan = local::read(&root.join("comparison-intent.json"))?;
    if plan(&p.operation.request, token)? != p {
        return Err(invalid(
            "comparison source/native acceptance/settings/tools changed; retain root and use a new intent",
        ));
    }
    Ok(p)
}
fn child(root: &Path) -> PathBuf {
    root.join("children/compare")
}
fn present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() || m.is_dir() => Ok(true),
        Ok(_) => Err(invalid("symbolic comparison member")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(invalid("comparison member unavailable")),
    }
}
fn disjoint(root: &Path, op: &Operation) -> Result<()> {
    local::disjoint(
        root,
        &crate::destination::MediaInput::AcceptedVideo {
            run_directory: op.request.video_run_directory.clone(),
            artifact_id: op.request.artifact_id.clone(),
        },
        &op.artifact.locator,
    )?;
    if let Some(c) = &op.request.configuration {
        for input in [
            &c.tools.ffmpeg.executable,
            &c.tools.ffprobe.executable,
            op.request.provider_executable.as_ref().unwrap(),
        ] {
            if root.starts_with(input) || input.starts_with(root) {
                return Err(invalid("comparison output overlaps tools"));
            }
        }
    }
    Ok(())
}
pub fn status(root: &Path, token: &CancellationToken) -> Result<Status> {
    ensure_canonical_directory(root)?;
    let p = retained(root, token)?;
    disjoint(root, &p.operation)?;
    let review_present = present(&root.join("review.json"))?;
    let n = present(&child(root))?;
    local::entries(
        root,
        &if review_present {
            vec!["comparison-intent.json", "review.json"]
        } else {
            vec!["comparison-intent.json"]
        },
        &["children"],
        true,
    )?;
    local::entries(
        &root.join("children"),
        &[],
        &if n { vec!["compare"] } else { vec![] },
        false,
    )?;
    if p.bypassed && (n || review_present) {
        return Err(invalid(
            "bypassed comparison cannot contain native media or review",
        ));
    }
    let mut complete = p.bypassed;
    let mut native_manifest_sha256 = None;
    let mut report = None;
    let mut comparison_directory = None;
    if n {
        let expected = p
            .native_plan
            .as_ref()
            .ok_or_else(|| invalid("unselected comparison child"))?;
        crate::audio_inspection::require_same_plan(&child(root), expected)?;
        let accepted = crate::status_v3(&child(root))?;
        if accepted.payload.plan_sha256 != expected.plan_sha256 {
            return Err(invalid("comparison native plan/manifest differs"));
        }
        native_manifest_sha256 = Some(accepted.manifest_sha256.clone());
        if accepted.payload.state == crate::PipelineV3RunState::Complete
            && accepted.payload.delivery.is_some()
        {
            let e = accepted
                .payload
                .outputs
                .iter()
                .find(|a| a.id == "comparison_report")
                .ok_or_else(|| invalid("comparison report missing"))?;
            let path = child(root).join(
                e.relative_path
                    .as_deref()
                    .ok_or_else(|| invalid("comparison report locator missing"))?,
            );
            let r: Report = local::read(&path)?;
            if identity(&path, local::DOCUMENT_BYTES)? != (e.sha256.clone(), e.byte_count)
                || r.schema != "aniflow.deflicker-comparison-report/v1"
                || r.operation != p.operation
                || r.provider_lock_sha256 != expected.payload.stages[0].provider_lock.lock_sha256
                || r.comparison_sha256 != canonical_sha256(&r.frames)?
                || r.frames.len() as u64 != p.operation.request.frame_count
                || r.visual_approval != "not_performed"
                || r.runtime_qualified
                || r.audio_scope != "silent_visual_excerpt; original_audio_unchanged_unassessed"
            {
                return Err(invalid(
                    "comparison report differs from current selected native output/context",
                ));
            }
            let directory = child(root).join("artifacts/deflicker/comparison");
            native::inspect_frames(&directory, &p.operation, &r.frames)?;
            comparison_directory = Some(directory);
            report = Some(r);
            complete = true;
        }
    }
    let review = if review_present {
        let r: ManualReview = local::read(&root.join("review.json"))?;
        require_review(
            &p,
            report
                .as_ref()
                .ok_or_else(|| invalid("review precedes complete accepted comparison"))?,
            &r,
        )?;
        Some(r)
    } else {
        None
    };
    if retained(root, token)? != p {
        return Err(invalid("comparison source changed during status"));
    }
    Ok(Status {
        schema: "aniflow.deflicker-comparison-status/v1".into(),
        run_directory: root.to_path_buf(),
        context_sha256: p.context_sha256,
        bypassed: p.bypassed,
        complete,
        native_manifest_sha256,
        comparison_directory,
        report,
        review,
        publication_authority: "none; excerpt_review_does_not_approve_full_video".into(),
    })
}
pub(super) fn require_review(p: &Plan, r: &Report, d: &ManualReview) -> Result<()> {
    d.validate()?;
    if d.context_sha256 != p.context_sha256 || d.subject_sha256 != r.comparison_sha256 {
        return Err(invalid(
            "supplied comparison review differs from exact context/actual before-after frames",
        ));
    }
    Ok(())
}
pub fn record_review(
    root: &Path,
    declaration: &ManualReview,
    token: &CancellationToken,
) -> Result<Status> {
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let before = status(root, token)?;
    if before.review.is_some() || before.bypassed {
        return Err(invalid(
            "comparison review is immutable and unavailable for bypass",
        ));
    }
    let _source = crate::cache_v3::ExclusiveWriter::acquire(
        &retained(root, token)?.operation.request.video_run_directory,
        "writer.lock",
    )?;
    let _child = crate::cache_v3::ExclusiveWriter::acquire(&child(root), "writer.lock")?;
    let current = status(root, token)?;
    if current != before {
        return Err(invalid("comparison changed before review"));
    }
    let p = retained(root, token)?;
    require_review(
        &p,
        current
            .report
            .as_ref()
            .ok_or_else(|| invalid("review requires complete native comparison"))?,
        declaration,
    )?;
    cancelled(token)?;
    local::write(&root.join("review.json"), &local::bytes(declaration)?)?;
    status(root, token)
}
pub fn run<F: FnMut(&crate::PipelineV3RunProgress)>(
    r: &Request,
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<Status> {
    let p = plan(r, token)?;
    local::parent(root)?;
    disjoint(root, &p.operation)?;
    local::directory(root)?;
    local::directory(&root.join("children"))?;
    local::write(&root.join("comparison-intent.json"), &local::bytes(&p)?)?;
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
    let current = status(root, token)?;
    if current.complete {
        return Ok(current);
    }
    let p = retained(root, token)?;
    let _source = crate::cache_v3::ExclusiveWriter::acquire(
        &p.operation.request.video_run_directory,
        "writer.lock",
    )?;
    let ready = prepare(&p.operation.request, token)?;
    if ready.plan != p {
        return Err(invalid("comparison changed before native execution"));
    }
    let c = p.operation.request.configuration.as_ref().unwrap();
    crate::video_assembly::require_storage(
        root,
        p.operation.working_bytes,
        c.tools.minimum_free_bytes,
    )?;
    let limits = crate::ProviderExecutionLimits {
        timeout: std::time::Duration::from_millis(c.tools.timeout_milliseconds),
        termination_grace_period: std::time::Duration::from_secs(2),
        maximum_stdout_bytes: 1_048_576,
        maximum_stderr_bytes: 1_048_576,
        maximum_artifact_files: 512,
        maximum_artifact_bytes: c
            .tools
            .maximum_total_frame_bytes
            .checked_add(4_194_304)
            .ok_or_else(|| invalid("comparison artifact bound overflow"))?,
    };
    if present(&child(root))? {
        crate::resume_v3_with_progress_and_cancellation(
            crate::PipelineV3ResumeRequest::new(
                child(root),
                ready.bindings,
                ready.registry.unwrap(),
            )
            .with_execution_limits(limits),
            token,
            progress,
        )?;
    } else {
        crate::run_v3::run_v3_at_with_progress_and_cancellation(
            crate::PipelineV3RunRequest::new(
                p.native_plan.unwrap(),
                ready.bindings,
                ready.registry.unwrap(),
            )
            .with_output_directory(root.join("children"))
            .with_execution_limits(limits),
            &child(root),
            token,
            progress,
        )?;
    }
    status(root, token)
}
