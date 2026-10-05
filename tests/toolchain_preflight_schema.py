#!/usr/bin/env python3
"""Plan-bound preflight contract cases, authored and unrun under issue #64."""

import copy
import json
from pathlib import Path
import unittest

import jsonschema


ROOT = Path(__file__).resolve().parents[1] / "docs" / "contracts"
NAMES = ("toolchain-preflight-v1", "toolchain-preflight-report-v1", "pipeline-v3-plan-v1")
SCHEMAS = {
    name: json.loads((ROOT / f"{name}.schema.json").read_text())
    for name in NAMES
}
EXAMPLES = {
    name: json.loads((ROOT / "examples" / f"{name}.example.json").read_text())
    for name in NAMES
}


class ToolchainPreflightSchema(unittest.TestCase):
    def validator(self, name):
        return jsonschema.Draft202012Validator(SCHEMAS[name])

    def test_published_shapes_preserve_closed_nested_boundaries(self):
        for name in NAMES:
            jsonschema.Draft202012Validator.check_schema(SCHEMAS[name])
            self.validator(name).validate(EXAMPLES[name])
        for path in [("bindings", 0), ("profile",), ("inventory", "artifacts", 0)]:
            document = copy.deepcopy(EXAMPLES["toolchain-preflight-v1"])
            target = document
            for part in path:
                target = target[part]
            target["execute"] = "not an invocation contract"
            self.assertFalse(self.validator("toolchain-preflight-v1").is_valid(document))

    def test_report_never_grants_native_qualification_or_omits_fresh_inspection(self):
        name = "toolchain-preflight-report-v1"
        for field, value in [("native_qualification", True), ("inspections", [])]:
            document = copy.deepcopy(EXAMPLES[name])
            document[field] = value
            self.assertFalse(self.validator(name).is_valid(document))
        document = copy.deepcopy(EXAMPLES[name])
        document["inspections"][0]["native_qualification"] = True
        self.assertFalse(self.validator(name).is_valid(document))

    def test_plan_guard_is_optional_but_never_null_or_malformed(self):
        name = "pipeline-v3-plan-v1"
        historical = copy.deepcopy(EXAMPLES[name])
        self.assertNotIn("toolchain_preflight_sha256", historical["payload"])
        self.validator(name).validate(historical)
        for value in [None, "", "a" * 63, "F" * 64]:
            document = copy.deepcopy(historical)
            document["payload"]["toolchain_preflight_sha256"] = value
            self.assertFalse(self.validator(name).is_valid(document))
        document = copy.deepcopy(historical)
        document["payload"]["toolchain_preflight_sha256"] = "a" * 64
        self.validator(name).validate(document)
        # Schema checks shape only; Rust must reject the unchanged plan digest.


if __name__ == "__main__":
    unittest.main()
