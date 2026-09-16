use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorCategory, Result};
use crate::pipeline_v3::CANONICAL_JSON_SCHEMA_V1;
use crate::provider::{
    ArtifactRole, StreamRole, canonical_sha256, require_sha256 as provider_require_sha256,
};
use crate::provider_runtime::ArtifactKind;
use crate::workspace_v3::PipelineV3Workspace;

pub const PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1: &str = "aniflow.pipeline-run/v1";
pub const PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1: &str = "aniflow.stage-checkpoint/v1";
const SHA256_ALGORITHM: &str = "sha256";

/// Complete run lifecycle retained by append-only manifest revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineV3RunState {
    Running,
    Interrupted,
    Failed,
    Cancelled,
    Complete,
}

/// Persisted lifecycle of one Pipeline v3 stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineV3StageState {
    Pending,
    Running,
    Validating,
    Complete,
    Failed,
    Cancelled,
    Invalidated,
    Skipped,
}

/// Immutable, content-addressed reference to persisted evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReference {
    pub relative_path: String,
    pub sha256: String,
}

impl EvidenceReference {
    pub fn validate(&self) -> Result<()> {
        validate_relative_path(&self.relative_path, "evidence relative_path")?;
        require_state_sha256(&self.sha256, "evidence sha256")
    }
}

/// Immutable reference from run state to one accepted stage checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageCheckpointReference {
    pub stage_id: String,
    pub relative_path: String,
    pub checkpoint_sha256: String,
}

impl StageCheckpointReference {
    pub fn validate(&self) -> Result<()> {
        validate_local_id(&self.stage_id, "checkpoint stage_id")?;
        require_state_sha256(&self.checkpoint_sha256, "checkpoint sha256")?;
        let expected = checkpoint_relative_path(&self.stage_id, &self.checkpoint_sha256);
        if self.relative_path != expected {
            return Err(state_error(format!(
                "stage checkpoint reference must use {expected}"
            )));
        }
        Ok(())
    }
}

/// Content and semantic identity observed for one stage input or output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidence {
    pub id: String,
    pub port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_role: Option<StreamRole>,
    pub kind: ArtifactKind,
    pub file_count: u64,
    pub byte_count: u64,
    pub sha256: String,
}

impl ArtifactEvidence {
    pub fn validate(&self) -> Result<()> {
        validate_local_id(&self.id, "artifact id")?;
        validate_local_id(&self.port, "artifact port")?;
        validate_printable_token(&self.artifact_type, "artifact_type")?;
        if let Some(path) = &self.relative_path {
            validate_relative_path(path, "artifact relative_path")?;
            if !path.starts_with("artifacts/") || path.len() == "artifacts/".len() {
                return Err(state_error(
                    "workspace artifact paths must remain beneath artifacts/",
                ));
            }
        }
        match self.kind {
            ArtifactKind::File if self.file_count != 1 => {
                return Err(state_error(format!(
                    "file artifact {} must report exactly one file",
                    self.id
                )));
            }
            ArtifactKind::Directory | ArtifactKind::File => {}
        }
        require_state_sha256(&self.sha256, "artifact sha256")
    }
}

/// Evidence that one declared validation accepted an exact artifact identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEvidence {
    pub id: String,
    pub contract: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub accepted: bool,
    pub evidence: EvidenceReference,
}

impl ValidationEvidence {
    pub fn validate(&self) -> Result<()> {
        validate_local_id(&self.id, "validation id")?;
        validate_printable_token(&self.contract, "validation contract")?;
        validate_local_id(&self.artifact_id, "validated artifact id")?;
        require_state_sha256(&self.artifact_sha256, "validated artifact sha256")?;
        if !self.accepted {
            return Err(state_error(
                "an accepted stage checkpoint cannot retain a rejected validation",
            ));
        }
        self.evidence.validate()
    }
}

/// Stable cause for accepting or rejecting checkpoint reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityReasonCode {
    MissingCheckpoint,
    UnsupportedCheckpointSchema,
    CorruptCheckpoint,
    StagePlanChanged,
    ProviderLockChanged,
    ExecutionPolicyChanged,
    InputChanged,
    DependencyChanged,
    ExecutionReportMissing,
    ExecutionReportInvalid,
    OutputMissing,
    OutputChanged,
    ValidationMissing,
    ValidationFailed,
    ValidationChanged,
}

/// One typed, human-explainable compatibility observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityReason {
    pub code: CompatibilityReasonCode,
    pub detail: String,
}

impl CompatibilityReason {
    pub fn new(code: CompatibilityReasonCode, detail: impl Into<String>) -> Result<Self> {
        let reason = Self {
            code,
            detail: detail.into(),
        };
        reason.validate()?;
        Ok(reason)
    }

    pub fn validate(&self) -> Result<()> {
        validate_printable_text(&self.detail, "compatibility reason detail")
    }
}

/// Persistable explanation of a stage checkpoint reuse decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityDecision {
    pub reusable: bool,
    pub reasons: Vec<CompatibilityReason>,
}

impl CompatibilityDecision {
    #[must_use]
    pub const fn compatible() -> Self {
        Self {
            reusable: true,
            reasons: Vec::new(),
        }
    }

