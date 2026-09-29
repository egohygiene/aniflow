#!/usr/bin/env python3
"""Independent closed timed-text schemas and deterministic synthetic examples.

No processor or model is used. Generated review evidence is supplied fixture
provenance, not independent authentication. Rust owns byte-length limits, reduced
rational checks, source duration, cue ordering/overlaps, identity cross references,
and consistency between requested and reported losses. Schemas check JSON shape.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import io
import json
import subprocess
import tempfile
from pathlib import Path
import unittest
import wave
try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit("Timed-text schema checks require an already installed jsonschema package.") from error

ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "docs/contracts"
EXAMPLES = CONTRACTS / "examples"
SCHEMAS = {name: CONTRACTS / filename for name, filename in {
    "document": "timed-text-v1.schema.json", "context": "timed-text-context-v1.schema.json",
    "conversion_report": "timed-text-conversion-v1.schema.json", "registry": "timed-text-registry-v1.schema.json",
}.items()}
EXAMPLE_PATHS = {name: EXAMPLES / path.name.replace(".schema.json", ".example.json") for name, path in SCHEMAS.items()}
RAW_SOURCE = "7\n00:00:00,000 --> 00:00:01,250\nCafé — こんにちは\n\n42\n00:00:01,500 --> 00:00:02,500\nnaïve e\u0301 🌙\n".encode("utf-8")
RAW_OUTPUT = "Café — こんにちは\nnaïve e\u0301 🌙\n".encode("utf-8")
SOURCE_PATH = EXAMPLES / "timed-text-source-v1.srt"
REVIEW_PATH = EXAMPLES / "timed-text-review-v1.txt"
REVIEW_BYTES = ("Synthetic supplied review for a generated Unicode fixture.\n"
                f"Original source SHA-256: {hashlib.sha256(RAW_SOURCE).hexdigest()}\n"
                "This record is not independently authenticated.\n").encode()

def reject_nonfinite(value):
    raise ValueError(f"Invalid JSON numeric token: {value}")

def load(path):
    return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject_nonfinite)

def sha256(content):
    return hashlib.sha256(content).hexdigest()

def artifact(identifier, content):
    return {"id": identifier, "sha256": sha256(content), "byte_size": len(content)}

def canonical_digest(document):
    return sha256(json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode())

def time(numerator, denominator=1):
    return {"numerator": numerator, "denominator": denominator}

def audio_source():
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(44100)
        audio.writeframes(bytes(44100 * 3 * 2))
    return {"artifact": artifact("source_audio", output.getvalue()), "stream_index": 0, "sample_rate_hz": 44100, "channels": 1, "frame_count": 132300, "origin": time(0), "stem": None}

def context():
    return {"schema": "aniflow.timed-text-context/v1", "provenance": {"kind": "reviewed_lyrics", "authority": {"supplied_by": "synthetic-fixture-author", "provenance_artifact_id": "review_evidence"}, "evidence": artifact("review_evidence", REVIEW_BYTES), "source_sha256": sha256(RAW_SOURCE)}, "language": "ja", "audio_source": audio_source(), "overlap_policy": "reject"}

def document():
    result = context()
    result.update(schema="aniflow.timed-text/v1", source=artifact("source_text", RAW_SOURCE), metadata={}, cues=[
        {"id": "cue_000001", "source_label": "7", "text": "Café — こんにちは", "timing": {"kind": "interval", "start": time(0), "end": time(5, 4)}, "speaker": None},
        {"id": "cue_000002", "source_label": "42", "text": "naïve e\u0301 🌙", "timing": {"kind": "interval", "start": time(3, 2), "end": time(5, 2)}, "speaker": None},
    ])
    return result

def conversion_report():
    normalized = document()
    canonical_bytes = json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    return {"schema": "aniflow.timed-text-conversion/v1", "input": artifact("conversion_input", RAW_SOURCE), "output": artifact("conversion_output", canonical_bytes), "from": "srt", "to": "json", "input_document_sha256": canonical_digest(normalized), "output_document_sha256": canonical_digest(normalized), "allowed_losses": [], "losses": [], "lexical": {"utf8_bom_removed": False, "crlf_pairs": 0, "lone_cr": 0}, "carrier_omissions": [], "review_attestation_independently_verified": False}

def loss_shape():
    # A structural fixture for refusal tests; no published conversion claim.
    value = conversion_report()
    value.update(to="plain", output=artifact("conversion_output", RAW_OUTPUT), allowed_losses=["timing"], losses=[{"kind": "timing", "cue_id": "cue_000001", "field": "timing", "detail": "Plain text cannot carry cue timing."}], carrier_omissions=["provenance", "audio_binding", "internal_cue_identities"])
    return value

def registry():
    rows = [
        ("plain", "txt", "text/plain", "one untimed UTF-8 text cue"),
        ("lrc", "lrc", "text/plain", "single centisecond timestamp; leading basic metadata"),
        ("srt", "srt", "application/x-subrip", "numeric labels; explicit millisecond intervals; plain text"),
        ("webvtt", "vtt", "text/vtt", "explicit millisecond intervals; optional whole-cue voice"),
        ("ttml", "ttml", "application/ttml+xml", "tt/body/div/p/br; explicit intervals; preserved XML space"),
        ("json", "json", "application/json", "validated aniflow.timed-text/v1 semantic transport"),
    ]
    return {"schema": "aniflow.timed-text-registry/v1", "formats": [{"format": format, "extension": extension, "media_type": media, "subset": subset, "transport_only": format == "json"} for format, extension, media, subset in rows]}

BUILDERS = {"document": document, "context": context, "conversion_report": conversion_report, "registry": registry}

def validators():
    result = {}
    for name, path in SCHEMAS.items():
        schema = load(path)
        Draft202012Validator.check_schema(schema)
        result[name] = Draft202012Validator(schema)
    return result

def verify_cli(binary):
    """Prove published fixtures match the native converter's exact byte output."""
    binary = binary.resolve(strict=True)
    immutable_paths = [SOURCE_PATH, REVIEW_PATH, EXAMPLE_PATHS["context"]]
    before = {path: sha256(path.read_bytes()) for path in immutable_paths}
    def invoke(*arguments):
        process = subprocess.run([str(binary), "--output", "json", "timed-text", *map(str, arguments)], capture_output=True, check=False, timeout=30)
        if process.returncode:
            raise AssertionError(process.stderr.decode("utf-8", errors="replace"))
        return json.loads(process.stdout)["result"]
    actual_registry = invoke("formats")
    assert actual_registry == load(EXAMPLE_PATHS["registry"])
    with tempfile.TemporaryDirectory(prefix="aniflow-timed-text-schema-") as temporary:
        result = invoke("convert", "--input", SOURCE_PATH, "--from", "srt", "--to", "json", "--context", EXAMPLE_PATHS["context"], "--output-directory", Path(temporary).resolve() / "converted")
        actual_report = load(Path(result["report_path"]))
        assert actual_report == result["report"] == load(EXAMPLE_PATHS["conversion_report"])
        assert load(Path(result["input_document_path"])) == load(EXAMPLE_PATHS["document"])
        assert load(Path(result["output_document_path"])) == load(EXAMPLE_PATHS["document"])
        canonical = json.dumps(document(), ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        for key in ["payload_path", "input_document_path", "output_document_path"]:
            assert Path(result[key]).read_bytes() == canonical, key
        expected_report_bytes = json.dumps(actual_report, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        assert Path(result["report_path"]).read_bytes() == expected_report_bytes
        validators()["conversion_report"].validate(actual_report)
        verification = {"case": "published_reviewed_srt_to_json", "source_sha256": sha256(RAW_SOURCE), "payload_sha256": sha256(canonical), "input_document_sha256": actual_report["input_document_sha256"], "output_document_sha256": actual_report["output_document_sha256"], "report_sha256": sha256(expected_report_bytes), "registry_profiles": len(actual_registry["formats"]), "canonical_json_has_trailing_newline": canonical.endswith(b"\n")}
    assert before == {path: sha256(path.read_bytes()) for path in immutable_paths}
    return verification

class TimedTextSchemas(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = validators()

    def test_published_synthetic_fixtures_and_raw_identity(self):
        for name, build in BUILDERS.items():
            value = load(EXAMPLE_PATHS[name])
            self.assertEqual(value, build(), name)
            self.validators[name].validate(value)
        self.assertEqual(SOURCE_PATH.read_bytes(), RAW_SOURCE)
        self.assertEqual(REVIEW_PATH.read_bytes(), REVIEW_BYTES)
        self.assertEqual(document()["source"]["sha256"], context()["provenance"]["source_sha256"])
        self.assertIn("e\u0301", document()["cues"][1]["text"])

    def test_provenance_variants_and_review_authority(self):
        for provenance in [{"kind": "unreviewed"}, {"kind": "observed_transcript", "producer": "synthetic-observer"}, context()["provenance"]]:
            value = context()
            value["provenance"] = provenance
            self.validators["context"].validate(value)
        for missing in ["authority", "evidence", "source_sha256"]:
            value = context()
            del value["provenance"][missing]
            self.assertFalse(self.validators["context"].is_valid(value), missing)
        value = context()
        value["provenance"]["evidence"]["byte_size"] = 8388609
        self.assertFalse(self.validators["context"].is_valid(value))

    def test_closed_nested_fields(self):
        paths = [[], ["source"], ["provenance"], ["provenance", "authority"], ["provenance", "evidence"], ["audio_source"], ["audio_source", "origin"], ["cues", 0], ["cues", 0, "timing"], ["cues", 0, "timing", "end"]]
        for path in paths:
            value = document()
            target = value
            for key in path: target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["document"].is_valid(value), path)
        for timing in [{"kind": "untimed"}, {"kind": "point", "at": time(1)}]:
            value = document()
            value["cues"][0]["timing"] = {**timing, "unknown": True}
            self.assertFalse(self.validators["document"].is_valid(value))

    def test_timing_shapes_and_exact_millisecond_boundaries(self):
        value = document()
        for timing in [{"kind": "untimed"}, {"kind": "point", "at": time(1, 1000)}, {"kind": "point", "at": time(86400)}, {"kind": "interval", "start": time(0), "end": time(3)}]:
            value["audio_source"] = None
            value["cues"][0]["timing"] = timing
            self.validators["document"].validate(value)
        for invalid_time in [time(-1), time(1, 3), time(1, 0), time(86401), time(86400001, 1000), {"numerator": 0.5, "denominator": 1}]:
            value["cues"][0]["timing"] = {"kind": "point", "at": invalid_time}
            self.assertFalse(self.validators["document"].is_valid(value), invalid_time)

    def test_unicode_controls_source_labels_and_language(self):
        value = document()
        value["cues"][0]["text"] = " \t\r\nCafé e\u0301 🌙 "
        self.validators["document"].validate(value)
        for text in ["", "nul\0", "c1\u0085", "line\v"]:
            value["cues"][0]["text"] = text
            self.assertFalse(self.validators["document"].is_valid(value), repr(text))
        for path, replacement in [(["cues", 0, "id"], "7"), (["cues", 0, "source_label"], "label\n"), (["source", "sha256"], "0" * 64 + "\n"), (["language"], "ja_ja"), (["language"], "x"), (["metadata"], {"Bad-Key": "value"})]:
            value = document()
            target = value
            for key in path[:-1]: target = target[key]
            target[path[-1]] = replacement
            self.assertFalse(self.validators["document"].is_valid(value), path)
        value = document()
        value["cues"][0]["source_label"] = "007"
        self.validators["document"].validate(value)

    def test_collection_bounds_and_optional_fields(self):
        value = document()
        del value["language"]
        del value["audio_source"]
        del value["cues"][0]["source_label"]
        del value["cues"][0]["speaker"]
        self.validators["document"].validate(value)
        for field, replacement in [("metadata", {f"key{index}": "value" for index in range(33)}), ("cues", [document()["cues"][0]] * 10001)]:
            value = document()
            value[field] = replacement
            self.assertFalse(self.validators["document"].is_valid(value), field)
        value = document()
        value["cues"][0]["text"] = "x" * 65537
        self.assertFalse(self.validators["document"].is_valid(value))
        value = document()
        value["source"]["byte_size"] = 1048577
        self.assertFalse(self.validators["document"].is_valid(value))
        value = context()
        value["provenance"]["evidence"]["id"] = "review.evidence"
        self.assertFalse(self.validators["context"].is_valid(value))
        value = document()
        value["cues"][1]["source_label"] = value["cues"][0]["source_label"]
        self.validators["document"].validate(value)  # Authored labels may repeat.

    def test_conversion_losses_are_explicit_closed_and_digest_bound(self):
        value = loss_shape()
        self.validators["conversion_report"].validate(value)
        for path, replacement in [(["losses", 0, "kind"], "invented"), (["allowed_losses"], ["timing", "timing"]), (["carrier_omissions"], ["provenance", "provenance"]), (["input", "id"], "source_text"), (["output_document_sha256"], "bad"), (["review_attestation_independently_verified"], True), (["lexical", "crlf_pairs"], -1)]:
            value = loss_shape()
            target = value
            for key in path[:-1]: target = target[key]
            target[path[-1]] = replacement
            self.assertFalse(self.validators["conversion_report"].is_valid(value), path)
        value = loss_shape()
        value["losses"][0]["unknown"] = True
        self.assertFalse(self.validators["conversion_report"].is_valid(value))
        value = conversion_report()
        value["allowed_losses"] = ["metadata"]
        value["losses"] = [{"kind": "metadata", "cue_id": None, "field": "metadata.title", "detail": "Invented loss in lossless JSON transport."}]
        self.assertFalse(self.validators["conversion_report"].is_valid(value))

    def test_registry_is_exactly_five_formats_and_json_transport(self):
        value = registry()
        self.validators["registry"].validate(value)
        value["formats"][0], value["formats"][1] = value["formats"][1], value["formats"][0]
        self.assertFalse(self.validators["registry"].is_valid(value))
        value = registry()
        value["formats"][-1]["transport_only"] = False
        self.assertFalse(self.validators["registry"].is_valid(value))
        value = registry()
        value["formats"][0]["unknown"] = True
        self.assertFalse(self.validators["registry"].is_valid(value))

    def test_optional_audio_binding_supports_valid_stems(self):
        value = document()
        original = copy.deepcopy(value["audio_source"]["artifact"])
        original["id"] = "original_mix"
        value["audio_source"]["stem"] = {"id": "voice_layer", "original_mix": original, "relationship_evidence_id": "stem_lineage"}
        self.validators["document"].validate(value)
        value["audio_source"]["channels"] = 65
        self.assertFalse(self.validators["document"].is_valid(value))

    def test_rust_owns_cross_reference_and_relational_checks(self):
        # Accepted JSON shapes require Rust semantic refusal, not schema claims.
        value = document()
        value["cues"][0]["timing"]["end"] = time(4)  # source is only 3 seconds
        self.validators["document"].validate(value)
        value["cues"][0]["timing"]["end"] = time(2, 2)  # not reduced
        self.validators["document"].validate(value)
        value["provenance"]["source_sha256"] = "0" * 64
        self.validators["document"].validate(value)
        value = loss_shape()
        value["allowed_losses"] = []
        self.validators["conversion_report"].validate(value)

    def test_schemas_contain_only_reachable_definitions(self):
        for name, path in SCHEMAS.items():
            schema = load(path)
            definitions = schema.get("$defs", {})
            pending = []
            def walk(node):
                if isinstance(node, dict):
                    if "$ref" in node:
                        self.assertTrue(node["$ref"].startswith("#/$defs/"))
                        pending.append(node["$ref"].removeprefix("#/$defs/"))
                    for key, value in node.items():
                        if key != "$defs": walk(value)
                elif isinstance(node, list):
                    for value in node: walk(value)
            walk(schema)
            seen = set()
            while pending:
                key = pending.pop()
                if key in seen: continue
                seen.add(key)
                walk(definitions[key])
            self.assertEqual(seen, set(definitions), name)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-examples", action="store_true")
    parser.add_argument("--verify-cli", type=Path, help="Compare published fixtures with exact native CLI output bytes.")
    for name in SCHEMAS:
        parser.add_argument("--" + name.replace("_", "-"), action="append", type=Path, default=[])
    args, unittest_args = parser.parse_known_args()
    if args.write_examples:
        for name, build in BUILDERS.items():
            EXAMPLE_PATHS[name].write_text(json.dumps(build(), indent=2, ensure_ascii=False, allow_nan=False) + "\n")
        SOURCE_PATH.write_bytes(RAW_SOURCE)
        REVIEW_PATH.write_bytes(REVIEW_BYTES)
    if args.verify_cli:
        print(json.dumps(verify_cli(args.verify_cli), sort_keys=True))
    for name, validator in validators().items():
        for path in getattr(args, name):
            validator.validate(load(path))
            print(f"Validated timed-text {name}: {path}")
    unittest.main(argv=[__file__, *unittest_args])
