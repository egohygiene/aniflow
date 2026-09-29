//! Closed WebVTT and TTML profiles. Unsupported structure is refused, never stripped.
//!
//! Parsing owns syntax only. The caller supplies already normalized physical line
//! endings and records lexical normalization separately from preserved cue text.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Document, Node, ParsingOptions};

use super::{
    CueTiming, ParsedCue, ParsedText, TimedTextDocument, invalid, milliseconds,
    time_from_milliseconds,
};
use crate::Result;

const MAXIMUM_BYTES: usize = 1024 * 1024;
const MAXIMUM_CUES: usize = 10_000;
const MAXIMUM_CUE_BYTES: usize = 64 * 1024;
const MAXIMUM_MILLISECONDS: u64 = 24 * 60 * 60 * 1000;
const TTML_NAMESPACE: &str = "http://www.w3.org/ns/ttml";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const TTML_ID_KEYS: [&str; 3] = ["ttml.tt.id", "ttml.body.id", "ttml.div.id"];

fn check_input(input: &str) -> Result<()> {
    if input.len() > MAXIMUM_BYTES || input.is_empty() || input.contains('\0') {
        return Err(invalid(
            "markup input must be nonempty UTF-8 no larger than 1 MiB",
        ));
    }
    Ok(())
}

fn cue_text(text: &str) -> Result<()> {
    if text.is_empty()
        || text.len() > MAXIMUM_CUE_BYTES
        || text
            .chars()
            .any(|value| value.is_control() && !matches!(value, '\n' | '\r' | '\t'))
    {
        return Err(invalid(
            "cue text must be nonempty, bounded, and free of unsupported controls",
        ));
    }
    Ok(())
}

fn append(output: &mut String, value: &str) -> Result<()> {
    if output.len().saturating_add(value.len()) > MAXIMUM_BYTES {
        return Err(invalid("serialized timed text exceeds 1 MiB"));
    }
    output.push_str(value);
    Ok(())
}

fn digits(value: &str) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid("timestamp requires unsigned decimal digits"));
    }
    value
        .parse()
        .map_err(|_| invalid("timestamp integer exceeds its bound"))
}

fn interval(start: u64, end: u64) -> Result<CueTiming> {
    if start >= end || end > MAXIMUM_MILLISECONDS {
        return Err(invalid(
            "cue interval must be increasing and remain within 24 hours",
        ));
    }
    Ok(CueTiming::Interval {
        start: time_from_milliseconds(start),
        end: time_from_milliseconds(end),
    })
}

fn interval_milliseconds(timing: &CueTiming) -> Result<(u64, u64)> {
    let CueTiming::Interval { start, end } = timing else {
        return Err(invalid(
            "this timed-text format requires explicit interval timing",
        ));
    };
    let start = milliseconds(*start)?;
    let end = milliseconds(*end)?;
    interval(start, end)?;
    Ok((start, end))
}

fn clock(value: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        value / 3_600_000,
        value / 60_000 % 60,
        value / 1000 % 60,
        value % 1000
    )
}

fn webvtt_time(value: &str) -> Result<u64> {
    let fields: Vec<_> = value.split(':').collect();
    let (hours, minutes, seconds) = match fields.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] if hours.len() >= 2 => (digits(hours)?, *minutes, *seconds),
        _ => {
            return Err(invalid(
                "WebVTT timestamp requires mm:ss.mmm or hh:mm:ss.mmm",
            ));
        }
    };
    let (seconds, fraction) = seconds
        .split_once('.')
        .ok_or_else(|| invalid("WebVTT timestamp requires millisecond precision"))?;
    if minutes.len() != 2 || seconds.len() != 2 || fraction.len() != 3 {
        return Err(invalid(
            "WebVTT timestamp fields have unsupported precision",
        ));
    }
    let minutes = digits(minutes)?;
    let seconds = digits(seconds)?;
    let fraction = digits(fraction)?;
    if hours > 24 || minutes > 59 || seconds > 59 {
        return Err(invalid("WebVTT timestamp fields are out of range"));
    }
    let result = hours * 3_600_000 + minutes * 60_000 + seconds * 1000 + fraction;
    if result > MAXIMUM_MILLISECONDS {
        return Err(invalid("WebVTT timestamp exceeds 24 hours"));
    }
    Ok(result)
}

fn reserved_webvtt(value: &str) -> bool {
    ["NOTE", "STYLE", "REGION"].iter().any(|reserved| {
        value == *reserved
            || value
                .strip_prefix(reserved)
                .is_some_and(|suffix| suffix.starts_with([' ', '\t']))
    })
}

pub(super) fn webvtt_label(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAXIMUM_CUE_BYTES
        || value.contains("-->")
        || value.chars().any(|value| value.is_control())
        || reserved_webvtt(value)
    {
        return Err(invalid("WebVTT cue identifier is unsupported or reserved"));
    }
    Ok(())
}

