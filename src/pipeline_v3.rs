//! Deterministic, read-only planning for the Pipeline v3 contract.
//!
//! Planning resolves explicit provider registrations and identities, but it
//! never launches a provider, creates a workspace, or publishes an artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::{Error, ErrorCategory, Result};
use crate::provider::{
    ArtifactCardinality, ArtifactPort, ArtifactRole, CapabilityDeclaration, ComponentIdentity,
    ComponentRequirement, PROVIDER_MANIFEST_SCHEMA_V1, ProvenanceContract, ProvenanceField,
    ProviderConfiguration, ProviderIdentity, ProviderManifest, RequirementLevel, SideEffect,
    StreamRole, canonical_json_bytes, canonical_sha256, decode_json, invalid, require_nonempty,
    require_schema, require_sha256, require_token, validate_capability_id,
};
use crate::provider_runtime::{
    AvailabilityCode, ComponentInventory, HostResources, ProviderCandidate, ProviderLock,
    ProviderRegistration, ProviderRegistry, ProviderResolutionAttempt, ProviderResolutionRequest,
    ProviderSelection, ProviderSelectionSource,
};

/// Authored Pipeline v3 configuration contract.
pub const PIPELINE_V3_SCHEMA: &str = "aniflow.pipeline/v3";
/// Resolved deterministic plan contract.
pub const PIPELINE_PLAN_SCHEMA_V1: &str = "aniflow.pipeline-plan/v1";
/// Typed planning failure contract.
pub const PIPELINE_PLANNING_FAILURE_SCHEMA_V1: &str = "aniflow.pipeline-planning-failure/v1";
/// Portable provider registration document contract.
pub const PROVIDER_REGISTRATION_SCHEMA_V1: &str = "aniflow.provider-registration/v1";
/// Canonical JSON algorithm identity used by digested v3 contracts.
pub const CANONICAL_JSON_SCHEMA_V1: &str = "aniflow.canonical-json/v1";

/// Alias spelling for callers that name the configuration contract explicitly.
pub const PIPELINE_V3_CONFIGURATION_SCHEMA: &str = PIPELINE_V3_SCHEMA;
/// Alias spelling for callers that name the plan contract explicitly.
pub const PIPELINE_V3_PLAN_SCHEMA_V1: &str = PIPELINE_PLAN_SCHEMA_V1;

/// One immutable source artifact declared by a Pipeline v3 configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredPipelineInput {
    pub id: String,
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_role: Option<StreamRole>,
}

/// Capability identity and acceptable semantic versions for one stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequirement {
    pub id: String,
    pub version_requirement: String,
}

/// Explicit replacement, primary, and ordered fallback provider policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSelectionIntent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<ProviderCandidate>,
    pub primary: ProviderCandidate,
    #[serde(default)]
    pub fallbacks: Vec<ProviderCandidate>,
}

/// Artifact ids assigned to one declared capability input port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredInputBinding {
    pub port: String,
    pub artifacts: Vec<String>,
}

/// One future immutable artifact and its workspace-confined destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedArtifact {
    pub id: String,
    pub relative_path: String,
}

/// Expected artifacts assigned to one declared capability output port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredOutputBinding {
    pub port: String,
    pub artifacts: Vec<ExpectedArtifact>,
}

/// Named validation that must accept an immutable artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactValidation {
    pub id: String,
    pub artifact: String,
    pub contract: String,
}

/// One ordered, dependency-aware Pipeline v3 stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredPipelineStage {
    pub id: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub capability: CapabilityRequirement,
    pub provider: ProviderSelectionIntent,
    pub inputs: Vec<AuthoredInputBinding>,
    pub outputs: Vec<AuthoredOutputBinding>,
    #[serde(default)]
    pub validations: Vec<ArtifactValidation>,
}

/// A named public output and the validations required before it is usable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalOutputRequirement {
    pub id: String,
    pub artifact: String,
    #[serde(default)]
    pub required_validations: Vec<String>,
}

/// Closed authored configuration for deterministic Pipeline v3 planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3Configuration {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub inputs: Vec<AuthoredPipelineInput>,
    pub stages: Vec<AuthoredPipelineStage>,
    pub outputs: Vec<FinalOutputRequirement>,
}

/// Ergonomic public name for the authored v3 contract.
pub type PipelineV3 = PipelineV3Configuration;

/// Runtime source binding. The host path is read during planning but is never
/// serialized into the resolved plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineInputBinding {
    pub artifact_id: String,
    pub path: PathBuf,
}

impl PipelineInputBinding {
    #[must_use]
    pub fn new(artifact_id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            artifact_id: artifact_id.into(),
            path: path.into(),
        }
    }
}

/// Explicit host observation and authorization policy used for every stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelinePlanningContext {
    pub host: HostResources,
    #[serde(default)]
    pub allowed_side_effects: Vec<SideEffect>,
    #[serde(default)]
    pub offline: bool,
}

/// Stable reason code for a failed planning precondition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PipelinePlanningDiagnosticCode {
    InvalidYaml,
    InvalidJson,
    InvalidSchema,
    UnsupportedVersion,
    LegacyPipelineV2,
    RenderflowRemoved,
    InvalidConfiguration,
    InvalidIdentifier,
    DuplicateIdentifier,
    InvalidDependency,
    UnknownDependency,
    ForwardDependency,
    UnknownArtifact,
    InvalidBinding,
    UnknownPort,
    CardinalityMismatch,
    ArtifactTypeMismatch,
    StreamRoleMismatch,
    UnsafeArtifactPath,
    ArtifactPathOverlap,
    InvalidValidation,
    InvalidFinalOutput,
    MissingInputBinding,
    UnexpectedInputBinding,
    InputUnavailable,
    UnsupportedInputKind,
    SymlinkInput,
    InvalidPolicy,
    ProviderUnavailable,
    InvalidProviderRegistration,
    InvalidResolvedPlan,
    DigestMismatch,
    Io,
}

/// One structured, non-secret planning diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelinePlanningDiagnostic {
    pub code: PipelinePlanningDiagnosticCode,
    pub category: ErrorCategory,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<ProviderResolutionAttempt>,
}

/// Serializable typed failure returned before execution can start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelinePlanningFailure {
    pub schema: String,
    pub category: ErrorCategory,
    pub message: String,
    pub diagnostics: Vec<PipelinePlanningDiagnostic>,
}

impl PipelinePlanningFailure {
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        self.category
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[PipelinePlanningDiagnostic] {
        &self.diagnostics
    }

    /// Decode and validate a typed planning failure JSON document.
    pub fn from_json_slice(input: &[u8]) -> std::result::Result<Self, PipelinePlanningFailure> {
        let failure: Self = serde_json::from_slice(input).map_err(|error| {
            planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidJson,
                format!("invalid Pipeline v3 planning failure JSON: {error}"),
                None,
                None,
            )
        })?;
        let value: serde_json::Value = serde_json::from_slice(input).map_err(|error| {
            planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidJson,
                format!("invalid Pipeline v3 planning failure JSON: {error}"),
                None,
                None,
            )
        })?;
        if json_contains_null(&value) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                "Pipeline v3 planning failure JSON must not contain null values",
                None,
                "failure",
            ));
        }
        failure.validate()?;
        Ok(failure)
    }

    /// Validate the failure envelope and its primary diagnostic consistency.
    pub fn validate(&self) -> std::result::Result<(), PipelinePlanningFailure> {
        if self.schema != PIPELINE_PLANNING_FAILURE_SCHEMA_V1 {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidSchema,
                format!(
                    "unsupported planning failure schema; expected {PIPELINE_PLANNING_FAILURE_SCHEMA_V1}"
                ),
                None,
                Some("schema"),
            ));
        }
        if self.message.trim().is_empty()
            || self.message.chars().any(char::is_control)
            || self.diagnostics.is_empty()
        {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                "planning failure must contain a message and at least one diagnostic",
                None,
                Some("diagnostics"),
            ));
        }
        let first = &self.diagnostics[0];
        if first.category != self.category || first.message != self.message {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                "planning failure category and message must match its first diagnostic",
                None,
                Some("diagnostics[0]"),
            ));
        }
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if diagnostic.message.trim().is_empty()
                || diagnostic.message.chars().any(char::is_control)
            {
                return Err(planning_failure(
                    ErrorCategory::Configuration,
                    PipelinePlanningDiagnosticCode::InvalidConfiguration,
                    format!("planning diagnostic {index} message must be non-empty and printable"),
                    None,
                    Some("diagnostics"),
                ));
            }
            for (field, value) in [
                ("stage", diagnostic.stage.as_deref()),
                ("field", diagnostic.field.as_deref()),
            ] {
                if value.is_some_and(|value| {
                    value.trim().is_empty() || value.chars().any(char::is_control)
                }) {
                    return Err(planning_failure(
                        ErrorCategory::Configuration,
                        PipelinePlanningDiagnosticCode::InvalidConfiguration,
                        format!(
                            "planning diagnostic {index} {field} must be non-empty and printable"
                        ),
                        None,
                        Some("diagnostics"),
                    ));
                }
            }
            if diagnostic.code == PipelinePlanningDiagnosticCode::ProviderUnavailable {
                if diagnostic.category != ErrorCategory::Dependency {
                    return Err(planning_failure(
                        ErrorCategory::Configuration,
                        PipelinePlanningDiagnosticCode::InvalidConfiguration,
                        format!(
                            "provider_unavailable diagnostic {index} must use the dependency category"
                        ),
                        None,
                        Some("diagnostics"),
                    ));
                }
                validate_failed_resolution_attempts(&diagnostic.attempts)?;
            } else if !diagnostic.attempts.is_empty() {
                return Err(planning_failure(
                    ErrorCategory::Configuration,
                    PipelinePlanningDiagnosticCode::InvalidConfiguration,
                    format!(
                        "planning diagnostic {index} retains provider attempts without a provider_unavailable code"
                    ),
                    None,
                    Some("diagnostics"),
                ));
            }
        }
        Ok(())
    }

    fn one(
        category: ErrorCategory,
        code: PipelinePlanningDiagnosticCode,
        message: impl Into<String>,
        stage: Option<&str>,
        field: Option<&str>,
        attempts: Vec<ProviderResolutionAttempt>,
    ) -> Self {
        let message = message.into();
        Self {
            schema: PIPELINE_PLANNING_FAILURE_SCHEMA_V1.to_owned(),
            category,
            message: message.clone(),
            diagnostics: vec![PipelinePlanningDiagnostic {
                code,
                category,
                message,
                stage: stage.map(str::to_owned),
                field: field.map(str::to_owned),
                attempts,
            }],
        }
    }
}

