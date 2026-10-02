//! Synthetic #34 cases. Authored only; execution is deferred under #64.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use aniflow::*;
use aniflow::cache_v3::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

struct Fixture { root: tempfile::TempDir }
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let kit = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/validation-v1");
        for entry in fs::read_dir(kit).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() { fs::copy(entry.path(), root.path().join(entry.file_name())).unwrap(); }
        }
        // This counter belongs exclusively to the synthetic fixture provider.
        let path = root.path().join("provider.py");
        let code = fs::read_to_string(&path).unwrap().replace("if mode == \"copy\":", "if mode == \"copy\":\n    count = Path(__file__).with_name(\"producer-count\")\n    count.write_text(str(int(count.read_text()) + 1 if count.exists() else 1))");
        let code = code.replace("    shutil.copyfile(request[\"inputs\"][0][\"path\"], output)", "    shutil.copyfile(request[\"inputs\"][0][\"path\"], output)\n    extra = Path(__file__).with_name(\"fixture-extra\")\n    if extra.exists():\n        output.write_bytes(output.read_bytes() + extra.read_bytes())");
        fs::write(&path, code).unwrap(); fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        Self { root }
    }
    fn policy(&self) -> CachePolicy {
        let mut policy = CachePolicy::new(self.root.path().join("cache"), "fixture");
        policy.maximum_entry_bytes = 4 * 1024 * 1024;
        policy.maximum_bytes = 64 * 1024 * 1024;
        policy.minimum_free_bytes = 0;
        policy
    }
    fn bindings(&self) -> Vec<PipelineInputBinding> { vec![PipelineInputBinding::new("source", self.root.path().join("input.bin"))] }
    fn registry(&self) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        for name in ["copy", "passed"] {
            registry.register(ProviderRegistrationDocument::load(self.root.path().join(format!("{name}.registration.json"))).unwrap().into_registration().unwrap()).unwrap();
        }
        registry
    }
    fn config(&self) -> PipelineV3Configuration { PipelineV3Configuration::load(self.root.path().join("pipeline.yml")).unwrap() }
    fn request_with(&self, configuration: PipelineV3Configuration) -> PipelineV3RunRequest {
        let context = PipelinePlanningContext { host: HostResources { cpu_threads: 2, memory_mib: 1024, storage_mib: 1024, gpu_available: false, network_available: false },
            allowed_side_effects: vec![SideEffect::FilesystemRead, SideEffect::FilesystemWrite], offline: true };
        let plan = resolve_pipeline_v3(&configuration, &self.bindings(), &self.registry(), &context).unwrap();
        PipelineV3RunRequest::new(plan, self.bindings(), self.registry()).with_output_directory(self.root.path().join("runs")).with_cache(self.policy())
    }
    fn request(&self) -> PipelineV3RunRequest { self.request_with(self.config()) }
    fn run(&self) -> PipelineV3RunOutcome { run_v3(self.request()).unwrap() }
    fn count(&self, name: &str) -> u64 { fs::read_to_string(self.root.path().join(name)).unwrap().parse().unwrap() }
    fn resume(&self, run: &Path) -> PipelineV3ResumeRequest { PipelineV3ResumeRequest::new(run, self.bindings(), self.registry()).with_cache(self.policy()) }
    fn key(&self) -> String { inspect_cache(&self.policy()).unwrap().entries[0].key_sha256.clone() }
    fn entry(&self) -> PathBuf { self.policy().root.join("entries").join(self.key()) }
}

fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = Vec::new();
    for item in walkdir::WalkDir::new(root).follow_links(false) {
        let item = item.unwrap();
        if item.file_type().is_file() { result.push((item.path().strip_prefix(root).unwrap().to_path_buf(), fs::read(item.path()).unwrap())); }
    }
    result.sort(); result
}

