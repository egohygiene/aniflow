#!/usr/bin/env python3
"""Offline profile contract cases, authored and unrun under issue #64."""

import copy
import json
from pathlib import Path
import unittest

import jsonschema

ROOT = Path(__file__).resolve().parents[1] / "docs" / "contracts"
NAMES = ("profile", "inventory", "report")
SCHEMAS = {
    name: json.loads((ROOT / f"toolchain-{name}-v1.schema.json").read_text())
    for name in NAMES
}
EXAMPLES = {
    name: json.loads((ROOT / "examples" / f"toolchain-{name}-v1.example.json").read_text())
    for name in NAMES
}


class ToolchainSchema(unittest.TestCase):
    def assert_rejected(self, name, document):
        self.assertFalse(jsonschema.Draft202012Validator(SCHEMAS[name]).is_valid(document))

    def test_published_documents_are_closed_versioned_examples(self):
        for name in NAMES:
            jsonschema.Draft202012Validator.check_schema(SCHEMAS[name])
            jsonschema.Draft202012Validator(SCHEMAS[name]).validate(EXAMPLES[name])

    def test_future_versions_and_unknown_root_fields_are_refused(self):
        for name in NAMES:
            for key, value in [("schema", f"aniflow.toolchain-{name}/v2"), ("execute", True)]:
                document = copy.deepcopy(EXAMPLES[name])
                document[key] = value
                self.assert_rejected(name, document)

    def test_nested_objects_do_not_admit_commands_or_unobserved_authority(self):
        objects = {
            "profile": [("dependencies", 0), ("capabilities", 0), ("capabilities", 1, "backend"), ("capabilities", 1, "scale")],
            "inventory": [("platform",), ("artifacts", 0), ("artifacts", 0, "observation")],
            "report": [("facts", 0), ("actions", 0)],
        }
        for name, paths in objects.items():
            for path in paths:
                document = copy.deepcopy(EXAMPLES[name])
                target = document
                for item in path:
                    target = target[item]
                target["execute"] = "shell command is not part of this contract"
                self.assert_rejected(name, document)

    def test_unqualified_report_cannot_claim_native_qualification(self):
        document = copy.deepcopy(EXAMPLES["report"])
        document["native_qualification"] = True
        self.assert_rejected("report", document)
        document = copy.deepcopy(EXAMPLES["report"])
        document["facts"][0]["status"] = "qualified"
        self.assert_rejected("report", document)

    def test_observations_need_bound_identity_and_explicit_provenance(self):
        for key in ["executable_sha256", "provenance", "flags", "features"]:
            document = copy.deepcopy(EXAMPLES["inventory"])
            del document["artifacts"][0]["observation"][key]
            self.assert_rejected("inventory", document)
        document = copy.deepcopy(EXAMPLES["inventory"])
        document["artifacts"][0]["observation"]["native_scale"] = 0
        self.assert_rejected("inventory", document)

    def test_digest_path_scale_and_budget_boundaries_are_explicit(self):
        for value in ["not-a-digest", "F" * 64, "a" * 63]:
            document = copy.deepcopy(EXAMPLES["inventory"])
            document["artifacts"][0]["expected_sha256"] = value
            self.assert_rejected("inventory", document)
        for value in [0, 1_073_741_825]:
            document = copy.deepcopy(EXAMPLES["inventory"])
            document["artifacts"][0]["maximum_bytes"] = value
            self.assert_rejected("inventory", document)
        document = copy.deepcopy(EXAMPLES["inventory"])
        document["artifacts"][0]["path"] = "relative/tool"
        self.assert_rejected("inventory", document)
        for field in ["native", "requested"]:
            document = copy.deepcopy(EXAMPLES["profile"])
            document["capabilities"][1]["scale"][field] = 65
            self.assert_rejected("profile", document)

    def test_optional_model_templates_do_not_invent_downloads_or_pins(self):
        document = copy.deepcopy(EXAMPLES["profile"])
        del document["dependencies"][2]["sha256"]
        jsonschema.Draft202012Validator(SCHEMAS["profile"]).validate(document)
        document["dependencies"][2]["suggested_locators"] = ["file:///etc/passwd"]
        self.assert_rejected("profile", document)
        document = copy.deepcopy(EXAMPLES["profile"])
        document["dependencies"][0]["native_scale"] = 4
        self.assert_rejected("profile", document)


if __name__ == "__main__":
    unittest.main()