fn validate_failed_resolution_attempts(
    attempts: &[ProviderResolutionAttempt],
) -> std::result::Result<(), PipelinePlanningFailure> {
    if attempts.is_empty() {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "provider_unavailable diagnostic must retain every failed resolution attempt",
            None,
            "diagnostics.attempts",
        ));
    }
    let mut registrations = BTreeSet::new();
    let mut saw_primary = false;
    let mut next_fallback = 0_u32;
    for (index, attempt) in attempts.iter().enumerate() {
        require_token(
            &attempt.candidate.registration_id,
            "candidate registration_id",
        )
        .map_err(|error| {
            configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                error.to_string(),
                None,
                "diagnostics.attempts.candidate.registration_id",
            )
        })?;
        if !registrations.insert(attempt.candidate.registration_id.as_str()) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                "provider resolution failure repeats a candidate registration",
                None,
                "diagnostics.attempts",
            ));
        }
        match attempt.selection.source {
            ProviderSelectionSource::Replacement
                if index == 0 && attempt.selection.fallback_index.is_none() && !saw_primary => {}
            ProviderSelectionSource::Primary
                if !saw_primary && attempt.selection.fallback_index.is_none() =>
            {
                saw_primary = true;
            }
            ProviderSelectionSource::Fallback
                if saw_primary && attempt.selection.fallback_index == Some(next_fallback) =>
            {
                next_fallback += 1;
            }
            _ => {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidConfiguration,
                    "provider resolution failure attempts violate replacement, primary, fallback order",
                    None,
                    "diagnostics.attempts.selection",
                ));
            }
        }
        if attempt.available || attempt.reasons.is_empty() {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                "provider_unavailable attempts must all be unavailable with at least one reason",
                None,
                "diagnostics.attempts",
            ));
        }
        for reason in &attempt.reasons {
            if reason.detail.trim().is_empty() || reason.detail.chars().any(char::is_control) {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidConfiguration,
                    "provider availability reason detail must be non-empty and printable",
                    None,
                    "diagnostics.attempts.reasons.detail",
                ));
            }
        }
        let codes = attempt
            .reasons
            .iter()
            .map(|reason| reason.code)
            .collect::<Vec<_>>();
        validate_availability_code_combination(&codes, None).map_err(|failure| {
            configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                failure.message,
                None,
                "diagnostics.attempts.reasons.code",
            )
        })?;
    }
    if !saw_primary {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "provider resolution failure attempts must include the primary candidate",
            None,
            "diagnostics.attempts",
        ));
    }
    Ok(())
}

impl fmt::Display for PipelinePlanningFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PipelinePlanningFailure {}

/// Serializable locator document for one explicit provider registration.
///
/// All three locators are portable relative paths interpreted relative to the
/// document file. The private base is intentionally omitted on serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRegistrationDocument {
    pub schema: String,
    pub registration_id: String,
    pub manifest: String,
    pub configuration: String,
    pub executable: String,
    pub implementation_id: String,
    #[serde(default)]
    pub components: ComponentInventory,
    #[serde(skip)]
    base_directory: Option<PathBuf>,
}

impl ProviderRegistrationDocument {
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let document: Self = decode_json(input, "provider registration")?;
        let value: serde_json::Value = decode_json(input, "provider registration")?;
        if json_contains_null(&value) {
            return Err(invalid(
                "provider registration JSON must not contain null values",
            ));
        }
        document.validate()?;
        Ok(document)
    }

    /// Load a document and bind its locators to the document's directory.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let input = fs::read(path).map_err(|error| {
            Error::new(
                ErrorCategory::Input,
                format!("failed to read provider registration document: {error}"),
            )
        })?;
        let mut document = Self::from_json_slice(&input)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        document.base_directory = Some(fs::canonicalize(parent).map_err(|error| {
            Error::new(
                ErrorCategory::Input,
                format!("failed to resolve provider registration directory: {error}"),
            )
        })?);
        Ok(document)
    }

    /// Parse a document and bind locators relative to the supplied document
    /// path without requiring that the document itself be read again.
    pub fn from_json_slice_at(input: &[u8], document_path: impl AsRef<Path>) -> Result<Self> {
        let mut document = Self::from_json_slice(input)?;
        let parent = document_path
            .as_ref()
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        document.base_directory = Some(fs::canonicalize(parent).map_err(|error| {
            Error::new(
                ErrorCategory::Input,
                format!("failed to resolve provider registration directory: {error}"),
            )
        })?);
        Ok(document)
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, PROVIDER_REGISTRATION_SCHEMA_V1)?;
        require_token(&self.registration_id, "registration_id")?;
        require_token(&self.implementation_id, "implementation_id")?;
        validate_portable_locator(&self.manifest, "manifest")?;
        validate_portable_locator(&self.configuration, "configuration")?;
        validate_portable_locator(&self.executable, "executable")?;
        self.components.validate()
    }

    /// Materialize inert manifest/configuration data into a registration.
    /// This reads and validates local files but never launches the executable.
    pub fn into_registration(self) -> Result<ProviderRegistration> {
        self.validate()?;
        let base = self.base_directory.ok_or_else(|| {
            invalid(
                "provider registration locators have no document directory; use load or from_json_slice_at",
            )
        })?;
        let manifest_path =
            resolve_existing_registration_target(&base, &self.manifest, "manifest")?;
        let configuration_path =
            resolve_existing_registration_target(&base, &self.configuration, "configuration")?;
        let executable_path = resolve_executable_registration_target(&base, &self.executable)?;
        let manifest = ProviderManifest::from_json_slice(&read_registration_json(
            &manifest_path,
            "manifest",
        )?)?;
        let configuration = ProviderConfiguration::from_json_slice(&read_registration_json(
            &configuration_path,
            "configuration",
        )?)?;
        ProviderRegistration::new(
            self.registration_id,
            manifest,
            configuration,
            executable_path,
            self.implementation_id,
            self.components,
        )
    }
}

/// Filesystem shape covered by a source content identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineInputKind {
    File,
    Directory,
}

/// Portable content identity for one runtime-bound source artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineInputIdentity {
    pub id: String,
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_role: Option<StreamRole>,
    pub kind: PipelineInputKind,
    pub file_count: u64,
    pub byte_count: u64,
    pub content_sha256: String,
}

/// Sorted authorization policy and exact host observation bound to a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedPlanningPolicy {
    pub host: HostResources,
    pub allowed_side_effects: Vec<SideEffect>,
    pub offline: bool,
}

/// Output artifact enriched with the exact selected capability port contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedExpectedArtifact {
    pub id: String,
    pub relative_path: String,
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_role: Option<StreamRole>,
}

/// Resolved expected artifacts assigned to a selected output port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedOutputBinding {
    pub port: String,
    pub artifacts: Vec<PlannedExpectedArtifact>,
}

/// Stable, prose-free resolution evidence retained in a plan digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedProviderResolutionAttempt {
    pub candidate: ProviderCandidate,
    pub selection: ProviderSelection,
    pub available: bool,
    pub reason_codes: Vec<AvailabilityCode>,
}

/// One stage after deterministic capability and provider resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPipelineStage {
    pub id: String,
    pub depends_on: Vec<String>,
    pub capability_requirement: CapabilityRequirement,
    pub capability: CapabilityDeclaration,
    pub provider_selection: ProviderSelectionIntent,
    pub resolution_attempts: Vec<PlannedProviderResolutionAttempt>,
    pub provider_lock: ProviderLock,
    pub inputs: Vec<AuthoredInputBinding>,
    pub outputs: Vec<PlannedOutputBinding>,
    pub validations: Vec<ArtifactValidation>,
}

/// Canonical semantic payload covered by `plan_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3PlanPayload {
    pub pipeline_schema: String,
    pub pipeline_name: String,
    pub configuration_sha256: String,
    pub policy: NormalizedPlanningPolicy,
    pub inputs: Vec<PipelineInputIdentity>,
    pub stages: Vec<ResolvedPipelineStage>,
    pub outputs: Vec<FinalOutputRequirement>,
}

/// Self-validating deterministic Pipeline v3 resolved plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineV3Plan {
    pub schema: String,
    pub algorithm: String,
    pub canonicalization: String,
    pub plan_sha256: String,
    pub payload: PipelineV3PlanPayload,
}

/// Ergonomic public name for a resolved Pipeline v3 plan.
pub type ResolvedPipelinePlan = PipelineV3Plan;

impl PipelineV3Configuration {
    /// Load, parse, and validate a Pipeline v3 YAML document without creating
    /// any workspace or runtime state.
    pub fn load(path: impl AsRef<Path>) -> std::result::Result<Self, PipelinePlanningFailure> {
        let input = fs::read(path.as_ref()).map_err(|error| {
            planning_failure(
                ErrorCategory::Input,
                PipelinePlanningDiagnosticCode::Io,
                format!("failed to read Pipeline v3 configuration: {error}"),
                None,
                None,
            )
        })?;
        Self::from_yaml_slice(&input)
    }

    /// Parse and validate a strict Pipeline v3 YAML document.
    pub fn from_yaml_slice(input: &[u8]) -> std::result::Result<Self, PipelinePlanningFailure> {
        let value: serde_yaml::Value = serde_yaml::from_slice(input).map_err(|error| {
            planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidYaml,
                format!("invalid Pipeline v3 YAML: {error}"),
                None,
                None,
            )
        })?;
        if yaml_contains_null(&value) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                "Pipeline v3 configuration must not contain null values",
                None,
                "configuration",
            ));
        }
        diagnose_pipeline_version(&value)?;
        let configuration: Self = serde_yaml::from_value(value).map_err(|error| {
            planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                format!("invalid Pipeline v3 configuration: {error}"),
                None,
                None,
            )
        })?;
        configuration.validate()?;
        Ok(configuration)
    }

    /// Parse and validate a strict Pipeline v3 YAML string.
    pub fn from_yaml_str(input: &str) -> std::result::Result<Self, PipelinePlanningFailure> {
        Self::from_yaml_slice(input.as_bytes())
    }

    /// Validate graph, artifact, path, selection, and final-output invariants
    /// that do not require a provider registry.
    pub fn validate(&self) -> std::result::Result<(), PipelinePlanningFailure> {
        validate_authored_configuration(self)
    }

    /// Digest semantic authored intent. Human description text is deliberately
    /// excluded, while every ordered stage and selection decision is retained.
    pub fn configuration_sha256(&self) -> std::result::Result<String, PipelinePlanningFailure> {
        self.validate()?;
        canonical_sha256(&SemanticConfiguration::from(self)).map_err(|error| {
            planning_failure(
                ErrorCategory::Internal,
                PipelinePlanningDiagnosticCode::DigestMismatch,
                error.to_string(),
                None,
                None,
            )
        })
    }
}

