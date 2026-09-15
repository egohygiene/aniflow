#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use aniflow::{
    ArtifactRole, AvailabilityCode, CapabilityReference, ComponentIdentity, ComponentInventory,
    ErrorCategory, HostResources, PipelineInputBinding, PipelinePlanningContext,
    PipelinePlanningDiagnosticCode, PipelinePlanningFailure, PipelineV3Configuration,
    PipelineV3Plan, ProviderCandidate, ProviderConfiguration, ProviderManifest, ProviderReference,
    ProviderRegistration, ProviderRegistry, ProviderSelectionSource, SideEffect,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use walkdir::WalkDir;

const BASE_PIPELINE: &str = r#"schema: aniflow.pipeline/v3
name: deterministic-frames
inputs:
  - id: source
    artifact_type: application/vnd.aniflow.frame-set+directory
    artifact_role: temporal_component
    stream_role: video
stages:
  - id: enhance
    capability:
      id: aniflow/frame.process
      version_requirement: ^1.0
    provider:
      primary:
        registration_id: example-local
    inputs:
      - port: frames
        artifacts: [source]
    outputs:
      - port: processed_frames
        artifacts:
          - id: enhanced
            relative_path: artifacts/enhance/frames
    validations:
      - id: enhanced-valid
        artifact: enhanced
        contract: aniflow.validation/frame-set/v1
outputs:
  - id: enhanced-frames
    artifact: enhanced
    required_validations: [enhanced-valid]
"#;

const REORDERED_PIPELINE: &str = r#"outputs:
  - required_validations:
      - enhanced-valid
    artifact: enhanced
    id: enhanced-frames
stages:
  - validations:
      - contract: aniflow.validation/frame-set/v1
        artifact: enhanced
        id: enhanced-valid
    outputs:
      - artifacts:
          - relative_path: artifacts/enhance/frames
            id: enhanced
        port: processed_frames
    inputs:
      - artifacts:
          - source
        port: frames
    provider:
      fallbacks: []
      primary:
        registration_id: example-local
    capability:
      version_requirement: ^1.0
      id: aniflow/frame.process
    depends_on: []
    id: enhance
inputs:
  - stream_role: video
    artifact_role: temporal_component
    artifact_type: application/vnd.aniflow.frame-set+directory
    id: source
description: ""
name: deterministic-frames
schema: aniflow.pipeline/v3
"#;

#[test]
fn canonical_json_and_digest_are_stable_and_self_validating() {
    let fixture = PlanningFixture::new("canonical");
    let configuration = parse_pipeline(BASE_PIPELINE);
    let registry = fixture.registry(vec![("example-local", ProviderVariant::default())]);
    let plan = resolve(
        &configuration,
        &fixture.source,
        &registry,
        &planning_context(),
    );

    plan.validate().expect("resolved plan should validate");
    let canonical = plan
        .canonical_json_bytes()
        .expect("resolved plan should have canonical JSON");
    assert_eq!(
        canonical,
        plan.canonical_json_bytes()
            .expect("canonical encoding should be repeatable")
    );
    let parsed = PipelineV3Plan::from_json_slice(&canonical)
        .expect("canonical JSON should round-trip through the public parser");
    assert_eq!(parsed, plan);
    assert_eq!(plan.schema, aniflow::PIPELINE_V3_PLAN_SCHEMA_V1);
    assert_eq!(plan.algorithm, "sha256");
    assert_eq!(plan.canonicalization, aniflow::CANONICAL_JSON_SCHEMA_V1);

    let rendered = String::from_utf8(canonical).expect("canonical plan JSON should be UTF-8");
    assert!(!rendered.contains(&fixture.root().display().to_string()));
    for forbidden in [
        "started_at",
        "finished_at",
        "run_id",
        "arguments",
        "environment",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "canonical plan must not contain ephemeral field {forbidden}"
        );
    }

    let mut tampered = serde_json::to_value(&plan).expect("plan should serialize");
    tampered["plan_sha256"] = Value::String("0".repeat(64));
    let failure = PipelineV3Plan::from_json_slice(
        &serde_json::to_vec(&tampered).expect("tampered plan should encode"),
    )
    .expect_err("a stale plan digest must fail closed");
    assert_failure_code(&failure, PipelinePlanningDiagnosticCode::DigestMismatch);

    let mut semantically_invalid = plan;
    semantically_invalid.payload.policy.host.cpu_threads = 1;
    semantically_invalid.plan_sha256 = canonical_sha256(&semantically_invalid.payload);
    let failure = semantically_invalid
        .validate()
        .expect_err("validly digested but contradictory plan evidence must fail closed");
    assert_failure_code(
        &failure,
        PipelinePlanningDiagnosticCode::InvalidResolvedPlan,
    );
}

