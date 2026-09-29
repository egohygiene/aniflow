//! Bounded text codecs with exact timing and explicit conversion loss.
//!
//! These functions preserve supplied provenance; they do not transcribe audio,
//! align lyrics, or authenticate a caller's review attestation. Raw serde
//! deserialization is unvalidated: use the contract constructors or `validate`.

mod file;
mod legacy;
mod markup;
mod types;

use std::collections::BTreeSet;

use crate::audio_analysis::{AudioArtifactReference, AudioRationalTime};
use sha2::{Digest, Sha256};

pub use file::{FileConversionOutcome, convert_file};
pub use types::*;
use types::{ParsedCue, ParsedText, invalid};

/// The five supported text subsets and the normalized JSON transport.
#[must_use]
pub fn registry() -> TimedTextRegistry {
    let profiles = [
        (
            TimedTextFormat::Plain,
            "text/plain",
            "one untimed UTF-8 text cue",
        ),
        (
            TimedTextFormat::Lrc,
            "text/plain",
            "single centisecond timestamp; leading basic metadata",
        ),
        (
            TimedTextFormat::Srt,
            "application/x-subrip",
            "numeric labels; explicit millisecond intervals; plain text",
        ),
        (
            TimedTextFormat::Webvtt,
            "text/vtt",
            "explicit millisecond intervals; optional whole-cue voice",
        ),
        (
            TimedTextFormat::Ttml,
            "application/ttml+xml",
            "tt/body/div/p/br; explicit intervals; preserved XML space",
        ),
        (
            TimedTextFormat::Json,
            "application/json",
            "validated aniflow.timed-text/v1 semantic transport",
        ),
    ];
    TimedTextRegistry {
        schema: TIMED_TEXT_REGISTRY_SCHEMA_V1.into(),
        formats: profiles
            .into_iter()
            .map(|(format, media_type, subset)| TimedTextFormatProfile {
                format,
                extension: format.extension().into(),
                media_type: media_type.into(),
                subset: subset.into(),
                transport_only: format == TimedTextFormat::Json,
            })
            .collect(),
    }
}

/// Decode a supported carrier without guessing timing or review authority.
/// Default context preserves an existing JSON document's semantic context.
pub fn decode(
    bytes: &[u8],
    format: TimedTextFormat,
    context: &ImportContext,
) -> crate::Result<DecodedText> {
    context.validate()?;
    let (input, lexical) = normalize_input(bytes)?;
    if format == TimedTextFormat::Json {
        let document = TimedTextDocument::from_json_slice(input.as_bytes())?;
        if context != &ImportContext::default()
            && (context.provenance != document.provenance
                || context.language != document.language
                || context.audio_source != document.audio_source
                || context.overlap_policy != document.overlap_policy)
        {
            return Err(invalid(
                "JSON transport cannot override existing text provenance or context",
            ));
        }
        return Ok(DecodedText { document, lexical });
    }
    let parsed = match format {
        TimedTextFormat::Plain => legacy::parse_plain(&input),
        TimedTextFormat::Lrc => legacy::parse_lrc(&input),
        TimedTextFormat::Srt => legacy::parse_srt(&input),
        TimedTextFormat::Webvtt => markup::parse_webvtt(&input),
        TimedTextFormat::Ttml => markup::parse_ttml(&input),
        TimedTextFormat::Json => unreachable!(),
    }?;
    if parsed.language.is_some()
        && context.language.is_some()
        && parsed.language != context.language
    {
        return Err(invalid(
            "supplied language conflicts with the authored document language",
        ));
    }
    let document = TimedTextDocument {
        schema: TIMED_TEXT_SCHEMA_V1.into(),
        source: artifact("source_text", bytes),
        provenance: context.provenance.clone(),
        language: parsed.language.or_else(|| context.language.clone()),
        audio_source: context.audio_source.clone(),
        overlap_policy: context.overlap_policy,
        metadata: parsed.metadata,
        cues: parsed
            .cues
            .into_iter()
            .enumerate()
            .map(|(index, cue)| TimedTextCue {
                id: format!("cue_{:06}", index + 1),
                source_label: cue.source_label,
                text: cue.text,
                timing: cue.timing,
                speaker: cue.speaker,
            })
            .collect(),
    };
    document.validate()?;
    Ok(DecodedText { document, lexical })
}

