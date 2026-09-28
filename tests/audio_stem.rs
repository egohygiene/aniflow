#![cfg(unix)]

//! Accepted synthetic separation runs exercise public lineage APIs, not model quality.
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::audio_analysis::{AudioAnalysis, AudioFrameRange};
use aniflow::audio_inspection::{self, AudioInspectionConfiguration, AudioInspectionRequest};
use aniflow::audio_signal::{self, SignalAnalysisConfiguration, SignalAnalysisRequest};
use aniflow::audio_stem::{AudioStemLineageReport, StemSelection};
use aniflow::{
    CancellationToken, PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunState, status_v3,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;
use walkdir::WalkDir;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                digest(&fs::read(entry.path()).unwrap()),
            )
        })
        .collect()
}

struct Fixture {
    root: TempDir,
    descriptor: Value,
}

impl Fixture {
    fn new(provider: &str) -> Self {
        Self::with_duration(provider, 48000, 0)
    }

    fn with_duration(provider: &str, rate: u32, extra_frames: u32) -> Self {
        let root = tempfile::Builder::new()
            .prefix("stem lineage café 雪 ")
            .tempdir()
            .unwrap();
        let output = Command::new("python3")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/smoke-audio-stem.py"))
            .arg("--aniflow")
            .arg(env!("CARGO_BIN_EXE_aniflow"))
            .arg("--prepare-only")
            .arg(root.path())
            .arg("--provider")
            .arg(provider)
            .arg("--rate")
            .arg(rate.to_string())
            .arg("--extra-frames")
            .arg(extra_frames.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "synthetic {provider} setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let descriptor = serde_json::from_slice(&output.stdout).unwrap();
        Self { root, descriptor }
    }

    fn path(&self, key: &str) -> PathBuf {
        PathBuf::from(self.descriptor[key].as_str().unwrap())
    }

    fn stem(&self, index: usize) -> &str {
        self.descriptor["stems"][index].as_str().unwrap()
    }

    fn selection(&self, stem: &str) -> StemSelection {
        StemSelection::new(
            self.path("separation_run"),
            self.descriptor["stage"].as_str().unwrap(),
            stem,
        )
    }

    fn request(&self, selection: Option<StemSelection>) -> AudioInspectionRequest {
        let configuration = AudioInspectionConfiguration::from_json_slice(
            &fs::read(self.path("configuration")).unwrap(),
        )
        .unwrap();
        let request = AudioInspectionRequest::new(
            self.path("mix"),
            configuration,
            env!("CARGO_BIN_EXE_aniflow"),
        );
        match selection {
            Some(selection) => request.with_stem_selection(selection),
            None => request,
        }
    }

    fn inspect(&self, stem: &str) -> PipelineV3RunOutcome {
        audio_inspection::run(
            self.request(Some(self.selection(stem))),
            Some(self.root.path().join("analysis runs")),
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap()
    }

    fn artifact(&self, id: &str) -> PathBuf {
        PathBuf::from(
            self.descriptor["outputs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|output| output["artifact"] == id)
                .unwrap()["path"]
                .as_str()
                .unwrap(),
        )
    }

    fn launches(&self) -> Vec<u8> {
        fs::read(self.path("tools").join("launches")).unwrap_or_default()
    }

    fn refuse(&self, selection: StemSelection) {
        let before = snapshot(&self.path("separation_run"));
        let launches = self.launches();
        assert!(
            audio_inspection::plan(
                &self.request(Some(selection.clone())),
                &CancellationToken::default()
            )
            .is_err(),
            "accepted invalid selection {selection:?}"
        );
        assert_eq!(snapshot(&self.path("separation_run")), before);
        assert_eq!(
            self.launches(),
            launches,
            "refusal launched an analysis tool"
        );
    }
}

fn output(outcome: &PipelineV3RunOutcome, id: &str) -> PathBuf {
    outcome
        .outputs
        .iter()
        .find(|output| output.id == id)
        .unwrap()
        .path
        .clone()
}

fn check_lineage(fixture: &Fixture, stem: &str, outcome: &PipelineV3RunOutcome) {
    let analysis =
        AudioAnalysis::from_json_slice(&fs::read(output(outcome, "analysis")).unwrap()).unwrap();
    let lineage = AudioStemLineageReport::from_json_slice(
        &fs::read(output(outcome, "stem_lineage")).unwrap(),
    )
    .unwrap();
    let identity = analysis.source.stem.as_ref().unwrap();
    assert_eq!(identity.id, stem);
    assert_eq!(
        identity.original_mix.sha256,
        fixture.descriptor["mix_sha256"]
    );
    assert_eq!(identity.relationship_evidence_id, "stem_lineage");
    assert_eq!(
        analysis.source.artifact.sha256,
        digest(&fs::read(fixture.artifact(stem)).unwrap())
    );
    assert_eq!(lineage.lineage.source_stem_artifact_id, stem);
    assert_eq!(
        lineage.lineage.original_mix.artifact.sha256,
        identity.original_mix.sha256
    );
    assert_eq!(lineage.lineage.scope.channels, [0, 1]);
    assert_eq!(lineage.lineage.scope.stem_id.as_deref(), Some(stem));
    assert_eq!(
        lineage.lineage.range,
        AudioFrameRange {
            start: 0,
            end: analysis.source.frame_count
        }
    );
    assert_eq!(
        lineage.lineage.timing_basis,
        aniflow::audio_stem::AudioStemTimingBasis::ZeroOriginDurationOnly
    );
    for observation in &analysis.observations {
        assert_eq!(observation.scope.stem_id.as_deref(), Some(stem));
    }
    for timeline in &analysis.timelines {
        assert_eq!(timeline.scope.stem_id.as_deref(), Some(stem));
    }
    for artifact in &outcome.outputs {
        assert_eq!(digest(&fs::read(&artifact.path).unwrap()), artifact.sha256);
    }
    assert_eq!(
        digest(&fs::read(fixture.path("mix")).unwrap()),
        fixture.descriptor["mix_sha256"]
    );
}

#[test]
fn typed_demucs_fixture_mix_and_both_stems_retain_distinct_normalized_identity() {
    let fixture = Fixture::new("demucs");
    let retained = snapshot(&fixture.path("separation_run"));
    let mix = audio_inspection::run(
        fixture.request(None),
        Some(fixture.root.path().join("mix analysis")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    let mix_analysis =
        AudioAnalysis::from_json_slice(&fs::read(output(&mix, "analysis")).unwrap()).unwrap();
    assert!(mix_analysis.source.stem.is_none());
    for stem in [fixture.stem(0), fixture.stem(1)] {
        let outcome = fixture.inspect(stem);
        check_lineage(&fixture, stem, &outcome);
        let before_status = snapshot(&outcome.run_directory);
        assert_eq!(
            status_v3(&outcome.run_directory).unwrap().payload.state,
            PipelineV3RunState::Complete
        );
        assert_eq!(snapshot(&outcome.run_directory), before_status);
        let launches = fixture.launches();
        let resumed = audio_inspection::resume(
            fixture.request(Some(fixture.selection(stem))),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
        assert!(resumed.executed_stages.is_empty());
        assert_eq!(resumed.outputs, outcome.outputs);
        assert_eq!(fixture.launches(), launches);
    }
    assert_eq!(snapshot(&fixture.path("separation_run")), retained);
}

#[test]
fn non_demucs_arbitrary_roles_and_opaque_evidence_support_signal_analysis() {
    let fixture = Fixture::new("generic");
    let retained = snapshot(&fixture.path("separation_run"));
    assert!(
        serde_json::from_slice::<Value>(&fs::read(fixture.artifact("opaque_evidence")).unwrap())
            .is_err()
    );
    for stem in [fixture.stem(0), fixture.stem(1)] {
        let request = SignalAnalysisRequest::new(
            fixture.request(Some(fixture.selection(stem))),
            SignalAnalysisConfiguration::default(),
        );
        let outcome = audio_signal::run(
            request.clone(),
            Some(fixture.root.path().join("signal runs")),
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
        check_lineage(&fixture, stem, &outcome);
        let report = AudioStemLineageReport::from_json_slice(
            &fs::read(output(&outcome, "stem_lineage")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            report.lineage.source_provider.id,
            "org.example.synthetic.stem-separator"
        );
        assert_eq!(
            report.lineage.source_stem_port,
            if stem == "drums" {
                "percussion"
            } else {
                "ambience"
            }
        );
        let launches = fixture.launches();
        let resumed = audio_signal::resume(
            request,
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
        assert!(resumed.executed_stages.is_empty());
        assert_eq!(fixture.launches(), launches);
    }
    assert_eq!(snapshot(&fixture.path("separation_run")), retained);
}

#[test]
fn unsupported_partial_or_ambiguous_selections_refuse_without_runtime_work() {
    let fixture = Fixture::new("generic");
    let selection = fixture.selection(fixture.stem(0));
    let mut invalid = selection.clone();
    invalid.stage_id = "unknown".to_owned();
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.stem_id = "unknown".to_owned();
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.stem_id = "opaque_evidence".to_owned();
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.channels = Some(vec![0]);
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.channels = Some(vec![1, 0]);
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.range = Some(AudioFrameRange {
        start: 1,
        end: 4800,
    });
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.range = Some(AudioFrameRange {
        start: 0,
        end: 4799,
    });
    fixture.refuse(invalid);
    let mut invalid = selection.clone();
    invalid.duration_tolerance_milliseconds = 21;
    fixture.refuse(invalid);
    let mut explicit = selection;
    explicit.channels = Some(vec![0, 1]);
    explicit.range = Some(AudioFrameRange {
        start: 0,
        end: 4800,
    });
    audio_inspection::plan(
        &fixture.request(Some(explicit)),
        &CancellationToken::default(),
    )
    .unwrap();
}

#[test]
fn stale_missing_outside_workspace_and_sibling_evidence_refuse() {
    let fixture = Fixture::new("generic");
    let selection = fixture.selection(fixture.stem(0));
    for artifact in ["opaque_evidence", fixture.stem(1)] {
        let path = fixture.artifact(artifact);
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"changed synthetic evidence").unwrap();
        fixture.refuse(selection.clone());
        fs::remove_file(&path).unwrap();
        fixture.refuse(selection.clone());
        let outside = fixture.root.path().join("outside-workspace-copy");
        fs::write(&outside, &original).unwrap();
        symlink(&outside, &path).unwrap();
        fixture.refuse(selection.clone());
        fs::remove_file(&path).unwrap();
        fs::write(&path, original).unwrap();
    }
    let status = status_v3(fixture.path("separation_run")).unwrap();
    let checkpoint = fixture.path("separation_run").join(
        &status.payload.stages[0]
            .checkpoint
            .as_ref()
            .unwrap()
            .relative_path,
    );
    let original = fs::read(&checkpoint).unwrap();
    fs::write(&checkpoint, b"{}").unwrap();
    fixture.refuse(selection.clone());
    fs::write(&checkpoint, original).unwrap();
    let mix = fixture.path("mix");
    let original = fs::read(&mix).unwrap();
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    fs::write(&mix, changed).unwrap();
    fixture.refuse(selection);
    fs::write(&mix, original).unwrap();
}

#[test]
fn exact_rational_duration_tolerance_accepts_boundary_and_refuses_one_frame_over() {
    for (extra_frames, accepted) in [(882, true), (883, false)] {
        let fixture = Fixture::with_duration("generic", 44100, extra_frames);
        let selection = fixture.selection(fixture.stem(0));
        let result = audio_inspection::plan(
            &fixture.request(Some(selection.clone())),
            &CancellationToken::default(),
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "44100Hz extra_frames={extra_frames}: {result:?}"
        );
        if accepted {
            let mut tighter = selection;
            tighter.duration_tolerance_milliseconds = 19;
            fixture.refuse(tighter);
        }
    }
}

#[test]
fn changed_raw_lineage_authority_cannot_reuse_a_completed_analysis() {
    let fixture = Fixture::new("generic");
    let selection = fixture.selection(fixture.stem(0));
    let outcome = fixture.inspect(fixture.stem(0));
    let before = snapshot(&outcome.run_directory);
    let launches = fixture.launches();
    let path = fixture.path("separation_run").join("plan/plan.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(&path, bytes).unwrap();
    // The source plan remains canonically valid; its retained raw identity changed.
    assert_eq!(
        status_v3(fixture.path("separation_run"))
            .unwrap()
            .payload
            .state,
        PipelineV3RunState::Complete
    );
    audio_inspection::plan(
        &fixture.request(Some(selection.clone())),
        &CancellationToken::default(),
    )
    .unwrap();
    assert!(
        audio_inspection::resume(
            fixture.request(Some(selection)),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
    assert_eq!(snapshot(&outcome.run_directory), before);
    assert_eq!(fixture.launches(), launches);
}

#[test]
fn cancelled_stem_inspection_retains_recovery_and_preserves_mix_and_stems() {
    let fixture = Fixture::new("generic");
    let retained = snapshot(&fixture.path("separation_run"));
    fs::write(fixture.path("tools").join("mode"), "sleep").unwrap();
    let cancellation = CancellationToken::default();
    let signal = cancellation.clone();
    let marker = fixture.path("tools").join("analysis-started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !marker.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        signal.cancel();
    });
    let mut run = None;
    let result = audio_inspection::run(
        fixture.request(Some(fixture.selection(fixture.stem(0)))),
        Some(fixture.root.path().join("cancelled runs")),
        &cancellation,
        |progress| {
            if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
                run = Some(run_directory.clone());
            }
        },
    );
    cancel.join().unwrap();
    assert!(result.is_err());
    let run = run.unwrap();
    assert_eq!(
        status_v3(&run).unwrap().payload.state,
        PipelineV3RunState::Cancelled
    );
    assert_eq!(snapshot(&fixture.path("separation_run")), retained);
    fs::write(fixture.path("tools").join("mode"), "ok").unwrap();
    let resumed = audio_inspection::resume(
        fixture.request(Some(fixture.selection(fixture.stem(0)))),
        &run,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    check_lineage(&fixture, fixture.stem(0), &resumed);
    assert_eq!(
        status_v3(run).unwrap().payload.state,
        PipelineV3RunState::Complete
    );
}
