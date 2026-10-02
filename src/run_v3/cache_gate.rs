//! Bridge retained cache proof into ordinary Pipeline v3 acceptance.
use super::*;
use crate::cache_v3::{
    CacheDecision, CacheDecisionKind, CacheEntryPayload, CacheFailureCode, CachePolicy,
    CacheSession, CACHE_DECISION_SCHEMA_V1, CACHE_KEY_SEMANTICS_V1, cache_error,
};

pub(super) fn cache_key(plan: &PipelineV3Plan, stage: &ResolvedPipelineStage, inputs: &[ArtifactEvidence], checkpoints: &BTreeMap<String, StageCheckpoint>, limits: ProviderExecutionLimits) -> Result<String> {
    canonical_sha256(&serde_json::json!({
        "schema": CACHE_KEY_SEMANTICS_V1,
        "stage_invocation_sha256": stage_invocation_sha256(stage, inputs, checkpoints, limits.into())?,
        "planning_policy": plan.payload.policy,
    }))
}

pub(super) fn decision(stage_id: &str, key: Option<String>, kind: CacheDecisionKind, entry: Option<String>) -> CacheDecision {
    CacheDecision { schema: CACHE_DECISION_SCHEMA_V1.to_owned(), stage_id: stage_id.to_owned(), key_sha256: key, decision: kind, entry_sha256: entry }
}

pub(super) fn retain_decision(workspace: &PipelineV3Workspace, decisions: &mut Vec<CacheDecision>, value: CacheDecision) -> Result<()> {
    let digest = canonical_sha256(&value)?;
    publish_content_addressed_json(workspace, &workspace.providers().join(format!("cache-decision-{digest}.json")), &value, &digest)?;
    decisions.push(value); Ok(())
}

pub(super) fn rerun_frontier(plan: &PipelineV3Plan, selected: &[String]) -> Result<BTreeSet<String>> {
    let mut frontier = BTreeSet::new();
    for id in selected {
        if !plan.payload.stages.iter().any(|stage| &stage.id == id) {
            return Err(cache_error(CacheFailureCode::InvalidPolicy, format!("unknown rerun stage {id}")));
        }
        if !frontier.insert(id.clone()) { return Err(cache_error(CacheFailureCode::InvalidPolicy, "duplicate rerun stage")); }
    }
    loop {
        let previous = frontier.len();
        for stage in &plan.payload.stages {
            if stage.depends_on.iter().any(|id| frontier.contains(id)) { frontier.insert(stage.id.clone()); }
        }
        if previous == frontier.len() { return Ok(frontier); }
    }
}

/// Resolve all paths read-only before creating a cache or touching a workspace.
pub(super) fn preflight_cache_policy(policy: &CachePolicy, inputs: &BTreeMap<String, BoundInput>, workspace: &Path) -> Result<()> {
    policy.validate()?;
    crate::cache_v3::require_safe_path(&policy.root)?;
    let root = resolve_future_path(&policy.root)?;
    let workspace = resolve_future_path(workspace)?;
    if root.starts_with(&workspace) || workspace.starts_with(&root) {
        return Err(cache_error(CacheFailureCode::UnsafePath, "cache and run parent/workspace must not overlap"));
    }
    for input in inputs.values() {
        if root.starts_with(&input.path) || input.path.starts_with(&root) {
            return Err(cache_error(CacheFailureCode::UnsafePath, "cache and source paths must not overlap"));
        }
    }
    let parent = root.parent().ok_or_else(|| cache_error(CacheFailureCode::UnsafePath, "cache root has no parent"))?;
    // Initialization creates only an absent leaf; the caller selects an existing parent.
    crate::cache_v3::available_storage(if root.exists() { &root } else { parent })?;
    Ok(())
}