impl PipelineV3Plan {
    fn new(payload: PipelineV3PlanPayload) -> std::result::Result<Self, PipelinePlanningFailure> {
        validate_plan_payload(&payload)?;
        let plan_sha256 = canonical_sha256(&payload).map_err(|error| {
            planning_failure(
                ErrorCategory::Internal,
                PipelinePlanningDiagnosticCode::DigestMismatch,
                error.to_string(),
                None,
                None,
            )
        })?;
        Ok(Self {
            schema: PIPELINE_PLAN_SCHEMA_V1.to_owned(),
            algorithm: "sha256".to_owned(),
            canonicalization: CANONICAL_JSON_SCHEMA_V1.to_owned(),
            plan_sha256,
            payload,
        })
    }

    /// Decode and fully validate a resolved plan JSON document.
    pub fn from_json_slice(input: &[u8]) -> std::result::Result<Self, PipelinePlanningFailure> {
        // Deserialize directly first so serde can reject duplicate object
        // fields before a generic JSON value would collapse them.
        let plan: Self = serde_json::from_slice(input).map_err(|error| {
            planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidJson,
                format!("invalid Pipeline v3 plan JSON: {error}"),
                None,
                None,
            )
        })?;
        let value: serde_json::Value = serde_json::from_slice(input).map_err(|error| {
            planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidJson,
                format!("invalid Pipeline v3 plan JSON: {error}"),
                None,
                None,
            )
        })?;
        let normalized = serde_json::to_value(&plan).map_err(|error| {
            planning_failure(
                ErrorCategory::Internal,
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!("failed to normalize Pipeline v3 plan: {error}"),
                None,
                None,
            )
        })?;
        if value != normalized {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                "resolved plan JSON must use the normalized contract representation",
                None,
                "plan",
            ));
        }
        plan.validate()?;
        Ok(plan)
    }

    /// Recheck the schema, semantic payload, embedded locks, and digest.
    pub fn validate(&self) -> std::result::Result<(), PipelinePlanningFailure> {
        if self.schema != PIPELINE_PLAN_SCHEMA_V1 {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidSchema,
                format!("unsupported resolved plan schema; expected {PIPELINE_PLAN_SCHEMA_V1}"),
                None,
                Some("schema"),
            ));
        }
        if self.algorithm != "sha256" {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                "unsupported plan digest algorithm; expected sha256",
                None,
                Some("algorithm"),
            ));
        }
        if self.canonicalization != CANONICAL_JSON_SCHEMA_V1 {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!("unsupported plan canonicalization; expected {CANONICAL_JSON_SCHEMA_V1}"),
                None,
                Some("canonicalization"),
            ));
        }
        validate_digest(&self.plan_sha256, "plan_sha256", None)?;
        validate_plan_payload(&self.payload)?;
        let expected = canonical_sha256(&self.payload).map_err(|error| {
            planning_failure(
                ErrorCategory::Internal,
                PipelinePlanningDiagnosticCode::DigestMismatch,
                error.to_string(),
                None,
                Some("plan_sha256"),
            )
        })?;
        if expected != self.plan_sha256 {
            return Err(planning_failure(
                ErrorCategory::Configuration,
                PipelinePlanningDiagnosticCode::DigestMismatch,
                format!(
                    "resolved plan digest does not match its canonical payload; expected {expected}"
                ),
                None,
                Some("plan_sha256"),
            ));
        }
        Ok(())
    }

    /// Encode the complete plan with recursively sorted JSON object keys.
    pub fn canonical_json_bytes(&self) -> std::result::Result<Vec<u8>, PipelinePlanningFailure> {
        self.validate()?;
        canonical_json_bytes(self).map_err(|error| {
            planning_failure(
                ErrorCategory::Internal,
                PipelinePlanningDiagnosticCode::DigestMismatch,
                error.to_string(),
                None,
                None,
            )
        })
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SemanticConfiguration<'a> {
    schema: &'a str,
    name: &'a str,
    inputs: &'a [AuthoredPipelineInput],
    stages: &'a [AuthoredPipelineStage],
    outputs: &'a [FinalOutputRequirement],
}

impl<'a> From<&'a PipelineV3Configuration> for SemanticConfiguration<'a> {
    fn from(configuration: &'a PipelineV3Configuration) -> Self {
        Self {
            schema: &configuration.schema,
            name: &configuration.name,
            inputs: &configuration.inputs,
            stages: &configuration.stages,
            outputs: &configuration.outputs,
        }
    }
}

#[derive(Debug, Clone)]
struct ArtifactContract {
    artifact_type: String,
    stream_role: Option<StreamRole>,
    producer: Option<String>,
}

