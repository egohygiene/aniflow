use super::*;
use crate::frame_io::identity;
pub(crate) fn observe(input: &MediaInput, token: &CancellationToken) -> Result<MediaObservation> {
    cancelled(token)?;
    match input {
        MediaInput::AcceptedGainCandidate { run_directory } => {
            let completion = crate::delivery_audio::gain::observe(run_directory, token)?;
            Ok(MediaObservation {
                input: input.clone(),
                file: completion.file.clone(),
                facts: MediaFacts::GainCandidate {
                    completion: Box::new(completion),
                },
            })
        }
        MediaInput::AcceptedAudioWithQc {
            subject,
            qc_directory,
            qc_receipt_sha256,
        } => {
            let qc = crate::delivery_audio::final_lossless_qc::observe(
                qc_directory,
                qc_receipt_sha256,
                token,
            )?;
            if &qc.intent.request.subject != subject {
                return Err(invalid(
                    "final audio QC must bind the exact explicitly selected native subject",
                ));
            }
            let current = observe(&subject.input(), token)?;
            if current != qc.intent.source {
                return Err(invalid(
                    "final audio QC differs from current final audio bytes/acceptance",
                ));
            }
            Ok(MediaObservation {
                input: input.clone(),
                file: current.file,
                facts: MediaFacts::AudioWithQc { qc: Box::new(qc) },
            })
        }
        MediaInput::AcceptedVideoWithQc {
            run_directory,
            artifact_id,
            qc_directory,
            qc_receipt_sha256,
        } => {
            let qc =
                crate::delivery_audio::final_qc::observe(qc_directory, qc_receipt_sha256, token)?;
            if &qc.intent.request.source_run_directory != run_directory
                || &qc.intent.request.artifact_id != artifact_id
            {
                return Err(invalid(
                    "final-file QC must bind the exact selected producer/artifact",
                ));
            }
            let completion = crate::final_video::observe(run_directory)?;
            let artifact = video(&completion, artifact_id)?;
            if completion != qc.intent.completion || *artifact != qc.intent.artifact {
                return Err(invalid(
                    "final-file QC refers to different current video bytes or acceptance",
                ));
            }
            let file = MediaFile {
                locator: artifact.locator.clone(),
                sha256: artifact.sha256.clone(),
                byte_count: artifact.byte_count,
            };
            cancelled(token)?;
            Ok(MediaObservation {
                input: input.clone(),
                file,
                facts: MediaFacts::VideoWithQc {
                    completion,
                    artifact_id: artifact_id.clone(),
                    qc: Box::new(qc),
                },
            })
        }
        MediaInput::AcceptedAudioExport { run_directory } => {
            let completion = crate::lossless_audio::observe(run_directory, token)?;
            Ok(MediaObservation {
                input: input.clone(),
                file: completion.file,
                facts: MediaFacts::LosslessAudio {
                    report: Box::new(completion.report),
                },
            })
        }
        MediaInput::AcceptedVideo {
            run_directory,
            artifact_id,
        } => {
            if !identifier(artifact_id) {
                return Err(invalid(
                    "video artifact selection requires a bounded exact ID",
                ));
            }
            let completion = crate::final_video::observe(run_directory)?;
            let artifact = video(&completion, artifact_id)?;
            let file = MediaFile {
                locator: artifact.locator.clone(),
                sha256: artifact.sha256.clone(),
                byte_count: artifact.byte_count,
            };
            if identity(&file.locator, 1_099_511_627_776)? != (file.sha256.clone(), file.byte_count)
            {
                return Err(invalid(
                    "selected accepted video changed during destination observation",
                ));
            }
            cancelled(token)?;
            Ok(MediaObservation {
                input: input.clone(),
                file,
                facts: MediaFacts::Video {
                    completion,
                    artifact_id: artifact_id.clone(),
                },
            })
        }
        MediaInput::AcceptedPcm {
            file,
            signal_run_directory,
        } => {
            let before = crate::status_v3(signal_run_directory)?;
            let summary = crate::delivery_audio::loudness_summary(signal_run_directory)?;
            summary.source.validate()?;
            let expected = (
                summary.source.artifact.sha256.clone(),
                summary.source.artifact.byte_size,
            );
            if summary.source.stem.is_some()
                || identity(
                    file,
                    crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,
                )? != expected
            {
                return Err(invalid(
                    "PCM destination file must match its exact accepted original signal subject",
                ));
            }
            let wave = crate::audio_inspection::wav::inspect_native(file)?;
            if wave.frame_count != summary.source.frame_count
                || wave.channels != summary.source.channels
                || wave.sample_rate_hz != summary.source.sample_rate_hz
                || identity(
                    file,
                    crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,
                )? != expected
                || crate::status_v3(signal_run_directory)? != before
            {
                return Err(invalid(
                    "PCM bytes, clock or signal acceptance changed during observation",
                ));
            }
            let encoding = match wave.sample_format {
                crate::audio_inspection::wav::NativeSampleFormat::Pcm16 => PcmEncoding::Pcm16,
                crate::audio_inspection::wav::NativeSampleFormat::Pcm24 => PcmEncoding::Pcm24,
                crate::audio_inspection::wav::NativeSampleFormat::Float32 => PcmEncoding::Float32,
            };
            cancelled(token)?;
            Ok(MediaObservation {
                input: input.clone(),
                file: MediaFile {
                    locator: file.clone(),
                    sha256: expected.0,
                    byte_count: expected.1,
                },
                facts: MediaFacts::Pcm {
                    summary,
                    encoding,
                    pcm_sha256: wave.pcm_sha256,
                    run_manifest_sha256: before.manifest_sha256,
                },
            })
        }
    }
}