#[test]
fn fresh_and_cached_runs_have_equivalent_delivery_and_fresh_inodes() {
    let fixture = Fixture::new(); let first = fixture.run(); let second = fixture.run();
    assert_eq!(fixture.count("producer-count"), 1);
    assert_eq!(fixture.count("validator-launch-count"), 2);
    assert_eq!(second.reused_stages, vec!["copy"]); assert!(second.executed_stages.is_empty());
    assert_eq!(first.outputs[0].sha256, second.outputs[0].sha256);
    for run in [&first, &second] {
        assert_eq!(status_v3(&run.run_directory).unwrap().payload.state, PipelineV3RunState::Complete);
        let delivery: aniflow::validation::AcceptanceRecord = serde_json::from_slice(&fs::read(run.run_directory.join(&run.delivery.as_ref().unwrap().relative_path)).unwrap()).unwrap();
        assert_eq!(delivery.artifacts[0].sha256, first.outputs[0].sha256);
    }
    let cached = fixture.entry().join("snapshot/artifacts/copy/candidate.bin");
    assert_ne!(fs::metadata(&cached).unwrap().ino(), fs::metadata(&second.outputs[0].path).unwrap().ino());
    let original = fs::read(&cached).unwrap();
    fs::write(&second.outputs[0].path, b"run changed").unwrap();
    assert_eq!(fs::read(&cached).unwrap(), original);
    assert_eq!(fs::read(&first.outputs[0].path).unwrap(), original);
    assert!(second.cache_decisions.iter().any(|d| d.decision == CacheDecisionKind::Hit));
}

#[test]
fn source_and_execution_bound_changes_miss_the_cache() {
    let fixture = Fixture::new(); fixture.run();
    fs::write(fixture.root.path().join("input.bin"), b"different synthetic source").unwrap();
    let changed = fixture.run(); assert_eq!(changed.executed_stages, vec!["copy"]);
    let mut request = fixture.request(); request.execution_limits.maximum_artifact_files = 9;
    assert_eq!(run_v3(request).unwrap().executed_stages, vec!["copy"]);
    assert_eq!(fixture.count("producer-count"), 3);
}

#[test]
fn unrelated_plan_name_change_keeps_stage_identity_but_revalidates_new_context() {
    let fixture = Fixture::new(); let first = fixture.run();
    let mut configuration = fixture.config(); configuration.name = "another-run".to_owned();
    let second = run_v3(fixture.request_with(configuration)).unwrap();
    assert_ne!(first.plan_sha256, second.plan_sha256);
    assert_eq!(second.reused_stages, vec!["copy"]);
    assert_eq!(fixture.count("validator-launch-count"), 2);
    status_v3(second.run_directory).unwrap();
}

#[test]
fn inspection_and_prune_preview_are_read_only_including_missing_root() {
    let fixture = Fixture::new();
    assert!(!inspect_cache(&fixture.policy()).unwrap().exists);
    assert!(!fixture.policy().root.exists());
    fixture.run(); let before = tree(&fixture.policy().root);
    inspect_cache(&fixture.policy()).unwrap(); prune_cache(&fixture.policy(), true).unwrap();
    assert_eq!(before, tree(&fixture.policy().root));
}

#[test]
fn corruption_and_missing_acceptance_never_reuse_cached_outputs() {
    for corrupt_evidence in [false, true] {
        let fixture = Fixture::new(); fixture.run();
        let target = if corrupt_evidence {
            fs::read_dir(fixture.entry().join("snapshot/providers")).unwrap().map(|e| e.unwrap().path())
                .find(|p| p.file_name().unwrap().to_str().unwrap().starts_with("acceptance-")).unwrap()
        } else { fixture.entry().join("snapshot/artifacts/copy/candidate.bin") };
        let mut bytes = fs::read(&target).unwrap(); bytes[0] ^= 1; fs::write(target, bytes).unwrap();
        run_v3(fixture.request()).unwrap_err(); assert_eq!(fixture.count("producer-count"), 1);
        assert_eq!(fs::read(fixture.root.path().join("input.bin")).unwrap(), b"synthetic validation source\n");
    }
}

