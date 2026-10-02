//! Synthetic coverage authored for #33. Execution is deferred under #64.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use aniflow::*;
use aniflow::validation::*;

struct Fixture { root: tempfile::TempDir }
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/validation-v1");
        for entry in fs::read_dir(kit).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() { fs::copy(entry.path(), root.path().join(entry.file_name())).unwrap(); }
        }
        fs::set_permissions(root.path().join("provider.py"), fs::Permissions::from_mode(0o755)).unwrap();
        Self { root }
    }
    fn bindings(&self) -> Vec<PipelineInputBinding> { vec![PipelineInputBinding::new("source",self.root.path().join("input.bin"))] }
    fn registry(&self, mode: &str) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        for name in ["copy",mode] {
            let registration = ProviderRegistrationDocument::load(self.root.path().join(format!("{name}.registration.json"))).unwrap().into_registration().unwrap();
            registry.register(registration).unwrap();
        }
        registry
    }
    fn configuration(&self) -> PipelineV3Configuration { PipelineV3Configuration::load(self.root.path().join("pipeline.yml")).unwrap() }
    fn context(&self) -> PipelinePlanningContext {
        PipelinePlanningContext { host:HostResources { cpu_threads:2,memory_mib:1024,storage_mib:1024,gpu_available:false,network_available:false },
            allowed_side_effects:vec![SideEffect::FilesystemRead,SideEffect::FilesystemWrite],offline:true }
    }
    fn plan(&self, mode: &str) -> PipelineV3Plan { resolve_pipeline_v3(&self.configuration(),&self.bindings(),&self.registry(mode),&self.context()).unwrap() }
    fn request(&self, mode: &str) -> PipelineV3RunRequest {
        PipelineV3RunRequest::new(self.plan(mode),self.bindings(),self.registry(mode)).with_output_directory(self.root.path().join("runs"))
    }
    fn run(&self) -> PipelineV3RunOutcome { run_v3(self.request("passed")).unwrap() }
    fn count(&self) -> u64 { fs::read_to_string(self.root.path().join("validator-launch-count")).unwrap().parse().unwrap() }
    fn run_directory(&self) -> PathBuf { fs::read_dir(self.root.path().join("runs")).unwrap().next().unwrap().unwrap().path() }
    fn resume(&self, run: &Path) -> aniflow::Result<PipelineV3RunOutcome> { resume_v3(PipelineV3ResumeRequest::new(run,self.bindings(),self.registry("passed"))) }
}

fn record(root: &Path, reference: &EvidenceReference) -> AcceptanceRecord {
    let result: AcceptanceRecord = serde_json::from_slice(&fs::read(root.join(&reference.relative_path)).unwrap()).unwrap();
    result.validate().unwrap(); assert_eq!(result.sha256().unwrap(),reference.sha256); result
}

#[test]
fn all_four_acceptance_layers_bind_the_same_delivery_chain() {
    let fixture = Fixture::new(); let outcome = fixture.run();
    let manifest = status_v3(&outcome.run_directory).unwrap();
    assert_eq!(manifest.payload.delivery,outcome.delivery);
    let delivery = record(&outcome.run_directory,outcome.delivery.as_ref().unwrap());
    assert_eq!(delivery.layer,ValidationLayer::Delivery);
    let candidate = record(&outcome.run_directory,&delivery.children[0]);
    assert_eq!(candidate.layer,ValidationLayer::CandidateMaster);
    let stage = record(&outcome.run_directory,&candidate.children[0]);
    assert_eq!(stage.layer,ValidationLayer::Stage);
    let component = record(&outcome.run_directory,&stage.children[0]);
    assert_eq!(component.layer,ValidationLayer::Component);
    assert_eq!(component.artifacts[0].sha256,outcome.outputs[0].sha256);
    assert_eq!(component.validations.len(),2);
    assert_eq!(component.inputs[0].id,"source");
    assert_eq!(fixture.count(),1);
}

