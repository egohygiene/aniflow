use aniflow::{
    AvailabilityCode, AvailabilityReason, CommandName, Error, ErrorCategory, MachineEnvelope,
    MachineOutcome, PIPELINE_PLANNING_FAILURE_SCHEMA_V1, PipelinePlanningDiagnostic,
    PipelinePlanningDiagnosticCode, PipelinePlanningFailure, ProviderCandidate,
    ProviderResolutionAttempt, ProviderSelection, ProviderSelectionSource, RunStatus,
};
use serde_json::Value;

#[test]
fn status_success_matches_the_v1_golden_fixture() {
    let fixture = include_bytes!("fixtures/contracts/status-success-v1.json");
    let envelope = MachineEnvelope::<RunStatus>::from_json_slice(fixture)
        .expect("status fixture should be a supported contract");
    let actual = serde_json::to_value(envelope).expect("status envelope should serialize");
    let expected: Value = serde_json::from_slice(fixture).expect("status fixture should parse");

    assert_eq!(actual, expected);
}

#[test]
fn input_error_matches_the_v1_golden_fixture() {
    let error = Error::new(
        ErrorCategory::Input,
        "input video does not exist: missing.mp4",
    );
    let actual = serde_json::to_value(MachineEnvelope::<Value>::failure(
        CommandName::Inspect,
        &error,
        None,
    ))
    .expect("error envelope should serialize");
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/contracts/input-error-v1.json"))
            .expect("error fixture should parse");

    assert_eq!(actual, expected);
}

#[test]
fn parser_rejects_unknown_machine_contract_versions() {
    let unsupported = br#"{
        "schema_version": 99,
        "command": "status",
        "status": "success",
        "result": {
            "run_id": "run",
            "pipeline_name": "pipeline",
            "source_file": "source.mp4",
            "stages": [],
            "artifacts": []
        }
    }"#;

    let error = MachineEnvelope::<RunStatus>::from_json_slice(unsupported)
        .expect_err("unknown contract versions must be rejected");

    assert_eq!(error.category(), ErrorCategory::Configuration);
    assert!(
        error
            .message()
            .contains("unsupported machine contract version 99")
    );
}

#[test]
fn error_categories_keep_the_documented_exit_codes() {
    assert_eq!(ErrorCategory::Input.exit_code(), 3);
    assert_eq!(ErrorCategory::Configuration.exit_code(), 4);
    assert_eq!(ErrorCategory::Dependency.exit_code(), 5);
    assert_eq!(ErrorCategory::Media.exit_code(), 6);
    assert_eq!(ErrorCategory::Execution.exit_code(), 7);
    assert_eq!(ErrorCategory::State.exit_code(), 8);
    assert_eq!(ErrorCategory::Io.exit_code(), 9);
    assert_eq!(ErrorCategory::Internal.exit_code(), 70);
}

#[test]
fn plan_v3_command_name_has_a_stable_machine_spelling() {
    assert_eq!(
        serde_json::to_value(CommandName::PlanV3).expect("command name should serialize"),
        Value::String("plan_v3".to_owned())
    );
    assert_eq!(CommandName::PlanV3.to_string(), "plan_v3");
}

#[test]
fn planning_failure_envelopes_retain_typed_diagnostics() {
    let planning = PipelinePlanningFailure {
        schema: PIPELINE_PLANNING_FAILURE_SCHEMA_V1.to_owned(),
        category: ErrorCategory::Dependency,
        message: "provider fixture is unavailable".to_owned(),
        diagnostics: vec![PipelinePlanningDiagnostic {
            code: PipelinePlanningDiagnosticCode::ProviderUnavailable,
            category: ErrorCategory::Dependency,
            message: "provider fixture is unavailable".to_owned(),
            stage: Some("transform".to_owned()),
            field: Some("provider".to_owned()),
            attempts: vec![ProviderResolutionAttempt {
                candidate: ProviderCandidate {
                    registration_id: "fixture".to_owned(),
                },
                selection: ProviderSelection {
                    source: ProviderSelectionSource::Primary,
                    fallback_index: None,
                },
                available: false,
                reasons: vec![AvailabilityReason {
                    code: AvailabilityCode::NotRegistered,
                    detail: "provider registration was not supplied".to_owned(),
                }],
            }],
        }],
    };
    planning
        .validate()
        .expect("planning failure fixture should satisfy its typed contract");
    let error = Error::new(planning.category(), planning.message.clone());
    let encoded = serde_json::to_vec(&MachineEnvelope::failure(
        CommandName::PlanV3,
        &error,
        Some(planning.clone()),
    ))
    .expect("planning failure envelope should serialize");
    let decoded = MachineEnvelope::<PipelinePlanningFailure>::from_json_slice(&encoded)
        .expect("planning failure envelope should parse");

    match decoded.outcome {
        MachineOutcome::Error { result, .. } => {
            let result = result.expect("planning failure result should be retained");
            result
                .validate()
                .expect("decoded planning failure should satisfy its typed contract");
            assert_eq!(result, planning);
        }
        _ => panic!("planning failure should remain an error outcome"),
    }
}
