//! Content-aware Pipeline v3 execution and deterministic run-local resume.
//!
//! The executor consumes an already resolved plan, re-establishes every
//! selected provider from explicit caller authority, and treats only a
//! validated immutable checkpoint as evidence of stage completion.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tempfile::Builder as TempFileBuilder;
use walkdir::WalkDir;

use crate::error::{Error, ErrorCategory, Result};
use crate::invocation_v3::{
    ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1, PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1,
    PROVIDER_INVOCATION_SCHEMA_V1, ProviderInvocationArtifactBinding, ProviderInvocationRequest,
    provider_invocation_arguments,
};
use crate::pipeline_v3::{
    PipelineInputBinding, PipelineInputIdentity, PipelineInputKind, PipelineV3Plan,
    PlannedExpectedArtifact, ResolvedPipelineStage, reobserve_pipeline_input,
};
use crate::provider::{ArtifactCardinality, CapabilityKind, SideEffect, canonical_sha256};
use crate::provider_runtime::{
    ArtifactContentObservation, ArtifactKind, ArtifactObservation, ExpectedProviderOutput,
    ProviderExecutionBounds, ProviderExecutionFailureCode, ProviderExecutionLimits,
    ProviderExecutionOutcome, ProviderExecutionReport, ProviderExecutionRequest, ProviderLock,
    ProviderRegistry, ProviderResolutionRequest, ResolvedProvider, observe_existing_artifact,
    observe_existing_artifact_cancellable,
};
use crate::segmentation::CancellationToken;
use crate::state_v3::{
    ArtifactEvidence, CompatibilityDecision, CompatibilityReason, CompatibilityReasonCode,
    EvidenceReference, PipelineV3RunManifest, PipelineV3RunState, PipelineV3StageState,
    StageCheckpoint, StageCheckpointPayload, StageCheckpointReference, StageRunRecord,
    ValidationEvidence, append_run_manifest, load_latest_run_manifest, load_stage_checkpoint,
    publish_stage_checkpoint,
};
use crate::workspace_v3::PipelineV3Workspace;

/// Versioned machine result emitted by Pipeline v3 run and resume operations.
pub const PIPELINE_V3_RUN_OUTCOME_SCHEMA_V1: &str = "aniflow.pipeline-run-outcome/v1";

/// Versioned partial machine result emitted when a Pipeline v3 run can be recovered.
pub const PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1: &str = "aniflow.pipeline-run-recovery/v1";

/// Exact resolved plan, immutable input bindings, and provider authority for a new run.
#[derive(Debug, Clone)]
pub struct PipelineV3RunRequest {
    pub plan: PipelineV3Plan,
    pub input_bindings: Vec<PipelineInputBinding>,
    pub provider_registry: ProviderRegistry,
    pub output_directory: Option<PathBuf>,
    pub execution_limits: ProviderExecutionLimits,
}

impl PipelineV3RunRequest {
    #[must_use]
    pub fn new(
        plan: PipelineV3Plan,
        input_bindings: Vec<PipelineInputBinding>,
        provider_registry: ProviderRegistry,
    ) -> Self {
        Self {
            plan,
            input_bindings,
            provider_registry,
            output_directory: None,
            execution_limits: ProviderExecutionLimits::default(),
        }
    }

    #[must_use]
    pub fn with_output_directory(mut self, output_directory: impl Into<PathBuf>) -> Self {
        self.output_directory = Some(output_directory.into());
        self
    }

    #[must_use]
    pub const fn with_execution_limits(
        mut self,
        execution_limits: ProviderExecutionLimits,
    ) -> Self {
        self.execution_limits = execution_limits;
        self
    }
}

/// Existing run and explicit authority required to verify and resume it.
#[derive(Debug, Clone)]
pub struct PipelineV3ResumeRequest {
    pub run_directory: PathBuf,
    pub input_bindings: Vec<PipelineInputBinding>,
    pub provider_registry: ProviderRegistry,
    pub execution_limits: ProviderExecutionLimits,
}

impl PipelineV3ResumeRequest {
    #[must_use]
    pub fn new(
        run_directory: impl Into<PathBuf>,
        input_bindings: Vec<PipelineInputBinding>,
        provider_registry: ProviderRegistry,
    ) -> Self {
        Self {
            run_directory: run_directory.into(),
            input_bindings,
            provider_registry,
            execution_limits: ProviderExecutionLimits::default(),
        }
    }

    #[must_use]
    pub const fn with_execution_limits(
        mut self,
        execution_limits: ProviderExecutionLimits,
    ) -> Self {
        self.execution_limits = execution_limits;
        self
    }
}

/// One validated public output from a completed Pipeline v3 run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3Output {
    pub id: String,
    pub artifact: String,
    pub path: PathBuf,
    pub sha256: String,
}

/// Stable result shared by the library and machine CLI modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3RunOutcome {
    pub schema: String,
    pub run_directory: PathBuf,
    pub plan_sha256: String,
    pub run_manifest: PathBuf,
    pub outputs: Vec<PipelineV3Output>,
    pub executed_stages: Vec<String>,
    pub reused_stages: Vec<String>,
}

/// Durable state locators retained when Pipeline v3 execution fails after startup.
///
/// `run_manifest` is present only when the CLI can validate the latest manifest
/// and its referenced authority before rendering the failure envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3RunRecovery {
    pub schema: String,
    pub run_directory: PathBuf,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_recovery_manifest"
    )]
    pub run_manifest: Option<PathBuf>,
}

fn deserialize_optional_recovery_manifest<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    PathBuf::deserialize(deserializer).map(Some)
}

/// Operation represented by a Pipeline v3 progress observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineV3RunOperation {
    Run,
    Resume,
}

/// Non-authoritative lifecycle observation for an embedding application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineV3ProgressState {
    Running,
    Validating,
    Complete,
    Reused,
    Invalidated,
    Failed,
    Cancelled,
}

/// Progress emitted after the durable state transition it describes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PipelineV3RunProgress {
    /// The durable workspace layout exists; later state initialization may still fail.
    Started {
        operation: PipelineV3RunOperation,
        run_directory: PathBuf,
        pipeline_name: String,
    },
    Stage {
        stage_id: String,
        state: PipelineV3ProgressState,
    },
}

#[derive(Debug, Clone)]
struct BoundInput {
    identity: PipelineInputIdentity,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct RuntimeArtifact {
    artifact_type: String,
    artifact_role: crate::provider::ArtifactRole,
    stream_role: Option<crate::provider::StreamRole>,
    kind: ArtifactKind,
    path: PathBuf,
    relative_path: Option<String>,
    source_identity: Option<PipelineInputIdentity>,
    observation: ArtifactContentObservation,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct StageInvocationIdentity<'a> {
    provider_invocation_schema: &'static str,
    execution_semantics: &'static str,
    execution_bounds: ProviderExecutionBounds,
    stage_plan_sha256: &'a str,
    provider_lock_sha256: &'a str,
    inputs: &'a [ArtifactEvidence],
    dependencies: Vec<DependencySemanticIdentity<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct DependencySemanticIdentity<'a> {
    stage_id: &'a str,
    stage_invocation_sha256: &'a str,
    outputs: &'a [ArtifactEvidence],
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ProviderExecutionEvidenceIdentity<'a> {
    stage_id: &'a str,
    stage_invocation_sha256: &'a str,
    report_sha256: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactIntegrityValidationReport {
    schema: String,
    validation_id: String,
    contract: String,
    artifact_id: String,
    artifact_sha256: String,
    accepted: bool,
}

const ARTIFACT_INTEGRITY_VALIDATION_REPORT_SCHEMA_V1: &str =
    "aniflow.artifact-integrity-validation/v1";

/// Execute an exact resolved Pipeline v3 plan without progress presentation.
pub fn run_v3(request: PipelineV3RunRequest) -> Result<PipelineV3RunOutcome> {
    run_v3_with_progress_and_cancellation(request, &CancellationToken::default(), |_| {})
}

/// Execute an exact resolved plan with durable state and observable progress.
pub fn run_v3_with_progress_and_cancellation<F>(
    request: PipelineV3RunRequest,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<PipelineV3RunOutcome>
where
    F: FnMut(&PipelineV3RunProgress),
{
    validate_execution_subset(&request.plan)?;
    ensure_no_duplicate_stage_ids(&request.plan)?;
    request.execution_limits.validate()?;
    let inputs = bind_inputs(&request.plan, &request.input_bindings)?;
    validate_workspace_parent_outside_inputs(request.output_directory.as_deref(), &inputs)?;
    let providers = resolve_exact_providers(&request.plan, &request.provider_registry)?;

    // Creation is intentionally after every configuration, input, provider,
    // shape, and validation-contract preflight above.
    let created = PipelineV3Workspace::create(
        request.output_directory.as_deref(),
        &request.plan.payload.pipeline_name,
    )?;
    let canonical_root = fs::canonicalize(created.root()).map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!("failed to resolve new Pipeline v3 workspace: {error}"),
        )
    })?;
    let workspace = PipelineV3Workspace::open_read_only(canonical_root)?;

    // Workspace creation and layout sync are the durable recovery boundary.
    // Later initialization can fail, so expose the run directory before
    // publishing the plan, locks, or first manifest revision.
    progress(&PipelineV3RunProgress::Started {
        operation: PipelineV3RunOperation::Run,
        run_directory: workspace.root().to_path_buf(),
        pipeline_name: request.plan.payload.pipeline_name.clone(),
    });

    workspace.publish_plan_bytes(&normalized_plan_bytes(&request.plan)?)?;
    publish_provider_locks(&workspace, &request.plan)?;

    let stages = request
        .plan
        .payload
        .stages
        .iter()
        .map(|stage| StageRunRecord {
            stage_id: stage.id.clone(),
            state: PipelineV3StageState::Pending,
            checkpoint: None,
            compatibility: None,
            message: None,
        })
        .collect();
    let mut manifest =
        PipelineV3RunManifest::new(workspace.run_id(), request.plan.plan_sha256.clone(), stages)?;
    append_run_manifest(&workspace, &manifest)?;

    execute_plan(
        &workspace,
        &request.plan,
        &inputs,
        &providers,
        request.execution_limits,
        false,
        &mut manifest,
        cancellation,
        &mut progress,
    )
}

/// Resume a Pipeline v3 run without progress presentation.
pub fn resume_v3(request: PipelineV3ResumeRequest) -> Result<PipelineV3RunOutcome> {
    resume_v3_with_progress_and_cancellation(request, &CancellationToken::default(), |_| {})
}

/// Revalidate persisted evidence and resume only the incompatible stage frontier.
pub fn resume_v3_with_progress_and_cancellation<F>(
    request: PipelineV3ResumeRequest,
    cancellation: &CancellationToken,
    mut progress: F,
) -> Result<PipelineV3RunOutcome>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let canonical_root = fs::canonicalize(&request.run_directory).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("failed to resolve Pipeline v3 run directory: {error}"),
        )
    })?;
    let workspace = PipelineV3Workspace::open_read_only(canonical_root)?;
    let plan = load_workspace_plan(&workspace)?;
    validate_execution_subset(&plan)?;
    ensure_no_duplicate_stage_ids(&plan)?;
    request.execution_limits.validate()?;
    let inputs = bind_inputs(&plan, &request.input_bindings)?;
    let providers = resolve_exact_providers(&plan, &request.provider_registry)?;
    let mut manifest = load_latest_run_manifest(&workspace)?;
    validate_manifest_authority(&workspace, &plan, &manifest)?;
    validate_provider_lock_evidence(&workspace, &plan)?;

    // Everything above is read-only preflight. Expose the recoverable run
    // directory before appending the first resume transition so a failure in
    // any subsequent mutation can still be reported with durable context.
    progress(&PipelineV3RunProgress::Started {
        operation: PipelineV3RunOperation::Resume,
        run_directory: workspace.root().to_path_buf(),
        pipeline_name: plan.payload.pipeline_name.clone(),
    });

    if manifest.payload.state == PipelineV3RunState::Running {
        let stages = manifest.payload.stages.clone();
        append_transition(
            &workspace,
            &mut manifest,
            PipelineV3RunState::Interrupted,
            &stages,
            Vec::new(),
            Some("resume observed a run without a terminal revision".to_owned()),
        )?;
    }

    execute_plan(
        &workspace,
        &plan,
        &inputs,
        &providers,
        request.execution_limits,
        true,
        &mut manifest,
        cancellation,
        &mut progress,
    )
}