pub(super) fn webvtt_speaker(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAXIMUM_CUE_BYTES
        || value.starts_with(char::is_whitespace)
        || value.ends_with(char::is_whitespace)
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '<' | '>' | '&'))
    {
        return Err(invalid(
            "WebVTT speaker annotation is outside the plain whole-cue profile",
        ));
    }
    Ok(())
}

fn decode_webvtt_text(input: &str) -> Result<String> {
    if input.contains(['<', '>']) {
        return Err(invalid(
            "WebVTT cue markup is unsupported; literal brackets must be escaped",
        ));
    }
    let mut output = String::new();
    let mut remaining = input;
    while let Some(position) = remaining.find('&') {
        output.push_str(&remaining[..position]);
        remaining = &remaining[position..];
        let end = remaining
            .find(';')
            .ok_or_else(|| invalid("WebVTT entity is incomplete"))?;
        let decoded = match &remaining[..=end] {
            "&amp;" => '&',
            "&lt;" => '<',
            "&gt;" => '>',
            "&nbsp;" => '\u{a0}',
            "&lrm;" => '\u{200e}',
            "&rlm;" => '\u{200f}',
            _ => {
                return Err(invalid(
                    "WebVTT entity is outside the supported named-entity set",
                ));
            }
        };
        output.push(decoded);
        remaining = &remaining[end + 1..];
    }
    output.push_str(remaining);
    cue_text(&output)?;
    Ok(output)
}

fn webvtt_payload(input: &str) -> Result<(String, Option<String>)> {
    if input.contains("-->") || input.contains('\r') {
        return Err(invalid(
            "WebVTT cue text contains unsupported timing syntax or line endings",
        ));
    }
    if let Some(annotation) = input.strip_prefix("<v ") {
        let (speaker, remainder) = annotation
            .split_once('>')
            .ok_or_else(|| invalid("WebVTT voice annotation is incomplete"))?;
        webvtt_speaker(speaker)?;
        let text = remainder.strip_suffix("</v>").ok_or_else(|| {
            invalid("WebVTT voice annotation must cover the entire explicitly closed cue")
        })?;
        return Ok((decode_webvtt_text(text)?, Some(speaker.to_owned())));
    }
    Ok((decode_webvtt_text(input)?, None))
}

pub(super) fn parse_webvtt(input: &str) -> Result<ParsedText> {
    check_input(input)?;
    if input.contains('\r') {
        return Err(invalid(
            "WebVTT parser requires normalized physical line endings",
        ));
    }
    let lines: Vec<_> = input.split('\n').collect();
    if lines.first() != Some(&"WEBVTT") || lines.get(1) != Some(&"") {
        return Err(invalid(
            "WebVTT requires the plain WEBVTT header and a blank separator",
        ));
    }
    let mut position = 2;
    let mut cues = Vec::new();
    let mut identifiers = BTreeSet::new();
    while position < lines.len() {
        if lines[position].is_empty() {
            position += 1;
            continue;
        }
        let begin = position;
        while position < lines.len() && !lines[position].is_empty() {
            position += 1;
        }
        let block = &lines[begin..position];
        if reserved_webvtt(block[0]) {
            return Err(invalid(
                "WebVTT NOTE, STYLE, and REGION blocks are unsupported",
            ));
        }
        let (source_label, timing_index) = if block[0].contains("-->") {
            (None, 0)
        } else {
            webvtt_label(block[0])?;
            if !identifiers.insert(block[0]) {
                return Err(invalid("WebVTT cue identifiers must be unique"));
            }
            (Some(block[0].to_owned()), 1)
        };
        let timing = block
            .get(timing_index)
            .ok_or_else(|| invalid("WebVTT cue has no interval"))?;
        let fields: Vec<_> = timing.split_ascii_whitespace().collect();
        if fields.len() != 3 || fields[1] != "-->" {
            return Err(invalid(
                "WebVTT requires explicit intervals without cue settings",
            ));
        }
        let timing = interval(webvtt_time(fields[0])?, webvtt_time(fields[2])?)?;
        let payload = block
            .get(timing_index + 1..)
            .ok_or_else(|| invalid("WebVTT cue has no text"))?
            .join("\n");
        let (text, speaker) = webvtt_payload(&payload)?;
        if cues.len() >= MAXIMUM_CUES {
            return Err(invalid("WebVTT exceeds 10000 cues"));
        }
        cues.push(ParsedCue {
            source_label,
            text,
            timing,
            speaker,
        });
    }
    if cues.is_empty() {
        return Err(invalid("WebVTT must contain at least one supported cue"));
    }
    Ok(ParsedText {
        cues,
        language: None,
        metadata: BTreeMap::new(),
    })
}

fn escape_webvtt(text: &str) -> String {
    let mut output = String::new();
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
    output
}