#[test]
fn compatible_resume_reuses_validators_without_relaunch() {
    let fixture = Fixture::new(); let outcome = fixture.run();
    let resumed = fixture.resume(&outcome.run_directory).unwrap();
    assert!(resumed.executed_stages.is_empty());
    assert_eq!(resumed.reused_stages,vec!["copy"]);
    assert_eq!(resumed.delivery,outcome.delivery);
    assert_eq!(fixture.count(),1);
}

#[test]
fn exit_zero_does_not_accept_partial_or_invalid_validator_evidence() {
    for mode in ["partial","failed","skipped","unavailable","duplicate","stale-context","wrong-artifact","contradictory","missing","malformed","nonzero","mutate-artifact"] {
        let fixture = Fixture::new();
        run_v3(fixture.request(mode)).expect_err(mode);
        let manifest = status_v3(fixture.run_directory()).unwrap();
        assert_eq!(manifest.payload.state,PipelineV3RunState::Failed,"{mode}");
        assert!(manifest.payload.outputs.is_empty(),"{mode}");
        assert!(manifest.payload.delivery.is_none(),"{mode}");
        assert!(manifest.payload.stages[0].checkpoint.is_none(),"{mode}");
        assert_eq!(fs::read(fixture.root.path().join("input.bin")).unwrap(),b"synthetic validation source\n");
    }
}

#[test]
fn cancellation_before_validator_acceptance_cannot_publish_completion() {
    let fixture = Fixture::new();
    let cancellation = CancellationToken::default();
    run_v3_with_progress_and_cancellation(fixture.request("passed"),&cancellation,|event| {
        if matches!(event,PipelineV3RunProgress::Stage { state:PipelineV3ProgressState::Validating,.. }) { cancellation.cancel(); }
    }).unwrap_err();
    let manifest = status_v3(fixture.run_directory()).unwrap();
    assert_eq!(manifest.payload.state,PipelineV3RunState::Cancelled);
    assert!(manifest.payload.delivery.is_none());
    assert!(manifest.payload.stages[0].checkpoint.is_none());
}

#[test]
fn validator_timeout_cannot_publish_completion() {
    let fixture = Fixture::new();
    let limits = ProviderExecutionLimits { timeout:Duration::from_secs(2),..ProviderExecutionLimits::default() };
    run_v3(fixture.request("slow").with_execution_limits(limits)).unwrap_err();
    let manifest = status_v3(fixture.run_directory()).unwrap();
    assert_eq!(manifest.payload.state,PipelineV3RunState::Failed);
    assert!(manifest.payload.delivery.is_none());
}

#[test]
fn status_refuses_missing_evidence_without_modifying_the_workspace() {
    let fixture = Fixture::new(); let outcome = fixture.run();
    let reference = outcome.delivery.as_ref().unwrap();
    fs::remove_file(outcome.run_directory.join(&reference.relative_path)).unwrap();
    let before = fs::read(&outcome.run_manifest).unwrap();
    status_v3(&outcome.run_directory).unwrap_err();
    assert_eq!(fs::read(&outcome.run_manifest).unwrap(),before);
    let resumed = fixture.resume(&outcome.run_directory).unwrap();
    assert_eq!(resumed.delivery,outcome.delivery);
    assert_eq!(fixture.count(),1);
}

#[test]
fn status_refuses_changed_output_and_resume_rebuilds_it() {
    let fixture = Fixture::new(); let outcome = fixture.run();
    fs::write(&outcome.outputs[0].path,b"tampered").unwrap();
    let error = status_v3(&outcome.run_directory).unwrap_err();
    assert_eq!(error.validation_diagnostic().unwrap().code,ValidationFailureCode::ArtifactChanged);
    let resumed = fixture.resume(&outcome.run_directory).unwrap();
    assert_eq!(resumed.executed_stages,vec!["copy"]);
    assert_eq!(fixture.count(),2);
}

