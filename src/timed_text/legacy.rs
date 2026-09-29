//! Registered plain-text, LRC, and SRT subsets.
//!
//! The outer adapter owns UTF-8/BOM and line-ending normalization, authority,
//! projection, and explicit loss permission. This layer preserves payload text,
//! cue order, numeric labels, and exact supported timing without inference.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::Result;

use super::{
    CueTiming, MAX_CUE_BYTES as MAXIMUM_CUE_BYTES, MAX_CUES as MAXIMUM_CUES,
    MAX_TEXT_BYTES as MAXIMUM_TEXT_BYTES, MAX_TIME_MILLISECONDS as MAXIMUM_TIME_MILLISECONDS,
    ParsedCue, ParsedText, TimedTextDocument, invalid, milliseconds, time_from_milliseconds,
};

const LRC_METADATA_KEYS: [&str; 7] = ["ar", "al", "ti", "au", "by", "re", "ve"];

pub(super) fn parse_plain(input: &str) -> Result<ParsedText> {
    validate_input(input)?;
    validate_payload(input, false)?;
    Ok(ParsedText {
        cues: vec![ParsedCue {
            source_label: None,
            text: input.to_owned(),
            timing: CueTiming::Untimed {},
            speaker: None,
        }],
        language: None,
        metadata: BTreeMap::new(),
    })
}

pub(super) fn parse_lrc(input: &str) -> Result<ParsedText> {
    validate_input(input)?;
    let mut cues = Vec::new();
    let mut metadata = BTreeMap::new();
    for line in input.split_terminator('\n') {
        let remaining = line
            .strip_prefix('[')
            .ok_or_else(|| invalid("LRC requires exactly one leading timestamp or metadata tag"))?;
        let (tag, text) = remaining
            .split_once(']')
            .ok_or_else(|| invalid("LRC tag is missing its closing bracket"))?;
        let (name, value) = tag
            .split_once(':')
            .ok_or_else(|| invalid("LRC tag must contain a colon"))?;
        if LRC_METADATA_KEYS.contains(&name) {
            if !cues.is_empty() || !text.is_empty() {
                return Err(invalid(
                    "LRC metadata is supported only as leading whole-line tags",
                ));
            }
            if value.contains(['[', ']']) {
                return Err(invalid("LRC metadata contains unsupported nested tags"));
            }
            if metadata.insert(name.to_owned(), value.to_owned()).is_some() {
                return Err(invalid("duplicate LRC metadata keys are unsupported"));
            }
            continue;
        }
        if !name.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid(
                "unknown LRC metadata, offsets, and extensions are unsupported",
            ));
        }
        let at = parse_lrc_time(tag)?;
        validate_payload(text, true)?;
        if text.starts_with('[') || contains_lrc_timestamp(text) {
            return Err(invalid(
                "LRC multiple timestamp or trailing extension tags are unsupported",
            ));
        }
        if cues.len() == MAXIMUM_CUES {
            return Err(invalid("timed text exceeds the 10000-cue bound"));
        }
        cues.push(ParsedCue {
            source_label: None,
            text: text.to_owned(),
            timing: CueTiming::Point {
                at: time_from_milliseconds(at),
            },
            speaker: None,
        });
    }
    if cues.is_empty() {
        return Err(invalid("LRC requires at least one timed cue"));
    }
    Ok(ParsedText {
        cues,
        language: None,
        metadata,
    })
}

pub(super) fn parse_srt(input: &str) -> Result<ParsedText> {
    validate_input(input)?;
    let mut cues = Vec::new();
    let mut lines = input.split('\n').peekable();
    while lines.peek().is_some() {
        // Empty separator lines are SRT structure. Whitespace-only payload
        // lines are distinct and are never trimmed or treated as separators.
        while lines.peek().is_some_and(|line| line.is_empty()) {
            lines.next();
        }
        let Some(label) = lines.next() else { break };
        validate_srt_label(label)?;
        let timing_line = lines
            .next()
            .ok_or_else(|| invalid("SRT cue is missing its interval"))?;
        let (start, end) = timing_line
            .split_once(" --> ")
            .ok_or_else(|| invalid("SRT requires an exact timestamp interval separator"))?;
        let start = parse_srt_time(start)?;
        let end = parse_srt_time(end)?;
        if start >= end {
            return Err(invalid("SRT intervals must have a strictly later end"));
        }
        let mut payload = Vec::new();
        while let Some(line) = lines.peek() {
            if line.is_empty() {
                break;
            }
            payload.push(lines.next().expect("peeked SRT payload line"));
        }
        let text = payload.join("\n");
        validate_payload(&text, true)?;
        if cues.len() == MAXIMUM_CUES {
            return Err(invalid("timed text exceeds the 10000-cue bound"));
        }
        cues.push(ParsedCue {
            source_label: Some(label.to_owned()),
            text,
            timing: CueTiming::Interval {
                start: time_from_milliseconds(start),
                end: time_from_milliseconds(end),
            },
            speaker: None,
        });
    }
    if cues.is_empty() {
        return Err(invalid("SRT requires at least one interval cue"));
    }
    Ok(ParsedText {
        cues,
        language: None,
        metadata: BTreeMap::new(),
    })
}

