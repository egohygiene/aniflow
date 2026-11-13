//! Fake tools exercise restart/authority boundaries, never meter accuracy.
use super::*;
use aniflow::delivery_audio::final_lossless_qc::{
    self as qc, FinalLosslessQcRequest, FinalLosslessSubject,
    resumable::{self, ResumableQcPhase, ResumableQcRequest},
};
fn fixture() -> Fixture {
    let mut f = Fixture::with_pcm(44100, 2, 44100 * 4);
    // Private snapshots use the exact pinned script but retain a fixed external
    // synthetic control/log directory, rather than depending on private files.
    let control = serde_json::to_string(&f.root.path()).unwrap();
    let script=TOOL.replace("root = pathlib.Path(__file__).parent",&format!("root = pathlib.Path({control})"))
        .replace("6.1.1","8.0.1")
        .replace("args = sys.argv[1:]","args = sys.argv[1:]\nwith (root / 'qc-invocations').open('a') as log: log.write(json.dumps(args) + '\\n')");
    for pin in [&mut f.tools.ffmpeg, &mut f.tools.ffprobe] {
        fs::write(&pin.executable, &script).unwrap();
        pin.version = "8.0.1".into();
        pin.sha256 = digest(script.as_bytes());
    }
    f
}
fn accepted(f: &Fixture) -> ResumableQcRequest {
    let original = audio_signal::run(
        f.native_request(),
        Some(f.root.path().join("original-runs")),
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    ResumableQcRequest {
        schema: "aniflow.resumable-final-audio-qc-request/v1".into(),
        qc: FinalLosslessQcRequest {
            schema: "aniflow.final-lossless-audio-qc-request/v2".into(),
            subject: FinalLosslessSubject::AcceptedPcm {
                file: f.input.clone(),
                signal_run_directory: original.run_directory,
            },
            configuration: f.tools.clone(),
            provider_executable: env!("CARGO_BIN_EXE_aniflow").into(),
            maximum_pcm_bytes: 268435456,
            maximum_working_bytes: 1073741824,
            minimum_free_bytes: 0,
        },
    }
}
fn cancel_after_inspection(f: &Fixture, request: &ResumableQcRequest) -> PathBuf {
    let root = f.root.path().join("final-qc");
    let token = CancellationToken::default();
    let cancel = token.clone();
    assert!(resumable::run(request,&root,&token,|event| {
        if matches!(event,PipelineV3RunProgress::Stage {stage_id,state:aniflow::PipelineV3ProgressState::Complete} if stage_id=="inspect_audio"){cancel.cancel();}
    }).is_err());
    root
}
#[test]
fn recognized_qc_resume_reuses_inspection_and_status_launches_no_tools() {
    let f = fixture();
    let request = accepted(&f);
    let root = cancel_after_inspection(&f, &request);
    let before = fs::read(f.root.path().join("qc-invocations")).unwrap();
    let current = resumable::status(&root, &CancellationToken::default()).unwrap();
    assert_eq!(current.phase, ResumableQcPhase::NativeSignal);
    assert!(!current.complete);
    assert_eq!(
        fs::read(f.root.path().join("qc-invocations")).unwrap(),
        before
    );
    let wav = fs::read(root.join("decoded.wav")).unwrap();
    let capture = fs::read(root.join("capture.json")).unwrap();
    let mut reused = Vec::new();
    let complete = resumable::resume(&root, &CancellationToken::default(), |event| {
        if let PipelineV3RunProgress::Stage {
            stage_id,
            state: aniflow::PipelineV3ProgressState::Reused,
        } = event
        {
            reused.push(stage_id.clone());
        }
    })
    .unwrap();
    assert!(complete.complete);
    assert_eq!(reused, ["inspect_audio"]);
    assert_eq!(fs::read(root.join("decoded.wav")).unwrap(), wav);
    assert_eq!(fs::read(root.join("capture.json")).unwrap(), capture);
    let report = complete.report.unwrap();
    assert_eq!(report.accepted_pcm.source.sample_rate_hz, 44100);
    assert!(!report.runtime_qualified);
    let external = digest(&fs::read(root.join("qc.json")).unwrap());
    assert_eq!(
        qc::observe(&root, &external, &CancellationToken::default()).unwrap(),
        report
    );
}
#[test]
fn committed_meter_resumes_final_receipt_without_decoding_or_launching_any_tool() {
    let f = fixture();
    let request = accepted(&f);
    let root = f.root.path().join("final-qc");
    let complete = resumable::run(&request, &root, &CancellationToken::default(), |_| {}).unwrap();
    let bytes = fs::read(root.join("qc.json")).unwrap();
    fs::remove_file(root.join("qc.json")).unwrap();
    let before = fs::read(f.root.path().join("qc-invocations")).unwrap();
    assert_eq!(
        resumable::status(&root, &CancellationToken::default())
            .unwrap()
            .phase,
        ResumableQcPhase::Meter
    );
    let resumed = resumable::resume(&root, &CancellationToken::default(), |_| {
        panic!("native child must be retained")
    })
    .unwrap();
    assert_eq!(resumed.report, complete.report);
    assert_eq!(fs::read(root.join("qc.json")).unwrap(), bytes);
    assert_eq!(
        fs::read(f.root.path().join("qc-invocations")).unwrap(),
        before
    );
}
#[test]
fn unmarked_decode_and_changed_original_are_refused_without_destroying_native_checkpoints() {
    let f = fixture();
    let request = accepted(&f);
    let root = cancel_after_inspection(&f, &request);
    let decoded = fs::read(root.join("decoded.json")).unwrap();
    let wav = fs::read(root.join("decoded.wav")).unwrap();
    fs::remove_file(root.join("decoded.json")).unwrap();
    let before = fs::read(f.root.path().join("qc-invocations")).unwrap();
    assert!(resumable::resume(&root, &CancellationToken::default(), |_| {}).is_err());
    assert_eq!(fs::read(root.join("decoded.wav")).unwrap(), wav);
    fs::write(root.join("decoded.json"), decoded).unwrap();
    let mut changed = fs::read(&f.input).unwrap();
    changed[44] ^= 1;
    fs::write(&f.input, &changed).unwrap();
    assert!(resumable::resume(&root, &CancellationToken::default(), |_| {}).is_err());
    assert!(root.join("children/signal").is_dir());
    assert!(!root.join("qc.json").exists());
    assert_eq!(
        fs::read(f.root.path().join("qc-invocations")).unwrap(),
        before
    );
    assert_eq!(fs::read(f.input).unwrap(), changed);
}
#[test]
fn publishing_v2_rebinds_exact_completed_qc_then_pauses_for_supplied_whole_file_review() {
    use aniflow::music_video_recipe::publishing::{PublishingPhase, PublishingStepState, audio};
    let f = fixture();
    let qc = accepted(&f);
    let request:audio::AudioPublishingRequest=serde_json::from_value(serde_json::json!({
        "schema":"aniflow.audio-publishing-request/v2","subject":qc.qc.subject,"provider_executable":qc.qc.provider_executable,"export":null,
        "audio_qc":{"configuration":qc.qc.configuration,"maximum_pcm_bytes":268435456,"maximum_working_bytes":1073741824,"minimum_free_bytes":0},
        "profile":{"schema":"aniflow.destination-profile/v1","id":"synthetic","revision":"1","destination":"soundcloud","route":"manual","evidence":{"source_urls":["https://help.soundcloud.com/hc/synthetic-fixture"],"checked_on":"2026-11-09","valid_through":"2026-11-09","checked_by":"synthetic fixture only"},"maximum_file_bytes":268435456,"maximum_duration":{"numerator":600,"denominator":1},"require_loudness_evidence":true,"requirements":{"kind":"pcm","sample_rates_hz":[44100],"channels":[2],"encodings":["pcm16"]}},
        "evaluated_on":"2026-11-09","package":{"public_metadata":{"title":"Synthetic","creator":"Fixture","release_id":"synthetic","version":"1","credits":"fixture only","description":null,"artwork_references":[],"provenance_reference":null},"metadata_scope_policy":"acknowledge_inherited_or_uninspected_scope","maximum_package_bytes":536870912,"minimum_free_bytes":0},"retention":null})).unwrap();
    let root = f.root.path().join("publishing");
    let first = audio::run(request, &root, &CancellationToken::default(), |_, _| {}).unwrap();
    assert_eq!(first.next_action.phase, Some(PublishingPhase::FinalReview));
    fs::remove_file(root.join("audio-qc-binding.json")).unwrap();
    fs::remove_file(root.join("preflight.json")).unwrap();
    let before = fs::read(f.root.path().join("qc-invocations")).unwrap();
    let current = audio::status(&root, &CancellationToken::default()).unwrap();
    assert!(
        current
            .steps
            .iter()
            .any(|s| s.phase == PublishingPhase::FinalAudioQc
                && s.state == PublishingStepState::Incomplete)
    );
    let resumed = audio::resume(&root, &CancellationToken::default(), |_, _| {
        panic!("completed native QC must not rerun")
    })
    .unwrap();
    assert_eq!(resumed.next_action.action, "record_review");
    assert_eq!(
        fs::read(f.root.path().join("qc-invocations")).unwrap(),
        before
    );
    assert!(resumed.review.is_none());
    assert!(resumed.package.is_none());
    assert!(resumed.retention.is_none());
    assert!(!resumed.complete);
}