/// Opting into a bounded cache also narrows provider output bounds. The resulting
/// values are recorded in the ordinary execution report and invocation identity.
pub(super) fn bounded_limits(limits: ProviderExecutionLimits, policy: Option<&CachePolicy>) -> ProviderExecutionLimits {
    match policy {
        None => limits,
        Some(policy) => ProviderExecutionLimits {
            maximum_artifact_bytes: limits.maximum_artifact_bytes.min(policy.maximum_entry_bytes / 4),
            maximum_artifact_files: limits.maximum_artifact_files.min((policy.maximum_entry_files / 4).max(1)),
            ..limits
        },
    }
}

pub(super) fn reserve_stage(session: &CacheSession, workspace: &PipelineV3Workspace, key: &str, cacheable: bool) -> Result<()> {
    let extra = if !cacheable || session.path(key)?.exists() { 0 } else { 1 };
    // Reserve the full entry envelope before producing anything, including the
    // proof documents. Temporary storage is included in namespace accounting.
    session.preflight(if extra == 1 { session.policy.maximum_entry_bytes } else { 0 }, extra)?;
    let required = session.policy.maximum_entry_bytes.checked_mul(3)
        .ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "stage storage reservation overflow"))?;
    let available = crate::cache_v3::available_storage(workspace.root())?;
    if required.checked_add(session.policy.minimum_free_bytes).is_none_or(|need| need > available) {
        return Err(cache_error(CacheFailureCode::StorageLimit, "insufficient run storage for bounded candidates, accepted outputs and evidence"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cached_attempt(session: &CacheSession, workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, stage: &ResolvedPipelineStage,
    artifacts: &BTreeMap<String, RuntimeArtifact>, checkpoints: &BTreeMap<String, StageCheckpoint>, limits: ProviderExecutionLimits,
    revision: u64, cancellation: &CancellationToken) -> Result<(Option<ProviderAttempt>, CacheDecision)> {
    let inputs = stage_input_evidence(stage, artifacts)?;
    let key = cache_key(plan, stage, &inputs, checkpoints, limits)?;
    let Some((entry, expired)) = session.lookup(&key, cancellation)? else {
        return Ok((None, decision(&stage.id, Some(key), CacheDecisionKind::Miss, None)));
    };
    if expired { return Ok((None, decision(&stage.id, Some(key), CacheDecisionKind::Expired, Some(entry.entry_sha256)))); }
    let origin = PipelineV3Workspace::open_read_only(session.path(&key)?.join("snapshot"))?;
    let origin_plan = load_workspace_plan(&origin)?;
    validate_execution_subset(&origin_plan)?;
    validate_provider_lock_evidence(&origin, &origin_plan)?;
    let origin_stage = origin_plan.payload.stages.iter().find(|value| value.id == stage.id)
        .ok_or_else(|| cache_error(CacheFailureCode::CorruptEntry, "cached producer is absent from origin plan"))?;
    if origin_plan.plan_sha256 != entry.payload.origin_plan_sha256 || origin_stage != stage || origin_plan.payload.policy != plan.payload.policy {
        return Err(cache_error(CacheFailureCode::IncompatibleEntry, "cached stage or policy differs from current semantics"));
    }
    let mut origin_checkpoints = BTreeMap::new();
    load_proof_checkpoints(&origin, &entry.payload.checkpoint, &mut origin_checkpoints, origin_plan.payload.stages.len())?;
    let checkpoint = origin_checkpoints.get(&stage.id).ok_or_else(|| cache_error(CacheFailureCode::CorruptEntry, "cached checkpoint missing"))?;
    let invocation = stage_invocation_sha256(stage, &inputs, checkpoints, limits.into())?;
    if checkpoint.payload.inputs != inputs || checkpoint.payload.stage_invocation_sha256 != invocation
        || cache_key(&origin_plan, origin_stage, &checkpoint.payload.inputs, &origin_checkpoints, limits)? != key
        || result_identity(&origin, checkpoint)? != entry.payload.result_sha256 {
        return Err(cache_error(CacheFailureCode::IncompatibleEntry, "cached invocation, lineage or accepted result differs"));
    }
    // Check the full origin acceptance boundary. Dependency checkpoint metadata
    // is retained; current upstream artifacts were accepted in this run.
    verify_checkpoint(&origin, &origin_plan, origin_stage, &origin_checkpoints)?;
    ensure_stage_not_cancelled(cancellation, &stage.id, "before cached artifact materialization")?;
    let candidate_root = create_attempt_directory(workspace, &stage.id, revision)?.join("cached-candidate");
    fs::create_dir(&candidate_root).map_err(crate::cache_v3::io_error)?;
    for output in &checkpoint.payload.outputs {
        let relative = output.relative_path.as_ref().ok_or_else(|| cache_error(CacheFailureCode::CorruptEntry, "cached output has no path"))?;
        let destination = candidate_root.join(relative);
        ensure_confined_directories(&candidate_root, destination.parent().ok_or_else(|| cache_error(CacheFailureCode::UnsafePath, "cached output has no parent"))?)?;
        copy_artifact_to_fresh_inodes(&origin.root().join(relative), &destination, output.kind, cancellation, &stage.id)?;
    }
    let bytes = read_regular_workspace_evidence(&origin, &checkpoint.payload.execution_report)?;
    let report = ProviderExecutionReport::from_json_slice(&bytes)?;
    if report.payload.bounds != limits.into() { return Err(cache_error(CacheFailureCode::IncompatibleEntry, "cached runtime bounds changed")); }
    let reference = checkpoint.payload.execution_report.clone();
    publish_content_addressed_json(workspace, &workspace.root().join(&reference.relative_path), &report, &report.report_sha256)?;
    let proof = serde_json::json!({ "schema":"aniflow.cache-origin/v1", "entry":&entry,
        "origin_checkpoint":checkpoint, "current_plan_sha256":plan.plan_sha256 });
    let digest = canonical_sha256(&proof)?;
    publish_content_addressed_json(workspace, &workspace.providers().join(format!("cache-origin-{digest}.json")), &proof, &digest)?;
    let receipt = decision(&stage.id, Some(key), CacheDecisionKind::Hit, Some(entry.entry_sha256));
    Ok((Some(ProviderAttempt { candidate_root, stage_invocation_sha256: invocation, report, report_reference: reference }), receipt))
}

pub(super) fn publish_cached_stage(session: &CacheSession, workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint, checkpoints: &BTreeMap<String, StageCheckpoint>, limits: ProviderExecutionLimits, cancellation: &CancellationToken) -> Result<CacheDecision> {
    let key = cache_key(plan, stage, &checkpoint.payload.inputs, checkpoints, limits)?;
    let result_sha256 = result_identity(workspace, checkpoint)?;
    if let Some((entry, _expired)) = session.lookup(&key, cancellation)? {
        if entry.payload.result_sha256 != result_sha256 {
            return Err(cache_error(CacheFailureCode::ConflictingEntry, "the same cache key produced different accepted results; invalidate explicitly before replacing it"));
        }
        return Ok(decision(&stage.id, Some(key), CacheDecisionKind::Existing, Some(entry.entry_sha256)));
    }
    let staging = session.staging()?;
    let snapshot = PipelineV3Workspace::create_at(staging.path().join("snapshot"))?;
    let plan_bytes = normalized_plan_bytes(plan)?;
    let mut copied_bytes = plan_bytes.len() as u64;
    for locked in executable_stages(plan) {
        reserve_proof_bytes(&mut copied_bytes, serde_json::to_vec_pretty(&locked.provider_lock).map_err(crate::cache_v3::io_error)?.len() as u64 + 1, &session.policy)?;
    }
    reserve_proof_bytes(&mut copied_bytes, 0, &session.policy)?;
    snapshot.publish_plan_bytes(&plan_bytes)?;
    publish_provider_locks(&snapshot, plan)?;
    let mut proof = BTreeMap::new();
    load_proof_checkpoints(workspace, &checkpoint.reference()?, &mut proof, plan.payload.stages.len())?;
    let mut references = BTreeSet::new();
    for value in proof.values() {
        reserve_proof_bytes(&mut copied_bytes, serde_json::to_vec_pretty(value).map_err(crate::cache_v3::io_error)?.len() as u64 + 1, &session.policy)?;
        publish_stage_checkpoint(&snapshot, value)?;
        let json = serde_json::to_value(value).map_err(crate::cache_v3::io_error)?;
        copy_evidence_references(workspace, &snapshot, &json, &mut references, &mut copied_bytes, &session.policy, cancellation)?;
    }
    for output in &checkpoint.payload.outputs {
        copied_bytes = copied_bytes.checked_add(output.byte_count).ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "snapshot budget overflow"))?;
        if copied_bytes > session.policy.maximum_entry_bytes { return Err(cache_error(CacheFailureCode::StorageLimit, "cache proof and outputs exceed entry budget")); }
        let relative = output.relative_path.as_ref().ok_or_else(|| cache_error(CacheFailureCode::CorruptEntry, "accepted output has no path"))?;
        let destination = snapshot.root().join(relative);
        ensure_confined_directories(snapshot.root(), destination.parent().ok_or_else(|| cache_error(CacheFailureCode::UnsafePath, "snapshot output has no parent"))?)?;
        copy_artifact_to_fresh_inodes(&workspace.root().join(relative), &destination, output.kind, cancellation, &stage.id)?;
    }
    verify_checkpoint(&snapshot, plan, stage, &proof)?;
    let payload = CacheEntryPayload {
        owner: session.policy.owner.clone(), key_sha256: key.clone(), stage_id: stage.id.clone(),
        origin_plan_sha256: plan.plan_sha256.clone(), checkpoint: checkpoint.reference()?, result_sha256,
        created_at: Utc::now(), contents: Vec::new(),
    };
    let (entry, published) = session.publish(staging, payload, cancellation)?;
    Ok(decision(&stage.id, Some(key), if published { CacheDecisionKind::Published } else { CacheDecisionKind::Existing }, Some(entry.entry_sha256)))
}

fn load_proof_checkpoints(workspace: &PipelineV3Workspace, reference: &StageCheckpointReference, result: &mut BTreeMap<String, StageCheckpoint>, limit: usize) -> Result<()> {
    // Iterative walk bounds malformed graphs without recursive stack growth.
    let mut pending = vec![reference.clone()];
    while let Some(reference) = pending.pop() {
        if let Some(previous) = result.get(&reference.stage_id) {
            if previous.reference()? != reference { return Err(cache_error(CacheFailureCode::CorruptEntry, "conflicting dependency checkpoints")); }
            continue;
        }
        if result.len() >= limit { return Err(cache_error(CacheFailureCode::CorruptEntry, "checkpoint graph exceeds its plan")); }
        let value = load_stage_checkpoint(workspace, &reference)?;
        pending.extend(value.payload.dependencies.iter().cloned());
        result.insert(reference.stage_id, value);
    }
    Ok(())
}

fn copy_evidence_references(source: &PipelineV3Workspace, destination: &PipelineV3Workspace, value: &serde_json::Value,
    seen: &mut BTreeSet<String>, bytes: &mut u64, policy: &CachePolicy, cancellation: &CancellationToken) -> Result<()> {
    // JSON depth is bounded by serde_json's default recursion limit; graph width
    // and cumulative copied bytes are separately bounded here.
    let mut pending = vec![value.clone()];
    while let Some(value) = pending.pop() {
        ensure_execution_not_cancelled(cancellation, "while retaining cache proof")?;
        match value {
            serde_json::Value::Object(fields) if fields.len() == 2 && fields.contains_key("relative_path") && fields.contains_key("sha256") => {
                let reference: EvidenceReference = serde_json::from_value(serde_json::Value::Object(fields)).map_err(crate::cache_v3::io_error)?;
                reference.validate()?;
                if !reference.relative_path.starts_with("providers/") { return Err(cache_error(CacheFailureCode::UnsafePath, "cache proof may only retain provider evidence")); }
                if !seen.insert(reference.relative_path.clone()) { continue; }
                if seen.len() as u64 > policy.maximum_entry_files { return Err(cache_error(CacheFailureCode::StorageLimit, "cache proof file count exceeds policy")); }
                let data = read_regular_workspace_evidence(source, &reference)?;
                *bytes = bytes.checked_add(data.len() as u64).ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "cache proof size overflow"))?;
                if *bytes > policy.maximum_entry_bytes { return Err(cache_error(CacheFailureCode::StorageLimit, "cache proof exceeds entry budget")); }
                let document: serde_json::Value = serde_json::from_slice(&data).map_err(crate::cache_v3::io_error)?;
                let target = destination.root().join(&reference.relative_path);
                ensure_confined_directories(destination.root(), target.parent().ok_or_else(|| cache_error(CacheFailureCode::UnsafePath, "proof path has no parent"))?)?;
                copy_file_to_fresh_inode(&source.root().join(&reference.relative_path), &target, cancellation, "cache-proof")?;
                pending.push(document);
            }
            serde_json::Value::Object(fields) => pending.extend(fields.into_iter().map(|(_, value)| value)),
            serde_json::Value::Array(values) => pending.extend(values),
            _ => {},
        }
    }
    Ok(())
}

