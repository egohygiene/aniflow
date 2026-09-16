use aniflow::{
    ArtifactCardinality, ArtifactRole, AvailabilityCode, BatchingMode, CancellationMode,
    CapabilityKind, DeterminismClass, ErrorCategory, FidelityClass,
    PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1, PipelineInputKind, PipelinePlanningDiagnosticCode,
    PipelinePlanningFailure, PipelineV3Configuration, PipelineV3Plan, PipelineV3RunOutcome,
    PipelineV3RunRecovery, ProgressMode, ProviderRegistrationDocument, ProviderSelectionSource,
    RequirementLevel, SideEffect, StreamRole,
};
use serde_json::Value;

fn enum_array<T: serde::Serialize, const N: usize>(values: [T; N]) -> Value {
    serde_json::to_value(values.as_slice()).expect("public enum values should serialize")
}

fn schema(source: &str) -> Value {
    serde_json::from_str(source).expect("published schema should be valid JSON")
}

#[test]
fn published_v3_configuration_registration_and_failure_examples_parse() {
    let configuration = PipelineV3Configuration::from_yaml_slice(include_bytes!(
        "../docs/contracts/examples/pipeline-v3-configuration-v1.example.json"
    ))
    .expect("published Pipeline v3 configuration should validate");
    assert_eq!(configuration.schema, "aniflow.pipeline/v3");
    assert_eq!(
        configuration.stages[0].outputs[0].artifacts[0].kind,
        Some(PipelineInputKind::Directory)
    );
    assert_eq!(
        configuration
            .configuration_sha256()
            .expect("published configuration should have a canonical identity"),
        "208286109934c27b50e0684bce3e00ef48bcf65f69d189b3a8e7589c8aeba426"
    );

    let registration = ProviderRegistrationDocument::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-registration-v1.example.json"
    ))
    .expect("published provider registration should validate");
    assert_eq!(registration.schema, "aniflow.provider-registration/v1");

    let failure = PipelinePlanningFailure::from_json_slice(include_bytes!(
        "../docs/contracts/examples/pipeline-v3-planning-failure-v1.example.json"
    ))
    .expect("published planning failure should parse and validate");
    assert_eq!(failure.schema, "aniflow.pipeline-planning-failure/v1");
    assert_eq!(failure.category, ErrorCategory::Configuration);
    assert_eq!(
        failure.diagnostics[0].code,
        PipelinePlanningDiagnosticCode::InvalidSchema
    );
}

#[test]
fn published_v3_plan_is_canonical_and_self_validating() {
    let input = include_bytes!("../docs/contracts/examples/pipeline-v3-plan-v1.example.json");
    let plan = PipelineV3Plan::from_json_slice(input)
        .expect("published Pipeline v3 plan should validate its embedded identities and digests");
    assert_eq!(
        plan.plan_sha256,
        "63847499f5a38842f828790fdf56991b2e52ca90c988bc789902726ced8cdea5"
    );
    assert_eq!(
        plan.payload.stages[0].outputs[0].artifacts[0].kind,
        Some(PipelineInputKind::Directory)
    );

    let canonical = plan
        .canonical_json_bytes()
        .expect("valid plans should have a canonical serialization");
    let reparsed = PipelineV3Plan::from_json_slice(&canonical)
        .expect("canonical plan bytes should round trip");
    assert_eq!(reparsed, plan);
}

#[test]
fn published_v3_run_outcome_matches_the_closed_rust_contract() {
    let input = include_bytes!("../docs/contracts/examples/pipeline-run-outcome-v1.example.json");
    let outcome: PipelineV3RunOutcome =
        serde_json::from_slice(input).expect("published run outcome should parse");
    assert_eq!(outcome.schema, "aniflow.pipeline-run-outcome/v1");
    assert_eq!(outcome.executed_stages, vec!["enhance".to_owned()]);
    assert!(outcome.reused_stages.is_empty());
    assert_eq!(
        serde_json::to_value(outcome).expect("run outcome should serialize"),
        serde_json::from_slice::<Value>(input).expect("run outcome fixture should be JSON")
    );
}