#[test]
fn unknown_roots_wrong_owners_and_links_are_refused() {
    let fixture = Fixture::new(); fs::create_dir(&fixture.policy().root).unwrap();
    fs::write(fixture.policy().root.join("precious"), b"keep").unwrap();
    run_v3(fixture.request()).unwrap_err();
    assert_eq!(fs::read(fixture.policy().root.join("precious")).unwrap(), b"keep");
    let fixture = Fixture::new(); fixture.run();
    let mut wrong = fixture.policy(); wrong.owner = "another-owner".to_owned();
    assert_eq!(inspect_cache(&wrong).unwrap_err().cache_diagnostic().unwrap().code, CacheFailureCode::OwnershipMismatch);
    std::os::unix::fs::symlink(fixture.root.path().join("input.bin"), fixture.entry().join("unexpected-link")).unwrap();
    prune_cache(&fixture.policy(), false).unwrap_err();
    assert!(fixture.root.path().join("input.bin").is_file());
}

#[test]
fn active_cache_writer_blocks_another_run_invalidation_and_pruning() {
    let fixture = Fixture::new(); fixture.run(); let key = fixture.key();
    let mut checked = false;
    run_v3_with_progress_and_cancellation(fixture.request(), &CancellationToken::default(), |event| {
        if !checked && matches!(event, PipelineV3RunProgress::Started { .. }) {
            checked = true;
            for error in [run_v3(fixture.request()).unwrap_err(), invalidate_cache(&fixture.policy(), &key).unwrap_err(), prune_cache(&fixture.policy(), false).unwrap_err()] {
                assert_eq!(error.cache_diagnostic().unwrap().code, CacheFailureCode::Busy);
            }
            assert!(inspect_cache(&fixture.policy()).unwrap().writer_present);
        }
    }).unwrap();
    assert!(checked); assert!(!inspect_cache(&fixture.policy()).unwrap().writer_present);
}

#[test]
fn explicit_invalidation_then_run_rebuilds_only_the_selected_entry() {
    let fixture = Fixture::new(); fixture.run(); let key = fixture.key();
    let operation = invalidate_cache(&fixture.policy(), &key).unwrap();
    assert_eq!(operation.removed_keys, vec![key]); assert!(operation.reclaimed_bytes > 0);
    assert_eq!(fixture.run().executed_stages, vec!["copy"]);
}

#[test]
fn explicit_rerun_survives_cancellation_and_excludes_historical_checkpoint() {
    let fixture = Fixture::new(); let first = fixture.run();
    let cancellation = CancellationToken::default();
    resume_v3_with_progress_and_cancellation(fixture.resume(&first.run_directory).with_rerun_stages(vec!["copy".to_owned()]), &cancellation, |event| {
        if matches!(event, PipelineV3RunProgress::Stage { state: PipelineV3ProgressState::Running, .. }) { cancellation.cancel(); }
    }).unwrap_err();
    let interrupted = status_v3(&first.run_directory).unwrap();
    assert!(interrupted.payload.stages[0].rerun_required);
    assert!(!interrupted.payload.stages[0].excluded_checkpoints.is_empty());
    let resumed = resume_v3(fixture.resume(&first.run_directory)).unwrap();
    assert_eq!(resumed.executed_stages, vec!["copy"]);
    assert_eq!(fixture.count("producer-count"), 2);
    assert!(!status_v3(first.run_directory).unwrap().payload.stages[0].rerun_required);
}

#[test]
fn unknown_rerun_stage_fails_before_creating_a_run_or_cache() {
    let fixture = Fixture::new();
    run_v3(fixture.request().with_rerun_stages(vec!["missing".to_owned()])).unwrap_err();
    assert!(!fixture.policy().root.exists()); assert!(!fixture.root.path().join("runs").exists());
}

