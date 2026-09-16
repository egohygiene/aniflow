use std::fs;
use std::path::{Path, PathBuf};

use aniflow::{
    ArtifactEvidence, ArtifactKind, ArtifactRole, CANONICAL_JSON_SCHEMA_V1, CompatibilityDecision,
    CompatibilityReasonCode, EvidenceReference, PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1,
    PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1, PipelineV3RunManifest, PipelineV3RunState,
    PipelineV3StageState, PipelineV3Workspace, StageCheckpoint, StageCheckpointPayload,
    StageRunRecord, StreamRole, ValidationEvidence, append_run_manifest, load_latest_run_manifest,
    load_run_manifest_chain, load_stage_checkpoint, publish_stage_checkpoint,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use tempfile::tempdir;

const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const CHECKPOINT_DIGEST: &str = "7feed54b0b034636d8efd28c3d15fc345c66d89eefc4983ad47ca3d7410e0540";
const MANIFEST_DIGEST: &str = "764992c750c92f5ff42aba1332e5cde864a5462cf90e1c7e8f162b839b25186a";

const CHECKPOINT_EXAMPLE: &[u8] =
    include_bytes!("../docs/contracts/examples/stage-checkpoint-v1.example.json");
const MANIFEST_EXAMPLE: &[u8] =
    include_bytes!("../docs/contracts/examples/pipeline-run-v1.example.json");

#[test]
fn published_state_examples_are_canonical_and_self_validating() {
    let checkpoint = StageCheckpoint::from_json_slice(CHECKPOINT_EXAMPLE)
        .expect("checkpoint example should be normalized and self-validating");
    assert_eq!(checkpoint.schema, PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1);
    assert_eq!(checkpoint.canonicalization, CANONICAL_JSON_SCHEMA_V1);
    assert_eq!(checkpoint.checkpoint_sha256, CHECKPOINT_DIGEST);
    assert_eq!(
        checkpoint
            .reference()
            .expect("published checkpoint should have a valid reference")
            .relative_path,
        format!("state/checkpoints/enhance-{CHECKPOINT_DIGEST}.json")
    );

    let manifest = PipelineV3RunManifest::from_json_slice(MANIFEST_EXAMPLE)
        .expect("manifest example should be normalized and self-validating");
    assert_eq!(manifest.schema, PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1);
    assert_eq!(manifest.canonicalization, CANONICAL_JSON_SCHEMA_V1);
    assert_eq!(manifest.manifest_sha256, MANIFEST_DIGEST);
    assert_eq!(
        manifest.payload.stages[0]
            .checkpoint
            .as_ref()
            .expect("complete example stage should reference a checkpoint")
            .checkpoint_sha256,
        checkpoint.checkpoint_sha256
    );

    let checkpoint_value: Value =
        serde_json::from_slice(CHECKPOINT_EXAMPLE).expect("checkpoint should be JSON");
    let manifest_value: Value =
        serde_json::from_slice(MANIFEST_EXAMPLE).expect("manifest should be JSON");
    assert_eq!(
        serde_json::to_value(checkpoint).expect("checkpoint should serialize"),
        checkpoint_value
    );
    assert_eq!(
        serde_json::to_value(manifest).expect("manifest should serialize"),
        manifest_value
    );
}

#[test]
fn state_contracts_reject_tampering_unknown_versions_and_unknown_fields() {
    let checkpoint: Value =
        serde_json::from_slice(CHECKPOINT_EXAMPLE).expect("checkpoint should be JSON");
    let manifest: Value =
        serde_json::from_slice(MANIFEST_EXAMPLE).expect("manifest should be JSON");

    let mut tampered_checkpoint = checkpoint.clone();
    tampered_checkpoint["payload"]["outputs"][0]["byte_count"] = Value::from(29_u64);
    StageCheckpoint::from_json_slice(
        &serde_json::to_vec(&tampered_checkpoint).expect("tampered checkpoint should encode"),
    )
    .expect_err("checkpoint payload tampering must invalidate the digest");

    let mut unknown_checkpoint = checkpoint.clone();
    unknown_checkpoint["schema"] = Value::String("aniflow.stage-checkpoint/v2".to_owned());
    StageCheckpoint::from_json_slice(
        &serde_json::to_vec(&unknown_checkpoint).expect("unknown checkpoint should encode"),
    )
    .expect_err("unknown checkpoint schema versions must fail closed");

    let mut checkpoint_with_unknown_field = checkpoint;
    checkpoint_with_unknown_field["payload"]["execution_report"]["trusted"] = Value::Bool(true);
    StageCheckpoint::from_json_slice(
        &serde_json::to_vec(&checkpoint_with_unknown_field)
            .expect("checkpoint with unknown field should encode"),
    )
    .expect_err("unknown nested checkpoint fields must be rejected");

    let mut tampered_manifest = manifest.clone();
    tampered_manifest["payload"]["updated_at"] = Value::String("2026-09-15T00:00:02Z".to_owned());
    PipelineV3RunManifest::from_json_slice(
        &serde_json::to_vec(&tampered_manifest).expect("tampered manifest should encode"),
    )
    .expect_err("manifest payload tampering must invalidate the digest");

    let mut unknown_manifest = manifest.clone();
    unknown_manifest["schema"] = Value::String("aniflow.pipeline-run/v2".to_owned());
    PipelineV3RunManifest::from_json_slice(
        &serde_json::to_vec(&unknown_manifest).expect("unknown manifest should encode"),
    )
    .expect_err("unknown manifest schema versions must fail closed");

    let mut manifest_with_unknown_field = manifest;
    manifest_with_unknown_field["payload"]["stages"][0]["unsafe_retry"] = Value::Bool(true);
    PipelineV3RunManifest::from_json_slice(
        &serde_json::to_vec(&manifest_with_unknown_field)
            .expect("manifest with unknown field should encode"),
    )
    .expect_err("unknown nested manifest fields must be rejected");

    let mut explicit_null: Value =
        serde_json::from_slice(MANIFEST_EXAMPLE).expect("manifest should be JSON");
    explicit_null["payload"]["diagnostic"] = Value::Null;
    PipelineV3RunManifest::from_json_slice(
        &serde_json::to_vec(&explicit_null).expect("manifest null fixture should encode"),
    )
    .expect_err("explicit null must not alias an omitted manifest field");

    let mut explicit_empty_default: Value =
        serde_json::from_slice(CHECKPOINT_EXAMPLE).expect("checkpoint should be JSON");
    explicit_empty_default["payload"]["dependencies"] = serde_json::json!([]);
    StageCheckpoint::from_json_slice(
        &serde_json::to_vec(&explicit_empty_default)
            .expect("checkpoint empty-array fixture should encode"),
    )
    .expect_err("explicit empty default arrays must not alias omitted checkpoint fields");
}

#[test]
fn published_schemas_track_the_closed_state_models_and_enums() {
    let run_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/pipeline-run-v1.schema.json"
    ))
    .expect("run-manifest schema should be JSON");
    let checkpoint_schema: Value = serde_json::from_str(include_str!(
        "../docs/contracts/stage-checkpoint-v1.schema.json"
    ))
    .expect("checkpoint schema should be JSON");

    assert_eq!(
        run_schema["properties"]["schema"]["const"],
        PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1
    );
    assert_eq!(
        checkpoint_schema["properties"]["schema"]["const"],
        PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1
    );
    assert_eq!(run_schema["additionalProperties"], false);
    assert_eq!(checkpoint_schema["additionalProperties"], false);
    assert_eq!(
        run_schema["properties"]["canonicalization"]["const"],
        CANONICAL_JSON_SCHEMA_V1
    );
    assert_eq!(
        checkpoint_schema["properties"]["canonicalization"]["const"],
        CANONICAL_JSON_SCHEMA_V1
    );
    assert_eq!(
        run_schema["$defs"]["runManifestPayload"]["properties"]["state"]["enum"],
        serde_json::to_value([
            PipelineV3RunState::Running,
            PipelineV3RunState::Interrupted,
            PipelineV3RunState::Failed,
            PipelineV3RunState::Cancelled,
            PipelineV3RunState::Complete,
        ])
        .expect("run states should serialize")
    );
    assert_eq!(
        run_schema["$defs"]["stageRunRecord"]["properties"]["state"]["enum"],
        serde_json::to_value([
            PipelineV3StageState::Pending,
            PipelineV3StageState::Running,
            PipelineV3StageState::Validating,
            PipelineV3StageState::Complete,
            PipelineV3StageState::Failed,
            PipelineV3StageState::Cancelled,
            PipelineV3StageState::Invalidated,
            PipelineV3StageState::Skipped,
        ])
        .expect("stage states should serialize")
    );
    assert_eq!(
        run_schema["$defs"]["compatibilityReason"]["properties"]["code"]["enum"],
        serde_json::to_value([
            CompatibilityReasonCode::MissingCheckpoint,
            CompatibilityReasonCode::UnsupportedCheckpointSchema,
            CompatibilityReasonCode::CorruptCheckpoint,
            CompatibilityReasonCode::StagePlanChanged,
            CompatibilityReasonCode::ProviderLockChanged,
            CompatibilityReasonCode::ExecutionPolicyChanged,
            CompatibilityReasonCode::InputChanged,
            CompatibilityReasonCode::DependencyChanged,
            CompatibilityReasonCode::ExecutionReportMissing,
            CompatibilityReasonCode::ExecutionReportInvalid,
            CompatibilityReasonCode::OutputMissing,
            CompatibilityReasonCode::OutputChanged,
            CompatibilityReasonCode::ValidationMissing,
            CompatibilityReasonCode::ValidationFailed,
            CompatibilityReasonCode::ValidationChanged,
        ])
        .expect("compatibility reasons should serialize")
    );

    for definition in [
        "evidenceReference",
        "stageCheckpointReference",
        "artifactEvidence",
        "compatibilityReason",
        "compatibilityDecision",
        "stageRunRecord",
        "runManifestPayload",
    ] {
        assert_eq!(
            run_schema["$defs"][definition]["additionalProperties"], false,
            "{definition} must reject unknown properties"
        );
    }
    for definition in [
        "evidenceReference",
        "stageCheckpointReference",
        "artifactEvidence",
        "validationEvidence",
        "checkpointPayload",
    ] {
        assert_eq!(
            checkpoint_schema["$defs"][definition]["additionalProperties"], false,
            "{definition} must reject unknown properties"
        );
    }
}

