use std::collections::{BTreeMap, BTreeSet};

use semver::{Version, VersionReq};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{Error, ErrorCategory, Result};

pub const PROVIDER_MANIFEST_SCHEMA_V1: &str = "aniflow.provider-manifest/v1";
pub const PROVIDER_CONFIGURATION_SCHEMA_V1: &str = "aniflow.provider-configuration/v1";
pub const COMPATIBILITY_FINGERPRINT_SCHEMA_V1: &str = "aniflow.compatibility-fingerprint/v1";

/// Stable identity for one independently versioned temporal provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderIdentity {
    pub id: String,
    pub version: String,
    pub display_name: String,
}

/// Provider identity retained in configuration and compatibility evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderReference {
    pub id: String,
    pub version: String,
}

/// Capability identity retained independently from provider identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityReference {
    pub id: String,
    pub version: String,
}

/// Exact provider-owned JSON Schema used by a capability configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationSchemaReference {
    pub id: String,
    pub version: String,
    pub sha256: String,
}

/// Temporal extension families owned by aniflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    FrameProcessor,
    AudioProcessor,
    TimedTextProcessor,
    WholeVideoProcessor,
    StreamInspector,
    TemporalValidator,
    ArtifactValidator,
    AssemblerEncoder,
    DeliveryProvider,
    LifecycleObserver,
}

/// Semantic role of an immutable artifact at a provider boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    TemporalSource,
    TemporalComponent,
    Intermediate,
    CandidateMaster,
    ValidatedMaster,
    ValidationEvidence,
    RunEvidence,
    DeliveryReceipt,
}

/// Temporal stream meaning associated with an artifact port when applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamRole {
    Video,
    Audio,
    Subtitle,
    Transcript,
    TimedMetadata,
    Attachment,
}

/// Number of artifacts accepted or produced through one named port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCardinality {
    One,
    Optional,
    OneOrMore,
    Many,
}

/// One typed input or output in a capability declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPort {
    pub name: String,
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_role: Option<StreamRole>,
    pub cardinality: ArtifactCardinality,
    pub immutable: bool,
}

/// How a capability consumes its input collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchingMode {
    None,
    PerArtifact,
    DirectoryBatch,
    WholeArtifact,
    Stream,
}

/// Reproducibility behavior declared by the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismClass {
    Deterministic,
    ConfigurationDependent,
    EnvironmentDependent,
    Nondeterministic,
}

/// Information-preservation behavior for capability outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityClass {
    Lossless,
    Lossy,
    Observational,
    NotApplicable,
}

/// Externally observable effects requested by a provider capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    FilesystemRead,
    FilesystemWrite,
    EnvironmentRead,
    Subprocess,
    Network,
    Ai,
    Gpu,
    Publish,
}

/// Whether a hardware or connectivity resource is used by a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementLevel {
    Forbidden,
    Optional,
    Required,
}

/// Required tool, codec, or model with a semantic version range when known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentRequirement {
    pub id: String,
    pub version_requirement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Minimum local resources and explicit GPU/network behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeRequirements {
    pub minimum_cpu_threads: u16,
    pub minimum_memory_mib: u64,
    pub minimum_storage_mib: u64,
    pub gpu: RequirementLevel,
    pub network: RequirementLevel,
}

/// Complete declared dependency surface for one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequirements {
    pub tools: Vec<ComponentRequirement>,
    pub codecs: Vec<ComponentRequirement>,
    pub models: Vec<ComponentRequirement>,
    pub compute: ComputeRequirements,
}

/// Cache, fidelity, mutation, and effect declarations for one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityBehavior {
    pub determinism: DeterminismClass,
    pub fidelity: FidelityClass,
    pub cacheable: bool,
    pub content_changes: bool,
    pub side_effects: Vec<SideEffect>,
}

/// Granularity of progress observations emitted by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressMode {
    None,
    LifecycleEvents,
    ItemCounts,
    Fractional,
}

/// Cancellation behavior supported by a provider implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationMode {
    Unsupported,
    Cooperative,
    ProcessSignal,
}

/// Lifecycle behavior promised by one capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleSupport {
    pub progress: ProgressMode,
    pub cancellation: CancellationMode,
}