fn validate_authored_configuration(
    configuration: &PipelineV3Configuration,
) -> std::result::Result<(), PipelinePlanningFailure> {
    if configuration.schema != PIPELINE_V3_SCHEMA {
        return Err(planning_failure(
            ErrorCategory::Configuration,
            PipelinePlanningDiagnosticCode::InvalidSchema,
            format!("unsupported pipeline schema; expected {PIPELINE_V3_SCHEMA}"),
            None,
            Some("schema"),
        ));
    }
    if configuration.name.trim().is_empty() || configuration.name.chars().any(char::is_control) {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "pipeline name must be non-empty and printable",
            None,
            "name",
        ));
    }
    if configuration.inputs.is_empty() {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "Pipeline v3 must declare at least one input",
            None,
            "inputs",
        ));
    }
    if configuration.stages.is_empty() {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "Pipeline v3 must declare at least one stage",
            None,
            "stages",
        ));
    }
    if configuration.outputs.is_empty() {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidFinalOutput,
            "Pipeline v3 must declare at least one final output",
            None,
            "outputs",
        ));
    }

    let mut artifacts = BTreeMap::<String, ArtifactContract>::new();
    for (index, input) in configuration.inputs.iter().enumerate() {
        validate_local_id(&input.id, "input id", None, &format!("inputs[{index}].id"))?;
        validate_artifact_type(
            &input.artifact_type,
            None,
            &format!("inputs[{index}].artifact_type"),
        )?;
        if artifacts
            .insert(
                input.id.clone(),
                ArtifactContract {
                    artifact_type: input.artifact_type.clone(),
                    stream_role: input.stream_role,
                    producer: None,
                },
            )
            .is_some()
        {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::DuplicateIdentifier,
                format!("artifact id {} is declared more than once", input.id),
                None,
                &format!("inputs[{index}].id"),
            ));
        }
    }

    let mut all_stage_ids = BTreeSet::new();
    for (index, stage) in configuration.stages.iter().enumerate() {
        validate_local_id(&stage.id, "stage id", None, &format!("stages[{index}].id"))?;
        if !all_stage_ids.insert(stage.id.as_str()) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::DuplicateIdentifier,
                format!("stage id {} is declared more than once", stage.id),
                Some(&stage.id),
                &format!("stages[{index}].id"),
            ));
        }
    }

    let mut completed_stages = BTreeSet::<String>::new();
    let mut ancestors = BTreeMap::<String, BTreeSet<String>>::new();
    let mut validation_artifacts = BTreeMap::<String, String>::new();
    let mut artifact_paths = Vec::<(String, String)>::new();

    for (stage_index, stage) in configuration.stages.iter().enumerate() {
        let stage_field = |suffix: &str| {
            if suffix.is_empty() {
                format!("stages[{stage_index}]")
            } else {
                format!("stages[{stage_index}].{suffix}")
            }
        };
        let mut direct_dependencies = BTreeSet::new();
        let mut stage_ancestors = BTreeSet::new();
        for (dependency_index, dependency) in stage.depends_on.iter().enumerate() {
            if !direct_dependencies.insert(dependency.as_str()) {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidDependency,
                    format!(
                        "stage {} declares dependency {dependency} more than once",
                        stage.id
                    ),
                    Some(&stage.id),
                    &format!("{}.depends_on[{dependency_index}]", stage_field("")),
                ));
            }
            if !completed_stages.contains(dependency) {
                let (code, message) = if all_stage_ids.contains(dependency.as_str()) {
                    (
                        PipelinePlanningDiagnosticCode::ForwardDependency,
                        format!(
                            "stage {} depends on {dependency}, which is not earlier in the ordered stage list",
                            stage.id
                        ),
                    )
                } else {
                    (
                        PipelinePlanningDiagnosticCode::UnknownDependency,
                        format!("stage {} depends on unknown stage {dependency}", stage.id),
                    )
                };
                return Err(configuration_failure(
                    code,
                    message,
                    Some(&stage.id),
                    &format!("{}.depends_on[{dependency_index}]", stage_field("")),
                ));
            }
            stage_ancestors.insert(dependency.clone());
            if let Some(transitive) = ancestors.get(dependency) {
                stage_ancestors.extend(transitive.iter().cloned());
            }
        }

        validate_capability_requirement(&stage.capability, &stage.id, stage_index)?;
        validate_provider_selection(&stage.provider, &stage.id, stage_index)?;

        let mut input_ports = BTreeSet::new();
        for (binding_index, binding) in stage.inputs.iter().enumerate() {
            validate_local_id(
                &binding.port,
                "input port",
                Some(&stage.id),
                &format!("{}.inputs[{binding_index}].port", stage_field("")),
            )?;
            if !input_ports.insert(binding.port.as_str()) {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidBinding,
                    format!(
                        "stage {} binds input port {} more than once",
                        stage.id, binding.port
                    ),
                    Some(&stage.id),
                    &format!("{}.inputs[{binding_index}].port", stage_field("")),
                ));
            }
            if binding.artifacts.is_empty() {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidBinding,
                    format!(
                        "stage {} input port {} must bind at least one artifact",
                        stage.id, binding.port
                    ),
                    Some(&stage.id),
                    &format!("{}.inputs[{binding_index}].artifacts", stage_field("")),
                ));
            }
            let mut bound = BTreeSet::new();
            for (artifact_index, artifact_id) in binding.artifacts.iter().enumerate() {
                if !bound.insert(artifact_id.as_str()) {
                    return Err(configuration_failure(
                        PipelinePlanningDiagnosticCode::InvalidBinding,
                        format!(
                            "stage {} binds artifact {artifact_id} more than once on port {}",
                            stage.id, binding.port
                        ),
                        Some(&stage.id),
                        &format!(
                            "{}.inputs[{binding_index}].artifacts[{artifact_index}]",
                            stage_field("")
                        ),
                    ));
                }
                let contract = artifacts.get(artifact_id).ok_or_else(|| {
                    configuration_failure(
                        PipelinePlanningDiagnosticCode::UnknownArtifact,
                        format!(
                            "stage {} input port {} references unknown artifact {artifact_id}",
                            stage.id, binding.port
                        ),
                        Some(&stage.id),
                        &format!(
                            "{}.inputs[{binding_index}].artifacts[{artifact_index}]",
                            stage_field("")
                        ),
                    )
                })?;
                if let Some(producer) = &contract.producer {
                    if !stage_ancestors.contains(producer) {
                        return Err(configuration_failure(
                            PipelinePlanningDiagnosticCode::InvalidDependency,
                            format!(
                                "stage {} consumes artifact {artifact_id} from {producer} without depending on it",
                                stage.id
                            ),
                            Some(&stage.id),
                            &format!(
                                "{}.inputs[{binding_index}].artifacts[{artifact_index}]",
                                stage_field("")
                            ),
                        ));
                    }
                }
            }
        }

        let mut output_ports = BTreeSet::new();
        let mut stage_output_ids = BTreeSet::new();
        for (binding_index, binding) in stage.outputs.iter().enumerate() {
            validate_local_id(
                &binding.port,
                "output port",
                Some(&stage.id),
                &format!("{}.outputs[{binding_index}].port", stage_field("")),
            )?;
            if !output_ports.insert(binding.port.as_str()) {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidBinding,
                    format!(
                        "stage {} binds output port {} more than once",
                        stage.id, binding.port
                    ),
                    Some(&stage.id),
                    &format!("{}.outputs[{binding_index}].port", stage_field("")),
                ));
            }
            if binding.artifacts.is_empty() {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidBinding,
                    format!(
                        "stage {} output port {} must declare at least one artifact",
                        stage.id, binding.port
                    ),
                    Some(&stage.id),
                    &format!("{}.outputs[{binding_index}].artifacts", stage_field("")),
                ));
            }
            for (artifact_index, artifact) in binding.artifacts.iter().enumerate() {
                validate_local_id(
                    &artifact.id,
                    "artifact id",
                    Some(&stage.id),
                    &format!(
                        "{}.outputs[{binding_index}].artifacts[{artifact_index}].id",
                        stage_field("")
                    ),
                )?;
                if artifacts.contains_key(&artifact.id)
                    || !stage_output_ids.insert(artifact.id.clone())
                {
                    return Err(configuration_failure(
                        PipelinePlanningDiagnosticCode::DuplicateIdentifier,
                        format!("artifact id {} is declared more than once", artifact.id),
                        Some(&stage.id),
                        &format!(
                            "{}.outputs[{binding_index}].artifacts[{artifact_index}].id",
                            stage_field("")
                        ),
                    ));
                }
                validate_artifact_relative_path(&artifact.relative_path, Some(&stage.id))?;
                for (other_id, other_path) in &artifact_paths {
                    if paths_overlap(other_path, &artifact.relative_path) {
                        return Err(configuration_failure(
                            PipelinePlanningDiagnosticCode::ArtifactPathOverlap,
                            format!(
                                "artifact {} path {} overlaps artifact {other_id} path {other_path}",
                                artifact.id, artifact.relative_path
                            ),
                            Some(&stage.id),
                            &format!(
                                "{}.outputs[{binding_index}].artifacts[{artifact_index}].relative_path",
                                stage_field("")
                            ),
                        ));
                    }
                }
                artifact_paths.push((artifact.id.clone(), artifact.relative_path.clone()));
            }
        }

        // Capability resolution supplies output type/role/stream contracts.
        // Temporary placeholders are sufficient for graph validation here.
        for artifact_id in &stage_output_ids {
            artifacts.insert(
                artifact_id.clone(),
                ArtifactContract {
                    artifact_type: String::new(),
                    stream_role: None,
                    producer: Some(stage.id.clone()),
                },
            );
        }

        for (validation_index, validation) in stage.validations.iter().enumerate() {
            validate_local_id(
                &validation.id,
                "validation id",
                Some(&stage.id),
                &format!("{}.validations[{validation_index}].id", stage_field("")),
            )?;
            if !stage_output_ids.contains(&validation.artifact) {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidValidation,
                    format!(
                        "stage {} validation {} must reference an artifact produced by that stage",
                        stage.id, validation.id
                    ),
                    Some(&stage.id),
                    &format!(
                        "{}.validations[{validation_index}].artifact",
                        stage_field("")
                    ),
                ));
            }
            if validation.contract.trim().is_empty()
                || validation.contract.chars().any(char::is_control)
            {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidValidation,
                    format!(
                        "stage {} validation {} contract must be non-empty and printable",
                        stage.id, validation.id
                    ),
                    Some(&stage.id),
                    &format!(
                        "{}.validations[{validation_index}].contract",
                        stage_field("")
                    ),
                ));
            }
            if validation_artifacts
                .insert(validation.id.clone(), validation.artifact.clone())
                .is_some()
            {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::DuplicateIdentifier,
                    format!("validation id {} is declared more than once", validation.id),
                    Some(&stage.id),
                    &format!("{}.validations[{validation_index}].id", stage_field("")),
                ));
            }
        }

        ancestors.insert(stage.id.clone(), stage_ancestors);
        completed_stages.insert(stage.id.clone());
    }

    let mut output_ids = BTreeSet::new();
    for (index, output) in configuration.outputs.iter().enumerate() {
        validate_local_id(
            &output.id,
            "output id",
            None,
            &format!("outputs[{index}].id"),
        )?;
        if !output_ids.insert(output.id.as_str()) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::DuplicateIdentifier,
                format!("final output id {} is declared more than once", output.id),
                None,
                &format!("outputs[{index}].id"),
            ));
        }
        if !artifacts.contains_key(&output.artifact) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidFinalOutput,
                format!(
                    "final output {} references unknown artifact {}",
                    output.id, output.artifact
                ),
                None,
                &format!("outputs[{index}].artifact"),
            ));
        }
        if output.required_validations.is_empty() {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidFinalOutput,
                format!(
                    "final output {} must require at least one validation",
                    output.id
                ),
                None,
                &format!("outputs[{index}].required_validations"),
            ));
        }
        let mut required = BTreeSet::new();
        for (validation_index, validation_id) in output.required_validations.iter().enumerate() {
            if !required.insert(validation_id.as_str()) {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidFinalOutput,
                    format!(
                        "final output {} requires validation {validation_id} more than once",
                        output.id
                    ),
                    None,
                    &format!("outputs[{index}].required_validations[{validation_index}]"),
                ));
            }
            match validation_artifacts.get(validation_id) {
                Some(artifact) if artifact == &output.artifact => {}
                Some(_) => {
                    return Err(configuration_failure(
                        PipelinePlanningDiagnosticCode::InvalidFinalOutput,
                        format!(
                            "final output {} requires validation {validation_id} for a different artifact",
                            output.id
                        ),
                        None,
                        &format!("outputs[{index}].required_validations[{validation_index}]"),
                    ));
                }
                None => {
                    return Err(configuration_failure(
                        PipelinePlanningDiagnosticCode::InvalidFinalOutput,
                        format!(
                            "final output {} requires unknown validation {validation_id}",
                            output.id
                        ),
                        None,
                        &format!("outputs[{index}].required_validations[{validation_index}]"),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_capability_requirement(
    capability: &CapabilityRequirement,
    stage: &str,
    stage_index: usize,
) -> std::result::Result<(), PipelinePlanningFailure> {
    validate_capability_id(&capability.id).map_err(|error| {
        configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidIdentifier,
            error.to_string(),
            Some(stage),
            &format!("stages[{stage_index}].capability.id"),
        )
    })?;
    if capability.version_requirement.chars().any(char::is_control) {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "capability version requirement must not contain control characters",
            Some(stage),
            &format!("stages[{stage_index}].capability.version_requirement"),
        ));
    }
    VersionReq::parse(&capability.version_requirement).map_err(|error| {
        configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            format!(
                "invalid capability version requirement {}: {error}",
                capability.version_requirement
            ),
            Some(stage),
            &format!("stages[{stage_index}].capability.version_requirement"),
        )
    })?;
    Ok(())
}

fn validate_provider_selection(
    provider: &ProviderSelectionIntent,
    stage: &str,
    stage_index: usize,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let mut registrations = BTreeSet::new();
    for (field, candidate) in provider
        .replacement
        .iter()
        .map(|candidate| ("replacement", candidate))
        .chain(std::iter::once(("primary", &provider.primary)))
        .chain(
            provider
                .fallbacks
                .iter()
                .map(|candidate| ("fallbacks", candidate)),
        )
    {
        require_token(&candidate.registration_id, "provider registration_id").map_err(|error| {
            configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidIdentifier,
                error.to_string(),
                Some(stage),
                &format!("stages[{stage_index}].provider.{field}"),
            )
        })?;
        if !registrations.insert(candidate.registration_id.as_str()) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::InvalidConfiguration,
                format!(
                    "stage {stage} provider candidate {} appears more than once",
                    candidate.registration_id
                ),
                Some(stage),
                &format!("stages[{stage_index}].provider"),
            ));
        }
    }
    Ok(())
}

/// Load a Pipeline v3 configuration and explicit provider registrations, then
/// resolve them into a deterministic, read-only plan.
///
/// Registration documents are loaded in the supplied order. Every failure at
/// that file boundary is converted into the same typed planning contract used
/// by capability resolution.
pub fn plan_v3(
    pipeline: impl AsRef<Path>,
    input_bindings: &[PipelineInputBinding],
    provider_registration_paths: &[PathBuf],
    context: &PipelinePlanningContext,
) -> std::result::Result<PipelineV3Plan, PipelinePlanningFailure> {
    let configuration = PipelineV3Configuration::load(pipeline)?;
    let mut registry = ProviderRegistry::new();
    for (index, path) in provider_registration_paths.iter().enumerate() {
        let document = ProviderRegistrationDocument::load(path)
            .map_err(|error| provider_registration_failure(error, index))?;
        let registration = document
            .into_registration()
            .map_err(|error| provider_registration_failure(error, index))?;
        registry
            .register(registration)
            .map_err(|error| provider_registration_failure(error, index))?;
    }
    resolve_pipeline_v3(&configuration, input_bindings, &registry, context)
}

fn provider_registration_failure(error: Error, index: usize) -> PipelinePlanningFailure {
    PipelinePlanningFailure::one(
        error.category(),
        PipelinePlanningDiagnosticCode::InvalidProviderRegistration,
        error.to_string(),
        None,
        Some(&format!("provider_registration_paths[{index}]")),
        Vec::new(),
    )
}

