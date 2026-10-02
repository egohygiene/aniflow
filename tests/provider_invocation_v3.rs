use std::ffi::OsString;
use std::path::PathBuf;

use aniflow::{
    ArtifactKind, ArtifactRole, PROVIDER_INVOCATION_ARGUMENT,
    PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1, PROVIDER_INVOCATION_SCHEMA_V1,
    PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V2, PROVIDER_INVOCATION_SCHEMA_V2,
    ProviderInvocationRequest, StreamRole, provider_invocation_arguments,
};
use serde_json::Value;

const INVOCATION: &[u8] =
    include_bytes!("../docs/contracts/examples/provider-invocation-v1.example.json");

fn published_invocation() -> ProviderInvocationRequest {
    ProviderInvocationRequest::from_json_slice(INVOCATION)
        .expect("published provider invocation should satisfy the public model")
}

#[test]
fn published_provider_invocation_round_trips_through_the_public_model() {
    let invocation = published_invocation();
    assert_eq!(invocation.schema, PROVIDER_INVOCATION_SCHEMA_V1);
    assert_eq!(
        invocation.execution_semantics,
        PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1
    );
    assert_eq!(
        invocation.inputs[0].artifact_role,
        ArtifactRole::TemporalComponent
    );
    assert_eq!(invocation.inputs[0].stream_role, Some(StreamRole::Video));
    assert_eq!(invocation.outputs[0].kind, ArtifactKind::Directory);

    let actual: Value = serde_json::from_slice(
        &invocation
            .to_json_bytes()
            .expect("provider invocation should encode"),
    )
    .expect("encoded provider invocation should be JSON");
    let expected: Value =
        serde_json::from_slice(INVOCATION).expect("published invocation should be JSON");
    assert_eq!(actual, expected);
}

#[test]
fn provider_invocation_uses_one_fixed_direct_argv_option() {
    let request_path = PathBuf::from("/var/lib/aniflow/run/provider-invocation.json");
    let arguments = provider_invocation_arguments(&request_path)
        .expect("an absolute normalized invocation path should be accepted");

    assert_eq!(
        arguments,
        vec![
            OsString::from(PROVIDER_INVOCATION_ARGUMENT),
            request_path.into_os_string(),
        ]
    );
    assert!(provider_invocation_arguments("relative/invocation.json").is_err());
    assert!(provider_invocation_arguments("/var/lib/aniflow/../escape.json").is_err());
}

#[test]
fn provider_invocation_rejects_unknown_versions_fields_and_non_normalized_nulls() {
    let baseline: Value =
        serde_json::from_slice(INVOCATION).expect("published invocation should be JSON");

    let mut unknown_version = baseline.clone();
    unknown_version["schema"] = Value::String("aniflow.provider-invocation/v99".to_owned());
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&unknown_version).expect("changed invocation should encode"),
    )
    .expect_err("unknown invocation versions must fail closed");
    assert!(error.message().contains("unsupported"));

    let mut unknown_field = baseline.clone();
    unknown_field["arguments"] = serde_json::json!(["--unsafe"]);
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&unknown_field).expect("changed invocation should encode"),
    )
    .expect_err("unknown invocation fields must be rejected");
    assert!(error.message().contains("unknown field"));

    let mut explicit_null = baseline;
    explicit_null["inputs"][0]["stream_role"] = Value::Null;
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&explicit_null).expect("changed invocation should encode"),
    )
    .expect_err("explicit null must not alias an omitted optional field");
    assert!(
        error
            .message()
            .contains("normalized contract representation")
    );
}

#[test]
fn provider_invocation_rejects_tampered_configuration_and_unsafe_bindings() {
    let baseline: Value =
        serde_json::from_slice(INVOCATION).expect("published invocation should be JSON");

    let mut changed_configuration = baseline.clone();
    changed_configuration["configuration"]["values"]["scale"] = Value::from(4);
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&changed_configuration).expect("changed invocation should encode"),
    )
    .expect_err("effective configuration digest mismatches must be rejected");
    assert!(error.message().contains("effective configuration digest"));

    let mut duplicate_input = baseline.clone();
    let input = duplicate_input["inputs"][0].clone();
    duplicate_input["inputs"]
        .as_array_mut()
        .expect("inputs should be an array")
        .push(input);
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&duplicate_input).expect("changed invocation should encode"),
    )
    .expect_err("duplicate port and artifact input bindings must be rejected");
    assert!(error.message().contains("bound more than once"));

    let mut overlapping_output = baseline.clone();
    overlapping_output["outputs"][0]["path"] = overlapping_output["inputs"][0]["path"].clone();
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&overlapping_output).expect("changed invocation should encode"),
    )
    .expect_err("outputs must not overlap immutable input paths");
    assert!(error.message().contains("immutable inputs"));

    let mut relative_output = baseline;
    relative_output["outputs"][0]["path"] = Value::String("artifacts/output".to_owned());
    let error = ProviderInvocationRequest::from_json_slice(
        &serde_json::to_vec(&relative_output).expect("changed invocation should encode"),
    )
    .expect_err("relative provider output bindings must be rejected");
    assert!(error.message().contains("absolute normalized path"));
}