/// Provider capability metadata used before any implementation is executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDeclaration {
    pub id: String,
    pub version: String,
    pub kind: CapabilityKind,
    pub configuration_schema: ConfigurationSchemaReference,
    pub inputs: Vec<ArtifactPort>,
    pub outputs: Vec<ArtifactPort>,
    pub batching: BatchingMode,
    pub max_concurrency: u32,
    pub requirements: ProviderRequirements,
    pub behavior: CapabilityBehavior,
    pub lifecycle: LifecycleSupport,
}

/// Evidence classes every provider result must retain when applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceField {
    InputDigests,
    OutputDigests,
    PipelineConfiguration,
    EffectiveConfiguration,
    ProviderIdentity,
    CapabilityIdentity,
    ToolVersions,
    CodecVersions,
    ModelIdentities,
    ValidationEvidence,
}

/// Provider-wide provenance promise for every declared capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceContract {
    pub required: Vec<ProvenanceField>,
}

/// Inert, provider-owned discovery data for temporal capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderManifest {
    pub schema: String,
    pub provider: ProviderIdentity,
    pub configuration_schemas: Vec<ConfigurationSchemaReference>,
    pub capabilities: Vec<CapabilityDeclaration>,
    pub provenance: ProvenanceContract,
}

/// Effective provider configuration after provider-specific schema validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfiguration {
    pub schema: String,
    pub provider: ProviderReference,
    pub capability: CapabilityReference,
    pub configuration_schema: ConfigurationSchemaReference,
    pub values: BTreeMap<String, Value>,
    pub effective_configuration_sha256: String,
}

/// Exact tool or model identity retained in compatibility evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentIdentity {
    pub id: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Immutable artifact identity retained in compatibility evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintArtifact {
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    pub sha256: String,
}

/// Output identity bound to the validation evidence that accepted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatedOutputFingerprint {
    pub artifact: FingerprintArtifact,
    pub validation_contract: String,
    pub validation_sha256: String,
}

/// Canonical material hashed to decide whether provider state is compatible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityFingerprintPayload {
    pub input_artifacts: Vec<FingerprintArtifact>,
    pub pipeline_configuration_sha256: String,
    pub effective_configuration_sha256: String,
    pub configuration_schema: ConfigurationSchemaReference,
    pub provider: ProviderReference,
    pub capability: CapabilityReference,
    pub tools: Vec<ComponentIdentity>,
    pub codecs: Vec<ComponentIdentity>,
    pub models: Vec<ComponentIdentity>,
    pub validated_outputs: Vec<ValidatedOutputFingerprint>,
}

/// Self-validating SHA-256 compatibility evidence for checkpoint and resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityFingerprint {
    pub schema: String,
    pub algorithm: String,
    pub fingerprint_sha256: String,
    pub payload: CompatibilityFingerprintPayload,
}

impl ProviderManifest {
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let manifest: Self = decode_json(input, "provider manifest")?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, PROVIDER_MANIFEST_SCHEMA_V1)?;
        validate_provider_identity(&self.provider)?;
        if self.configuration_schemas.is_empty() {
            return Err(invalid(
                "provider manifest must declare a configuration schema",
            ));
        }
        if self.capabilities.is_empty() {
            return Err(invalid("provider manifest must declare a capability"));
        }

        let mut schemas = BTreeSet::new();
        for schema in &self.configuration_schemas {
            validate_configuration_schema(schema)?;
            if !schemas.insert(schema_key(schema)) {
                return Err(invalid(format!(
                    "provider manifest contains duplicate configuration schema {} {}",
                    schema.id, schema.version
                )));
            }
        }

        let mut capabilities = BTreeSet::new();
        for capability in &self.capabilities {
            validate_capability(capability, &schemas)?;
            if !capabilities.insert(capability.id.as_str()) {
                return Err(invalid(format!(
                    "provider manifest contains duplicate capability id {}",
                    capability.id
                )));
            }
        }
        validate_provenance(&self.provenance)
    }
}

