use aniflow::ErrorCategory;
use aniflow::audio_analysis::{
    AUDIO_MAX_SAFE_INTEGER, AudioAnalysis, AudioConfidence, AudioObservationValue,
    AudioRationalTime,
};
use serde_json::{Value, json};

const TECHNICAL: &[u8] =
    include_bytes!("../docs/contracts/examples/audio-analysis-technical-v1.example.json");
const ESTIMATED: &[u8] =
    include_bytes!("../docs/contracts/examples/audio-analysis-estimated-v1.example.json");
const UNAVAILABLE: &[u8] =
    include_bytes!("../docs/contracts/examples/audio-analysis-unavailable-v1.example.json");
const TIMELINE: &[u8] =
    include_bytes!("../docs/contracts/examples/audio-analysis-timeline-v1.example.json");

fn document(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("synthetic fixture JSON")
}

fn parse(value: &Value) -> aniflow::Result<AudioAnalysis> {
    AudioAnalysis::from_json_slice(&serde_json::to_vec(value).expect("encode test value"))
}

fn reject_at(bytes: &[u8], pointer: &str, replacement: Value) {
    let mut value = document(bytes);
    *value.pointer_mut(pointer).expect("fixture field exists") = replacement;
    let error = parse(&value).expect_err(pointer);
    assert_eq!(
        error.category(),
        ErrorCategory::Configuration,
        "{pointer}: {error}"
    );
}

#[test]
fn published_examples_round_trip_through_validated_public_api() {
    for bytes in [TECHNICAL, ESTIMATED, UNAVAILABLE, TIMELINE] {
        let value = AudioAnalysis::from_json_slice(bytes).expect("valid synthetic example");
        value.validate().expect("public validation");
        let round_trip = serde_json::to_vec(&value).expect("serialize");
        assert_eq!(AudioAnalysis::from_json_slice(&round_trip).unwrap(), value);
        let canonical = value.canonical_json_bytes().expect("validated encoding");
        assert_eq!(AudioAnalysis::from_json_slice(&canonical).unwrap(), value);
    }
}

#[test]
fn unknown_fields_are_closed_at_envelope_struct_and_tagged_variant_boundaries() {
    for pointer in [
        "",
        "/source",
        "/providers/0",
        "/providers/0/models",
        "/observations/0/value",
        "/observations/0/provenance/confidence",
    ] {
        let mut value = document(TECHNICAL);
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future_required_semantics".to_owned(), json!(true));
        assert!(parse(&value).is_err(), "unknown field at {pointer}");
    }
    reject_at(TECHNICAL, "/schema", json!("aniflow.audio-analysis/v2"));
    reject_at(TECHNICAL, "/observations/0/kind", json!("future_feature"));
}

#[test]
fn audio_profile_refuses_unsupported_clock_stream_and_channel_scopes() {
    for (pointer, value) in [
        ("/source/stream_index", json!(1)),
        ("/source/origin/numerator", json!(1)),
        ("/source/origin/denominator", json!(0)),
        ("/source/sample_rate_hz", json!(0)),
        ("/source/sample_rate_hz", json!(768_001)),
        ("/source/channels", json!(0)),
        ("/source/channels", json!(65)),
        ("/source/frame_count", json!(0)),
        ("/source/frame_count", json!(9_007_199_254_740_992_u64)),
        ("/observations/0/scope/channels", json!([])),
        ("/observations/0/scope/channels", json!([0, 0])),
        ("/observations/0/scope/channels", json!([64])),
        ("/observations/0/scope/stem_id", json!("undeclared-stem")),
    ] {
        reject_at(TECHNICAL, pointer, value);
    }
}

