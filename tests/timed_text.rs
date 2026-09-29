//! Public conversion contracts use generated text only; no media or model is read.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use aniflow::audio_analysis::{
    AudioArtifactReference, AudioRationalTime, AudioReviewedAuthority, AudioSource,
};
use aniflow::timed_text::{
    self, ConversionLossKind, ConversionOptions, CueTiming, ImportContext, OverlapPolicy,
    TextProvenance, TimedTextFormat,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

const SRT: &str = "1\n00:00:00,250 --> 00:00:01,500\nHello café 雪 🌍\n\n2\n00:00:02,000 --> 00:00:03,750\nمرحبا שלום\n";
const VTT: &str = "WEBVTT\n\n00:00:00.250 --> 00:00:01.500\nHello café 雪 🌍\n\n00:00:02.000 --> 00:00:03.750\nمرحبا שלום\n";
const TTML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><tt xmlns=\"http://www.w3.org/ns/ttml\" xml:space=\"preserve\"><body><div><p begin=\"00:00:00.250\" end=\"00:00:01.500\">Hello café 雪 🌍</p><p begin=\"00:00:02.000\" end=\"00:00:03.750\">مرحبا שלום</p></div></body></tt>";
const LRC: &str = "[00:00.25]Hello café 雪 🌍\n[00:02.00]مرحبا שלום\n";
const PLAIN: &str = "Hello café 雪 🌍\nمرحبا שלום\n";

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "timed-text"])
        .args(arguments)
        .output()
        .unwrap()
}

fn convert_cli(input: &Path, from: &str, to: &str, output: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "timed-text", "convert", "--input"])
        .arg(input)
        .args(["--from", from, "--to", to, "--output-directory"])
        .arg(output)
        .args(extra)
        .output()
        .unwrap()
}