/// Resolve all Pipeline v3 stages into a deterministic, read-only plan.
pub fn resolve_pipeline_v3(
    configuration: &PipelineV3Configuration,
    input_bindings: &[PipelineInputBinding],
    registry: &ProviderRegistry,
    context: &PipelinePlanningContext,
) -> std::result::Result<PipelineV3Plan, PipelinePlanningFailure> {
    configuration.validate()?;
    let policy = normalize_policy(context)?;

    let mut artifacts = BTreeMap::<String, ArtifactContract>::new();
    for input in &configuration.inputs {
        artifacts.insert(
            input.id.clone(),
            ArtifactContract {
                artifact_type: input.artifact_type.clone(),
                stream_role: input.stream_role,
                producer: None,
            },
        );
    }

    // Resolve the complete graph before reading potentially large source
    // content. Missing capabilities therefore fail at the cheapest boundary.
    let mut stages = Vec::with_capacity(configuration.stages.len());
    for stage in &configuration.stages {
        let request = ProviderResolutionRequest {
            capability_id: stage.capability.id.clone(),
            capability_version_requirement: stage.capability.version_requirement.clone(),
            replacement: stage.provider.replacement.clone(),
            primary: stage.provider.primary.clone(),
            fallbacks: stage.provider.fallbacks.clone(),
            allowed_side_effects: policy.allowed_side_effects.clone(),
            offline: policy.offline,
            host: policy.host.clone(),
        };
        let resolved = registry.resolve(&request).map_err(|failure| {
            PipelinePlanningFailure::one(
                ErrorCategory::Dependency,
                PipelinePlanningDiagnosticCode::ProviderUnavailable,
                failure.message,
                Some(&stage.id),
                Some("provider"),
                failure.attempts,
            )
        })?;
        let capability = resolved.registration().capability().clone();
        let outputs = resolve_stage_bindings(stage, &capability, &artifacts)?;

        for binding in &outputs {
            for artifact in &binding.artifacts {
                artifacts.insert(
                    artifact.id.clone(),
                    ArtifactContract {
                        artifact_type: artifact.artifact_type.clone(),
                        stream_role: artifact.stream_role,
                        producer: Some(stage.id.clone()),
                    },
                );
            }
        }

        let resolution_attempts = resolved
            .attempts()
            .iter()
            .map(stable_resolution_attempt)
            .collect();
        stages.push(ResolvedPipelineStage {
            id: stage.id.clone(),
            depends_on: stage.depends_on.clone(),
            capability_requirement: stage.capability.clone(),
            capability,
            provider_selection: stage.provider.clone(),
            resolution_attempts,
            provider_lock: resolved.provider_lock().clone(),
            inputs: stage.inputs.clone(),
            outputs,
            validations: stage.validations.clone(),
        });
    }

    let input_identities = resolve_input_identities(configuration, input_bindings)?;

    PipelineV3Plan::new(PipelineV3PlanPayload {
        pipeline_schema: PIPELINE_V3_SCHEMA.to_owned(),
        pipeline_name: configuration.name.clone(),
        configuration_sha256: configuration.configuration_sha256()?,
        policy,
        inputs: input_identities,
        stages,
        outputs: configuration.outputs.clone(),
    })
}

fn stable_resolution_attempt(
    attempt: &ProviderResolutionAttempt,
) -> PlannedProviderResolutionAttempt {
    PlannedProviderResolutionAttempt {
        candidate: attempt.candidate.clone(),
        selection: attempt.selection.clone(),
        available: attempt.available,
        reason_codes: attempt.reasons.iter().map(|reason| reason.code).collect(),
    }
}

fn normalize_policy(
    context: &PipelinePlanningContext,
) -> std::result::Result<NormalizedPlanningPolicy, PipelinePlanningFailure> {
    context.host.validate().map_err(|error| {
        planning_failure(
            ErrorCategory::Configuration,
            PipelinePlanningDiagnosticCode::InvalidPolicy,
            error.to_string(),
            None,
            Some("context.host"),
        )
    })?;
    let mut allowed_side_effects = context.allowed_side_effects.clone();
    allowed_side_effects.sort_unstable();
    allowed_side_effects.dedup();
    if context.offline && allowed_side_effects.contains(&SideEffect::Network) {
        return Err(planning_failure(
            ErrorCategory::Configuration,
            PipelinePlanningDiagnosticCode::InvalidPolicy,
            "offline planning cannot authorize the network side effect",
            None,
            Some("context.allowed_side_effects"),
        ));
    }
    Ok(NormalizedPlanningPolicy {
        host: context.host.clone(),
        allowed_side_effects,
        offline: context.offline,
    })
}

