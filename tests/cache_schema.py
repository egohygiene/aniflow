#!/usr/bin/env python3
"""Closed cache contracts authored for #34; execution deferred under #64."""
import copy
import json
from pathlib import Path
import unittest
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1] / "docs" / "contracts"
SCHEMAS = {path.name: json.loads(path.read_text()) for path in ROOT.glob("*.schema.json")}
REGISTRY = Registry().with_resources((schema["$id"], Resource.from_contents(schema)) for schema in SCHEMAS.values())


def example(name):
    return json.loads((ROOT / "examples" / (name + ".example.json")).read_text())


def validator(name):
    return Draft202012Validator(SCHEMAS[name + ".schema.json"], registry=REGISTRY)


class CacheContracts(unittest.TestCase):
    def test_closed_examples(self):
        for name in ["cache-policy-v1", "cache-entry-v1", "cache-inspection-v1", "cache-operation-v1", "cache-decision-v1", "cache-diagnostic-v1"]:
            with self.subTest(name=name):
                value = example(name)
                validator(name).validate(value)
                value["unknown_authority"] = True
                self.assertTrue(list(validator(name).iter_errors(value)))

    def test_invalid_bounds_and_unowned_paths(self):
        for key, value in [("maximum_entries", 0), ("maximum_entry_bytes", 0), ("maximum_age_seconds", 0), ("owner", "../other")]:
            policy = example("cache-policy-v1")
            policy[key] = value
            self.assertTrue(list(validator("cache-policy-v1").iter_errors(policy)))

    def test_entry_inventory_requires_bound_file_bytes(self):
        entry = example("cache-entry-v1")
        for change in [{"sha256": None}, {"relative_path": "../outside"}, {"kind": "symlink"}]:
            altered = copy.deepcopy(entry)
            altered["payload"]["contents"][0].update(change)
            self.assertTrue(list(validator("cache-entry-v1").iter_errors(altered)))

    def test_future_schema_and_duplicate_evictions_fail(self):
        entry = example("cache-entry-v1")
        entry["schema"] = "aniflow.cache-entry/v999"
        self.assertTrue(list(validator("cache-entry-v1").iter_errors(entry)))
        operation = example("cache-operation-v1")
        operation["selected_keys"] *= 2
        self.assertTrue(list(validator("cache-operation-v1").iter_errors(operation)))


if __name__ == "__main__":
    unittest.main()
