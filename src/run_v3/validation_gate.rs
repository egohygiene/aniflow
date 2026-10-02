//! Inline validator execution and content-addressed acceptance boundaries.
use super::*;
use crate::pipeline_v3::{ArtifactValidation, validation_context_id};
use crate::provider::ArtifactRole;
use crate::validation::*;
use sha2::{Digest, Sha256};

pub(super) fn executable_stages(plan: &PipelineV3Plan) -> impl Iterator<Item = &ResolvedPipelineStage> {
    plan.payload.stages.iter().flat_map(|stage| std::iter::once(stage).chain(stage.validation_providers.iter().map(|gate| gate.stage.as_ref())))
}

#[allow(clippy::too_many_arguments)]
fn context_for(plan_sha256: &str, stage: &ResolvedPipelineStage, validation: &ArtifactValidation, artifact: &ArtifactEvidence, inputs: &[ArtifactEvidence], invocation: &str) -> Result<ValidationContext> {
    let gate = stage.validation_providers.iter().find(|gate| gate.validation_id == validation.id)
        .ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &validation.id, "resolved validator is missing"))?;
    let context = ValidationContext {
        schema: VALIDATION_CONTEXT_SCHEMA_V1.to_owned(), validation_id: validation.id.clone(), contract: validation.contract.clone(),
        plan_sha256: plan_sha256.to_owned(), producer_invocation_sha256: invocation.to_owned(), producer_lock_sha256: stage.provider_lock.lock_sha256.clone(),
        validator_lock_sha256: gate.stage.provider_lock.lock_sha256.clone(), artifact: artifact.clone(), inputs: inputs.to_vec(), temporal: validation.temporal.clone(),
    };
    context.validate()?;
    Ok(context)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_validation_gate(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, stage: &ResolvedPipelineStage, validation: &ArtifactValidation, artifact: &ArtifactEvidence, inputs: &[ArtifactEvidence], invocation: &str, artifacts: &BTreeMap<String, RuntimeArtifact>, providers: &BTreeMap<String, ResolvedProvider>, limits: ProviderExecutionLimits, revision: u64, cancellation: &CancellationToken) -> Result<ValidationEvidence> {
    ensure_stage_not_cancelled(cancellation, &stage.id, "before validator execution")?;
    let context = context_for(&plan.plan_sha256, stage, validation, artifact, inputs, invocation)?;
    let gate = stage.validation_providers.iter().find(|gate| gate.validation_id == validation.id)
        .ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &validation.id, "validator plan missing"))?;
    let provider = providers.get(&gate.stage.id).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &validation.id, "exact validator authority missing"))?;
    let mut context_file = TempFileBuilder::new().prefix("aniflow-validation-context-").suffix(".json").tempfile()
        .map_err(|error| Error::new(ErrorCategory::Io, format!("cannot create validation context: {error}")))?;
    context_file.write_all(&crate::provider::canonical_json_bytes(&context)?).map_err(|error| Error::new(ErrorCategory::Io, format!("cannot write validation context: {error}")))?;
    context_file.as_file_mut().sync_all().map_err(|error| Error::new(ErrorCategory::Io, format!("cannot sync validation context: {error}")))?;
    let observation = observe_existing_artifact(context_file.path(), ArtifactKind::File)
        .map_err(|error| Error::new(ErrorCategory::State, error.detail))?;
    let mut bound = artifacts.clone();
    bound.insert(validation_context_id(&validation.id), RuntimeArtifact {
        artifact_type: VALIDATION_CONTEXT_SCHEMA_V1.to_owned(), artifact_role: ArtifactRole::RunEvidence, stream_role: None,
        kind: ArtifactKind::File, path: context_file.path().to_path_buf(), relative_path: None, source_identity: None, observation,
    });
    let attempt = execute_provider_attempt(workspace, &gate.stage, provider, &bound, &BTreeMap::new(), limits, revision, cancellation)?;
    if attempt.report.payload.outcome != ProviderExecutionOutcome::Succeeded {
        return Err(refusal(ValidationFailureCode::RejectedObservation, &validation.id, "validator execution did not succeed; no acceptance was published"));
    }
    // Check all readable inputs, including the target and context, after the process.
    for input in bound.values() { verify_runtime_artifact_cancellable(workspace, input, cancellation)?; }
    let expected = &gate.stage.outputs[0].artifacts[0];
    let path = attempt.candidate_root.join(&expected.relative_path);
    validate_existing_confined_parent(&attempt.candidate_root, &path)?;
    let observed = observe_existing_artifact_cancellable(&path, ArtifactKind::File, cancellation)
        .map_err(|error| refusal(ValidationFailureCode::MissingEvidence, &validation.id, error.detail))?;
    let recorded = attempt.report.payload.outputs.first().ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &validation.id, "validator report output missing"))?;
    if attempt.report.payload.outputs.len() != 1 || recorded.port != "report" || recorded.relative_path != expected.relative_path
        || recorded.sha256 != observed.sha256 || recorded.byte_count != observed.byte_count || observed.byte_count > 64 * 1024 * 1024 {
        return Err(refusal(ValidationFailureCode::StaleEvidence, &validation.id, "validator report differs from its bounded execution evidence"));
    }
    let bytes = read_bounded_report(&path)?;
    if format!("{:x}", Sha256::digest(&bytes)) != observed.sha256 || crate::temporal::sha256_file(&path).map_err(|e| Error::from_anyhow(ErrorCategory::Io, e))? != observed.sha256 {
        return Err(refusal(ValidationFailureCode::ArtifactChanged, &validation.id, "validator report changed during observation"));
    }
    let observation: ValidatorObservation = decode_closed_report(&bytes, &validation.id)?;
    observation.accept(&context)?;
    let raw_observation = String::from_utf8(bytes).map_err(|error| refusal(ValidationFailureCode::IncompatibleEvidence, &validation.id, error.to_string()))?;
    let report = ProviderValidationReport { schema: VALIDATION_REPORT_SCHEMA_V1.to_owned(), context, observation, raw_observation, execution_report: attempt.report_reference };
    report.validate()?;
    let sha256 = canonical_sha256(&report)?;
    let path = workspace.providers().join(format!("{}-{}-{}.validation.json", stage.id, validation.id, sha256));
    publish_content_addressed_json(workspace, &path, &report, &sha256)?;
    ensure_stage_not_cancelled(cancellation, &stage.id, "after validator acceptance")?;
    Ok(ValidationEvidence { id: validation.id.clone(), contract: validation.contract.clone(), artifact_id: artifact.id.clone(), artifact_sha256: artifact.sha256.clone(), accepted: true,
        evidence: EvidenceReference { relative_path: workspace_relative_path(workspace, &path)?, sha256 } })
}