#[test]
fn mapping_order_defaults_description_and_policy_order_do_not_change_the_plan() {
    let fixture = PlanningFixture::new("normalization");
    let baseline = parse_pipeline(BASE_PIPELINE);
    let reordered = parse_pipeline(REORDERED_PIPELINE);
    assert_eq!(baseline, reordered);

    let registry = fixture.registry(vec![("example-local", ProviderVariant::default())]);
    let baseline_plan = resolve(&baseline, &fixture.source, &registry, &planning_context());
    let mut reordered_context = planning_context();
    reordered_context.allowed_side_effects.reverse();
    reordered_context
        .allowed_side_effects
        .push(SideEffect::FilesystemRead);
    let reordered_plan = resolve(&reordered, &fixture.source, &registry, &reordered_context);
    assert_eq!(baseline_plan.plan_sha256, reordered_plan.plan_sha256);
    assert_eq!(
        baseline_plan.canonical_json_bytes().unwrap(),
        reordered_plan.canonical_json_bytes().unwrap()
    );

    let mut documented = baseline;
    documented.description = "Human guidance is not executable intent.".to_owned();
    let documented_plan = resolve(&documented, &fixture.source, &registry, &planning_context());
    assert_eq!(baseline_plan.plan_sha256, documented_plan.plan_sha256);
}

#[test]
fn equivalent_inputs_and_providers_in_different_roots_produce_identical_plans() {
    let left = PlanningFixture::new("left-root");
    let right = PlanningFixture::new("right-root");
    let configuration = parse_pipeline(BASE_PIPELINE);
    let left_registry = left.registry(vec![("example-local", ProviderVariant::default())]);
    let right_registry = right.registry(vec![("example-local", ProviderVariant::default())]);

    let left_plan = resolve(
        &configuration,
        &left.source,
        &left_registry,
        &planning_context(),
    );
    let right_plan = resolve(
        &configuration,
        &right.source,
        &right_registry,
        &planning_context(),
    );
    assert_eq!(left_plan.plan_sha256, right_plan.plan_sha256);
    assert_eq!(
        left_plan.canonical_json_bytes().unwrap(),
        right_plan.canonical_json_bytes().unwrap()
    );

    let rendered = String::from_utf8(left_plan.canonical_json_bytes().unwrap()).unwrap();
    assert!(!rendered.contains(&left.root().display().to_string()));
    assert!(!rendered.contains(&right.root().display().to_string()));
}