fn resolve_input_identities(
    configuration: &PipelineV3Configuration,
    bindings: &[PipelineInputBinding],
) -> std::result::Result<Vec<PipelineInputIdentity>, PipelinePlanningFailure> {
    let declared = configuration
        .inputs
        .iter()
        .map(|input| input.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut bound = BTreeMap::<&str, &Path>::new();
    for (index, binding) in bindings.iter().enumerate() {
        validate_local_id(
            &binding.artifact_id,
            "runtime input binding id",
            None,
            &format!("input_bindings[{index}].artifact_id"),
        )?;
        if !declared.contains(binding.artifact_id.as_str()) {
            return Err(planning_failure(
                ErrorCategory::Input,
                PipelinePlanningDiagnosticCode::UnexpectedInputBinding,
                format!(
                    "runtime input binding {} is not declared by the pipeline",
                    binding.artifact_id
                ),
                None,
                Some(&format!("input_bindings[{index}].artifact_id")),
            ));
        }
        if bound
            .insert(binding.artifact_id.as_str(), binding.path.as_path())
            .is_some()
        {
            return Err(planning_failure(
                ErrorCategory::Input,
                PipelinePlanningDiagnosticCode::UnexpectedInputBinding,
                format!(
                    "runtime input {} is bound more than once",
                    binding.artifact_id
                ),
                None,
                Some(&format!("input_bindings[{index}].artifact_id")),
            ));
        }
    }

    let mut identities = Vec::with_capacity(configuration.inputs.len());
    for (index, input) in configuration.inputs.iter().enumerate() {
        let path = bound.get(input.id.as_str()).ok_or_else(|| {
            planning_failure(
                ErrorCategory::Input,
                PipelinePlanningDiagnosticCode::MissingInputBinding,
                format!("declared input {} has no runtime binding", input.id),
                None,
                Some(&format!("inputs[{index}]")),
            )
        })?;
        identities.push(hash_pipeline_input(input, path)?);
    }
    Ok(identities)
}

fn resolve_stage_bindings(
    stage: &AuthoredPipelineStage,
    capability: &CapabilityDeclaration,
    artifacts: &BTreeMap<String, ArtifactContract>,
) -> std::result::Result<Vec<PlannedOutputBinding>, PipelinePlanningFailure> {
    let input_ports = capability
        .inputs
        .iter()
        .map(|port| (port.name.as_str(), port))
        .collect::<BTreeMap<_, _>>();
    for binding in &stage.inputs {
        let port = input_ports.get(binding.port.as_str()).ok_or_else(|| {
            stage_configuration_failure(
                PipelinePlanningDiagnosticCode::UnknownPort,
                format!(
                    "stage {} binds undeclared capability input port {}",
                    stage.id, binding.port
                ),
                stage,
                "inputs",
            )
        })?;
        for artifact_id in &binding.artifacts {
            let artifact = artifacts.get(artifact_id).ok_or_else(|| {
                stage_configuration_failure(
                    PipelinePlanningDiagnosticCode::UnknownArtifact,
                    format!(
                        "stage {} input port {} references unknown artifact {artifact_id}",
                        stage.id, binding.port
                    ),
                    stage,
                    "inputs",
                )
            })?;
            validate_artifact_against_port(stage, artifact_id, artifact, port, "input")?;
        }
    }
    for port in &capability.inputs {
        let count = stage
            .inputs
            .iter()
            .find(|binding| binding.port == port.name)
            .map_or(0, |binding| binding.artifacts.len());
        validate_cardinality(stage, port, count, "input")?;
    }

    let output_ports = capability
        .outputs
        .iter()
        .map(|port| (port.name.as_str(), port))
        .collect::<BTreeMap<_, _>>();
    let mut resolved_outputs = Vec::with_capacity(stage.outputs.len());
    for binding in &stage.outputs {
        let port = output_ports.get(binding.port.as_str()).ok_or_else(|| {
            stage_configuration_failure(
                PipelinePlanningDiagnosticCode::UnknownPort,
                format!(
                    "stage {} binds undeclared capability output port {}",
                    stage.id, binding.port
                ),
                stage,
                "outputs",
            )
        })?;
        resolved_outputs.push(PlannedOutputBinding {
            port: binding.port.clone(),
            artifacts: binding
                .artifacts
                .iter()
                .map(|artifact| PlannedExpectedArtifact {
                    id: artifact.id.clone(),
                    relative_path: artifact.relative_path.clone(),
                    artifact_type: port.artifact_type.clone(),
                    artifact_role: port.artifact_role,
                    stream_role: port.stream_role,
                })
                .collect(),
        });
    }
    for port in &capability.outputs {
        let count = stage
            .outputs
            .iter()
            .find(|binding| binding.port == port.name)
            .map_or(0, |binding| binding.artifacts.len());
        validate_cardinality(stage, port, count, "output")?;
    }
    Ok(resolved_outputs)
}

fn validate_cardinality(
    stage: &AuthoredPipelineStage,
    port: &ArtifactPort,
    count: usize,
    direction: &str,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let valid = match port.cardinality {
        ArtifactCardinality::One => count == 1,
        ArtifactCardinality::Optional => count <= 1,
        ArtifactCardinality::OneOrMore => count >= 1,
        ArtifactCardinality::Many => true,
    };
    if valid {
        Ok(())
    } else {
        Err(stage_configuration_failure(
            PipelinePlanningDiagnosticCode::CardinalityMismatch,
            format!(
                "stage {} {direction} port {} with {:?} cardinality received {count} artifacts",
                stage.id, port.name, port.cardinality
            ),
            stage,
            direction,
        ))
    }
}

fn validate_artifact_against_port(
    stage: &AuthoredPipelineStage,
    artifact_id: &str,
    artifact: &ArtifactContract,
    port: &ArtifactPort,
    direction: &str,
) -> std::result::Result<(), PipelinePlanningFailure> {
    if artifact.artifact_type != port.artifact_type {
        return Err(stage_configuration_failure(
            PipelinePlanningDiagnosticCode::ArtifactTypeMismatch,
            format!(
                "stage {} {direction} port {} requires artifact type {}, but {artifact_id} is {}",
                stage.id, port.name, port.artifact_type, artifact.artifact_type
            ),
            stage,
            direction,
        ));
    }
    if artifact.stream_role != port.stream_role {
        return Err(stage_configuration_failure(
            PipelinePlanningDiagnosticCode::StreamRoleMismatch,
            format!(
                "stage {} {direction} port {} requires stream role {:?}, but {artifact_id} is {:?}",
                stage.id, port.name, port.stream_role, artifact.stream_role
            ),
            stage,
            direction,
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct DirectoryContentEntry {
    relative_path: String,
    kind: PipelineInputKind,
    byte_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_sha256: Option<String>,
}

fn hash_pipeline_input(
    declaration: &AuthoredPipelineInput,
    path: &Path,
) -> std::result::Result<PipelineInputIdentity, PipelinePlanningFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        input_failure(
            PipelinePlanningDiagnosticCode::InputUnavailable,
            format!(
                "declared input {} could not be inspected: {error}",
                declaration.id
            ),
            &declaration.id,
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(input_failure(
            PipelinePlanningDiagnosticCode::SymlinkInput,
            format!(
                "declared input {} cannot be a symbolic link",
                declaration.id
            ),
            &declaration.id,
        ));
    }

    let (kind, file_count, byte_count, content_sha256) = if metadata.is_file() {
        let (byte_count, digest) = hash_file(path).map_err(|error| {
            input_failure(
                PipelinePlanningDiagnosticCode::InputUnavailable,
                format!(
                    "declared input {} could not be read: {error}",
                    declaration.id
                ),
                &declaration.id,
            )
        })?;
        (PipelineInputKind::File, 1, byte_count, digest)
    } else if metadata.is_dir() {
        let (file_count, byte_count, digest) = hash_directory(path, &declaration.id)?;
        (PipelineInputKind::Directory, file_count, byte_count, digest)
    } else {
        return Err(input_failure(
            PipelinePlanningDiagnosticCode::UnsupportedInputKind,
            format!(
                "declared input {} must be a regular file or directory",
                declaration.id
            ),
            &declaration.id,
        ));
    };

    Ok(PipelineInputIdentity {
        id: declaration.id.clone(),
        artifact_type: declaration.artifact_type.clone(),
        artifact_role: declaration.artifact_role,
        stream_role: declaration.stream_role,
        kind,
        file_count,
        byte_count,
        content_sha256,
    })
}

fn hash_directory(
    root: &Path,
    artifact_id: &str,
) -> std::result::Result<(u64, u64, String), PipelinePlanningFailure> {
    let mut entries = Vec::new();
    let mut portable_paths = BTreeSet::new();
    let mut file_count = 0_u64;
    let mut total_bytes = 0_u64;
    for entry in WalkDir::new(root).min_depth(1).follow_links(false) {
        let entry = entry.map_err(|_| {
            input_failure(
                PipelinePlanningDiagnosticCode::InputUnavailable,
                format!("declared input {artifact_id} could not be traversed"),
                artifact_id,
            )
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            input_failure(
                PipelinePlanningDiagnosticCode::InputUnavailable,
                format!("declared input {artifact_id} could not be inspected: {error}"),
                artifact_id,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(input_failure(
                PipelinePlanningDiagnosticCode::SymlinkInput,
                format!("declared input {artifact_id} contains a symbolic link"),
                artifact_id,
            ));
        }
        let relative = entry.path().strip_prefix(root).map_err(|_| {
            input_failure(
                PipelinePlanningDiagnosticCode::InputUnavailable,
                format!("declared input {artifact_id} contains an unrelativizable entry"),
                artifact_id,
            )
        })?;
        let relative_path = normalized_filesystem_relative_path(relative).ok_or_else(|| {
            input_failure(
                PipelinePlanningDiagnosticCode::UnsupportedInputKind,
                format!("declared input {artifact_id} contains a non-portable path"),
                artifact_id,
            )
        })?;
        if !portable_paths.insert(relative_path.to_lowercase()) {
            return Err(input_failure(
                PipelinePlanningDiagnosticCode::UnsupportedInputKind,
                format!(
                    "declared input {artifact_id} contains paths that collide under portable case folding"
                ),
                artifact_id,
            ));
        }
        if metadata.is_file() {
            let (byte_count, digest) = hash_file(entry.path()).map_err(|error| {
                input_failure(
                    PipelinePlanningDiagnosticCode::InputUnavailable,
                    format!("declared input {artifact_id} contains an unreadable file: {error}"),
                    artifact_id,
                )
            })?;
            file_count = file_count.checked_add(1).ok_or_else(|| {
                input_failure(
                    PipelinePlanningDiagnosticCode::InputUnavailable,
                    format!("declared input {artifact_id} contains too many files"),
                    artifact_id,
                )
            })?;
            total_bytes = total_bytes.checked_add(byte_count).ok_or_else(|| {
                input_failure(
                    PipelinePlanningDiagnosticCode::InputUnavailable,
                    format!("declared input {artifact_id} is too large to identify"),
                    artifact_id,
                )
            })?;
            entries.push(DirectoryContentEntry {
                relative_path,
                kind: PipelineInputKind::File,
                byte_count,
                content_sha256: Some(digest),
            });
        } else if metadata.is_dir() {
            entries.push(DirectoryContentEntry {
                relative_path,
                kind: PipelineInputKind::Directory,
                byte_count: 0,
                content_sha256: None,
            });
        } else {
            return Err(input_failure(
                PipelinePlanningDiagnosticCode::UnsupportedInputKind,
                format!("declared input {artifact_id} contains an unsupported filesystem entry"),
                artifact_id,
            ));
        }
    }
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let digest = canonical_sha256(&entries).map_err(|error| {
        input_failure(
            PipelinePlanningDiagnosticCode::InputUnavailable,
            format!("declared input {artifact_id} could not be identified: {error}"),
            artifact_id,
        )
    })?;
    Ok((file_count, total_bytes, digest))
}

fn hash_file(path: &Path) -> io::Result<(u64, String)> {
    let mut input = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut byte_count = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        byte_count = byte_count
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("input byte count overflow"))?;
        hasher.update(&buffer[..read]);
    }
    Ok((byte_count, format!("{:x}", hasher.finalize())))
}

fn normalized_filesystem_relative_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return None;
        };
        let part = part.to_str()?;
        if !is_portable_path_segment(part) {
            return None;
        }
        parts.push(part);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn validate_plan_payload(
    payload: &PipelineV3PlanPayload,
) -> std::result::Result<(), PipelinePlanningFailure> {
    if payload.pipeline_schema != PIPELINE_V3_SCHEMA {
        return Err(planning_failure(
            ErrorCategory::Configuration,
            PipelinePlanningDiagnosticCode::InvalidSchema,
            format!(
                "resolved plan targets an unsupported pipeline schema; expected {PIPELINE_V3_SCHEMA}"
            ),
            None,
            Some("payload.pipeline_schema"),
        ));
    }
    if payload.pipeline_name.trim().is_empty()
        || payload.pipeline_name.chars().any(char::is_control)
    {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            "resolved plan pipeline_name must be non-empty and printable",
            None,
            "payload.pipeline_name",
        ));
    }
    validate_digest(&payload.configuration_sha256, "configuration_sha256", None)?;
    validate_normalized_policy(&payload.policy)?;

    for (index, input) in payload.inputs.iter().enumerate() {
        validate_digest(
            &input.content_sha256,
            "content_sha256",
            Some(&format!("payload.inputs[{index}]")),
        )?;
        match input.kind {
            PipelineInputKind::File if input.file_count != 1 => {
                return Err(configuration_failure(
                    PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                    format!("file input {} must report exactly one file", input.id),
                    None,
                    &format!("payload.inputs[{index}].file_count"),
                ));
            }
            PipelineInputKind::File | PipelineInputKind::Directory => {}
        }
    }

    let authored = authored_configuration_from_plan(payload);
    authored.validate()?;
    let expected_configuration = canonical_sha256(&SemanticConfiguration::from(&authored))
        .map_err(|error| {
            planning_failure(
                ErrorCategory::Internal,
                PipelinePlanningDiagnosticCode::DigestMismatch,
                error.to_string(),
                None,
                Some("payload.configuration_sha256"),
            )
        })?;
    if expected_configuration != payload.configuration_sha256 {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::DigestMismatch,
            format!(
                "configuration digest does not match resolved semantic intent; expected {expected_configuration}"
            ),
            None,
            "payload.configuration_sha256",
        ));
    }

    let mut artifacts = BTreeMap::new();
    for input in &payload.inputs {
        artifacts.insert(
            input.id.clone(),
            ArtifactContract {
                artifact_type: input.artifact_type.clone(),
                stream_role: input.stream_role,
                producer: None,
            },
        );
    }
    for stage in &payload.stages {
        validate_resolved_stage(stage, &payload.policy, &artifacts)?;
        for binding in &stage.outputs {
            for artifact in &binding.artifacts {
                artifacts.insert(
                    artifact.id.clone(),
                    ArtifactContract {
                        artifact_type: artifact.artifact_type.clone(),
                        stream_role: artifact.stream_role,
                        producer: Some(stage.id.clone()),
                    },
                );
            }
        }
    }
    Ok(())
}

fn validate_normalized_policy(
    policy: &NormalizedPlanningPolicy,
) -> std::result::Result<(), PipelinePlanningFailure> {
    policy.host.validate().map_err(|error| {
        planning_failure(
            ErrorCategory::Configuration,
            PipelinePlanningDiagnosticCode::InvalidPolicy,
            error.to_string(),
            None,
            Some("payload.policy.host"),
        )
    })?;
    if !policy
        .allowed_side_effects
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidPolicy,
            "resolved allowed_side_effects must be unique and sorted",
            None,
            "payload.policy.allowed_side_effects",
        ));
    }
    if policy.offline && policy.allowed_side_effects.contains(&SideEffect::Network) {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidPolicy,
            "offline planning cannot authorize the network side effect",
            None,
            "payload.policy.allowed_side_effects",
        ));
    }
    Ok(())
}

fn authored_configuration_from_plan(payload: &PipelineV3PlanPayload) -> PipelineV3Configuration {
    PipelineV3Configuration {
        schema: payload.pipeline_schema.clone(),
        name: payload.pipeline_name.clone(),
        description: String::new(),
        inputs: payload
            .inputs
            .iter()
            .map(|input| AuthoredPipelineInput {
                id: input.id.clone(),
                artifact_type: input.artifact_type.clone(),
                artifact_role: input.artifact_role,
                stream_role: input.stream_role,
            })
            .collect(),
        stages: payload
            .stages
            .iter()
            .map(authored_stage_from_plan)
            .collect(),
        outputs: payload.outputs.clone(),
    }
}

fn authored_stage_from_plan(stage: &ResolvedPipelineStage) -> AuthoredPipelineStage {
    AuthoredPipelineStage {
        id: stage.id.clone(),
        depends_on: stage.depends_on.clone(),
        capability: stage.capability_requirement.clone(),
        provider: stage.provider_selection.clone(),
        inputs: stage.inputs.clone(),
        outputs: stage
            .outputs
            .iter()
            .map(|binding| AuthoredOutputBinding {
                port: binding.port.clone(),
                artifacts: binding
                    .artifacts
                    .iter()
                    .map(|artifact| ExpectedArtifact {
                        id: artifact.id.clone(),
                        relative_path: artifact.relative_path.clone(),
                    })
                    .collect(),
            })
            .collect(),
        validations: stage.validations.clone(),
    }
}