fn read_bounded_report(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    if bytes.len() > 64 * 1024 * 1024 { return Err(refusal(ValidationFailureCode::IncompatibleEvidence, "report", "validation report exceeds 64 MiB bound")); }
    Ok(bytes)
}

fn decode_closed_report<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8], id: &str) -> Result<T> {
    let value: T = serde_json::from_slice(bytes).map_err(|error| refusal(ValidationFailureCode::IncompatibleEvidence, id, format!("invalid validation document: {error}")))?;
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| Error::new(ErrorCategory::State, error.to_string()))?;
    if serde_json::to_value(&value).map_err(|error| Error::new(ErrorCategory::Internal, error.to_string()))? != raw {
        return Err(refusal(ValidationFailureCode::IncompatibleEvidence, id, "validation evidence must use its normalized closed representation"));
    }
    Ok(value)
}

pub(super) fn verify_provider_validation(workspace: &PipelineV3Workspace, stage: &ResolvedPipelineStage, checkpoint: &StageCheckpoint, requirement: &ArtifactValidation, evidence: &ValidationEvidence, bytes: &[u8]) -> Result<()> {
    let report: ProviderValidationReport = decode_closed_report(bytes, &requirement.id)?;
    report.validate()?;
    let artifact = checkpoint.payload.outputs.iter().find(|artifact| artifact.id == requirement.artifact)
        .ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &requirement.id, "validation artifact missing"))?;
    let context = context_for(&checkpoint.payload.plan_sha256, stage, requirement, artifact, &checkpoint.payload.inputs, &checkpoint.payload.stage_invocation_sha256)?;
    if report.context != context || canonical_sha256(&report)? != evidence.evidence.sha256 {
        return Err(refusal(ValidationFailureCode::StaleEvidence, &requirement.id, "validation context or canonical digest changed"));
    }
    let gate = stage.validation_providers.iter().find(|gate| gate.validation_id == requirement.id)
        .ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &requirement.id, "validator lock missing"))?;
    let bytes = read_regular_workspace_evidence(workspace, &report.execution_report)?;
    let execution = ProviderExecutionReport::from_json_slice(&bytes)?;
    let producer_bytes = read_regular_workspace_evidence(workspace, &checkpoint.payload.execution_report)?;
    let producer_execution = ProviderExecutionReport::from_json_slice(&producer_bytes)?;
    if execution.payload.bounds != producer_execution.payload.bounds {
        return Err(refusal(ValidationFailureCode::IncompatibleEvidence, &requirement.id, "validator execution bounds differ from producer policy"));
    }
    if execution.report_sha256 != report.execution_report.sha256 || execution.payload.provider_lock != gate.stage.provider_lock
        || execution.payload.outcome != ProviderExecutionOutcome::Succeeded || execution.payload.outputs.len() != 1 {
        return Err(refusal(ValidationFailureCode::IncompatibleEvidence, &requirement.id, "validator process evidence is not an exact successful locked execution"));
    }
    // Bind the retained raw observation to the exact runtime output bytes.
    let output = &execution.payload.outputs[0];
    if output.port != "report" || output.relative_path != gate.stage.outputs[0].artifacts[0].relative_path || output.kind != ArtifactKind::File || output.file_count != 1 || output.byte_count == 0 {
        return Err(refusal(ValidationFailureCode::IncompatibleEvidence, &requirement.id, "validator output shape changed"));
    }
    if output.sha256 != format!("{:x}", Sha256::digest(report.raw_observation.as_bytes())) || output.byte_count != report.raw_observation.len() as u64 {
        return Err(refusal(ValidationFailureCode::ContradictoryEvidence, &requirement.id, "provider observation differs from runtime output identity"));
    }
    verify_validator_execution_binding(&report, &execution, &gate.stage)?;
    Ok(())
}