#[test]
fn every_material_input_configuration_provider_and_output_change_changes_the_digest() {
    let fixture = PlanningFixture::new("material-identities");
    let configuration = parse_pipeline(BASE_PIPELINE);
    let baseline_registry = fixture.registry(vec![("example-local", ProviderVariant::default())]);
    let baseline = resolve(
        &configuration,
        &fixture.source,
        &baseline_registry,
        &planning_context(),
    );

    fs::write(
        fixture.source.join("frame-00000001.png"),
        b"different immutable frame bytes",
    )
    .expect("input fixture should be mutable between independent plans");
    let changed_input = resolve(
        &configuration,
        &fixture.source,
        &baseline_registry,
        &planning_context(),
    );
    assert_digest_changed(&baseline, &changed_input, "input content");
    fixture.restore_source();

    let changed_configuration_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            scale: 3,
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_configuration_registry,
            &planning_context(),
        ),
        "effective provider configuration",
    );

    let changed_provider_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            provider_id: "org.egohygiene.aniflow.alternate-frame".to_owned(),
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_provider_registry,
            &planning_context(),
        ),
        "provider identity",
    );

    let changed_capability_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            capability_version: "1.1.0".to_owned(),
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_capability_registry,
            &planning_context(),
        ),
        "capability version",
    );

    let changed_tool_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            tool_version: "2.16.0".to_owned(),
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_tool_registry,
            &planning_context(),
        ),
        "tool lock",
    );

    let changed_model_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            model_digest: "b".repeat(64),
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_model_registry,
            &planning_context(),
        ),
        "model lock",
    );

    let changed_implementation_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            implementation_id: "alternate-implementation".to_owned(),
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_implementation_registry,
            &planning_context(),
        ),
        "implementation lock",
    );

    let alternate_executable = fixture.write_provider_executable("provider-bin-v2", "# v2\n");
    let changed_executable_registry = registry_with(
        &alternate_executable,
        vec![("example-local", ProviderVariant::default())],
    );
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_executable_registry,
            &planning_context(),
        ),
        "executable digest",
    );

    let mut changed_output = configuration.clone();
    changed_output.stages[0].outputs[0].artifacts[0].relative_path =
        "artifacts/enhance/alternate-frames".to_owned();
    assert_digest_changed(
        &baseline,
        &resolve(
            &changed_output,
            &fixture.source,
            &baseline_registry,
            &planning_context(),
        ),
        "expected output",
    );

    let mut changed_validation = configuration.clone();
    changed_validation.stages[0].validations[0].contract =
        "aniflow.validation/frame-set/v2".to_owned();
    assert_digest_changed(
        &baseline,
        &resolve(
            &changed_validation,
            &fixture.source,
            &baseline_registry,
            &planning_context(),
        ),
        "validation obligation",
    );

    let changed_role_registry = fixture.registry(vec![(
        "example-local",
        ProviderVariant {
            output_role: ArtifactRole::CandidateMaster,
            ..ProviderVariant::default()
        },
    )]);
    assert_digest_changed(
        &baseline,
        &resolve(
            &configuration,
            &fixture.source,
            &changed_role_registry,
            &planning_context(),
        ),
        "output artifact role",
    );

    let ordered = two_independent_stages(&configuration);
    let ordered_plan = resolve(
        &ordered,
        &fixture.source,
        &baseline_registry,
        &planning_context(),
    );
    let mut reordered = ordered;
    reordered.stages.swap(0, 1);
    let reordered_plan = resolve(
        &reordered,
        &fixture.source,
        &baseline_registry,
        &planning_context(),
    );
    assert_digest_changed(&ordered_plan, &reordered_plan, "processor order");
}

#[test]
fn provider_precedence_and_role_transitions_are_explicit_in_the_plan() {
    let fixture = PlanningFixture::new("provider-precedence");
    let mut fallback_configuration = parse_pipeline(BASE_PIPELINE);
    fallback_configuration.stages[0].provider.replacement = Some(ProviderCandidate {
        registration_id: "missing-replacement".to_owned(),
    });
    fallback_configuration.stages[0]
        .provider
        .primary
        .registration_id = "missing-primary".to_owned();
    fallback_configuration.stages[0]
        .provider
        .fallbacks
        .push(ProviderCandidate {
            registration_id: "fallback".to_owned(),
        });
    let fallback_registry = fixture.registry(vec![("fallback", ProviderVariant::default())]);
    let fallback_plan = resolve(
        &fallback_configuration,
        &fixture.source,
        &fallback_registry,
        &planning_context(),
    );
    let stage = &fallback_plan.payload.stages[0];
    assert_eq!(stage.resolution_attempts.len(), 3);
    assert_eq!(
        stage.resolution_attempts[0].reason_codes,
        vec![AvailabilityCode::NotRegistered]
    );
    assert_eq!(
        stage.resolution_attempts[1].reason_codes,
        vec![AvailabilityCode::NotRegistered]
    );
    assert!(stage.resolution_attempts[2].available);
    assert_eq!(
        stage.provider_lock.payload.selection.source,
        ProviderSelectionSource::Fallback
    );
    assert_eq!(
        stage.provider_lock.payload.selection.fallback_index,
        Some(0)
    );

    let mut replacement_configuration = fallback_configuration.clone();
    replacement_configuration.stages[0]
        .provider
        .replacement
        .as_mut()
        .unwrap()
        .registration_id = "replacement".to_owned();
    let replacement_registry = fixture.registry(vec![
        ("replacement", ProviderVariant::default()),
        ("fallback", ProviderVariant::default()),
    ]);
    let replacement_plan = resolve(
        &replacement_configuration,
        &fixture.source,
        &replacement_registry,
        &planning_context(),
    );
    let replacement = &replacement_plan.payload.stages[0];
    assert_eq!(replacement.resolution_attempts.len(), 1);
    assert_eq!(
        replacement.provider_lock.payload.selection.source,
        ProviderSelectionSource::Replacement
    );

    let chained = chained_role_transition(&parse_pipeline(BASE_PIPELINE));
    let chained_registry = fixture.registry(vec![("example-local", ProviderVariant::default())]);
    let chained_plan = resolve(
        &chained,
        &fixture.source,
        &chained_registry,
        &planning_context(),
    );
    assert_eq!(
        chained_plan.payload.stages[0].outputs[0].artifacts[0].artifact_role,
        ArtifactRole::Intermediate
    );
    assert_eq!(
        chained_plan.payload.stages[1].capability.inputs[0].artifact_role,
        ArtifactRole::TemporalComponent
    );
    assert_eq!(
        chained_plan.payload.stages[1].inputs[0].artifacts,
        vec!["enhanced"]
    );
}