pub(super) fn write_plain(document: &TimedTextDocument) -> Result<String> {
    render_plain(&projected_fields(document))
}

pub(super) fn write_lrc(document: &TimedTextDocument) -> Result<String> {
    render_lrc(&projected_fields(document))
}

pub(super) fn write_srt(document: &TimedTextDocument) -> Result<String> {
    render_srt(&projected_fields(document))
}

fn projected_fields(document: &TimedTextDocument) -> ParsedText {
    ParsedText {
        cues: document
            .cues
            .iter()
            .map(|cue| ParsedCue {
                source_label: cue.source_label.clone(),
                text: cue.text.clone(),
                timing: cue.timing.clone(),
                speaker: cue.speaker.clone(),
            })
            .collect(),
        language: document.language.clone(),
        metadata: document.metadata.clone(),
    }
}

fn render_plain(document: &ParsedText) -> Result<String> {
    if document.cues.len() != 1 || document.language.is_some() || !document.metadata.is_empty() {
        return Err(invalid(
            "plain text requires one projected cue without language or metadata",
        ));
    }
    let cue = &document.cues[0];
    if cue.source_label.is_some()
        || cue.speaker.is_some()
        || !matches!(cue.timing, CueTiming::Untimed {})
    {
        return Err(invalid(
            "plain text cannot silently discard cue labels, speakers, or timing",
        ));
    }
    validate_input(&cue.text)?;
    validate_payload(&cue.text, false)?;
    Ok(cue.text.clone())
}

fn render_lrc(document: &ParsedText) -> Result<String> {
    validate_document_size(document)?;
    if document.language.is_some() {
        return Err(invalid("LRC cannot silently discard language metadata"));
    }
    let mut output = String::new();
    for (key, value) in &document.metadata {
        if !LRC_METADATA_KEYS.contains(&key.as_str()) || value.contains(['\n', '\r', '[', ']']) {
            return Err(invalid(
                "LRC supports only its declared single-line metadata tags",
            ));
        }
        writeln!(output, "[{key}:{value}]").expect("writing to String cannot fail");
        validate_input(&output)?;
    }
    for cue in &document.cues {
        if cue.source_label.is_some() || cue.speaker.is_some() {
            return Err(invalid(
                "LRC cannot silently discard cue labels or speakers",
            ));
        }
        let CueTiming::Point { at } = cue.timing else {
            return Err(invalid(
                "LRC requires projected point timing; cue ends are never silently discarded",
            ));
        };
        let at = milliseconds(at)?;
        if at > MAXIMUM_TIME_MILLISECONDS || at % 10 != 0 {
            return Err(invalid(
                "LRC timing must be exact centiseconds within 24 hours",
            ));
        }
        validate_payload(&cue.text, true)?;
        if cue.text.contains('\n') || cue.text.starts_with('[') || contains_lrc_timestamp(&cue.text)
        {
            return Err(invalid(
                "LRC cue text must fit one unambiguous timestamp line",
            ));
        }
        writeln!(
            output,
            "[{:02}:{:02}.{:02}]{}",
            at / 60_000,
            at / 1000 % 60,
            at / 10 % 100,
            cue.text
        )
        .expect("writing to String cannot fail");
        if output.len() > MAXIMUM_TEXT_BYTES {
            return Err(invalid("serialized LRC exceeds the 1 MiB text bound"));
        }
    }
    validate_input(&output)?;
    Ok(output)
}

