//! Strict native admission and conservative mapping to exact reviewed cue text.
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::types::*;
use crate::audio_analysis::*;
use crate::timed_text::{CueTiming, TextProvenance, TimedTextDocument};
use crate::{CapabilityReference, Result};

const TIMING_METADATA: &str = "aniflow_alignment_timing";
const FILLERS: [&str; 6] = ["<s>", "</s>", "<sil>", "[NOISE]", "[SPEECH]", "(NULL)"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeWord {
    b: serde_json::Number,
    d: serde_json::Number,
    p: serde_json::Number,
    t: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeObservation {
    b: serde_json::Number,
    d: serde_json::Number,
    p: serde_json::Number,
    t: String,
    w: Vec<NativeWord>,
}

/// Tokenize without altering authored cue text. ASCII punctuation separates words;
/// apostrophes are retained only between letters, and unsupported lexemes fail.
pub fn tokenize(reviewed: &ReviewedLyrics) -> Result<Vec<AlignmentToken>> {
    reviewed.validate()?;
    tokenize_document(&reviewed.document)
}
pub(super) fn tokenize_document(document: &TimedTextDocument) -> Result<Vec<AlignmentToken>> {
    if document
        .metadata
        .get(TIMING_METADATA)
        .is_some_and(|value| value != "candidate")
        || (!document.metadata.contains_key(TIMING_METADATA) && document.metadata.len() == 32)
    {
        return Err(invalid(
            "reviewed metadata has no unambiguous space for the candidate timing marker",
        ));
    }
    let mut result = Vec::new();
    let mut phrase_bytes = 0usize;
    for cue in &document.cues {
        let bytes = cue.text.as_bytes();
        if !cue.text.is_ascii() || bytes.iter().any(|b| b.is_ascii_digit()) {
            return Err(invalid(
                "alignment supports ASCII English words only; digits and non-ASCII lexemes require an explicit reviewed revision",
            ));
        }
        let mut index = 0;
        while index < bytes.len() {
            if !bytes[index].is_ascii_alphabetic() {
                index += 1;
                continue;
            }
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphabetic()
                    || (bytes[index] == b'\''
                        && index + 1 < bytes.len()
                        && bytes[index - 1].is_ascii_alphabetic()
                        && bytes[index + 1].is_ascii_alphabetic()))
            {
                index += 1;
            }
            let text = &cue.text[start..index];
            phrase_bytes += text.len() + usize::from(!result.is_empty());
            if result.len() == AUDIO_ALIGNMENT_MAXIMUM_TOKENS
                || phrase_bytes > AUDIO_ALIGNMENT_MAXIMUM_PHRASE_BYTES
            {
                return Err(invalid(
                    "reviewed lyrics exceed 512 tokens or the 16 KiB normalized phrase bound",
                ));
            }
            result.push(AlignmentToken {
                cue_id: cue.id.clone(),
                byte_start: start as u32,
                byte_end: index as u32,
                text: text.into(),
                normalized: text.to_ascii_lowercase(),
            });
        }
    }
    if result.is_empty() {
        return Err(invalid(
            "reviewed lyrics require at least one supported English word",
        ));
    }
    Ok(result)
}
/// The lowercased phrase is provider input, never a replacement for reviewed text.
pub fn alignment_phrase(reviewed: &ReviewedLyrics) -> Result<String> {
    Ok(tokenize(reviewed)?
        .into_iter()
        .map(|token| token.normalized)
        .collect::<Vec<_>>()
        .join(" "))
}

/// Admit the pinned basic JSON dialect. Native `d` is a duration and is added to
/// `b` with integer milliseconds. Native scores are validated but never confidence.
pub fn normalize(
    raw: &[u8],
    source: &AudioSource,
    reviewed: &ReviewedLyrics,
) -> Result<AlignmentObservation> {
    bounded(
        raw,
        AUDIO_ALIGNMENT_MAXIMUM_RAW_BYTES,
        "native alignment JSON",
    )?;
    validate_native_numbers(raw)?;
    let native: NativeObservation =
        crate::provider::decode_json(raw, "native alignment observation")?;
    if native.w.len() > AUDIO_ALIGNMENT_MAXIMUM_TOKENS * 3 + 2
        || native.t.len() > AUDIO_ALIGNMENT_MAXIMUM_PHRASE_BYTES
        || decimal_milliseconds(&native.b)? != 0
        || native.p.as_f64() != Some(1.0)
    {
        return Err(invalid(
            "native alignment header differs from the pinned bounded forced-graph profile",
        ));
    }
    source.validate()?;
    let header_end = decimal_milliseconds(&native.d)?;
    check_source_end(header_end, source)?;
    let mut previous_end = 0;
    let mut words = Vec::new();
    for word in native.w {
        let start = decimal_milliseconds(&word.b)?;
        let duration = decimal_milliseconds(&word.d)?;
        let end = start
            .checked_add(duration)
            .ok_or_else(|| invalid("native alignment timing overflow"))?;
        if duration == 0
            || start < previous_end
            || end > header_end
            || !word
                .p
                .as_f64()
                .is_some_and(|p| p.is_finite() && (0.0..=1.0).contains(&p))
        {
            return Err(invalid(
                "native alignment requires ordered nonempty intervals within its header and bounded scores",
            ));
        }
        check_source_end(end, source)?;
        previous_end = end;
        if FILLERS.contains(&word.t.as_str()) {
            continue;
        }
        if !native_lexeme(&word.t) {
            return Err(invalid(
                "native alignment emitted an unsupported lexeme or pronunciation suffix",
            ));
        }
        words.push(AlignmentNativeWord {
            text: word.t,
            start: rational_milliseconds(start),
            end: rational_milliseconds(end),
        });
    }
    if native.t
        != words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    {
        return Err(invalid(
            "native alignment hypothesis disagrees with its non-filler word sequence",
        ));
    }
    map_words(words, source, reviewed)
}

pub(super) fn map_words(
    native_words: Vec<AlignmentNativeWord>,
    source: &AudioSource,
    reviewed: &ReviewedLyrics,
) -> Result<AlignmentObservation> {
    reviewed.validate_source(source)?;
    if source.sample_rate_hz != 16000
        || source.channels != 1
        || native_words.len() > AUDIO_ALIGNMENT_MAXIMUM_TOKENS
    {
        return Err(invalid(
            "alignment candidates require bounded mono 16 kHz audio and words",
        ));
    }
    let tokens = tokenize_document(&reviewed.document)?;
    let mut previous_end = 0;
    for word in &native_words {
        let start = milliseconds(word.start)?;
        let end = milliseconds(word.end)?;
        if !native_lexeme(&word.text) || start >= end || start < previous_end {
            return Err(invalid(
                "normalized native words must be ordered nonempty supported lexemes",
            ));
        }
        check_source_end(end, source)?;
        previous_end = end;
    }
    // Earliest and latest embeddings bound every monotonic subsequence match.
    // A word is timed only if exactly one authored occurrence can own it across
    // all complete embeddings. Repeated missing occurrences stay ambiguous.
    let mut earliest = Vec::new();
    let mut cursor = 0;
    for native in &native_words {
        while cursor < tokens.len() && tokens[cursor].normalized != native.text {
            cursor += 1;
        }
        if cursor == tokens.len() {
            return Err(invalid(
                "native alignment contains extra, contradictory, or out-of-order words",
            ));
        }
        earliest.push(cursor);
        cursor += 1;
    }
    let mut latest = vec![0; native_words.len()];
    cursor = tokens.len();
    for (index, native) in native_words.iter().enumerate().rev() {
        loop {
            if cursor == 0 {
                return Err(invalid(
                    "native alignment cannot preserve authored token order",
                ));
            }
            cursor -= 1;
            if tokens[cursor].normalized == native.text {
                break;
            }
        }
        latest[index] = cursor;
    }
    let mut timings = vec![AlignmentWordTiming::Unmatched {}; tokens.len()];
    for (native_index, native) in native_words.iter().enumerate() {
        let first = if native_index == 0 {
            0
        } else {
            earliest[native_index - 1] + 1
        };
        let last = if native_index + 1 == native_words.len() {
            tokens.len()
        } else {
            latest[native_index + 1]
        };
        let positions: Vec<usize> = (first..last)
            .filter(|&index| tokens[index].normalized == native.text)
            .collect();
        if positions.len() == 1 {
            timings[positions[0]] = AlignmentWordTiming::Candidate {
                start: native.start,
                end: native.end,
            };
        } else {
            for position in positions {
                timings[position] = AlignmentWordTiming::Ambiguous {};
            }
        }
    }
    let words: Vec<AlignedWord> = tokens
        .into_iter()
        .zip(timings)
        .map(|(token, timing)| AlignedWord { token, timing })
        .collect();
    let cues = reviewed
        .document
        .cues
        .iter()
        .map(|cue| {
            let matching: Vec<&AlignedWord> = words
                .iter()
                .filter(|word| word.token.cue_id == cue.id)
                .collect();
            let timing = if matching
                .iter()
                .any(|word| matches!(word.timing, AlignmentWordTiming::Ambiguous {}))
            {
                AlignmentCueTiming::Ambiguous {}
            } else if !matching.is_empty()
                && matching
                    .iter()
                    .all(|word| matches!(word.timing, AlignmentWordTiming::Candidate { .. }))
            {
                let AlignmentWordTiming::Candidate { start, .. } = matching[0].timing else {
                    unreachable!()
                };
                let AlignmentWordTiming::Candidate { end, .. } =
                    matching[matching.len() - 1].timing
                else {
                    unreachable!()
                };
                AlignmentCueTiming::Candidate { start, end }
            } else if matching
                .iter()
                .any(|word| matches!(word.timing, AlignmentWordTiming::Candidate { .. }))
            {
                AlignmentCueTiming::Partial {}
            } else {
                AlignmentCueTiming::Unmatched {}
            };
            AlignedCue {
                cue_id: cue.id.clone(),
                timing,
            }
        })
        .collect();
    Ok(AlignmentObservation {
        native_words,
        words,
        cues,
    })
}

/// Preserve authored text, cue metadata, source digest, and supplied review history.
/// Review applies to the text; proposed timing remains explicitly marked candidate.
pub fn timed_text(
    observation: &AlignmentObservation,
    source: &AudioSource,
    reviewed: &ReviewedLyrics,
) -> Result<TimedTextDocument> {
    observation.validate(source, reviewed)?;
    let mut document = reviewed.document.clone();
    document.audio_source = Some(source.clone());
    document
        .metadata
        .insert(TIMING_METADATA.into(), "candidate".into());
    for (cue, aligned) in document.cues.iter_mut().zip(&observation.cues) {
        cue.timing = match aligned.timing {
            AlignmentCueTiming::Candidate { start, end } => CueTiming::Interval { start, end },
            _ => CueTiming::Untimed {},
        };
    }
    document.validate()?;
    Ok(document)
}

/// Attach the original reviewed-lyrics artifact and distinct candidate timing
/// report. Supplied text review authority is never granted to the new timings.
pub fn normalized_analysis(
    mut analysis: AudioAnalysis,
    report: &AudioAlignmentReport,
    report_bytes: &[u8],
) -> Result<AudioAnalysis> {
    analysis.validate()?;
    report.validate()?;
    let upstream_bytes = analysis.canonical_json_bytes()?;
    if analysis.source != report.source
        || report_bytes != report.canonical_json_bytes()?
        || report.upstream_analysis_artifact.byte_size != upstream_bytes.len() as u64
        || report.upstream_analysis_artifact.sha256
            != format!("{:x}", Sha256::digest(&upstream_bytes))
        || !analysis
            .artifacts
            .iter()
            .any(|artifact| artifact == &report.technical_artifact)
    {
        return Err(invalid(
            "alignment source or companion bytes differ from the accepted upstream evidence",
        ));
    }
    if analysis
        .capabilities
        .iter()
        .any(|capability| capability.capability.id == AUDIO_ALIGNMENT_CAPABILITY_ID)
    {
        return Err(invalid("upstream analysis already declares alignment"));
    }
    let TextProvenance::ReviewedLyrics {
        authority,
        evidence: review_evidence,
        ..
    } = &report.reviewed_lyrics.document.provenance
    else {
        return Err(invalid("alignment requires supplied reviewed lyrics"));
    };
    let report_artifact = AudioArtifactReference {
        id: "alignment".into(),
        sha256: format!("{:x}", Sha256::digest(report_bytes)),
        byte_size: report_bytes.len() as u64,
    };
    for artifact in [
        report.upstream_analysis_artifact.clone(),
        report.reviewed_lyrics.artifact.clone(),
        report.reviewed_lyrics.document.source.clone(),
        review_evidence.clone(),
        report_artifact,
    ] {
        if analysis
            .artifacts
            .iter()
            .any(|existing| existing.id == artifact.id)
        {
            return Err(invalid(
                "alignment artifact identity collides with upstream or supplied review evidence",
            ));
        }
        analysis.artifacts.push(artifact);
    }
    let provider_id = "audio-alignment-provider";
    let license = AudioLicenseEvidence::Recorded { statement: "The companion records upstream tool, acoustic model, and dictionary notices. Pins do not authenticate their origin or independently verify license compliance.".into(), evidence_artifact_id: "alignment".into() };
    analysis.providers.push(AudioProviderEvidence {
        id: provider_id.into(), provider: report.provider.clone(), implementation_sha256: report.implementation_sha256.clone(), configuration_sha256: report.configuration_sha256.clone(),
        tools: vec![AudioComponentEvidence { id: "pocketsphinx".into(), version: AUDIO_ALIGNMENT_POCKETSPHINX_VERSION.into(), revision: AUDIO_ALIGNMENT_POCKETSPHINX_REVISION.into(), sha256: report.settings.pocketsphinx_sha256.clone(), license: license.clone() }],
        models: AudioModelEvidence::Available { components: vec![
            AudioComponentEvidence { id: "pocketsphinx-en-us".into(), version: report.settings.model.revision.clone(), revision: report.settings.model.revision.clone(), sha256: crate::provider::canonical_sha256(&report.settings.model.files)?, license: license.clone() },
            AudioComponentEvidence { id: "pocketsphinx-dictionary".into(), version: report.settings.model.revision.clone(), revision: report.settings.model.revision.clone(), sha256: report.settings.dictionary.sha256.clone(), license },
        ] }, license: AudioLicenseEvidence::Unavailable { reason: "Native adapter license evidence is not independently collected by this observation.".into() },
    });
    let evidence = vec![
        "source_audio".into(),
        "technical".into(),
        report.upstream_analysis_artifact.id.clone(),
        "reviewed_lyrics".into(),
        report.reviewed_lyrics.document.source.id.clone(),
        review_evidence.id.clone(),
        "alignment".into(),
    ];
    let provenance = AudioObservationProvenance {
        class: AudioProvenanceClass::Probabilistic,
        confidence: report.confidence.clone(),
        provider_evidence_id: provider_id.into(),
        evidence_artifact_ids: evidence.clone(),
    };
    let mut diagnostic_ids = Vec::new();
    let status = match &report.result {
        AlignmentResult::Candidate { observation } => {
            // This reference is the unchanged input document, not the alignment report.
            analysis.semantic_artifacts.push(AudioSemanticArtifact {
                artifact_id: "reviewed_lyrics".into(),
                capability_id: AUDIO_ALIGNMENT_CAPABILITY_ID.into(),
                kind: AudioSemanticArtifactKind::ReviewedLyrics,
                provenance: provenance.clone(),
                authority: Some(authority.clone()),
            });
            let mut events = Vec::new();
            for (index, word) in observation.words.iter().enumerate() {
                if let AlignmentWordTiming::Candidate { start, end } = word.timing {
                    events.push(AudioEvent {
                        id: format!("alignment_word_{:06}", index + 1),
                        range: AudioFrameRange {
                            start: report.source.frame_for_time(start)?,
                            end: report.source.frame_for_time(end)?,
                        },
                        label: "proposed reviewed-lyric word timing".into(),
                        provenance: provenance.clone(),
                    });
                }
            }
            if !events.is_empty() {
                analysis.timelines.push(AudioTimeline {
                    id: "alignment-word-candidates".into(),
                    capability_id: AUDIO_ALIGNMENT_CAPABILITY_ID.into(),
                    kind: AudioTimelineKind::DisjointRegions,
                    scope: report.scope.clone(),
                    events,
                });
            }
            if observation
                .cues
                .iter()
                .all(|cue| matches!(cue.timing, AlignmentCueTiming::Candidate { .. }))
            {
                AudioAnalysisStatus::Complete
            } else {
                add_diagnostic(
                    &mut analysis,
                    &mut diagnostic_ids,
                    "alignment-incomplete",
                    "Some supplied words or cues have unmatched, partial, or ambiguous mappings; candidate timing does not prove words occur in the audio.",
                );
                AudioAnalysisStatus::Partial
            }
        }
        AlignmentResult::Unavailable { .. } => {
            add_diagnostic(
                &mut analysis,
                &mut diagnostic_ids,
                "alignment-unavailable",
                "The bounded alignment profile did not invoke the analyzer; inspect the companion's explicit reason.",
            );
            AudioAnalysisStatus::Unavailable
        }
    };
    if status != AudioAnalysisStatus::Complete {
        analysis.status = AudioAnalysisStatus::Partial;
    }
    analysis.capabilities.push(AudioCapabilityOutcome {
        capability: CapabilityReference {
            id: AUDIO_ALIGNMENT_CAPABILITY_ID.into(),
            version: "1.0.0".into(),
        },
        status,
        provider_evidence_ids: vec![provider_id.into()],
        evidence_artifact_ids: evidence,
        diagnostic_ids,
    });
    analysis.validate()?;
    Ok(analysis)
}
fn add_diagnostic(analysis: &mut AudioAnalysis, ids: &mut Vec<String>, id: &str, message: &str) {
    analysis.diagnostics.push(AudioDiagnostic {
        id: id.into(),
        code: id.into(),
        severity: AudioDiagnosticSeverity::Warning,
        message: message.into(),
    });
    ids.push(id.into());
}
fn native_lexeme(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= AUDIO_ALIGNMENT_MAXIMUM_PHRASE_BYTES
        && bytes[0].is_ascii_lowercase()
        && bytes[bytes.len() - 1].is_ascii_lowercase()
        && bytes.iter().enumerate().all(|(i, b)| {
            b.is_ascii_lowercase()
                || (*b == b'\''
                    && i > 0
                    && i + 1 < bytes.len()
                    && bytes[i - 1].is_ascii_lowercase()
                    && bytes[i + 1].is_ascii_lowercase())
        })
}
fn check_source_end(end: u64, source: &AudioSource) -> Result<()> {
    if end > 600000
        || u128::from(end) * u128::from(source.sample_rate_hz)
            > u128::from(source.frame_count) * 1000
    {
        return Err(invalid(
            "native alignment timing exceeds the source duration",
        ));
    }
    Ok(())
}
fn decimal_milliseconds(number: &serde_json::Number) -> Result<u64> {
    let encoded = number.to_string();
    let (seconds, fractional) = encoded.split_once('.').unwrap_or((&encoded, ""));
    if fractional.len() > 3
        || seconds.starts_with('-')
        || !seconds.bytes().all(|b| b.is_ascii_digit())
        || !fractional.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid(
            "native alignment timing requires exact nonnegative decimal milliseconds",
        ));
    }
    let whole: u64 = seconds
        .parse()
        .map_err(|_| invalid("native alignment seconds overflow"))?;
    let fraction: u64 = if fractional.is_empty() {
        0
    } else {
        fractional
            .parse()
            .map_err(|_| invalid("invalid native fractional time"))?
    };
    let ms = whole
        .checked_mul(1000)
        .and_then(|value| value.checked_add(fraction * 10u64.pow(3 - fractional.len() as u32)))
        .ok_or_else(|| invalid("native alignment milliseconds overflow"))?;
    if ms > 600000 || ms % 10 != 0 {
        return Err(invalid(
            "native alignment timestamps must be on the bounded 10 ms grid",
        ));
    }
    Ok(ms)
}
/// Refuse hidden sub-millisecond precision before serde's floating point parser
/// could round it away. This scans only number lexemes outside JSON strings.
fn validate_native_numbers(bytes: &[u8]) -> Result<()> {
    let mut index = 0;
    let mut string = false;
    while index < bytes.len() {
        if string {
            if bytes[index] == b'\\' {
                index += 2;
                continue;
            }
            if bytes[index] == b'"' {
                string = false;
            }
            index += 1;
            continue;
        }
        if bytes[index] == b'"' {
            string = true;
            index += 1;
            continue;
        }
        if bytes[index] == b'-' || bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_digit()
                    || matches!(bytes[index], b'-' | b'+' | b'.' | b'e' | b'E'))
            {
                index += 1;
            }
            let number = &bytes[start..index];
            let parts: Vec<&[u8]> = number.split(|b| *b == b'.').collect();
            if parts.len() > 2
                || parts[0].is_empty()
                || parts[0].len() > 6
                || !parts[0].iter().all(u8::is_ascii_digit)
                || (parts.len() == 2
                    && (parts[1].is_empty()
                        || parts[1].len() > 3
                        || !parts[1].iter().all(u8::is_ascii_digit)))
            {
                return Err(invalid(
                    "native alignment numbers require bounded plain decimals with at most three fractional digits",
                ));
            }
        } else {
            index += 1;
        }
    }
    Ok(())
}
fn rational_milliseconds(milliseconds: u64) -> AudioRationalTime {
    let mut left = milliseconds;
    let mut right = 1000;
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    AudioRationalTime {
        numerator: (milliseconds / left) as i64,
        denominator: (1000 / left) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timed_text::{OverlapPolicy, TIMED_TEXT_SCHEMA_V1, TimedTextCue};
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    pub(super) fn source() -> AudioSource {
        AudioSource {
            artifact: AudioArtifactReference {
                id: "source_audio".into(),
                sha256: "a".repeat(64),
                byte_size: 128044,
            },
            stream_index: 0,
            sample_rate_hz: 16000,
            channels: 1,
            frame_count: 64000,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        }
    }
    fn lyrics(texts: &[&str]) -> ReviewedLyrics {
        let document = TimedTextDocument {
            schema: TIMED_TEXT_SCHEMA_V1.into(),
            source: AudioArtifactReference {
                id: "source_text".into(),
                sha256: "b".repeat(64),
                byte_size: 100,
            },
            provenance: TextProvenance::ReviewedLyrics {
                authority: AudioReviewedAuthority {
                    supplied_by: "synthetic reviewer".into(),
                    provenance_artifact_id: "review_record".into(),
                },
                evidence: AudioArtifactReference {
                    id: "review_record".into(),
                    sha256: "c".repeat(64),
                    byte_size: 100,
                },
                source_sha256: "b".repeat(64),
            },
            language: Some("en".into()),
            audio_source: None,
            overlap_policy: OverlapPolicy::Reject,
            metadata: BTreeMap::new(),
            cues: texts
                .iter()
                .enumerate()
                .map(|(index, text)| TimedTextCue {
                    id: format!("cue_{index}"),
                    source_label: Some(format!("Line {index}")),
                    text: (*text).into(),
                    timing: CueTiming::Untimed {},
                    speaker: Some("synthetic speaker".into()),
                })
                .collect(),
        };
        ReviewedLyrics::from_json_slice(&document.canonical_json_bytes().unwrap()).unwrap()
    }
    fn raw(words: &[&str]) -> Value {
        json!({"b":0.0,"d":4.0,"p":1.0,"t":words.iter().filter(|word| !FILLERS.contains(word)).copied().collect::<Vec<_>>().join(" "),"w":words.iter().enumerate().map(|(index,word)| json!({"b":index as f64 *0.25,"d":0.25,"p":0.8,"t":word})).collect::<Vec<_>>()})
    }
    fn parse(value: &Value, reviewed: &ReviewedLyrics) -> Result<AlignmentObservation> {
        normalize(&serde_json::to_vec(value).unwrap(), &source(), reviewed)
    }
    #[test]
    fn exact_text_punctuation_offsets_and_review_provenance_survive_candidates() {
        let reviewed = lyrics(&["  Hello, WORLD!\n", "Don't stop--now."]);
        let tokens = tokenize(&reviewed).unwrap();
        assert_eq!(
            alignment_phrase(&reviewed).unwrap(),
            "hello world don't stop now"
        );
        assert_eq!((tokens[0].byte_start, tokens[0].byte_end), (2, 7));
        assert_eq!(tokens[2].text, "Don't");
        let observation = parse(
            &raw(&["<sil>", "hello", "world", "don't", "stop", "now", "</s>"]),
            &reviewed,
        )
        .unwrap();
        assert!(observation.is_complete());
        let document = timed_text(&observation, &source(), &reviewed).unwrap();
        assert_eq!(document.provenance, reviewed.document.provenance);
        assert_eq!(document.source, reviewed.document.source);
        assert_eq!(document.cues[0].text, "  Hello, WORLD!\n");
        assert_eq!(document.cues[1].speaker, reviewed.document.cues[1].speaker);
        assert_eq!(
            document.cues[1].source_label,
            reviewed.document.cues[1].source_label
        );
        assert_eq!(document.metadata[TIMING_METADATA], "candidate");
        assert_eq!(document.audio_source, Some(source()));
        assert_eq!(
            document.cues[0].timing,
            CueTiming::Interval {
                start: rational_milliseconds(250),
                end: rational_milliseconds(750)
            }
        );
    }
    #[test]
    fn missing_unique_words_are_partial_without_invented_cue_timing() {
        let reviewed = lyrics(&["hello wide world", "stay"]);
        let observation = parse(&raw(&["hello", "world"]), &reviewed).unwrap();
        assert!(!observation.is_complete());
        assert_eq!(
            observation.words[1].timing,
            AlignmentWordTiming::Unmatched {}
        );
        assert_eq!(observation.cues[0].timing, AlignmentCueTiming::Partial {});
        assert_eq!(observation.cues[1].timing, AlignmentCueTiming::Unmatched {});
        let document = timed_text(&observation, &source(), &reviewed).unwrap();
        assert!(
            document
                .cues
                .iter()
                .all(|cue| cue.timing == CueTiming::Untimed {})
        );
    }
    #[test]
    fn missing_repeated_occurrence_is_ambiguous_but_context_can_force_unique_pairs() {
        let reviewed = lyrics(&["hello hello", "world"]);
        let observation = parse(&raw(&["hello", "world"]), &reviewed).unwrap();
        assert_eq!(
            observation.words[0].timing,
            AlignmentWordTiming::Ambiguous {}
        );
        assert_eq!(
            observation.words[1].timing,
            AlignmentWordTiming::Ambiguous {}
        );
        assert!(matches!(
            observation.words[2].timing,
            AlignmentWordTiming::Candidate { .. }
        ));
        assert_eq!(observation.cues[0].timing, AlignmentCueTiming::Ambiguous {});
        let all = parse(&raw(&["hello", "hello", "world"]), &reviewed).unwrap();
        assert!(all.is_complete());
        let contextual = lyrics(&["hello world hello"]);
        let partial = parse(&raw(&["world", "hello"]), &contextual).unwrap();
        assert_eq!(partial.words[0].timing, AlignmentWordTiming::Unmatched {});
        assert!(matches!(
            partial.words[2].timing,
            AlignmentWordTiming::Candidate { .. }
        ));
    }
    #[test]
    fn extra_reordered_and_alternate_pronunciation_tokens_are_refused() {
        let reviewed = lyrics(&["hello world"]);
        for words in [
            &["world", "hello"][..],
            &["hello", "world", "world"],
            &["hello(2)", "world"],
            &["HELLO", "world"],
            &["hello", "<unknown>"],
        ] {
            assert!(parse(&raw(words), &reviewed).is_err());
        }
    }
    #[test]
    fn empty_native_words_remain_explicitly_unmatched() {
        let reviewed = lyrics(&["hello world"]);
        let observation = parse(&raw(&[]), &reviewed).unwrap();
        assert!(!observation.is_complete());
        assert!(
            observation
                .words
                .iter()
                .all(|word| word.timing == AlignmentWordTiming::Unmatched {})
        );
        assert_eq!(
            timed_text(&observation, &source(), &reviewed).unwrap().cues[0].timing,
            CueTiming::Untimed {}
        );
    }
    #[test]
    fn native_header_order_bounds_and_uncalibrated_scores_are_checked() {
        let reviewed = lyrics(&["hello world"]);
        for (pointer, replacement) in [
            ("/b", json!(0.01)),
            ("/d", json!(4.01)),
            ("/p", json!(0.5)),
            ("/t", json!("hello")),
            ("/w/0/d", json!(0.0)),
            ("/w/0/b", json!(0.001)),
            ("/w/1/b", json!(0.1)),
            ("/w/0/p", json!(1.01)),
            ("/w/1/d", json!(4.0)),
        ] {
            let mut value = raw(&["hello", "world"]);
            *value.pointer_mut(pointer).unwrap() = replacement;
            assert!(parse(&value, &reviewed).is_err(), "{pointer}");
        }
        let mut value = raw(&["hello", "world"]);
        value["w"][0]["phones"] = json!([]);
        assert!(parse(&value, &reviewed).is_err());
        let duplicate = serde_json::to_string(&raw(&["hello", "world"]))
            .unwrap()
            .replacen("\"b\":0.0", "\"b\":0.0,\"b\":0.0", 1);
        assert!(normalize(duplicate.as_bytes(), &source(), &reviewed).is_err());
    }
    #[test]
    fn numeric_lexemes_cannot_hide_precision_or_exponents_before_parsing() {
        let reviewed = lyrics(&["hello"]);
        let json = serde_json::to_string(&raw(&["hello"])).unwrap();
        for replacement in ["0.0000000000000000000000001", "0e0", "-0.0", "1e999", "NaN"] {
            let changed = json.replacen("\"b\":0.0", &format!("\"b\":{replacement}"), 1);
            assert!(
                normalize(changed.as_bytes(), &source(), &reviewed).is_err(),
                "{replacement}"
            );
        }
    }
    #[test]
    fn invalid_source_authority_lexemes_and_metadata_are_not_silently_rewritten() {
        let base = lyrics(&["hello"]);
        for text in ["café", "hello2", "世界", "123", "!!!"] {
            let mut reviewed = base.clone();
            reviewed.document.cues[0].text = text.into();
            assert!(reviewed.validate().is_err());
        }
        let mut reviewed = base.clone();
        reviewed.document.provenance = TextProvenance::Unreviewed {};
        assert!(reviewed.validate().is_err());
        let mut reviewed = base.clone();
        reviewed.document.provenance = TextProvenance::ObservedTranscript {
            producer: "synthetic".into(),
        };
        assert!(reviewed.validate().is_err());
        let mut reviewed = base.clone();
        reviewed
            .document
            .metadata
            .insert(TIMING_METADATA.into(), "reviewed".into());
        assert!(reviewed.validate().is_err());
        let mut reviewed = base;
        let mut other = source();
        other.artifact.sha256 = "d".repeat(64);
        reviewed.document.audio_source = Some(other);
        assert!(parse(&raw(&["hello"]), &reviewed).is_err());
    }
    #[test]
    fn deterministic_validation_refuses_forged_promotions_and_text_changes() {
        let reviewed = lyrics(&["hello hello"]);
        let observation = parse(&raw(&["hello"]), &reviewed).unwrap();
        let mut forged = observation.clone();
        forged.words[0].timing = AlignmentWordTiming::Candidate {
            start: rational_milliseconds(0),
            end: rational_milliseconds(250),
        };
        assert!(forged.validate(&source(), &reviewed).is_err());
        let mut forged = observation;
        forged.words[0].token.text = "changed".into();
        assert!(forged.validate(&source(), &reviewed).is_err());
    }
    #[test]
    fn bounded_tokenization_preserves_exact_original_json_identity() {
        let reviewed = lyrics(&["hello"]);
        let compact = reviewed.document.canonical_json_bytes().unwrap();
        let mut pretty = serde_json::to_vec_pretty(&reviewed.document).unwrap();
        pretty.push(b'\n');
        let another = ReviewedLyrics::from_json_slice(&pretty).unwrap();
        assert_eq!(another.document, reviewed.document);
        assert_ne!(
            another.artifact.sha256,
            format!("{:x}", Sha256::digest(&compact))
        );
        assert_eq!(
            another.artifact.sha256,
            format!("{:x}", Sha256::digest(&pretty))
        );
        let mut excessive = reviewed;
        excessive.document.cues[0].text = vec!["hello"; 513].join(" ");
        assert!(excessive.validate().is_err());
    }
    #[test]
    fn conservative_mapping_agrees_with_all_small_monotonic_embeddings() {
        fn embeddings(
            authored: &[&str],
            native: &[&str],
            cursor: usize,
            selected: &mut Vec<usize>,
            found: &mut Vec<Vec<usize>>,
        ) {
            if selected.len() == native.len() {
                found.push(selected.clone());
                return;
            }
            for position in cursor..authored.len() {
                if authored[position] == native[selected.len()] {
                    selected.push(position);
                    embeddings(authored, native, position + 1, selected, found);
                    selected.pop();
                }
            }
        }
        for count in 1..=5 {
            for mask in 0..(1 << count) {
                let authored: Vec<&str> = (0..count)
                    .map(|index| if mask & (1 << index) == 0 { "a" } else { "b" })
                    .collect();
                let reviewed = lyrics(&[&authored.join(" ")]);
                for native_count in 0..=count {
                    for native_mask in 0..(1 << native_count) {
                        let native: Vec<&str> = (0..native_count)
                            .map(|index| {
                                if native_mask & (1 << index) == 0 {
                                    "a"
                                } else {
                                    "b"
                                }
                            })
                            .collect();
                        let mut found = Vec::new();
                        embeddings(&authored, &native, 0, &mut Vec::new(), &mut found);
                        let result = parse(&raw(&native), &reviewed);
                        if found.is_empty() {
                            assert!(result.is_err());
                            continue;
                        }
                        let observation = result.unwrap();
                        for position in 0..count {
                            let possible: Vec<Option<usize>> = found
                                .iter()
                                .map(|mapping| mapping.iter().position(|&value| value == position))
                                .collect();
                            let expected = if possible.iter().all(Option::is_none) {
                                "unmatched"
                            } else if possible[0].is_some()
                                && possible.iter().all(|value| *value == possible[0])
                            {
                                "candidate"
                            } else {
                                "ambiguous"
                            };
                            let actual = match observation.words[position].timing {
                                AlignmentWordTiming::Candidate { .. } => "candidate",
                                AlignmentWordTiming::Unmatched {} => "unmatched",
                                AlignmentWordTiming::Ambiguous {} => "ambiguous",
                            };
                            assert_eq!(
                                actual, expected,
                                "authored={authored:?} native={native:?} position={position}"
                            );
                        }
                    }
                }
            }
        }
    }
}