#[test]
fn typed_failures_cover_provider_inputs_graph_ports_cardinality_and_paths() {
    let fixture = PlanningFixture::new("typed-failures");
    let configuration = parse_pipeline(BASE_PIPELINE);
    let registry = fixture.registry(vec![("example-local", ProviderVariant::default())]);

    let empty_registry = ProviderRegistry::new();
    let unavailable = aniflow::resolve_pipeline_v3(
        &configuration,
        &[PipelineInputBinding::new("source", &fixture.source)],
        &empty_registry,
        &planning_context(),
    )
    .expect_err("an unregistered provider must fail planning");
    assert_failure_code(
        &unavailable,
        PipelinePlanningDiagnosticCode::ProviderUnavailable,
    );
    assert_eq!(unavailable.category(), ErrorCategory::Dependency);
    assert_eq!(
        unavailable.diagnostics()[0].attempts[0].reasons[0].code,
        AvailabilityCode::NotRegistered
    );

    let mut incompatible = configuration.clone();
    incompatible.stages[0].capability.version_requirement = ">=2.0".to_owned();
    let incompatible = resolve_failure(
        &incompatible,
        &[PipelineInputBinding::new("source", &fixture.source)],
        &registry,
    );
    assert_failure_code(
        &incompatible,
        PipelinePlanningDiagnosticCode::ProviderUnavailable,
    );
    assert!(
        incompatible.diagnostics()[0].attempts[0]
            .reasons
            .iter()
            .any(|reason| reason.code == AvailabilityCode::VersionMismatch)
    );

    let missing_input = resolve_failure(&configuration, &[], &registry);
    assert_failure_code(
        &missing_input,
        PipelinePlanningDiagnosticCode::MissingInputBinding,
    );

    let duplicate_input = resolve_failure(
        &configuration,
        &[
            PipelineInputBinding::new("source", &fixture.source),
            PipelineInputBinding::new("source", &fixture.source),
        ],
        &registry,
    );
    assert_failure_code(
        &duplicate_input,
        PipelinePlanningDiagnosticCode::UnexpectedInputBinding,
    );

    let unexpected_input = resolve_failure(
        &configuration,
        &[
            PipelineInputBinding::new("source", &fixture.source),
            PipelineInputBinding::new("undeclared", &fixture.source),
        ],
        &registry,
    );
    assert_failure_code(
        &unexpected_input,
        PipelinePlanningDiagnosticCode::UnexpectedInputBinding,
    );

    let mut duplicate_authored_input = configuration.clone();
    duplicate_authored_input
        .inputs
        .push(duplicate_authored_input.inputs[0].clone());
    assert_configuration_failure(
        &duplicate_authored_input,
        PipelinePlanningDiagnosticCode::DuplicateIdentifier,
    );

    let mut unknown_dependency = configuration.clone();
    unknown_dependency.stages[0]
        .depends_on
        .push("missing-stage".to_owned());
    assert_configuration_failure(
        &unknown_dependency,
        PipelinePlanningDiagnosticCode::UnknownDependency,
    );

    let mut forward_dependency = two_independent_stages(&configuration);
    forward_dependency.stages[0]
        .depends_on
        .push("polish".to_owned());
    assert_configuration_failure(
        &forward_dependency,
        PipelinePlanningDiagnosticCode::ForwardDependency,
    );

    let mut missing_artifact_dependency = chained_role_transition(&configuration);
    missing_artifact_dependency.stages[1].depends_on.clear();
    assert_configuration_failure(
        &missing_artifact_dependency,
        PipelinePlanningDiagnosticCode::InvalidDependency,
    );

    let mut unsafe_path = configuration.clone();
    unsafe_path.stages[0].outputs[0].artifacts[0].relative_path = "../escape".to_owned();
    assert_configuration_failure(
        &unsafe_path,
        PipelinePlanningDiagnosticCode::UnsafeArtifactPath,
    );

    let mut overlapping_path = configuration.clone();
    let mut overlapping_artifact = overlapping_path.stages[0].outputs[0].artifacts[0].clone();
    overlapping_artifact.id = "nested".to_owned();
    overlapping_artifact.relative_path = "artifacts/enhance/frames/nested".to_owned();
    overlapping_path.stages[0].outputs[0]
        .artifacts
        .push(overlapping_artifact);
    assert_configuration_failure(
        &overlapping_path,
        PipelinePlanningDiagnosticCode::ArtifactPathOverlap,
    );

    let mut unknown_port = configuration.clone();
    unknown_port.stages[0].inputs[0].port = "unknown".to_owned();
    let unknown_port = resolve_failure(
        &unknown_port,
        &[PipelineInputBinding::new("source", &fixture.source)],
        &registry,
    );
    assert_failure_code(&unknown_port, PipelinePlanningDiagnosticCode::UnknownPort);

    let mut cardinality = configuration.clone();
    let mut second = cardinality.stages[0].outputs[0].artifacts[0].clone();
    second.id = "enhanced-second".to_owned();
    second.relative_path = "artifacts/enhance/second".to_owned();
    cardinality.stages[0].outputs[0].artifacts.push(second);
    let cardinality = resolve_failure(
        &cardinality,
        &[PipelineInputBinding::new("source", &fixture.source)],
        &registry,
    );
    assert_failure_code(
        &cardinality,
        PipelinePlanningDiagnosticCode::CardinalityMismatch,
    );

    let mut wrong_type = configuration.clone();
    wrong_type.inputs[0].artifact_type = "application/octet-stream".to_owned();
    let wrong_type = resolve_failure(
        &wrong_type,
        &[PipelineInputBinding::new("source", &fixture.source)],
        &registry,
    );
    assert_failure_code(
        &wrong_type,
        PipelinePlanningDiagnosticCode::ArtifactTypeMismatch,
    );

    let source_symlink = fixture.root().join("source-symlink");
    symlink(&fixture.source, &source_symlink).expect("source symlink fixture should be created");
    let symlink_input = resolve_failure(
        &configuration,
        &[PipelineInputBinding::new("source", &source_symlink)],
        &registry,
    );
    assert_failure_code(&symlink_input, PipelinePlanningDiagnosticCode::SymlinkInput);
    fs::remove_file(source_symlink).expect("source symlink fixture should be removed");

    let reserved_input = fixture.source.join("CON");
    fs::write(&reserved_input, b"reserved portable name")
        .expect("reserved-name fixture should be written");
    let non_portable = resolve_failure(
        &configuration,
        &[PipelineInputBinding::new("source", &fixture.source)],
        &registry,
    );
    assert_failure_code(
        &non_portable,
        PipelinePlanningDiagnosticCode::UnsupportedInputKind,
    );
    fs::remove_file(reserved_input).expect("reserved-name fixture should be removed");

    let lowercase_input = fixture.source.join("portable-collision.txt");
    let uppercase_input = fixture.source.join("PORTABLE-COLLISION.TXT");
    fs::write(&lowercase_input, b"lowercase").expect("lowercase fixture should be written");
    fs::write(&uppercase_input, b"uppercase").expect("uppercase fixture should be written");
    let names = fs::read_dir(&fixture.source)
        .expect("source directory should be readable")
        .map(|entry| entry.expect("source entry should be readable").file_name())
        .collect::<Vec<_>>();
    if names.contains(&lowercase_input.file_name().unwrap().to_owned())
        && names.contains(&uppercase_input.file_name().unwrap().to_owned())
    {
        let case_collision = resolve_failure(
            &configuration,
            &[PipelineInputBinding::new("source", &fixture.source)],
            &registry,
        );
        assert_failure_code(
            &case_collision,
            PipelinePlanningDiagnosticCode::UnsupportedInputKind,
        );
    }
    fs::remove_file(lowercase_input).expect("lowercase fixture should be removed");
    if uppercase_input.exists() {
        fs::remove_file(uppercase_input).expect("uppercase fixture should be removed");
    }

    let mut invalid_validation = configuration.clone();
    invalid_validation.stages[0].validations[0].artifact = "source".to_owned();
    assert_configuration_failure(
        &invalid_validation,
        PipelinePlanningDiagnosticCode::InvalidValidation,
    );

    let mut invalid_final_output = configuration.clone();
    invalid_final_output.outputs[0].required_validations = vec!["unknown-validation".to_owned()];
    assert_configuration_failure(
        &invalid_final_output,
        PipelinePlanningDiagnosticCode::InvalidFinalOutput,
    );

    let mut duplicate_candidate = configuration;
    let duplicate_primary = duplicate_candidate.stages[0].provider.primary.clone();
    duplicate_candidate.stages[0]
        .provider
        .fallbacks
        .push(duplicate_primary);
    assert_configuration_failure(
        &duplicate_candidate,
        PipelinePlanningDiagnosticCode::InvalidConfiguration,
    );
}