#[test]
fn published_v3_run_recovery_matches_the_partial_rust_contract() {
    let input = include_bytes!("../docs/contracts/examples/pipeline-run-recovery-v1.example.json");
    let recovery: PipelineV3RunRecovery =
        serde_json::from_slice(input).expect("published run recovery should parse");
    assert_eq!(recovery.schema, PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1);
    assert!(recovery.run_directory.is_absolute());
    assert!(
        recovery
            .run_manifest
            .as_ref()
            .is_some_and(|path| path.is_absolute())
    );
    assert_eq!(
        serde_json::to_value(recovery).expect("run recovery should serialize"),
        serde_json::from_slice::<Value>(input).expect("run recovery fixture should be JSON")
    );

    let without_manifest = PipelineV3RunRecovery {
        schema: PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1.to_owned(),
        run_directory: "/var/lib/aniflow/runs/recovery".into(),
        run_manifest: None,
    };
    assert!(
        serde_json::to_value(without_manifest)
            .expect("partial run recovery should serialize")
            .get("run_manifest")
            .is_none()
    );

    let mut explicit_null: Value = serde_json::from_slice(input).expect("example should be JSON");
    explicit_null["run_manifest"] = Value::Null;
    serde_json::from_value::<PipelineV3RunRecovery>(explicit_null)
        .expect_err("an explicit null manifest must not alias an omitted locator");
}

