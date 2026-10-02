//! Authored #24 fixture consumers; execution is deferred under #64.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use aniflow::temporal;
use aniflow::timed_text::{self, ConversionOptions, ImportContext, TimedTextFormat};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

fn bundle() -> (tempfile::TempDir, PathBuf, Value) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("synthetic corpus");
    let python = std::env::var_os("ANIFLOW_CORPUS_PYTHON").unwrap_or_else(|| "python3".into());
    let result = Command::new(python)
        .arg("-B")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/temporal-corpus.py"))
        .args(["generate", "--output"])
        .arg(&root)
        .output()
        .expect("explicit Python 3.10+ interpreter is required for corpus qualification");
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let catalog = serde_json::from_slice(&fs::read(root.join("catalog.json")).unwrap()).unwrap();
    (temporary, root, catalog)
}

fn input(root: &Path, case: &Value) -> (PathBuf, Vec<u8>) {
    let item = &case["inventory"]["files"][0];
    let path = root.join(case["id"].as_str().unwrap()).join(item["path"].as_str().unwrap());
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len() as u64, item["size_bytes"].as_u64().unwrap());
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), item["sha256"].as_str().unwrap());
    (path, bytes)
}

#[test]
fn corpus_temporal_protocols_have_exact_typed_outcomes_and_preserve_sources() {
    let (_temporary, root, catalog) = bundle();
    let mut consumed = 0;
    for case in catalog["cases"].as_array().unwrap().iter().filter(|case| case["family"] == "temporal_protocol") {
        let (path, before) = input(&root, case);
        let value: Value = serde_json::from_slice(&before).unwrap();
        let observations = value["observations"].as_array().unwrap().iter().map(|item| (
            u32::try_from(item["stream_index"].as_u64().unwrap()).unwrap(),
            item["frames"].clone(), item["packets"].clone(),
        )).collect::<Vec<_>>();
        let selection = serde_json::from_value(value["selection"].clone()).unwrap();
        let parse = || temporal::from_probe_documents(
            value["source_sha256"].as_str().unwrap(), value["source_size_bytes"].as_u64().unwrap(),
            &selection, &value["inventory"], &observations,
        );
        let expected = &case["oracle"];
        let codes = if expected["outcome"] == "parser_error" {
            vec![serde_json::to_value(parse().unwrap_err().temporal_diagnostic().unwrap().code).unwrap()]
        } else {
            let report = parse().unwrap();
            assert_eq!(report.processing.supported, expected["outcome"] == "supported", "{}", case["id"]);
            assert_eq!(report, parse().unwrap(), "same input must preserve normalized observations");
            if report.processing.supported {
                report.require_processing().unwrap();
                temporal::validate_reconstruction(&report, &report, false).unwrap();
            } else {
                assert!(report.require_processing().is_err());
            }
            report.processing.diagnostics.iter().map(|item| serde_json::to_value(item.code).unwrap()).collect()
        };
        for code in expected["codes"].as_array().unwrap() {
            assert!(codes.contains(code), "{}: missing {code} in {codes:?}", case["id"]);
        }
        assert_eq!(fs::read(path).unwrap(), before);
        consumed += 1;
    }
    assert!(consumed >= 32, "the corpus must consume existing temporal recipes plus new missing-stream cases");
}

#[test]
fn corpus_text_preserves_exact_semantics_or_returns_typed_refusal() {
    let (_temporary, root, catalog) = bundle();
    let mut consumed = 0;
    for case in catalog["cases"].as_array().unwrap().iter().filter(|case| case["family"] == "timed_text") {
        let (path, before) = input(&root, case);
        let expected = &case["oracle"];
        let format: TimedTextFormat = serde_json::from_value(expected["properties"]["format"].clone()).unwrap();
        let context = ImportContext::default();
        let result = timed_text::decode(&before, format, &context);
        if expected["outcome"] == "accept" {
            let decoded = result.unwrap();
            assert_eq!(decoded.document.cues.len() as u64, expected["properties"]["cue_count"].as_u64().unwrap());
            let converted = timed_text::convert(&before, format, TimedTextFormat::Json, &context, &ConversionOptions::default()).unwrap();
            assert!(converted.report.losses.is_empty());
            let restored = timed_text::decode(&converted.bytes, TimedTextFormat::Json, &context).unwrap();
            assert_eq!(decoded.document, restored.document);
            assert_eq!(decoded.document.source.sha256, format!("{:x}", Sha256::digest(&before)));
        } else {
            let error = result.expect_err("malformed or unsupported carrier must never become accepted text");
            assert!(expected["codes"].as_array().unwrap().contains(&serde_json::to_value(error.category()).unwrap()), "{}: {error}", case["id"]);
        }
        assert_eq!(fs::read(path).unwrap(), before);
        consumed += 1;
    }
    assert!(consumed >= 16);
}

#[test]
fn corpus_generation_refuses_existing_destinations_without_mutating_them() {
    let (temporary, root, _catalog) = bundle();
    let original = fs::read(root.join("catalog.json")).unwrap();
    let python = std::env::var_os("ANIFLOW_CORPUS_PYTHON").unwrap_or_else(|| "python3".into());
    let result = Command::new(python).arg("-B")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/temporal-corpus.py"))
        .args(["generate", "--output"]).arg(&root).output().unwrap();
    assert!(!result.status.success());
    let error: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(error["code"], "corpus_refused");
    assert_eq!(fs::read(root.join("catalog.json")).unwrap(), original);
    assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 1);
}
