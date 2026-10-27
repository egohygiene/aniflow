//! Synthetic runtime-integrity regressions authored for the #37 checkpoint.
//! Execution is deferred under #64; these fixtures do not qualify Upscayl.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use aniflow::{
    CancellationToken, Error, ErrorCategory, HostResources, PipelineInputBinding,
    PipelinePlanningContext, PipelineV3Configuration, PipelineV3ProgressState,
    PipelineV3RunProgress, PipelineV3RunRequest, PipelineV3RunState,
    PipelineV3StageState, ProviderRegistrationDocument, ProviderRegistry,
    SideEffect, resolve_pipeline_v3, run_v3_with_progress_and_cancellation, status_v3,
};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new(mutation: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/validation-v1");
        for entry in fs::read_dir(kit).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                fs::copy(entry.path(), root.join(entry.file_name())).unwrap();
            }
        }

        // Keep the existing synthetic protocol/report implementation. Inject
        // controlled mutations after the copy or immediately before the
        // validator reports success, so process exit alone cannot pass a gate.
        let executable = root.join("provider.py");
        let source = fs::read_to_string(&executable).unwrap();
        let copy = "    shutil.copyfile(request[\"inputs\"][0][\"path\"], output)";
        assert_eq!(source.matches(copy).count(), 1);
        let source = source.replace(copy, r#"    fixture_root = Path(__file__).resolve().parent
    with (fixture_root / "copy-launches").open("a") as launches:
        launches.write(request["stage_id"] + "\n")
    shutil.copyfile(request["inputs"][0]["path"], output)
    mutation = (fixture_root / "mutation-mode").read_text()
    if request["stage_id"] == "copy":
        if mutation == "bound_input":
            Path(request["inputs"][0]["path"]).write_bytes(b"changed bound input")
        elif mutation == "unrelated_input":
            (fixture_root / "unrelated.bin").write_bytes(b"changed unrelated input")"#);
        let anchor = "canonical = json.dumps(context,";
        assert_eq!(source.matches(anchor).count(), 1);
        let source = source.replace(anchor, r#"fixture_root = Path(__file__).resolve().parent
if (fixture_root / "mutation-mode").read_text() == "validator_unrelated":
    (fixture_root / "unrelated.bin").write_bytes(b"changed by external validator")

canonical = json.dumps(context,"#);
        fs::write(&executable, source).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(root.join("mutation-mode"), mutation).unwrap();
        fs::write(root.join("unrelated.bin"), b"unrelated immutable input").unwrap();
        Self { _directory: directory, root }
    }

    fn configuration(&self, external_validator: bool, later_use: bool) -> PipelineV3Configuration {
        let mut configuration = PipelineV3Configuration::load(self.root.join("pipeline.yml")).unwrap();
        let mut unrelated = configuration.inputs[0].clone();
        unrelated.id = "unrelated".to_owned();
        configuration.inputs.push(unrelated);

        if !external_validator {
            configuration.stages[0].validations.retain(|validation| validation.validator.is_none());
            configuration.outputs[0].required_validations = vec!["candidate-integrity".to_owned()];
        }
        if later_use {
            assert!(!external_validator);
            let mut second = configuration.stages[0].clone();
            second.id = "consume_unrelated".to_owned();
            second.depends_on.clear();
            second.inputs[0].artifacts = vec!["unrelated".to_owned()];
            second.outputs[0].artifacts[0].id = "unrelated_candidate".to_owned();
            second.outputs[0].artifacts[0].relative_path =
                "artifacts/consume_unrelated/candidate.bin".to_owned();
            second.validations[0].id = "unrelated-integrity".to_owned();
            second.validations[0].artifact = "unrelated_candidate".to_owned();
            configuration.stages.push(second);
            configuration.outputs[0].artifact = "unrelated_candidate".to_owned();
            configuration.outputs[0].required_validations = vec!["unrelated-integrity".to_owned()];
        }
        configuration
    }

    fn registry(&self, external_validator: bool) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        for name in if external_validator { vec!["copy", "passed"] } else { vec!["copy"] } {
            let registration = ProviderRegistrationDocument::load(
                self.root.join(format!("{name}.registration.json")),
            ).unwrap().into_registration().unwrap();
            registry.register(registration).unwrap();
        }
        registry
    }

    fn run_failure(
        &self,
        external_validator: bool,
        later_use: bool,
    ) -> (Error, PathBuf, Vec<(String, PipelineV3ProgressState)>) {
        let bindings = vec![
            PipelineInputBinding::new("source", self.root.join("input.bin")),
            PipelineInputBinding::new("unrelated", self.root.join("unrelated.bin")),
        ];
        let registry = self.registry(external_validator);
        let context = PipelinePlanningContext {
            host: HostResources {
                cpu_threads: 2,
                memory_mib: 1024,
                storage_mib: 1024,
                gpu_available: false,
                network_available: false,
            },
            allowed_side_effects: vec![SideEffect::FilesystemRead, SideEffect::FilesystemWrite],
            offline: true,
        };
        let plan = resolve_pipeline_v3(
            &self.configuration(external_validator, later_use), &bindings, &registry, &context,
        ).unwrap();
        let mut run_directory = None;
        let mut stages = Vec::new();
        let error = run_v3_with_progress_and_cancellation(
            PipelineV3RunRequest::new(plan, bindings, registry)
                .with_output_directory(self.root.join("runs")),
            &CancellationToken::default(),
            |event| match event {
                PipelineV3RunProgress::Started { run_directory: directory, .. } => {
                    run_directory = Some(directory.clone());
                }
                PipelineV3RunProgress::Stage { stage_id, state } => {
                    stages.push((stage_id.clone(), *state));
                }
            },
        ).expect_err("a changed artifact must never complete the whole run");
        (error, run_directory.unwrap(), stages)
    }

    fn launches(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("copy-launches")).unwrap()
            .lines().map(str::to_owned).collect()
    }
}