#[test]
fn v3_contract_parsers_reject_tampering_and_unknown_contracts() {
    let duplicate_schema = include_str!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    )
    .replacen('{', "{\"schema\":\"aniflow.pipeline-plan/v1\",", 1);
    let failure = PipelineV3Plan::from_json_slice(duplicate_schema.as_bytes())
        .expect_err("duplicate resolved-plan fields must fail before normalization");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidJson
    );

    let mut plan: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    ))
    .expect("plan fixture should parse");
    plan["payload"]["pipeline_name"] = Value::String("tampered-name".to_owned());
    let failure = PipelineV3Plan::from_json_slice(&serde_json::to_vec(&plan).unwrap())
        .expect_err("payload tampering must invalidate the plan");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::DigestMismatch
    );

    let mut plan: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    ))
    .expect("plan fixture should parse");
    plan["payload"]["stages"][0]["provider_selection"]["replacement"] = Value::Null;
    let failure = PipelineV3Plan::from_json_slice(&serde_json::to_vec(&plan).unwrap())
        .expect_err("explicit null must not alias an omitted normalized plan field");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidResolvedPlan
    );

    let mut plan: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    ))
    .expect("plan fixture should parse");
    plan["payload"]["stages"][0]["provider_selection"]
        .as_object_mut()
        .expect("provider selection should be an object")
        .remove("fallbacks");
    let failure = PipelineV3Plan::from_json_slice(&serde_json::to_vec(&plan).unwrap())
        .expect_err("normalized provider selections must retain empty fallback arrays");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidResolvedPlan
    );

    let mut plan: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    ))
    .expect("plan fixture should parse");
    plan["payload"]["stages"][0]["outputs"][0]["artifacts"][0]
        .as_object_mut()
        .expect("planned artifact should be an object")
        .remove("kind");
    let failure = PipelineV3Plan::from_json_slice(&serde_json::to_vec(&plan).unwrap())
        .expect_err("removing a planned output kind without updating its digest is tampering");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::DigestMismatch
    );

    let mut unknown_plan: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-plan-v1.example.json"
    ))
    .expect("plan fixture should parse");
    unknown_plan["schema"] = Value::String("aniflow.pipeline-plan/v2".to_owned());
    let failure = PipelineV3Plan::from_json_slice(&serde_json::to_vec(&unknown_plan).unwrap())
        .expect_err("unknown plan revisions must fail closed");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidSchema
    );

    let mut configuration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-configuration-v1.example.json"
    ))
    .expect("configuration fixture should parse");
    configuration["schema"] = Value::String("aniflow.pipeline/v4".to_owned());
    let failure =
        PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&configuration).unwrap())
            .expect_err("unknown authored pipeline schemas must fail closed");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidSchema
    );

    let mut configuration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-configuration-v1.example.json"
    ))
    .expect("configuration fixture should parse");
    configuration["name"] = Value::String("unsafe\u{1b}[31mname".to_owned());
    let failure =
        PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&configuration).unwrap())
            .expect_err("pipeline names with terminal controls must fail closed");
    assert_eq!(
        failure.diagnostics()[0].code,
        PipelinePlanningDiagnosticCode::InvalidConfiguration
    );
    assert!(
        !failure.message.chars().any(char::is_control),
        "rejected control characters must not be copied into diagnostics"
    );

    let mut configuration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-configuration-v1.example.json"
    ))
    .expect("configuration fixture should parse");
    configuration["inputs"][0]["stream_role"] = Value::Null;
    PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&configuration).unwrap())
        .expect_err("explicit null must not alias an omitted authored field");

    let mut configuration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-configuration-v1.example.json"
    ))
    .expect("configuration fixture should parse");
    configuration["stages"][0]["outputs"][0]["artifacts"][0]
        .as_object_mut()
        .expect("expected artifact should be an object")
        .remove("kind");
    PipelineV3Configuration::from_yaml_slice(&serde_json::to_vec(&configuration).unwrap())
        .expect("output kind remains optional for planning compatibility");

    let mut registration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/provider-registration-v1.example.json"
    ))
    .expect("registration fixture should parse");
    registration["schema"] = Value::String("aniflow.provider-registration/v2".to_owned());
    assert!(
        ProviderRegistrationDocument::from_json_slice(&serde_json::to_vec(&registration).unwrap())
            .is_err()
    );

    let mut registration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/provider-registration-v1.example.json"
    ))
    .expect("registration fixture should parse");
    registration["registration_id"] = Value::String("unsafe\u{1b}[31m-id".to_owned());
    let error =
        ProviderRegistrationDocument::from_json_slice(&serde_json::to_vec(&registration).unwrap())
            .expect_err("provider identifiers with terminal controls must fail closed");
    assert!(
        !error.message().chars().any(char::is_control),
        "rejected control characters must not be copied into provider diagnostics"
    );

    let mut registration: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/provider-registration-v1.example.json"
    ))
    .expect("registration fixture should parse");
    registration["components"]["tools"][0]["sha256"] = Value::Null;
    ProviderRegistrationDocument::from_json_slice(&serde_json::to_vec(&registration).unwrap())
        .expect_err("explicit null must not alias an omitted component digest");

    let mut planning_failure: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/pipeline-v3-planning-failure-v1.example.json"
    ))
    .expect("planning failure fixture should parse");
    planning_failure["diagnostics"][0]["stage"] = Value::Null;
    PipelinePlanningFailure::from_json_slice(&serde_json::to_vec(&planning_failure).unwrap())
        .expect_err("explicit null must not alias an omitted diagnostic location");
}