/// Inspect the latest valid Pipeline v3 run-manifest revision without mutation.
pub fn status_v3(run_directory: impl AsRef<Path>) -> Result<PipelineV3RunManifest> {
    let canonical_root = fs::canonicalize(run_directory.as_ref()).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("failed to resolve Pipeline v3 run directory: {error}"),
        )
    })?;
    let workspace = PipelineV3Workspace::open_read_only(canonical_root)?;
    let plan = load_workspace_plan(&workspace)?;
    let manifest = load_latest_run_manifest(&workspace)?;
    validate_manifest_authority(&workspace, &plan, &manifest)?;
    validate_provider_lock_evidence(&workspace, &plan)?;
    for stage in &manifest.payload.stages {
        if let Some(reference) = &stage.checkpoint {
            load_stage_checkpoint(&workspace, reference)?;
        }
    }
    Ok(manifest)
}

fn load_workspace_plan(workspace: &PipelineV3Workspace) -> Result<PipelineV3Plan> {
    let metadata = fs::symlink_metadata(workspace.plan()).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("failed to inspect immutable Pipeline v3 plan: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::new(
            ErrorCategory::State,
            "immutable Pipeline v3 plan must be a regular file",
        ));
    }
    let bytes = fs::read(workspace.plan()).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("failed to read immutable Pipeline v3 plan: {error}"),
        )
    })?;
    PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| {
        Error::new(
            ErrorCategory::State,
            format!("invalid immutable Pipeline v3 plan: {failure}"),
        )
    })
}

fn provider_lock_path(workspace: &PipelineV3Workspace, stage: &ResolvedPipelineStage) -> PathBuf {
    workspace.providers().join(format!(
        "{}-{}.lock.json",
        stage.id, stage.provider_lock.lock_sha256
    ))
}

fn publish_provider_locks(workspace: &PipelineV3Workspace, plan: &PipelineV3Plan) -> Result<()> {
    for stage in &plan.payload.stages {
        workspace.publish_json_new(&provider_lock_path(workspace, stage), &stage.provider_lock)?;
    }
    Ok(())
}

fn validate_provider_lock_evidence(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
) -> Result<()> {
    for stage in &plan.payload.stages {
        let path = provider_lock_path(workspace, stage);
        let relative_path = workspace_relative_path(workspace, &path)?;
        let reference = EvidenceReference {
            relative_path,
            sha256: stage.provider_lock.lock_sha256.clone(),
        };
        let bytes = read_regular_workspace_evidence(workspace, &reference).map_err(|error| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "provider-lock evidence for stage {} is unavailable: {error}",
                    stage.id
                ),
            )
        })?;
        let provider_lock = ProviderLock::from_json_slice(&bytes).map_err(|error| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "provider-lock evidence for stage {} is invalid: {error}",
                    stage.id
                ),
            )
        })?;
        if provider_lock != stage.provider_lock {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "provider-lock evidence for stage {} differs from the immutable plan",
                    stage.id
                ),
            ));
        }
    }
    Ok(())
}

fn validate_manifest_authority(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
    manifest: &PipelineV3RunManifest,
) -> Result<()> {
    manifest.validate()?;
    if manifest.payload.run_id != workspace.run_id() {
        return Err(Error::new(
            ErrorCategory::State,
            "Pipeline v3 run manifest does not match its workspace directory",
        ));
    }
    if manifest.payload.plan_sha256 != plan.plan_sha256 {
        return Err(Error::new(
            ErrorCategory::State,
            "Pipeline v3 run manifest references a different immutable plan",
        ));
    }
    let planned = plan
        .payload
        .stages
        .iter()
        .map(|stage| stage.id.as_str())
        .collect::<Vec<_>>();
    let recorded = manifest
        .payload
        .stages
        .iter()
        .map(|stage| stage.stage_id.as_str())
        .collect::<Vec<_>>();
    if planned != recorded {
        return Err(Error::new(
            ErrorCategory::State,
            "Pipeline v3 run manifest stage order does not match its immutable plan",
        ));
    }
    Ok(())
}

struct ReusableCheckpoint {
    checkpoint: StageCheckpoint,
    reference: StageCheckpointReference,
    outputs: Vec<(String, RuntimeArtifact)>,
}

struct ProviderAttempt {
    candidate_root: PathBuf,
    stage_invocation_sha256: String,
    report: ProviderExecutionReport,
    report_reference: EvidenceReference,
}

#[allow(clippy::too_many_arguments)]
fn execute_plan<F>(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
    inputs: &BTreeMap<String, BoundInput>,
    providers: &BTreeMap<String, ResolvedProvider>,
    execution_limits: ProviderExecutionLimits,
    resume: bool,
    manifest: &mut PipelineV3RunManifest,
    cancellation: &CancellationToken,
    progress: &mut F,
) -> Result<PipelineV3RunOutcome>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let mut records = manifest.payload.stages.clone();
    let mut artifacts = initial_runtime_artifacts(inputs);
    let mut checkpoints = BTreeMap::<String, StageCheckpoint>::new();
    let mut invalidated = BTreeSet::<String>::new();
    let mut executed_stages = Vec::new();
    let mut reused_stages = Vec::new();

    for stage in &plan.payload.stages {
        if cancellation.is_cancelled() {
            append_transition(
                workspace,
                manifest,
                PipelineV3RunState::Cancelled,
                &records,
                Vec::new(),
                Some("cancellation requested before provider execution".to_owned()),
            )?;
            progress(&PipelineV3RunProgress::Stage {
                stage_id: stage.id.clone(),
                state: PipelineV3ProgressState::Cancelled,
            });
            return Err(Error::new(
                ErrorCategory::Execution,
                format!("Pipeline v3 stage {} was cancelled", stage.id),
            ));
        }

        if resume && !invalidated.contains(&stage.id) {
            match assess_checkpoint(
                workspace,
                plan,
                stage,
                &artifacts,
                &checkpoints,
                execution_limits.into(),
                record(&records, &stage.id)?.checkpoint.as_ref(),
            )? {
                Ok(reusable) => {
                    for (id, artifact) in reusable.outputs {
                        artifacts.insert(id, artifact);
                    }
                    checkpoints.insert(stage.id.clone(), reusable.checkpoint);
                    let stage_record = record_mut(&mut records, &stage.id)?;
                    stage_record.state = PipelineV3StageState::Complete;
                    stage_record.checkpoint = Some(reusable.reference);
                    stage_record.compatibility = Some(CompatibilityDecision::compatible());
                    stage_record.message = Some("reused compatible stage checkpoint".to_owned());
                    append_transition(
                        workspace,
                        manifest,
                        PipelineV3RunState::Running,
                        &records,
                        Vec::new(),
                        None,
                    )?;
                    progress(&PipelineV3RunProgress::Stage {
                        stage_id: stage.id.clone(),
                        state: PipelineV3ProgressState::Reused,
                    });
                    reused_stages.push(stage.id.clone());
                    continue;
                }
                Err(decision) => {
                    let frontier = invalidate_stage_frontier(
                        workspace,
                        plan,
                        manifest,
                        &mut records,
                        &stage.id,
                        decision,
                    )?;
                    for stage_id in frontier {
                        invalidated.insert(stage_id.clone());
                        progress(&PipelineV3RunProgress::Stage {
                            stage_id,
                            state: PipelineV3ProgressState::Invalidated,
                        });
                    }
                }
            }
        }

        let input_verification: Result<()> = (|| {
            for input in stage
                .inputs
                .iter()
                .flat_map(|binding| binding.artifacts.iter())
            {
                let artifact = artifacts.get(input).ok_or_else(|| {
                    Error::new(
                        ErrorCategory::State,
                        format!("stage {} input artifact {input} is unavailable", stage.id),
                    )
                })?;
                verify_runtime_artifact_cancellable(workspace, artifact, cancellation)?;
            }
            Ok(())
        })();
        if let Err(error) = input_verification {
            if cancellation.is_cancelled() {
                return Err(persist_stage_cancellation(
                    workspace,
                    manifest,
                    &mut records,
                    &stage.id,
                    "during input validation",
                    progress,
                )?);
            }
            fail_stage(
                workspace,
                manifest,
                &mut records,
                &stage.id,
                PipelineV3StageState::Failed,
                error.message(),
            )?;
            progress(&PipelineV3RunProgress::Stage {
                stage_id: stage.id.clone(),
                state: PipelineV3ProgressState::Failed,
            });
            return Err(error);
        }

        {
            let stage_record = record_mut(&mut records, &stage.id)?;
            stage_record.state = PipelineV3StageState::Running;
            stage_record.checkpoint = None;
            stage_record.compatibility = None;
            stage_record.message = None;
        }
        append_transition(
            workspace,
            manifest,
            PipelineV3RunState::Running,
            &records,
            Vec::new(),
            None,
        )?;
        progress(&PipelineV3RunProgress::Stage {
            stage_id: stage.id.clone(),
            state: PipelineV3ProgressState::Running,
        });

        let provider = providers.get(&stage.id).ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                format!("stage {} has no preflighted provider", stage.id),
            )
        })?;
        let attempt = match execute_provider_attempt(
            workspace,
            stage,
            provider,
            &artifacts,
            &checkpoints,
            execution_limits,
            manifest.payload.revision,
            cancellation,
        ) {
            Ok(attempt) => attempt,
            Err(error) => {
                if cancellation.is_cancelled() {
                    return Err(persist_stage_cancellation(
                        workspace,
                        manifest,
                        &mut records,
                        &stage.id,
                        "after provider execution",
                        progress,
                    )?);
                }
                fail_stage(
                    workspace,
                    manifest,
                    &mut records,
                    &stage.id,
                    PipelineV3StageState::Failed,
                    error.message(),
                )?;
                progress(&PipelineV3RunProgress::Stage {
                    stage_id: stage.id.clone(),
                    state: PipelineV3ProgressState::Failed,
                });
                return Err(error);
            }
        };

        if attempt.report.payload.outcome != ProviderExecutionOutcome::Succeeded {
            let cancelled = attempt.report.payload.outcome == ProviderExecutionOutcome::Cancelled;
            let detail = attempt
                .report
                .payload
                .failure
                .as_ref()
                .map_or("provider execution failed", |failure| {
                    failure.detail.as_str()
                });
            let state = if cancelled {
                PipelineV3StageState::Cancelled
            } else {
                PipelineV3StageState::Failed
            };
            fail_stage(workspace, manifest, &mut records, &stage.id, state, detail)?;
            progress(&PipelineV3RunProgress::Stage {
                stage_id: stage.id.clone(),
                state: if cancelled {
                    PipelineV3ProgressState::Cancelled
                } else {
                    PipelineV3ProgressState::Failed
                },
            });
            return Err(Error::new(
                ErrorCategory::Execution,
                format!("stage {} provider failed: {detail}", stage.id),
            ));
        }

        if cancellation.is_cancelled() {
            return Err(persist_stage_cancellation(
                workspace,
                manifest,
                &mut records,
                &stage.id,
                "before output acceptance",
                progress,
            )?);
        }

        {
            let stage_record = record_mut(&mut records, &stage.id)?;
            stage_record.state = PipelineV3StageState::Validating;
        }
        append_transition(
            workspace,
            manifest,
            PipelineV3RunState::Running,
            &records,
            Vec::new(),
            None,
        )?;
        progress(&PipelineV3RunProgress::Stage {
            stage_id: stage.id.clone(),
            state: PipelineV3ProgressState::Validating,
        });
        if cancellation.is_cancelled() {
            return Err(persist_stage_cancellation(
                workspace,
                manifest,
                &mut records,
                &stage.id,
                "during output validation",
                progress,
            )?);
        }

        let accepted = match accept_provider_outputs(
            workspace,
            plan,
            stage,
            &artifacts,
            &checkpoints,
            &attempt,
            cancellation,
        ) {
            Ok(accepted) => accepted,
            Err(error) => {
                if cancellation.is_cancelled() {
                    return Err(persist_stage_cancellation(
                        workspace,
                        manifest,
                        &mut records,
                        &stage.id,
                        "during output validation or publication",
                        progress,
                    )?);
                }
                fail_stage(
                    workspace,
                    manifest,
                    &mut records,
                    &stage.id,
                    PipelineV3StageState::Failed,
                    error.message(),
                )?;
                progress(&PipelineV3RunProgress::Stage {
                    stage_id: stage.id.clone(),
                    state: PipelineV3ProgressState::Failed,
                });
                return Err(error);
            }
        };

        if cancellation.is_cancelled() {
            return Err(persist_stage_cancellation(
                workspace,
                manifest,
                &mut records,
                &stage.id,
                "before checkpoint publication",
                progress,
            )?);
        }
        let (checkpoint, accepted_artifacts) = accepted;
        let checkpoint_reference = match publish_stage_checkpoint(workspace, &checkpoint) {
            Ok(reference) => reference,
            Err(error) => {
                if cancellation.is_cancelled() {
                    return Err(persist_stage_cancellation(
                        workspace,
                        manifest,
                        &mut records,
                        &stage.id,
                        "during checkpoint publication",
                        progress,
                    )?);
                }
                fail_stage(
                    workspace,
                    manifest,
                    &mut records,
                    &stage.id,
                    PipelineV3StageState::Failed,
                    error.message(),
                )?;
                progress(&PipelineV3RunProgress::Stage {
                    stage_id: stage.id.clone(),
                    state: PipelineV3ProgressState::Failed,
                });
                return Err(error);
            }
        };
        if cancellation.is_cancelled() {
            return Err(persist_stage_cancellation(
                workspace,
                manifest,
                &mut records,
                &stage.id,
                "during checkpoint publication",
                progress,
            )?);
        }
        {
            let stage_record = record_mut(&mut records, &stage.id)?;
            stage_record.state = PipelineV3StageState::Complete;
            stage_record.checkpoint = Some(checkpoint_reference);
            stage_record.compatibility = None;
            stage_record.message = None;
        }
        if cancellation.is_cancelled() {
            return Err(persist_stage_cancellation(
                workspace,
                manifest,
                &mut records,
                &stage.id,
                "before durable stage completion",
                progress,
            )?);
        }
        append_transition(
            workspace,
            manifest,
            PipelineV3RunState::Running,
            &records,
            Vec::new(),
            None,
        )?;
        for (id, artifact) in accepted_artifacts {
            artifacts.insert(id, artifact);
        }
        checkpoints.insert(stage.id.clone(), checkpoint);
        progress(&PipelineV3RunProgress::Stage {
            stage_id: stage.id.clone(),
            state: PipelineV3ProgressState::Complete,
        });
        executed_stages.push(stage.id.clone());
        if cancellation.is_cancelled() {
            return Err(persist_run_cancellation(
                workspace,
                manifest,
                &records,
                &format!(
                    "Pipeline v3 run was cancelled after stage {} completed",
                    stage.id
                ),
            )?);
        }
    }

    if cancellation.is_cancelled() {
        return Err(persist_run_cancellation(
            workspace,
            manifest,
            &records,
            "Pipeline v3 run was cancelled before final output validation",
        )?);
    }
    let (final_evidence, outputs) =
        match final_outputs(plan, workspace, &artifacts, &checkpoints, cancellation) {
            Ok(outputs) => outputs,
            Err(error) => {
                if cancellation.is_cancelled() {
                    return Err(persist_run_cancellation(
                        workspace,
                        manifest,
                        &records,
                        "Pipeline v3 run was cancelled during final output validation",
                    )?);
                }
                append_transition(
                    workspace,
                    manifest,
                    PipelineV3RunState::Failed,
                    &records,
                    Vec::new(),
                    Some(printable_message(error.message())),
                )?;
                return Err(error);
            }
        };
    if cancellation.is_cancelled() {
        return Err(persist_run_cancellation(
            workspace,
            manifest,
            &records,
            "Pipeline v3 run was cancelled before final completion",
        )?);
    }
    append_transition(
        workspace,
        manifest,
        PipelineV3RunState::Complete,
        &records,
        final_evidence,
        None,
    )?;
    if cancellation.is_cancelled() {
        return Err(persist_run_cancellation(
            workspace,
            manifest,
            &records,
            "Pipeline v3 run was cancelled during final completion",
        )?);
    }
    Ok(PipelineV3RunOutcome {
        schema: PIPELINE_V3_RUN_OUTCOME_SCHEMA_V1.to_owned(),
        run_directory: workspace.root().to_path_buf(),
        plan_sha256: plan.plan_sha256.clone(),
        run_manifest: workspace.manifest_revision(manifest.payload.revision),
        outputs,
        executed_stages,
        reused_stages,
    })
}

