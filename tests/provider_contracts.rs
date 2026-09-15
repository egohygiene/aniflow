use std::collections::BTreeMap;

use aniflow::{
    ArtifactCardinality, ArtifactRole, BatchingMode, CancellationMode, CapabilityKind,
    CompatibilityFingerprint, DeterminismClass, FidelityClass, ProgressMode, ProvenanceField,
    ProviderConfiguration, ProviderManifest, RequirementLevel, SideEffect, StreamRole,
};
use serde_json::Value;

const MANIFEST: &[u8] =
    include_bytes!("../docs/contracts/examples/provider-manifest-v1.example.json");
const CONFIGURATION: &[u8] =
    include_bytes!("../docs/contracts/examples/provider-configuration-v1.example.json");
const FINGERPRINT: &[u8] =
    include_bytes!("../docs/contracts/examples/compatibility-fingerprint-v1.example.json");

fn enum_array<T: serde::Serialize, const N: usize>(values: [T; N]) -> Value {
    serde_json::to_value(values.as_slice()).expect("public enum values should serialize")
}

#[test]
fn provider_manifest_matches_the_v1_example() {
    let manifest = ProviderManifest::from_json_slice(MANIFEST)
        .expect("provider manifest example should satisfy the public model");
    let actual = serde_json::to_value(manifest).expect("provider manifest should serialize");
    let expected: Value =
        serde_json::from_slice(MANIFEST).expect("provider manifest example should parse");

    assert_eq!(actual, expected);
}

#[test]
fn provider_configuration_digest_is_canonical_and_self_validating() {
    let configuration = ProviderConfiguration::from_json_slice(CONFIGURATION)
        .expect("provider configuration example should satisfy the public model");
    let rebuilt = ProviderConfiguration::new(
        configuration.provider.clone(),
        configuration.capability.clone(),
        configuration.configuration_schema.clone(),
        configuration.values.clone(),
    )
    .expect("configuration constructor should derive a valid digest");

    assert_eq!(rebuilt, configuration);

    let mut reordered = BTreeMap::new();
    reordered.insert("tile_size".to_owned(), Value::from(128));
    reordered.insert("scale".to_owned(), Value::from(2));
    let expected_digest = configuration.effective_configuration_sha256.clone();
    let rebuilt = ProviderConfiguration::new(
        configuration.provider,
        configuration.capability,
        configuration.configuration_schema,
        reordered,
    )
    .expect("key insertion order should not change canonical configuration identity");
    assert_eq!(rebuilt.effective_configuration_sha256, expected_digest);
}

#[test]
fn compatibility_fingerprint_binds_every_required_identity() {
    let fingerprint = CompatibilityFingerprint::from_json_slice(FINGERPRINT)
        .expect("fingerprint example should satisfy the public model");
    let rebuilt = CompatibilityFingerprint::new(fingerprint.payload.clone())
        .expect("fingerprint constructor should derive canonical identity");

    assert_eq!(rebuilt, fingerprint);

    let mut changed_codec = fingerprint.clone();
    changed_codec.payload.codecs[0].version = "1.6.44".to_owned();
    let error = changed_codec
        .validate()
        .expect_err("codec version changes must invalidate compatibility");
    assert!(error.message().contains("canonical payload"));

    let mut tampered = fingerprint;
    tampered.payload.provider.version = "1.0.1".to_owned();
    let error = tampered
        .validate()
        .expect_err("provider version changes must invalidate compatibility");
    assert!(error.message().contains("canonical payload"));
}

#[test]
fn unknown_contract_versions_and_unknown_fields_fail_closed() {
    let mut manifest: Value =
        serde_json::from_slice(MANIFEST).expect("provider manifest example should parse");
    manifest["schema"] = Value::String("aniflow.provider-manifest/v99".to_owned());
    let error = ProviderManifest::from_json_slice(
        &serde_json::to_vec(&manifest).expect("changed manifest should serialize"),
    )
    .expect_err("unknown provider manifest versions must be rejected");
    assert!(error.message().contains("unsupported contract"));

    let mut configuration: Value =
        serde_json::from_slice(CONFIGURATION).expect("provider configuration should parse");
    configuration["unexpected"] = Value::Bool(true);
    let error = ProviderConfiguration::from_json_slice(
        &serde_json::to_vec(&configuration).expect("changed configuration should serialize"),
    )
    .expect_err("unknown provider configuration fields must be rejected");
    assert!(error.message().contains("unknown field"));

    let mut fingerprint: Value =
        serde_json::from_slice(FINGERPRINT).expect("fingerprint should parse");
    fingerprint["schema"] = Value::String("aniflow.compatibility-fingerprint/v2".to_owned());
    let error = CompatibilityFingerprint::from_json_slice(
        &serde_json::to_vec(&fingerprint).expect("changed fingerprint should serialize"),
    )
    .expect_err("unknown compatibility fingerprint versions must be rejected");
    assert!(error.message().contains("unsupported contract"));
}