#[test]
fn published_v3_schema_enums_match_the_public_rust_contract() {
    let configuration = schema(include_str!(
        "../docs/contracts/pipeline-v3-configuration-v1.schema.json"
    ));
    let plan = schema(include_str!(
        "../docs/contracts/pipeline-v3-plan-v1.schema.json"
    ));
    let failure = schema(include_str!(
        "../docs/contracts/pipeline-v3-planning-failure-v1.schema.json"
    ));

    let artifact_roles = enum_array([
        ArtifactRole::TemporalSource,
        ArtifactRole::TemporalComponent,
        ArtifactRole::Intermediate,
        ArtifactRole::CandidateMaster,
        ArtifactRole::ValidatedMaster,
        ArtifactRole::ValidationEvidence,
        ArtifactRole::RunEvidence,
        ArtifactRole::DeliveryReceipt,
    ]);
    let stream_roles = enum_array([
        StreamRole::Video,
        StreamRole::Audio,
        StreamRole::Subtitle,
        StreamRole::Transcript,
        StreamRole::TimedMetadata,
        StreamRole::Attachment,
    ]);
    assert_eq!(
        configuration["$defs"]["artifactRole"]["enum"],
        artifact_roles
    );
    assert_eq!(plan["$defs"]["artifactRole"]["enum"], artifact_roles);
    assert_eq!(configuration["$defs"]["streamRole"]["enum"], stream_roles);
    assert_eq!(plan["$defs"]["streamRole"]["enum"], stream_roles);

    let artifact_kinds = enum_array([PipelineInputKind::File, PipelineInputKind::Directory]);
    assert_eq!(
        configuration["$defs"]["expectedArtifact"]["properties"]["kind"]["enum"],
        artifact_kinds
    );
    assert_eq!(
        plan["$defs"]["plannedExpectedArtifact"]["properties"]["kind"]["enum"],
        artifact_kinds
    );
    assert_eq!(
        plan["$defs"]["inputIdentity"]["properties"]["kind"]["enum"],
        artifact_kinds
    );
    assert_eq!(
        plan["$defs"]["selection"]["properties"]["source"]["enum"],
        enum_array([
            ProviderSelectionSource::Replacement,
            ProviderSelectionSource::Primary,
            ProviderSelectionSource::Fallback,
        ])
    );
    assert_eq!(
        plan["$defs"]["sideEffect"]["enum"],
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
        plan["$defs"]["capability"]["properties"]["kind"]["enum"],
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
        plan["$defs"]["artifactPort"]["properties"]["cardinality"]["enum"],
        enum_array([
            ArtifactCardinality::One,
            ArtifactCardinality::Optional,
            ArtifactCardinality::OneOrMore,
            ArtifactCardinality::Many,
        ])
    );
    assert_eq!(
        plan["$defs"]["capability"]["properties"]["batching"]["enum"],
        enum_array([
            BatchingMode::None,
            BatchingMode::PerArtifact,
            BatchingMode::DirectoryBatch,
            BatchingMode::WholeArtifact,
            BatchingMode::Stream,
        ])
    );
    assert_eq!(
        plan["$defs"]["behavior"]["properties"]["determinism"]["enum"],
        enum_array([
            DeterminismClass::Deterministic,
            DeterminismClass::ConfigurationDependent,
            DeterminismClass::EnvironmentDependent,
            DeterminismClass::Nondeterministic,
        ])
    );
    assert_eq!(
        plan["$defs"]["behavior"]["properties"]["fidelity"]["enum"],
        enum_array([
            FidelityClass::Lossless,
            FidelityClass::Lossy,
            FidelityClass::Observational,
            FidelityClass::NotApplicable,
        ])
    );
    assert_eq!(
        plan["$defs"]["requirementLevel"]["enum"],
        enum_array([
            RequirementLevel::Forbidden,
            RequirementLevel::Optional,
            RequirementLevel::Required,
        ])
    );
    assert_eq!(
        plan["$defs"]["lifecycle"]["properties"]["progress"]["enum"],
        enum_array([
            ProgressMode::None,
            ProgressMode::LifecycleEvents,
            ProgressMode::ItemCounts,
            ProgressMode::Fractional,
        ])
    );
    assert_eq!(
        plan["$defs"]["lifecycle"]["properties"]["cancellation"]["enum"],
        enum_array([
            CancellationMode::Unsupported,
            CancellationMode::Cooperative,
            CancellationMode::ProcessSignal,
        ])
    );
    assert_eq!(
        plan["$defs"]["availabilityCode"]["enum"],
        enum_array([
            AvailabilityCode::NotRegistered,
            AvailabilityCode::CapabilityMismatch,
            AvailabilityCode::VersionMismatch,
            AvailabilityCode::ExecutableUnavailable,
            AvailabilityCode::ExecutableNotRegularFile,
            AvailabilityCode::ExecutableNotRunnable,
            AvailabilityCode::ComponentMissing,
            AvailabilityCode::ComponentVersionMismatch,
            AvailabilityCode::ComponentDigestMismatch,
            AvailabilityCode::InsufficientCpu,
            AvailabilityCode::InsufficientMemory,
            AvailabilityCode::InsufficientStorage,
            AvailabilityCode::GpuUnavailable,
            AvailabilityCode::NetworkUnavailable,
            AvailabilityCode::OfflineIncompatible,
            AvailabilityCode::SideEffectDenied,
        ])
    );
    assert_eq!(
        failure["$defs"]["availabilityCode"]["enum"],
        plan["$defs"]["availabilityCode"]["enum"]
    );

    assert_eq!(
        failure["$defs"]["errorCategory"]["enum"],
        enum_array([
            ErrorCategory::Input,
            ErrorCategory::Configuration,
            ErrorCategory::Dependency,
            ErrorCategory::Media,
            ErrorCategory::Execution,
            ErrorCategory::State,
            ErrorCategory::Io,
            ErrorCategory::Internal,
        ])
    );
    assert_eq!(
        failure["$defs"]["diagnosticCode"]["enum"],
        enum_array([
            PipelinePlanningDiagnosticCode::InvalidYaml,
            PipelinePlanningDiagnosticCode::InvalidJson,
            PipelinePlanningDiagnosticCode::InvalidSchema,
            PipelinePlanningDiagnosticCode::UnsupportedVersion,
            PipelinePlanningDiagnosticCode::LegacyPipelineV2,
            PipelinePlanningDiagnosticCode::RenderflowRemoved,
            PipelinePlanningDiagnosticCode::InvalidConfiguration,
            PipelinePlanningDiagnosticCode::InvalidIdentifier,
            PipelinePlanningDiagnosticCode::DuplicateIdentifier,
            PipelinePlanningDiagnosticCode::InvalidDependency,
            PipelinePlanningDiagnosticCode::UnknownDependency,
            PipelinePlanningDiagnosticCode::ForwardDependency,
            PipelinePlanningDiagnosticCode::UnknownArtifact,
            PipelinePlanningDiagnosticCode::InvalidBinding,
            PipelinePlanningDiagnosticCode::UnknownPort,
            PipelinePlanningDiagnosticCode::CardinalityMismatch,
            PipelinePlanningDiagnosticCode::ArtifactTypeMismatch,
            PipelinePlanningDiagnosticCode::StreamRoleMismatch,
            PipelinePlanningDiagnosticCode::UnsafeArtifactPath,
            PipelinePlanningDiagnosticCode::ArtifactPathOverlap,
            PipelinePlanningDiagnosticCode::InvalidValidation,
            PipelinePlanningDiagnosticCode::InvalidFinalOutput,
            PipelinePlanningDiagnosticCode::MissingInputBinding,
            PipelinePlanningDiagnosticCode::UnexpectedInputBinding,
            PipelinePlanningDiagnosticCode::InputUnavailable,
            PipelinePlanningDiagnosticCode::UnsupportedInputKind,
            PipelinePlanningDiagnosticCode::SymlinkInput,
            PipelinePlanningDiagnosticCode::InvalidPolicy,
            PipelinePlanningDiagnosticCode::ProviderUnavailable,
            PipelinePlanningDiagnosticCode::InvalidProviderRegistration,
            PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
            PipelinePlanningDiagnosticCode::DigestMismatch,
            PipelinePlanningDiagnosticCode::Io,
        ])
    );
}

#[test]
fn published_v3_schemas_close_every_declared_object_shape() {
    for (name, document) in [
        (
            "configuration",
            schema(include_str!(
                "../docs/contracts/pipeline-v3-configuration-v1.schema.json"
            )),
        ),
        (
            "plan",
            schema(include_str!(
                "../docs/contracts/pipeline-v3-plan-v1.schema.json"
            )),
        ),
        (
            "failure",
            schema(include_str!(
                "../docs/contracts/pipeline-v3-planning-failure-v1.schema.json"
            )),
        ),
        (
            "registration",
            schema(include_str!(
                "../docs/contracts/provider-registration-v1.schema.json"
            )),
        ),
    ] {
        assert_eq!(
            document["additionalProperties"],
            Value::Bool(false),
            "{name} root must be closed"
        );
        for (definition_name, definition) in document["$defs"].as_object().into_iter().flatten() {
            if definition["type"] == Value::String("object".to_owned()) {
                assert_eq!(
                    definition["additionalProperties"],
                    Value::Bool(false),
                    "{name} definition {definition_name} must be closed"
                );
            }
        }
    }
}