#[test]
fn pipeline_v2_unknown_versions_and_renderflow_receive_migration_diagnostics() {
    let legacy = PipelineV3Configuration::from_yaml_str("version: 2\nname: legacy\n")
        .expect_err("Pipeline v2 must not be reinterpreted as Pipeline v3");
    assert_failure_code(&legacy, PipelinePlanningDiagnosticCode::LegacyPipelineV2);

    let renderflow = PipelineV3Configuration::from_yaml_str(
        "schema: aniflow.pipeline/v3\nname: invalid\nrenderflow:\n  enabled: false\n",
    )
    .expect_err("Pipeline v3 must reject cross-holon renderflow configuration");
    assert_failure_code(
        &renderflow,
        PipelinePlanningDiagnosticCode::RenderflowRemoved,
    );

    let legacy_version_field =
        PipelineV3Configuration::from_yaml_str("version: 3\nname: invalid-version-spelling\n")
            .expect_err("Pipeline v3 must use its explicit schema identifier");
    assert_failure_code(
        &legacy_version_field,
        PipelinePlanningDiagnosticCode::UnsupportedVersion,
    );

    let unknown =
        PipelineV3Configuration::from_yaml_str("schema: aniflow.pipeline/v99\nname: unknown\n")
            .expect_err("unknown Pipeline v3 schema versions must fail closed");
    assert_failure_code(&unknown, PipelinePlanningDiagnosticCode::InvalidSchema);
}