fn record<'a>(records: &'a [StageRunRecord], stage_id: &str) -> Result<&'a StageRunRecord> {
    records
        .iter()
        .find(|record| record.stage_id == stage_id)
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                format!("run manifest has no stage record for {stage_id}"),
            )
        })
}

fn record_mut<'a>(
    records: &'a mut [StageRunRecord],
    stage_id: &str,
) -> Result<&'a mut StageRunRecord> {
    records
        .iter_mut()
        .find(|record| record.stage_id == stage_id)
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                format!("run manifest has no stage record for {stage_id}"),
            )
        })
}

fn append_transition(
    workspace: &PipelineV3Workspace,
    manifest: &mut PipelineV3RunManifest,
    state: PipelineV3RunState,
    stages: &[StageRunRecord],
    outputs: Vec<ArtifactEvidence>,
    diagnostic: Option<String>,
) -> Result<()> {
    let next = manifest.next_revision(state, stages.to_vec(), outputs, diagnostic)?;
    append_run_manifest(workspace, &next)?;
    *manifest = next;
    Ok(())
}

fn invalidate_stage_frontier(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
    manifest: &mut PipelineV3RunManifest,
    records: &mut [StageRunRecord],
    stage_id: &str,
    decision: CompatibilityDecision,
) -> Result<Vec<String>> {
    let mut frontier = BTreeSet::from([stage_id.to_owned()]);
    loop {
        let previous_len = frontier.len();
        for stage in &plan.payload.stages {
            if stage
                .depends_on
                .iter()
                .any(|dependency| frontier.contains(dependency))
            {
                frontier.insert(stage.id.clone());
            }
        }
        if frontier.len() == previous_len {
            break;
        }
    }

    let dependency_decision = CompatibilityDecision::incompatible(vec![CompatibilityReason::new(
        CompatibilityReasonCode::DependencyChanged,
        "a declared dependency was invalidated in this resume",
    )?])?;
    let ordered_frontier = plan
        .payload
        .stages
        .iter()
        .filter(|stage| frontier.contains(&stage.id))
        .map(|stage| stage.id.clone())
        .collect::<Vec<_>>();
    if !frontier.contains(stage_id) || ordered_frontier.len() != frontier.len() {
        return Err(Error::new(
            ErrorCategory::State,
            format!("Pipeline v3 invalidation frontier contains unknown stage {stage_id}"),
        ));
    }
    for invalidated_stage_id in &ordered_frontier {
        let stage = record_mut(records, invalidated_stage_id)?;
        stage.state = PipelineV3StageState::Invalidated;
        stage.checkpoint = None;
        stage.compatibility = Some(if invalidated_stage_id == stage_id {
            decision.clone()
        } else {
            dependency_decision.clone()
        });
        stage.message = Some("checkpoint evidence is incompatible and will be rebuilt".to_owned());
        append_transition(
            workspace,
            manifest,
            PipelineV3RunState::Running,
            records,
            Vec::new(),
            None,
        )?;
    }
    Ok(ordered_frontier)
}

fn fail_stage(
    workspace: &PipelineV3Workspace,
    manifest: &mut PipelineV3RunManifest,
    records: &mut [StageRunRecord],
    stage_id: &str,
    stage_state: PipelineV3StageState,
    diagnostic: &str,
) -> Result<()> {
    let clean = printable_message(diagnostic);
    let stage = record_mut(records, stage_id)?;
    stage.state = stage_state;
    stage.checkpoint = None;
    stage.compatibility = None;
    stage.message = Some(clean.clone());
    append_transition(
        workspace,
        manifest,
        if stage_state == PipelineV3StageState::Cancelled {
            PipelineV3RunState::Cancelled
        } else {
            PipelineV3RunState::Failed
        },
        records,
        Vec::new(),
        Some(clean),
    )
}

fn stage_cancellation_error(stage_id: &str, phase: &str) -> Error {
    Error::new(
        ErrorCategory::Execution,
        format!("Pipeline v3 stage {stage_id} was cancelled {phase}"),
    )
}

fn ensure_stage_not_cancelled(
    cancellation: &CancellationToken,
    stage_id: &str,
    phase: &str,
) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(stage_cancellation_error(stage_id, phase));
    }
    Ok(())
}

fn ensure_execution_not_cancelled(cancellation: &CancellationToken, phase: &str) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(Error::new(
            ErrorCategory::Execution,
            format!("Pipeline v3 execution was cancelled {phase}"),
        ));
    }
    Ok(())
}

fn persist_stage_cancellation<F>(
    workspace: &PipelineV3Workspace,
    manifest: &mut PipelineV3RunManifest,
    records: &mut [StageRunRecord],
    stage_id: &str,
    phase: &str,
    progress: &mut F,
) -> Result<Error>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let error = stage_cancellation_error(stage_id, phase);
    fail_stage(
        workspace,
        manifest,
        records,
        stage_id,
        PipelineV3StageState::Cancelled,
        error.message(),
    )?;
    progress(&PipelineV3RunProgress::Stage {
        stage_id: stage_id.to_owned(),
        state: PipelineV3ProgressState::Cancelled,
    });
    Ok(error)
}

fn persist_run_cancellation(
    workspace: &PipelineV3Workspace,
    manifest: &mut PipelineV3RunManifest,
    records: &[StageRunRecord],
    diagnostic: &str,
) -> Result<Error> {
    let clean = printable_message(diagnostic);
    append_transition(
        workspace,
        manifest,
        PipelineV3RunState::Cancelled,
        records,
        Vec::new(),
        Some(clean.clone()),
    )?;
    Ok(Error::new(ErrorCategory::Execution, clean))
}