#[test]
fn manifest_semantics_reject_unsafe_or_incoherent_declarations() {
    let manifest = ProviderManifest::from_json_slice(MANIFEST)
        .expect("provider manifest example should satisfy the public model");

    let mut invalid_version = manifest.clone();
    invalid_version.provider.version = "01.0.0".to_owned();
    assert!(
        invalid_version
            .validate()
            .expect_err("invalid semantic versions must fail")
            .message()
            .contains("invalid provider version")
    );

    let mut nondeterministic_cache = manifest.clone();
    nondeterministic_cache.capabilities[0].behavior.determinism =
        DeterminismClass::Nondeterministic;
    assert!(
        nondeterministic_cache
            .validate()
            .expect_err("nondeterministic capabilities cannot claim cacheability")
            .message()
            .contains("cannot be cacheable")
    );

    let mut undeclared_network = manifest.clone();
    undeclared_network.capabilities[0]
        .behavior
        .side_effects
        .push(SideEffect::Network);
    assert!(
        undeclared_network
            .validate()
            .expect_err("network requirements and effects must agree")
            .message()
            .contains("network requirement")
    );

    let mut malformed_provider = manifest.clone();
    malformed_provider.provider.id = "example..provider".to_owned();
    assert!(
        malformed_provider
            .validate()
            .expect_err("provider IDs cannot contain empty segments")
            .message()
            .contains("lowercase dotted identifier")
    );

    let mut malformed_capability = manifest;
    malformed_capability.capabilities[0].id = "aniflow/frame..process".to_owned();
    assert!(
        malformed_capability
            .validate()
            .expect_err("capability IDs cannot contain empty segments")
            .message()
            .contains("lowercase letters")
    );
}

#[test]
fn lifecycle_observers_are_structurally_read_only() {
    let mut manifest = ProviderManifest::from_json_slice(MANIFEST)
        .expect("provider manifest example should satisfy the public model");
    {
        let observer = &mut manifest.capabilities[0];
        observer.id = "aniflow/lifecycle.observe".to_owned();
        observer.kind = CapabilityKind::LifecycleObserver;
        observer.outputs.clear();
        observer.batching = BatchingMode::None;
        observer.requirements.compute.gpu = RequirementLevel::Forbidden;
        observer.behavior.determinism = DeterminismClass::Deterministic;
        observer.behavior.fidelity = FidelityClass::NotApplicable;
        observer.behavior.content_changes = false;
        observer
            .behavior
            .side_effects
            .retain(|effect| *effect == SideEffect::FilesystemRead);
    }

    manifest
        .validate()
        .expect("a read-only observer declaration should be valid");

    manifest.capabilities[0]
        .behavior
        .side_effects
        .push(SideEffect::FilesystemWrite);
    let error = manifest
        .validate()
        .expect_err("observer filesystem writes must be rejected");
    assert!(error.message().contains("must be read-only"));
}