fn verify_validator_execution_binding(report: &ProviderValidationReport, execution: &ProviderExecutionReport, gate: &ResolvedPipelineStage) -> Result<()> {
    let context_bytes = crate::provider::canonical_json_bytes(&report.context)?;
    let mut inputs = Vec::new();
    for binding in &gate.inputs {
        let (artifact, role) = if binding.port == "artifact" {
            (report.context.artifact.clone(), report.context.artifact.artifact_role)
        } else if binding.port == "context" {
            (ArtifactEvidence { id: binding.artifacts[0].clone(), port: "context".to_owned(), relative_path: None,
                artifact_type: VALIDATION_CONTEXT_SCHEMA_V1.to_owned(), artifact_role: ArtifactRole::RunEvidence, stream_role: None,
                kind: ArtifactKind::File, file_count: 1, byte_count: context_bytes.len() as u64, sha256: report.context.sha256()? }, ArtifactRole::RunEvidence)
        } else {
            let input = report.context.inputs.iter().find(|input| input.id == binding.artifacts[0]).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &report.context.validation_id, "validator source binding missing"))?;
            (input.clone(), input.artifact_role)
        };
        let mut artifact = artifact;
        artifact.port = binding.port.clone(); artifact.artifact_role = role;
        inputs.push(artifact);
    }
    let invocation = stage_invocation_sha256(gate, &inputs, &BTreeMap::new(), execution.payload.bounds)?;
    if report.execution_report.relative_path != execution_report_relative_path(&gate.id, &invocation, &execution.report_sha256)? {
        return Err(refusal(ValidationFailureCode::StaleEvidence, &report.context.validation_id, "validator execution belongs to a different invocation"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record(layer: ValidationLayer, scope_id: &str, plan: &str, invocation: Option<&str>, lock: Option<&str>, inputs: &[ArtifactEvidence], artifacts: &[ArtifactEvidence], validations: &[ValidationEvidence], children: Vec<EvidenceReference>) -> AcceptanceRecord {
    AcceptanceRecord { schema: ACCEPTANCE_RECORD_SCHEMA_V1.to_owned(), semantics: ACCEPTANCE_SEMANTICS_V1.to_owned(), layer, scope_id: scope_id.to_owned(), plan_sha256: plan.to_owned(),
        producer_invocation_sha256: invocation.map(str::to_owned), producer_lock_sha256: lock.map(str::to_owned), inputs: inputs.to_vec(), artifacts: artifacts.to_vec(), validations: validations.to_vec(), children }
}

fn reference_for(record: &AcceptanceRecord) -> Result<EvidenceReference> {
    let sha256 = record.sha256()?;
    Ok(EvidenceReference { relative_path: format!("providers/acceptance-{sha256}.json"), sha256 })
}

fn publish_record(workspace: &PipelineV3Workspace, record: &AcceptanceRecord) -> Result<EvidenceReference> {
    let reference = reference_for(record)?;
    publish_content_addressed_json(workspace, &workspace.root().join(&reference.relative_path), record, &reference.sha256)?;
    Ok(reference)
}

fn verify_record(workspace: &PipelineV3Workspace, expected: &AcceptanceRecord, reference: &EvidenceReference) -> Result<()> {
    if reference != &reference_for(expected)? { return Err(refusal(ValidationFailureCode::StaleEvidence, &expected.scope_id, "acceptance reference differs from expected boundary")); }
    let bytes = read_regular_workspace_evidence(workspace, reference)?;
    let observed: AcceptanceRecord = decode_closed_report(&bytes, &expected.scope_id)?;
    observed.validate()?;
    if observed != *expected { return Err(refusal(ValidationFailureCode::ContradictoryEvidence, &expected.scope_id, "acceptance document differs from its required boundary")); }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_records(plan: &PipelineV3Plan, stage: &ResolvedPipelineStage, invocation: &str, inputs: &[ArtifactEvidence], outputs: &[ArtifactEvidence], validations: &[ValidationEvidence]) -> Result<(Vec<AcceptanceRecord>, AcceptanceRecord)> {
    let mut components = Vec::new();
    for artifact in outputs {
        let artifact_validations = validations.iter().filter(|v| v.artifact_id == artifact.id).cloned().collect::<Vec<_>>();
        components.push(record(ValidationLayer::Component, &artifact.id, &plan.plan_sha256, Some(invocation), Some(&stage.provider_lock.lock_sha256), inputs, std::slice::from_ref(artifact), &artifact_validations, Vec::new()));
    }
    let children = components.iter().map(reference_for).collect::<Result<Vec<_>>>()?;
    let stage_record = record(ValidationLayer::Stage, &stage.id, &plan.plan_sha256, Some(invocation), Some(&stage.provider_lock.lock_sha256), inputs, outputs, validations, children);
    Ok((components, stage_record))
}

pub(super) fn publish_stage_acceptance(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, stage: &ResolvedPipelineStage, invocation: &str, inputs: &[ArtifactEvidence], outputs: &[ArtifactEvidence], validations: &[ValidationEvidence]) -> Result<EvidenceReference> {
    let (components, stage_record) = stage_records(plan, stage, invocation, inputs, outputs, validations)?;
    for component in &components { publish_record(workspace, component)?; }
    publish_record(workspace, &stage_record)
}

pub(super) fn verify_stage_acceptance(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, stage: &ResolvedPipelineStage, checkpoint: &StageCheckpoint) -> Result<()> {
    let reference = checkpoint.payload.acceptance.as_ref().ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &stage.id, "checkpoint predates layered acceptance or lost its evidence"))?;
    let (components, stage_record) = stage_records(plan, stage, &checkpoint.payload.stage_invocation_sha256, &checkpoint.payload.inputs, &checkpoint.payload.outputs, &checkpoint.payload.validations)?;
    for component in &components { verify_record(workspace, component, &reference_for(component)?)?; }
    verify_record(workspace, &stage_record, reference)
}

pub(super) fn verify_all_checkpoints(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, checkpoints: &BTreeMap<String, StageCheckpoint>) -> Result<()> {
    if checkpoints.len() != plan.payload.stages.len() { return Err(refusal(ValidationFailureCode::MissingEvidence, "delivery", "delivery requires every planned stage checkpoint")); }
    for stage in &plan.payload.stages { verify_checkpoint(workspace, plan, stage, checkpoints)?; }
    Ok(())
}

pub(super) fn verify_checkpoint(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, stage: &ResolvedPipelineStage, checkpoints: &BTreeMap<String, StageCheckpoint>) -> Result<()> {
    let checkpoint = checkpoints.get(&stage.id).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &stage.id, "accepted checkpoint missing"))?;
    checkpoint.validate()?;
    if load_stage_checkpoint(workspace, &checkpoint.reference()?)? != *checkpoint {
        return Err(refusal(ValidationFailureCode::StaleEvidence, &stage.id, "durable checkpoint differs from accepted memory"));
    }
    if checkpoint.payload.plan_sha256 != plan.plan_sha256 || checkpoint.payload.provider_lock_sha256 != stage.provider_lock.lock_sha256 {
        return Err(refusal(ValidationFailureCode::StaleEvidence, &stage.id, "checkpoint authority differs from the plan"));
    }
    let mut inputs = Vec::new();
    for binding in &stage.inputs {
        for id in &binding.artifacts {
            let mut input = if let Some(source) = plan.payload.inputs.iter().find(|input| &input.id == id) {
                ArtifactEvidence { id: id.clone(), port: binding.port.clone(), relative_path: None, artifact_type: source.artifact_type.clone(), artifact_role: source.artifact_role, stream_role: source.stream_role,
                    kind: artifact_kind(source.kind), file_count: source.file_count, byte_count: source.byte_count, sha256: source.content_sha256.clone() }
            } else {
                checkpoints.values().flat_map(|cp| &cp.payload.outputs).find(|output| &output.id == id).cloned().ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &stage.id, "upstream input evidence missing"))?
            };
            input.port = binding.port.clone(); inputs.push(input);
        }
    }
    if checkpoint.payload.inputs != inputs { return Err(refusal(ValidationFailureCode::StaleEvidence, &stage.id, "checkpoint source lineage differs from its plan and dependencies")); }
    let bytes = read_regular_workspace_evidence(workspace, &checkpoint.payload.execution_report)?;
    let execution = ProviderExecutionReport::from_json_slice(&bytes)?;
    let invocation = stage_invocation_sha256(stage, &inputs, checkpoints, execution.payload.bounds)?;
    if invocation != checkpoint.payload.stage_invocation_sha256 { return Err(refusal(ValidationFailureCode::StaleEvidence, &stage.id, "checkpoint invocation changed")); }
    let mut reasons = Vec::new();
    compare_dependency_evidence(workspace, stage, checkpoint, checkpoints, &mut reasons)?;
    validate_execution_report_evidence(workspace, stage, execution.payload.bounds, &invocation, checkpoint, &mut reasons)?;
    let expected = expected_artifacts(stage).collect::<Vec<_>>();
    if expected.len() != checkpoint.payload.outputs.len() { return Err(refusal(ValidationFailureCode::MissingEvidence, &stage.id, "checkpoint output set is incomplete")); }
    for ((port, expected), output) in expected.into_iter().zip(&checkpoint.payload.outputs) {
        if output.id != expected.id || output.port != port || output.relative_path.as_deref() != Some(expected.relative_path.as_str()) || output.kind != expected_artifact_kind(expected)?
            || output.artifact_type != expected.artifact_type || output.artifact_role != expected.artifact_role || output.stream_role != expected.stream_role {
            return Err(refusal(ValidationFailureCode::ContradictoryEvidence, &stage.id, "output semantics differ from planned ports"));
        }
        let path = workspace.root().join(&expected.relative_path);
        validate_existing_confined_parent(workspace.root(), &path)?;
        let observed = observe_existing_artifact(&path, output.kind).map_err(|failure| refusal(ValidationFailureCode::ArtifactChanged, &stage.id, failure.detail))?;
        if observed.sha256 != output.sha256 || observed.byte_count != output.byte_count || observed.file_count != output.file_count {
            return Err(refusal(ValidationFailureCode::ArtifactChanged, &stage.id, "accepted artifact bytes changed"));
        }
    }
    validate_validation_evidence(workspace, stage, checkpoint, &checkpoint.payload.outputs, &mut reasons)?;
    if !reasons.is_empty() { return Err(refusal(ValidationFailureCode::IncompatibleEvidence, &stage.id, reasons.iter().map(|r| r.detail.as_str()).collect::<Vec<_>>().join("; "))); }
    verify_stage_acceptance(workspace, plan, stage, checkpoint)
}