    pub fn incompatible(reasons: Vec<CompatibilityReason>) -> Result<Self> {
        let decision = Self {
            reusable: false,
            reasons,
        };
        decision.validate()?;
        Ok(decision)
    }

    pub fn validate(&self) -> Result<()> {
        if self.reusable != self.reasons.is_empty() {
            return Err(state_error(
                "a reusable checkpoint must have no incompatibility reasons, and a rejected checkpoint must have at least one",
            ));
        }
        let mut reasons = BTreeSet::new();
        for reason in &self.reasons {
            reason.validate()?;
            if !reasons.insert((reason.code, reason.detail.as_str())) {
                return Err(state_error(
                    "compatibility decision contains a duplicate reason",
                ));
            }
        }
        Ok(())
    }
}

/// Current summary for one stage in a run-manifest revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageRunRecord {
    pub stage_id: String,
    pub state: PipelineV3StageState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<StageCheckpointReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CompatibilityDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl StageRunRecord {
    pub fn validate(&self) -> Result<()> {
        validate_local_id(&self.stage_id, "stage_id")?;
        if let Some(checkpoint) = &self.checkpoint {
            checkpoint.validate()?;
            if checkpoint.stage_id != self.stage_id {
                return Err(state_error(format!(
                    "stage {} references a checkpoint for {}",
                    self.stage_id, checkpoint.stage_id
                )));
            }
        }
        if self.state == PipelineV3StageState::Complete && self.checkpoint.is_none() {
            return Err(state_error(format!(
                "complete stage {} must reference an accepted checkpoint",
                self.stage_id
            )));
        }
        if self.state != PipelineV3StageState::Complete && self.checkpoint.is_some() {
            return Err(state_error(format!(
                "non-complete stage {} cannot claim an accepted checkpoint",
                self.stage_id
            )));
        }
        if let Some(compatibility) = &self.compatibility {
            compatibility.validate()?;
            if compatibility.reusable && self.state != PipelineV3StageState::Complete {
                return Err(state_error(format!(
                    "only complete stage {} can retain a reusable compatibility decision",
                    self.stage_id
                )));
            }
            if !compatibility.reusable && self.state != PipelineV3StageState::Invalidated {
                return Err(state_error(format!(
                    "only invalidated stage {} can retain an incompatible checkpoint decision",
                    self.stage_id
                )));
            }
        }
        if self.state == PipelineV3StageState::Invalidated
            && self
                .compatibility
                .as_ref()
                .is_none_or(|decision| decision.reusable)
        {
            return Err(state_error(format!(
                "invalidated stage {} must explain its incompatible checkpoint decision",
                self.stage_id
            )));
        }
        if let Some(message) = &self.message {
            validate_printable_text(message, "stage message")?;
        }
        Ok(())
    }
}

/// Canonical material covered by a run-manifest revision digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3RunManifestPayload {
    pub run_id: String,
    pub aniflow_version: String,
    pub plan_sha256: String,
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_manifest_sha256: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub state: PipelineV3RunState,
    pub stages: Vec<StageRunRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<ArtifactEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

/// One append-only, self-validating Pipeline v3 run-manifest revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3RunManifest {
    pub schema: String,
    pub algorithm: String,
    pub canonicalization: String,
    pub manifest_sha256: String,
    pub payload: PipelineV3RunManifestPayload,
}

impl PipelineV3RunManifest {
    pub fn new(
        run_id: impl Into<String>,
        plan_sha256: impl Into<String>,
        stages: Vec<StageRunRecord>,
    ) -> Result<Self> {
        let now = Utc::now();
        Self::from_payload(PipelineV3RunManifestPayload {
            run_id: run_id.into(),
            aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
            plan_sha256: plan_sha256.into(),
            revision: 1,
            previous_manifest_sha256: None,
            created_at: now,
            updated_at: now,
            state: PipelineV3RunState::Running,
            stages,
            outputs: Vec::new(),
            diagnostic: None,
        })
    }

    pub fn next_revision(
        &self,
        state: PipelineV3RunState,
        stages: Vec<StageRunRecord>,
        outputs: Vec<ArtifactEvidence>,
        diagnostic: Option<String>,
    ) -> Result<Self> {
        self.validate()?;
        let revision = self
            .payload
            .revision
            .checked_add(1)
            .ok_or_else(|| state_error("run-manifest revision overflow"))?;
        let now = Utc::now().max(self.payload.updated_at);
        Self::from_payload(PipelineV3RunManifestPayload {
            run_id: self.payload.run_id.clone(),
            aniflow_version: env!("CARGO_PKG_VERSION").to_owned(),
            plan_sha256: self.payload.plan_sha256.clone(),
            revision,
            previous_manifest_sha256: Some(self.manifest_sha256.clone()),
            created_at: self.payload.created_at,
            updated_at: now,
            state,
            stages,
            outputs,
            diagnostic,
        })
    }