fn validate_resolved_stage(
    stage: &ResolvedPipelineStage,
    policy: &NormalizedPlanningPolicy,
    artifacts: &BTreeMap<String, ArtifactContract>,
) -> std::result::Result<(), PipelinePlanningFailure> {
    if stage.capability.id != stage.capability_requirement.id {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} resolved capability {} does not match required capability {}",
                stage.id, stage.capability.id, stage.capability_requirement.id
            ),
            &stage.id,
            "capability",
        ));
    }
    if stage
        .capability_requirement
        .version_requirement
        .chars()
        .any(char::is_control)
    {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} contains a capability version requirement with control characters",
                stage.id
            ),
            &stage.id,
            "capability_requirement.version_requirement",
        ));
    }
    let requirement = VersionReq::parse(&stage.capability_requirement.version_requirement)
        .map_err(|error| {
            stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!(
                    "stage {} contains an invalid capability version requirement: {error}",
                    stage.id
                ),
                &stage.id,
                "capability_requirement.version_requirement",
            )
        })?;
    if stage.capability.version.chars().any(char::is_control) {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} contains a resolved capability version with control characters",
                stage.id
            ),
            &stage.id,
            "capability.version",
        ));
    }
    let capability_version = Version::parse(&stage.capability.version).map_err(|error| {
        stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} contains an invalid resolved capability version: {error}",
                stage.id
            ),
            &stage.id,
            "capability.version",
        )
    })?;
    if !requirement.matches(&capability_version) {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} resolved capability version {} does not satisfy {}",
                stage.id,
                stage.capability.version,
                stage.capability_requirement.version_requirement
            ),
            &stage.id,
            "capability.version",
        ));
    }
    validate_embedded_capability(stage)?;
    stage.provider_lock.validate().map_err(|error| {
        stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            error.to_string(),
            &stage.id,
            "provider_lock",
        )
    })?;
    let lock = &stage.provider_lock.payload;
    if lock.capability.id != stage.capability.id
        || lock.capability.version != stage.capability.version
        || lock.configuration_schema != stage.capability.configuration_schema
    {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} provider lock does not match its resolved capability declaration",
                stage.id
            ),
            &stage.id,
            "provider_lock",
        ));
    }
    if lock.offline != policy.offline {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} provider lock offline policy differs from the plan",
                stage.id
            ),
            &stage.id,
            "provider_lock.payload.offline",
        ));
    }
    let mut declared_effects = stage.capability.behavior.side_effects.clone();
    declared_effects.sort_unstable();
    if lock.authorized_side_effects != declared_effects
        || !lock
            .authorized_side_effects
            .iter()
            .all(|effect| policy.allowed_side_effects.contains(effect))
    {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} provider lock side effects do not match its capability and plan policy",
                stage.id
            ),
            &stage.id,
            "provider_lock.payload.authorized_side_effects",
        ));
    }
    validate_resolved_requirements(stage, policy)?;
    validate_resolution_evidence(stage)?;

    let authored = authored_stage_from_plan(stage);
    let expected_outputs = resolve_stage_bindings(&authored, &stage.capability, artifacts)?;
    if expected_outputs != stage.outputs {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} resolved outputs do not match its selected capability ports",
                stage.id
            ),
            &stage.id,
            "outputs",
        ));
    }
    Ok(())
}

fn validate_resolved_requirements(
    stage: &ResolvedPipelineStage,
    policy: &NormalizedPlanningPolicy,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let compute = &stage.capability.requirements.compute;
    let host = &policy.host;
    let resources_available = host.cpu_threads >= compute.minimum_cpu_threads
        && host.memory_mib >= compute.minimum_memory_mib
        && host.storage_mib >= compute.minimum_storage_mib
        && (compute.gpu != RequirementLevel::Required || host.gpu_available)
        && (compute.network != RequirementLevel::Required || host.network_available)
        && (!policy.offline || compute.network == RequirementLevel::Forbidden);
    if !resources_available {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} capability requirements exceed the plan host or offline policy",
                stage.id
            ),
            &stage.id,
            "capability.requirements.compute",
        ));
    }
    let requirements = &stage.capability.requirements;
    let lock = &stage.provider_lock.payload;
    validate_component_inventory(&stage.id, "tools", &requirements.tools, &lock.tools)?;
    validate_component_inventory(&stage.id, "codecs", &requirements.codecs, &lock.codecs)?;
    validate_component_inventory(&stage.id, "models", &requirements.models, &lock.models)
}

fn validate_component_inventory(
    stage: &str,
    kind: &str,
    requirements: &[ComponentRequirement],
    inventory: &[ComponentIdentity],
) -> std::result::Result<(), PipelinePlanningFailure> {
    if requirements.len() != inventory.len()
        || !inventory
            .windows(2)
            .all(|pair| (&pair[0].id, &pair[0].version) < (&pair[1].id, &pair[1].version))
    {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {stage} locked {kind} inventory is not the exact sorted requirement set"
            ),
            stage,
            &format!("provider_lock.payload.{kind}"),
        ));
    }
    for requirement in requirements {
        let component = inventory
            .iter()
            .find(|component| component.id == requirement.id)
            .ok_or_else(|| {
                stage_configuration_failure_from_id(
                    PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                    format!(
                        "stage {stage} locked {kind} inventory omits {}",
                        requirement.id
                    ),
                    stage,
                    &format!("provider_lock.payload.{kind}"),
                )
            })?;
        if requirement
            .version_requirement
            .chars()
            .any(char::is_control)
        {
            return Err(stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!("stage {stage} has a {kind} version requirement with control characters"),
                stage,
                "capability.requirements",
            ));
        }
        let version_requirement =
            VersionReq::parse(&requirement.version_requirement).map_err(|error| {
                stage_configuration_failure_from_id(
                    PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                    format!(
                        "stage {stage} has invalid {kind} version requirement {}: {error}",
                        requirement.version_requirement
                    ),
                    stage,
                    "capability.requirements",
                )
            })?;
        if component.version.chars().any(char::is_control) {
            return Err(stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!("stage {stage} locked {kind} version contains control characters"),
                stage,
                &format!("provider_lock.payload.{kind}"),
            ));
        }
        let version = Version::parse(&component.version).map_err(|error| {
            stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!(
                    "stage {stage} locked {kind} {} has invalid version: {error}",
                    component.id
                ),
                stage,
                &format!("provider_lock.payload.{kind}"),
            )
        })?;
        if !version_requirement.matches(&version)
            || requirement
                .sha256
                .as_ref()
                .is_some_and(|digest| component.sha256.as_ref() != Some(digest))
        {
            return Err(stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!(
                    "stage {stage} locked {kind} {} does not satisfy its exact requirement",
                    component.id
                ),
                stage,
                &format!("provider_lock.payload.{kind}"),
            ));
        }
    }
    Ok(())
}

fn validate_embedded_capability(
    stage: &ResolvedPipelineStage,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let manifest = ProviderManifest {
        schema: PROVIDER_MANIFEST_SCHEMA_V1.to_owned(),
        provider: ProviderIdentity {
            id: stage.provider_lock.payload.provider.id.clone(),
            version: stage.provider_lock.payload.provider.version.clone(),
            display_name: "resolved provider".to_owned(),
        },
        configuration_schemas: vec![stage.capability.configuration_schema.clone()],
        capabilities: vec![stage.capability.clone()],
        provenance: ProvenanceContract {
            required: vec![
                ProvenanceField::InputDigests,
                ProvenanceField::OutputDigests,
                ProvenanceField::PipelineConfiguration,
                ProvenanceField::EffectiveConfiguration,
                ProvenanceField::ProviderIdentity,
                ProvenanceField::CapabilityIdentity,
                ProvenanceField::ToolVersions,
                ProvenanceField::CodecVersions,
                ProvenanceField::ModelIdentities,
                ProvenanceField::ValidationEvidence,
            ],
        },
    };
    manifest.validate().map_err(|error| {
        stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            error.to_string(),
            &stage.id,
            "capability",
        )
    })
}

fn validate_resolution_evidence(
    stage: &ResolvedPipelineStage,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let mut expected = Vec::new();
    if let Some(candidate) = &stage.provider_selection.replacement {
        expected.push((
            candidate,
            ProviderSelection {
                source: ProviderSelectionSource::Replacement,
                fallback_index: None,
            },
        ));
    }
    expected.push((
        &stage.provider_selection.primary,
        ProviderSelection {
            source: ProviderSelectionSource::Primary,
            fallback_index: None,
        },
    ));
    for (index, candidate) in stage.provider_selection.fallbacks.iter().enumerate() {
        let fallback_index = u32::try_from(index).map_err(|_| {
            stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!("stage {} contains too many fallback providers", stage.id),
                &stage.id,
                "provider_selection.fallbacks",
            )
        })?;
        expected.push((
            candidate,
            ProviderSelection {
                source: ProviderSelectionSource::Fallback,
                fallback_index: Some(fallback_index),
            },
        ));
    }

    let selected_index = expected
        .iter()
        .position(|(candidate, selection)| {
            candidate.registration_id == stage.provider_lock.payload.registration_id
                && selection == &stage.provider_lock.payload.selection
        })
        .ok_or_else(|| {
            stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!(
                    "stage {} provider lock selection is absent from authored provider policy",
                    stage.id
                ),
                &stage.id,
                "provider_lock.payload.selection",
            )
        })?;
    if stage.resolution_attempts.len() != selected_index + 1 {
        return Err(stage_configuration_failure_from_id(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            format!(
                "stage {} resolution evidence does not end at the selected provider",
                stage.id
            ),
            &stage.id,
            "resolution_attempts",
        ));
    }
    for (index, attempt) in stage.resolution_attempts.iter().enumerate() {
        let (candidate, selection) = &expected[index];
        if &attempt.candidate != *candidate || attempt.selection != *selection {
            return Err(stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!(
                    "stage {} resolution attempt {index} violates provider precedence",
                    stage.id
                ),
                &stage.id,
                "resolution_attempts",
            ));
        }
        let selected = index == selected_index;
        if attempt.available != selected
            || (selected && !attempt.reason_codes.is_empty())
            || (!selected && attempt.reason_codes.is_empty())
        {
            return Err(stage_configuration_failure_from_id(
                PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
                format!(
                    "stage {} resolution attempt {index} has contradictory availability evidence",
                    stage.id
                ),
                &stage.id,
                "resolution_attempts",
            ));
        }
        if !selected {
            validate_availability_code_combination(&attempt.reason_codes, Some(&stage.id))?;
        }
    }
    Ok(())
}