impl ProviderConfiguration {
    pub fn new(
        provider: ProviderReference,
        capability: CapabilityReference,
        configuration_schema: ConfigurationSchemaReference,
        values: BTreeMap<String, Value>,
    ) -> Result<Self> {
        let effective_configuration_sha256 = canonical_sha256(&values)?;
        let configuration = Self {
            schema: PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
            provider,
            capability,
            configuration_schema,
            values,
            effective_configuration_sha256,
        };
        configuration.validate()?;
        Ok(configuration)
    }

    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let configuration: Self = decode_json(input, "provider configuration")?;
        configuration.validate()?;
        Ok(configuration)
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, PROVIDER_CONFIGURATION_SCHEMA_V1)?;
        validate_provider_reference(&self.provider)?;
        validate_capability_reference(&self.capability)?;
        validate_configuration_schema(&self.configuration_schema)?;
        require_sha256(
            &self.effective_configuration_sha256,
            "effective_configuration_sha256",
        )?;
        let expected = canonical_sha256(&self.values)?;
        if self.effective_configuration_sha256 != expected {
            return Err(invalid(format!(
                "effective configuration digest does not match canonical values; expected {expected}"
            )));
        }
        Ok(())
    }
}

impl CompatibilityFingerprint {
    pub fn new(payload: CompatibilityFingerprintPayload) -> Result<Self> {
        validate_fingerprint_payload(&payload)?;
        let fingerprint_sha256 = canonical_sha256(&payload)?;
        Ok(Self {
            schema: COMPATIBILITY_FINGERPRINT_SCHEMA_V1.to_owned(),
            algorithm: "sha256".to_owned(),
            fingerprint_sha256,
            payload,
        })
    }

    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let fingerprint: Self = decode_json(input, "compatibility fingerprint")?;
        fingerprint.validate()?;
        Ok(fingerprint)
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, COMPATIBILITY_FINGERPRINT_SCHEMA_V1)?;
        if self.algorithm != "sha256" {
            return Err(invalid(format!(
                "unsupported compatibility fingerprint algorithm {}; expected sha256",
                self.algorithm
            )));
        }
        require_sha256(&self.fingerprint_sha256, "fingerprint_sha256")?;
        validate_fingerprint_payload(&self.payload)?;
        let expected = canonical_sha256(&self.payload)?;
        if self.fingerprint_sha256 != expected {
            return Err(invalid(format!(
                "compatibility fingerprint does not match its canonical payload; expected {expected}"
            )));
        }
        Ok(())
    }
}

fn validate_provider_identity(identity: &ProviderIdentity) -> Result<()> {
    validate_provider_id(&identity.id)?;
    validate_semantic_version(&identity.version, "provider version")?;
    require_nonempty(&identity.display_name, "provider display_name")
}

pub(crate) fn validate_provider_reference(reference: &ProviderReference) -> Result<()> {
    validate_provider_id(&reference.id)?;
    validate_semantic_version(&reference.version, "provider version")
}

pub(crate) fn validate_capability_reference(reference: &CapabilityReference) -> Result<()> {
    validate_capability_id(&reference.id)?;
    validate_semantic_version(&reference.version, "capability version")
}

pub(crate) fn validate_configuration_schema(schema: &ConfigurationSchemaReference) -> Result<()> {
    if !is_configuration_schema_id(&schema.id) {
        return Err(invalid(format!(
            "configuration schema id {} must use aniflow.<name>/v<major>",
            schema.id
        )));
    }
    validate_semantic_version(&schema.version, "configuration schema version")?;
    require_sha256(&schema.sha256, "configuration schema sha256")
}

