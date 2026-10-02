//! Deterministic acceptance contracts. Providers observe; aniflow accepts.

pub mod native;

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use crate::error::{Error, ErrorCategory, Result};
use crate::pipeline_v3::{CapabilityRequirement, ProviderSelectionIntent};
use crate::provider::{canonical_sha256, require_sha256};
use crate::state_v3::{ArtifactEvidence, EvidenceReference, ValidationEvidence};
use crate::temporal::{StreamSelection, TemporalInspection, assess_processing, validate_reconstruction};

pub const VALIDATION_CONTEXT_SCHEMA_V1: &str = "aniflow.validation-context/v1";
pub const VALIDATOR_OBSERVATION_SCHEMA_V1: &str = "aniflow.validator-observation/v1";
pub const VALIDATION_REPORT_SCHEMA_V1: &str = "aniflow.validation-report/v1";
pub const ACCEPTANCE_RECORD_SCHEMA_V1: &str = "aniflow.acceptance-record/v1";
pub const ACCEPTANCE_SEMANTICS_V1: &str = "aniflow.layered-acceptance/v1";
pub const TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1: &str = "aniflow.validation/temporal-media/v1";
pub const PROVIDER_ARTIFACT_VALIDATION_CONTRACT_V1: &str = "aniflow.validation/provider-artifact/v1";

/// An explicit validator registration policy, resolved while planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationProviderRequirement {
    pub capability: CapabilityRequirement,
    pub provider: ProviderSelectionIntent,
}

/// The admitted #32 temporal profile. Source must be an explicit stage input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalValidationRequirement {
    pub source_artifact: String,
    pub source_selection: StreamSelection,
    pub artifact_selection: StreamSelection,
    pub allow_authored_subtitles: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLayer { Component, Stage, CandidateMaster, Delivery }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationDisposition { Passed, Failed, Partial, Skipped, Unavailable }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCriterion { ArtifactIdentity, SourceLineage, Provenance, Decodability }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationCheck {
    pub criterion: ValidationCriterion,
    pub disposition: ValidationDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationFailureCode {
    MissingEvidence, DuplicateEvidence, IncompatibleEvidence, StaleEvidence,
    RejectedObservation, ContradictoryEvidence, UnsupportedProfile, ArtifactChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationDiagnostic {
    pub code: ValidationFailureCode,
    pub validation_id: String,
    pub message: String,
}

pub(crate) fn refusal(code: ValidationFailureCode, id: &str, message: impl Into<String>) -> Error {
    Error::from_validation(ValidationDiagnostic { code, validation_id: id.to_owned(), message: message.into() })
}

fn valid_local_id(value: &str) -> bool {
    value.bytes().next().is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_'))
}

/// Path-free authority sent to a validator as its immutable context input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationContext {
    pub schema: String,
    pub validation_id: String,
    pub contract: String,
    pub plan_sha256: String,
    pub producer_invocation_sha256: String,
    pub producer_lock_sha256: String,
    pub validator_lock_sha256: String,
    pub artifact: ArtifactEvidence,
    pub inputs: Vec<ArtifactEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<TemporalValidationRequirement>,
}

impl ValidationContext {
    pub fn validate(&self) -> Result<()> {
        if self.schema != VALIDATION_CONTEXT_SCHEMA_V1 || !valid_local_id(&self.validation_id) {
            return Err(refusal(ValidationFailureCode::IncompatibleEvidence, &self.validation_id, "invalid validation context schema or identity"));
        }
        for digest in [&self.plan_sha256, &self.producer_invocation_sha256, &self.producer_lock_sha256, &self.validator_lock_sha256] {
            require_sha256(digest, "validation context identity")?;
        }
        self.artifact.validate()?;
        if self.artifact.byte_count == 0 || self.artifact.file_count == 0 || self.inputs.is_empty() {
            return Err(refusal(ValidationFailureCode::MissingEvidence, &self.validation_id, "nonempty artifact and source lineage are required"));
        }
        let mut seen = BTreeSet::new();
        for input in &self.inputs {
            input.validate()?;
            if !seen.insert((&input.port, &input.id)) {
                return Err(refusal(ValidationFailureCode::DuplicateEvidence, &self.validation_id, "duplicate source input binding"));
            }
        }
        match (self.contract.as_str(), &self.temporal) {
            (PROVIDER_ARTIFACT_VALIDATION_CONTRACT_V1, None) => Ok(()),
            (TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1, Some(policy))
                if self.artifact.kind == crate::ArtifactKind::File
                    && self.inputs.iter().any(|input| input.id == policy.source_artifact && input.kind == crate::ArtifactKind::File) => Ok(()),
            _ => Err(refusal(ValidationFailureCode::UnsupportedProfile, &self.validation_id, "unsupported or incomplete validator profile")),
        }
    }

    pub fn sha256(&self) -> Result<String> { self.validate()?; canonical_sha256(self) }
}

/// Full clock observations; their support flags are recomputed, never trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalValidationObservation {
    pub source: TemporalInspection,
    pub artifact: TemporalInspection,
}