    /// Decode normalized, closed run state and reject every explicit null.
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let manifest: Self = decode_normalized_json(input, "Pipeline v3 run manifest")?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1 {
            return Err(state_error(format!(
                "unsupported Pipeline v3 run-manifest schema {}; expected {}",
                self.schema, PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1
            )));
        }
        validate_algorithm(&self.algorithm, "run manifest")?;
        validate_canonicalization(&self.canonicalization, "run manifest")?;
        require_state_sha256(&self.manifest_sha256, "manifest_sha256")?;
        validate_run_manifest_payload(&self.payload)?;
        let expected = canonical_sha256(&self.payload)?;
        if expected != self.manifest_sha256 {
            return Err(state_error(format!(
                "Pipeline v3 run manifest digest mismatch; expected {expected}"
            )));
        }
        Ok(())
    }

    fn from_payload(payload: PipelineV3RunManifestPayload) -> Result<Self> {
        validate_run_manifest_payload(&payload)?;
        let manifest_sha256 = canonical_sha256(&payload)?;
        Ok(Self {
            schema: PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1.to_owned(),
            algorithm: SHA256_ALGORITHM.to_owned(),
            canonicalization: CANONICAL_JSON_SCHEMA_V1.to_owned(),
            manifest_sha256,
            payload,
        })
    }
}

/// Canonical material covered by an immutable stage-checkpoint digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageCheckpointPayload {
    pub stage_id: String,
    /// Whole-plan authority for this run; cross-plan checkpoint import is forbidden.
    pub plan_sha256: String,
    /// Deterministic semantic key excluding runtime paths, timestamps, and attempt evidence.
    pub stage_invocation_sha256: String,
    pub provider_lock_sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<StageCheckpointReference>,
    pub inputs: Vec<ArtifactEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<ArtifactEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validations: Vec<ValidationEvidence>,
    pub execution_report: EvidenceReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility_fingerprint: Option<EvidenceReference>,
    pub completed_at: DateTime<Utc>,
}

/// Immutable evidence that one stage completed and its outputs were accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageCheckpoint {
    pub schema: String,
    pub algorithm: String,
    pub canonicalization: String,
    pub checkpoint_sha256: String,
    pub payload: StageCheckpointPayload,
}

impl StageCheckpoint {
    pub fn new(payload: StageCheckpointPayload) -> Result<Self> {
        validate_stage_checkpoint_payload(&payload)?;
        let checkpoint_sha256 = canonical_sha256(&payload)?;
        Ok(Self {
            schema: PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1.to_owned(),
            algorithm: SHA256_ALGORITHM.to_owned(),
            canonicalization: CANONICAL_JSON_SCHEMA_V1.to_owned(),
            checkpoint_sha256,
            payload,
        })
    }

    /// Decode a normalized, closed checkpoint and reject every explicit null.
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let checkpoint: Self = decode_normalized_json(input, "Pipeline v3 stage checkpoint")?;
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1 {
            return Err(state_error(format!(
                "unsupported Pipeline v3 stage-checkpoint schema {}; expected {}",
                self.schema, PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1
            )));
        }
        validate_algorithm(&self.algorithm, "stage checkpoint")?;
        validate_canonicalization(&self.canonicalization, "stage checkpoint")?;
        require_state_sha256(&self.checkpoint_sha256, "checkpoint_sha256")?;
        validate_stage_checkpoint_payload(&self.payload)?;
        let expected = canonical_sha256(&self.payload)?;
        if expected != self.checkpoint_sha256 {
            return Err(state_error(format!(
                "Pipeline v3 stage checkpoint digest mismatch; expected {expected}"
            )));
        }
        Ok(())
    }

    pub fn reference(&self) -> Result<StageCheckpointReference> {
        self.validate()?;
        Ok(StageCheckpointReference {
            stage_id: self.payload.stage_id.clone(),
            relative_path: checkpoint_relative_path(
                &self.payload.stage_id,
                &self.checkpoint_sha256,
            ),
            checkpoint_sha256: self.checkpoint_sha256.clone(),
        })
    }
}

/// Publish one immutable run-manifest revision after validating its predecessor.
pub fn append_run_manifest(
    workspace: &PipelineV3Workspace,
    manifest: &PipelineV3RunManifest,
) -> Result<PathBuf> {
    manifest.validate()?;
    validate_workspace_run_id(workspace, manifest)?;
    let existing = load_optional_run_manifest_chain(workspace)?;
    match existing.last() {
        None => {
            if manifest.payload.revision != 1 || manifest.payload.previous_manifest_sha256.is_some()
            {
                return Err(state_error(
                    "the first run-manifest revision must be revision 1 without a predecessor",
                ));
            }
        }
        Some(previous) => validate_manifest_successor(previous, manifest)?,
    }

    let path = workspace.manifest_revision(manifest.payload.revision);
    workspace.publish_json_new(&path, manifest)?;
    Ok(path)
}

/// Load every manifest revision and validate its complete hash chain.
pub fn load_run_manifest_chain(
    workspace: &PipelineV3Workspace,
) -> Result<Vec<PipelineV3RunManifest>> {
    let manifests = load_optional_run_manifest_chain(workspace)?;
    if manifests.is_empty() {
        return Err(state_error(format!(
            "Pipeline v3 run contains no manifest revisions in {}",
            workspace.manifests().display()
        )));
    }
    Ok(manifests)
}

/// Load the newest valid manifest revision without mutating the workspace.
pub fn load_latest_run_manifest(workspace: &PipelineV3Workspace) -> Result<PipelineV3RunManifest> {
    load_run_manifest_chain(workspace)?
        .pop()
        .ok_or_else(|| state_error("Pipeline v3 run contains no manifest revisions"))
}