#[test]
fn storage_exhaustion_refuses_before_producer_launch() {
    let fixture = Fixture::new(); let mut policy = fixture.policy(); policy.minimum_free_bytes = u64::MAX;
    let error = run_v3(fixture.request().with_cache(policy)).unwrap_err();
    assert_eq!(error.cache_diagnostic().unwrap().code, CacheFailureCode::StorageLimit);
    assert!(!fixture.root.path().join("producer-count").exists());
}

#[test]
fn cancellation_during_validation_does_not_publish_a_cache_entry() {
    let fixture = Fixture::new(); let cancellation = CancellationToken::default();
    run_v3_with_progress_and_cancellation(fixture.request(), &cancellation, |event| {
        if matches!(event, PipelineV3RunProgress::Stage { state: PipelineV3ProgressState::Validating, .. }) { cancellation.cancel(); }
    }).unwrap_err();
    assert!(inspect_cache(&fixture.policy()).unwrap().entries.is_empty());
    assert!(!inspect_cache(&fixture.policy()).unwrap().writer_present);
}

#[test]
fn noncacheable_provider_runs_each_time_and_records_the_reason() {
    let fixture = Fixture::new(); let path = fixture.root.path().join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["capabilities"][0]["behavior"]["cacheable"] = Value::Bool(false);
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fixture.run(); let second = fixture.run();
    assert_eq!(fixture.count("producer-count"), 2);
    assert!(inspect_cache(&fixture.policy()).unwrap().entries.is_empty());
    assert!(second.cache_decisions.iter().any(|d| d.decision == CacheDecisionKind::NotCacheable));
}

#[test]
fn expired_entries_are_misses_and_pruning_is_bounded_and_explicit() {
    let fixture = Fixture::new(); fixture.run(); let path = fixture.entry().join("entry.json");
    let mut entry: CacheEntry = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    entry.payload.created_at = chrono::Utc::now() - chrono::Duration::days(60);
    // BTreeMap-backed serde_json::Value yields the documented canonical key order.
    let payload = serde_json::to_value(&entry.payload).unwrap();
    entry.entry_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&payload).unwrap()));
    fs::write(path, serde_json::to_vec_pretty(&entry).unwrap()).unwrap();
    assert_eq!(inspect_cache(&fixture.policy()).unwrap().entries[0].state, CacheEntryState::Expired);
    assert_eq!(fixture.run().executed_stages, vec!["copy"]);
    let before = tree(&fixture.policy().root);
    let preview = prune_cache(&fixture.policy(), true).unwrap(); assert_eq!(preview.selected_keys.len(), 1);
    assert_eq!(before, tree(&fixture.policy().root));
    let applied = prune_cache(&fixture.policy(), false).unwrap(); assert_eq!(applied.removed_keys, preview.selected_keys);
}

#[test]
fn conflicting_accepted_outputs_never_overwrite_an_existing_key() {
    let fixture = Fixture::new(); fixture.run();
    let entry = fixture.entry(); let before = tree(&entry);
    fs::write(fixture.root.path().join("fixture-extra"), b"injected nondeterminism").unwrap();
    let error = run_v3(fixture.request().with_rerun_stages(vec!["copy".to_owned()])).unwrap_err();
    assert_eq!(error.cache_diagnostic().unwrap().code, CacheFailureCode::ConflictingEntry);
    assert_eq!(tree(&entry), before);
}

#[test]
fn writer_locks_survive_only_abnormal_ownership_and_are_never_stolen() {
    let fixture = Fixture::new(); fixture.run();
    let path = fixture.policy().root.join("writer.lock");
    fs::write(&path, b"unreconciled crashed writer").unwrap();
    assert_eq!(run_v3(fixture.request()).unwrap_err().cache_diagnostic().unwrap().code, CacheFailureCode::Busy);
    assert_eq!(fs::read(path).unwrap(), b"unreconciled crashed writer");
}