fn validate_availability_code_combination(
    codes: &[AvailabilityCode],
    stage: Option<&str>,
) -> std::result::Result<(), PipelinePlanningFailure> {
    if codes.contains(&AvailabilityCode::NotRegistered) && codes.len() != 1 {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            "not_registered cannot be combined with other provider availability reasons",
            stage,
            "resolution_attempts.reason_codes",
        ));
    }
    let executable_states = codes
        .iter()
        .filter(|code| {
            matches!(
                code,
                AvailabilityCode::ExecutableUnavailable
                    | AvailabilityCode::ExecutableNotRegularFile
                    | AvailabilityCode::ExecutableNotRunnable
            )
        })
        .count();
    if executable_states > 1 {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            "provider availability evidence contains mutually exclusive executable states",
            stage,
            "resolution_attempts.reason_codes",
        ));
    }
    Ok(())
}

fn diagnose_pipeline_version(
    value: &serde_yaml::Value,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let mapping = value.as_mapping().ok_or_else(|| {
        configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "Pipeline v3 YAML root must be a mapping",
            None,
            "root",
        )
    })?;
    let key = |name: &str| serde_yaml::Value::String(name.to_owned());
    if let Some(version) = mapping.get(key("version")) {
        if version.as_u64() == Some(2) {
            return Err(configuration_failure(
                PipelinePlanningDiagnosticCode::LegacyPipelineV2,
                "this is a Pipeline v2 document; keep using the v2 plan/run surface or migrate to schema aniflow.pipeline/v3",
                None,
                "version",
            ));
        }
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::UnsupportedVersion,
            "Pipeline v3 uses `schema: aniflow.pipeline/v3`; remove the legacy version field",
            None,
            "version",
        ));
    }
    let schema = mapping
        .get(key("schema"))
        .and_then(serde_yaml::Value::as_str);
    match schema {
        Some(PIPELINE_V3_SCHEMA) if mapping.contains_key(key("renderflow")) => {
            Err(configuration_failure(
                PipelinePlanningDiagnosticCode::RenderflowRemoved,
                "renderflow was removed from Pipeline v3; compose cross-holon work in flow instead",
                None,
                "renderflow",
            ))
        }
        Some(PIPELINE_V3_SCHEMA) => Ok(()),
        Some("aniflow.pipeline/v2") => Err(configuration_failure(
            PipelinePlanningDiagnosticCode::LegacyPipelineV2,
            "this is a Pipeline v2 document; keep using the v2 plan/run surface or migrate to schema aniflow.pipeline/v3",
            None,
            "schema",
        )),
        Some(_) => Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidSchema,
            format!("unsupported pipeline schema; expected {PIPELINE_V3_SCHEMA}"),
            None,
            "schema",
        )),
        None => Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidSchema,
            format!("Pipeline v3 requires `schema: {PIPELINE_V3_SCHEMA}`"),
            None,
            "schema",
        )),
    }
}

fn validate_local_id(
    value: &str,
    label: &str,
    stage: Option<&str>,
    field: &str,
) -> std::result::Result<(), PipelinePlanningFailure> {
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
        Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidIdentifier,
            format!(
                "{label} must start with a lowercase letter and contain only lowercase letters, numbers, hyphens, or underscores"
            ),
            stage,
            field,
        ))
    }
}

fn validate_artifact_type(
    value: &str,
    stage: Option<&str>,
    field: &str,
) -> std::result::Result<(), PipelinePlanningFailure> {
    require_nonempty(value, "artifact_type").map_err(|error| {
        configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            error.to_string(),
            stage,
            field,
        )
    })?;
    if value.chars().any(char::is_whitespace) || value.chars().any(char::is_control) {
        return Err(configuration_failure(
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            "artifact_type must be a printable token",
            stage,
            field,
        ));
    }
    Ok(())
}

fn validate_artifact_relative_path(
    value: &str,
    stage: Option<&str>,
) -> std::result::Result<(), PipelinePlanningFailure> {
    let safe = value.starts_with("artifacts/")
        && value.len() > "artifacts/".len()
        && portable_relative_parts(value).is_some();
    if safe {
        Ok(())
    } else {
        Err(configuration_failure(
            PipelinePlanningDiagnosticCode::UnsafeArtifactPath,
            "artifact path must be a portable slash-delimited relative path beneath artifacts/",
            stage,
            "outputs.relative_path",
        ))
    }
}

fn validate_portable_locator(value: &str, field: &str) -> Result<()> {
    if portable_relative_parts(value).is_some() {
        Ok(())
    } else {
        Err(invalid(format!(
            "provider registration {field} must be a portable confined relative locator"
        )))
    }
}

fn resolve_existing_registration_target(
    base: &Path,
    locator: &str,
    field: &str,
) -> Result<PathBuf> {
    let target = fs::canonicalize(base.join(locator)).map_err(|error| {
        Error::new(
            ErrorCategory::Input,
            format!("failed to resolve provider registration {field}: {error}"),
        )
    })?;
    if target.starts_with(base) {
        Ok(target)
    } else {
        Err(invalid(format!(
            "provider registration {field} resolves outside its document directory"
        )))
    }
}

const MAXIMUM_REGISTRATION_JSON_BYTES: u64 = 8 * 1024 * 1024;

fn read_registration_json(path: &Path, field: &str) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path).map_err(|error| {
        Error::new(
            ErrorCategory::Input,
            format!("failed to inspect provider {field}: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(invalid(format!(
            "provider registration {field} must resolve to a regular file"
        )));
    }
    if metadata.len() > MAXIMUM_REGISTRATION_JSON_BYTES {
        return Err(invalid(format!(
            "provider registration {field} exceeds the 8 MiB document limit"
        )));
    }
    let file = File::open(path).map_err(|error| {
        Error::new(
            ErrorCategory::Input,
            format!("failed to open provider {field}: {error}"),
        )
    })?;
    let mut input = Vec::new();
    file.take(MAXIMUM_REGISTRATION_JSON_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|error| {
            Error::new(
                ErrorCategory::Input,
                format!("failed to read provider {field}: {error}"),
            )
        })?;
    if input.len() as u64 > MAXIMUM_REGISTRATION_JSON_BYTES {
        return Err(invalid(format!(
            "provider registration {field} exceeds the 8 MiB document limit"
        )));
    }
    Ok(input)
}

fn resolve_executable_registration_target(base: &Path, locator: &str) -> Result<PathBuf> {
    let unresolved = base.join(locator);
    match fs::symlink_metadata(&unresolved) {
        Ok(_) => resolve_existing_registration_target(base, locator, "executable"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = unresolved.parent().ok_or_else(|| {
                invalid("provider registration executable has no parent directory")
            })?;
            let parent = fs::canonicalize(parent).map_err(|error| {
                Error::new(
                    ErrorCategory::Input,
                    format!("failed to resolve provider registration executable parent: {error}"),
                )
            })?;
            if !parent.starts_with(base) {
                return Err(invalid(
                    "provider registration executable parent resolves outside its document directory",
                ));
            }
            let file_name = unresolved.file_name().ok_or_else(|| {
                invalid("provider registration executable locator has no file name")
            })?;
            Ok(parent.join(file_name))
        }
        Err(error) => Err(Error::new(
            ErrorCategory::Input,
            format!("failed to inspect provider registration executable: {error}"),
        )),
    }
}

fn portable_relative_parts(value: &str) -> Option<Vec<&str>> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
        || Path::new(value).is_absolute()
    {
        return None;
    }
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.iter().any(|part| !is_portable_path_segment(part)) {
        None
    } else {
        Some(parts)
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

fn paths_overlap(left: &str, right: &str) -> bool {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    left == right
        || right
            .strip_prefix(&left)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || left
            .strip_prefix(&right)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn json_contains_null(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Array(values) => values.iter().any(json_contains_null),
        serde_json::Value::Object(values) => values.values().any(json_contains_null),
        _ => false,
    }
}

fn yaml_contains_null(value: &serde_yaml::Value) -> bool {
    match value {
        serde_yaml::Value::Null => true,
        serde_yaml::Value::Sequence(values) => values.iter().any(yaml_contains_null),
        serde_yaml::Value::Mapping(values) => values
            .iter()
            .any(|(key, value)| yaml_contains_null(key) || yaml_contains_null(value)),
        serde_yaml::Value::Tagged(value) => yaml_contains_null(&value.value),
        _ => false,
    }
}

fn validate_digest(
    value: &str,
    label: &str,
    field_prefix: Option<&str>,
) -> std::result::Result<(), PipelinePlanningFailure> {
    require_sha256(value, label).map_err(|error| {
        let field =
            field_prefix.map_or_else(|| label.to_owned(), |prefix| format!("{prefix}.{label}"));
        configuration_failure(
            PipelinePlanningDiagnosticCode::DigestMismatch,
            error.to_string(),
            None,
            &field,
        )
    })
}

fn planning_failure(
    category: ErrorCategory,
    code: PipelinePlanningDiagnosticCode,
    message: impl Into<String>,
    stage: Option<&str>,
    field: Option<&str>,
) -> PipelinePlanningFailure {
    PipelinePlanningFailure::one(category, code, message, stage, field, Vec::new())
}

fn configuration_failure(
    code: PipelinePlanningDiagnosticCode,
    message: impl Into<String>,
    stage: Option<&str>,
    field: &str,
) -> PipelinePlanningFailure {
    planning_failure(
        ErrorCategory::Configuration,
        code,
        message,
        stage,
        Some(field),
    )
}

fn stage_configuration_failure(
    code: PipelinePlanningDiagnosticCode,
    message: impl Into<String>,
    stage: &AuthoredPipelineStage,
    field: &str,
) -> PipelinePlanningFailure {
    stage_configuration_failure_from_id(code, message, &stage.id, field)
}

fn stage_configuration_failure_from_id(
    code: PipelinePlanningDiagnosticCode,
    message: impl Into<String>,
    stage: &str,
    field: &str,
) -> PipelinePlanningFailure {
    configuration_failure(code, message, Some(stage), field)
}

fn input_failure(
    code: PipelinePlanningDiagnosticCode,
    message: impl Into<String>,
    artifact_id: &str,
) -> PipelinePlanningFailure {
    planning_failure(ErrorCategory::Input, code, message, None, Some(artifact_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_executable_leaf_is_left_for_typed_provider_resolution() {
        let root = tempfile::tempdir().expect("temporary root should exist");
        let bin = root.path().join("bin");
        fs::create_dir(&bin).expect("bin directory should be created");
        let resolved = resolve_executable_registration_target(root.path(), "bin/missing")
            .expect("an absent confined executable should remain a resolution concern");

        assert!(resolved.is_absolute());
        assert_eq!(resolved, bin.join("missing"));
    }

    #[cfg(unix)]
    #[test]
    fn executable_symlink_cannot_escape_registration_directory() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary root should exist");
        let outside = tempfile::NamedTempFile::new().expect("outside file should exist");
        symlink(outside.path(), root.path().join("provider"))
            .expect("test symlink should be created");

        assert!(
            resolve_executable_registration_target(root.path(), "provider").is_err(),
            "a registration symlink must remain confined"
        );
    }
}