/// Encode a validated document after applying only explicitly permitted losses.
pub fn encode(
    document: &TimedTextDocument,
    format: TimedTextFormat,
    options: &ConversionOptions,
) -> Result<EncodedText, ConversionFailure> {
    document.validate()?;
    options.validate()?;
    let mut projected = document.clone();
    let mut losses = Vec::new();
    project(&mut projected, format, &mut losses).map_err(|error| ConversionFailure {
        error,
        losses: losses.clone(),
    })?;
    if losses
        .iter()
        .any(|loss| !options.allow_losses.contains(&loss.kind))
    {
        return Err(ConversionFailure {
            error: invalid("conversion requires explicit permission for every reported loss"),
            losses,
        });
    }
    let render = || -> crate::Result<Vec<u8>> {
        projected.validate()?;
        let bytes = match format {
            TimedTextFormat::Plain => legacy::write_plain(&projected)?.into_bytes(),
            TimedTextFormat::Lrc => legacy::write_lrc(&projected)?.into_bytes(),
            TimedTextFormat::Srt => legacy::write_srt(&projected)?.into_bytes(),
            TimedTextFormat::Webvtt => markup::write_webvtt(&projected)?.into_bytes(),
            TimedTextFormat::Ttml => markup::write_ttml(&projected)?.into_bytes(),
            TimedTextFormat::Json => projected.canonical_json_bytes()?,
        };
        if bytes.is_empty() || bytes.len() > MAX_TEXT_BYTES {
            return Err(invalid("rendered text must be nonempty and at most 1 MiB"));
        }
        Ok(bytes)
    };
    let bytes = render().map_err(|error| ConversionFailure {
        error,
        losses: losses.clone(),
    })?;
    let mut carrier_omissions = Vec::new();
    if format != TimedTextFormat::Json {
        carrier_omissions.push(CarrierOmission::Provenance);
        if projected.audio_source.is_some() {
            carrier_omissions.push(CarrierOmission::AudioBinding);
        }
        carrier_omissions.push(CarrierOmission::InternalCueIdentities);
    }
    Ok(EncodedText {
        document: projected,
        bytes,
        losses,
        carrier_omissions,
    })
}

/// Convert in memory, retaining both normalized documents and byte identities.
pub fn convert(
    bytes: &[u8],
    from: TimedTextFormat,
    to: TimedTextFormat,
    context: &ImportContext,
    options: &ConversionOptions,
) -> Result<TimedTextConversion, ConversionFailure> {
    let decoded = decode(bytes, from, context)?;
    let encoded = encode(&decoded.document, to, options)?;
    let report = TimedTextConversionReport {
        schema: TIMED_TEXT_CONVERSION_SCHEMA_V1.into(),
        input: artifact("conversion_input", bytes),
        output: artifact("conversion_output", &encoded.bytes),
        from,
        to,
        input_document_sha256: sha256(&decoded.document.canonical_json_bytes()?),
        output_document_sha256: sha256(&encoded.document.canonical_json_bytes()?),
        allowed_losses: options.allow_losses.clone(),
        losses: encoded.losses,
        lexical: decoded.lexical,
        carrier_omissions: encoded.carrier_omissions,
        review_attestation_independently_verified: false,
    };
    report
        .canonical_json_bytes()
        .map_err(|error| ConversionFailure {
            error,
            losses: report.losses.clone(),
        })?;
    Ok(TimedTextConversion {
        input_document: decoded.document,
        output_document: encoded.document,
        bytes: encoded.bytes,
        report,
    })
}