/// Atomically publish one immutable, content-addressed stage checkpoint.
pub fn publish_stage_checkpoint(
    workspace: &PipelineV3Workspace,
    checkpoint: &StageCheckpoint,
) -> Result<StageCheckpointReference> {
    checkpoint.validate()?;
    let reference = checkpoint.reference()?;
    let path =
        workspace.stage_checkpoint(&checkpoint.payload.stage_id, &checkpoint.checkpoint_sha256);
    workspace.publish_json_new(&path, checkpoint)?;
    Ok(reference)
}

/// Load and validate a referenced stage checkpoint without repairing state.
pub fn load_stage_checkpoint(
    workspace: &PipelineV3Workspace,
    reference: &StageCheckpointReference,
) -> Result<StageCheckpoint> {
    reference.validate()?;
    let path = workspace.root().join(&reference.relative_path);
    require_regular_file(&path, "stage checkpoint")?;
    let checkpoint: StageCheckpoint = read_json(&path, "stage checkpoint")?;
    checkpoint.validate()?;
    if checkpoint.payload.stage_id != reference.stage_id
        || checkpoint.checkpoint_sha256 != reference.checkpoint_sha256
    {
        return Err(state_error(format!(
            "stage checkpoint {} does not match its manifest reference",
            path.display()
        )));
    }
    Ok(checkpoint)
}

fn validate_run_manifest_payload(payload: &PipelineV3RunManifestPayload) -> Result<()> {
    validate_printable_token(&payload.run_id, "run_id")?;
    validate_printable_token(&payload.aniflow_version, "aniflow_version")?;
    require_state_sha256(&payload.plan_sha256, "plan_sha256")?;
    if payload.revision == 0 {
        return Err(state_error("run-manifest revision must begin at 1"));
    }
    match (payload.revision, &payload.previous_manifest_sha256) {
        (1, None) => {}
        (1, Some(_)) => {
            return Err(state_error(
                "run-manifest revision 1 cannot identify a predecessor",
            ));
        }
        (_, Some(digest)) => require_state_sha256(digest, "previous_manifest_sha256")?,
        (_, None) => {
            return Err(state_error(
                "run-manifest revisions after 1 must identify their predecessor",
            ));
        }
    }
    if payload.updated_at < payload.created_at {
        return Err(state_error(
            "run-manifest updated_at cannot precede created_at",
        ));
    }
    if payload.stages.is_empty() {
        return Err(state_error(
            "Pipeline v3 run must retain at least one stage",
        ));
    }
    let mut stages = BTreeSet::new();
    for stage in &payload.stages {
        stage.validate()?;
        if !stages.insert(stage.stage_id.as_str()) {
            return Err(state_error(format!(
                "run manifest repeats stage {}",
                stage.stage_id
            )));
        }
    }
    if payload.revision == 1
        && (payload.state != PipelineV3RunState::Running
            || payload
                .stages
                .iter()
                .any(|stage| stage.state != PipelineV3StageState::Pending)
            || payload.diagnostic.is_some())
    {
        return Err(state_error(
            "run-manifest revision 1 must begin running with only pending stages",
        ));
    }
    let mut outputs = BTreeSet::new();
    for output in &payload.outputs {
        output.validate()?;
        if output.relative_path.is_none() {
            return Err(state_error(format!(
                "final output {} must identify its workspace artifact path",
                output.id
            )));
        }
        validate_nonempty_output(output)?;
        if !outputs.insert(output.id.as_str()) {
            return Err(state_error(format!(
                "run manifest repeats final output {}",
                output.id
            )));
        }
    }
    if payload.state == PipelineV3RunState::Complete {
        if payload.outputs.is_empty() {
            return Err(state_error(
                "a complete Pipeline v3 run must retain a final output",
            ));
        }
        if payload.stages.iter().any(|stage| {
            !matches!(
                stage.state,
                PipelineV3StageState::Complete | PipelineV3StageState::Skipped
            )
        }) {
            return Err(state_error(
                "a complete Pipeline v3 run cannot retain an incomplete stage",
            ));
        }
    } else if !payload.outputs.is_empty() {
        return Err(state_error(
            "only a complete Pipeline v3 run can retain final outputs",
        ));
    }
    if let Some(diagnostic) = &payload.diagnostic {
        validate_printable_text(diagnostic, "run diagnostic")?;
    }
    Ok(())
}