fn printable_message(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn execute_provider_attempt(
    workspace: &PipelineV3Workspace,
    stage: &ResolvedPipelineStage,
    provider: &ResolvedProvider,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
    checkpoints: &BTreeMap<String, StageCheckpoint>,
    limits: ProviderExecutionLimits,
    revision: u64,
    cancellation: &CancellationToken,
) -> Result<ProviderAttempt> {
    let input_evidence = stage_input_evidence(stage, artifacts)?;
    let stage_invocation_sha256 =
        stage_invocation_sha256(stage, &input_evidence, checkpoints, limits.into())?;
    let attempt_directory = create_attempt_directory(workspace, &stage.id, revision)?;
    let candidate_root = attempt_directory.join("candidate");
    let provider_work = attempt_directory.join("provider-work");
    fs::create_dir(&candidate_root).map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!("failed to create provider candidate directory: {error}"),
        )
    })?;
    fs::create_dir(&provider_work).map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!("failed to create provider working directory: {error}"),
        )
    })?;

    let invocation = ProviderInvocationRequest::new(
        stage.id.clone(),
        stage.provider_lock.lock_sha256.clone(),
        provider.registration().configuration().clone(),
        invocation_inputs(stage, artifacts)?,
        invocation_outputs(stage, &candidate_root)?,
    )?;
    let mut invocation_file = TempFileBuilder::new()
        .prefix("aniflow-provider-invocation-")
        .suffix(".json")
        .tempfile()
        .map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!("failed to create ephemeral provider invocation: {error}"),
            )
        })?;
    invocation_file
        .as_file_mut()
        .write_all(&invocation.to_json_bytes()?)
        .map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!("failed to write ephemeral provider invocation: {error}"),
            )
        })?;
    invocation_file.as_file_mut().sync_all().map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!("failed to sync ephemeral provider invocation: {error}"),
        )
    })?;
    let invocation_path = invocation_file.path().to_path_buf();
    let arguments = provider_invocation_arguments(&invocation_path)?;
    let mut sensitive_values = vec![
        workspace.root().to_string_lossy().into_owned(),
        invocation_path.to_string_lossy().into_owned(),
    ];
    for input in &invocation.inputs {
        sensitive_values.push(input.path.to_string_lossy().into_owned());
    }
    for value in provider.registration().configuration().values.values() {
        collect_sensitive_strings(value, &mut sensitive_values);
    }
    sensitive_values.retain(|value| !value.is_empty());
    sensitive_values
        .sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    sensitive_values.dedup();

    let execution_request = ProviderExecutionRequest {
        arguments,
        working_directory: provider_work,
        output_directory: candidate_root.clone(),
        expected_outputs: provider_expected_outputs(stage)?,
        limits,
        sensitive_values,
    };
    let report = provider
        .execute_v3_strict(&execution_request, cancellation, |_| {})
        .map_err(|error| {
            Error::new(
                ErrorCategory::Execution,
                format!("provider execution could not start or report safely: {error}"),
            )
        })?;
    drop(invocation_file);
    report.validate().map_err(|error| {
        Error::new(
            ErrorCategory::Execution,
            format!("provider execution report is invalid: {error}"),
        )
    })?;
    let report_relative_path =
        execution_report_relative_path(&stage.id, &stage_invocation_sha256, &report.report_sha256)?;
    let report_path = workspace.root().join(&report_relative_path);
    workspace.publish_json_new(&report_path, &report)?;
    let report_reference = EvidenceReference {
        relative_path: report_relative_path,
        sha256: report.report_sha256.clone(),
    };
    report_reference.validate()?;
    // A terminal provider report remains durable evidence even when the
    // cancellation token is set. Publish it before propagating cancellation;
    // the caller will mark the stage cancelled without accepting a checkpoint.
    ensure_stage_not_cancelled(cancellation, &stage.id, "after provider execution")?;
    Ok(ProviderAttempt {
        candidate_root,
        stage_invocation_sha256,
        report,
        report_reference,
    })
}

fn create_attempt_directory(
    workspace: &PipelineV3Workspace,
    stage_id: &str,
    revision: u64,
) -> Result<PathBuf> {
    let stage_directory = workspace.stage_work(stage_id);
    match fs::symlink_metadata(&stage_directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "Pipeline v3 stage work path is not a real directory: {}",
                    stage_directory.display()
                ),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&stage_directory).map_err(|error| {
                Error::new(
                    ErrorCategory::Io,
                    format!("failed to create stage work directory: {error}"),
                )
            })?;
        }
        Err(error) => {
            return Err(Error::new(
                ErrorCategory::Io,
                format!("failed to inspect stage work directory: {error}"),
            ));
        }
    }

    for offset in 0_u64..1_000 {
        let sequence = revision.checked_add(offset).ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                "Pipeline v3 attempt sequence overflow",
            )
        })?;
        let attempt = stage_directory.join(format!("attempt-{sequence:020}"));
        match fs::create_dir(&attempt) {
            Ok(()) => return Ok(attempt),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(Error::new(
                    ErrorCategory::Io,
                    format!("failed to create provider attempt directory: {error}"),
                ));
            }
        }
    }
    Err(Error::new(
        ErrorCategory::State,
        format!("could not allocate a unique attempt for stage {stage_id}"),
    ))
}

fn collect_sensitive_strings(value: &serde_json::Value, values: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => values.push(value.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_sensitive_strings(item, values);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values() {
                collect_sensitive_strings(value, values);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn workspace_relative_path(workspace: &PipelineV3Workspace, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(workspace.root()).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            format!(
                "evidence path escaped Pipeline v3 workspace: {}",
                path.display()
            ),
        )
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err(Error::new(
                ErrorCategory::State,
                "workspace evidence path is not normalized",
            ));
        };
        let part = part.to_str().ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                "workspace evidence path is not valid UTF-8",
            )
        })?;
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(Error::new(
            ErrorCategory::State,
            "workspace evidence path cannot identify the workspace root",
        ));
    }
    Ok(parts.join("/"))
}

fn stage_input_evidence(
    stage: &ResolvedPipelineStage,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
) -> Result<Vec<ArtifactEvidence>> {
    let mut evidence = Vec::new();
    for binding in &stage.inputs {
        for artifact_id in &binding.artifacts {
            let artifact = artifacts.get(artifact_id).ok_or_else(|| {
                Error::new(
                    ErrorCategory::State,
                    format!(
                        "stage {} input artifact {} has no accepted evidence",
                        stage.id, artifact_id
                    ),
                )
            })?;
            evidence.push(ArtifactEvidence {
                id: artifact_id.clone(),
                port: binding.port.clone(),
                relative_path: artifact.relative_path.clone(),
                artifact_type: artifact.artifact_type.clone(),
                artifact_role: artifact.artifact_role,
                stream_role: artifact.stream_role,
                kind: artifact.kind,
                file_count: artifact.observation.file_count,
                byte_count: artifact.observation.byte_count,
                sha256: artifact.observation.sha256.clone(),
            });
        }
    }
    Ok(evidence)
}

fn dependency_references(
    stage: &ResolvedPipelineStage,
    checkpoints: &BTreeMap<String, StageCheckpoint>,
) -> Result<Vec<StageCheckpointReference>> {
    stage
        .depends_on
        .iter()
        .map(|dependency| {
            checkpoints
                .get(dependency)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCategory::State,
                        format!(
                            "stage {} dependency {} has no accepted checkpoint",
                            stage.id, dependency
                        ),
                    )
                })?
                .reference()
        })
        .collect()
}

fn stage_invocation_sha256(
    stage: &ResolvedPipelineStage,
    inputs: &[ArtifactEvidence],
    checkpoints: &BTreeMap<String, StageCheckpoint>,
    execution_bounds: ProviderExecutionBounds,
) -> Result<String> {
    let stage_plan_sha256 = stage_plan_sha256(stage)?;
    let mut dependencies = Vec::with_capacity(stage.depends_on.len());
    for dependency in &stage.depends_on {
        let checkpoint = checkpoints.get(dependency).ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "stage {} dependency {} has no accepted checkpoint",
                    stage.id, dependency
                ),
            )
        })?;
        dependencies.push(DependencySemanticIdentity {
            stage_id: dependency,
            stage_invocation_sha256: &checkpoint.payload.stage_invocation_sha256,
            outputs: &checkpoint.payload.outputs,
        });
    }
    canonical_sha256(&StageInvocationIdentity {
        provider_invocation_schema: PROVIDER_INVOCATION_SCHEMA_V1,
        execution_semantics: PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1,
        execution_bounds,
        stage_plan_sha256: &stage_plan_sha256,
        provider_lock_sha256: &stage.provider_lock.lock_sha256,
        inputs,
        dependencies,
    })
}

fn execution_report_relative_path(
    stage_id: &str,
    stage_invocation_sha256: &str,
    report_sha256: &str,
) -> Result<String> {
    let binding_sha256 = canonical_sha256(&ProviderExecutionEvidenceIdentity {
        stage_id,
        stage_invocation_sha256,
        report_sha256,
    })?;
    Ok(format!("providers/{stage_id}-{binding_sha256}.report.json"))
}

fn accept_provider_outputs(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
    stage: &ResolvedPipelineStage,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
    checkpoints: &BTreeMap<String, StageCheckpoint>,
    attempt: &ProviderAttempt,
    cancellation: &CancellationToken,
) -> Result<(StageCheckpoint, Vec<(String, RuntimeArtifact)>)> {
    ensure_stage_not_cancelled(cancellation, &stage.id, "before output validation")?;
    let inputs = stage_input_evidence(stage, artifacts)?;
    let dependencies = dependency_references(stage, checkpoints)?;
    let invocation_sha256 =
        stage_invocation_sha256(stage, &inputs, checkpoints, attempt.report.payload.bounds)?;
    if attempt.stage_invocation_sha256 != invocation_sha256 {
        return Err(Error::new(
            ErrorCategory::State,
            format!(
                "stage {} execution report was published for a different invocation",
                stage.id
            ),
        ));
    }

    // The runtime already validated the complete candidate root. Reobserve
    // each declared output before any accepted artifact is replaced.
    let mut candidates = Vec::new();
    for (port, expected) in expected_artifacts(stage) {
        ensure_stage_not_cancelled(cancellation, &stage.id, "during output validation")?;
        let kind = expected_artifact_kind(expected)?;
        let candidate_path = attempt.candidate_root.join(&expected.relative_path);
        let observation =
            observe_existing_artifact_cancellable(&candidate_path, kind, cancellation).map_err(
                |failure| {
                    if failure.code == ProviderExecutionFailureCode::Cancelled {
                        stage_cancellation_error(&stage.id, "during output validation")
                    } else {
                        Error::new(
                            ErrorCategory::Execution,
                            format!(
                                "stage {} candidate output {} failed integrity validation: {}",
                                stage.id, expected.id, failure.detail
                            ),
                        )
                    }
                },
            )?;
        let report_observation = attempt
            .report
            .payload
            .outputs
            .iter()
            .find(|output| output.port == port)
            .ok_or_else(|| {
                Error::new(
                    ErrorCategory::Execution,
                    format!(
                        "stage {} execution report omitted output port {port}",
                        stage.id
                    ),
                )
            })?;
        if report_observation.relative_path != expected.relative_path
            || report_observation.kind != kind
            || report_observation.file_count != observation.file_count
            || report_observation.byte_count != observation.byte_count
            || report_observation.sha256 != observation.sha256
        {
            return Err(Error::new(
                ErrorCategory::Execution,
                format!(
                    "stage {} candidate output {} no longer matches its execution report",
                    stage.id, expected.id
                ),
            ));
        }
        candidates.push((port.to_owned(), expected, kind, candidate_path, observation));
    }

    // A provider has read authority only over its inputs. Verify it did not
    // alter any source or accepted upstream artifact before committing output.
    for binding in &stage.inputs {
        for artifact_id in &binding.artifacts {
            ensure_stage_not_cancelled(cancellation, &stage.id, "while revalidating stage inputs")?;
            verify_runtime_artifact_cancellable(
                workspace,
                artifacts.get(artifact_id).ok_or_else(|| {
                    Error::new(
                        ErrorCategory::State,
                        format!("stage {} lost input artifact {artifact_id}", stage.id),
                    )
                })?,
                cancellation,
            )?;
            ensure_stage_not_cancelled(cancellation, &stage.id, "while revalidating stage inputs")?;
        }
    }

    let mut accepted = Vec::new();
    let mut output_evidence = Vec::new();
    for (port, expected, kind, candidate_path, candidate_observation) in candidates {
        ensure_stage_not_cancelled(cancellation, &stage.id, "during artifact publication")?;
        let destination = workspace.root().join(&expected.relative_path);
        prepare_artifact_destination(workspace, &destination)?;
        ensure_stage_not_cancelled(cancellation, &stage.id, "during artifact publication")?;
        copy_artifact_to_fresh_inodes(&candidate_path, &destination, kind, cancellation, &stage.id)
            .map_err(|error| {
                Error::new(
                    error.category(),
                    format!(
                        "failed to publish stage {} artifact {}: {}",
                        stage.id,
                        expected.id,
                        error.message()
                    ),
                )
            })?;
        sync_artifact_content(&destination, cancellation, &stage.id)?;
        let observed = observe_existing_artifact_cancellable(&destination, kind, cancellation)
            .map_err(|failure| {
                if failure.code == ProviderExecutionFailureCode::Cancelled {
                    stage_cancellation_error(&stage.id, "during artifact publication")
                } else {
                    Error::new(
                        ErrorCategory::State,
                        format!(
                            "published stage {} artifact {} could not be revalidated: {}",
                            stage.id, expected.id, failure.detail
                        ),
                    )
                }
            })?;
        if observed != candidate_observation {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "published stage {} artifact {} changed during commit",
                    stage.id, expected.id
                ),
            ));
        }
        let evidence = ArtifactEvidence {
            id: expected.id.clone(),
            port,
            relative_path: Some(expected.relative_path.clone()),
            artifact_type: expected.artifact_type.clone(),
            artifact_role: expected.artifact_role,
            stream_role: expected.stream_role,
            kind,
            file_count: observed.file_count,
            byte_count: observed.byte_count,
            sha256: observed.sha256.clone(),
        };
        evidence.validate()?;
        output_evidence.push(evidence);
        accepted.push((
            expected.id.clone(),
            RuntimeArtifact {
                artifact_type: expected.artifact_type.clone(),
                artifact_role: expected.artifact_role,
                stream_role: expected.stream_role,
                kind,
                path: destination,
                relative_path: Some(expected.relative_path.clone()),
                source_identity: None,
                observation: observed,
            },
        ));
        remove_candidate_artifact(&candidate_path, kind, cancellation, &stage.id)?;
    }

    ensure_stage_not_cancelled(cancellation, &stage.id, "before validation publication")?;
    let validations =
        publish_validation_evidence(workspace, stage, &output_evidence, cancellation)?;
    ensure_stage_not_cancelled(cancellation, &stage.id, "before checkpoint construction")?;
    let checkpoint = StageCheckpoint::new(StageCheckpointPayload {
        stage_id: stage.id.clone(),
        plan_sha256: plan.plan_sha256.clone(),
        stage_invocation_sha256: invocation_sha256,
        provider_lock_sha256: stage.provider_lock.lock_sha256.clone(),
        dependencies,
        inputs,
        outputs: output_evidence,
        validations,
        execution_report: attempt.report_reference.clone(),
        compatibility_fingerprint: None,
        completed_at: Utc::now(),
    })?;
    Ok((checkpoint, accepted))
}