#[test]
fn planning_is_read_only_and_never_launches_the_selected_provider() {
    let fixture = PlanningFixture::new("purity");
    let configuration = parse_pipeline(BASE_PIPELINE);
    let registry = fixture.registry(vec![("example-local", ProviderVariant::default())]);
    let before = directory_snapshot(fixture.root());
    let source_before = fs::read(fixture.source.join("frame-00000001.png"))
        .expect("source fixture should be readable");

    let plan = resolve(
        &configuration,
        &fixture.source,
        &registry,
        &planning_context(),
    );

    assert!(!fixture.sentinel.exists(), "planning launched the provider");
    assert_eq!(
        fs::read(fixture.source.join("frame-00000001.png")).unwrap(),
        source_before,
        "planning mutated source bytes"
    );
    assert_eq!(directory_snapshot(fixture.root()), before);
    for forbidden in [".aniflow", "state", "output", "delivery", "providers"] {
        assert!(!fixture.root().join(forbidden).exists());
    }
    plan.validate()
        .expect("read-only planning should still produce a valid plan");
}

fn parse_pipeline(yaml: &str) -> PipelineV3Configuration {
    PipelineV3Configuration::from_yaml_str(yaml)
        .unwrap_or_else(|error| panic!("Pipeline v3 fixture should parse: {error:#?}"))
}

fn resolve(
    configuration: &PipelineV3Configuration,
    source: &Path,
    registry: &ProviderRegistry,
    context: &PipelinePlanningContext,
) -> PipelineV3Plan {
    aniflow::resolve_pipeline_v3(
        configuration,
        &[PipelineInputBinding::new("source", source)],
        registry,
        context,
    )
    .unwrap_or_else(|error| panic!("Pipeline v3 fixture should resolve: {error:#?}"))
}

fn resolve_failure(
    configuration: &PipelineV3Configuration,
    bindings: &[PipelineInputBinding],
    registry: &ProviderRegistry,
) -> PipelinePlanningFailure {
    aniflow::resolve_pipeline_v3(configuration, bindings, registry, &planning_context())
        .expect_err("Pipeline v3 fixture should fail planning")
}

fn assert_configuration_failure(
    configuration: &PipelineV3Configuration,
    expected: PipelinePlanningDiagnosticCode,
) {
    let failure = configuration
        .validate()
        .expect_err("invalid Pipeline v3 configuration should fail");
    assert_failure_code(&failure, expected);
}

