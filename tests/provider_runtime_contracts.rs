use aniflow::{
    ArtifactKind, ProviderEvent, ProviderEventKind, ProviderExecutionFailureCode,
    ProviderExecutionOutcome, ProviderExecutionReport, ProviderLock, ProviderSelectionSource,
    SideEffect, StreamKind, TerminationReason,
};
use serde_json::Value;

fn enum_array<T: serde::Serialize, const N: usize>(values: [T; N]) -> Value {
    serde_json::to_value(values.as_slice()).expect("public enum values should serialize")
}

#[test]
fn published_runtime_examples_parse_and_verify_their_digests() {
    let provider_lock = ProviderLock::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-lock-v1.example.json"
    ))
    .expect("provider lock example should validate");
    let event: ProviderEvent = serde_json::from_slice(include_bytes!(
        "../docs/contracts/examples/provider-event-v1.example.json"
    ))
    .expect("provider event example should parse");
    event
        .validate()
        .expect("provider event example should validate");
    assert_eq!(event.provider_lock_sha256, provider_lock.lock_sha256);

    let report = ProviderExecutionReport::from_json_slice(include_bytes!(
        "../docs/contracts/examples/provider-execution-report-v1.example.json"
    ))
    .expect("provider execution report example should validate");
    assert_eq!(report.payload.provider_lock, provider_lock);
}

#[test]
fn published_runtime_schema_enums_match_the_public_rust_contract() {
    let lock_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-lock-v1.schema.json"
    ))
    .expect("provider lock schema should be valid JSON");
    let event_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-event-v1.schema.json"
    ))
    .expect("provider event schema should be valid JSON");
    let report_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/provider-execution-report-v1.schema.json"
    ))
    .expect("provider report schema should be valid JSON");

    assert_eq!(
        lock_schema["$defs"]["selection"]["properties"]["source"]["enum"],
        enum_array([
            ProviderSelectionSource::Replacement,
            ProviderSelectionSource::Primary,
            ProviderSelectionSource::Fallback,
        ])
    );
    assert_eq!(
        lock_schema["$defs"]["sideEffect"]["enum"],
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
        event_schema["properties"]["kind"]["enum"],
        enum_array([
            ProviderEventKind::ExecutionStarted,
            ProviderEventKind::CancellationObserved,
            ProviderEventKind::TimeoutExceeded,
            ProviderEventKind::CaptureLimitExceeded,
            ProviderEventKind::ArtifactLimitExceeded,
            ProviderEventKind::ProcessExited,
            ProviderEventKind::OutputValidationStarted,
            ProviderEventKind::OutputValidated,
            ProviderEventKind::ExecutionSucceeded,
            ProviderEventKind::ExecutionFailed,
        ])
    );
    assert_eq!(
        report_schema["$defs"]["payload"]["properties"]["outcome"]["enum"],
        enum_array([
            ProviderExecutionOutcome::Succeeded,
            ProviderExecutionOutcome::Failed,
            ProviderExecutionOutcome::Cancelled,
            ProviderExecutionOutcome::TimedOut,
            ProviderExecutionOutcome::CaptureLimitExceeded,
            ProviderExecutionOutcome::ArtifactLimitExceeded,
            ProviderExecutionOutcome::InvalidOutput,
        ])
    );
    assert_eq!(
        report_schema["$defs"]["termination"]["properties"]["reason"]["enum"],
        enum_array([
            TerminationReason::NaturalExit,
            TerminationReason::PreflightRejected,
            TerminationReason::SpawnFailed,
            TerminationReason::Cancelled,
            TerminationReason::TimedOut,
            TerminationReason::CaptureLimitExceeded,
            TerminationReason::ArtifactLimitExceeded,
        ])
    );
    assert_eq!(
        report_schema["$defs"]["failure"]["properties"]["code"]["enum"],
        enum_array([
            ProviderExecutionFailureCode::ImplementationChanged,
            ProviderExecutionFailureCode::SpawnFailed,
            ProviderExecutionFailureCode::ExitFailure,
            ProviderExecutionFailureCode::Cancelled,
            ProviderExecutionFailureCode::Timeout,
            ProviderExecutionFailureCode::StdoutLimit,
            ProviderExecutionFailureCode::StderrLimit,
            ProviderExecutionFailureCode::ArtifactFileLimit,
            ProviderExecutionFailureCode::ArtifactByteLimit,
            ProviderExecutionFailureCode::CaptureReadFailed,
            ProviderExecutionFailureCode::MissingOutput,
            ProviderExecutionFailureCode::UnexpectedOutput,
            ProviderExecutionFailureCode::InvalidOutputType,
            ProviderExecutionFailureCode::EmptyOutput,
            ProviderExecutionFailureCode::SymlinkOutput,
            ProviderExecutionFailureCode::OutputReadFailed,
        ])
    );
    assert_eq!(
        report_schema["$defs"]["artifact"]["properties"]["kind"]["enum"],
        enum_array([ArtifactKind::File, ArtifactKind::Directory])
    );
    assert_eq!(
        report_schema["$defs"]["diagnostic"]["properties"]["stream"]["enum"],
        enum_array([StreamKind::Stdout, StreamKind::Stderr])
    );
}

#[test]
fn runtime_contract_parsers_reject_unknown_versions_and_tampering() {
    let mut lock: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/provider-lock-v1.example.json"
    ))
    .expect("provider lock fixture should parse");
    lock["schema"] = Value::String("aniflow.provider-lock/v2".to_owned());
    assert!(ProviderLock::from_json_slice(&serde_json::to_vec(&lock).unwrap()).is_err());

    let mut event: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/provider-event-v1.example.json"
    ))
    .expect("provider event fixture should parse");
    event["schema"] = Value::String("aniflow.provider-event/v2".to_owned());
    let event: ProviderEvent =
        serde_json::from_slice(&serde_json::to_vec(&event).unwrap()).unwrap();
    assert!(event.validate().is_err());

    let mut report: Value = serde_json::from_str(include_str!(
        "../docs/contracts/examples/provider-execution-report-v1.example.json"
    ))
    .expect("provider report fixture should parse");
    report["schema"] = Value::String("aniflow.provider-execution-report/v2".to_owned());
    assert!(
        ProviderExecutionReport::from_json_slice(&serde_json::to_vec(&report).unwrap()).is_err()
    );
}