fn copy_artifact_to_fresh_inodes(
    source: &Path,
    destination: &Path,
    kind: ArtifactKind,
    cancellation: &CancellationToken,
    stage_id: &str,
) -> Result<()> {
    ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")?;
    verify_copy_source_kind(source, kind)?;
    match kind {
        ArtifactKind::File => copy_file_to_fresh_inode(source, destination, cancellation, stage_id),
        ArtifactKind::Directory => {
            fs::create_dir(destination).map_err(|error| {
                Error::new(
                    ErrorCategory::Io,
                    format!(
                        "failed to create accepted artifact directory {}: {error}",
                        destination.display()
                    ),
                )
            })?;
            for entry in WalkDir::new(source).min_depth(1).follow_links(false) {
                ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")?;
                let entry = entry.map_err(|error| {
                    Error::new(
                        ErrorCategory::Io,
                        format!("failed to traverse provider output during publication: {error}"),
                    )
                })?;
                let relative = entry.path().strip_prefix(source).map_err(|_| {
                    Error::new(
                        ErrorCategory::State,
                        "provider output entry escaped its candidate artifact",
                    )
                })?;
                let accepted_path = destination.join(relative);
                let file_type = entry.file_type();
                if file_type.is_symlink() {
                    return Err(Error::new(
                        ErrorCategory::State,
                        format!(
                            "provider output changed to a symbolic link during publication: {}",
                            entry.path().display()
                        ),
                    ));
                }
                if file_type.is_dir() {
                    verify_copy_source_kind(entry.path(), ArtifactKind::Directory)?;
                    fs::create_dir(&accepted_path).map_err(|error| {
                        Error::new(
                            ErrorCategory::Io,
                            format!(
                                "failed to create accepted artifact directory {}: {error}",
                                accepted_path.display()
                            ),
                        )
                    })?;
                } else if file_type.is_file() {
                    copy_file_to_fresh_inode(entry.path(), &accepted_path, cancellation, stage_id)?;
                } else {
                    return Err(Error::new(
                        ErrorCategory::State,
                        format!(
                            "provider output contains an unsupported filesystem entry: {}",
                            entry.path().display()
                        ),
                    ));
                }
            }
            ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")?;
            Ok(())
        }
    }
}

fn verify_copy_source_kind(path: &Path, kind: ArtifactKind) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!(
                "failed to inspect provider output before publication at {}: {error}",
                path.display()
            ),
        )
    })?;
    if metadata.file_type().is_symlink()
        || !match kind {
            ArtifactKind::File => metadata.is_file(),
            ArtifactKind::Directory => metadata.is_dir(),
        }
    {
        return Err(Error::new(
            ErrorCategory::State,
            format!(
                "provider output changed filesystem kind during publication at {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn copy_file_to_fresh_inode(
    source: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
    stage_id: &str,
) -> Result<()> {
    ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")?;
    verify_copy_source_kind(source, ArtifactKind::File)?;
    let mut source_file = fs::File::open(source).map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!(
                "failed to open provider output file {}: {error}",
                source.display()
            ),
        )
    })?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!(
                    "failed to create accepted artifact file {}: {error}",
                    destination.display()
                ),
            )
        })?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")?;
        let read = source_file.read(&mut buffer).map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!(
                    "failed to read provider output file {}: {error}",
                    source.display()
                ),
            )
        })?;
        if read == 0 {
            break;
        }
        destination_file
            .write_all(&buffer[..read])
            .map_err(|error| {
                Error::new(
                    ErrorCategory::Io,
                    format!(
                        "failed to copy provider output file {}: {error}",
                        source.display()
                    ),
                )
            })?;
    }
    ensure_stage_not_cancelled(cancellation, stage_id, "before artifact durability sync")?;
    destination_file.sync_all().map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!(
                "failed to sync accepted artifact file {}: {error}",
                destination.display()
            ),
        )
    })?;
    ensure_stage_not_cancelled(cancellation, stage_id, "after artifact durability sync")
}

fn remove_candidate_artifact(
    path: &Path,
    kind: ArtifactKind,
    cancellation: &CancellationToken,
    stage_id: &str,
) -> Result<()> {
    ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")?;
    verify_copy_source_kind(path, kind)?;
    let removal = match kind {
        ArtifactKind::File => fs::remove_file(path),
        ArtifactKind::Directory => fs::remove_dir_all(path),
    };
    removal.map_err(|error| {
        Error::new(
            ErrorCategory::Io,
            format!(
                "failed to remove provider candidate after publication at {}: {error}",
                path.display()
            ),
        )
    })?;
    ensure_stage_not_cancelled(cancellation, stage_id, "during artifact publication")
}

fn publish_validation_evidence(
    workspace: &PipelineV3Workspace,
    stage: &ResolvedPipelineStage,
    outputs: &[ArtifactEvidence],
    cancellation: &CancellationToken,
) -> Result<Vec<ValidationEvidence>> {
    let mut validations = Vec::with_capacity(stage.validations.len());
    for validation in &stage.validations {
        ensure_stage_not_cancelled(cancellation, &stage.id, "during validation publication")?;
        let artifact = outputs
            .iter()
            .find(|artifact| artifact.id == validation.artifact)
            .ok_or_else(|| {
                Error::new(
                    ErrorCategory::Configuration,
                    format!(
                        "stage {} validation {} references unavailable output {}",
                        stage.id, validation.id, validation.artifact
                    ),
                )
            })?;
        let report = ArtifactIntegrityValidationReport {
            schema: ARTIFACT_INTEGRITY_VALIDATION_REPORT_SCHEMA_V1.to_owned(),
            validation_id: validation.id.clone(),
            contract: validation.contract.clone(),
            artifact_id: artifact.id.clone(),
            artifact_sha256: artifact.sha256.clone(),
            accepted: true,
        };
        let report_sha256 = canonical_sha256(&report)?;
        let path = workspace.providers().join(format!(
            "{}-{}-{}.validation.json",
            stage.id, validation.id, report_sha256
        ));
        publish_content_addressed_json(workspace, &path, &report, &report_sha256)?;
        let evidence = EvidenceReference {
            relative_path: workspace_relative_path(workspace, &path)?,
            sha256: report_sha256,
        };
        evidence.validate()?;
        validations.push(ValidationEvidence {
            id: validation.id.clone(),
            contract: validation.contract.clone(),
            artifact_id: artifact.id.clone(),
            artifact_sha256: artifact.sha256.clone(),
            accepted: true,
            evidence,
        });
    }
    ensure_stage_not_cancelled(cancellation, &stage.id, "during validation publication")?;
    Ok(validations)
}

fn publish_content_addressed_json<T: Serialize>(
    workspace: &PipelineV3Workspace,
    path: &Path,
    value: &T,
    sha256: &str,
) -> Result<()> {
    match workspace.publish_json_new(path, value) {
        Ok(()) => Ok(()),
        Err(error) if error.category() == ErrorCategory::State => {
            let reference = EvidenceReference {
                relative_path: workspace_relative_path(workspace, path)?,
                sha256: sha256.to_owned(),
            };
            let actual = read_regular_workspace_evidence(workspace, &reference)?;
            let mut expected = serde_json::to_vec_pretty(value).map_err(|encode_error| {
                Error::new(
                    ErrorCategory::Internal,
                    format!("failed to encode content-addressed evidence: {encode_error}"),
                )
            })?;
            expected.push(b'\n');
            if actual == expected {
                Ok(())
            } else {
                Err(Error::new(
                    ErrorCategory::State,
                    format!(
                        "content-addressed evidence conflicts with existing state at {}",
                        path.display()
                    ),
                ))
            }
        }
        Err(error) => Err(error),
    }
}

fn prepare_artifact_destination(workspace: &PipelineV3Workspace, destination: &Path) -> Result<()> {
    let relative = destination.strip_prefix(workspace.root()).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            "artifact destination escaped Pipeline v3 workspace",
        )
    })?;
    if relative.components().next()
        != Some(std::path::Component::Normal(std::ffi::OsStr::new(
            "artifacts",
        )))
    {
        return Err(Error::new(
            ErrorCategory::State,
            "Pipeline v3 artifact destination must remain beneath artifacts/",
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        Error::new(
            ErrorCategory::State,
            "Pipeline v3 artifact destination has no parent",
        )
    })?;
    ensure_confined_directories(workspace.root(), parent)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(destination).map_err(|error| {
                Error::new(
                    ErrorCategory::Io,
                    format!("failed to remove incompatible artifact: {error}"),
                )
            })?;
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(destination).map_err(|error| {
                Error::new(
                    ErrorCategory::Io,
                    format!("failed to remove incompatible artifact directory: {error}"),
                )
            })?;
        }
        Ok(_) => {
            return Err(Error::new(
                ErrorCategory::State,
                "artifact destination has an unsupported filesystem type",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Error::new(
                ErrorCategory::Io,
                format!("failed to inspect artifact destination: {error}"),
            ));
        }
    }
    Ok(())
}

fn ensure_confined_directories(root: &Path, target: &Path) -> Result<()> {
    let relative = target.strip_prefix(root).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            "artifact parent escaped Pipeline v3 workspace",
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err(Error::new(
                ErrorCategory::State,
                "artifact parent path is not normalized",
            ));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(Error::new(
                    ErrorCategory::State,
                    format!(
                        "artifact parent is not a real directory: {}",
                        current.display()
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|error| {
                    Error::new(
                        ErrorCategory::Io,
                        format!("failed to create artifact parent: {error}"),
                    )
                })?;
            }
            Err(error) => {
                return Err(Error::new(
                    ErrorCategory::Io,
                    format!("failed to inspect artifact parent: {error}"),
                ));
            }
        }
    }
    Ok(())
}