fn validate_capability(
    capability: &CapabilityDeclaration,
    schemas: &BTreeSet<(String, String, String)>,
) -> Result<()> {
    validate_capability_id(&capability.id)?;
    validate_semantic_version(&capability.version, "capability version")?;
    validate_configuration_schema(&capability.configuration_schema)?;
    if !schemas.contains(&schema_key(&capability.configuration_schema)) {
        return Err(invalid(format!(
            "capability {} references an undeclared configuration schema",
            capability.id
        )));
    }
    if capability.inputs.is_empty() {
        return Err(invalid(format!(
            "capability {} must declare at least one input",
            capability.id
        )));
    }
    validate_artifact_ports(&capability.inputs, "input", &capability.id)?;
    validate_artifact_ports(&capability.outputs, "output", &capability.id)?;
    if capability.max_concurrency == 0 {
        return Err(invalid(format!(
            "capability {} max_concurrency must be at least 1",
            capability.id
        )));
    }
    validate_requirements(&capability.requirements)?;
    validate_behavior(capability)?;

    if capability.kind == CapabilityKind::LifecycleObserver {
        if !capability.outputs.is_empty()
            || capability.behavior.content_changes
            || capability.batching != BatchingMode::None
            || capability
                .behavior
                .side_effects
                .iter()
                .any(|effect| matches!(effect, SideEffect::FilesystemWrite | SideEffect::Publish))
        {
            return Err(invalid(format!(
                "lifecycle observer {} must be read-only, unbatched, and produce no artifacts",
                capability.id
            )));
        }
    } else if capability.outputs.is_empty() {
        return Err(invalid(format!(
            "capability {} must declare an immutable output",
            capability.id
        )));
    }

    if capability.behavior.content_changes && capability.outputs.is_empty() {
        return Err(invalid(format!(
            "content-changing capability {} must produce a new immutable artifact",
            capability.id
        )));
    }
    if capability
        .behavior
        .side_effects
        .contains(&SideEffect::Publish)
        && capability.kind != CapabilityKind::DeliveryProvider
    {
        return Err(invalid(format!(
            "only a delivery provider may declare the publish side effect; found {}",
            capability.id
        )));
    }
    Ok(())
}

fn validate_artifact_ports(
    ports: &[ArtifactPort],
    direction: &str,
    capability: &str,
) -> Result<()> {
    let mut names = BTreeSet::new();
    for port in ports {
        if !is_local_id(&port.name) {
            return Err(invalid(format!(
                "{direction} port name {} on {capability} must use lowercase letters, numbers, hyphens, or underscores",
                port.name
            )));
        }
        require_token(&port.artifact_type, "artifact_type")?;
        if !port.immutable {
            return Err(invalid(format!(
                "{direction} port {} on {capability} must declare immutable=true",
                port.name
            )));
        }
        if !names.insert(port.name.as_str()) {
            return Err(invalid(format!(
                "capability {capability} contains duplicate {direction} port {}",
                port.name
            )));
        }
    }
    Ok(())
}

fn validate_requirements(requirements: &ProviderRequirements) -> Result<()> {
    validate_component_requirements(&requirements.tools, "tool")?;
    validate_component_requirements(&requirements.codecs, "codec")?;
    validate_component_requirements(&requirements.models, "model")?;
    if requirements.compute.minimum_cpu_threads == 0 {
        return Err(invalid("minimum_cpu_threads must be at least 1"));
    }
    Ok(())
}

fn validate_component_requirements(
    requirements: &[ComponentRequirement],
    kind: &str,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    for requirement in requirements {
        require_token(&requirement.id, &format!("{kind} id"))?;
        VersionReq::parse(&requirement.version_requirement).map_err(|error| {
            invalid(format!(
                "invalid {kind} version requirement {}: {error}",
                requirement.version_requirement
            ))
        })?;
        if let Some(digest) = &requirement.sha256 {
            require_sha256(digest, &format!("{kind} sha256"))?;
        }
        if !ids.insert(requirement.id.as_str()) {
            return Err(invalid(format!(
                "duplicate {kind} requirement {}",
                requirement.id
            )));
        }
    }
    Ok(())
}

fn validate_behavior(capability: &CapabilityDeclaration) -> Result<()> {
    let effects = capability
        .behavior
        .side_effects
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if effects.len() != capability.behavior.side_effects.len() {
        return Err(invalid(format!(
            "capability {} contains duplicate side effects",
            capability.id
        )));
    }
    if capability.behavior.cacheable
        && matches!(
            capability.behavior.determinism,
            DeterminismClass::EnvironmentDependent | DeterminismClass::Nondeterministic
        )
    {
        return Err(invalid(format!(
            "capability {} cannot be cacheable with {:?} determinism",
            capability.id, capability.behavior.determinism
        )));
    }
    if capability.behavior.content_changes
        && capability.behavior.fidelity == FidelityClass::NotApplicable
    {
        return Err(invalid(format!(
            "content-changing capability {} must declare its fidelity",
            capability.id
        )));
    }

    validate_effect_requirement(
        &capability.id,
        "network",
        capability.requirements.compute.network,
        effects.contains(&SideEffect::Network),
    )?;
    validate_effect_requirement(
        &capability.id,
        "gpu",
        capability.requirements.compute.gpu,
        effects.contains(&SideEffect::Gpu),
    )
}