/// A provider claim, not an accepted validation or a completion record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorObservation {
    pub schema: String,
    pub context_sha256: String,
    pub artifact_sha256: String,
    pub disposition: ValidationDisposition,
    pub checks: Vec<ValidationCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<TemporalValidationObservation>,
}

impl ValidatorObservation {
    /// Evaluate exact obligations; no provider-controlled `accepted` shortcut.
    pub fn accept(&self, context: &ValidationContext) -> Result<()> {
        context.validate()?;
        let id = &context.validation_id;
        if self.schema != VALIDATOR_OBSERVATION_SCHEMA_V1 {
            return Err(refusal(ValidationFailureCode::IncompatibleEvidence, id, "unsupported validator observation schema"));
        }
        if self.context_sha256 != context.sha256()? || self.artifact_sha256 != context.artifact.sha256 {
            return Err(refusal(ValidationFailureCode::StaleEvidence, id, "validator observation does not bind this context and artifact"));
        }
        if self.disposition != ValidationDisposition::Passed {
            return Err(refusal(ValidationFailureCode::RejectedObservation, id, "validator did not complete every required check"));
        }
        let mut criteria = BTreeSet::new();
        for check in &self.checks {
            if !criteria.insert(check.criterion) {
                return Err(refusal(ValidationFailureCode::DuplicateEvidence, id, "duplicate validator check"));
            }
            if check.disposition != ValidationDisposition::Passed {
                return Err(refusal(ValidationFailureCode::ContradictoryEvidence, id, "passed observation contains a non-passing check"));
            }
        }
        let mut expected = BTreeSet::from([ValidationCriterion::ArtifactIdentity, ValidationCriterion::SourceLineage, ValidationCriterion::Provenance]);
        if context.temporal.is_some() { expected.insert(ValidationCriterion::Decodability); }
        if criteria != expected {
            return Err(refusal(ValidationFailureCode::MissingEvidence, id, "validator check set must exactly match the contract"));
        }
        match (&context.temporal, &self.temporal) {
            (None, None) => Ok(()),
            (Some(policy), Some(observation)) => {
                let source = context.inputs.iter().find(|input| input.id == policy.source_artifact).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, id, "source lineage is missing"))?;
                for (inspection, artifact, selection) in [(&observation.source, source, &policy.source_selection), (&observation.artifact, &context.artifact, &policy.artifact_selection)] {
                    if inspection.schema != crate::temporal::TEMPORAL_INSPECTION_SCHEMA_V1
                        || inspection.source_sha256 != artifact.sha256
                        || inspection.source_size_bytes != artifact.byte_count
                        || &inspection.selection_intent != selection {
                        return Err(refusal(ValidationFailureCode::StaleEvidence, id, "temporal observations must bind exact bytes and declared stream selection"));
                    }
                    let assessment = assess_processing(inspection).map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
                    if assessment != inspection.processing {
                        return Err(refusal(ValidationFailureCode::ContradictoryEvidence, id, "temporal support and rational tolerance claims differ from recomputed policy"));
                    }
                }
                validate_reconstruction(&observation.source, &observation.artifact, policy.allow_authored_subtitles)
                    .map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))
            }
            _ => Err(refusal(ValidationFailureCode::MissingEvidence, id, "temporal evidence does not match the declared profile")),
        }
    }
}

/// Aniflow's durable acceptance of a particular provider observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderValidationReport {
    pub schema: String,
    pub context: ValidationContext,
    pub observation: ValidatorObservation,
    /// Exact UTF-8 output bytes, bound to the runtime output digest and size.
    pub raw_observation: String,
    pub execution_report: EvidenceReference,
}