fn sync_artifact_content(
    path: &Path,
    cancellation: &CancellationToken,
    stage_id: &str,
) -> Result<()> {
    ensure_stage_not_cancelled(cancellation, stage_id, "during artifact durability sync")?;
    let mut directories = Vec::new();
    for entry in WalkDir::new(path).follow_links(false) {
        ensure_stage_not_cancelled(cancellation, stage_id, "during artifact durability sync")?;
        let entry = entry.map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!("failed to traverse accepted artifact for durability: {error}"),
            )
        })?;
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            return Err(Error::new(
                ErrorCategory::State,
                "accepted artifact contains a symbolic link during durability sync",
            ));
        }
        if file_type.is_file() {
            fs::File::open(entry.path())
                .and_then(|file| file.sync_all())
                .map_err(|error| {
                    Error::new(
                        ErrorCategory::Io,
                        format!(
                            "failed to sync accepted artifact file {}: {error}",
                            entry.path().display()
                        ),
                    )
                })?;
        } else if file_type.is_dir() {
            directories.push((entry.depth(), entry.path().to_path_buf()));
        } else {
            return Err(Error::new(
                ErrorCategory::State,
                "accepted artifact contains an unsupported filesystem entry",
            ));
        }
    }
    directories.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    for (_, directory) in directories {
        ensure_stage_not_cancelled(cancellation, stage_id, "during artifact durability sync")?;
        sync_directory_for_durability(&directory)?;
    }
    ensure_stage_not_cancelled(cancellation, stage_id, "during artifact durability sync")?;
    sync_parent_directory(path)?;
    ensure_stage_not_cancelled(cancellation, stage_id, "after artifact durability sync")
}

#[cfg(unix)]
fn sync_directory_for_durability(path: &Path) -> Result<()> {
    use std::fs::OpenOptions;

    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!(
                    "failed to sync accepted artifact directory {}: {error}",
                    path.display()
                ),
            )
        })
}

#[cfg(not(unix))]
fn sync_directory_for_durability(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<()> {
    use std::fs::OpenOptions;

    let parent = path
        .parent()
        .ok_or_else(|| Error::new(ErrorCategory::State, "published artifact has no parent"))?;
    OpenOptions::new()
        .read(true)
        .open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            Error::new(
                ErrorCategory::Io,
                format!("failed to sync published artifact directory: {error}"),
            )
        })
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<()> {
    Ok(())
}

fn assess_checkpoint(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
    stage: &ResolvedPipelineStage,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
    checkpoints: &BTreeMap<String, StageCheckpoint>,
    execution_bounds: ProviderExecutionBounds,
    preferred: Option<&StageCheckpointReference>,
) -> Result<std::result::Result<ReusableCheckpoint, CompatibilityDecision>> {
    let (candidates, saw_corrupt) = checkpoint_candidates(workspace, &stage.id, preferred)?;
    if candidates.is_empty() {
        let reason = if saw_corrupt {
            CompatibilityReason::new(
                CompatibilityReasonCode::CorruptCheckpoint,
                "no valid immutable checkpoint could be loaded for this stage",
            )?
        } else {
            CompatibilityReason::new(
                CompatibilityReasonCode::MissingCheckpoint,
                "no immutable checkpoint exists for this stage",
            )?
        };
        return Ok(Err(CompatibilityDecision::incompatible(vec![reason])?));
    }

    let mut first_reasons = None;
    for (checkpoint, reference) in candidates {
        match assess_checkpoint_candidate(
            workspace,
            plan,
            stage,
            artifacts,
            checkpoints,
            execution_bounds,
            checkpoint,
            reference,
        )? {
            Ok(reusable) => return Ok(Ok(reusable)),
            Err(reasons) if first_reasons.is_none() => first_reasons = Some(reasons),
            Err(_) => {}
        }
    }
    let reasons = first_reasons.unwrap_or_else(|| {
        vec![CompatibilityReason {
            code: CompatibilityReasonCode::CorruptCheckpoint,
            detail: "no checkpoint candidate retained valid compatibility evidence".to_owned(),
        }]
    });
    Ok(Err(CompatibilityDecision::incompatible(reasons)?))
}

fn checkpoint_candidates(
    workspace: &PipelineV3Workspace,
    stage_id: &str,
    preferred: Option<&StageCheckpointReference>,
) -> Result<(Vec<(StageCheckpoint, StageCheckpointReference)>, bool)> {
    let mut candidates = Vec::new();
    let mut digests = BTreeSet::new();
    let mut saw_corrupt = false;
    if let Some(reference) = preferred {
        match load_stage_checkpoint(workspace, reference) {
            Ok(checkpoint) => {
                digests.insert(checkpoint.checkpoint_sha256.clone());
                candidates.push((checkpoint, reference.clone()));
            }
            Err(_) => saw_corrupt = true,
        }
    }

    let prefix = format!("{stage_id}-");
    for entry in fs::read_dir(workspace.checkpoints()).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("failed to inspect Pipeline v3 checkpoints: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            Error::new(
                ErrorCategory::State,
                format!("failed to inspect Pipeline v3 checkpoint entry: {error}"),
            )
        })?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".json") {
            continue;
        }
        match fs::symlink_metadata(entry.path()) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Ok(_) | Err(_) => {
                saw_corrupt = true;
                continue;
            }
        }
        let bytes = match fs::read(entry.path()) {
            Ok(bytes) => bytes,
            Err(_) => {
                saw_corrupt = true;
                continue;
            }
        };
        let checkpoint = match StageCheckpoint::from_json_slice(&bytes) {
            Ok(checkpoint) => checkpoint,
            Err(_) => {
                saw_corrupt = true;
                continue;
            }
        };
        if checkpoint.validate().is_err() || checkpoint.payload.stage_id != stage_id {
            saw_corrupt = true;
            continue;
        }
        let reference = checkpoint.reference()?;
        if workspace.root().join(&reference.relative_path) != entry.path() {
            saw_corrupt = true;
            continue;
        }
        if digests.insert(checkpoint.checkpoint_sha256.clone()) {
            candidates.push((checkpoint, reference));
        }
    }
    candidates.sort_by(|left, right| {
        right
            .0
            .payload
            .completed_at
            .cmp(&left.0.payload.completed_at)
            .then_with(|| right.0.checkpoint_sha256.cmp(&left.0.checkpoint_sha256))
    });
    Ok((candidates, saw_corrupt))
}

#[allow(clippy::too_many_arguments)]
fn assess_checkpoint_candidate(
    workspace: &PipelineV3Workspace,
    plan: &PipelineV3Plan,
    stage: &ResolvedPipelineStage,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
    checkpoints: &BTreeMap<String, StageCheckpoint>,
    execution_bounds: ProviderExecutionBounds,
    checkpoint: StageCheckpoint,
    reference: StageCheckpointReference,
) -> Result<std::result::Result<ReusableCheckpoint, Vec<CompatibilityReason>>> {
    let mut reasons = Vec::new();
    if checkpoint.payload.plan_sha256 != plan.plan_sha256 {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::StagePlanChanged,
            "checkpoint belongs to a different resolved plan",
        )?);
    }
    if checkpoint.payload.provider_lock_sha256 != stage.provider_lock.lock_sha256 {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ProviderLockChanged,
            "checkpoint provider lock differs from the resolved plan",
        )?);
    }

    let inputs = stage_input_evidence(stage, artifacts)?;
    if checkpoint.payload.inputs != inputs {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::InputChanged,
            "ordered stage input identities changed",
        )?);
    }

    let current_invocation =
        stage_invocation_sha256(stage, &inputs, checkpoints, execution_bounds)?;
    if checkpoint.payload.stage_invocation_sha256 != current_invocation {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ExecutionPolicyChanged,
            "stage invocation semantics or semantic dependencies changed",
        )?);
    }
    compare_dependency_evidence(workspace, stage, &checkpoint, checkpoints, &mut reasons)?;
    validate_execution_report_evidence(
        workspace,
        stage,
        execution_bounds,
        &current_invocation,
        &checkpoint,
        &mut reasons,
    )?;

    let mut runtime_outputs = Vec::new();
    let mut observed_evidence = Vec::new();
    for (port, expected) in expected_artifacts(stage) {
        let kind = expected_artifact_kind(expected)?;
        let path = workspace.root().join(&expected.relative_path);
        if validate_existing_confined_parent(workspace.root(), &path).is_err() {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::OutputChanged,
                format!("output {} has an unsafe parent path", expected.id),
            )?);
            continue;
        }
        match observe_existing_artifact(&path, kind) {
            Ok(observation) => {
                observed_evidence.push(ArtifactEvidence {
                    id: expected.id.clone(),
                    port: port.to_owned(),
                    relative_path: Some(expected.relative_path.clone()),
                    artifact_type: expected.artifact_type.clone(),
                    artifact_role: expected.artifact_role,
                    stream_role: expected.stream_role,
                    kind,
                    file_count: observation.file_count,
                    byte_count: observation.byte_count,
                    sha256: observation.sha256.clone(),
                });
                runtime_outputs.push((
                    expected.id.clone(),
                    RuntimeArtifact {
                        artifact_type: expected.artifact_type.clone(),
                        artifact_role: expected.artifact_role,
                        stream_role: expected.stream_role,
                        kind,
                        path,
                        relative_path: Some(expected.relative_path.clone()),
                        source_identity: None,
                        observation,
                    },
                ));
            }
            Err(failure) => {
                let code = if failure.code == ProviderExecutionFailureCode::MissingOutput {
                    CompatibilityReasonCode::OutputMissing
                } else {
                    CompatibilityReasonCode::OutputChanged
                };
                reasons.push(CompatibilityReason::new(
                    code,
                    format!(
                        "output {} could not be revalidated: {}",
                        expected.id, failure.detail
                    ),
                )?);
            }
        }
    }
    if checkpoint.payload.outputs != observed_evidence {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::OutputChanged,
            "checkpoint output metadata or content identity changed",
        )?);
    }
    validate_validation_evidence(
        workspace,
        stage,
        &checkpoint,
        &observed_evidence,
        &mut reasons,
    )?;

    if reasons.is_empty() {
        Ok(Ok(ReusableCheckpoint {
            checkpoint,
            reference,
            outputs: runtime_outputs,
        }))
    } else {
        Ok(Err(reasons))
    }
}

fn compare_dependency_evidence(
    workspace: &PipelineV3Workspace,
    stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint,
    current: &BTreeMap<String, StageCheckpoint>,
    reasons: &mut Vec<CompatibilityReason>,
) -> Result<()> {
    let recorded_ids = checkpoint
        .payload
        .dependencies
        .iter()
        .map(|dependency| dependency.stage_id.as_str())
        .collect::<Vec<_>>();
    let expected_ids = stage
        .depends_on
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if recorded_ids != expected_ids {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::DependencyChanged,
            "ordered dependency checkpoint set changed",
        )?);
        return Ok(());
    }
    for reference in &checkpoint.payload.dependencies {
        let Some(current_checkpoint) = current.get(&reference.stage_id) else {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::DependencyChanged,
                format!(
                    "dependency {} has no currently accepted checkpoint",
                    reference.stage_id
                ),
            )?);
            continue;
        };
        let recorded_checkpoint = match load_stage_checkpoint(workspace, reference) {
            Ok(checkpoint) => checkpoint,
            Err(_) => {
                reasons.push(CompatibilityReason::new(
                    CompatibilityReasonCode::DependencyChanged,
                    format!(
                        "recorded dependency checkpoint {} is unavailable or corrupt",
                        reference.stage_id
                    ),
                )?);
                continue;
            }
        };
        if recorded_checkpoint.payload.stage_invocation_sha256
            != current_checkpoint.payload.stage_invocation_sha256
            || recorded_checkpoint.payload.outputs != current_checkpoint.payload.outputs
        {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::DependencyChanged,
                format!(
                    "dependency {} semantic identity changed",
                    reference.stage_id
                ),
            )?);
        }
    }
    Ok(())
}