#[test]
fn published_json_schemas_parse_and_track_public_enum_values() {
    let manifest_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-manifest-v1.schema.json"
    ))
    .expect("provider manifest schema should be valid JSON");
    let configuration_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-configuration-v1.schema.json"
    ))
    .expect("provider configuration schema should be valid JSON");
    let fingerprint_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/compatibility-fingerprint-v1.schema.json"
    ))
    .expect("compatibility fingerprint schema should be valid JSON");
    let example_configuration_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/example-frame-configuration-v1.schema.json"
    ))
    .expect("example provider configuration schema should be valid JSON");

    assert_eq!(
        manifest_schema["properties"]["schema"]["const"],
        "aniflow.provider-manifest/v1"
    );
    assert_eq!(
        configuration_schema["properties"]["schema"]["const"],
        "aniflow.provider-configuration/v1"
    );
    assert_eq!(
        fingerprint_schema["properties"]["schema"]["const"],
        "aniflow.compatibility-fingerprint/v1"
    );
    assert_eq!(example_configuration_schema["additionalProperties"], false);

    let provider_id_pattern = "^[a-z][a-z0-9-]*(?:\\.[a-z][a-z0-9-]*)+$";
    assert_eq!(
        manifest_schema["$defs"]["providerIdentity"]["properties"]["id"]["pattern"],
        provider_id_pattern
    );
    assert_eq!(
        configuration_schema["$defs"]["providerReference"]["properties"]["id"]["pattern"],
        provider_id_pattern
    );
    assert_eq!(
        fingerprint_schema["$defs"]["providerReference"]["properties"]["id"]["pattern"],
        provider_id_pattern
    );

    let capability_id_pattern = "^aniflow/[a-z][a-z0-9-]*(?:\\.[a-z][a-z0-9-]*)*$";
    assert_eq!(
        manifest_schema["$defs"]["capability"]["properties"]["id"]["pattern"],
        capability_id_pattern
    );
    assert_eq!(
        configuration_schema["$defs"]["capabilityReference"]["properties"]["id"]["pattern"],
        capability_id_pattern
    );
    assert_eq!(
        fingerprint_schema["$defs"]["capabilityReference"]["properties"]["id"]["pattern"],
        capability_id_pattern
    );

    let configuration_schema_id_pattern =
        "^aniflow\\.[a-z][a-z0-9-]*(?:\\.[a-z][a-z0-9-]*)*/v[1-9][0-9]*$";
    assert_eq!(
        manifest_schema["$defs"]["configurationSchema"]["properties"]["id"]["pattern"],
        configuration_schema_id_pattern
    );
    assert_eq!(
        configuration_schema["$defs"]["configurationSchema"]["properties"]["id"]["pattern"],
        configuration_schema_id_pattern
    );
    assert_eq!(
        fingerprint_schema["$defs"]["configurationSchema"]["properties"]["id"]["pattern"],
        configuration_schema_id_pattern
    );

    assert_eq!(
        manifest_schema["$defs"]["capability"]["properties"]["kind"]["enum"],
        enum_array([
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
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["artifactPort"]["properties"]["artifact_role"]["enum"],
        enum_array([
            ArtifactRole::TemporalSource,
            ArtifactRole::TemporalComponent,
            ArtifactRole::Intermediate,
            ArtifactRole::CandidateMaster,
            ArtifactRole::ValidatedMaster,
            ArtifactRole::ValidationEvidence,
            ArtifactRole::RunEvidence,
            ArtifactRole::DeliveryReceipt,
        ])
    );
    assert_eq!(
        fingerprint_schema["$defs"]["artifact"]["properties"]["artifact_role"]["enum"],
        enum_array([
            ArtifactRole::TemporalSource,
            ArtifactRole::TemporalComponent,
            ArtifactRole::Intermediate,
            ArtifactRole::CandidateMaster,
            ArtifactRole::ValidatedMaster,
            ArtifactRole::ValidationEvidence,
            ArtifactRole::RunEvidence,
            ArtifactRole::DeliveryReceipt,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["artifactPort"]["properties"]["stream_role"]["enum"],
        enum_array([
            StreamRole::Video,
            StreamRole::Audio,
            StreamRole::Subtitle,
            StreamRole::Transcript,
            StreamRole::TimedMetadata,
            StreamRole::Attachment,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["artifactPort"]["properties"]["cardinality"]["enum"],
        enum_array([
            ArtifactCardinality::One,
            ArtifactCardinality::Optional,
            ArtifactCardinality::OneOrMore,
            ArtifactCardinality::Many,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["capability"]["properties"]["batching"]["enum"],
        enum_array([
            BatchingMode::None,
            BatchingMode::PerArtifact,
            BatchingMode::DirectoryBatch,
            BatchingMode::WholeArtifact,
            BatchingMode::Stream,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["behavior"]["properties"]["determinism"]["enum"],
        enum_array([
            DeterminismClass::Deterministic,
            DeterminismClass::ConfigurationDependent,
            DeterminismClass::EnvironmentDependent,
            DeterminismClass::Nondeterministic,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["behavior"]["properties"]["fidelity"]["enum"],
        enum_array([
            FidelityClass::Lossless,
            FidelityClass::Lossy,
            FidelityClass::Observational,
            FidelityClass::NotApplicable,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["behavior"]["properties"]["side_effects"]["items"]["enum"],
        enum_array([
            SideEffect::FilesystemRead,
            SideEffect::FilesystemWrite,
            SideEffect::EnvironmentRead,
            SideEffect::Subprocess,
            SideEffect::Network,
            SideEffect::Ai,
            SideEffect::Gpu,
            SideEffect::Publish,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["requirementLevel"]["enum"],
        enum_array([
            RequirementLevel::Forbidden,
            RequirementLevel::Optional,
            RequirementLevel::Required,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["lifecycle"]["properties"]["progress"]["enum"],
        enum_array([
            ProgressMode::None,
            ProgressMode::LifecycleEvents,
            ProgressMode::ItemCounts,
            ProgressMode::Fractional,
        ])
    );
    assert_eq!(
        manifest_schema["$defs"]["lifecycle"]["properties"]["cancellation"]["enum"],
        enum_array([
            CancellationMode::Unsupported,
            CancellationMode::Cooperative,
            CancellationMode::ProcessSignal,
        ])
    );
    assert_eq!(
        manifest_schema["properties"]["provenance"]["properties"]["required"]["items"]["enum"],
        enum_array([
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
        ])
    );
}