pub(super) fn write_webvtt(document: &TimedTextDocument) -> Result<String> {
    if document.language.is_some() || !document.metadata.is_empty() {
        return Err(invalid(
            "plain WebVTT cannot encode document language or metadata",
        ));
    }
    if document.cues.is_empty() || document.cues.len() > MAXIMUM_CUES {
        return Err(invalid("WebVTT requires between 1 and 10000 cues"));
    }
    let mut output = "WEBVTT\n\n".to_owned();
    let mut identifiers = BTreeSet::new();
    for cue in &document.cues {
        cue_text(&cue.text)?;
        if cue.text.split('\n').any(str::is_empty)
            || cue.text.contains("-->")
            || cue.text.contains('\r')
        {
            return Err(invalid(
                "WebVTT cannot encode empty cue-text lines, carriage returns, or timing arrows",
            ));
        }
        if let Some(label) = &cue.source_label {
            webvtt_label(label)?;
            if !identifiers.insert(label) {
                return Err(invalid("WebVTT cue identifiers must be unique"));
            }
            append(&mut output, label)?;
            append(&mut output, "\n")?;
        }
        let (start, end) = interval_milliseconds(&cue.timing)?;
        append(
            &mut output,
            &format!("{} --> {}\n", clock(start), clock(end)),
        )?;
        if let Some(speaker) = &cue.speaker {
            webvtt_speaker(speaker)?;
            append(&mut output, &format!("<v {speaker}>"))?;
        }
        append(&mut output, &escape_webvtt(&cue.text))?;
        if cue.speaker.is_some() {
            append(&mut output, "</v>")?;
        }
        append(&mut output, "\n\n")?;
    }
    Ok(output)
}

fn decimal_milliseconds(value: &str, multiplier: u64) -> Result<u64> {
    if value.is_empty() || value.len() > 64 {
        return Err(invalid("TTML decimal time is empty or oversized"));
    }
    let (integer, fraction) = match value.split_once('.') {
        Some((integer, fraction)) if !fraction.is_empty() => (integer, fraction),
        Some(_) => return Err(invalid("TTML fractional time is incomplete")),
        None => (value, ""),
    };
    if integer.is_empty()
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("TTML time requires unsigned decimal notation"));
    }
    let mut numerator = 0_u128;
    for byte in integer.bytes().chain(fraction.bytes()) {
        numerator = numerator
            .checked_mul(10)
            .and_then(|value| value.checked_add(u128::from(byte - b'0')))
            .ok_or_else(|| invalid("TTML decimal time exceeds exact arithmetic bounds"))?;
    }
    let scale = 10_u128
        .checked_pow(fraction.len() as u32)
        .ok_or_else(|| invalid("TTML decimal time has unsupported precision"))?;
    let scaled = numerator
        .checked_mul(u128::from(multiplier))
        .ok_or_else(|| invalid("TTML decimal time exceeds exact arithmetic bounds"))?;
    if scaled % scale != 0 || scaled / scale > u128::from(MAXIMUM_MILLISECONDS) {
        return Err(invalid(
            "TTML time must be exact milliseconds within 24 hours",
        ));
    }
    u64::try_from(scaled / scale).map_err(|_| invalid("TTML timestamp exceeds its bound"))
}

fn ttml_time(value: &str) -> Result<u64> {
    if value.contains(':') {
        let fields: Vec<_> = value.split(':').collect();
        let [hours, minutes, seconds] = fields.as_slice() else {
            return Err(invalid(
                "TTML clock time requires hours, minutes, and seconds",
            ));
        };
        let whole_seconds = seconds.split('.').next().unwrap_or_default();
        if hours.len() < 2 || minutes.len() != 2 || whole_seconds.len() != 2 {
            return Err(invalid("TTML clock fields have unsupported widths"));
        }
        let hours = digits(hours)?;
        let minutes = digits(minutes)?;
        let seconds = decimal_milliseconds(seconds, 1000)?;
        if hours > 24 || minutes > 59 || seconds >= 60_000 {
            return Err(invalid("TTML clock fields are out of range"));
        }
        let result = hours * 3_600_000 + minutes * 60_000 + seconds;
        if result > MAXIMUM_MILLISECONDS {
            return Err(invalid("TTML clock exceeds 24 hours"));
        }
        return Ok(result);
    }
    for (unit, multiplier) in [("ms", 1), ("h", 3_600_000), ("m", 60_000), ("s", 1000)] {
        if let Some(decimal) = value.strip_suffix(unit) {
            return decimal_milliseconds(decimal, multiplier);
        }
    }
    Err(invalid(
        "TTML permits only exact media-time clocks and h/m/s/ms offsets",
    ))
}

fn xml_name_start(value: char) -> bool {
    value == '_'
        || value.is_ascii_alphabetic()
        || matches!(value as u32,
            0xc0..=0xd6 | 0xd8..=0xf6 | 0xf8..=0x2ff | 0x370..=0x37d |
            0x37f..=0x1fff | 0x200c..=0x200d | 0x2070..=0x218f |
            0x2c00..=0x2fef | 0x3001..=0xd7ff | 0xf900..=0xfdcf |
            0xfdf0..=0xfffd | 0x10000..=0xeffff)
}

