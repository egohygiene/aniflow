use super::*;
fn disjoint(root: &Path, intent: &AudioPublishingIntent) -> Result<()> {
    local::disjoint(
        root,
        &intent.request.subject.input(),
        &intent.source.file.locator,
    )?;
    if let destination::MediaFacts::LosslessAudio { report } = &intent.source.facts {
        let original = &report.operation.source;
        local::disjoint(
            root,
            &MediaInput::AcceptedPcm {
                file: original.file.clone(),
                signal_run_directory: original.signal_run_directory.clone(),
            },
            &original.file,
        )?;
    }
    for component in &intent.components {
        if root.starts_with(&component.path) || component.path.starts_with(root) {
            return Err(state("audio publishing root overlaps explicit component"));
        }
    }
    Ok(())
}
fn current_subject(media: &MediaInput) -> Result<FinalLosslessSubject> {
    match media {
        MediaInput::AcceptedPcm {
            file,
            signal_run_directory,
        } => Ok(FinalLosslessSubject::AcceptedPcm {
            file: file.clone(),
            signal_run_directory: signal_run_directory.clone(),
        }),
        MediaInput::AcceptedAudioExport { run_directory } => {
            Ok(FinalLosslessSubject::AcceptedAudioExport {
                run_directory: run_directory.clone(),
            })
        }
        MediaInput::AcceptedAudioWithQc { subject, .. } => Ok(subject.clone()),
        _ => Err(state(
            "audio publishing final media is not an accepted audio subject",
        )),
    }
}
/// Protect current evidence per phase. The final QC owner acquires its own
/// actual subject writer; exclude that one path during its fresh execution.
fn protect(
    root: &Path,
    intent: &AudioPublishingIntent,
    current: &AudioPublishingStatus,
    skip: Option<&Path>,
    token: &CancellationToken,
) -> Result<Vec<crate::cache_v3::ExclusiveWriter>> {
    let mut paths = std::collections::BTreeSet::new();
    paths.insert(intent.request.subject.run_directory().to_path_buf());
    if let Some(media) = &current.final_media {
        paths.insert(current_subject(media)?.run_directory().to_path_buf());
    }
    if let Some(pin) = &current.qc_receipt_sha256 {
        paths.insert(audio_qc::observe(&qc_root(root), pin, token)?.signal_run_directory);
    }
    if current.package.is_some() {
        paths.insert(package_root(root));
    }
    let mut guards = Vec::new();
    for path in paths {
        if skip != Some(path.as_path()) {
            guards.push(crate::cache_v3::ExclusiveWriter::acquire(
                &path,
                "writer.lock",
            )?);
        }
    }
    Ok(guards)
}
pub fn run<F: FnMut(PublishingPhase, &PipelineV3RunProgress)>(
    request: AudioPublishingRequest,
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<AudioPublishingStatus> {
    let initial = plan(&request, token)?;
    local::parent(root)?;
    disjoint(root, &initial.intent)?;
    local::directory(root)?;
    local::directory(&root.join("children"))?;
    local::write(
        &root.join("audio-publishing-intent.json"),
        &local::bytes(&initial.intent)?,
    )?;
    drive(root, token, progress)
}
pub fn resume<F: FnMut(PublishingPhase, &PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<AudioPublishingStatus> {
    readback::retained(root, token)?;
    drive(root, token, progress)
}
fn drive<F: FnMut(PublishingPhase, &PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    mut progress: F,
) -> Result<AudioPublishingStatus> {
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let intent = readback::retained(root, token)?;
    disjoint(root, &intent)?;
    for _ in 0..8 {
        cancelled(token)?;
        let current = status(root, token)?;
        if current.complete
            || current.steps.iter().any(|s| {
                matches!(
                    s.state,
                    PublishingStepState::AwaitingReview
                        | PublishingStepState::Rejected
                        | PublishingStepState::Blocked
                )
            })
        {
            return Ok(current);
        }
        let next = current
            .steps
            .iter()
            .find(|s| s.state != PublishingStepState::Complete)
            .ok_or_else(|| state("audio publishing has no next fixed phase"))?;
        let phase = next.phase;
        let qc_subject = if phase == PublishingPhase::FinalAudioQc {
            Some(current_subject(current.final_media.as_ref().ok_or_else(
                || state("audio QC awaits accepted final audio"),
            )?)?)
        } else {
            None
        };
        let _evidence = protect(
            root,
            &intent,
            &current,
            qc_subject.as_ref().map(FinalLosslessSubject::run_directory),
            token,
        )?;
        let stable = status(root, token)?;
        if stable != current {
            return Err(state(
                "audio publishing predecessor changed before its selected next phase",
            ));
        }
        let request = &intent.request;
        match phase {
            PublishingPhase::DestinationExport => {
                let selected = export_request(request)?;
                let path = export_root(root);
                let report = |event: &PipelineV3RunProgress| progress(phase, event);
                if exists(&path)? {
                    native_export::resume(selected, &path, token, report)?;
                } else {
                    native_export::run_at(
                        selected,
                        &path,
                        intent
                            .export_plan
                            .as_ref()
                            .ok_or_else(|| state("initial audio export native plan missing"))?,
                        token,
                        report,
                    )?;
                }
            }
            PublishingPhase::FinalAudioQc => {
                let selected = qc_request(request, qc_subject.unwrap())?;
                if version2(request) {
                    if exists(&qc_root(root))? {
                        resumable_qc::resume(&qc_root(root), token, |event| {
                            progress(phase, event)
                        })?;
                    } else {
                        resumable_qc::run(
                            &resumable_request(selected.clone()),
                            &qc_root(root),
                            token,
                            |event| progress(phase, event),
                        )?;
                    }
                } else {
                    audio_qc::run(&selected, &qc_root(root), token)?;
                }
                let binding = readback::QcBinding {
                    schema: schema(request, "qc-binding"),
                    intent_sha256: intent.intent_sha256.clone(),
                    request_sha256: canonical_sha256(&selected)?,
                    receipt_sha256: identity(
                        &qc_root(root).join("qc.json"),
                        local::DOCUMENT_BYTES,
                    )?
                    .0,
                };
                audio_qc::observe(&qc_root(root), &binding.receipt_sha256, token)?;
                cancelled(token)?;
                local::write(
                    &root.join("audio-qc-binding.json"),
                    &local::bytes(&binding)?,
                )?;
            }
            PublishingPhase::DestinationPreflight => {
                let selected = PreflightRequest {
                    schema: "aniflow.destination-preflight-request/v1".into(),
                    input: current.final_media.clone().ok_or_else(|| {
                        state(
                            "audio publishing preflight needs accepted final bytes and selected QC",
                        )
                    })?,
                    profile: request.profile.clone(),
                    evaluated_on: request.evaluated_on,
                };
                let report = destination::preflight(&selected, token)?;
                local::write(&root.join("preflight.json"), &local::bytes(&report)?)?;
            }
            PublishingPhase::ManualPackage => {
                let selected = posting_request_for(
                    &request.package,
                    current.preflight.clone().ok_or_else(|| {
                        state("audio package awaits matching exact final preflight")
                    })?,
                );
                posting::create(&selected,current.review.as_ref().ok_or_else(||state("audio package requires the separately supplied whole-file/public-metadata decision"))?,&package_root(root),token)?;
            }
            PublishingPhase::RetentionHandoff => {
                let handoff = common::retention(
                    root,
                    &request.package,
                    request
                        .retention
                        .as_ref()
                        .ok_or_else(|| state("audio retention handoff was not selected"))?,
                    token,
                )?;
                local::write(
                    &root.join("retention-handoff.json"),
                    &local::bytes(&handoff)?,
                )?;
            }
            PublishingPhase::FinalReview => {
                return Err(state(
                    "execution cannot create or bypass the supplied final audio review",
                ));
            }
        }
    }
    Err(state(
        "audio publishing exceeded its finite fixed phase count",
    ))
}
/// Retain one supplied approval/rejection. This operation never packages, seals
/// or publishes; a separate resume is required after an exact supplied approval.
pub fn record_review(
    root: &Path,
    declaration: &ManualReview,
    token: &CancellationToken,
) -> Result<AudioPublishingStatus> {
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let intent = readback::retained(root, token)?;
    let current = status(root, token)?;
    let _evidence = protect(root, &intent, &current, None, token)?;
    let current = status(root, token)?;
    let plan=current.posting_plan.as_ref().ok_or_else(||state("audio final review awaits complete native file, selected QC and matching dated preflight"))?;
    require_review(plan, declaration)?;
    if current.review.is_some() || current.package.is_some() {
        return Err(state(
            "audio publishing decision is immutable; preserve it and use a new intent for a changed decision or subject",
        ));
    }
    if current.next_action.phase != Some(PublishingPhase::FinalReview)
        || current.next_action.action != "record_review"
    {
        return Err(state(
            "audio publishing is not at its explicit final review pause",
        ));
    }
    cancelled(token)?;
    local::write(
        &root.join("posting-review.json"),
        &local::bytes(declaration)?,
    )?;
    status(root, token)
}
