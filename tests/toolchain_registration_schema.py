#!/usr/bin/env python3
"""Registration-preparation contract cases, authored and unrun under issue #64."""

import copy
import hashlib
import json
from pathlib import Path
import unittest

import jsonschema


REPOSITORY = Path(__file__).resolve().parents[1]
ROOT = REPOSITORY / "docs" / "contracts"
NAMES = ("toolchain-audio-inspection-registration-v1", "toolchain-registration-preparation-v1")
SCHEMAS = {
    name: json.loads((ROOT / f"{name}.schema.json").read_text()) for name in NAMES
}
EXAMPLES = {
    name: json.loads((ROOT / "examples" / f"{name}.example.json").read_text())
    for name in NAMES
}
REQUEST, RESULT = NAMES


def digest(value):
    return hashlib.sha256(json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()).hexdigest()


def ready_shape():
    """Author transport data, not a locally observed preparation receipt."""
    request = copy.deepcopy(EXAMPLES[REQUEST])
    manifest = json.loads((REPOSITORY / "providers/audio-inspection/manifest.json").read_text())
    settings = {
        "schema": "aniflow.audio-inspection.configuration/v1",
        "tool_timeout_milliseconds": request["tool_timeout_milliseconds"],
        "maximum_tool_output_bytes": request["maximum_tool_output_bytes"],
    }
    components = []
    for tool in request["preflight"]["inventory"]["artifacts"]:
        identifier = tool["dependency_id"]
        settings[identifier] = {
            "executable": tool["path"], "sha256": tool["expected_sha256"],
            "version": tool["observation"]["version"],
        }
        components.append({
            "id": identifier, "version": tool["observation"]["version"],
            "sha256": tool["expected_sha256"],
        })
    values = {
        "schema": "aniflow.audio-inspection.provider-configuration/v2",
        "settings": settings, "source": request["source"],
    }
    configuration = {
        "schema": "aniflow.provider-configuration/v1",
        "provider": {"id": manifest["provider"]["id"], "version": "2.0.0"},
        "capability": {"id": manifest["capabilities"][0]["id"], "version": "2.0.0"},
        "configuration_schema": manifest["configuration_schemas"][0],
        "values": values, "effective_configuration_sha256": digest(values),
    }
    registration = {
        "schema": "aniflow.provider-registration/v1",
        "registration_id": request["preflight"]["bindings"][0]["registration_id"],
        "manifest": "manifest.json", "configuration": "configuration.json",
        "executable": request["adapter"]["relative_path"],
        "implementation_id": request["adapter"]["implementation_id"],
        "components": {"tools": components, "codecs": [], "models": []},
    }
    result = copy.deepcopy(EXAMPLES[RESULT])
    result.update(
        request_sha256=digest(request), ready=True, diagnostics=[],
        adapter={"id": request["adapter"]["implementation_id"],
                 "executable_sha256": request["adapter"]["expected_sha256"]},
    )
    result["inspection"].update(ready=True, facts=[], actions=[])
    result["files"] = [
        {"relative_path": filename, "content": value, "sha256": digest(value)}
        for filename, value in [
            ("configuration.json", configuration), ("manifest.json", manifest),
            ("preflight.json", request["preflight"]), ("registration.json", registration),
        ]
    ]
    return result