pub(super) fn xml_id(value: &str) -> Result<()> {
    let mut characters = value.chars();
    if value.len() > MAXIMUM_CUE_BYTES
        || !characters.next().is_some_and(xml_name_start)
        || characters.any(|character| {
            !xml_name_start(character)
                && !character.is_ascii_digit()
                && !matches!(character, '-' | '.' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')
        })
    {
        return Err(invalid("TTML xml:id must be an XML non-colon name"));
    }
    Ok(())
}

fn xml_language(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
    {
        return Err(invalid(
            "TTML language must be a bounded plain language tag",
        ));
    }
    Ok(())
}

fn element(node: Node<'_, '_>, expected: &str) -> Result<()> {
    if !node.is_element()
        || node.tag_name().namespace() != Some(TTML_NAMESPACE)
        || node.tag_name().name() != expected
    {
        return Err(invalid(
            "TTML supports only the flat tt/body/div/p/br namespace profile",
        ));
    }
    Ok(())
}

fn element_children<'a, 'input>(node: Node<'a, 'input>) -> Result<Vec<Node<'a, 'input>>> {
    let mut children = Vec::new();
    for child in node.children() {
        if child.is_element() {
            children.push(child);
        } else if !child.is_text()
            || !child
                .text()
                .unwrap_or_default()
                .chars()
                .all(|value| matches!(value, ' ' | '\t' | '\n' | '\r'))
        {
            return Err(invalid("TTML has unsupported content outside a cue"));
        }
    }
    Ok(children)
}

fn attributes(
    node: Node<'_, '_>,
    name: &str,
    language: Option<&str>,
    identifiers: &mut BTreeSet<String>,
) -> Result<Option<String>> {
    let mut identifier = None;
    for attribute in node.attributes() {
        match (attribute.namespace(), attribute.name()) {
            (Some(XML_NAMESPACE), "id") if name != "br" => {
                xml_id(attribute.value())?;
                if !identifiers.insert(attribute.value().to_owned()) {
                    return Err(invalid(
                        "TTML xml:id values must be unique throughout the document",
                    ));
                }
                identifier = Some(attribute.value().to_owned());
            }
            (Some(XML_NAMESPACE), "space") if name != "br" && attribute.value() == "preserve" => {}
            (Some(XML_NAMESPACE), "lang") if name != "br" => {
                let value = (!attribute.value().is_empty()).then_some(attribute.value());
                if let Some(value) = value {
                    xml_language(value)?;
                }
                if value != language {
                    return Err(invalid(
                        "TTML mixed or overridden cue languages are unsupported",
                    ));
                }
            }
            (None, "begin" | "end") if name == "p" => {}
            _ => {
                return Err(invalid(
                    "TTML attribute, styling, resource, or timing mapping is unsupported",
                ));
            }
        }
    }
    Ok(identifier)
}

fn xml_space(value: char) -> bool {
    matches!(value, ' ' | '\t' | '\n' | '\r')
}

// roxmltree validates the declaration's syntax but intentionally ignores its
// values. Our UTF-8, XML 1.0 profile must not silently accept another encoding
// or XML version whose semantics the parser does not implement.
fn xml_declaration(input: &str) -> Result<()> {
    let Some(rest) = input
        .strip_prefix("<?xml")
        .filter(|rest| rest.starts_with(xml_space))
    else {
        return Ok(());
    };
    let (mut declaration, _) = rest
        .split_once("?>")
        .ok_or_else(|| invalid("TTML XML declaration is incomplete"))?;
    if declaration.len() > 256 {
        return Err(invalid("TTML XML declaration exceeds its bound"));
    }
    let mut fields = Vec::new();
    while !declaration.trim_matches(xml_space).is_empty() {
        if !declaration.starts_with(xml_space) || fields.len() >= 3 {
            return Err(invalid("TTML XML declaration has unsupported fields"));
        }
        declaration = declaration.trim_start_matches(xml_space);
        let (name, rest) = declaration
            .split_once('=')
            .ok_or_else(|| invalid("TTML XML declaration field is incomplete"))?;
        let rest = rest.trim_start_matches(xml_space);
        let quote = rest
            .chars()
            .next()
            .filter(|quote| matches!(quote, '\'' | '"'))
            .ok_or_else(|| invalid("TTML XML declaration values must be quoted"))?;
        let (value, rest) = rest[1..]
            .split_once(quote)
            .ok_or_else(|| invalid("TTML XML declaration value is incomplete"))?;
        fields.push((name.trim_end_matches(xml_space), value));
        declaration = rest;
    }
    let mut fields = fields.into_iter();
    if fields.next() != Some(("version", "1.0")) {
        return Err(invalid("TTML requires XML version 1.0"));
    }
    let mut next = fields.next();
    if let Some(("encoding", value)) = next {
        if !value.eq_ignore_ascii_case("UTF-8") {
            return Err(invalid("TTML supports only declared UTF-8 encoding"));
        }
        next = fields.next();
    }
    if let Some(("standalone", "yes" | "no")) = next {
        next = fields.next();
    }
    if next.is_some() || fields.next().is_some() {
        return Err(invalid("TTML XML declaration has unsupported values"));
    }
    Ok(())
}

pub(super) fn parse_ttml(input: &str) -> Result<ParsedText> {
    check_input(input)?;
    if input.starts_with('\u{feff}') {
        return Err(invalid(
            "TTML parser requires the single optional BOM to be normalized by the caller",
        ));
    }
    xml_declaration(input)?;
    if input.contains("<!DOCTYPE") || input.contains("<!ENTITY") {
        return Err(invalid(
            "TTML document type and entity declarations are forbidden",
        ));
    }
    // roxmltree recognizes declarations only with a literal space after xml.
    // XML permits any XML whitespace there. The declaration was validated above;
    // normalize only that syntax separator without touching document content.
    let declaration_normalized;
    let input = if input.starts_with("<?xml")
        && input
            .as_bytes()
            .get(5)
            .is_some_and(|byte| matches!(byte, b'\t' | b'\n' | b'\r'))
    {
        declaration_normalized = format!("<?xml {}", &input[6..]);
        declaration_normalized.as_str()
    } else {
        input
    };
    let document = Document::parse_with_options(
        input,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 60_000,
        },
    )
    .map_err(|_| invalid("TTML is malformed XML or exceeds its node limit"))?;
    for node in document.descendants() {
        if node.is_pi() || node.is_comment() || node.ancestors().count() > 6 {
            return Err(invalid(
                "TTML processing instructions, comments, or nested structures are unsupported",
            ));
        }
        if node.is_element()
            && (node.attributes().len() > 8
                || node.namespaces().count() > 8
                || node
                    .namespaces()
                    .any(|namespace| ![TTML_NAMESPACE, XML_NAMESPACE].contains(&namespace.uri())))
        {
            return Err(invalid(
                "TTML foreign namespaces or excessive attributes are unsupported",
            ));
        }
    }
    let root = document.root_element();
    element(root, "tt")?;
    if root.attribute((XML_NAMESPACE, "space")) != Some("preserve") {
        return Err(invalid("TTML requires explicit root xml:space=preserve"));
    }
    let language = root
        .attribute((XML_NAMESPACE, "lang"))
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let mut identifiers = BTreeSet::new();
    let mut metadata = BTreeMap::new();
    if let Some(id) = attributes(root, "tt", language.as_deref(), &mut identifiers)? {
        metadata.insert(TTML_ID_KEYS[0].to_owned(), id);
    }
    let body = element_children(root)?;
    if body.len() != 1 {
        return Err(invalid("TTML requires exactly one body"));
    }
    let body = body[0];
    element(body, "body")?;
    if let Some(id) = attributes(body, "body", language.as_deref(), &mut identifiers)? {
        metadata.insert(TTML_ID_KEYS[1].to_owned(), id);
    }
    let division = element_children(body)?;
    if division.len() != 1 {
        return Err(invalid("TTML requires exactly one flat div"));
    }
    let division = division[0];
    element(division, "div")?;
    if let Some(id) = attributes(division, "div", language.as_deref(), &mut identifiers)? {
        metadata.insert(TTML_ID_KEYS[2].to_owned(), id);
    }
    let paragraphs = element_children(division)?;
    if paragraphs.is_empty() || paragraphs.len() > MAXIMUM_CUES {
        return Err(invalid("TTML requires between 1 and 10000 cues"));
    }
    let mut cues = Vec::new();
    for paragraph in paragraphs {
        element(paragraph, "p")?;
        let source_label = attributes(paragraph, "p", language.as_deref(), &mut identifiers)?;
        let begin = paragraph
            .attribute("begin")
            .ok_or_else(|| invalid("TTML cue requires an explicit begin"))?;
        let end = paragraph
            .attribute("end")
            .ok_or_else(|| invalid("TTML cue requires an explicit end"))?;
        let timing = interval(ttml_time(begin)?, ttml_time(end)?)?;
        let mut text = String::new();
        for child in paragraph.children() {
            if child.is_text() {
                text.push_str(child.text().unwrap_or_default());
            } else {
                element(child, "br")?;
                if child.attributes().len() != 0 || child.children().next().is_some() {
                    return Err(invalid("TTML br must be empty and have no attributes"));
                }
                text.push('\n');
            }
            if text.len() > MAXIMUM_CUE_BYTES {
                return Err(invalid("TTML cue text exceeds 64 KiB"));
            }
        }
        cue_text(&text)?;
        cues.push(ParsedCue {
            source_label,
            text,
            timing,
            speaker: None,
        });
    }
    Ok(ParsedText {
        cues,
        language,
        metadata,
    })
}