fn assert_failure_code(
    failure: &PipelinePlanningFailure,
    expected: PipelinePlanningDiagnosticCode,
) {
    assert_eq!(
        failure
            .diagnostics()
            .first()
            .map(|diagnostic| diagnostic.code),
        Some(expected),
        "unexpected planning failure: {failure:#?}"
    );
    failure
        .validate()
        .expect("typed planning failure should self-validate");
}

fn assert_digest_changed(baseline: &PipelineV3Plan, changed: &PipelineV3Plan, dimension: &str) {
    assert_ne!(
        baseline.plan_sha256, changed.plan_sha256,
        "changing {dimension} must change the resolved plan digest"
    );
}

fn canonical_sha256(value: &impl Serialize) -> String {
    let value = serde_json::to_value(value).expect("canonical fixture value should serialize");
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).expect("canonical fixture JSON should encode");
    format!("{:x}", Sha256::digest(bytes))
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        scalar => scalar,
    }
}

fn planning_context() -> PipelinePlanningContext {
    PipelinePlanningContext {
        host: HostResources {
            cpu_threads: 8,
            memory_mib: 16 * 1024,
            storage_mib: 64 * 1024,
            gpu_available: true,
            network_available: false,
        },
        allowed_side_effects: vec![
            SideEffect::Subprocess,
            SideEffect::FilesystemWrite,
            SideEffect::Gpu,
            SideEffect::FilesystemRead,
        ],
        offline: true,
    }
}

fn two_independent_stages(configuration: &PipelineV3Configuration) -> PipelineV3Configuration {
    let mut configuration = configuration.clone();
    let mut second = configuration.stages[0].clone();
    second.id = "polish".to_owned();
    second.outputs[0].artifacts[0].id = "polished".to_owned();
    second.outputs[0].artifacts[0].relative_path = "artifacts/polish/frames".to_owned();
    second.validations[0].id = "polished-valid".to_owned();
    second.validations[0].artifact = "polished".to_owned();
    configuration.stages.push(second);
    let mut output = configuration.outputs[0].clone();
    output.id = "polished-frames".to_owned();
    output.artifact = "polished".to_owned();
    output.required_validations = vec!["polished-valid".to_owned()];
    configuration.outputs.push(output);
    configuration
}

fn chained_role_transition(configuration: &PipelineV3Configuration) -> PipelineV3Configuration {
    let mut configuration = configuration.clone();
    let mut second = configuration.stages[0].clone();
    second.id = "polish".to_owned();
    second.depends_on = vec!["enhance".to_owned()];
    second.inputs[0].artifacts = vec!["enhanced".to_owned()];
    second.outputs[0].artifacts[0].id = "polished".to_owned();
    second.outputs[0].artifacts[0].relative_path = "artifacts/polish/frames".to_owned();
    second.validations[0].id = "polished-valid".to_owned();
    second.validations[0].artifact = "polished".to_owned();
    configuration.stages.push(second);
    configuration.outputs = vec![aniflow::FinalOutputRequirement {
        id: "polished-frames".to_owned(),
        artifact: "polished".to_owned(),
        required_validations: vec!["polished-valid".to_owned()],
    }];
    configuration
}

#[derive(Debug, Clone)]
struct ProviderVariant {
    provider_id: String,
    provider_version: String,
    capability_version: String,
    scale: u64,
    tool_version: String,
    model_digest: String,
    implementation_id: String,
    output_role: ArtifactRole,
}

impl Default for ProviderVariant {
    fn default() -> Self {
        Self {
            provider_id: "org.egohygiene.aniflow.example-frame".to_owned(),
            provider_version: "1.0.0".to_owned(),
            capability_version: "1.0.0".to_owned(),
            scale: 2,
            tool_version: "2.15.0".to_owned(),
            model_digest: "a".repeat(64),
            implementation_id: "fixture-process".to_owned(),
            output_role: ArtifactRole::Intermediate,
        }
    }
}

fn registry_with(
    executable: &Path,
    registrations: Vec<(&str, ProviderVariant)>,
) -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    for (registration_id, variant) in registrations {
        registry
            .register(provider_registration(registration_id, executable, &variant))
            .expect("fixture provider registrations should be unique");
    }
    registry
}