fn validate_effect_requirement(
    capability: &str,
    resource: &str,
    requirement: RequirementLevel,
    effect_declared: bool,
) -> Result<()> {
    let used = requirement != RequirementLevel::Forbidden;
    if used != effect_declared {
        return Err(invalid(format!(
            "capability {capability} must keep its {resource} requirement and side effect consistent"
        )));
    }
    Ok(())
}

fn validate_provenance(provenance: &ProvenanceContract) -> Result<()> {
    let required = provenance.required.iter().copied().collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
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
    ]);
    if required != expected || required.len() != provenance.required.len() {
        return Err(invalid(
            "provider provenance must declare every required v1 evidence class exactly once",
        ));
    }
    Ok(())
}

fn validate_fingerprint_payload(payload: &CompatibilityFingerprintPayload) -> Result<()> {
    if payload.input_artifacts.is_empty() {
        return Err(invalid(
            "compatibility fingerprint requires an input artifact",
        ));
    }
    if payload.validated_outputs.is_empty() {
        return Err(invalid(
            "compatibility fingerprint requires a validated output artifact",
        ));
    }
    require_sha256(
        &payload.pipeline_configuration_sha256,
        "pipeline_configuration_sha256",
    )?;
    require_sha256(
        &payload.effective_configuration_sha256,
        "effective_configuration_sha256",
    )?;
    validate_configuration_schema(&payload.configuration_schema)?;
    validate_provider_reference(&payload.provider)?;
    validate_capability_reference(&payload.capability)?;
    validate_fingerprint_artifacts(&payload.input_artifacts, "input artifact")?;
    validate_component_identities(&payload.tools, "tool")?;
    validate_component_identities(&payload.codecs, "codec")?;
    validate_component_identities(&payload.models, "model")?;

    let mut outputs = BTreeSet::new();
    for output in &payload.validated_outputs {
        validate_fingerprint_artifact(&output.artifact, "validated output")?;
        require_token(&output.validation_contract, "validation contract")?;
        require_sha256(&output.validation_sha256, "validation_sha256")?;
        let key = (
            output.artifact.artifact_type.as_str(),
            output.artifact.sha256.as_str(),
            output.validation_contract.as_str(),
        );
        if !outputs.insert(key) {
            return Err(invalid(
                "compatibility fingerprint contains duplicate validated outputs",
            ));
        }
    }
    Ok(())
}

fn validate_fingerprint_artifacts(artifacts: &[FingerprintArtifact], kind: &str) -> Result<()> {
    let mut identities = BTreeSet::new();
    for artifact in artifacts {
        validate_fingerprint_artifact(artifact, kind)?;
        let key = (artifact.artifact_type.as_str(), artifact.sha256.as_str());
        if !identities.insert(key) {
            return Err(invalid(format!(
                "compatibility fingerprint contains duplicate {kind} identities"
            )));
        }
    }
    Ok(())
}

fn validate_fingerprint_artifact(artifact: &FingerprintArtifact, kind: &str) -> Result<()> {
    require_token(&artifact.artifact_type, &format!("{kind} type"))?;
    require_sha256(&artifact.sha256, &format!("{kind} sha256"))
}

pub(crate) fn validate_component_identities(
    components: &[ComponentIdentity],
    kind: &str,
) -> Result<()> {
    let mut previous: Option<(&str, &str)> = None;
    for component in components {
        require_token(&component.id, &format!("{kind} id"))?;
        require_nonempty(&component.version, &format!("{kind} version"))?;
        if let Some(digest) = &component.sha256 {
            require_sha256(digest, &format!("{kind} sha256"))?;
        }
        let current = (component.id.as_str(), component.version.as_str());
        if previous.is_some_and(|value| value >= current) {
            return Err(invalid(format!(
                "compatibility fingerprint {kind} identities must be unique and sorted by id then version"
            )));
        }
        previous = Some(current);
    }
    Ok(())
}

fn validate_provider_id(value: &str) -> Result<()> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|part| !is_name_segment(part)) {
        return Err(invalid(format!(
            "provider id {value} must be a lowercase dotted identifier"
        )));
    }
    Ok(())
}