fn envelope(output: &Output) -> Value {
    let bytes = if output.status.success() {
        &output.stdout
    } else {
        &output.stderr
    };
    serde_json::from_slice(bytes).unwrap_or_else(|error| {
        panic!(
            "invalid CLI JSON: {error}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn all_native_subsets_round_trip_through_normalized_json_transport() {
    for (format, source) in [
        (TimedTextFormat::Plain, PLAIN),
        (TimedTextFormat::Lrc, LRC),
        (TimedTextFormat::Srt, SRT),
        (TimedTextFormat::Webvtt, VTT),
        (TimedTextFormat::Ttml, TTML),
    ] {
        let context = ImportContext::default();
        let options = ConversionOptions::default();
        let decoded = timed_text::decode(source.as_bytes(), format, &context).unwrap();
        if format == TimedTextFormat::Plain {
            assert_eq!(decoded.document.cues.len(), 1);
            assert_eq!(decoded.document.cues[0].text, PLAIN);
        } else {
            assert_eq!(decoded.document.cues.len(), 2, "{format:?}");
            assert_eq!(decoded.document.cues[0].text, "Hello café 雪 🌍");
            assert_eq!(decoded.document.cues[1].text, "مرحبا שלום");
        }
        let transport = timed_text::convert(
            source.as_bytes(),
            format,
            TimedTextFormat::Json,
            &context,
            &options,
        )
        .unwrap();
        assert!(transport.report.losses.is_empty(), "{format:?}");
        assert!(transport.report.carrier_omissions.is_empty());
        let transported =
            timed_text::decode(&transport.bytes, TimedTextFormat::Json, &context).unwrap();
        assert_eq!(transported.document, decoded.document, "{format:?}");
        let back = timed_text::convert(
            &transport.bytes,
            TimedTextFormat::Json,
            format,
            &context,
            &options,
        )
        .unwrap();
        assert_eq!(back.input_document.cues, decoded.document.cues);
        assert_eq!(back.output_document.cues, decoded.document.cues);
        let decoded_back = timed_text::decode(&back.bytes, format, &context).unwrap();
        assert_eq!(
            decoded_back.document.cues, decoded.document.cues,
            "{format:?}"
        );
        assert_eq!(transport.report.input.sha256, digest(source.as_bytes()));
        assert_eq!(transport.report.output.sha256, digest(&transport.bytes));
        assert!(!transport.report.review_attestation_independently_verified);
    }
}

#[test]
fn cross_format_interval_timing_and_source_order_remain_exact() {
    let context = ImportContext::default();
    let original = timed_text::decode(SRT.as_bytes(), TimedTextFormat::Srt, &context)
        .unwrap()
        .document;
    let converted = timed_text::convert(
        SRT.as_bytes(),
        TimedTextFormat::Srt,
        TimedTextFormat::Webvtt,
        &context,
        &ConversionOptions::default(),
    )
    .unwrap();
    assert!(converted.report.losses.is_empty());
    assert_eq!(converted.output_document.cues, original.cues);
    assert_eq!(
        original.cues[0].timing,
        CueTiming::Interval {
            start: AudioRationalTime {
                numerator: 1,
                denominator: 4
            },
            end: AudioRationalTime {
                numerator: 3,
                denominator: 2
            },
        }
    );
    let repeated = timed_text::decode(SRT.as_bytes(), TimedTextFormat::Srt, &context).unwrap();
    assert_eq!(repeated.document.cues, original.cues);
    assert_ne!(original.cues[0].id, original.cues[1].id);
}

#[test]
fn unapproved_losses_are_structured_and_missing_timing_is_never_invented() {
    let context = ImportContext::default();
    let refused = timed_text::convert(
        SRT.as_bytes(),
        TimedTextFormat::Srt,
        TimedTextFormat::Lrc,
        &context,
        &ConversionOptions::default(),
    )
    .unwrap_err();
    assert!(
        refused
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::EndTimes)
    );
    let allowed = ConversionOptions {
        allow_losses: vec![
            ConversionLossKind::EndTimes,
            ConversionLossKind::CueIdentifiers,
        ],
    };
    let converted = timed_text::convert(
        SRT.as_bytes(),
        TimedTextFormat::Srt,
        TimedTextFormat::Lrc,
        &context,
        &allowed,
    )
    .unwrap();
    assert!(!converted.report.losses.is_empty());
    assert!(
        converted
            .output_document
            .cues
            .iter()
            .all(|cue| matches!(cue.timing, CueTiming::Point { .. }))
    );
    let options = ConversionOptions {
        allow_losses: vec![
            ConversionLossKind::Timing,
            ConversionLossKind::EndTimes,
            ConversionLossKind::Precision,
            ConversionLossKind::CueIdentifiers,
            ConversionLossKind::Speaker,
            ConversionLossKind::Metadata,
            ConversionLossKind::Language,
        ],
    };
    assert!(
        timed_text::convert(
            PLAIN.as_bytes(),
            TimedTextFormat::Plain,
            TimedTextFormat::Srt,
            &context,
            &options
        )
        .is_err()
    );
    assert!(
        timed_text::convert(
            LRC.as_bytes(),
            TimedTextFormat::Lrc,
            TimedTextFormat::Srt,
            &context,
            &options
        )
        .is_err()
    );
}

#[test]
fn speaker_language_and_metadata_require_explicit_loss_authority() {
    let voice = b"WEBVTT\n\nverse\n00:00:00.250 --> 00:00:01.500\n<v Synthetic speaker>words</v>\n";
    let context = ImportContext {
        language: Some("en".into()),
        ..ImportContext::default()
    };
    let refused = timed_text::convert(
        voice,
        TimedTextFormat::Webvtt,
        TimedTextFormat::Srt,
        &context,
        &ConversionOptions::default(),
    )
    .unwrap_err();
    for kind in [
        ConversionLossKind::Speaker,
        ConversionLossKind::Language,
        ConversionLossKind::CueIdentifiers,
    ] {
        assert!(
            refused.losses.iter().any(|loss| loss.kind == kind),
            "missing {kind:?} loss"
        );
    }
    let options = ConversionOptions {
        allow_losses: vec![
            ConversionLossKind::Speaker,
            ConversionLossKind::Language,
            ConversionLossKind::CueIdentifiers,
        ],
    };
    let projected = timed_text::convert(
        voice,
        TimedTextFormat::Webvtt,
        TimedTextFormat::Srt,
        &context,
        &options,
    )
    .unwrap();
    assert_eq!(projected.output_document.cues[0].text, "words");
    assert!(projected.output_document.cues[0].speaker.is_none());
    assert!(projected.output_document.language.is_none());
    assert_eq!(
        projected.input_document.cues[0].timing,
        projected.output_document.cues[0].timing
    );
    let metadata = b"[ar:Synthetic artist]\n[00:00.25]words\n";
    let refused = timed_text::convert(
        metadata,
        TimedTextFormat::Lrc,
        TimedTextFormat::Plain,
        &ImportContext::default(),
        &ConversionOptions::default(),
    )
    .unwrap_err();
    assert!(
        refused
            .losses
            .iter()
            .any(|loss| loss.kind == ConversionLossKind::Metadata)
    );
    let projected = timed_text::convert(
        metadata,
        TimedTextFormat::Lrc,
        TimedTextFormat::Plain,
        &ImportContext::default(),
        &ConversionOptions {
            allow_losses: vec![ConversionLossKind::Metadata, ConversionLossKind::Timing],
        },
    )
    .unwrap();
    assert!(projected.output_document.metadata.is_empty());
    assert!(matches!(
        projected.output_document.cues[0].timing,
        CueTiming::Untimed {}
    ));
}

#[test]
fn reviewed_observed_and_unreviewed_contexts_remain_distinct() {
    let evidence = AudioArtifactReference {
        id: "supplied_review".into(),
        sha256: digest(b"synthetic supplied review evidence"),
        byte_size: 34,
    };
    let provenances = [
        TextProvenance::Unreviewed {},
        TextProvenance::ObservedTranscript {
            producer: "synthetic-transcriber/v1".into(),
        },
        TextProvenance::ReviewedLyrics {
            authority: AudioReviewedAuthority {
                supplied_by: "synthetic-reviewer".into(),
                provenance_artifact_id: evidence.id.clone(),
            },
            evidence,
            source_sha256: digest(PLAIN.as_bytes()),
        },
    ];
    for provenance in provenances {
        let context = ImportContext {
            provenance: provenance.clone(),
            ..ImportContext::default()
        };
        let converted = timed_text::convert(
            PLAIN.as_bytes(),
            TimedTextFormat::Plain,
            TimedTextFormat::Json,
            &context,
            &ConversionOptions::default(),
        )
        .unwrap();
        assert_eq!(converted.output_document.provenance, provenance);
        assert!(
            converted
                .output_document
                .cues
                .iter()
                .all(|cue| matches!(cue.timing, CueTiming::Untimed {}))
        );
        assert!(!converted.report.review_attestation_independently_verified);
        let imported = timed_text::decode(
            &converted.bytes,
            TimedTextFormat::Json,
            &ImportContext::default(),
        )
        .unwrap();
        assert_eq!(imported.document.provenance, provenance);
        let override_context = ImportContext {
            provenance: TextProvenance::ObservedTranscript {
                producer: "different-synthetic-authority".into(),
            },
            ..ImportContext::default()
        };
        assert!(
            timed_text::decode(&converted.bytes, TimedTextFormat::Json, &override_context).is_err()
        );
        if let TextProvenance::ReviewedLyrics { .. } = provenance {
            assert!(
                timed_text::decode(
                    b"changed synthetic lyrics",
                    TimedTextFormat::Plain,
                    &context
                )
                .is_err()
            );
        }
    }
}

#[test]
fn overlap_order_and_optional_audio_bounds_are_separate_policies() {
    let overlapping =
        b"1\n00:00:00,000 --> 00:00:02,000\nfirst\n\n2\n00:00:01,000 --> 00:00:03,000\nsecond\n";
    assert!(
        timed_text::decode(overlapping, TimedTextFormat::Srt, &ImportContext::default()).is_err()
    );
    let overlap_context = ImportContext {
        overlap_policy: OverlapPolicy::Allow,
        ..ImportContext::default()
    };
    let accepted = timed_text::decode(overlapping, TimedTextFormat::Srt, &overlap_context).unwrap();
    assert_eq!(accepted.document.cues[0].text, "first");
    let reordered =
        b"1\n00:00:01,000 --> 00:00:03,000\nfirst\n\n2\n00:00:00,000 --> 00:00:02,000\nsecond\n";
    assert!(timed_text::decode(reordered, TimedTextFormat::Srt, &overlap_context).is_err());
    let bound_context = ImportContext {
        audio_source: Some(AudioSource {
            artifact: AudioArtifactReference {
                id: "audio_source".into(),
                sha256: digest(b"synthetic audio identity only"),
                byte_size: 176444,
            },
            stream_index: 0,
            sample_rate_hz: 44100,
            channels: 1,
            frame_count: 88200,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        }),
        ..ImportContext::default()
    };
    assert!(timed_text::decode(SRT.as_bytes(), TimedTextFormat::Srt, &bound_context).is_err());
    let bounded = timed_text::decode(
        b"1\n00:00:00,000 --> 00:00:02,000\nexact end\n",
        TimedTextFormat::Srt,
        &bound_context,
    )
    .unwrap();
    assert_eq!(bounded.document.audio_source, bound_context.audio_source);
}

#[test]
fn bom_eol_and_unicode_are_observed_without_changing_source_bytes() {
    let source = format!("\u{feff}{}", SRT.replace('\n', "\r\n"));
    let converted = timed_text::convert(
        source.as_bytes(),
        TimedTextFormat::Srt,
        TimedTextFormat::Json,
        &ImportContext::default(),
        &ConversionOptions::default(),
    )
    .unwrap();
    assert!(converted.report.lexical.utf8_bom_removed);
    assert_eq!(
        converted.report.lexical.crlf_pairs,
        SRT.matches('\n').count() as u64
    );
    assert_eq!(converted.report.lexical.lone_cr, 0);
    assert_eq!(converted.report.input.sha256, digest(source.as_bytes()));
    assert_eq!(converted.output_document.cues[0].text, "Hello café 雪 🌍");
    assert_eq!(converted.output_document.cues[1].text, "مرحبا שלום");
}

#[test]
fn bounded_import_refuses_xml_authority_and_unsupported_markup() {
    for source in [
        "<!DOCTYPE tt [<!ENTITY secret SYSTEM 'file:///unread-synthetic-path'>]><tt xmlns='http://www.w3.org/ns/ttml' xml:space='preserve'><body><div><p begin='00:00:00.000' end='00:00:01.000'>&secret;</p></div></body></tt>",
        "<!DOCTYPE tt SYSTEM 'https://invalid.example/synthetic.dtd'><tt xmlns='http://www.w3.org/ns/ttml' xml:space='preserve'><body><div/></body></tt>",
        "<tt xmlns='http://www.w3.org/ns/ttml' xml:space='preserve'><body><div><p begin='00:00:00.000' end='00:00:01.000'><unsupported>hidden</unsupported></p></div></body></tt>",
    ] {
        assert!(
            timed_text::decode(
                source.as_bytes(),
                TimedTextFormat::Ttml,
                &ImportContext::default()
            )
            .is_err()
        );
    }
    assert!(
        timed_text::decode(
            &vec![b'x'; 1_048_577],
            TimedTextFormat::Plain,
            &ImportContext::default()
        )
        .is_err()
    );
    assert!(
        timed_text::decode(
            b"WEBVTT\n\n00:00:00.000 --> 00:00:01.000 align:start\ntext\n",
            TimedTextFormat::Webvtt,
            &ImportContext::default()
        )
        .is_err()
    );
}

fn published_files(directory: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (PathBuf::from(path.file_name().unwrap()), bytes)
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

#[test]
fn canonical_cli_registry_and_complete_new_directory_preserve_sources() {
    let formats = cli(&["formats"]);
    assert!(formats.status.success());
    let registry = envelope(&formats);
    assert_eq!(registry["command"], "timed_text_formats");
    assert_eq!(registry["schema_version"], 1);
    for format in ["plain", "lrc", "srt", "webvtt", "ttml", "json"] {
        assert!(
            registry["result"]
                .to_string()
                .contains(&format!("\"{format}\""))
        );
    }
    let temporary = tempfile::Builder::new()
        .prefix("timed text café 雪 ")
        .tempdir()
        .unwrap();
    let input = temporary.path().join("source.srt");
    fs::write(&input, SRT).unwrap();
    let destination = temporary.path().join("new conversion");
    let result = convert_cli(&input, "srt", "webvtt", &destination, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(envelope(&result)["command"], "timed_text_convert");
    let files = published_files(&destination);
    assert_eq!(
        files
            .iter()
            .map(|(path, _)| path.to_string_lossy())
            .collect::<Vec<_>>(),
        [
            "conversion.json",
            "normalized-input.json",
            "normalized-output.json",
            "payload.vtt"
        ]
    );
    let report: Value =
        serde_json::from_slice(&fs::read(destination.join("conversion.json")).unwrap()).unwrap();
    assert_eq!(report["schema"], "aniflow.timed-text-conversion/v1");
    assert_eq!(report["input"]["sha256"], digest(SRT.as_bytes()));
    assert_eq!(
        report["output"]["sha256"],
        digest(&fs::read(destination.join("payload.vtt")).unwrap())
    );
    assert_eq!(report["review_attestation_independently_verified"], false);
    for name in ["normalized-input.json", "normalized-output.json"] {
        let document = timed_text::decode(
            &fs::read(destination.join(name)).unwrap(),
            TimedTextFormat::Json,
            &ImportContext::default(),
        )
        .unwrap();
        assert_eq!(document.document.cues.len(), 2);
        assert_eq!(document.document.provenance, TextProvenance::Unreviewed {});
    }
    let refused = convert_cli(&input, "srt", "webvtt", &destination, &[]);
    assert!(!refused.status.success());
    assert_eq!(published_files(&destination), files);
    assert_eq!(fs::read(&input).unwrap(), SRT.as_bytes());
}

#[test]
fn cli_loss_refusal_is_machine_readable_and_never_publishes_partial_output() {
    let temporary = TempDir::new().unwrap();
    let input = temporary.path().join("source.srt");
    fs::write(&input, SRT).unwrap();
    let destination = temporary.path().join("lossy");
    let refused = convert_cli(&input, "srt", "lrc", &destination, &[]);
    assert!(!refused.status.success());
    let report = envelope(&refused);
    assert_eq!(report["command"], "timed_text_convert");
    assert!(
        report["result"]["losses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|loss| loss["kind"] == "end_times")
    );
    assert!(!destination.exists());
    let approved = convert_cli(
        &input,
        "srt",
        "lrc",
        &destination,
        &["--allow-loss", "end_times,cue_identifiers"],
    );
    assert!(
        approved.status.success(),
        "{}",
        String::from_utf8_lossy(&approved.stderr)
    );
    let output_report: Value =
        serde_json::from_slice(&fs::read(destination.join("conversion.json")).unwrap()).unwrap();
    assert!(
        output_report["losses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|loss| loss["kind"] == "end_times")
    );
    let unsupported = temporary.path().join("unsupported");
    let unknown_format = convert_cli(&input, "ass", "srt", &unsupported, &[]);
    assert!(!unknown_format.status.success());
    assert_eq!(envelope(&unknown_format)["command"], "timed_text_convert");
    assert_eq!(envelope(&unknown_format)["status"], "error");
    assert!(!unsupported.exists());
    let unknown_loss = convert_cli(
        &input,
        "srt",
        "srt",
        &unsupported,
        &["--allow-loss", "synthetic_unknown_loss"],
    );
    assert!(!unknown_loss.status.success());
    assert_eq!(envelope(&unknown_loss)["command"], "timed_text_convert");
    assert_eq!(envelope(&unknown_loss)["status"], "error");
    assert!(!unsupported.exists());
    let context = temporary.path().join("bad-context.json");
    fs::write(&context, b"{not-json").unwrap();
    let malformed_context = convert_cli(
        &input,
        "srt",
        "json",
        &unsupported,
        &["--context", context.to_str().unwrap()],
    );
    assert!(!malformed_context.status.success());
    assert_eq!(
        envelope(&malformed_context)["command"],
        "timed_text_convert"
    );
    assert_eq!(envelope(&malformed_context)["status"], "error");
    assert!(!unsupported.exists());
    let oversized = temporary.path().join("oversized.txt");
    fs::write(&oversized, vec![b'x'; 1_048_577]).unwrap();
    let large_input = convert_cli(&oversized, "plain", "json", &unsupported, &[]);
    assert!(!large_input.status.success());
    assert_eq!(envelope(&large_input)["command"], "timed_text_convert");
    assert_eq!(envelope(&large_input)["status"], "error");
    assert!(!unsupported.exists());
    let malformed = temporary.path().join("bad.srt");
    fs::write(&malformed, b"1\ninvalid clock\ntext\n").unwrap();
    assert!(
        !convert_cli(&malformed, "srt", "json", &unsupported, &[])
            .status
            .success()
    );
    assert!(!unsupported.exists());
    assert_eq!(fs::read(&input).unwrap(), SRT.as_bytes());
}