#[test]
fn built_in_integrity_refuses_bound_input_mutation_before_checkpoint_acceptance() {
    let fixture = Fixture::new("bound_input");
    let (error, run_directory, stages) = fixture.run_failure(false, false);
    assert_eq!(error.category(), ErrorCategory::State);
    assert!(!stages.contains(&("copy".to_owned(), PipelineV3ProgressState::Complete)));
    let manifest = status_v3(&run_directory).unwrap();
    assert_eq!(manifest.payload.state, PipelineV3RunState::Failed);
    assert!(manifest.payload.stages[0].checkpoint.is_none());
    assert!(manifest.payload.delivery.is_none());
    assert_eq!(fixture.launches(), vec!["copy"]);
}

#[test]
fn unrelated_input_mutation_is_refused_before_a_later_independent_stage_uses_it() {
    let fixture = Fixture::new("unrelated_input");
    let (error, run_directory, stages) = fixture.run_failure(false, true);
    assert_eq!(error.category(), ErrorCategory::State);
    assert!(stages.contains(&("copy".to_owned(), PipelineV3ProgressState::Complete)));
    assert_eq!(fixture.launches(), vec!["copy"], "the later provider must not launch");
    let manifest = status_v3(&run_directory).unwrap();
    assert_eq!(manifest.payload.state, PipelineV3RunState::Failed);
    assert!(manifest.payload.stages[0].checkpoint.is_some());
    assert_eq!(manifest.payload.stages[1].state, PipelineV3StageState::Failed);
    assert!(manifest.payload.stages[1].checkpoint.is_none());
    assert!(manifest.payload.delivery.is_none());
}

#[test]
fn unrelated_input_mutation_is_refused_at_final_delivery_even_if_never_consumed() {
    let fixture = Fixture::new("unrelated_input");
    let (error, run_directory, stages) = fixture.run_failure(false, false);
    assert_eq!(error.category(), ErrorCategory::State);
    assert!(stages.contains(&("copy".to_owned(), PipelineV3ProgressState::Complete)));
    let manifest = status_v3(&run_directory).unwrap();
    assert_eq!(manifest.payload.state, PipelineV3RunState::Failed);
    assert!(manifest.payload.stages[0].checkpoint.is_some());
    assert!(manifest.payload.outputs.is_empty());
    assert!(manifest.payload.delivery.is_none());
}

#[test]
fn external_validator_still_rechecks_unrelated_inputs_before_stage_acceptance() {
    let fixture = Fixture::new("validator_unrelated");
    let (error, run_directory, stages) = fixture.run_failure(true, false);
    assert_eq!(error.category(), ErrorCategory::State);
    assert_eq!(fs::read_to_string(fixture.root.join("validator-launch-count")).unwrap(), "1");
    assert!(!stages.contains(&("copy".to_owned(), PipelineV3ProgressState::Complete)));
    let manifest = status_v3(&run_directory).unwrap();
    assert_eq!(manifest.payload.state, PipelineV3RunState::Failed);
    assert!(manifest.payload.stages[0].checkpoint.is_none());
    assert!(manifest.payload.delivery.is_none());
    assert_eq!(fs::read(fixture.root.join("input.bin")).unwrap(), b"synthetic validation source\n");
}
