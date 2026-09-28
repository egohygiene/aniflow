#!/usr/bin/env python3
"""Independent schemas and reproducible neutral stem-lineage shape fixture.

The example uses generated silent PCM clocks and synthetic authority/provider
marker bytes. It does not claim a separation run or model inference occurred.
Use --lineage-report to validate an actual generated companion. Requires an
already installed jsonschema package; no tools or assets are downloaded.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import io
import json
import unittest
import wave
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit("Stem schema checks require an already installed Python jsonschema package; no installation was attempted.") from error

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "providers" / "audio-stem"
CONTRACTS = ROOT / "docs" / "contracts"
EXAMPLE = CONTRACTS / "examples" / "audio-stem-lineage-v1.example.json"


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def payload(label):
    return f"Synthetic stem lineage shape only: {label}; no provider was executed.\n".encode()


def digest(label):
    return hashlib.sha256(payload(label)).hexdigest()


def artifact(identifier, label=None):
    content = payload(identifier if label is None else label)
    return {"id": identifier, "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}


def audio_bytes(rate):
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(2)
        audio.setsampwidth(2)
        audio.setframerate(rate)
        audio.writeframes(bytes(rate * 4))
    return output.getvalue()


def audio_source(identifier, rate):
    content = audio_bytes(rate)
    return {"artifact": {"id": identifier, "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}, "stream_index": 0, "sample_rate_hz": rate, "channels": 2, "frame_count": rate, "origin": {"numerator": 0, "denominator": 1}, "stem": None}


def lineage():
    mix = audio_source("original_mix", 48000)
    stem = audio_source("source_audio", 44100)
    stem["stem"] = {"id": "voice_layer", "original_mix": copy.deepcopy(mix["artifact"]), "relationship_evidence_id": "stem_lineage"}
    return {
        "original_mix": mix,
        "selected_stem": stem,
        "stem_id": "voice_layer",
        "source_stage_id": "separate_layers",
        "source_mix_artifact_id": "mix",
        "source_stem_artifact_id": "voice_layer",
        "source_stem_port": "voice",
        "scope": {"channels": [0, 1], "stem_id": "voice_layer"},
        "range": {"start": 0, "end": 44100},
        "relationship_evidence": artifact("separation_evidence", "opaque relationship bytes"),
        "authority_artifacts": [artifact("stem_authority_" + name) for name in ["plan", "manifest", "checkpoint", "lock", "report", "validation_0", "validation_1"]],
        "source_plan_sha256": digest("canonical source plan"),
        "source_checkpoint_sha256": digest("canonical source checkpoint"),
        "source_provider": {"id": "org.example.synthetic-separator", "version": "1.0.0"},
        "source_provider_lock_sha256": digest("canonical source provider lock"),
        "source_implementation_sha256": digest("source implementation"),
        "source_configuration_sha256": digest("source configuration"),
        "duration_tolerance_milliseconds": 20,
        "timing_basis": "zero_origin_duration_only",
    }


def configuration(relationship=None):
    return {"schema": "aniflow.audio-stem.configuration/v1", "lineage": lineage() if relationship is None else relationship, "upstream_analysis_artifact_id": "analysis"}


def canonical_digest(document):
    return hashlib.sha256(json.dumps(document, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()).hexdigest()


def example():
    relationship = lineage()
    return {
        "schema": "aniflow.audio-stem-lineage/v1",
        "lineage": relationship,
        "upstream_analysis_artifact": artifact("analysis", "upstream normalized analysis shape"),
        "provider": {"id": "org.egohygiene.aniflow.audio-stem", "version": "1.0.0"},
        "implementation_sha256": digest("lineage provider implementation"),
        "configuration_sha256": canonical_digest(configuration(relationship)),
        "provider_lock_sha256": digest("lineage provider lock"),
    }


def encoded(document):
    return json.dumps(document, indent=2, allow_nan=False) + "\n"


def changed(document, path, value):
    document = copy.deepcopy(document)
    node = document
    for key in path[:-1]:
        node = node[key]
    node[path[-1]] = value
    return document


class AudioStemSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.configuration_schema = load(BUNDLE / "configuration.schema.json")
        cls.report_schema = load(CONTRACTS / "audio-stem-lineage-v1.schema.json")
        for schema in [cls.configuration_schema, cls.report_schema]:
            Draft202012Validator.check_schema(schema)
        cls.configuration_validator = Draft202012Validator(cls.configuration_schema)
        cls.report_validator = Draft202012Validator(cls.report_schema)

    def test_reproducible_source_clocks_and_example(self):
        self.assertEqual(EXAMPLE.read_text(encoding="utf-8"), encoded(example()))
        self.report_validator.validate(load(EXAMPLE))
        for name in ["original_mix", "selected_stem"]:
            source = example()["lineage"][name]
            content = audio_bytes(source["sample_rate_hz"])
            self.assertEqual(source["artifact"]["sha256"], hashlib.sha256(content).hexdigest())
            self.assertEqual(source["artifact"]["byte_size"], len(content))
            self.assertEqual(source["frame_count"], source["sample_rate_hz"])

    def test_configuration_identity_and_shared_shape(self):
        self.configuration_validator.validate(configuration())
        self.assertEqual(example()["configuration_sha256"], canonical_digest(configuration()))
        self.assertEqual(self.configuration_schema["$defs"], self.report_schema["$defs"])
        manifest = load(BUNDLE / "manifest.json")
        Draft202012Validator(load(CONTRACTS / "provider-manifest-v1.schema.json")).validate(manifest)
        reference = {"id": "aniflow.audio-stem.configuration/v1", "version": "1.0.0", "sha256": hashlib.sha256((BUNDLE / "configuration.schema.json").read_bytes()).hexdigest()}
        self.assertEqual(manifest["configuration_schemas"], [reference])
        self.assertEqual(manifest["capabilities"][0]["configuration_schema"], reference)
        evidence = next(port for port in manifest["capabilities"][0]["inputs"] if port["name"] == "separation_evidence")
        self.assertEqual(evidence["artifact_type"], "application/octet-stream")

    def test_unknown_nested_fields_are_rejected(self):
        paths = [(), ("lineage",), ("lineage", "original_mix"), ("lineage", "original_mix", "artifact"), ("lineage", "original_mix", "origin"), ("lineage", "selected_stem", "stem"), ("lineage", "selected_stem", "stem", "original_mix"), ("lineage", "scope"), ("lineage", "range"), ("lineage", "relationship_evidence"), ("lineage", "authority_artifacts", 0), ("lineage", "source_provider"), ("upstream_analysis_artifact",), ("provider",)]
        for path in paths:
            with self.subTest(path=path):
                self.assertFalse(self.report_validator.is_valid(changed(example(), (*path, "unknown"), True)))

    def test_invalid_scope_authority_and_identity_shapes_are_rejected(self):
        cases = [
            (("schema",), "aniflow.audio-stem-lineage/v2"),
            (("provider", "version"), "2.0.0"),
            (("lineage", "original_mix", "origin", "numerator"), 1),
            (("lineage", "selected_stem", "origin", "denominator"), 0),
            (("lineage", "selected_stem", "stem"), None),
            (("lineage", "selected_stem", "stem", "relationship_evidence_id"), "separation_evidence"),
            (("lineage", "scope", "channels"), [0]),
            (("lineage", "scope", "channels"), [1, 0]),
            (("lineage", "range", "start"), 1),
            (("lineage", "range", "end"), 0),
            (("lineage", "duration_tolerance_milliseconds"), 21),
            (("lineage", "duration_tolerance_milliseconds"), -1),
            (("lineage", "timing_basis"), "sample_aligned"),
            (("lineage", "stem_id"), "voice_layer\n"),
            (("lineage", "source_plan_sha256"), "a" * 64 + "\n"),
            (("lineage", "relationship_evidence", "id"), "stem_lineage"),
            (("lineage", "authority_artifacts"), []),
            (("lineage", "authority_artifacts", 0, "id"), "arbitrary_plan"),
            (("lineage", "authority_artifacts", 0, "byte_size"), 0),
            (("upstream_analysis_artifact", "id"), "unrelated_analysis"),
        ]
        for path, value in cases:
            with self.subTest(path=path, value=value):
                self.assertFalse(self.report_validator.is_valid(changed(example(), path, value)))

    def test_each_mandatory_authority_role_is_required(self):
        for index in range(7):
            document = example()
            del document["lineage"]["authority_artifacts"][index]
            with self.subTest(index=index):
                self.assertFalse(self.report_validator.is_valid(document))
        no_validations = example()
        no_validations["lineage"]["authority_artifacts"] = no_validations["lineage"]["authority_artifacts"][:5]
        self.assertFalse(self.report_validator.is_valid(no_validations))

    def test_declared_non_demucs_roles_and_upstream_signal_shape(self):
        document = example()
        document["lineage"]["source_stem_port"] = "declared_texture_layer"
        self.report_validator.validate(document)
        document["upstream_analysis_artifact"]["id"] = "signal_analysis"
        self.report_validator.validate(document)

    def test_configuration_rejects_unbound_or_unknown_fields(self):
        for path, value in [(("schema",), "aniflow.audio-stem.configuration/v2"), (("upstream_analysis_artifact_id",), "other"), (("unknown",), True), (("lineage", "scope", "channels"), [0])]:
            with self.subTest(path=path):
                self.assertFalse(self.configuration_validator.is_valid(changed(configuration(), path, value)))

    def test_schema_does_not_claim_byte_or_relational_validation(self):
        # Public Rust validation additionally binds exact duration, selected scope,
        # contiguous authority IDs and the configuration digest. Provider execution
        # rehashes retained bytes; structural validity is not evidence of a run.
        for path, value in [(("lineage", "range", "end"), 88200), (("lineage", "selected_stem", "frame_count"), 44000), (("configuration_sha256",), "a" * 64), (("lineage", "authority_artifacts", 6, "id"), "stem_authority_validation_9")]:
            with self.subTest(path=path):
                self.report_validator.validate(changed(example(), path, value))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lineage-report", type=Path, help="Validate an actual generated neutral stem-lineage companion.")
    parser.add_argument("--write-example", action="store_true", help="Regenerate the synthetic shape example deterministically.")
    arguments = parser.parse_args()
    if arguments.write_example:
        EXAMPLE.write_text(encoded(example()), encoding="utf-8")
    if arguments.lineage_report is not None:
        Draft202012Validator(load(CONTRACTS / "audio-stem-lineage-v1.schema.json")).validate(load(arguments.lineage_report))
        print(f"Validated generated stem lineage report: {arguments.lineage_report}")
    unittest.main(argv=[__file__])