#[test]
fn identities_and_evidence_must_be_nonempty_unique_and_resolvable() {
    for (pointer, value) in [
        ("/source/artifact/sha256", json!("not-a-digest")),
        ("/source/artifact/id", json!("")),
        ("/providers/0/configuration_sha256", json!("")),
        ("/providers/0/implementation_sha256", json!("")),
        ("/providers/0/provider/version", json!("unknown")),
        (
            "/observations/0/provenance/provider_evidence_id",
            json!("missing"),
        ),
        (
            "/observations/0/provenance/evidence_artifact_ids",
            json!([]),
        ),
        (
            "/observations/0/provenance/evidence_artifact_ids",
            json!(["missing"]),
        ),
        (
            "/observations/0/capability_id",
            json!("aniflow/unregistered"),
        ),
    ] {
        reject_at(TECHNICAL, pointer, value);
    }
    for array in ["artifacts", "providers", "capabilities", "observations"] {
        let mut value = document(TECHNICAL);
        let entries = value[array].as_array_mut().unwrap();
        entries.push(entries[0].clone());
        assert!(parse(&value).is_err(), "duplicate {array}");
    }
}

#[test]
fn confidence_and_units_cannot_launder_invalid_estimates() {
    reject_at(
        TECHNICAL,
        "/observations/0/value/unit",
        json!("beats_per_minute"),
    );
    reject_at(
        TECHNICAL,
        "/observations/0/value/unit",
        json!("invented_unit"),
    );
    for confidence in [
        json!({"kind": "calibrated", "score": -0.1}),
        json!({"kind": "uncalibrated", "score": 1.1}),
        json!({"kind": "unavailable", "reason": ""}),
        json!({"kind": "not_applicable"}),
    ] {
        reject_at(
            ESTIMATED,
            "/observations/0/provenance/confidence",
            confidence,
        );
    }
    let mut value = document(ESTIMATED);
    value["observations"][0]["provenance"]["confidence"] =
        json!({"kind": "unavailable", "reason": "provider has no calibrated score"});
    parse(&value).expect("honest missing confidence is allowed");
}

#[test]
fn constructed_nonfinite_numbers_fail_before_serialization_can_turn_them_into_null() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut value = AudioAnalysis::from_json_slice(ESTIMATED).unwrap();
        value.observations[0].provenance.confidence =
            AudioConfidence::Uncalibrated { score: invalid };
        assert!(value.validate().is_err());
        assert!(value.canonical_json_bytes().is_err());
        let mut value = AudioAnalysis::from_json_slice(ESTIMATED).unwrap();
        if let AudioObservationValue::Quantity { value: number, .. } =
            &mut value.observations[0].value
        {
            *number = invalid;
        } else {
            panic!("estimated fixture must begin with numeric tempo");
        }
        assert!(value.validate().is_err());
        assert!(value.canonical_json_bytes().is_err());
    }
}

#[test]
fn complete_outcome_requires_complete_capabilities_and_available_evidence() {
    reject_at(TECHNICAL, "/capabilities/0/status", json!("partial"));
    reject_at(TECHNICAL, "/capabilities/0/status", json!("cancelled"));
    reject_at(
        TECHNICAL,
        "/capabilities/0/provider_evidence_ids",
        json!([]),
    );
    reject_at(
        TECHNICAL,
        "/capabilities/0/evidence_artifact_ids",
        json!([]),
    );
    reject_at(
        TECHNICAL,
        "/providers/0/models",
        json!({"kind": "unavailable", "reason": "model missing"}),
    );
    reject_at(UNAVAILABLE, "/status", json!("complete"));
}

#[test]
fn extensions_are_namespaced_optional_data_and_cannot_replace_required_evidence() {
    let mut value = document(TECHNICAL);
    value["extensions"] = json!({"org.example.analyzer": {"native_hint": "opaque"}});
    parse(&value).expect("namespaced metadata is allowed");
    value["extensions"] = json!({"core": {"pretend_evidence": true}});
    assert!(parse(&value).is_err());
    value["extensions"] = json!({"org.example.analyzer": {"evidence_artifact_ids": ["pretend"]}});
    value["observations"][0]["provenance"]["evidence_artifact_ids"] = json!([]);
    assert!(parse(&value).is_err());
}