fn delivery_records(plan: &PipelineV3Plan, checkpoints: &BTreeMap<String, StageCheckpoint>, final_artifacts: &[ArtifactEvidence]) -> Result<(Vec<AcceptanceRecord>, AcceptanceRecord)> {
    let mut candidates = Vec::new();
    let mut expected_final = Vec::new();
    for output in &plan.payload.outputs {
        let checkpoint = checkpoints.values().find(|cp| cp.payload.outputs.iter().any(|a| a.id == output.artifact)).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &output.id, "candidate master producer missing"))?;
        let artifact = checkpoint.payload.outputs.iter().find(|a| a.id == output.artifact).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &output.id, "candidate master missing"))?;
        let mut validations = Vec::new();
        for id in &output.required_validations {
            let validation = checkpoint.payload.validations.iter().find(|v| &v.id == id && v.artifact_id == artifact.id && v.artifact_sha256 == artifact.sha256 && v.accepted)
                .ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &output.id, "required candidate validation missing"))?;
            validations.push(validation.clone());
        }
        let parent = checkpoint.payload.acceptance.clone().ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, &output.id, "candidate lacks accepted stage evidence"))?;
        candidates.push(record(ValidationLayer::CandidateMaster, &output.id, &plan.plan_sha256, Some(&checkpoint.payload.stage_invocation_sha256), Some(&checkpoint.payload.provider_lock_sha256), &checkpoint.payload.inputs, std::slice::from_ref(artifact), &validations, vec![parent]));
        let mut final_artifact = artifact.clone(); final_artifact.id = output.id.clone(); final_artifact.port = "final".to_owned(); expected_final.push(final_artifact);
    }
    if expected_final != final_artifacts { return Err(refusal(ValidationFailureCode::ContradictoryEvidence, "delivery", "delivery output set differs from candidate masters")); }
    let children = candidates.iter().map(reference_for).collect::<Result<Vec<_>>>()?;
    let delivery = record(ValidationLayer::Delivery, "delivery", &plan.plan_sha256, None, None, &[], final_artifacts, &[], children);
    Ok((candidates, delivery))
}