fn validate_stage_checkpoint_payload(payload: &StageCheckpointPayload) -> Result<()> {
    validate_local_id(&payload.stage_id, "stage checkpoint stage_id")?;
    require_state_sha256(&payload.plan_sha256, "stage checkpoint plan_sha256")?;
    require_state_sha256(&payload.stage_invocation_sha256, "stage_invocation_sha256")?;
    require_state_sha256(&payload.provider_lock_sha256, "provider_lock_sha256")?;
    payload.execution_report.validate()?;
    if let Some(fingerprint) = &payload.compatibility_fingerprint {
        fingerprint.validate()?;
    }

    let mut dependencies = BTreeSet::new();
    for dependency in &payload.dependencies {
        dependency.validate()?;
        if dependency.stage_id == payload.stage_id {
            return Err(state_error("a stage checkpoint cannot depend on itself"));
        }
        if !dependencies.insert(dependency.stage_id.as_str()) {
            return Err(state_error(format!(
                "stage checkpoint repeats dependency {}",
                dependency.stage_id
            )));
        }
    }

    if payload.inputs.is_empty() {
        return Err(state_error(
            "a stage checkpoint must retain at least one input identity",
        ));
    }
    let mut inputs = BTreeSet::new();
    for input in &payload.inputs {
        input.validate()?;
        if !inputs.insert((input.port.as_str(), input.id.as_str())) {
            return Err(state_error(format!(
                "stage checkpoint repeats input {} on port {}",
                input.id, input.port
            )));
        }
    }

    let mut outputs = BTreeSet::new();
    for output in &payload.outputs {
        output.validate()?;
        if output.relative_path.is_none() {
            return Err(state_error(format!(
                "stage output {} must identify its workspace artifact path",
                output.id
            )));
        }
        validate_nonempty_output(output)?;
        if !outputs.insert(output.id.as_str()) {
            return Err(state_error(format!(
                "stage checkpoint repeats output {}",
                output.id
            )));
        }
    }

    let mut validations = BTreeSet::new();
    for validation in &payload.validations {
        validation.validate()?;
        if !validations.insert(validation.id.as_str()) {
            return Err(state_error(format!(
                "stage checkpoint repeats validation {}",
                validation.id
            )));
        }
        let output = payload
            .outputs
            .iter()
            .find(|output| output.id == validation.artifact_id)
            .ok_or_else(|| {
                state_error(format!(
                    "validation {} references unknown stage output {}",
                    validation.id, validation.artifact_id
                ))
            })?;
        if output.sha256 != validation.artifact_sha256 {
            return Err(state_error(format!(
                "validation {} does not identify the accepted bytes for {}",
                validation.id, validation.artifact_id
            )));
        }
    }
    Ok(())
}

fn validate_nonempty_output(output: &ArtifactEvidence) -> Result<()> {
    if output.file_count == 0 || output.byte_count == 0 {
        Err(state_error(format!(
            "accepted output {} must retain non-empty content evidence",
            output.id
        )))
    } else {
        Ok(())
    }
}

