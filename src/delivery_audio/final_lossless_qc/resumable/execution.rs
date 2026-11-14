use super::*;
pub fn run<F: FnMut(&crate::PipelineV3RunProgress)>(
    request: &ResumableQcRequest,
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<ResumableQcStatus> {
    let intent = plan(request, token)?;
    let parent = local::parent(root)?;
    runtime::subject_disjoint(root, &intent.native)?;
    crate::video_assembly::require_storage(
        parent,
        intent.projected_working_bytes,
        request.qc.minimum_free_bytes,
    )?;
    local::directory(root)?;
    local::directory(&root.join("children"))?;
    local::write(&root.join("resume-intent.json"), &local::bytes(&intent)?)?;
    drive(root, token, progress)
}
pub fn resume<F: FnMut(&crate::PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    progress: F,
) -> Result<ResumableQcStatus> {
    // No caller-supplied replacement request. Unrecognized/legacy partial roots
    // fail retained readback; native partial initialization stays retained.
    retained(root, token)?;
    drive(root, token, progress)
}
fn drive<F: FnMut(&crate::PipelineV3RunProgress)>(
    root: &Path,
    token: &CancellationToken,
    mut progress: F,
) -> Result<ResumableQcStatus> {
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    let intent = retained(root, token)?;
    if status(root, token)?.complete {
        return status(root, token);
    }
    let _source = crate::cache_v3::ExclusiveWriter::acquire(
        intent.request.qc.subject.run_directory(),
        "writer.lock",
    )?;
    if retained(root, token)? != intent {
        return Err(invalid("resumable QC source changed before execution"));
    }
    for _ in 0..6 {
        cancelled(token)?;
        let current = status(root, token)?;
        if current.complete {
            return Ok(current);
        }
        crate::video_assembly::require_storage(
            root,
            intent.projected_working_bytes,
            intent.request.qc.minimum_free_bytes,
        )?;
        match current.phase {
            ResumableQcPhase::Capture => {
                copy_pinned(
                    &intent.native.source.file.locator,
                    &root.join("source.media"),
                    &(
                        intent.native.source.file.sha256.clone(),
                        intent.native.source.file.byte_count,
                    ),
                    crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,
                    token,
                )?;
                for (component, name) in
                    intent
                        .native
                        .components
                        .iter()
                        .zip(["ffmpeg", "ffprobe", "aniflow-provider"])
                {
                    let destination = root.join(name);
                    copy_pinned(
                        &component.path,
                        &destination,
                        &(component.sha256.clone(), component.byte_count),
                        134_217_728,
                        token,
                    )?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(
                            &destination,
                            std::fs::Permissions::from_mode(0o700),
                        )
                        .map_err(|_| invalid("cannot enable private resumable-QC component"))?;
                    }
                }
                capture_files(root, &intent)?;
                if retained(root, token)? != intent {
                    return Err(invalid("resumable QC source changed during capture"));
                }
                cancelled(token)?;
                local::write(
                    &root.join("capture.json"),
                    &local::bytes(&capture_marker(&intent))?,
                )?;
            }
            ResumableQcPhase::Decode => {
                let native = &intent.native;
                let output = root.join("decoded.wav");
                if flac(native) {
                    let MediaFacts::LosslessAudio { report } = &native.source.facts else {
                        return Err(invalid("resumable QC FLAC report missing"));
                    };
                    crate::lossless_audio::decode_for_qc(
                        report,
                        &root.join("source.media"),
                        &output,
                        root,
                        &private_configuration(root, native),
                        native.request.maximum_pcm_bytes,
                        token,
                    )?;
                } else {
                    copy_pinned(
                        &root.join("source.media"),
                        &output,
                        &(
                            native.source.file.sha256.clone(),
                            native.source.file.byte_count,
                        ),
                        native.request.maximum_pcm_bytes,
                        token,
                    )?;
                }
                let pin = wave(root, native)?;
                let decoded = Decoded {
                    schema: "aniflow.resumable-final-audio-qc-decoded/v1".into(),
                    intent_sha256: intent.intent_sha256.clone(),
                    wav_sha256: pin.0,
                    wav_bytes: pin.1,
                    plan: crate::audio_signal::plan_read_only(
                        &signal_request(root, native),
                        token,
                    )?,
                };
                runtime::recheck_files(root, native)?;
                if retained(root, token)? != intent {
                    return Err(invalid(
                        "resumable QC source changed during complete decode",
                    ));
                }
                cancelled(token)?;
                local::write(&root.join("decoded.json"), &local::bytes(&decoded)?)?;
            }
            ResumableQcPhase::NativeSignal => {
                let decoded: Decoded = local::read(&root.join("decoded.json"))?;
                let selected = signal_request(root, &intent.native);
                let path = child(root);
                if present(&path)? {
                    crate::audio_signal::resume(selected, &path, token, &mut progress)
                        .map_err(|e| e.error)?;
                } else {
                    crate::audio_signal::run_at(
                        selected,
                        &path,
                        &decoded.plan,
                        token,
                        &mut progress,
                    )?;
                }
            }
            ResumableQcPhase::Meter => {
                // The native writer is free during analysis execution/resume,
                // then held while recording/readback of its meter observation.
                let _child =
                    crate::cache_v3::ExclusiveWriter::acquire(&child(root), "writer.lock")?;
                if status(root, token)? != current {
                    return Err(invalid("native signal changed before final-QC meter"));
                }
                let decoded: Decoded = local::read(&root.join("decoded.json"))?;
                if !present(&root.join("meter.json"))? {
                    let report = measure(root, &intent, token)?;
                    accepted_report(root, &intent, &decoded, &report, token)?;
                    let meter = Meter {
                        schema: "aniflow.resumable-final-audio-qc-meter/v1".into(),
                        intent_sha256: intent.intent_sha256.clone(),
                        report,
                    };
                    if retained(root, token)? != intent {
                        return Err(invalid("resumable QC source changed during meter"));
                    }
                    cancelled(token)?;
                    local::write(&root.join("meter.json"), &local::bytes(&meter)?)?;
                }
                let saved: Meter = local::read(&root.join("meter.json"))?;
                accepted_report(root, &intent, &decoded, &saved.report, token)?;
                if retained(root, token)? != intent {
                    return Err(invalid("resumable QC source changed before final receipt"));
                }
                cancelled(token)?;
                local::write(&root.join("qc.json"), &local::bytes(&saved.report)?)?;
            }
            ResumableQcPhase::Complete => return Ok(current),
        }
    }
    Err(invalid("resumable QC exceeded its finite phase count"))
}
fn measure(
    root: &Path,
    intent: &ResumableQcIntent,
    token: &CancellationToken,
) -> Result<FinalLosslessQcReport> {
    let native = &intent.native;
    let accepted = crate::status_v3(&child(root))?;
    let summary = super::super::super::loudness_summary(&child(root))?;
    let pin = wave(root, native)?;
    runtime::bind_summary(native, &pin, &summary)?;
    let (integrated_loudness, true_peak, meter) = if let Some(schema) = meter_schema(
        &native.request,
        summary.source.sample_rate_hz,
        summary.source.channels,
    ) {
        let configuration = private_configuration(root, native);
        let pcm = root.join("decoded.wav");
        let receipt = if schema == "aniflow.native-pcm-loudness/v3" {
            crate::audio_signal::loudness::measure_native_44100(
                &pcm,
                &pin,
                &configuration,
                root,
                token,
            )?
        } else {
            crate::audio_signal::loudness::measure_native(&pcm, &pin, &configuration, root, token)?
        };
        (
            receipt.integrated_loudness.clone(),
            receipt.true_peak.clone(),
            FinalLosslessMeter::Observed {
                receipt: Box::new(receipt),
            },
        )
    } else {
        unavailable(summary.source.channels)
    };
    let (_, encoding, pcm) = source_facts(&native.source)?;
    Ok(FinalLosslessQcReport {
        schema: "aniflow.final-lossless-audio-qc/v2".into(),
        intent: native.clone(),
        intent_sha256: crate::provider::canonical_sha256(native)?,
        decoded_wav_sha256: pin.0,
        decoded_wav_bytes: pin.1,
        decoded_pcm_sha256: pcm.into(),
        encoding,
        signal_run_directory: child(root),
        signal_plan_sha256: accepted.payload.plan_sha256,
        signal_manifest_sha256: accepted.manifest_sha256,
        accepted_pcm: summary,
        integrated_loudness,
        true_peak,
        meter,
        normalization: "none".into(),
        runtime_qualified: false,
        creative_approval: "not_performed; final-file measurements are not musical quality review"
            .into(),
        publication_authority: "none".into(),
    })
}
