#!/usr/bin/env python3
"""Offline schema coverage for #33. Run explicitly during deferred qualification."""
import copy
import json
from pathlib import Path
import unittest
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1] / "docs" / "contracts"
SCHEMAS = {path.name: json.loads(path.read_text()) for path in ROOT.glob("*.schema.json")}
REGISTRY = Registry().with_resources(
    (schema["$id"], Resource.from_contents(schema)) for schema in SCHEMAS.values()
)


def validator(name):
    return Draft202012Validator(SCHEMAS[name + ".schema.json"], registry=REGISTRY)


def example(name):
    return json.loads((ROOT / "examples" / (name + ".example.json")).read_text())


class ValidationSchemas(unittest.TestCase):
    def test_published_examples_and_all_four_layers(self):
        names = ["validation-context-v1", "validator-observation-v1", "validation-report-v1", "acceptance-record-v1", "validation-diagnostic-v1", "native-validation-configuration-v1"]
        for name in names:
            with self.subTest(name=name):
                Draft202012Validator.check_schema(SCHEMAS[name + ".schema.json"])
                validator(name).validate(example(name))
        for layer in ["component", "stage", "candidate-master"]:
            validator("acceptance-record-v1").validate(example("acceptance-" + layer + "-v1"))

    def test_provider_shortcut_and_unknown_fields_are_rejected(self):
        observation = example("validator-observation-v1")
        observation["accepted"] = True
        self.assertTrue(list(validator("validator-observation-v1").iter_errors(observation)))
        context = example("validation-context-v1")
        context["validator_lock_sha256"] = "not-a-digest"
        self.assertTrue(list(validator("validation-context-v1").iter_errors(context)))

    def test_partial_observation_is_representable_but_not_delivery_truth(self):
        observation = example("validator-observation-v1")
        observation["disposition"] = "partial"
        validator("validator-observation-v1").validate(observation)
        # Rust semantic acceptance tests establish that this cannot complete.

    def test_evidence_path_traversal_and_tool_boundaries_are_rejected(self):
        delivery = example("acceptance-record-v1")
        delivery["children"][0]["relative_path"] = "../outside.json"
        self.assertTrue(list(validator("acceptance-record-v1").iter_errors(delivery)))
        settings = example("native-validation-configuration-v1")
        for field, value in [("maximum_media_bytes", 0), ("decode_timeout_seconds", 3601)]:
            invalid = copy.deepcopy(settings)
            invalid[field] = value
            self.assertTrue(list(validator("native-validation-configuration-v1").iter_errors(invalid)))


if __name__ == "__main__":
    unittest.main()
