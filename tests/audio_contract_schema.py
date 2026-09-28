#!/usr/bin/env python3
"""Independent Draft 2020-12 checks for the normalized audio contract.

Requires an already installed jsonschema package; this script never installs
anything or downloads models. Run directly with Python. These structural checks
complement tests/audio_analysis_contracts.rs, which owns cross-reference, range
ordering, scope consistency and other relational checks that JSON Schema cannot
express. Source bytes are synthetic in-memory PCM; no real media is opened.
"""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import unittest
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit("Audio schema checks require an already installed Python jsonschema package; no installation was attempted.") from error

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "docs" / "contracts" / "audio-analysis-v1.schema.json"
SPEC = importlib.util.spec_from_file_location("audio_fixture_generator", ROOT / "scripts" / "generate-audio-contract-fixtures.py")
assert SPEC is not None and SPEC.loader is not None
FIXTURES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURES)


def reject_nonfinite(value: str):
    raise ValueError(f"Non-finite JSON token: {value}")


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject_nonfinite)


def changed(document: dict, path: tuple, value):
    result = copy.deepcopy(document)
    target = result
    for component in path[:-1]:
        target = target[component]
    target[path[-1]] = value
    return result


class AudioContractSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = load(SCHEMA_PATH)
        Draft202012Validator.check_schema(cls.schema)
        cls.validator = Draft202012Validator(cls.schema)
        cls.examples = FIXTURES.examples()

    def document(self, kind):
        return self.examples[f"audio-analysis-{kind}-v1.example.json"]

    def assert_rejected(self, document):
        self.assertFalse(self.validator.is_valid(document), "Malformed contract passed structural validation")

    def test_generated_examples_are_current_and_schema_valid(self):
        for name, expected in self.examples.items():
            with self.subTest(example=name):
                path = FIXTURES.EXAMPLES / name
                self.assertEqual(path.read_text(encoding="utf-8"), FIXTURES.encoded(expected))
                self.validator.validate(load(path))

    def test_synthetic_artifact_digests_have_reproducible_bytes(self):
        for name, document in self.examples.items():
            with self.subTest(example=name):
                source = document["source"]["artifact"]
                self.assertEqual(source, FIXTURES.artifact(source["id"], FIXTURES.synthetic_source()))
                identities = list(document["artifacts"])
                if document["source"]["stem"] is not None:
                    identities.append(document["source"]["stem"]["original_mix"])
                for identity in identities:
                    payload = FIXTURES.synthetic_payload(identity["id"])
                    self.assertEqual(identity["sha256"], hashlib.sha256(payload).hexdigest())
                    self.assertEqual(identity["byte_size"], len(payload))

    def test_rejects_unknown_fields_at_every_contract_layer(self):
        rich = self.document("timeline")
        rich_paths = [(), ("source",), ("source", "artifact"), ("source", "origin"), ("source", "stem"), ("providers", 0), ("providers", 0, "provider"), ("providers", 0, "license"), ("providers", 0, "tools", 0), ("providers", 0, "models"), ("providers", 0, "models", "components", 0), ("capabilities", 0), ("capabilities", 0, "capability"), ("timelines", 0), ("timelines", 0, "scope"), ("timelines", 0, "events", 0), ("timelines", 0, "events", 0, "range"), ("timelines", 0, "events", 0, "provenance"), ("timelines", 0, "events", 0, "provenance", "confidence"), ("semantic_artifacts", 1), ("semantic_artifacts", 1, "authority"), ("excerpts", 0)]
        technical = self.document("technical")
        paths = [(rich, path) for path in rich_paths] + [(technical, ("observations", 0)), (technical, ("observations", 0, "value"))]
        for document, path in paths:
            with self.subTest(path=path):
                malformed = copy.deepcopy(document)
                node = malformed
                for part in path:
                    node = node[part]
                node["undeclared"] = True
                self.assert_rejected(malformed)

    def test_rejects_invalid_types_units_confidence_and_identities(self):
        cases = [
            ("technical", ("schema",), "aniflow.audio-analysis/v2"),
            ("technical", ("source", "sample_rate_hz"), 0),
            ("technical", ("source", "channels"), 65),
            ("technical", ("source", "frame_count"), 0),
            ("technical", ("source", "frame_count"), 9007199254740992),
            ("technical", ("source", "artifact", "byte_size"), 0),
            ("technical", ("source", "artifact", "sha256"), "invalid"),
            ("technical", ("source", "artifact", "id"), "fixture\n"),
            ("technical", ("source", "artifact", "sha256"), "a" * 64 + "\n"),
            ("technical", ("providers", 0, "provider", "version"), "1.0.0\n"),
            ("technical", ("providers", 0, "provider", "id"), "org.example.fixture\n"),
            ("estimated", ("observations", 1, "value", "value"), "C major\n"),
            ("technical", ("source", "origin", "numerator"), 1),
            ("technical", ("source", "origin", "denominator"), 0),
            ("technical", ("source", "stream_index"), 1),
            ("technical", ("observations", 0, "value", "value"), "48000"),
            ("technical", ("observations", 0, "value", "unit"), "seconds"),
            ("technical", ("observations", 0, "value", "unit"), "channels"),
            ("technical", ("observations", 0, "provenance", "class"), "probabilistic"),
            ("technical", ("observations", 0, "scope", "channels"), [0, 0]),
            ("technical", ("observations", 0, "scope", "channels"), [-1]),
            ("technical", ("providers", 0, "provider", "version"), "01.0.0"),
            ("technical", ("capabilities", 0, "capability", "id"), "vendor/custom-capability"),
            ("technical", ("capabilities", 0, "provider_evidence_ids"), []),
            ("estimated", ("observations", 0, "provenance", "confidence", "score"), 1.01),
            ("estimated", ("observations", 0, "provenance", "confidence", "score"), -0.01),
            ("estimated", ("observations", 0, "provenance", "confidence"), {"kind": "not_applicable"}),
            ("estimated", ("observations", 0, "value", "value"), 0),
            ("estimated", ("observations", 0, "value", "value"), 1001),
            ("estimated", ("observations", 0, "value", "unit"), "hertz"),
            ("unavailable", ("capabilities", 0, "diagnostic_ids"), []),
            ("timeline", ("timelines", 0, "events", 0, "range", "start"), -1),
            ("timeline", ("timelines", 0, "events", 0, "range", "end"), 0),
            ("timeline", ("semantic_artifacts", 0, "provenance", "class"), "heuristic"),
            ("timeline", ("semantic_artifacts", 0, "capability_id"), "aniflow/audio-timed-text"),
            ("timeline", ("semantic_artifacts", 1, "authority"), None),
            ("timeline", ("semantic_artifacts", 2, "authority"), {"supplied_by": "fixture", "provenance_artifact_id": "review_record"}),
        ]
        for kind, path, value in cases:
            with self.subTest(kind=kind, path=path, value=value):
                self.assert_rejected(changed(self.document(kind), path, value))

    def test_required_core_evidence_cannot_be_replaced_by_extensions(self):
        document = copy.deepcopy(self.document("technical"))
        document["extensions"]["org.egohygiene.fixture"]["providers"] = document.pop("providers")
        self.assert_rejected(document)
        document = copy.deepcopy(self.document("technical"))
        document["extensions"]["not-namespaced"] = {}
        self.assert_rejected(document)

    def test_opaque_namespaced_extensions_preserve_unknown_payloads(self):
        document = copy.deepcopy(self.document("technical"))
        document["extensions"]["org.example.fixture"] = {"future": [{"arbitrary": True}], "null": None}
        self.validator.validate(document)

    def test_scalar_json_parser_rejects_nonfinite_tokens(self):
        for token in ["NaN", "Infinity", "-Infinity"]:
            with self.subTest(token=token), self.assertRaises(ValueError):
                json.loads('{"value": ' + token + '}', parse_constant=reject_nonfinite)

    def test_schema_does_not_claim_relational_validation(self):
        document = self.document("timeline")
        for path, value in [(("timelines", 0, "events", 0, "range", "end"), 999999), (("timelines", 0, "events", 0, "provenance", "provider_evidence_id"), "unknown_provider")]:
            with self.subTest(path=path):
                self.validator.validate(changed(document, path, value))
        # The Rust acceptance path rejects both mutations. This test records why
        # schema success alone is not contract acceptance or media validation.


if __name__ == "__main__":
    unittest.main()
