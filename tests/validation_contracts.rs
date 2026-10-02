//! Exact, offline validation decisions; authored coverage, not passing evidence.
use aniflow::{ArtifactEvidence,ArtifactKind,ArtifactRole,StreamRole};
use aniflow::temporal::{self,StreamSelection,TemporalInspection};
use aniflow::validation::*;
use serde_json::Value;

fn inspection(name: &str) -> TemporalInspection {
    let value: Value = serde_json::from_slice(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal").join(format!("{name}.json"))).unwrap()).unwrap();
    let observations = value["observations"].as_array().unwrap().iter().map(|item| (item["stream_index"].as_u64().unwrap() as u32,item["frames"].clone(),item["packets"].clone())).collect::<Vec<_>>();
    temporal::from_probe_documents(value["source_sha256"].as_str().unwrap(),value["source_size_bytes"].as_u64().unwrap(),&serde_json::from_value(value["selection"].clone()).unwrap(),&value["inventory"],&observations).unwrap()
}

fn artifact(id: &str, source: &TemporalInspection) -> ArtifactEvidence {
    ArtifactEvidence { id:id.to_owned(),port:"video".to_owned(),relative_path:if id=="source" {None} else {Some("artifacts/master.mp4".to_owned())},
        artifact_type:"video/mp4".to_owned(),artifact_role:if id=="source" {ArtifactRole::TemporalSource} else {ArtifactRole::CandidateMaster},stream_role:Some(StreamRole::Video),
        kind:ArtifactKind::File,file_count:1,byte_count:source.source_size_bytes,sha256:source.source_sha256.clone() }
}

fn pair() -> (ValidationContext,ValidatorObservation) {
    let source = inspection("cfr-24000-1001");
    let mut output = source.clone(); output.source_sha256="b".repeat(64);
    let context = ValidationContext { schema:VALIDATION_CONTEXT_SCHEMA_V1.to_owned(),validation_id:"media".to_owned(),contract:TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1.to_owned(),
        plan_sha256:"c".repeat(64),producer_invocation_sha256:"d".repeat(64),producer_lock_sha256:"e".repeat(64),validator_lock_sha256:"f".repeat(64),
        artifact:artifact("master",&output),inputs:vec![artifact("source",&source)],
        temporal:Some(TemporalValidationRequirement { source_artifact:"source".to_owned(),source_selection:source.selection_intent.clone(),artifact_selection:output.selection_intent.clone(),allow_authored_subtitles:false }) };
    let observation = ValidatorObservation { schema:VALIDATOR_OBSERVATION_SCHEMA_V1.to_owned(),context_sha256:context.sha256().unwrap(),artifact_sha256:output.source_sha256.clone(),disposition:ValidationDisposition::Passed,
        checks:[ValidationCriterion::ArtifactIdentity,ValidationCriterion::SourceLineage,ValidationCriterion::Provenance,ValidationCriterion::Decodability].into_iter().map(|criterion| ValidationCheck {criterion,disposition:ValidationDisposition::Passed}).collect(),
        temporal:Some(TemporalValidationObservation { source,artifact:output }) };
    (context,observation)
}

#[test]
fn exact_fractional_cfr_media_evidence_is_accepted() {
    let (context,observation)=pair();observation.accept(&context).unwrap();
    assert_eq!(observation.temporal.unwrap().source.video_timeline().unwrap().frame_period.unwrap().denominator(),24000);
}

#[test]
fn each_incomplete_disposition_fails_closed() {
    for disposition in [ValidationDisposition::Failed,ValidationDisposition::Partial,ValidationDisposition::Skipped,ValidationDisposition::Unavailable] {
        let (context,mut observation)=pair();observation.disposition=disposition;
        let error=observation.accept(&context).unwrap_err();
        assert_eq!(error.validation_diagnostic().unwrap().code,ValidationFailureCode::RejectedObservation);
    }
}

#[test]
fn required_checks_cannot_be_omitted_duplicated_or_contradicted() {
    let (context,observation)=pair();
    let mut missing=observation.clone();missing.checks.pop();missing.accept(&context).unwrap_err();
    let mut duplicate=observation.clone();let check=duplicate.checks[0].clone();duplicate.checks.push(check);
    assert_eq!(duplicate.accept(&context).unwrap_err().validation_diagnostic().unwrap().code,ValidationFailureCode::DuplicateEvidence);
    let mut contradictory=observation;contradictory.checks[0].disposition=ValidationDisposition::Skipped;
    assert_eq!(contradictory.accept(&context).unwrap_err().validation_diagnostic().unwrap().code,ValidationFailureCode::ContradictoryEvidence);
}

#[test]
fn source_and_validator_identity_changes_invalidate_observations() {
    let (context,observation)=pair();
    let mut changed=context.clone();changed.validator_lock_sha256="0".repeat(64);observation.accept(&changed).unwrap_err();
    let mut changed=context.clone();changed.inputs[0].sha256="0".repeat(64);observation.accept(&changed).unwrap_err();
    let mut changed=context;changed.producer_invocation_sha256="0".repeat(64);observation.accept(&changed).unwrap_err();
}

#[test]
fn clock_claims_are_recomputed_instead_of_trusting_provider_support() {
    let (context,mut observation)=pair();
    observation.temporal.as_mut().unwrap().artifact.timelines[0].frames[0].presentation_ticks+=1;
    observation.accept(&context).unwrap_err();
    let (context,mut observation)=pair();
    observation.temporal.as_mut().unwrap().artifact.processing.frame_count=Some(1);
    assert_eq!(observation.accept(&context).unwrap_err().validation_diagnostic().unwrap().code,ValidationFailureCode::ContradictoryEvidence);
}

#[test]
fn stream_selection_and_inspection_byte_identity_are_bound_to_policy() {
    let (context,mut observation)=pair();
    observation.temporal.as_mut().unwrap().artifact.source_sha256="0".repeat(64);
    observation.accept(&context).unwrap_err();
    let (context,mut observation)=pair();
    observation.temporal.as_mut().unwrap().artifact.selection_intent=StreamSelection { video_stream:Some(99),..StreamSelection::default() };
    observation.accept(&context).unwrap_err();
}

#[test]
fn variable_rate_and_nonzero_origins_cannot_claim_supported_delivery() {
    for name in ["vfr-visible-refusal","positive-origin","negative-origin"] {
        let (mut context,mut observation)=pair();let output=inspection(name);
        context.artifact=artifact("master",&output);
        context.temporal.as_mut().unwrap().artifact_selection=output.selection_intent.clone();
        observation.context_sha256=context.sha256().unwrap();observation.artifact_sha256=output.source_sha256.clone();
        observation.temporal.as_mut().unwrap().artifact=output;
        observation.accept(&context).expect_err(name);
    }
}

#[test]
fn duplicate_temporal_streams_and_arbitrary_extra_checks_are_refused() {
    let (context,mut observation)=pair();let media=observation.temporal.as_mut().unwrap();
    let stream=media.artifact.streams[0].clone();media.artifact.streams.push(stream);observation.accept(&context).unwrap_err();
    let (_,observation)=pair();let mut value=serde_json::to_value(observation).unwrap();value["accepted"]=Value::Bool(true);
    serde_json::from_value::<ValidatorObservation>(value).unwrap_err();
}

#[test]
fn public_acceptance_layers_require_their_own_authority_shape() {
    let examples=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/contracts/examples");
    let mut candidate: AcceptanceRecord=serde_json::from_slice(&std::fs::read(examples.join("acceptance-candidate-master-v1.example.json")).unwrap()).unwrap();
    candidate.validate().unwrap();
    candidate.producer_invocation_sha256=None;candidate.producer_lock_sha256=None;
    candidate.validate().unwrap_err();
    let mut delivery: AcceptanceRecord=serde_json::from_slice(&std::fs::read(examples.join("acceptance-record-v1.example.json")).unwrap()).unwrap();
    delivery.validate().unwrap();
    delivery.producer_invocation_sha256=Some("a".repeat(64));delivery.producer_lock_sha256=Some("b".repeat(64));
    delivery.validate().unwrap_err();
}

#[test]
fn native_registration_pins_bytes_and_normalizes_only_component_versions() {
    use aniflow::validation::native::{self,NativeValidationConfiguration,ValidationToolPin};
    use sha2::{Digest,Sha256};
    let root=tempfile::tempdir().unwrap();
    let path=root.path().join("synthetic-tool");
    let bytes=b"synthetic tool bytes; must never execute in this test";
    std::fs::write(&path,bytes).unwrap();
    let pin=ValidationToolPin { path:path.clone(),version:"8.0".to_owned(),sha256:format!("{:x}",Sha256::digest(bytes)) };
    let settings=NativeValidationConfiguration { ffmpeg:pin.clone(),ffprobe:pin,maximum_media_bytes:1024,decode_timeout_seconds:1 };
    let registration=native::registration("native-fixture",&path,settings.clone(),"video/mp4","video/mp4").unwrap();
    assert_eq!(registration.capability().kind,aniflow::CapabilityKind::TemporalValidator);
    assert!(registration.components().tools.iter().all(|tool| tool.version=="8.0.0"));
    assert_eq!(registration.configuration().values["ffmpeg"]["version"],"8.0");
    std::fs::write(&path,b"changed bytes").unwrap();
    native::registration("native-fixture",&path,settings,"video/mp4","video/mp4").unwrap_err();
}

#[test]
fn native_configuration_refuses_unbounded_or_implicit_tools() {
    use aniflow::validation::native::{NativeValidationConfiguration,ValidationToolPin};
    let pin=ValidationToolPin { path:std::env::temp_dir().join("pinned-tool"),version:"8.0".to_owned(),sha256:"a".repeat(64) };
    let settings=NativeValidationConfiguration { ffmpeg:pin.clone(),ffprobe:pin,maximum_media_bytes:1024,decode_timeout_seconds:1 };
    settings.validate().unwrap();
    let mut invalid=settings.clone();invalid.maximum_media_bytes=0;invalid.validate().unwrap_err();
    let mut invalid=settings.clone();invalid.decode_timeout_seconds=3601;invalid.validate().unwrap_err();
    let mut invalid=settings;invalid.ffmpeg.path="ffmpeg".into();invalid.validate().unwrap_err();
}