fn project(
    document: &mut TimedTextDocument,
    format: TimedTextFormat,
    losses: &mut Vec<ConversionLoss>,
) -> crate::Result<()> {
    if format == TimedTextFormat::Json {
        return Ok(());
    }
    if format == TimedTextFormat::Plain && document.cues[0].text.starts_with('\u{feff}') {
        return Err(invalid(
            "plain text cannot distinguish this authored leading U+FEFF from a byte-order mark; use JSON or a timed carrier",
        ));
    }
    if document.cues.iter().any(|cue| cue.text.contains('\r')) && format != TimedTextFormat::Ttml {
        return Err(invalid(
            "this carrier cannot preserve authored carriage-return text; use TTML or JSON",
        ));
    }
    if format != TimedTextFormat::Ttml && document.language.take().is_some() {
        loss(
            losses,
            ConversionLossKind::Language,
            None,
            "language",
            "target carrier omits the supplied document language",
        );
    }
    let retained_keys: &[&str] = match format {
        TimedTextFormat::Lrc => &["ar", "al", "ti", "au", "by", "re", "ve"],
        TimedTextFormat::Ttml => &["ttml.tt.id", "ttml.body.id", "ttml.div.id"],
        _ => &[],
    };
    document.metadata.retain(|key, _| {
        let retain = retained_keys.contains(&key.as_str());
        if !retain {
            loss(
                losses,
                ConversionLossKind::Metadata,
                None,
                &format!("metadata.{key}"),
                "target carrier cannot preserve this metadata field",
            );
        }
        retain
    });
    let mut used_labels: BTreeSet<String> = if format == TimedTextFormat::Ttml {
        document.metadata.values().cloned().collect()
    } else {
        BTreeSet::new()
    };
    for (index, cue) in document.cues.iter_mut().enumerate() {
        if cue.speaker.is_some()
            && (format != TimedTextFormat::Webvtt
                || markup::webvtt_speaker(cue.speaker.as_deref().unwrap_or("")).is_err())
        {
            loss(
                losses,
                ConversionLossKind::Speaker,
                Some(&cue.id),
                "speaker",
                "target carrier cannot preserve this speaker annotation",
            );
            cue.speaker = None;
        }
        let retain_label = cue.source_label.as_ref().is_none_or(|label| match format {
            TimedTextFormat::Srt => {
                !label.is_empty() && label.bytes().all(|byte| byte.is_ascii_digit())
            }
            TimedTextFormat::Webvtt => {
                markup::webvtt_label(label).is_ok() && used_labels.insert(label.clone())
            }
            TimedTextFormat::Ttml => {
                markup::xml_id(label).is_ok() && used_labels.insert(label.clone())
            }
            _ => false,
        });
        if !retain_label {
            loss(
                losses,
                ConversionLossKind::CueIdentifiers,
                Some(&cue.id),
                "source_label",
                "target carrier cannot preserve this authored cue label",
            );
            cue.source_label = None;
        }
        if format == TimedTextFormat::Srt && cue.source_label.is_none() {
            cue.source_label = Some((index + 1).to_string());
        }
        match (format, &cue.timing) {
            (TimedTextFormat::Plain, CueTiming::Untimed {}) => {}
            (TimedTextFormat::Plain, _) => {
                loss(
                    losses,
                    ConversionLossKind::Timing,
                    Some(&cue.id),
                    "timing",
                    "plain text removes the cue timing",
                );
                cue.timing = CueTiming::Untimed {};
            }
            (TimedTextFormat::Lrc, CueTiming::Point { at }) => {
                let value = round_lrc(*at, &cue.id, "timing.at", losses)?;
                cue.timing = CueTiming::Point { at: value };
            }
            (TimedTextFormat::Lrc, CueTiming::Interval { start, .. }) => {
                let value = round_lrc(*start, &cue.id, "timing.start", losses)?;
                loss(
                    losses,
                    ConversionLossKind::EndTimes,
                    Some(&cue.id),
                    "timing.end",
                    "LRC retains the start only and removes the explicit end",
                );
                cue.timing = CueTiming::Point { at: value };
            }
            (TimedTextFormat::Lrc, CueTiming::Untimed {}) => {
                return Err(invalid(
                    "LRC requires an explicit timestamp; conversion cannot invent timing",
                ));
            }
            (
                TimedTextFormat::Srt | TimedTextFormat::Webvtt | TimedTextFormat::Ttml,
                CueTiming::Interval { .. },
            ) => {}
            _ => {
                return Err(invalid(
                    "target format requires an explicit start and end; conversion cannot invent timing",
                ));
            }
        }
    }
    if format == TimedTextFormat::Plain && document.cues.len() > 1 {
        let mut text = String::new();
        for (index, cue) in document.cues.iter().enumerate() {
            if index > 0 {
                loss(
                    losses,
                    ConversionLossKind::CueIdentifiers,
                    Some(&cue.id),
                    "cue_boundary",
                    "plain text joins this cue after two line feeds and retains only the first internal cue identity",
                );
                text.push_str("\n\n");
            }
            if text.len().saturating_add(cue.text.len()) > MAX_CUE_BYTES {
                return Err(invalid(
                    "plain text projection exceeds the 64 KiB single-cue limit",
                ));
            }
            text.push_str(&cue.text);
        }
        document.cues.truncate(1);
        document.cues[0].text = text;
    }
    Ok(())
}