fn load_optional_run_manifest_chain(
    workspace: &PipelineV3Workspace,
) -> Result<Vec<PipelineV3RunManifest>> {
    let mut revisions = Vec::new();
    let entries = fs::read_dir(workspace.manifests()).map_err(|error| {
        state_error(format!(
            "failed to read Pipeline v3 manifests {}: {error}",
            workspace.manifests().display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            state_error(format!("failed to inspect Pipeline v3 manifests: {error}"))
        })?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(revision) = parse_manifest_revision(&file_name) else {
            continue;
        };
        require_regular_file(&entry.path(), "run-manifest revision")?;
        revisions.push((revision, entry.path()));
    }
    revisions.sort_by_key(|(revision, _)| *revision);

    let mut manifests = Vec::with_capacity(revisions.len());
    for (position, (revision, path)) in revisions.into_iter().enumerate() {
        let expected_revision = u64::try_from(position + 1)
            .map_err(|_| state_error("run-manifest chain is too large"))?;
        if revision != expected_revision {
            return Err(state_error(format!(
                "run-manifest chain is not contiguous; expected revision {expected_revision}, found {revision}"
            )));
        }
        let manifest: PipelineV3RunManifest = read_json(&path, "run manifest")?;
        manifest.validate()?;
        validate_workspace_run_id(workspace, &manifest)?;
        if manifest.payload.revision != revision {
            return Err(state_error(format!(
                "run-manifest file {} contains revision {}",
                path.display(),
                manifest.payload.revision
            )));
        }
        if let Some(previous) = manifests.last() {
            validate_manifest_successor(previous, &manifest)?;
        }
        manifests.push(manifest);
    }
    Ok(manifests)
}

fn validate_workspace_run_id(
    workspace: &PipelineV3Workspace,
    manifest: &PipelineV3RunManifest,
) -> Result<()> {
    if manifest.payload.run_id == workspace.run_id() {
        Ok(())
    } else {
        Err(state_error(format!(
            "run manifest {} does not belong to workspace {}",
            manifest.payload.run_id,
            workspace.root().display()
        )))
    }
}

fn validate_manifest_successor(
    previous: &PipelineV3RunManifest,
    next: &PipelineV3RunManifest,
) -> Result<()> {
    previous.validate()?;
    next.validate()?;
    let expected_revision = previous
        .payload
        .revision
        .checked_add(1)
        .ok_or_else(|| state_error("run-manifest revision overflow"))?;
    if next.payload.revision != expected_revision
        || next.payload.previous_manifest_sha256.as_deref()
            != Some(previous.manifest_sha256.as_str())
    {
        return Err(state_error(format!(
            "run-manifest revision {} does not extend revision {}",
            next.payload.revision, previous.payload.revision
        )));
    }
    if next.payload.run_id != previous.payload.run_id
        || next.payload.plan_sha256 != previous.payload.plan_sha256
        || next.payload.created_at != previous.payload.created_at
    {
        return Err(state_error(
            "run-manifest successor changed immutable run authority",
        ));
    }
    if !next
        .payload
        .stages
        .iter()
        .map(|stage| stage.stage_id.as_str())
        .eq(previous
            .payload
            .stages
            .iter()
            .map(|stage| stage.stage_id.as_str()))
    {
        return Err(state_error(
            "run-manifest successor changed the planned stage order",
        ));
    }
    if !valid_run_transition(previous.payload.state, next.payload.state) {
        return Err(state_error(format!(
            "run-manifest lifecycle cannot transition from {:?} to {:?}",
            previous.payload.state, next.payload.state
        )));
    }
    let mut changed_stages = 0_u8;
    for (previous_stage, next_stage) in previous.payload.stages.iter().zip(&next.payload.stages) {
        if previous_stage != next_stage {
            changed_stages = changed_stages.saturating_add(1);
            validate_stage_successor(previous_stage, next_stage)?;
        }
    }
    if changed_stages > 1 {
        return Err(state_error(
            "one run-manifest revision cannot change more than one stage record",
        ));
    }
    if next.payload.updated_at < previous.payload.updated_at {
        return Err(state_error(
            "run-manifest successor timestamp precedes its predecessor",
        ));
    }
    Ok(())
}

const fn valid_run_transition(previous: PipelineV3RunState, next: PipelineV3RunState) -> bool {
    match previous {
        PipelineV3RunState::Running => matches!(
            next,
            PipelineV3RunState::Running
                | PipelineV3RunState::Interrupted
                | PipelineV3RunState::Failed
                | PipelineV3RunState::Cancelled
                | PipelineV3RunState::Complete
        ),
        PipelineV3RunState::Interrupted
        | PipelineV3RunState::Failed
        | PipelineV3RunState::Cancelled
        | PipelineV3RunState::Complete => matches!(
            next,
            PipelineV3RunState::Running | PipelineV3RunState::Cancelled
        ),
    }
}

fn validate_stage_successor(previous: &StageRunRecord, next: &StageRunRecord) -> Result<()> {
    if previous.state == next.state {
        if previous.state != PipelineV3StageState::Complete
            || previous.checkpoint != next.checkpoint
            || next
                .compatibility
                .as_ref()
                .is_none_or(|decision| !decision.reusable)
        {
            return Err(state_error(format!(
                "stage {} changed evidence without a lifecycle transition",
                previous.stage_id
            )));
        }
        return Ok(());
    }

    let allowed = match previous.state {
        PipelineV3StageState::Pending => matches!(
            next.state,
            PipelineV3StageState::Running
                | PipelineV3StageState::Failed
                | PipelineV3StageState::Cancelled
                | PipelineV3StageState::Invalidated
        ),
        PipelineV3StageState::Running => matches!(
            next.state,
            PipelineV3StageState::Validating
                | PipelineV3StageState::Complete
                | PipelineV3StageState::Failed
                | PipelineV3StageState::Cancelled
                | PipelineV3StageState::Invalidated
        ),
        PipelineV3StageState::Validating => matches!(
            next.state,
            PipelineV3StageState::Complete
                | PipelineV3StageState::Failed
                | PipelineV3StageState::Cancelled
                | PipelineV3StageState::Invalidated
        ),
        PipelineV3StageState::Complete => next.state == PipelineV3StageState::Invalidated,
        PipelineV3StageState::Failed | PipelineV3StageState::Cancelled => matches!(
            next.state,
            PipelineV3StageState::Complete
                | PipelineV3StageState::Cancelled
                | PipelineV3StageState::Invalidated
        ),
        PipelineV3StageState::Invalidated => matches!(
            next.state,
            PipelineV3StageState::Running
                | PipelineV3StageState::Complete
                | PipelineV3StageState::Failed
                | PipelineV3StageState::Cancelled
        ),
        PipelineV3StageState::Skipped => next.state == PipelineV3StageState::Invalidated,
    };
    if !allowed {
        return Err(state_error(format!(
            "stage {} lifecycle cannot transition from {:?} to {:?}",
            previous.stage_id, previous.state, next.state
        )));
    }
    Ok(())
}

fn parse_manifest_revision(file_name: &str) -> Option<u64> {
    let digits = file_name.strip_suffix(".json")?;
    if digits.len() != 20 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn checkpoint_relative_path(stage_id: &str, checkpoint_sha256: &str) -> String {
    format!("state/checkpoints/{stage_id}-{checkpoint_sha256}.json")
}

fn read_json<T>(path: &Path, kind: &str) -> Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let bytes = fs::read(path).map_err(|error| {
        state_error(format!("failed to read {kind} {}: {error}", path.display()))
    })?;
    decode_normalized_json(&bytes, &format!("{kind} in {}", path.display()))
}

fn decode_normalized_json<T>(input: &[u8], kind: &str) -> Result<T>
where
    T: DeserializeOwned + Serialize,
{
    // Decode the typed structure first so duplicate fields cannot be hidden by
    // the generic JSON value used for normalized-representation checks.
    let decoded: T = serde_json::from_slice(input)
        .map_err(|error| state_error(format!("invalid {kind} JSON: {error}")))?;
    let value: serde_json::Value = serde_json::from_slice(input)
        .map_err(|error| state_error(format!("invalid {kind} JSON: {error}")))?;
    if json_contains_null(&value) {
        return Err(state_error(format!(
            "{kind} JSON must omit unavailable values instead of using null"
        )));
    }
    let normalized = serde_json::to_value(&decoded).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("failed to normalize {kind} JSON: {error}"),
        )
    })?;
    if value != normalized {
        return Err(state_error(format!(
            "{kind} JSON must use the normalized contract representation"
        )));
    }
    Ok(decoded)
}