#[test]
fn published_schema_tracks_the_closed_request_shape_and_public_enums() {
    let schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-invocation-v1.schema.json"
    ))
    .expect("published provider invocation schema should be valid JSON");

    assert_eq!(
        schema["properties"]["schema"]["const"],
        PROVIDER_INVOCATION_SCHEMA_V1
    );
    assert_eq!(
        schema["properties"]["execution_semantics"]["const"],
        PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["inputs"]["minItems"], 1);
    assert_eq!(
        schema["$defs"]["artifactBinding"]["properties"]["artifact_role"]["enum"],
        serde_json::json!([
            "temporal_source",
            "temporal_component",
            "intermediate",
            "candidate_master",
            "validated_master",
            "validation_evidence",
            "run_evidence",
            "delivery_receipt"
        ])
    );
    assert_eq!(
        schema["$defs"]["artifactBinding"]["properties"]["stream_role"]["enum"],
        serde_json::json!([
            "video",
            "audio",
            "subtitle",
            "transcript",
            "timed_metadata",
            "attachment"
        ])
    );
    assert_eq!(
        schema["$defs"]["artifactBinding"]["properties"]["kind"]["enum"],
        serde_json::json!(["file", "directory"])
    );
}

fn artifact_set_invocation() -> ProviderInvocationRequest {
    let baseline = published_invocation();
    let mut sibling = baseline.outputs[0].clone();
    sibling.artifact_id = "second-artifact".to_owned();
    sibling.path = baseline.outputs[0]
        .path
        .parent()
        .expect("output has a parent")
        .join("second-artifact");
    let mut outputs = baseline.outputs;
    outputs.push(sibling);
    ProviderInvocationRequest::new_artifact_set(
        baseline.stage_id,
        baseline.provider_lock_sha256,
        baseline.configuration,
        baseline.inputs,
        outputs,
    )
    .expect("the exact v2 output set should construct")
}

#[test]
fn artifact_set_invocation_round_trips_distinct_members_on_the_same_port() {
    let invocation = artifact_set_invocation();
    assert_eq!(invocation.schema, PROVIDER_INVOCATION_SCHEMA_V2);
    assert_eq!(invocation.execution_semantics, PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V2);
    assert_eq!(invocation.outputs[0].port, invocation.outputs[1].port);
    assert_ne!(invocation.outputs[0].artifact_id, invocation.outputs[1].artifact_id);
    let bytes = invocation.to_json_bytes().expect("v2 should encode");
    assert_eq!(ProviderInvocationRequest::from_json_slice(&bytes).expect("v2 should decode"), invocation);
}

#[test]
fn artifact_set_invocation_refuses_downgrade_mixed_versions_and_empty_sets() {
    let baseline = artifact_set_invocation();
    let mut downgraded = baseline.clone();
    downgraded.schema = PROVIDER_INVOCATION_SCHEMA_V1.to_owned();
    downgraded.execution_semantics = PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1.to_owned();
    assert!(downgraded.validate().expect_err("v1 must refuse repeated ports").message().contains("one artifact"));
    let mut mixed = baseline.clone();
    mixed.execution_semantics = PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1.to_owned();
    assert!(mixed.validate().is_err());
    let mut empty = baseline;
    empty.outputs.clear();
    assert!(empty.validate().is_err());
}

#[test]
fn artifact_set_invocation_refuses_duplicate_ids_and_portable_path_collisions() {
    let baseline = artifact_set_invocation();
    let mut duplicate_id = baseline.clone();
    duplicate_id.outputs[1].artifact_id = duplicate_id.outputs[0].artifact_id.clone();
    assert!(duplicate_id.validate().is_err());
    let mut duplicate_path = baseline.clone();
    duplicate_path.outputs[1].path = duplicate_path.outputs[0].path.clone();
    assert!(duplicate_path.validate().is_err());
    let mut nested_path = baseline.clone();
    nested_path.outputs[1].path = nested_path.outputs[0].path.join("nested");
    assert!(nested_path.validate().is_err());
    let mut folded_path = baseline;
    folded_path.outputs[1].path = PathBuf::from(folded_path.outputs[0].path.to_str().expect("UTF-8 path").to_uppercase());
    assert!(folded_path.validate().is_err());
}

#[test]
fn published_artifact_set_invocation_example_uses_the_v2_schema() {
    let invocation = ProviderInvocationRequest::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-invocation-v2.example.json"
    )).expect("authored v2 example should satisfy the public model");
    assert_eq!(invocation.schema, PROVIDER_INVOCATION_SCHEMA_V2);
    assert!(invocation.outputs.len() >= 2);
    let schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-invocation-v2.schema.json"
    )).expect("authored v2 schema should be JSON");
    assert_eq!(schema["properties"]["schema"]["const"], PROVIDER_INVOCATION_SCHEMA_V2);
    assert_eq!(schema["properties"]["execution_semantics"]["const"], PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V2);
    assert_eq!(schema["properties"]["outputs"]["minItems"], 1);
    assert_eq!(schema["additionalProperties"], false);
}