pub(crate) fn validate_capability_id(value: &str) -> Result<()> {
    let Some(name) = value.strip_prefix("aniflow/") else {
        return Err(invalid(format!(
            "capability id {value} must be owned by the aniflow/ namespace"
        )));
    };
    if !is_name_segment(name) && !is_dotted_name(name) {
        return Err(invalid(format!(
            "capability id {value} must use lowercase letters, numbers, periods, or hyphens"
        )));
    }
    Ok(())
}

pub(crate) fn validate_semantic_version(value: &str, field: &str) -> Result<()> {
    Version::parse(value)
        .map(|_| ())
        .map_err(|error| invalid(format!("invalid {field} {value}: {error}")))
}

pub(crate) fn require_schema(actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(invalid(format!(
            "unsupported contract {actual}; expected {expected}"
        )))
    }
}

pub(crate) fn require_nonempty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(invalid(format!("{field} cannot be empty")))
    } else {
        Ok(())
    }
}

pub(crate) fn require_token(value: &str, field: &str) -> Result<()> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        Err(invalid(format!("{field} must be a non-empty token")))
    } else {
        Ok(())
    }
}

pub(crate) fn require_sha256(value: &str, field: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{field} must be a lowercase 64-character SHA-256 digest"
        )))
    }
}

fn is_name_segment(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_dotted_name(value: &str) -> bool {
    value.split('.').all(is_name_segment)
}

fn is_local_id(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn is_configuration_schema_id(value: &str) -> bool {
    let Some((name, version)) = value.rsplit_once("/v") else {
        return false;
    };
    let Some(name) = name.strip_prefix("aniflow.") else {
        return false;
    };
    is_dotted_name(name)
        && !version.is_empty()
        && !version.starts_with('0')
        && version.bytes().all(|byte| byte.is_ascii_digit())
}

fn schema_key(schema: &ConfigurationSchemaReference) -> (String, String, String) {
    (
        schema.id.clone(),
        schema.version.clone(),
        schema.sha256.clone(),
    )
}

pub(crate) fn decode_json<T: DeserializeOwned>(input: &[u8], name: &str) -> Result<T> {
    serde_json::from_slice(input).map_err(|error| invalid(format!("invalid {name} JSON: {error}")))
}

pub(crate) fn canonical_sha256<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value).map_err(|error| {
        Error::new(
            ErrorCategory::Internal,
            format!("failed to serialize canonical compatibility material: {error}"),
        )
    })?;
    let mut bytes = Vec::new();
    write_canonical_json(&value, &mut bytes)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn write_canonical_json(value: &Value, output: &mut Vec<u8>) -> Result<()> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            serde_json::to_writer(&mut *output, value).map_err(|error| {
                Error::new(
                    ErrorCategory::Internal,
                    format!("failed to encode canonical JSON string: {error}"),
                )
            })?;
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_json(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).map_err(|error| {
                    Error::new(
                        ErrorCategory::Internal,
                        format!("failed to encode canonical JSON key: {error}"),
                    )
                })?;
                output.push(b':');
                write_canonical_json(value, output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

pub(crate) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_kind_contract_covers_every_parent_extension_family() {
        let kinds = [
            CapabilityKind::FrameProcessor,
            CapabilityKind::AudioProcessor,
            CapabilityKind::TimedTextProcessor,
            CapabilityKind::WholeVideoProcessor,
            CapabilityKind::StreamInspector,
            CapabilityKind::TemporalValidator,
            CapabilityKind::ArtifactValidator,
            CapabilityKind::AssemblerEncoder,
            CapabilityKind::DeliveryProvider,
            CapabilityKind::LifecycleObserver,
        ];
        let values = kinds
            .into_iter()
            .map(|kind| serde_json::to_value(kind).expect("kind should serialize"))
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 10);
        assert!(values.contains(&Value::String("frame_processor".to_owned())));
        assert!(values.contains(&Value::String("lifecycle_observer".to_owned())));
    }

    #[test]
    fn canonical_digest_ignores_object_key_insertion_order() {
        let left = serde_json::json!({"tile_size": 128, "scale": 2});
        let right = serde_json::json!({"scale": 2, "tile_size": 128});

        assert_eq!(
            canonical_sha256(&left).expect("left should hash"),
            canonical_sha256(&right).expect("right should hash")
        );
    }
}