pub(super) fn publish_delivery_acceptance(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, checkpoints: &BTreeMap<String, StageCheckpoint>, final_artifacts: &[ArtifactEvidence]) -> Result<EvidenceReference> {
    let (candidates, delivery) = delivery_records(plan, checkpoints, final_artifacts)?;
    for candidate in &candidates { publish_record(workspace, candidate)?; }
    publish_record(workspace, &delivery)
}

pub(super) fn verify_status_acceptance(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan, manifest: &PipelineV3RunManifest) -> Result<()> {
    let mut checkpoints = BTreeMap::new();
    for record in &manifest.payload.stages {
        if let Some(reference) = &record.checkpoint { checkpoints.insert(record.stage_id.clone(), load_stage_checkpoint(workspace, reference)?); }
    }
    if manifest.payload.state == PipelineV3RunState::Complete {
        verify_all_checkpoints(workspace, plan, &checkpoints)?;
        let reference = manifest.payload.delivery.as_ref().ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, "delivery", "complete state lacks current layered delivery evidence; resume to revalidate"))?;
        let (candidates, delivery) = delivery_records(plan, &checkpoints, &manifest.payload.outputs)?;
        for candidate in &candidates { verify_record(workspace, candidate, &reference_for(candidate)?)?; }
        verify_record(workspace, &delivery, reference)?;
    } else {
        for stage in plan.payload.stages.iter().filter(|stage| checkpoints.contains_key(&stage.id)) {
            verify_checkpoint(workspace, plan, stage, &checkpoints)?;
        }
    }
    Ok(())
}
