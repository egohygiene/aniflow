#!/usr/bin/env python3
"""Closed catalog contract tests, authored for the deferred qualification pass."""

import copy
import json
from pathlib import Path
import unittest

import jsonschema

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = json.loads((ROOT / "conformance/temporal-v1/catalog.schema.json").read_text())
CATALOG = json.loads((ROOT / "conformance/temporal-v1/catalog.json").read_text())
VALIDATOR = jsonschema.Draft202012Validator(SCHEMA)


class CorpusSchema(unittest.TestCase):
    def test_catalog_and_schema_are_valid(self):
        jsonschema.Draft202012Validator.check_schema(SCHEMA)
        VALIDATOR.validate(CATALOG)

    def test_unknown_fields_and_future_versions_are_refused(self):
        for field, value in [("surprise", True), ("schema", "aniflow.adversarial-corpus/v2")]:
            document = copy.deepcopy(CATALOG)
            document[field] = value
            self.assertFalse(VALIDATOR.is_valid(document))

    def test_source_and_inventory_paths_and_digests_are_constrained(self):
        for path, digest in [("../escape", "a" * 64), ("safe", "not-a-digest"), ("/root", "b" * 64)]:
            document = copy.deepcopy(CATALOG)
            document["sources"][0].update({"path": path, "sha256": digest})
            self.assertFalse(VALIDATOR.is_valid(document))

    def test_catalog_never_claims_executed_qualification(self):
        document = copy.deepcopy(CATALOG)
        document["qualification"] = "passed"
        self.assertFalse(VALIDATOR.is_valid(document))
        document = copy.deepcopy(CATALOG)
        document["cases"][0]["qualification"] = "passed"
        self.assertFalse(VALIDATOR.is_valid(document))


if __name__ == "__main__":
    unittest.main()