fn escape_xml(value: &str, attribute: bool) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if attribute => output.push_str("&quot;"),
            '\r' => output.push_str("&#13;"),
            '\n' if !attribute => output.push_str("<br/>"),
            '\n' => output.push_str("&#10;"),
            '\t' if attribute => output.push_str("&#9;"),
            _ => output.push(character),
        }
    }
    output
}

fn write_identifier(
    output: &mut String,
    identifier: Option<&str>,
    identifiers: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(identifier) = identifier {
        xml_id(identifier)?;
        if !identifiers.insert(identifier.to_owned()) {
            return Err(invalid(
                "TTML xml:id values must be unique throughout the document",
            ));
        }
        append(
            output,
            &format!(" xml:id=\"{}\"", escape_xml(identifier, true)),
        )?;
    }
    Ok(())
}

pub(super) fn write_ttml(document: &TimedTextDocument) -> Result<String> {
    if document.cues.is_empty() || document.cues.len() > MAXIMUM_CUES {
        return Err(invalid("TTML requires between 1 and 10000 cues"));
    }
    if document
        .metadata
        .keys()
        .any(|key| !TTML_ID_KEYS.contains(&key.as_str()))
    {
        return Err(invalid("TTML cannot encode undeclared document metadata"));
    }
    let mut output = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<tt xmlns=\"{TTML_NAMESPACE}\" xml:space=\"preserve\""
    );
    if let Some(language) = &document.language {
        xml_language(language)?;
        append(
            &mut output,
            &format!(" xml:lang=\"{}\"", escape_xml(language, true)),
        )?;
    } else {
        append(&mut output, " xml:lang=\"\"")?;
    }
    let mut identifiers = BTreeSet::new();
    write_identifier(
        &mut output,
        document.metadata.get(TTML_ID_KEYS[0]).map(String::as_str),
        &mut identifiers,
    )?;
    append(&mut output, ">\n<body")?;
    write_identifier(
        &mut output,
        document.metadata.get(TTML_ID_KEYS[1]).map(String::as_str),
        &mut identifiers,
    )?;
    append(&mut output, ">\n<div")?;
    write_identifier(
        &mut output,
        document.metadata.get(TTML_ID_KEYS[2]).map(String::as_str),
        &mut identifiers,
    )?;
    append(&mut output, ">\n")?;
    for cue in &document.cues {
        cue_text(&cue.text)?;
        if cue.speaker.is_some() {
            return Err(invalid("flat TTML cannot encode speaker annotations"));
        }
        if cue.text.chars().any(|character| {
            !matches!(character as u32, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
        }) {
            return Err(invalid("TTML cue text contains a character forbidden by XML 1.0"));
        }
        let (start, end) = interval_milliseconds(&cue.timing)?;
        append(
            &mut output,
            &format!("<p begin=\"{}\" end=\"{}\"", clock(start), clock(end)),
        )?;
        write_identifier(&mut output, cue.source_label.as_deref(), &mut identifiers)?;
        append(&mut output, ">")?;
        append(&mut output, &escape_xml(&cue.text, false))?;
        append(&mut output, "</p>\n")?;
    }
    append(&mut output, "</div>\n</body>\n</tt>\n")?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::{OverlapPolicy, TIMED_TEXT_SCHEMA_V1, TextProvenance, TimedTextCue};
    use super::*;
    use crate::audio_analysis::AudioArtifactReference;

    fn document(parsed: ParsedText) -> TimedTextDocument {
        TimedTextDocument {
            schema: TIMED_TEXT_SCHEMA_V1.into(),
            source: AudioArtifactReference {
                id: "source_text".into(),
                sha256: "a".repeat(64),
                byte_size: 100,
            },
            provenance: TextProvenance::Unreviewed {},
            language: parsed.language,
            audio_source: None,
            overlap_policy: OverlapPolicy::Reject,
            metadata: parsed.metadata,
            cues: parsed
                .cues
                .into_iter()
                .enumerate()
                .map(|(index, cue)| TimedTextCue {
                    id: format!("cue-{index}"),
                    source_label: cue.source_label,
                    text: cue.text,
                    timing: cue.timing,
                    speaker: cue.speaker,
                })
                .collect(),
        }
    }

    fn ttml(paragraph: &str) -> String {
        format!(
            "<tt xmlns=\"{TTML_NAMESPACE}\" xml:space=\"preserve\"><body><div>{paragraph}</div></body></tt>"
        )
    }

    #[test]
    fn webvtt_preserves_unicode_whitespace_entities_and_whole_cue_speaker() {
        let parsed = parse_webvtt("WEBVTT\n\ncue雪\n00:01.002 --> 00:03.004\n<v Speaker 雪>  café &amp; e\u{301}\nsecond &lt;line&gt; </v>\n\n").unwrap();
        assert_eq!(parsed.cues[0].source_label.as_deref(), Some("cue雪"));
        assert_eq!(parsed.cues[0].speaker.as_deref(), Some("Speaker 雪"));
        assert_eq!(parsed.cues[0].text, "  café & e\u{301}\nsecond <line> ");
        assert_eq!(
            interval_milliseconds(&parsed.cues[0].timing).unwrap(),
            (1002, 3004)
        );
    }

    #[test]
    fn webvtt_refuses_unsupported_blocks_settings_markup_and_ambiguity() {
        for cue in [
            "NOTE comment\nunsupported",
            "STYLE\n::cue {color:red}",
            "REGION\nid:region",
            "00:00.000 --> 00:01.000 align:start\ntext",
            "00:00.000 --> 00:01.000\n<b>text</b>",
            "00:00.000 --> 00:01.000\n<v Speaker>text",
            "00:00.000 --> 00:01.000\n&unknown;",
            "00:00.000 --> 00:01.000\n&#65;",
            "00:00.000 --> 00:01.000\ntext\n<v Another>nested</v>",
            "00:00.000 --> 00:00.000\ntext",
            "00:00.000 --> 24:00:00.001\ntext",
            "same\n00:00.000 --> 00:01.000\nfirst\n\nsame\n00:01.000 --> 00:02.000\nsecond",
        ] {
            assert!(
                parse_webvtt(&format!("WEBVTT\n\n{cue}\n")).is_err(),
                "{cue}"
            );
        }
        assert!(parse_webvtt("WEBVTT header annotation\n\n00:00.000 --> 00:01.000\ntext").is_err());
    }

    #[test]
    fn ttml_preserves_xml_text_and_exact_decimal_media_times() {
        let parsed = parse_ttml(&ttml("<p xml:id=\"cue雪\" begin=\"1.002s\" end=\"0.05m\">  café &amp; e&#x301;<br/>tail&#13; </p>")).unwrap();
        assert_eq!(parsed.cues[0].source_label.as_deref(), Some("cue雪"));
        assert_eq!(parsed.cues[0].text, "  café & e\u{301}\ntail\r ");
        assert_eq!(
            interval_milliseconds(&parsed.cues[0].timing).unwrap(),
            (1002, 3000)
        );
        assert_eq!(ttml_time("00:00:01.002000").unwrap(), 1002);
        assert_eq!(ttml_time("0.00001h").unwrap(), 36);
        assert!(ttml_time("0.0001s").is_err());
        assert!(ttml_time("1f").is_err());
        assert!(ttml_time("00:00:01:15").is_err());
    }

    #[test]
    fn ttml_refuses_foreign_resources_dtd_and_unmodeled_structure() {
        let good = ttml("<p begin=\"0s\" end=\"1s\">text</p>");
        for source in [
            format!("<!DOCTYPE tt>{good}"),
            format!("<!DOCTYPE tt SYSTEM \"https://invalid.example/schema.dtd\">{good}"),
            format!("<!DOCTYPE tt [<!ENTITY leak SYSTEM \"file:///private\">]>{good}"),
            format!("<?resource fetch=\"https://invalid.example\"?>{good}"),
            good.replace("<body>", "<body begin=\"1s\">"),
            good.replace("<div>", "<div><div>")
                .replace("</div>", "</div></div>"),
            good.replace("xml:space=\"preserve\"", "xml:space=\"default\""),
            good.replace("xml:space=\"preserve\"", ""),
            good.replace("text", "<span>text</span>"),
            good.replace("text", "<!-- comment -->text"),
            good.replace("<body>", "<head/><body>"),
            good.replace("<p begin", "<p style=\"main\" begin"),
            good.replace("<p begin", "<p xml:lang=\"fr\" begin"),
            good.replace("<p begin", "<p xmlns:x=\"https://invalid.example\" begin"),
            good.replace(TTML_NAMESPACE, "https://invalid.example/ttml"),
            good.replace("text", "<br xml:id=\"break\"/>"),
            good.replace("text", "<br>not empty</br>"),
        ] {
            assert!(parse_ttml(&source).is_err(), "{source}");
        }
    }

    #[test]
    fn ttml_retains_structural_ids_and_refuses_duplicate_ids() {
        let source = format!(
            "<tt xmlns=\"{TTML_NAMESPACE}\" xml:space=\"preserve\" xml:lang=\"en-US\" xml:id=\"root\"><body xml:id=\"body\"><div xml:id=\"division\"><p begin=\"0s\" end=\"1s\" xml:id=\"cue\">text</p></div></body></tt>"
        );
        let parsed = parse_ttml(&source).unwrap();
        assert_eq!(parsed.language.as_deref(), Some("en-US"));
        assert_eq!(parsed.metadata["ttml.tt.id"], "root");
        assert_eq!(parsed.metadata["ttml.body.id"], "body");
        assert_eq!(parsed.metadata["ttml.div.id"], "division");
        assert!(parse_ttml(&source.replace("xml:id=\"cue\"", "xml:id=\"body\"")).is_err());
        assert!(parse_ttml(&source.replace("xml:id=\"cue\"", "xml:id=\"not a name\"")).is_err());
    }

    #[test]
    fn ttml_unknown_language_and_xml_declaration_are_explicit() {
        let source = ttml("<p begin=\"0s\" end=\"1s\" xml:lang=\"\">text</p>");
        let parsed = parse_ttml(&source).unwrap();
        assert_eq!(parsed.language, None);
        let written = write_ttml(&document(parsed)).unwrap();
        assert!(written.contains("xml:lang=\"\""));
        assert_eq!(parse_ttml(&written).unwrap().language, None);
        for declaration in [
            "<?xml version='1.0'?>",
            "<?xml version = '1.0' encoding = 'utf-8' standalone='yes'?>",
            "<?xml\nversion='1.0' standalone='no' ?>",
        ] {
            assert!(
                parse_ttml(&format!("{declaration}{source}")).is_ok(),
                "{declaration}"
            );
        }
        for declaration in [
            "<?xml version='1.1'?>",
            "<?xml version='anything'?>",
            "<?xml version='1.0' encoding='ISO-8859-1'?>",
            "<?xml version='1.0' standalone='perhaps'?>",
            "<?xml version='1.0' standalone='yes' encoding='UTF-8'?>",
            "<?xml version='1.0'encoding='UTF-8'?>",
        ] {
            assert!(
                parse_ttml(&format!("{declaration}{source}")).is_err(),
                "{declaration}"
            );
        }
        assert!(
            parse_ttml(&source.replace(
                "xml:space=\"preserve\"",
                "xml:space=\"preserve\" xml:lang=\"en\""
            ))
            .is_err()
        );
    }

    #[test]
    fn markup_normalizes_exactly_one_leading_bom() {
        use super::super::{ImportContext, TimedTextFormat, decode};
        for (format, source) in [
            (
                TimedTextFormat::Webvtt,
                "WEBVTT\n\n00:00.000 --> 00:01.000\ntext".to_owned(),
            ),
            (
                TimedTextFormat::Ttml,
                ttml("<p begin=\"0s\" end=\"1s\">text</p>"),
            ),
        ] {
            let once = format!("\u{feff}{source}");
            let decoded = decode(once.as_bytes(), format, &ImportContext::default()).unwrap();
            assert!(decoded.lexical.utf8_bom_removed);
            assert!(
                decode(
                    format!("\u{feff}{once}").as_bytes(),
                    format,
                    &ImportContext::default()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn markup_bounds_and_escaping_are_explicit() {
        assert!(parse_webvtt(&"x".repeat(MAXIMUM_BYTES + 1)).is_err());
        assert!(
            parse_ttml(&ttml(&format!(
                "<p begin=\"0s\" end=\"1s\">{}</p>",
                "x".repeat(MAXIMUM_CUE_BYTES + 1)
            )))
            .is_err()
        );
        assert_eq!(
            escape_xml("&< >\"\r\n\t", false),
            "&amp;&lt; &gt;\"&#13;<br/>\t"
        );
        assert_eq!(escape_xml("&\"\n\t", true), "&amp;&quot;&#10;&#9;");
        assert_eq!(escape_webvtt("&< >"), "&amp;&lt; &gt;");
    }

    #[test]
    fn markup_writers_roundtrip_semantics_without_inventing_labels() {
        let original = document(parse_webvtt("WEBVTT\n\n00:00.000 --> 00:01.000\n<v Speaker 雪>  e\u{301} &amp; text\nline two </v>\n").unwrap());
        let written = write_webvtt(&original).unwrap();
        let parsed = parse_webvtt(&written).unwrap();
        assert_eq!(parsed.cues[0].source_label, None);
        assert_eq!(parsed.cues[0].text, original.cues[0].text);
        assert_eq!(parsed.cues[0].speaker, original.cues[0].speaker);
        assert_eq!(parsed.cues[0].timing, original.cues[0].timing);

        let source = ttml("<p xml:id=\"cue雪\" begin=\"0s\" end=\"1s\">  e&#x301; &amp; text<br/><br/>tail&#13; </p>")
            .replace("xml:space=\"preserve\"", "xml:space=\"preserve\" xml:lang=\"en\" xml:id=\"root\"");
        let original = document(parse_ttml(&source).unwrap());
        let written = write_ttml(&original).unwrap();
        let parsed = parse_ttml(&written).unwrap();
        assert_eq!(parsed.language, original.language);
        assert_eq!(parsed.metadata, original.metadata);
        assert_eq!(parsed.cues[0].source_label, original.cues[0].source_label);
        assert_eq!(parsed.cues[0].text, original.cues[0].text);
        assert_eq!(parsed.cues[0].timing, original.cues[0].timing);
    }

    #[test]
    fn markup_writers_refuse_unprojected_or_unrepresentable_semantics() {
        let original = document(parse_webvtt("WEBVTT\n\n00:00.000 --> 00:01.000\ntext\n").unwrap());
        let mut changed = original.clone();
        changed.language = Some("en".into());
        assert!(write_webvtt(&changed).is_err());
        changed = original.clone();
        changed.cues[0].text = "first\n\nlast".into();
        assert!(write_webvtt(&changed).is_err());
        assert!(write_ttml(&changed).is_ok());
        changed.cues[0].text = "forbidden \u{ffff}".into();
        assert!(write_ttml(&changed).is_err());
        changed = original.clone();
        changed.cues[0].speaker = Some("Speaker".into());
        assert!(write_ttml(&changed).is_err());
        changed = original;
        changed.cues[0].timing = CueTiming::Point {
            at: time_from_milliseconds(0),
        };
        assert!(write_webvtt(&changed).is_err());
        assert!(write_ttml(&changed).is_err());
    }
}