impl ProviderValidationReport {
    pub fn validate(&self) -> Result<()> {
        if self.schema != VALIDATION_REPORT_SCHEMA_V1 {
            return Err(refusal(ValidationFailureCode::IncompatibleEvidence, &self.context.validation_id, "unsupported validation report schema"));
        }
        self.execution_report.validate()?;
        let raw: ValidatorObservation = serde_json::from_str(&self.raw_observation).map_err(|error| refusal(ValidationFailureCode::IncompatibleEvidence, &self.context.validation_id, error.to_string()))?;
        if raw != self.observation {
            return Err(refusal(ValidationFailureCode::ContradictoryEvidence, &self.context.validation_id, "normalized observation differs from provider output bytes"));
        }
        self.observation.accept(&self.context)
    }
}

/// Stable boundary evidence consumed by standalone clients and flow.
/// Its canonical SHA-256 is stored in an EvidenceReference, not in this payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceRecord {
    pub schema: String,
    pub semantics: String,
    pub layer: ValidationLayer,
    pub scope_id: String,
    pub plan_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_invocation_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_lock_sha256: Option<String>,
    pub inputs: Vec<ArtifactEvidence>,
    pub artifacts: Vec<ArtifactEvidence>,
    pub validations: Vec<ValidationEvidence>,
    pub children: Vec<EvidenceReference>,
}

impl AcceptanceRecord {
    pub fn validate(&self) -> Result<()> {
        let id = &self.scope_id;
        if self.schema != ACCEPTANCE_RECORD_SCHEMA_V1 || self.semantics != ACCEPTANCE_SEMANTICS_V1 || !valid_local_id(id) || self.artifacts.is_empty() {
            return Err(refusal(ValidationFailureCode::IncompatibleEvidence, id, "invalid acceptance boundary"));
        }
        require_sha256(&self.plan_sha256, "acceptance plan")?;
        if self.producer_invocation_sha256.is_some() != self.producer_lock_sha256.is_some() {
            return Err(refusal(ValidationFailureCode::MissingEvidence, id, "producer invocation and lock must be retained together"));
        }
        for digest in self.producer_invocation_sha256.iter().chain(self.producer_lock_sha256.iter()) { require_sha256(digest, "acceptance producer")?; }
        let mut ids = BTreeSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if artifact.byte_count == 0 || artifact.file_count == 0 || !ids.insert(&artifact.id) {
                return Err(refusal(ValidationFailureCode::ContradictoryEvidence, id, "accepted artifacts must be nonempty and unique"));
            }
        }
        let mut inputs = BTreeSet::new();
        for input in &self.inputs {
            input.validate()?;
            if !inputs.insert((&input.port, &input.id)) {
                return Err(refusal(ValidationFailureCode::DuplicateEvidence, id, "duplicate acceptance input binding"));
            }
        }
        let mut validations = BTreeSet::new();
        for validation in &self.validations {
            validation.validate()?;
            if !validations.insert(&validation.id) || !self.artifacts.iter().any(|a| a.id == validation.artifact_id && a.sha256 == validation.artifact_sha256) {
                return Err(refusal(ValidationFailureCode::ContradictoryEvidence, id, "validation identity is duplicate or mismatched"));
            }
        }
        let mut children = BTreeSet::new();
        for child in &self.children {
            child.validate()?;
            if !children.insert((&child.relative_path, &child.sha256)) {
                return Err(refusal(ValidationFailureCode::DuplicateEvidence, id, "duplicate acceptance child"));
            }
        }
        match self.layer {
            ValidationLayer::Component if self.artifacts.len() == 1 && self.children.is_empty() && self.producer_lock_sha256.is_some() && !self.inputs.is_empty() => Ok(()),
            ValidationLayer::Stage if self.children.len() == self.artifacts.len() && self.producer_lock_sha256.is_some() && !self.inputs.is_empty() => Ok(()),
            ValidationLayer::CandidateMaster if self.artifacts.len() == 1 && self.children.len() == 1 && self.producer_lock_sha256.is_some() && !self.inputs.is_empty() => Ok(()),
            ValidationLayer::Delivery if self.children.len() == self.artifacts.len() && self.validations.is_empty() && self.inputs.is_empty() && self.producer_lock_sha256.is_none() => Ok(()),
            _ => Err(refusal(ValidationFailureCode::MissingEvidence, id, "acceptance layer lacks its required evidence children")),
        }
    }
    pub fn sha256(&self) -> Result<String> { self.validate()?; canonical_sha256(self) }
}