fn timeline(kind: &str, ranges: &[(u64, u64)]) -> Value {
    let mut value = document(ESTIMATED);
    let provenance = value["observations"][0]["provenance"].clone();
    let events: Vec<Value> = ranges
        .iter()
        .enumerate()
        .map(|(index, (start, end))| {
            json!({
                "id": format!("event-{index}"),
                "range": {"start": start, "end": end},
                "label": "synthetic event",
                "provenance": provenance,
            })
        })
        .collect();
    value["timelines"] = json!([{
        "id": "synthetic-timeline",
        "capability_id": value["capabilities"][0]["capability"]["id"],
        "kind": kind,
        "scope": value["observations"][0]["scope"],
        "events": events,
    }]);
    value
}

#[test]
fn timeline_overlap_is_kind_specific_and_ranges_are_half_open() {
    parse(&timeline("markers", &[(0, 1), (10, 11)])).unwrap();
    assert!(parse(&timeline("markers", &[(0, 2)])).is_err());
    parse(&timeline("disjoint_regions", &[(0, 10), (10, 20)])).unwrap();
    assert!(parse(&timeline("disjoint_regions", &[(0, 10), (9, 20)])).is_err());
    parse(&timeline("overlapping_regions", &[(0, 15), (5, 10)])).unwrap();
    for ranges in [vec![(0, 0)], vec![(10, 9)], vec![(10, 11), (0, 1)]] {
        assert!(parse(&timeline("overlapping_regions", &ranges)).is_err());
    }
    let source_frames = document(ESTIMATED)["source"]["frame_count"]
        .as_u64()
        .unwrap();
    parse(&timeline("markers", &[(source_frames - 1, source_frames)])).unwrap();
    assert!(parse(&timeline("markers", &[(source_frames, source_frames + 1)])).is_err());
}

fn semantic_artifact(kind: &str, capability: &str) -> Value {
    let mut value = document(ESTIMATED);
    let provenance = value["observations"][0]["provenance"].clone();
    value["observations"] = json!([]);
    value["timelines"] = json!([]);
    value["semantic_artifacts"] = json!([{
        "artifact_id": "synthetic-output",
        "capability_id": capability,
        "kind": kind,
        "provenance": provenance,
        "authority": null,
    }]);
    value["artifacts"].as_array_mut().unwrap().push(json!({
        "id": "synthetic-output", "sha256": "c".repeat(64), "byte_size": 42,
    }));
    value["capabilities"][0]["capability"]["id"] = json!(capability);
    value
}

#[test]
fn reviewed_lyrics_require_explicit_authority_without_promoting_observed_transcripts() {
    let observed = semantic_artifact("observed_transcript", "aniflow/audio-transcription");
    parse(&observed).expect("observed transcript reference");
    let mut reviewed = semantic_artifact("reviewed_lyrics", "aniflow/audio-timed-text");
    assert!(
        parse(&reviewed).is_err(),
        "reviewed text requires supplied authority"
    );
    let authority = json!({
        "supplied_by": "synthetic-reviewer",
        "provenance_artifact_id": reviewed["artifacts"][0]["id"],
    });
    reviewed["semantic_artifacts"][0]["authority"] = authority.clone();
    parse(&reviewed).expect("explicit supplied authority reference");
    let mut invalid_observed = observed;
    invalid_observed["semantic_artifacts"][0]["authority"] = authority;
    assert!(
        parse(&invalid_observed).is_err(),
        "observed text cannot claim reviewed authority"
    );
    reviewed["semantic_artifacts"][0]["authority"]["provenance_artifact_id"] = json!("missing");
    assert!(parse(&reviewed).is_err());
}

#[test]
fn midi_references_remain_probabilistic_candidates() {
    let mut value = semantic_artifact("candidate_midi", "aniflow/audio-midi-extraction");
    value["semantic_artifacts"][0]["provenance"]["class"] = json!("probabilistic");
    parse(&value).expect("probabilistic MIDI candidate");
    value["semantic_artifacts"][0]["provenance"]["class"] = json!("deterministic");
    assert!(parse(&value).is_err());
    value["semantic_artifacts"][0]["provenance"]["class"] = json!("heuristic");
    assert!(parse(&value).is_err());
    value["semantic_artifacts"][0]["provenance"]["class"] = json!("probabilistic");
    value["capabilities"][0]["capability"]["id"] = json!("aniflow/audio-transcription");
    value["semantic_artifacts"][0]["capability_id"] = json!("aniflow/audio-transcription");
    assert!(
        parse(&value).is_err(),
        "MIDI cannot masquerade as transcription"
    );
}