#[test]
fn manifest_chain_and_checkpoint_publication_are_immutable_and_read_only() {
    let temporary = tempdir().expect("temporary directory should be created");
    let run_directory = temporary.path().join("run");
    let workspace = PipelineV3Workspace::create_at(&run_directory)
        .expect("Pipeline v3 workspace should be created");

    let first = PipelineV3RunManifest::new(
        workspace.run_id(),
        DIGEST_A,
        vec![StageRunRecord {
            stage_id: "enhance".to_owned(),
            state: PipelineV3StageState::Pending,
            checkpoint: None,
            compatibility: None,
            message: None,
        }],
    )
    .expect("initial manifest should be valid");
    append_run_manifest(&workspace, &first).expect("initial manifest should publish");

    let output = artifact_evidence(
        "enhanced",
        "processed_frames",
        Some("artifacts/enhance/frames"),
        2,
        28,
        DIGEST_C,
    );
    let checkpoint = StageCheckpoint::new(StageCheckpointPayload {
        stage_id: "enhance".to_owned(),
        plan_sha256: DIGEST_A.to_owned(),
        stage_invocation_sha256: DIGEST_B.to_owned(),
        provider_lock_sha256: DIGEST_C.to_owned(),
        dependencies: Vec::new(),
        inputs: vec![artifact_evidence("source", "frames", None, 0, 0, DIGEST_A)],
        outputs: vec![output.clone()],
        validations: vec![ValidationEvidence {
            id: "enhanced_integrity".to_owned(),
            contract: "aniflow.validation/artifact-integrity/v1".to_owned(),
            artifact_id: output.id.clone(),
            artifact_sha256: output.sha256.clone(),
            accepted: true,
            evidence: EvidenceReference {
                relative_path: "providers/enhance/validation-enhanced.json".to_owned(),
                sha256: DIGEST_B.to_owned(),
            },
        }],
        execution_report: EvidenceReference {
            relative_path: "providers/enhance/report.json".to_owned(),
            sha256: DIGEST_C.to_owned(),
        },
        compatibility_fingerprint: None,
        completed_at: timestamp("2026-09-15T00:00:01Z"),
    })
    .expect("checkpoint should be valid");
    let reference =
        publish_stage_checkpoint(&workspace, &checkpoint).expect("checkpoint should publish once");

    let complete_stage = StageRunRecord {
        stage_id: "enhance".to_owned(),
        state: PipelineV3StageState::Complete,
        checkpoint: Some(reference.clone()),
        compatibility: None,
        message: None,
    };
    let running_stage = StageRunRecord {
        stage_id: "enhance".to_owned(),
        state: PipelineV3StageState::Running,
        checkpoint: None,
        compatibility: None,
        message: None,
    };
    let second = first
        .next_revision(
            PipelineV3RunState::Running,
            vec![running_stage],
            Vec::new(),
            None,
        )
        .expect("running successor should be valid");
    append_run_manifest(&workspace, &second).expect("second manifest should publish");
    let validating_stage = StageRunRecord {
        stage_id: "enhance".to_owned(),
        state: PipelineV3StageState::Validating,
        checkpoint: None,
        compatibility: None,
        message: None,
    };
    let third = second
        .next_revision(
            PipelineV3RunState::Running,
            vec![validating_stage],
            Vec::new(),
            None,
        )
        .expect("validating successor should be valid");
    append_run_manifest(&workspace, &third).expect("third manifest should publish");
    let fourth = third
        .next_revision(
            PipelineV3RunState::Running,
            vec![complete_stage.clone()],
            Vec::new(),
            None,
        )
        .expect("completed stage successor should be valid");
    append_run_manifest(&workspace, &fourth).expect("fourth manifest should publish");
    let fifth = fourth
        .next_revision(
            PipelineV3RunState::Complete,
            vec![complete_stage.clone()],
            vec![output.clone()],
            None,
        )
        .expect("complete successor should be valid");
    append_run_manifest(&workspace, &fifth).expect("complete manifest should publish");
    let reused_stage = StageRunRecord {
        compatibility: Some(CompatibilityDecision::compatible()),
        message: Some("reused compatible stage checkpoint".to_owned()),
        ..complete_stage
    };
    let sixth = fifth
        .next_revision(
            PipelineV3RunState::Running,
            vec![reused_stage.clone()],
            Vec::new(),
            None,
        )
        .expect("compatible resume successor should be valid");
    append_run_manifest(&workspace, &sixth).expect("resume manifest should publish");
    let seventh = sixth
        .next_revision(
            PipelineV3RunState::Complete,
            vec![reused_stage],
            vec![output],
            None,
        )
        .expect("resumed completion should be valid");
    append_run_manifest(&workspace, &seventh).expect("resumed completion should publish");
    let illegal_eighth = seventh
        .next_revision(
            PipelineV3RunState::Running,
            vec![StageRunRecord {
                stage_id: "enhance".to_owned(),
                state: PipelineV3StageState::Running,
                checkpoint: None,
                compatibility: None,
                message: None,
            }],
            Vec::new(),
            None,
        )
        .expect("the isolated manifest remains structurally valid");
    append_run_manifest(&workspace, &illegal_eighth)
        .expect_err("a complete stage must be invalidated before it can run again");

    let before = snapshot(&run_directory);
    publish_stage_checkpoint(&workspace, &checkpoint)
        .expect_err("checkpoint publication must never replace immutable evidence");
    append_run_manifest(&workspace, &seventh)
        .expect_err("manifest publication must never replace a prior revision");

    let reopened = PipelineV3Workspace::open_read_only(&run_directory)
        .expect("complete workspace should open read-only");
    assert_eq!(
        load_run_manifest_chain(&reopened).expect("manifest chain should validate"),
        vec![first, second, third, fourth, fifth, sixth, seventh.clone(),]
    );
    assert_eq!(
        load_latest_run_manifest(&reopened).expect("latest manifest should load"),
        seventh
    );
    assert_eq!(
        load_stage_checkpoint(&reopened, &reference).expect("checkpoint should load"),
        checkpoint
    );
    assert_eq!(snapshot(&run_directory), before);
}