fn round_lrc(
    time: AudioRationalTime,
    cue_id: &str,
    field: &str,
    losses: &mut Vec<ConversionLoss>,
) -> crate::Result<AudioRationalTime> {
    let milliseconds = milliseconds(time)?;
    if milliseconds % 10 == 0 {
        return Ok(time);
    }
    let rounded = (milliseconds + 5) / 10 * 10;
    loss(
        losses,
        ConversionLossKind::Precision,
        Some(cue_id),
        field,
        &format!(
            "LRC rounds {milliseconds} milliseconds to {rounded} milliseconds; nearest centisecond, ties upward"
        ),
    );
    Ok(time_from_milliseconds(rounded))
}

fn loss(
    losses: &mut Vec<ConversionLoss>,
    kind: ConversionLossKind,
    cue_id: Option<&str>,
    field: &str,
    detail: &str,
) {
    losses.push(ConversionLoss {
        kind,
        cue_id: cue_id.map(str::to_owned),
        field: field.into(),
        detail: detail.into(),
    });
}

fn normalize_input(bytes: &[u8]) -> crate::Result<(String, LexicalFacts)> {
    if bytes.is_empty() || bytes.len() > MAX_TEXT_BYTES {
        return Err(invalid("source text must be nonempty and at most 1 MiB"));
    }
    let value = std::str::from_utf8(bytes)
        .map_err(|_| invalid("source text must be valid UTF-8 without replacement"))?;
    let (value, bom) = value
        .strip_prefix('\u{feff}')
        .map_or((value, false), |value| (value, true));
    let mut lexical = LexicalFacts {
        utf8_bom_removed: bom,
        ..LexicalFacts::default()
    };
    let mut normalized = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
                lexical.crlf_pairs += 1;
            } else {
                lexical.lone_cr += 1;
            }
            normalized.push('\n');
        } else {
            normalized.push(character);
        }
    }
    Ok((normalized, lexical))
}