fn provider_registration(
    registration_id: &str,
    executable: &Path,
    variant: &ProviderVariant,
) -> ProviderRegistration {
    let mut manifest = ProviderManifest::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-manifest-v1.example.json"
    ))
    .expect("published provider manifest fixture should validate");
    manifest.provider.id.clone_from(&variant.provider_id);
    manifest
        .provider
        .version
        .clone_from(&variant.provider_version);
    manifest.capabilities[0]
        .version
        .clone_from(&variant.capability_version);
    manifest.capabilities[0].outputs[0].artifact_role = variant.output_role;
    manifest.capabilities[0].requirements.models[0].sha256 = Some(variant.model_digest.clone());

    let published_configuration = ProviderConfiguration::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-configuration-v1.example.json"
    ))
    .expect("published provider configuration fixture should validate");
    let mut values = BTreeMap::new();
    values.insert("scale".to_owned(), Value::from(variant.scale));
    values.insert("tile_size".to_owned(), Value::from(128));
    let configuration = ProviderConfiguration::new(
        ProviderReference {
            id: variant.provider_id.clone(),
            version: variant.provider_version.clone(),
        },
        CapabilityReference {
            id: "aniflow/frame.process".to_owned(),
            version: variant.capability_version.clone(),
        },
        published_configuration.configuration_schema,
        values,
    )
    .expect("fixture provider configuration should validate");

    ProviderRegistration::new(
        registration_id,
        manifest,
        configuration,
        executable,
        &variant.implementation_id,
        ComponentInventory {
            tools: vec![ComponentIdentity {
                id: "upscayl-bin".to_owned(),
                version: variant.tool_version.clone(),
                sha256: None,
            }],
            codecs: vec![ComponentIdentity {
                id: "png".to_owned(),
                version: "1.6.43".to_owned(),
                sha256: None,
            }],
            models: vec![ComponentIdentity {
                id: "realesr-animevideov3".to_owned(),
                version: "1.0.0".to_owned(),
                sha256: Some(variant.model_digest.clone()),
            }],
        },
    )
    .expect("fixture provider registration should validate")
}

struct PlanningFixture {
    temporary: TempDir,
    source: PathBuf,
    executable: PathBuf,
    sentinel: PathBuf,
}

impl PlanningFixture {
    fn new(label: &str) -> Self {
        let temporary = tempfile::Builder::new()
            .prefix(&format!("aniflow-{label}-"))
            .tempdir()
            .expect("temporary fixture root should be created");
        let source = temporary.path().join("source-frames");
        fs::create_dir(&source).expect("source directory should be created");
        fs::write(source.join("frame-00000001.png"), b"immutable frame bytes")
            .expect("source fixture should be written");
        let sentinel = temporary.path().join("provider-executed");
        let executable = write_provider_executable(temporary.path(), "provider-bin", "");
        Self {
            temporary,
            source,
            executable,
            sentinel,
        }
    }

    fn root(&self) -> &Path {
        self.temporary.path()
    }

    fn registry(&self, registrations: Vec<(&str, ProviderVariant)>) -> ProviderRegistry {
        registry_with(&self.executable, registrations)
    }

    fn restore_source(&self) {
        fs::write(
            self.source.join("frame-00000001.png"),
            b"immutable frame bytes",
        )
        .expect("source fixture should be restored");
    }

    fn write_provider_executable(&self, name: &str, suffix: &str) -> PathBuf {
        write_provider_executable(self.root(), name, suffix)
    }
}

fn write_provider_executable(root: &Path, name: &str, suffix: &str) -> PathBuf {
    let executable = root.join(name);
    let script = format!(
        r#"#!/bin/sh
set -eu
provider_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
printf "provider was executed\n" > "${{provider_directory}}/provider-executed"
exit 97
{suffix}"#
    );
    fs::write(&executable, script).expect("fixture provider executable should be written");
    let mut permissions = fs::metadata(&executable)
        .expect("fixture provider metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("fixture provider should be executable");
    executable
}

fn directory_snapshot(root: &Path) -> Vec<(String, bool, Vec<u8>)> {
    let mut entries = WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .into_iter()
        .map(|entry| {
            let entry = entry.expect("fixture tree should be readable");
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("fixture path should remain beneath its root")
                .to_string_lossy()
                .replace('\\', "/");
            let is_directory = entry.file_type().is_dir();
            let bytes = if entry.file_type().is_file() {
                fs::read(entry.path()).expect("fixture file should be readable")
            } else {
                Vec::new()
            };
            (relative, is_directory, bytes)
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}