#[test]
fn manifest_chain_rejects_arbitrary_stage_lifecycle_jumps() {
    let temporary = tempdir().expect("temporary directory should be created");
    let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
        .expect("Pipeline v3 workspace should be created");
    let first = PipelineV3RunManifest::new(
        workspace.run_id(),
        DIGEST_A,
        vec![StageRunRecord {
            stage_id: "enhance".to_owned(),
            state: PipelineV3StageState::Pending,
            checkpoint: None,
            compatibility: None,
            message: None,
        }],
    )
    .expect("initial manifest should be valid");
    append_run_manifest(&workspace, &first).expect("initial manifest should publish");

    let reference = StageCheckpoint::new(StageCheckpointPayload {
        stage_id: "enhance".to_owned(),
        plan_sha256: DIGEST_A.to_owned(),
        stage_invocation_sha256: DIGEST_B.to_owned(),
        provider_lock_sha256: DIGEST_C.to_owned(),
        dependencies: Vec::new(),
        inputs: vec![artifact_evidence("source", "frames", None, 0, 0, DIGEST_A)],
        outputs: vec![artifact_evidence(
            "enhanced",
            "processed_frames",
            Some("artifacts/enhance/frames"),
            1,
            1,
            DIGEST_B,
        )],
        validations: Vec::new(),
        execution_report: EvidenceReference {
            relative_path: "providers/enhance/report.json".to_owned(),
            sha256: DIGEST_C.to_owned(),
        },
        compatibility_fingerprint: None,
        completed_at: timestamp("2026-09-15T00:00:01Z"),
    })
    .expect("checkpoint reference fixture should be valid")
    .reference()
    .expect("checkpoint reference should be valid");
    let jumped = first
        .next_revision(
            PipelineV3RunState::Running,
            vec![StageRunRecord {
                stage_id: "enhance".to_owned(),
                state: PipelineV3StageState::Complete,
                checkpoint: Some(reference),
                compatibility: None,
                message: None,
            }],
            Vec::new(),
            None,
        )
        .expect("the isolated manifest remains structurally valid");
    append_run_manifest(&workspace, &jumped)
        .expect_err("pending stages cannot jump directly to complete");

    assert_eq!(
        load_run_manifest_chain(&workspace).expect("rejected transition must not mutate state"),
        vec![first]
    );
}