fn result_identity(workspace: &PipelineV3Workspace, checkpoint: &StageCheckpoint) -> Result<String> {
    let mut validations = Vec::new();
    for validation in &checkpoint.payload.validations {
        let bytes = read_regular_workspace_evidence(workspace, &validation.evidence)?;
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(crate::cache_v3::io_error)?;
        if validation.contract != ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1 {
            let report: crate::validation::ProviderValidationReport = serde_json::from_slice(&bytes).map_err(crate::cache_v3::io_error)?;
            value = serde_json::to_value(report.observation).map_err(crate::cache_v3::io_error)?;
            // Context hashes include origin-plan identity; all semantic context is
            // already bound by the key. Preserve every actual check/observation.
            if let Some(object) = value.as_object_mut() { object.remove("context_sha256"); }
        }
        validations.push(serde_json::json!({"id":validation.id,"contract":validation.contract,"observation":value}));
    }
    canonical_sha256(&serde_json::json!({"outputs":checkpoint.payload.outputs,"validations":validations}))
}

fn reserve_proof_bytes(total: &mut u64, additional: u64, policy: &CachePolicy) -> Result<()> {
    *total = total.checked_add(additional).ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "cache proof size overflow"))?;
    if *total > policy.maximum_entry_bytes { return Err(cache_error(CacheFailureCode::StorageLimit, "cache proof exceeds entry budget")); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rerun_frontier_is_transitive_and_preserves_independent_branches() {
        // Start from a structurally representative plan; graph-only selection
        // does not resolve providers or execute external commands.
        let bytes = include_bytes!("../../docs/contracts/examples/pipeline-v3-plan-v1.example.json");
        let mut plan: PipelineV3Plan = serde_json::from_slice(bytes).unwrap();
        let mut first = plan.payload.stages[0].clone(); first.id = "first".to_owned(); first.depends_on.clear();
        let mut second = first.clone(); second.id = "second".to_owned(); second.depends_on = vec!["first".to_owned()];
        let mut third = first.clone(); third.id = "third".to_owned(); third.depends_on = vec!["second".to_owned()];
        let mut independent = first.clone(); independent.id = "independent".to_owned();
        plan.payload.stages = vec![first, second, third, independent];
        assert_eq!(rerun_frontier(&plan, &["first".to_owned()]).unwrap(), BTreeSet::from(["first".to_owned(), "second".to_owned(), "third".to_owned()]));
        assert!(rerun_frontier(&plan, &["missing".to_owned()]).is_err());
    }
}