#[test]
fn excerpt_and_stem_references_are_bound_to_source_scope_and_evidence() {
    let mut value = document(TECHNICAL);
    value["excerpts"] = json!([{
        "id": "synthetic-preview-reference",
        "artifact_id": value["artifacts"][0]["id"],
        "range": {"start": 0, "end": 10},
        "scope": value["observations"][0]["scope"],
        "evidence_artifact_ids": [value["artifacts"][0]["id"]],
    }]);
    parse(&value).unwrap();
    value["excerpts"][0]["range"]["end"] = json!(u64::MAX);
    assert!(parse(&value).is_err());
    value["excerpts"] = json!([]);
    value["source"]["stem"] = json!({
        "id": "vocals",
        "original_mix": {"id": "synthetic-mix", "sha256": "d".repeat(64), "byte_size": 1024},
        "relationship_evidence_id": value["artifacts"][0]["id"],
    });
    for observation in value["observations"].as_array_mut().unwrap() {
        observation["scope"]["stem_id"] = json!("vocals");
    }
    parse(&value).expect("explicit stem lineage, without inferring alignment");
    value["source"]["stem"]["relationship_evidence_id"] = json!("missing");
    assert!(parse(&value).is_err());
}

#[test]
fn rational_time_is_exact_at_fractional_seconds_and_large_boundaries() {
    let mut source = AudioAnalysis::from_json_slice(TECHNICAL).unwrap().source;
    source.frame_count = AUDIO_MAX_SAFE_INTEGER;
    for rate in [1, 44_100, 48_000, 768_000] {
        source.sample_rate_hz = rate;
        for frame in [
            0,
            1,
            u64::from(rate) - 1,
            u64::from(rate),
            u64::from(rate) + 1,
            AUDIO_MAX_SAFE_INTEGER,
        ] {
            let exact = source.time_for_frame(frame).unwrap();
            exact.validate().unwrap();
            assert_eq!(source.frame_for_time(exact).unwrap(), frame);
        }
    }
    source.sample_rate_hz = 44_100;
    assert_eq!(
        source.time_for_frame(1).unwrap(),
        AudioRationalTime {
            numerator: 1,
            denominator: 44_100
        }
    );
    assert_eq!(
        source.time_for_frame(44_100).unwrap(),
        AudioRationalTime {
            numerator: 1,
            denominator: 1
        }
    );
    for time in [
        AudioRationalTime {
            numerator: -1,
            denominator: 1,
        },
        AudioRationalTime {
            numerator: 1,
            denominator: 0,
        },
        AudioRationalTime {
            numerator: 2,
            denominator: 2,
        },
        AudioRationalTime {
            numerator: 1,
            denominator: 88_200,
        },
        AudioRationalTime {
            numerator: i64::MIN,
            denominator: 1,
        },
        AudioRationalTime {
            numerator: i64::MAX,
            denominator: 1,
        },
    ] {
        assert!(
            source.frame_for_time(time).is_err(),
            "invalid or fractional frame: {time:?}"
        );
    }
    source.frame_count = 44_100;
    assert!(source.time_for_frame(44_101).is_err());
    assert!(
        source
            .frame_for_time(AudioRationalTime {
                numerator: 2,
                denominator: 1
            })
            .is_err()
    );
}

#[test]
fn parser_refuses_oversized_and_malformed_documents() {
    assert!(AudioAnalysis::from_json_slice(&vec![b' '; 8 * 1024 * 1024 + 1]).is_err());
    assert!(AudioAnalysis::from_json_slice(b"{\"schema\": NaN}").is_err());
    assert!(AudioAnalysis::from_json_slice(&[0xff]).is_err());
    let mut value = document(TECHNICAL);
    value["source"]["artifact"]["byte_size"] = json!(0);
    assert!(parse(&value).is_err());
}
