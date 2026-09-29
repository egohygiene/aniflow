//! Independent bounds, clock, ordering and ambiguity checks for native notes.
use super::types::*;
use crate::audio_analysis::*;
use crate::{CapabilityReference, Result};
use sha2::{Digest, Sha256};

pub fn normalize(bytes: &[u8], source: &AudioSource) -> Result<MidiObservation> {
    bounded(bytes, AUDIO_MIDI_MAXIMUM_RAW_BYTES, "MIDI observation")?;
    let native: AudioMidiNativeObservation =
        crate::provider::decode_json(bytes, "MIDI observation")?;
    from_native(native, source)
}
impl AudioMidiNativeObservation {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, AUDIO_MIDI_MAXIMUM_RAW_BYTES, "MIDI observation")?;
        let value: Self = crate::provider::decode_json(bytes, "MIDI observation")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MIDI_OBSERVATION_SCHEMA_V1
            || self.sample_rate_hz != AUDIO_MIDI_SAMPLE_RATE
            || self.channels != 1
            || !(1..=AUDIO_MIDI_MAXIMUM_FRAMES).contains(&self.sample_frames)
            || self.duration_microseconds != self.sample_frames * 1_000_000 / 22050
            || self.notes.len() > AUDIO_MIDI_MAXIMUM_NOTES
        {
            return Err(invalid(
                "MIDI observation is outside the bounded mono 22.05 kHz profile",
            ));
        }
        let mut previous = None;
        let mut pitch_ends = [0; 128];
        let mut tick_ends = [0; 128];
        for note in &self.notes {
            if note.start_microseconds >= note.end_microseconds
                || note.end_microseconds > self.duration_microseconds
                || !(21..=108).contains(&note.pitch)
                || !note.activation.is_finite()
                || !(0.0..=1.0).contains(&note.activation)
                || !(1..=127).contains(&note.velocity)
                || f64::from(note.velocity) != (127.0 * note.activation).round_ties_even()
            {
                return Err(invalid(
                    "MIDI native note has invalid bounds, pitch, activation or velocity",
                ));
            }
            let key = (note.start_microseconds, note.pitch, note.end_microseconds);
            if previous.is_some_and(|old| old >= key) {
                return Err(invalid(
                    "MIDI notes must be strictly ordered by start, pitch and end",
                ));
            }
            previous = Some(key);
            let index = usize::from(note.pitch);
            if note.start_microseconds < pitch_ends[index] {
                return Err(invalid(
                    "overlapping same-pitch notes are ambiguous in the fixed single-channel MIDI profile",
                ));
            }
            pitch_ends[index] = note.end_microseconds;
            let start_tick = tick(note.start_microseconds)?;
            let end_tick = tick(note.end_microseconds)?;
            if start_tick >= end_tick || start_tick < tick_ends[index] {
                return Err(invalid(
                    "MIDI tick quantization collapses or ambiguously overlaps a note",
                ));
            }
            tick_ends[index] = end_tick;
        }
        Ok(())
    }
}
pub(super) fn from_native(
    native: AudioMidiNativeObservation,
    source: &AudioSource,
) -> Result<MidiObservation> {
    source.validate()?;
    native.validate()?;
    if source.artifact.byte_size > 8 * 1024 * 1024
        || source.sample_rate_hz != native.sample_rate_hz
        || source.channels != native.channels
        || source.frame_count != native.sample_frames
    {
        return Err(invalid("MIDI observation differs from its source clock"));
    }
    let notes = native
        .notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            Ok(MidiNote {
                id: format!("note_{index:06}"),
                start: rational_microseconds(note.start_microseconds),
                end: rational_microseconds(note.end_microseconds),
                start_tick: tick(note.start_microseconds)?,
                end_tick: tick(note.end_microseconds)?,
                pitch: note.pitch,
                velocity: note.velocity,
                activation: note.activation,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(MidiObservation { native, notes })
}
/// Exact integer nearest-tick conversion at 960 PPQ and 500000 µs/quarter.
/// Positive half ties round upward. Native notes have already been bounded.
pub(super) fn tick(microseconds: u64) -> Result<u32> {
    let numerator = microseconds
        .checked_mul(1920)
        .and_then(|n| n.checked_add(500000))
        .ok_or_else(|| invalid("MIDI tick arithmetic overflow"))?;
    u32::try_from(numerator / 1_000_000).map_err(|_| invalid("MIDI tick count overflow"))
}
fn rational_microseconds(microseconds: u64) -> AudioRationalTime {
    let mut a = microseconds;
    let mut b = 1_000_000;
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    AudioRationalTime {
        numerator: (microseconds / a) as i64,
        denominator: (1_000_000 / a) as u32,
    }
}

/// Preserve accepted inspection/lineage evidence and add unreviewed candidate
/// notes. The report carries exact candidate times; no extra frame rounding is
/// hidden in the provider-neutral projection.
pub fn normalized_analysis(
    mut analysis: AudioAnalysis,
    report: &AudioMidiReport,
    report_bytes: &[u8],
) -> Result<AudioAnalysis> {
    analysis.validate()?;
    report.validate()?;
    let upstream = analysis.canonical_json_bytes()?;
    if analysis.source != report.source
        || report_bytes != report.canonical_json_bytes()?
        || report.upstream_analysis_artifact.byte_size != upstream.len() as u64
        || report.upstream_analysis_artifact.sha256 != format!("{:x}", Sha256::digest(&upstream))
        || !analysis
            .artifacts
            .iter()
            .any(|a| a == &report.technical_artifact)
    {
        return Err(invalid(
            "MIDI normalized projection differs from exact upstream evidence",
        ));
    }
    if analysis
        .capabilities
        .iter()
        .any(|c| c.capability.id == AUDIO_MIDI_CAPABILITY_ID)
    {
        return Err(invalid(
            "upstream analysis already declares MIDI candidates",
        ));
    }
    for artifact in [
        report.upstream_analysis_artifact.clone(),
        AudioArtifactReference {
            id: "midi".into(),
            sha256: format!("{:x}", Sha256::digest(report_bytes)),
            byte_size: report_bytes.len() as u64,
        },
    ] {
        if analysis.artifacts.iter().any(|a| a.id == artifact.id) {
            return Err(invalid(
                "MIDI artifact identity collides with upstream evidence",
            ));
        }
        analysis.artifacts.push(artifact);
    }
    let provider_id = "audio-midi-provider";
    let license = AudioLicenseEvidence::Recorded { statement: "The MIDI report records upstream Basic Pitch Apache-2.0 and ONNX Runtime MIT package declarations; pins do not authenticate origins or independently qualify every runtime dependency.".into(), evidence_artifact_id: "midi".into() };
    analysis.providers.push(AudioProviderEvidence {
        id: provider_id.into(),
        provider: report.provider.clone(),
        implementation_sha256: report.implementation_sha256.clone(),
        configuration_sha256: report.configuration_sha256.clone(),
        tools: vec![AudioComponentEvidence {
            id: "basic-pitch-runtime".into(),
            version: AUDIO_MIDI_BASIC_PITCH_VERSION.into(),
            revision: AUDIO_MIDI_BASIC_PITCH_REVISION.into(),
            sha256: report.settings.runtime.sha256.clone(),
            license: license.clone(),
        }],
        models: AudioModelEvidence::Available {
            components: vec![AudioComponentEvidence {
                id: report.settings.model.model_id.clone(),
                version: AUDIO_MIDI_BASIC_PITCH_VERSION.into(),
                revision: report.settings.model.revision.clone(),
                sha256: report.settings.model.sha256.clone(),
                license: license.clone(),
            }],
        },
        license,
    });
    let evidence = vec![
        "source_audio".into(),
        "technical".into(),
        report.upstream_analysis_artifact.id.clone(),
        "midi".into(),
    ];
    let has_notes = matches!(&report.result, MidiResult::Candidate { observation } if !observation.notes.is_empty());
    let mut diagnostic_ids = Vec::new();
    if has_notes {
        analysis.semantic_artifacts.push(AudioSemanticArtifact {
            artifact_id: "midi".into(),
            capability_id: AUDIO_MIDI_CAPABILITY_ID.into(),
            kind: AudioSemanticArtifactKind::CandidateMidi,
            provenance: AudioObservationProvenance {
                class: AudioProvenanceClass::Probabilistic,
                confidence: report.confidence.clone(),
                provider_evidence_id: provider_id.into(),
                evidence_artifact_ids: evidence.clone(),
            },
            authority: None,
        });
    }
    if !has_notes {
        let id = "midi-unavailable".to_owned();
        analysis.diagnostics.push(AudioDiagnostic { id: id.clone(), code: id.clone(), severity: AudioDiagnosticSeverity::Warning, message: if matches!(report.result, MidiResult::Candidate { .. }) { "The analyzer returned no note candidates; this does not establish silence or absence of pitched music." } else { "The bounded MIDI profile did not invoke note inference; inspect the explicit reason in its companion." }.into() });
        diagnostic_ids.push(id);
        analysis.status = AudioAnalysisStatus::Partial;
    }
    analysis.capabilities.push(AudioCapabilityOutcome {
        capability: CapabilityReference {
            id: AUDIO_MIDI_CAPABILITY_ID.into(),
            version: "1.0.0".into(),
        },
        status: if has_notes {
            AudioAnalysisStatus::Complete
        } else {
            AudioAnalysisStatus::Unavailable
        },
        provider_evidence_ids: vec![provider_id.into()],
        evidence_artifact_ids: evidence,
        diagnostic_ids,
    });
    analysis.validate()?;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    pub(super) fn source() -> AudioSource {
        AudioSource {
            artifact: AudioArtifactReference {
                id: "source_audio".into(),
                sha256: "a".repeat(64),
                byte_size: 88244,
            },
            stream_index: 0,
            sample_rate_hz: 22050,
            channels: 1,
            frame_count: 44100,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        }
    }
    fn raw() -> serde_json::Value {
        json!({"schema":AUDIO_MIDI_OBSERVATION_SCHEMA_V1,"sample_rate_hz":22050,"channels":1,"sample_frames":44100,"duration_microseconds":2000000,"notes":[{"start_microseconds":0,"end_microseconds":1000000,"pitch":60,"velocity":64,"activation":0.5},{"start_microseconds":500000,"end_microseconds":1500000,"pitch":64,"velocity":127,"activation":1.0}]})
    }
    fn parse(value: &serde_json::Value) -> Result<MidiObservation> {
        normalize(&serde_json::to_vec(value).unwrap(), &source())
    }
    #[test]
    fn polyphonic_candidates_keep_exact_microsecond_clock_and_native_activation() {
        let result = parse(&raw()).unwrap();
        assert_eq!(
            result.notes[1].start,
            AudioRationalTime {
                numerator: 1,
                denominator: 2
            }
        );
        assert_eq!(result.notes[1].end_tick, 2880);
        assert_eq!(result.notes[0].activation, 0.5);
        result.validate(&source()).unwrap();
    }
    #[test]
    fn rejects_unknown_duplicate_out_of_range_and_invalid_velocity_fields() {
        for (pointer, value) in [
            ("/notes/0/pitch", json!(20)),
            ("/notes/0/velocity", json!(63)),
            ("/notes/0/activation", json!(1.1)),
            ("/notes/0/end_microseconds", json!(2000001)),
            ("/notes/0/end_microseconds", json!(0)),
            ("/sample_frames", json!(44101)),
            ("/channels", json!(2)),
        ] {
            let mut changed = raw();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(parse(&changed).is_err(), "{pointer}");
        }
        let mut value = raw();
        value["notes"][0]["confidence"] = json!(0.99);
        assert!(parse(&value).is_err());
        let duplicate = serde_json::to_string(&raw())
            .unwrap()
            .replace("\"pitch\":60", "\"pitch\":60,\"pitch\":60");
        assert!(normalize(duplicate.as_bytes(), &source()).is_err());
    }
    #[test]
    fn refuses_same_pitch_overlap_reordering_and_tick_collapse() {
        let mut overlap = raw();
        overlap["notes"][1]["pitch"] = json!(60);
        assert!(parse(&overlap).is_err());
        let mut unordered = raw();
        unordered["notes"].as_array_mut().unwrap().reverse();
        assert!(parse(&unordered).is_err());
        let mut collapsed = raw();
        collapsed["notes"][0]["end_microseconds"] = json!(1);
        assert!(parse(&collapsed).is_err());
    }
    #[test]
    fn forged_normalized_note_or_source_is_refused() {
        let mut result = parse(&raw()).unwrap();
        result.notes[0].end_tick += 1;
        assert!(result.validate(&source()).is_err());
        let mut wrong = source();
        wrong.frame_count += 1;
        assert!(normalize(&serde_json::to_vec(&raw()).unwrap(), &wrong).is_err());
    }
    #[test]
    fn empty_observation_is_not_fabricated_as_a_note() {
        let mut value = raw();
        value["notes"] = json!([]);
        assert!(parse(&value).unwrap().notes.is_empty());
    }
    #[test]
    fn quantization_is_exact_and_half_ties_round_up() {
        assert_eq!(tick(15625).unwrap(), 30);
        assert_eq!(tick(7812).unwrap(), 15);
        assert_eq!(tick(7813).unwrap(), 15);
        for us in 0..=1_000_000 {
            let ticks = tick(us).unwrap();
            let error = (i64::from(ticks) * 1_000_000 - us as i64 * 1920).abs();
            assert!(error <= 500000);
        }
        assert!(tick(u64::MAX).is_err());
    }
    #[test]
    fn collection_bound_and_nonfinite_native_amplitude_are_checked() {
        let mut native = parse(&raw()).unwrap().native;
        native.notes[0].activation = f64::NAN;
        assert!(native.validate().is_err());
        native.notes[0].activation = 0.5;
        native.notes = vec![native.notes[0].clone(); AUDIO_MIDI_MAXIMUM_NOTES + 1];
        assert!(native.validate().is_err());
    }
}