fn json_contains_null(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Array(values) => values.iter().any(json_contains_null),
        serde_json::Value::Object(values) => values.values().any(json_contains_null),
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => false,
    }
}

fn require_regular_file(path: &Path, kind: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        state_error(format!(
            "{kind} is unavailable at {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(state_error(format!(
            "{kind} must be a regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_algorithm(value: &str, contract: &str) -> Result<()> {
    if value == SHA256_ALGORITHM {
        Ok(())
    } else {
        Err(state_error(format!(
            "unsupported {contract} digest algorithm {value}; expected sha256"
        )))
    }
}

fn validate_canonicalization(value: &str, contract: &str) -> Result<()> {
    if value == CANONICAL_JSON_SCHEMA_V1 {
        Ok(())
    } else {
        Err(state_error(format!(
            "unsupported {contract} canonicalization {value}; expected {CANONICAL_JSON_SCHEMA_V1}"
        )))
    }
}

fn require_state_sha256(value: &str, field: &str) -> Result<()> {
    provider_require_sha256(value, field).map_err(|error| state_error(error.to_string()))
}

fn validate_local_id(value: &str, field: &str) -> Result<()> {
    let valid = value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(state_error(format!(
            "{field} must use lowercase letters, numbers, hyphens, or underscores"
        )))
    }
}

fn validate_printable_token(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
    {
        Err(state_error(format!(
            "{field} must be a non-empty printable token"
        )))
    } else {
        Ok(())
    }
}

fn validate_printable_text(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        Err(state_error(format!(
            "{field} must be non-empty printable text"
        )))
    } else {
        Ok(())
    }
}

fn validate_relative_path(value: &str, field: &str) -> Result<()> {
    let path = Path::new(value);
    let portable = !value.is_empty()
        && !value.contains('\\')
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && value.split('/').all(is_portable_path_segment);
    if portable {
        Ok(())
    } else {
        Err(state_error(format!(
            "{field} must be a confined portable relative path"
        )))
    }
}