class ToolchainRegistrationSchema(unittest.TestCase):
    def validator(self, name):
        return jsonschema.Draft202012Validator(SCHEMAS[name])

    def reject(self, name, document):
        self.assertFalse(self.validator(name).is_valid(document))

    def test_authored_request_incomplete_result_and_ready_file_shapes(self):
        for name in NAMES:
            jsonschema.Draft202012Validator.check_schema(SCHEMAS[name])
            self.validator(name).validate(EXAMPLES[name])
        self.validator(RESULT).validate(ready_shape())

    def test_closed_request_cannot_gain_execution_install_or_environment_fields(self):
        for path in [(), ("adapter",), ("source",), ("preflight",),
                     ("preflight", "bindings", 0),
                     ("preflight", "inventory", "artifacts", 0)]:
            for field in ["execute", "install", "arguments", "environment"]:
                document = copy.deepcopy(EXAMPLES[REQUEST])
                target = document
                for item in path:
                    target = target[item]
                target[field] = "not registration-preparation authority"
                self.reject(REQUEST, document)

    def test_request_has_one_binding_explicit_source_and_bounded_pins(self):
        for count in [0, 2]:
            document = copy.deepcopy(EXAMPLES[REQUEST])
            document["preflight"]["bindings"] *= count
            self.reject(REQUEST, document)
        for field, value in [("id", "unbound_audio"), ("sha256", "F" * 64),
                             ("byte_size", 43), ("byte_size", 268435457)]:
            document = copy.deepcopy(EXAMPLES[REQUEST])
            document["source"][field] = value
            self.reject(REQUEST, document)
        for field, values in {
            "tool_timeout_milliseconds": [0, 120001],
            "maximum_tool_output_bytes": [1023, 1048577],
        }.items():
            for value in values:
                document = copy.deepcopy(EXAMPLES[REQUEST])
                document[field] = value
                self.reject(REQUEST, document)
        for value in [0, 1073741825]:
            document = copy.deepcopy(EXAMPLES[REQUEST])
            document["adapter"]["maximum_bytes"] = value
            self.reject(REQUEST, document)

    def test_adapter_locator_remains_confined_and_directory_remains_absolute(self):
        for value in ["/absolute/aniflow", "../aniflow", "bin/../aniflow",
                      "bin//aniflow", "bin\\aniflow", "bin/aniflow\n", "CON",
                      "registration.json", "Manifest.JSON/aniflow"]:
            document = copy.deepcopy(EXAMPLES[REQUEST])
            document["adapter"]["relative_path"] = value
            self.reject(REQUEST, document)
        for value in ["bundle", "", "/absolute/../bundle", "/absolute/bundle\n"]:
            document = copy.deepcopy(EXAMPLES[REQUEST])
            document["registration_directory"] = value
            self.reject(REQUEST, document)
        # Filesystem existence, nonsymlink targets and permission checks belong
        # to Rust preparation; JSON Schema cannot attest a usable local path.

    def test_ready_requires_all_four_ordered_files_and_observed_adapter(self):
        for mutate in [
            lambda value: value.pop("adapter"),
            lambda value: value["files"].pop(),
            lambda value: value["files"].reverse(),
            lambda value: value["files"].append(value["files"][0]),
            lambda value: value["inspection"].update(ready=False),
            lambda value: value.update(diagnostics=["dependency refused"]),
        ]:
            document = ready_shape()
            mutate(document)
            self.reject(RESULT, document)
        document = ready_shape()
        document["ready"] = False
        self.reject(RESULT, document)

    def test_generated_files_are_closed_native_v2_documents_without_qualification(self):
        for path in [(), ("adapter",), ("files", 0),
                     ("files", 0, "content", "values"),
                     ("files", 0, "content", "values", "settings"),
                     ("files", 3, "content")]:
            document = ready_shape()
            target = document
            for item in path:
                target = target[item]
            target["qualified"] = True
            self.reject(RESULT, document)
        for path in [(), ("inspection",)]:
            document = ready_shape()
            target = document
            for item in path:
                target = target[item]
            target["native_qualification"] = True
            self.reject(RESULT, document)
        document = ready_shape()
        document["files"][0]["content"]["values"]["schema"] = (
            "aniflow.audio-inspection.provider-configuration/v1"
        )
        self.reject(RESULT, document)
        document = ready_shape()
        document["files"][3]["content"]["executable"] = "../aniflow"
        self.reject(RESULT, document)
        # Digest recomputation and correspondence to fresh files are Rust
        # semantic checks, separate from these authored transport cases.


if __name__ == "__main__":
    unittest.main()