fn render_srt(document: &ParsedText) -> Result<String> {
    validate_document_size(document)?;
    if document.language.is_some() || !document.metadata.is_empty() {
        return Err(invalid(
            "SRT cannot silently discard language or document metadata",
        ));
    }
    let mut output = String::new();
    for (index, cue) in document.cues.iter().enumerate() {
        if cue.speaker.is_some() {
            return Err(invalid("SRT cannot silently discard speaker metadata"));
        }
        let label = cue
            .source_label
            .as_deref()
            .ok_or_else(|| invalid("SRT requires an explicitly projected numeric source label"))?;
        validate_srt_label(label)?;
        let CueTiming::Interval { start, end } = cue.timing else {
            return Err(invalid(
                "SRT requires explicit interval ends; ends are never inferred",
            ));
        };
        let start = milliseconds(start)?;
        let end = milliseconds(end)?;
        if start >= end || end > MAXIMUM_TIME_MILLISECONDS {
            return Err(invalid("SRT requires positive intervals within 24 hours"));
        }
        validate_payload(&cue.text, true)?;
        if cue.text.starts_with('\n') || cue.text.ends_with('\n') || cue.text.contains("\n\n") {
            return Err(invalid(
                "SRT payload contains blank lines that would change cue boundaries",
            ));
        }
        if index > 0 {
            output.push('\n');
        }
        writeln!(
            output,
            "{label}\n{} --> {}\n{}",
            format_srt_time(start),
            format_srt_time(end),
            cue.text
        )
        .expect("writing to String cannot fail");
        if output.len() > MAXIMUM_TEXT_BYTES {
            return Err(invalid("serialized SRT exceeds the 1 MiB text bound"));
        }
    }
    validate_input(&output)?;
    Ok(output)
}

fn validate_document_size(document: &ParsedText) -> Result<()> {
    if document.cues.is_empty() || document.cues.len() > MAXIMUM_CUES {
        return Err(invalid("timed text requires between 1 and 10000 cues"));
    }
    Ok(())
}

fn validate_input(input: &str) -> Result<()> {
    if input.is_empty() || input.len() > MAXIMUM_TEXT_BYTES {
        return Err(invalid(
            "timed text must contain between 1 byte and 1 MiB of UTF-8",
        ));
    }
    if input
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(invalid(
            "timed text contains unsupported control characters or unnormalized line endings",
        ));
    }
    Ok(())
}

fn validate_payload(text: &str, timed: bool) -> Result<()> {
    if text.is_empty() || text.len() > MAXIMUM_CUE_BYTES {
        return Err(invalid(
            "cue text must contain between 1 byte and 64 KiB of UTF-8",
        ));
    }
    if timed
        && (text.contains(['<', '>'])
            || text.contains("{\\")
            || text.contains("\\N")
            || text.contains("\\n")
            || text.contains("\\h")
            || has_entity(text))
    {
        return Err(invalid(
            "timed text markup, entities, and ASS-style overrides are unsupported",
        ));
    }
    Ok(())
}

fn has_entity(text: &str) -> bool {
    text.split('&').skip(1).any(|after| {
        after.split_once(';').is_some_and(|(entity, _)| {
            !entity.is_empty()
                && entity.len() <= 32
                && entity
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'#')
        })
    })
}

fn contains_lrc_timestamp(text: &str) -> bool {
    text.split('[').skip(1).any(|after| {
        after
            .split_once(']')
            .is_some_and(|(tag, _)| parse_lrc_time(tag).is_ok())
    })
}

fn validate_srt_label(label: &str) -> Result<()> {
    if label.is_empty() || !label.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid("SRT cue labels must contain only ASCII digits"));
    }
    Ok(())
}

fn decimal(value: &str, minimum: usize, maximum: usize) -> Result<u64> {
    if !(minimum..=maximum).contains(&value.len())
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(
            "timestamp component has an unsupported decimal form",
        ));
    }
    value.bytes().try_fold(0_u64, |number, byte| {
        number
            .checked_mul(10)
            .and_then(|number| number.checked_add(u64::from(byte - b'0')))
            .ok_or_else(|| invalid("timestamp arithmetic overflow"))
    })
}