fn validate_execution_report_evidence(
    workspace: &PipelineV3Workspace,
    stage: &ResolvedPipelineStage,
    execution_bounds: ProviderExecutionBounds,
    current_invocation: &str,
    checkpoint: &StageCheckpoint,
    reasons: &mut Vec<CompatibilityReason>,
) -> Result<()> {
    let reference = &checkpoint.payload.execution_report;
    let expected_relative_path =
        execution_report_relative_path(&stage.id, current_invocation, &reference.sha256)?;
    if reference.relative_path != expected_relative_path {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ExecutionReportInvalid,
            "execution report locator is not bound to this exact stage invocation",
        )?);
    }
    let bytes = match read_regular_workspace_evidence(workspace, reference) {
        Ok(bytes) => bytes,
        Err(_) => {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::ExecutionReportMissing,
                "provider execution report is unavailable",
            )?);
            return Ok(());
        }
    };
    let report = match ProviderExecutionReport::from_json_slice(&bytes) {
        Ok(report) => report,
        Err(_) => {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::ExecutionReportInvalid,
                "provider execution report is invalid",
            )?);
            return Ok(());
        }
    };
    if report.report_sha256 != reference.sha256
        || report.payload.provider_lock != stage.provider_lock
        || report.payload.outcome != ProviderExecutionOutcome::Succeeded
    {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ExecutionReportInvalid,
            "provider execution report does not prove this successful exact-lock invocation",
        )?);
    }
    if report.payload.bounds != execution_bounds {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ExecutionPolicyChanged,
            "provider execution bounds differ from the current execution policy",
        )?);
    }
    let mut checkpoint_outputs = checkpoint
        .payload
        .outputs
        .iter()
        .map(|output| {
            Ok(ArtifactObservation {
                port: output.port.clone(),
                relative_path: output.relative_path.clone().ok_or_else(|| {
                    Error::new(
                        ErrorCategory::State,
                        format!(
                            "stage {} checkpoint output {} has no workspace path",
                            stage.id, output.id
                        ),
                    )
                })?,
                kind: output.kind,
                file_count: output.file_count,
                byte_count: output.byte_count,
                sha256: output.sha256.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    checkpoint_outputs.sort_by(|left, right| left.port.cmp(&right.port));
    if report.payload.outputs != checkpoint_outputs {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ExecutionReportInvalid,
            "provider execution report outputs do not match the accepted checkpoint outputs",
        )?);
    }
    Ok(())
}

fn validate_validation_evidence(
    workspace: &PipelineV3Workspace,
    stage: &ResolvedPipelineStage,
    checkpoint: &StageCheckpoint,
    outputs: &[ArtifactEvidence],
    reasons: &mut Vec<CompatibilityReason>,
) -> Result<()> {
    if checkpoint.payload.validations.len() != stage.validations.len() {
        reasons.push(CompatibilityReason::new(
            CompatibilityReasonCode::ValidationMissing,
            "declared validation evidence set is incomplete",
        )?);
    }
    for expected in &stage.validations {
        let Some(validation) = checkpoint
            .payload
            .validations
            .iter()
            .find(|validation| validation.id == expected.id)
        else {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::ValidationMissing,
                format!("validation {} has no evidence", expected.id),
            )?);
            continue;
        };
        let output = outputs.iter().find(|output| output.id == expected.artifact);
        if validation.contract != expected.contract
            || validation.artifact_id != expected.artifact
            || !validation.accepted
            || output.is_none_or(|output| output.sha256 != validation.artifact_sha256)
        {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::ValidationChanged,
                format!(
                    "validation {} no longer identifies the accepted artifact",
                    expected.id
                ),
            )?);
            continue;
        }
        if !validation.evidence.relative_path.starts_with("providers/") {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::ValidationChanged,
                format!("validation {} evidence escaped providers/", expected.id),
            )?);
            continue;
        }
        let bytes = match read_regular_workspace_evidence(workspace, &validation.evidence) {
            Ok(bytes) => bytes,
            Err(_) => {
                reasons.push(CompatibilityReason::new(
                    CompatibilityReasonCode::ValidationMissing,
                    format!("validation {} evidence is unavailable", expected.id),
                )?);
                continue;
            }
        };
        let report: ArtifactIntegrityValidationReport = match serde_json::from_slice(&bytes) {
            Ok(report) => report,
            Err(_) => {
                reasons.push(CompatibilityReason::new(
                    CompatibilityReasonCode::ValidationChanged,
                    format!("validation {} evidence is invalid", expected.id),
                )?);
                continue;
            }
        };
        let digest = canonical_sha256(&report)?;
        if report.schema != ARTIFACT_INTEGRITY_VALIDATION_REPORT_SCHEMA_V1
            || report.validation_id != validation.id
            || report.contract != validation.contract
            || report.artifact_id != validation.artifact_id
            || report.artifact_sha256 != validation.artifact_sha256
            || !report.accepted
            || digest != validation.evidence.sha256
        {
            reasons.push(CompatibilityReason::new(
                CompatibilityReasonCode::ValidationChanged,
                format!("validation {} implementation evidence changed", expected.id),
            )?);
        }
    }
    Ok(())
}

fn read_regular_workspace_evidence(
    workspace: &PipelineV3Workspace,
    reference: &EvidenceReference,
) -> Result<Vec<u8>> {
    reference.validate()?;
    let path = workspace.root().join(&reference.relative_path);
    validate_existing_confined_parent(workspace.root(), &path)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("workspace evidence is unavailable: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::new(
            ErrorCategory::State,
            "workspace evidence must be a regular file",
        ));
    }
    fs::read(&path).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("workspace evidence could not be read: {error}"),
        )
    })
}

fn final_outputs(
    plan: &PipelineV3Plan,
    workspace: &PipelineV3Workspace,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
    checkpoints: &BTreeMap<String, StageCheckpoint>,
    cancellation: &CancellationToken,
) -> Result<(Vec<ArtifactEvidence>, Vec<PipelineV3Output>)> {
    ensure_execution_not_cancelled(cancellation, "before final output validation")?;
    let validations = checkpoints
        .values()
        .flat_map(|checkpoint| checkpoint.payload.validations.iter())
        .map(|validation| (validation.id.as_str(), validation))
        .collect::<BTreeMap<_, _>>();
    let mut evidence = Vec::with_capacity(plan.payload.outputs.len());
    let mut outputs = Vec::with_capacity(plan.payload.outputs.len());
    for output in &plan.payload.outputs {
        ensure_execution_not_cancelled(cancellation, "during final output validation")?;
        let artifact = artifacts.get(&output.artifact).ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "final output {} references unavailable artifact {}",
                    output.id, output.artifact
                ),
            )
        })?;
        verify_runtime_artifact_cancellable(workspace, artifact, cancellation)?;
        for validation_id in &output.required_validations {
            let validation = validations.get(validation_id.as_str()).ok_or_else(|| {
                Error::new(
                    ErrorCategory::State,
                    format!(
                        "final output {} is missing required validation {}",
                        output.id, validation_id
                    ),
                )
            })?;
            if !validation.accepted
                || validation.artifact_id != output.artifact
                || validation.artifact_sha256 != artifact.observation.sha256
            {
                return Err(Error::new(
                    ErrorCategory::State,
                    format!(
                        "final output {} required validation {} is incompatible",
                        output.id, validation_id
                    ),
                ));
            }
        }
        let relative_path = artifact.relative_path.clone().ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                format!("final output {} cannot expose a source input", output.id),
            )
        })?;
        evidence.push(ArtifactEvidence {
            id: output.id.clone(),
            port: "final".to_owned(),
            relative_path: Some(relative_path),
            artifact_type: artifact.artifact_type.clone(),
            artifact_role: artifact.artifact_role,
            stream_role: artifact.stream_role,
            kind: artifact.kind,
            file_count: artifact.observation.file_count,
            byte_count: artifact.observation.byte_count,
            sha256: artifact.observation.sha256.clone(),
        });
        outputs.push(PipelineV3Output {
            id: output.id.clone(),
            artifact: output.artifact.clone(),
            path: artifact.path.clone(),
            sha256: artifact.observation.sha256.clone(),
        });
    }
    for item in &evidence {
        ensure_execution_not_cancelled(cancellation, "during final output validation")?;
        item.validate()?;
    }
    ensure_execution_not_cancelled(cancellation, "after final output validation")?;
    Ok((evidence, outputs))
}

fn validate_execution_subset(plan: &PipelineV3Plan) -> Result<()> {
    plan.validate().map_err(|failure| {
        Error::new(
            ErrorCategory::Configuration,
            format!("invalid Pipeline v3 plan: {failure}"),
        )
    })?;

    for stage in &plan.payload.stages {
        if stage.capability.kind == CapabilityKind::LifecycleObserver {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!(
                    "stage {} uses lifecycle-observer semantics, which Pipeline v3 execution does not yet support",
                    stage.id
                ),
            ));
        }
        if stage
            .capability
            .behavior
            .side_effects
            .contains(&SideEffect::Publish)
        {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!(
                    "stage {} requests publish authority, which cannot be safely replayed by this executor",
                    stage.id
                ),
            ));
        }
        if stage.outputs.is_empty() {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!("stage {} must declare at least one output", stage.id),
            ));
        }
        if stage.inputs.is_empty() {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!(
                    "stage {} must declare at least one input for provider-invocation v1",
                    stage.id
                ),
            ));
        }
        for port in &stage.capability.outputs {
            let Some(output) = stage.outputs.iter().find(|output| output.port == port.name) else {
                return Err(Error::new(
                    ErrorCategory::Configuration,
                    format!(
                        "stage {} capability output port {} is not bound; Pipeline v3 execution requires exactly one artifact for every output port",
                        stage.id, port.name
                    ),
                ));
            };
            if port.cardinality != ArtifactCardinality::One || output.artifacts.len() != 1 {
                return Err(Error::new(
                    ErrorCategory::Configuration,
                    format!(
                        "stage {} output port {} must use the executable v1 subset: cardinality one with exactly one artifact",
                        stage.id, output.port
                    ),
                ));
            }
            let artifact = &output.artifacts[0];
            if artifact.kind.is_none() {
                return Err(Error::new(
                    ErrorCategory::Configuration,
                    format!(
                        "stage {} output {} must declare an explicit file or directory kind for Pipeline v3 execution",
                        stage.id, artifact.id
                    ),
                ));
            }
            if !artifact.relative_path.starts_with("artifacts/") {
                return Err(Error::new(
                    ErrorCategory::Configuration,
                    format!(
                        "stage {} output {} must remain beneath artifacts/",
                        stage.id, artifact.id
                    ),
                ));
            }
        }
        for validation in &stage.validations {
            if validation.contract != ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1 {
                return Err(Error::new(
                    ErrorCategory::Configuration,
                    format!(
                        "stage {} validation {} uses unsupported contract {}; expected {}",
                        stage.id,
                        validation.id,
                        validation.contract,
                        ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn bind_inputs(
    plan: &PipelineV3Plan,
    bindings: &[PipelineInputBinding],
) -> Result<BTreeMap<String, BoundInput>> {
    let planned = plan
        .payload
        .inputs
        .iter()
        .map(|input| (input.id.as_str(), input))
        .collect::<BTreeMap<_, _>>();
    let mut bound = BTreeMap::new();
    for binding in bindings {
        let Some(expected) = planned.get(binding.artifact_id.as_str()) else {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "input binding {} is not part of the immutable Pipeline v3 plan",
                    binding.artifact_id
                ),
            ));
        };
        if bound.contains_key(&binding.artifact_id) {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!("input {} is bound more than once", binding.artifact_id),
            ));
        }
        let observed = reobserve_pipeline_input(expected, &binding.path).map_err(|failure| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "could not revalidate immutable input {}: {failure}",
                    binding.artifact_id
                ),
            )
        })?;
        if &observed != *expected {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "immutable input {} no longer matches the resolved plan",
                    binding.artifact_id
                ),
            ));
        }
        let path = fs::canonicalize(&binding.path).map_err(|error| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "could not resolve immutable input {}: {error}",
                    binding.artifact_id
                ),
            )
        })?;
        bound.insert(
            binding.artifact_id.clone(),
            BoundInput {
                identity: observed,
                path,
            },
        );
    }
    if bound.len() != planned.len() {
        let missing = planned
            .keys()
            .filter(|id| !bound.contains_key(**id))
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::new(
            ErrorCategory::State,
            format!("missing immutable Pipeline v3 input bindings: {missing}"),
        ));
    }
    Ok(bound)
}