pub(super) fn time_from_milliseconds(value: u64) -> AudioRationalTime {
    let divisor = gcd(value, 1000);
    AudioRationalTime {
        numerator: (value / divisor) as i64,
        denominator: (1000 / divisor) as u32,
    }
}
pub(super) fn milliseconds(value: AudioRationalTime) -> crate::Result<u64> {
    value.validate()?;
    if value.numerator < 0 {
        return Err(invalid("cue times cannot be negative"));
    }
    let scaled = u128::from(value.numerator as u64) * 1000;
    if scaled % u128::from(value.denominator) != 0 {
        return Err(invalid(
            "registered text timing requires exact millisecond precision",
        ));
    }
    let milliseconds = scaled / u128::from(value.denominator);
    if milliseconds > u128::from(MAX_TIME_MILLISECONDS) {
        return Err(invalid("cue timing exceeds the 24-hour profile"));
    }
    Ok(milliseconds as u64)
}
fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
}
pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn artifact(id: &str, bytes: &[u8]) -> AudioArtifactReference {
    AudioArtifactReference {
        id: id.into(),
        sha256: sha256(bytes),
        byte_size: bytes.len() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &[u8] =
        b"1\n00:00:00,995 --> 00:00:01,500\nfirst\n\n2\n00:00:02,000 --> 00:00:03,000\nsecond\n";

    #[test]
    fn exact_millisecond_grid_rejects_unreduced_negative_and_fractional_times() {
        assert_eq!(milliseconds(time_from_milliseconds(1_250)).unwrap(), 1_250);
        assert_eq!(
            time_from_milliseconds(0),
            AudioRationalTime {
                numerator: 0,
                denominator: 1
            }
        );
        for value in [
            AudioRationalTime {
                numerator: 2,
                denominator: 4,
            },
            AudioRationalTime {
                numerator: -1,
                denominator: 1,
            },
            AudioRationalTime {
                numerator: 1,
                denominator: 3,
            },
            AudioRationalTime {
                numerator: 86_401,
                denominator: 1,
            },
        ] {
            assert!(milliseconds(value).is_err(), "{value:?}");
        }
        assert_eq!(
            milliseconds(time_from_milliseconds(MAX_TIME_MILLISECONDS)).unwrap(),
            MAX_TIME_MILLISECONDS
        );
    }

    #[test]
    fn lrc_rounding_is_explicit_and_ties_up_without_synthesizing_ends() {
        let document = decode(SOURCE, TimedTextFormat::Srt, &ImportContext::default())
            .unwrap()
            .document;
        let failure = encode(
            &document,
            TimedTextFormat::Lrc,
            &ConversionOptions::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure
                .losses
                .iter()
                .filter(|loss| loss.kind == ConversionLossKind::EndTimes)
                .count(),
            2
        );
        assert_eq!(
            failure
                .losses
                .iter()
                .filter(|loss| loss.kind == ConversionLossKind::Precision)
                .count(),
            1
        );
        let encoded = encode(
            &document,
            TimedTextFormat::Lrc,
            &ConversionOptions {
                allow_losses: vec![
                    ConversionLossKind::CueIdentifiers,
                    ConversionLossKind::EndTimes,
                    ConversionLossKind::Precision,
                ],
            },
        )
        .unwrap();
        assert!(
            String::from_utf8(encoded.bytes)
                .unwrap()
                .starts_with("[00:01.00]first")
        );
        assert!(
            encode(
                &encoded.document,
                TimedTextFormat::Srt,
                &ConversionOptions {
                    allow_losses: vec![ConversionLossKind::Timing, ConversionLossKind::EndTimes]
                }
            )
            .is_err()
        );
    }

    #[test]
    fn projection_revalidates_rounding_collisions_under_overlap_policy() {
        let input =
            b"1\n00:00:00,995 --> 00:00:00,997\none\n\n2\n00:00:00,998 --> 00:00:01,000\ntwo\n";
        let document = decode(input, TimedTextFormat::Srt, &ImportContext::default())
            .unwrap()
            .document;
        let failure = encode(
            &document,
            TimedTextFormat::Lrc,
            &ConversionOptions {
                allow_losses: vec![
                    ConversionLossKind::CueIdentifiers,
                    ConversionLossKind::EndTimes,
                    ConversionLossKind::Precision,
                ],
            },
        )
        .unwrap_err();
        assert!(failure.error.message().contains("overlapping"));
        assert_eq!(failure.losses.len(), 6);
    }

    #[test]
    fn authored_labels_are_not_internal_ids_and_plain_merge_is_reported() {
        let input =
            b"7\n00:00:00,000 --> 00:00:01,000\none\n\n7\n00:00:01,000 --> 00:00:02,000\ntwo\n";
        let document = decode(input, TimedTextFormat::Srt, &ImportContext::default())
            .unwrap()
            .document;
        assert_ne!(document.cues[0].id, document.cues[1].id);
        assert_eq!(document.cues[0].source_label, document.cues[1].source_label);
        let encoded = encode(
            &document,
            TimedTextFormat::Plain,
            &ConversionOptions {
                allow_losses: vec![
                    ConversionLossKind::Timing,
                    ConversionLossKind::CueIdentifiers,
                ],
            },
        )
        .unwrap();
        assert_eq!(encoded.bytes, b"one\n\ntwo");
        assert_eq!(encoded.document.cues.len(), 1);
        assert_eq!(
            encoded
                .losses
                .iter()
                .filter(|loss| loss.field == "cue_boundary")
                .count(),
            1
        );
    }

    #[test]
    fn plain_output_refuses_to_reinterpret_an_authored_leading_bom_character() {
        let mut document = decode(b"hello", TimedTextFormat::Plain, &ImportContext::default())
            .unwrap()
            .document;
        document.cues[0].text.insert(0, '\u{feff}');
        assert!(document.validate().is_ok());
        assert!(
            encode(
                &document,
                TimedTextFormat::Plain,
                &ConversionOptions::default()
            )
            .unwrap_err()
            .error
            .message()
            .contains("U+FEFF")
        );
        let encoded = encode(
            &document,
            TimedTextFormat::Json,
            &ConversionOptions::default(),
        )
        .unwrap();
        assert_eq!(
            decode(
                &encoded.bytes,
                TimedTextFormat::Json,
                &ImportContext::default()
            )
            .unwrap()
            .document
            .cues[0]
                .text,
            "\u{feff}hello"
        );
    }

    #[test]
    fn exact_audio_duration_accepts_non_sample_grid_cues_but_refuses_overrun() {
        let mut document = decode(SOURCE, TimedTextFormat::Srt, &ImportContext::default())
            .unwrap()
            .document;
        document.audio_source = Some(crate::audio_analysis::AudioSource {
            artifact: AudioArtifactReference {
                id: "audio".into(),
                sha256: "a".repeat(64),
                byte_size: 123,
            },
            stream_index: 0,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            sample_rate_hz: 44_100,
            channels: 1,
            frame_count: 132_300,
            stem: None,
        });
        assert!(document.validate().is_ok());
        document.audio_source.as_mut().unwrap().frame_count -= 1;
        assert!(document.validate().is_err());
    }

    #[test]
    fn normalized_json_refuses_duplicate_metadata_instead_of_dropping_a_value() {
        let document = decode(b"hello", TimedTextFormat::Plain, &ImportContext::default())
            .unwrap()
            .document;
        let serialized = String::from_utf8(document.canonical_json_bytes().unwrap()).unwrap();
        let duplicate = serialized.replace(
            "\"metadata\":{}",
            "\"metadata\":{\"ti\":\"first\",\"ti\":\"second\"}",
        );
        assert_ne!(duplicate, serialized);
        assert!(
            decode(
                duplicate.as_bytes(),
                TimedTextFormat::Json,
                &ImportContext::default()
            )
            .is_err()
        );
    }

    #[test]
    fn json_transport_report_cannot_claim_a_loss_even_when_permission_was_supplied() {
        let mut report = convert(
            b"hello",
            TimedTextFormat::Plain,
            TimedTextFormat::Json,
            &ImportContext::default(),
            &ConversionOptions::default(),
        )
        .unwrap()
        .report;
        report.allowed_losses.push(ConversionLossKind::Metadata);
        report.losses.push(ConversionLoss {
            kind: ConversionLossKind::Metadata,
            cue_id: None,
            field: "metadata.ti".into(),
            detail: "invented loss".into(),
        });
        assert!(report.validate().is_err());
    }

    #[test]
    fn closed_contract_fixtures_validate_without_promoting_authority() {
        let document = TimedTextDocument::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/timed-text-v1.example.json"
        ))
        .unwrap();
        let context = ImportContext::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/timed-text-context-v1.example.json"
        ))
        .unwrap();
        assert!(matches!(
            document.provenance,
            TextProvenance::ReviewedLyrics { .. }
        ));
        assert_eq!(context.provenance, document.provenance);
    }
}