fn parse_lrc_time(value: &str) -> Result<u64> {
    let (minutes, remainder) = value
        .split_once(':')
        .ok_or_else(|| invalid("LRC requires mm:ss.cc timestamps"))?;
    let (seconds, centiseconds) = remainder
        .split_once('.')
        .ok_or_else(|| invalid("LRC requires exactly two centisecond digits"))?;
    let minutes = decimal(minutes, 2, 4)?;
    let seconds = decimal(seconds, 2, 2)?;
    let centiseconds = decimal(centiseconds, 2, 2)?;
    if seconds >= 60 {
        return Err(invalid("LRC seconds must be below 60"));
    }
    minutes
        .checked_mul(60_000)
        .and_then(|value| value.checked_add(seconds * 1000))
        .and_then(|value| value.checked_add(centiseconds * 10))
        .filter(|value| *value <= MAXIMUM_TIME_MILLISECONDS)
        .ok_or_else(|| invalid("LRC timestamp exceeds the 24-hour bound"))
}

fn parse_srt_time(value: &str) -> Result<u64> {
    let (hours, remainder) = value
        .split_once(':')
        .ok_or_else(|| invalid("SRT requires HH:MM:SS,mmm timestamps"))?;
    let (minutes, remainder) = remainder
        .split_once(':')
        .ok_or_else(|| invalid("SRT requires HH:MM:SS,mmm timestamps"))?;
    let (seconds, milliseconds) = remainder
        .split_once(',')
        .ok_or_else(|| invalid("SRT requires exactly three millisecond digits"))?;
    let hours = decimal(hours, 2, 2)?;
    let minutes = decimal(minutes, 2, 2)?;
    let seconds = decimal(seconds, 2, 2)?;
    let milliseconds = decimal(milliseconds, 3, 3)?;
    if minutes >= 60 || seconds >= 60 {
        return Err(invalid("SRT minutes and seconds must be below 60"));
    }
    hours
        .checked_mul(3_600_000)
        .and_then(|value| value.checked_add(minutes * 60_000))
        .and_then(|value| value.checked_add(seconds * 1000))
        .and_then(|value| value.checked_add(milliseconds))
        .filter(|value| *value <= MAXIMUM_TIME_MILLISECONDS)
        .ok_or_else(|| invalid("SRT timestamp exceeds the 24-hour bound"))
}