#[test]
fn missing_provider_observation_reexecutes_its_producer_boundary() {
    let fixture = Fixture::new(); let outcome = fixture.run();
    let manifest = status_v3(&outcome.run_directory).unwrap();
    let workspace = PipelineV3Workspace::open_read_only(&outcome.run_directory).unwrap();
    let checkpoint = load_stage_checkpoint(&workspace,manifest.payload.stages[0].checkpoint.as_ref().unwrap()).unwrap();
    let observation = &checkpoint.payload.validations[1].evidence;
    fs::remove_file(outcome.run_directory.join(&observation.relative_path)).unwrap();
    status_v3(&outcome.run_directory).unwrap_err();
    fixture.resume(&outcome.run_directory).unwrap();
    assert_eq!(fixture.count(),2);
}

#[test]
fn changed_validator_implementation_is_refused_before_resume_mutation() {
    let fixture = Fixture::new(); let outcome = fixture.run();
    let before = fs::read(&outcome.run_manifest).unwrap();
    let path = fixture.root.path().join("provider.py");
    let mut bytes = fs::read(&path).unwrap();bytes.extend_from_slice(b"\n# changed implementation\n");fs::write(path,bytes).unwrap();
    fixture.resume(&outcome.run_directory).unwrap_err();
    assert_eq!(fs::read(&outcome.run_manifest).unwrap(),before);
    assert_eq!(fixture.count(),1);
}

#[test]
fn validator_plan_identity_is_stable_and_configuration_sensitive() {
    let fixture = Fixture::new(); let first = fixture.plan("passed");
    assert_eq!(first,fixture.plan("passed"));
    assert_ne!(first.plan_sha256,fixture.plan("partial").plan_sha256);
    assert_eq!(first.payload.stages[0].validation_providers.len(),1);
    let bytes = serde_json::to_vec(&first).unwrap();
    assert_eq!(PipelineV3Plan::from_json_slice(&bytes).unwrap(),first);
}

#[test]
fn missing_or_duplicate_validator_obligations_are_rejected_during_planning() {
    let fixture = Fixture::new(); let mut configuration = fixture.configuration();
    configuration.stages[0].validations[1].validator = None;
    let plan = resolve_pipeline_v3(&configuration,&fixture.bindings(),&fixture.registry("passed"),&fixture.context()).unwrap();
    run_v3(PipelineV3RunRequest::new(plan,fixture.bindings(),fixture.registry("passed")).with_output_directory(fixture.root.path().join("runs"))).unwrap_err();
    assert!(!fixture.root.path().join("runs").exists());
    let mut configuration = fixture.configuration();
    let duplicate = configuration.stages[0].validations[1].clone();
    configuration.stages[0].validations.push(duplicate);
    configuration.validate().unwrap_err();
}

#[test]
fn provider_cannot_rename_its_resolved_report_or_insert_nested_validators() {
    let fixture = Fixture::new(); let mut plan = fixture.plan("passed");
    plan.payload.stages[0].validation_providers[0].stage.outputs[0].artifacts[0].relative_path = "artifacts/other/report.json".to_owned();
    plan.validate().unwrap_err();
}

#[test]
fn forged_raw_provider_report_cannot_match_runtime_evidence() {
    let fixture = Fixture::new();let outcome = fixture.run();
    let manifest = status_v3(&outcome.run_directory).unwrap();
    let workspace = PipelineV3Workspace::open_read_only(&outcome.run_directory).unwrap();
    let checkpoint = load_stage_checkpoint(&workspace,manifest.payload.stages[0].checkpoint.as_ref().unwrap()).unwrap();
    let path = outcome.run_directory.join(&checkpoint.payload.validations[1].evidence.relative_path);
    let mut report: ProviderValidationReport = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    report.raw_observation.push(' ');
    fs::write(path,serde_json::to_vec(&report).unwrap()).unwrap();
    status_v3(&outcome.run_directory).unwrap_err();
}