fn is_portable_path_segment(segment: &str) -> bool {
    if segment.is_empty()
        || matches!(segment, "." | "..")
        || segment.ends_with(['.', ' '])
        || segment.chars().any(|character| {
            character.is_control()
                || matches!(character, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
    {
        return false;
    }
    let basename = segment
        .split_once('.')
        .map_or(segment, |(basename, _)| basename)
        .to_ascii_uppercase();
    !matches!(basename.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(basename.len() == 4
            && (basename.starts_with("COM") || basename.starts_with("LPT"))
            && basename.as_bytes()[3].is_ascii_digit()
            && basename.as_bytes()[3] != b'0')
}

fn state_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::State, message)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Utc;
    use tempfile::tempdir;

    use super::{
        ArtifactEvidence, EvidenceReference, PipelineV3RunManifest, PipelineV3RunState,
        PipelineV3StageState, StageCheckpoint, StageCheckpointPayload, StageRunRecord,
        ValidationEvidence, append_run_manifest, load_latest_run_manifest, load_run_manifest_chain,
        load_stage_checkpoint, publish_stage_checkpoint,
    };
    use crate::provider::{ArtifactRole, StreamRole};
    use crate::provider_runtime::ArtifactKind;
    use crate::workspace_v3::PipelineV3Workspace;

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    #[test]
    fn manifest_revisions_are_append_only_and_hash_linked() {
        let temporary = tempdir().expect("temporary directory should be created");
        let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
            .expect("workspace should be created");
        let first = PipelineV3RunManifest::new(workspace.run_id(), DIGEST_A, vec![pending_stage()])
            .expect("initial manifest should be valid");
        append_run_manifest(&workspace, &first).expect("first revision should publish");
        let second = first
            .next_revision(
                PipelineV3RunState::Interrupted,
                vec![pending_stage()],
                Vec::new(),
                Some("interrupted safely".to_owned()),
            )
            .expect("successor should be valid");
        append_run_manifest(&workspace, &second).expect("second revision should publish");

        let chain = load_run_manifest_chain(&workspace).expect("chain should validate");
        assert_eq!(chain, vec![first, second.clone()]);
        assert_eq!(
            load_latest_run_manifest(&workspace).expect("latest should load"),
            second
        );
    }

    #[test]
    fn tampered_manifest_chain_is_rejected_without_mutation() {
        let temporary = tempdir().expect("temporary directory should be created");
        let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
            .expect("workspace should be created");
        let first = PipelineV3RunManifest::new(workspace.run_id(), DIGEST_A, vec![pending_stage()])
            .expect("initial manifest should be valid");
        append_run_manifest(&workspace, &first).expect("first revision should publish");
        let path = workspace.manifest_revision(1);
        let before = fs::read(&path).expect("manifest should be readable");
        let mut value: serde_json::Value =
            serde_json::from_slice(&before).expect("manifest should parse");
        value["payload"]["run_id"] = serde_json::Value::String("tampered".to_owned());
        fs::write(
            &path,
            serde_json::to_vec_pretty(&value).expect("tampered value should encode"),
        )
        .expect("fixture should be tampered");
        let tampered = fs::read(&path).expect("tampered manifest should be readable");

        load_run_manifest_chain(&workspace).expect_err("tampered chain must be rejected");
        assert_eq!(
            fs::read(&path).expect("inspection must not rewrite state"),
            tampered
        );
    }

    #[test]
    fn checkpoint_is_content_addressed_immutable_and_self_validating() {
        let temporary = tempdir().expect("temporary directory should be created");
        let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
            .expect("workspace should be created");
        let checkpoint = stage_checkpoint();
        let reference =
            publish_stage_checkpoint(&workspace, &checkpoint).expect("checkpoint should publish");

        assert!(workspace.root().join(&reference.relative_path).is_file());
        assert_eq!(
            load_stage_checkpoint(&workspace, &reference).expect("checkpoint should load"),
            checkpoint
        );
        publish_stage_checkpoint(&workspace, &checkpoint)
            .expect_err("an immutable checkpoint must not be overwritten");
    }

    #[test]
    fn closed_contracts_reject_unknown_fields() {
        let checkpoint = stage_checkpoint();
        let mut value = serde_json::to_value(checkpoint).expect("checkpoint should serialize");
        value["unknown"] = serde_json::Value::Bool(true);
        StageCheckpoint::from_json_slice(
            &serde_json::to_vec(&value).expect("checkpoint fixture should encode"),
        )
        .expect_err("unknown checkpoint fields must be rejected");
    }

    #[test]
    fn state_rejects_unknown_canonicalization() {
        let mut checkpoint = stage_checkpoint();
        checkpoint.canonicalization = "aniflow.canonical-json/v2".to_owned();
        checkpoint
            .validate()
            .expect_err("unknown canonicalization must fail closed");
    }

    #[test]
    fn state_parsers_reject_explicit_nulls() {
        let temporary = tempdir().expect("temporary directory should be created");
        let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
            .expect("workspace should be created");
        let manifest =
            PipelineV3RunManifest::new(workspace.run_id(), DIGEST_A, vec![pending_stage()])
                .expect("initial manifest should be valid");
        let mut manifest_value = serde_json::to_value(manifest).expect("manifest should serialize");
        manifest_value["payload"]["diagnostic"] = serde_json::Value::Null;
        PipelineV3RunManifest::from_json_slice(
            &serde_json::to_vec(&manifest_value).expect("manifest fixture should encode"),
        )
        .expect_err("explicit manifest null must be rejected");

        let mut checkpoint_value =
            serde_json::to_value(stage_checkpoint()).expect("checkpoint should serialize");
        checkpoint_value["payload"]["compatibility_fingerprint"] = serde_json::Value::Null;
        StageCheckpoint::from_json_slice(
            &serde_json::to_vec(&checkpoint_value).expect("checkpoint fixture should encode"),
        )
        .expect_err("explicit checkpoint null must be rejected");
    }

    fn pending_stage() -> StageRunRecord {
        StageRunRecord {
            stage_id: "enhance".to_owned(),
            state: PipelineV3StageState::Pending,
            checkpoint: None,
            compatibility: None,
            message: None,
        }
    }

    fn stage_checkpoint() -> StageCheckpoint {
        let output = artifact(
            "enhanced",
            "processed_frames",
            Some("artifacts/enhance/frames"),
            DIGEST_B,
        );
        StageCheckpoint::new(StageCheckpointPayload {
            stage_id: "enhance".to_owned(),
            plan_sha256: DIGEST_A.to_owned(),
            stage_invocation_sha256: DIGEST_C.to_owned(),
            provider_lock_sha256: DIGEST_A.to_owned(),
            dependencies: Vec::new(),
            inputs: vec![artifact("source", "frames", None, DIGEST_A)],
            outputs: vec![output.clone()],
            validations: vec![ValidationEvidence {
                id: "enhanced-valid".to_owned(),
                contract: "aniflow.validation/frame-set/v1".to_owned(),
                artifact_id: output.id,
                artifact_sha256: output.sha256,
                accepted: true,
                evidence: EvidenceReference {
                    relative_path: "providers/enhance/validation.json".to_owned(),
                    sha256: DIGEST_C.to_owned(),
                },
            }],
            execution_report: EvidenceReference {
                relative_path: "providers/enhance/report.json".to_owned(),
                sha256: DIGEST_B.to_owned(),
            },
            compatibility_fingerprint: None,
            completed_at: Utc::now(),
        })
        .expect("checkpoint fixture should be valid")
    }

    fn artifact(
        id: &str,
        port: &str,
        relative_path: Option<&str>,
        sha256: &str,
    ) -> ArtifactEvidence {
        ArtifactEvidence {
            id: id.to_owned(),
            port: port.to_owned(),
            relative_path: relative_path.map(str::to_owned),
            artifact_type: "application/vnd.aniflow.frame-set+directory".to_owned(),
            artifact_role: ArtifactRole::TemporalComponent,
            stream_role: Some(StreamRole::Video),
            kind: ArtifactKind::Directory,
            file_count: 1,
            byte_count: 128,
            sha256: sha256.to_owned(),
        }
    }
}