fn format_srt_time(value: u64) -> String {
    format!(
        "{:02}:{:02}:{:02},{:03}",
        value / 3_600_000,
        value / 60_000 % 60,
        value / 1000 % 60,
        value % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_preserves_unicode_spaces_and_complete_multiline_payload() {
        let input = "  Héllo 雪\n\n  literal <b>text</b> &amp;\t\n";
        let parsed = parse_plain(input).unwrap();
        assert_eq!(parsed.cues.len(), 1);
        assert_eq!(parsed.cues[0].text, input);
        assert!(matches!(parsed.cues[0].timing, CueTiming::Untimed {}));
        assert_eq!(render_plain(&parsed).unwrap(), input);
    }

    #[test]
    fn lrc_preserves_points_payload_and_order_without_inferred_ends() {
        let input = "[ar:  Auteur 雪  ]\n[ti:Title: subtitle]\n[00:12.34]  Héllo  \n[00:01.00]earlier\n[00:12.34]same point\n";
        let parsed = parse_lrc(input).unwrap();
        assert_eq!(parsed.metadata["ar"], "  Auteur 雪  ");
        assert_eq!(parsed.metadata["ti"], "Title: subtitle");
        assert_eq!(parsed.cues[0].text, "  Héllo  ");
        for (cue, expected) in parsed.cues.iter().zip([12340, 1000, 12340]) {
            let CueTiming::Point { at } = cue.timing else {
                panic!("expected exact point")
            };
            assert_eq!(milliseconds(at).unwrap(), expected);
        }
        let rendered = render_lrc(&parsed).unwrap();
        assert_eq!(
            render_lrc(&parse_lrc(&rendered).unwrap()).unwrap(),
            rendered
        );
        assert!(rendered.contains("[00:12.34]  Héllo  \n[00:01.00]earlier"));
    }

    #[test]
    fn srt_preserves_numeric_labels_multiline_spaces_overlap_and_source_order() {
        let input = "0009\n00:00:05,000 --> 00:00:08,001\n  Héllo 雪  \n\tsecond line\n\n2\n00:00:04,123 --> 00:00:06,000\nlater in source\n";
        let parsed = parse_srt(input).unwrap();
        assert_eq!(parsed.cues.len(), 2);
        assert_eq!(parsed.cues[0].source_label.as_deref(), Some("0009"));
        assert_eq!(parsed.cues[0].text, "  Héllo 雪  \n\tsecond line");
        let rendered = render_srt(&parsed).unwrap();
        assert_eq!(rendered, input);
        assert_eq!(render_srt(&parse_srt(&rendered).unwrap()).unwrap(), input);
    }

    #[test]
    fn repeated_numeric_source_labels_remain_authored_text() {
        let input = "007\n00:00:01,000 --> 00:00:02,000\nfirst\n\n007\n00:00:02,000 --> 00:00:03,000\nsecond\n";
        let parsed = parse_srt(input).unwrap();
        assert_eq!(parsed.cues[0].source_label, parsed.cues[1].source_label);
        assert_eq!(render_srt(&parsed).unwrap(), input);
    }

    #[test]
    fn lrc_refuses_unknown_extensions_offsets_and_precision_coercion() {
        for input in [
            "[offset:100]\n[00:01.00]text",
            "[lang:en]\n[00:01.00]text",
            "[00:01.00][00:02.00]text",
            "[00:01.00]<00:01.20>word",
            "[00:01.000]text",
            "[00:01]text",
            "[0:01.00]text",
            "[00:60.00]text",
            "[1440:00.01]text",
            "[999999999999999999999:00.00]text",
            "[00:01.00]first\n[ar:late]",
            "[ar:first]\n[ar:second]\n[00:01.00]text",
            "[ar:name]suffix\n[00:01.00]text",
            "[00:01.00]text\n\n",
        ] {
            assert!(
                parse_lrc(input).is_err(),
                "accepted unsupported LRC: {input}"
            );
        }
    }

    #[test]
    fn srt_refuses_markup_unknown_settings_and_invalid_intervals() {
        for payload in [
            "<i>styled</i>",
            "{\\an8}positioned",
            "one\\Ntwo",
            "&amp;",
            "&#32;",
        ] {
            assert!(parse_srt(&format!("1\n00:00:01,000 --> 00:00:02,000\n{payload}")).is_err());
        }
        for timing in [
            "00:00:01.000 --> 00:00:02.000",
            "00:00:01,000-->00:00:02,000",
            "00:00:01,000 --> 00:00:02,000 X1:0",
            "00:00:02,000 --> 00:00:02,000",
            "00:00:03,000 --> 00:00:02,000",
            "00:00:01,000 --> 24:00:00,001",
            "00:00:01,000 --> 00:60:00,000",
            "00:00:01,000 --> 00:00:60,000",
            "00:00:01,00 --> 00:00:02,000",
            "999999999:00:01,000 --> 00:00:02,000",
        ] {
            assert!(
                parse_srt(&format!("1\n{timing}\ntext")).is_err(),
                "accepted interval: {timing}"
            );
        }
        assert!(parse_srt("label\n00:00:01,000 --> 00:00:02,000\ntext").is_err());
        assert!(parse_srt("1\n00:00:01,000 --> 00:00:02,000\n").is_err());
    }

    #[test]
    fn writers_refuse_unprojected_information_or_timing_loss() {
        let mut point = parse_lrc("[00:01.00]text").unwrap();
        assert!(render_plain(&point).is_err());
        assert!(render_srt(&point).is_err());
        point.cues[0].timing = CueTiming::Point {
            at: time_from_milliseconds(1001),
        };
        assert!(render_lrc(&point).is_err());
        let interval = parse_srt("0001\n00:00:01,000 --> 00:00:02,000\ntext").unwrap();
        assert!(render_lrc(&interval).is_err());
        let mut plain = parse_plain("text").unwrap();
        plain.language = Some("en".to_owned());
        assert!(render_plain(&plain).is_err());
    }

    #[test]
    fn exact_boundaries_and_payload_limits_are_enforced() {
        assert_eq!(parse_lrc_time("1440:00.00").unwrap(), 86_400_000);
        assert_eq!(parse_srt_time("24:00:00,000").unwrap(), 86_400_000);
        assert!(parse_srt("1\n23:59:59,999 --> 24:00:00,000\nx").is_ok());
        assert!(parse_plain(&"x".repeat(MAXIMUM_CUE_BYTES)).is_ok());
        assert!(parse_plain(&"x".repeat(MAXIMUM_CUE_BYTES + 1)).is_err());
        assert!(parse_lrc(&"[00:01.00]x\n".repeat(MAXIMUM_CUES)).is_ok());
        assert!(parse_lrc(&"[00:01.00]x\n".repeat(MAXIMUM_CUES + 1)).is_err());
        assert!(parse_plain("nul\0payload").is_err());
        assert!(parse_plain("").is_err());
    }
}