fn validate_workspace_parent_outside_inputs(
    output_directory: Option<&Path>,
    inputs: &BTreeMap<String, BoundInput>,
) -> Result<()> {
    let requested = output_directory.unwrap_or_else(|| Path::new(".aniflow/runs"));
    let resolved = resolve_future_path(requested)?;
    for input in inputs.values() {
        if input.identity.kind == PipelineInputKind::Directory
            && (resolved == input.path || resolved.starts_with(&input.path))
        {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!(
                    "Pipeline v3 run parent {} must not be inside immutable directory input {}",
                    requested.display(),
                    input.identity.id
                ),
            ));
        }
    }
    Ok(())
}

fn resolve_future_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                Error::new(
                    ErrorCategory::Io,
                    format!("failed to resolve the current directory: {error}"),
                )
            })?
            .join(path)
    };
    let normalized = normalize_absolute_path(&absolute)?;
    let mut existing = normalized.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(existing).map_err(|error| {
                    Error::new(
                        ErrorCategory::Io,
                        format!(
                            "failed to resolve Pipeline v3 run parent {}: {error}",
                            path.display()
                        ),
                    )
                })?;
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = existing.file_name().ok_or_else(|| {
                    Error::new(
                        ErrorCategory::Io,
                        format!(
                            "Pipeline v3 run parent has no resolvable ancestor: {}",
                            path.display()
                        ),
                    )
                })?;
                missing.push(name.to_os_string());
                existing = existing.parent().ok_or_else(|| {
                    Error::new(
                        ErrorCategory::Io,
                        format!(
                            "Pipeline v3 run parent has no resolvable ancestor: {}",
                            path.display()
                        ),
                    )
                })?;
            }
            Err(error) => {
                return Err(Error::new(
                    ErrorCategory::Io,
                    format!(
                        "failed to inspect Pipeline v3 run parent {}: {error}",
                        path.display()
                    ),
                ));
            }
        }
    }
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(Error::new(
                        ErrorCategory::Configuration,
                        format!("path escapes the filesystem root: {}", path.display()),
                    ));
                }
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn resolve_exact_providers(
    plan: &PipelineV3Plan,
    registry: &ProviderRegistry,
) -> Result<BTreeMap<String, ResolvedProvider>> {
    let mut resolved = BTreeMap::new();
    for stage in &plan.payload.stages {
        let registration_id = &stage.provider_lock.payload.registration_id;
        let registration = registry.registration(registration_id).ok_or_else(|| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "stage {} requires explicit provider registration {}",
                    stage.id, registration_id
                ),
            )
        })?;
        let mut exact_registry = ProviderRegistry::new();
        exact_registry
            .register(registration.clone())
            .map_err(|error| {
                Error::new(
                    ErrorCategory::State,
                    format!(
                        "could not establish exact provider authority for stage {}: {error}",
                        stage.id
                    ),
                )
            })?;
        let request = ProviderResolutionRequest {
            capability_id: stage.capability_requirement.id.clone(),
            capability_version_requirement: stage
                .capability_requirement
                .version_requirement
                .clone(),
            replacement: stage.provider_selection.replacement.clone(),
            primary: stage.provider_selection.primary.clone(),
            fallbacks: stage.provider_selection.fallbacks.clone(),
            allowed_side_effects: plan.payload.policy.allowed_side_effects.clone(),
            offline: plan.payload.policy.offline,
            host: plan.payload.policy.host.clone(),
        };
        let provider = exact_registry.resolve(&request).map_err(|failure| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "could not re-resolve exact provider for stage {}: {}",
                    stage.id, failure.message
                ),
            )
        })?;
        if provider.provider_lock() != &stage.provider_lock {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "provider authority for stage {} no longer matches lock {}",
                    stage.id, stage.provider_lock.lock_sha256
                ),
            ));
        }
        resolved.insert(stage.id.clone(), provider);
    }
    Ok(resolved)
}

fn artifact_kind(kind: PipelineInputKind) -> ArtifactKind {
    match kind {
        PipelineInputKind::File => ArtifactKind::File,
        PipelineInputKind::Directory => ArtifactKind::Directory,
    }
}

fn expected_artifact_kind(artifact: &PlannedExpectedArtifact) -> Result<ArtifactKind> {
    artifact.kind.map(artifact_kind).ok_or_else(|| {
        Error::new(
            ErrorCategory::Configuration,
            format!(
                "output {} must declare an explicit file or directory kind for Pipeline v3 execution",
                artifact.id
            ),
        )
    })
}

fn initial_runtime_artifacts(
    inputs: &BTreeMap<String, BoundInput>,
) -> BTreeMap<String, RuntimeArtifact> {
    inputs
        .iter()
        .map(|(id, input)| {
            (
                id.clone(),
                RuntimeArtifact {
                    artifact_type: input.identity.artifact_type.clone(),
                    artifact_role: input.identity.artifact_role,
                    stream_role: input.identity.stream_role,
                    kind: artifact_kind(input.identity.kind),
                    path: input.path.clone(),
                    relative_path: None,
                    source_identity: Some(input.identity.clone()),
                    observation: ArtifactContentObservation {
                        kind: artifact_kind(input.identity.kind),
                        file_count: input.identity.file_count,
                        byte_count: input.identity.byte_count,
                        sha256: input.identity.content_sha256.clone(),
                    },
                },
            )
        })
        .collect()
}

fn stage_plan_sha256(stage: &ResolvedPipelineStage) -> Result<String> {
    canonical_sha256(stage).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!(
                "failed to identify stage {} plan fragment: {error}",
                stage.id
            ),
        )
    })
}

fn expected_artifacts(
    stage: &ResolvedPipelineStage,
) -> impl Iterator<Item = (&str, &PlannedExpectedArtifact)> {
    stage.outputs.iter().flat_map(|binding| {
        binding
            .artifacts
            .iter()
            .map(move |artifact| (binding.port.as_str(), artifact))
    })
}

fn invocation_inputs(
    stage: &ResolvedPipelineStage,
    artifacts: &BTreeMap<String, RuntimeArtifact>,
) -> Result<Vec<ProviderInvocationArtifactBinding>> {
    let mut bindings = Vec::new();
    for port in &stage.inputs {
        for artifact_id in &port.artifacts {
            let artifact = artifacts.get(artifact_id).ok_or_else(|| {
                Error::new(
                    ErrorCategory::State,
                    format!(
                        "stage {} input artifact {} has no accepted identity",
                        stage.id, artifact_id
                    ),
                )
            })?;
            bindings.push(ProviderInvocationArtifactBinding::new(
                port.port.clone(),
                artifact_id.clone(),
                artifact.artifact_type.clone(),
                artifact.artifact_role,
                artifact.stream_role,
                artifact.kind,
                artifact.path.clone(),
            )?);
        }
    }
    Ok(bindings)
}

fn invocation_outputs(
    stage: &ResolvedPipelineStage,
    candidate_root: &Path,
) -> Result<Vec<ProviderInvocationArtifactBinding>> {
    expected_artifacts(stage)
        .map(|(port, artifact)| {
            ProviderInvocationArtifactBinding::new(
                port,
                artifact.id.clone(),
                artifact.artifact_type.clone(),
                artifact.artifact_role,
                artifact.stream_role,
                expected_artifact_kind(artifact)?,
                candidate_root.join(&artifact.relative_path),
            )
        })
        .collect()
}

fn provider_expected_outputs(stage: &ResolvedPipelineStage) -> Result<Vec<ExpectedProviderOutput>> {
    expected_artifacts(stage)
        .map(|(port, artifact)| {
            Ok(ExpectedProviderOutput {
                port: port.to_owned(),
                relative_path: PathBuf::from(&artifact.relative_path),
                kind: expected_artifact_kind(artifact)?,
            })
        })
        .collect()
}

fn verify_runtime_artifact_cancellable(
    workspace: &PipelineV3Workspace,
    artifact: &RuntimeArtifact,
    cancellation: &CancellationToken,
) -> Result<()> {
    verify_runtime_artifact_inner(workspace, artifact, Some(cancellation))
}

fn verify_runtime_artifact_inner(
    workspace: &PipelineV3Workspace,
    artifact: &RuntimeArtifact,
    cancellation: Option<&CancellationToken>,
) -> Result<()> {
    if let Some(cancellation) = cancellation {
        ensure_execution_not_cancelled(cancellation, "during artifact validation")?;
    }
    if let Some(identity) = &artifact.source_identity {
        let observed = reobserve_pipeline_input(identity, &artifact.path).map_err(|failure| {
            Error::new(
                ErrorCategory::State,
                format!(
                    "immutable input {} could not be revalidated: {failure}",
                    identity.id
                ),
            )
        })?;
        if &observed != identity {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "immutable input {} changed during Pipeline v3 execution",
                    identity.id
                ),
            ));
        }
        if let Some(cancellation) = cancellation {
            ensure_execution_not_cancelled(cancellation, "during artifact validation")?;
        }
        return Ok(());
    }
    if artifact.relative_path.is_some() {
        validate_existing_confined_parent(workspace.root(), &artifact.path)?;
    }
    let observed = match cancellation {
        Some(cancellation) => {
            observe_existing_artifact_cancellable(&artifact.path, artifact.kind, cancellation)
        }
        None => observe_existing_artifact(&artifact.path, artifact.kind),
    }
    .map_err(|failure| {
        if failure.code == ProviderExecutionFailureCode::Cancelled {
            Error::new(
                ErrorCategory::Execution,
                "Pipeline v3 execution was cancelled during artifact validation",
            )
        } else {
            Error::new(
                ErrorCategory::State,
                format!(
                    "accepted artifact could not be revalidated: {}",
                    failure.detail
                ),
            )
        }
    })?;
    if observed != artifact.observation {
        return Err(Error::new(
            ErrorCategory::State,
            "accepted artifact content changed during Pipeline v3 execution",
        ));
    }
    if let Some(cancellation) = cancellation {
        ensure_execution_not_cancelled(cancellation, "during artifact validation")?;
    }
    Ok(())
}

fn validate_existing_confined_parent(root: &Path, path: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        Error::new(
            ErrorCategory::State,
            "artifact path has no parent directory",
        )
    })?;
    let relative = parent.strip_prefix(root).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            "accepted artifact escaped Pipeline v3 workspace",
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err(Error::new(
                ErrorCategory::State,
                "accepted artifact path is not normalized",
            ));
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            Error::new(
                ErrorCategory::State,
                format!("accepted artifact parent is unavailable: {error}"),
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::new(
                ErrorCategory::State,
                format!(
                    "accepted artifact parent is not a real directory: {}",
                    current.display()
                ),
            ));
        }
    }
    Ok(())
}

fn normalized_plan_bytes(plan: &PipelineV3Plan) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(plan).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("failed to encode resolved Pipeline v3 plan: {error}"),
        )
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn ensure_no_duplicate_stage_ids(plan: &PipelineV3Plan) -> Result<()> {
    let mut ids = BTreeSet::new();
    for stage in &plan.payload.stages {
        if !ids.insert(stage.id.as_str()) {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!("duplicate resolved stage id {}", stage.id),
            ));
        }
    }
    Ok(())
}