fn artifact_evidence(
    id: &str,
    port: &str,
    relative_path: Option<&str>,
    file_count: u64,
    byte_count: u64,
    sha256: &str,
) -> ArtifactEvidence {
    ArtifactEvidence {
        id: id.to_owned(),
        port: port.to_owned(),
        relative_path: relative_path.map(str::to_owned),
        artifact_type: "application/vnd.aniflow.frame-set+directory".to_owned(),
        artifact_role: if relative_path.is_some() {
            ArtifactRole::TemporalComponent
        } else {
            ArtifactRole::TemporalSource
        },
        stream_role: Some(StreamRole::Video),
        kind: ArtifactKind::Directory,
        file_count,
        byte_count,
        sha256: sha256.to_owned(),
    }
}

fn timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp should be RFC 3339")
        .with_timezone(&Utc)
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    fn visit(root: &Path, current: &Path, entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let mut children = fs::read_dir(current)
            .expect("snapshot directory should be readable")
            .map(|entry| entry.expect("snapshot entry should be readable").path())
            .collect::<Vec<_>>();
        children.sort();
        for path in children {
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path should remain under root")
                .to_path_buf();
            if path.is_dir() {
                entries.push((relative, None));
                visit(root, &path, entries);
            } else {
                entries.push((
                    relative,
                    Some(fs::read(&path).expect("snapshot file should be readable")),
                ));
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}
