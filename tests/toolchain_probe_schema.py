#!/usr/bin/env python3
"""Observed probe contract cases, authored and unrun under issue #64."""

import copy
import hashlib
import json
from pathlib import Path
import unittest

import jsonschema

ROOT = Path(__file__).resolve().parents[1] / "docs" / "contracts"
NAMES = ("configuration", "report")
SCHEMAS = {
    name: json.loads((ROOT / f"toolchain-probe-{name}-v1.schema.json").read_text())
    for name in NAMES
}
EXAMPLES = {
    name: json.loads(
        (ROOT / "examples" / f"toolchain-probe-{name}-v1.example.json").read_text()
    )
    for name in NAMES
}


def captured(data):
    return {
        "retained_hex": data.hex(),
        "total_bytes": len(data),
        "truncated": False,
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def report_with_command():
    report = copy.deepcopy(EXAMPLES["report"])
    tool = report["tools"][0]
    tool["commands"] = [{
        "command_id": "version",
        "arguments": ["-version"],
        "outcome": "succeeded",
        "exit_code": 0,
        "stdout": captured(b"ffmpeg version 6.1.1 Copyright (c) synthetic fixture\n"),
        "stderr": captured(b""),
        "duration_milliseconds": 1,
    }]
    tool["before_sha256"] = report["configuration"]["tools"][0]["expected_sha256"]
    tool["after_sha256"] = tool["before_sha256"]
    # An incomplete report may retain a successful command without accepting an
    # observation. This fixture makes no claim that the rest of the probes ran.
    tool["status"] = "unverified"
    return report


def report_with_observation():
    report = report_with_command()
    report["configuration"]["tools"][0].update(dependency_id="ffprobe", profile="ffprobe")
    report["request_sha256"] = hashlib.sha256(json.dumps(
        report["configuration"], sort_keys=True, separators=(",", ":")
    ).encode()).hexdigest()
    tool = report["tools"][0]
    tool.update(dependency_id="ffprobe", profile="ffprobe", status="installed", diagnostics=[])
    tool["commands"][0]["stdout"] = captured(b"ffprobe version 6.1.1 Copyright (c) synthetic fixture\n")
    help_command = copy.deepcopy(tool["commands"][0])
    help_command.update(command_id="help_full", arguments=["-hide_banner", "-h", "full"])
    help_command["stdout"] = captured(b"Main options:\n-show_entries <entry_list>  select entries\n")
    tool["commands"].append(help_command)
    tool["version"] = {
        "raw_token": "6.1.1", "normalized_semver": "6.1.1", "normalization": "exact",
    }
    tool["observation"] = {
        "executable_sha256": tool["before_sha256"], "version": "6.1.1",
        "flags": ["-show_entries"], "features": [], "provenance": "synthetic observed output",
    }
    report["complete"] = True
    return report


class ToolchainProbeSchema(unittest.TestCase):
    def assert_rejected(self, name, document):
        self.assertFalse(jsonschema.Draft202012Validator(SCHEMAS[name]).is_valid(document))

    def test_authored_examples_and_partial_capture_are_versioned_contracts(self):
        for name in NAMES:
            jsonschema.Draft202012Validator.check_schema(SCHEMAS[name])
            jsonschema.Draft202012Validator(SCHEMAS[name]).validate(EXAMPLES[name])
        jsonschema.Draft202012Validator(SCHEMAS["report"]).validate(report_with_command())
        jsonschema.Draft202012Validator(SCHEMAS["report"]).validate(report_with_observation())

    def test_unknown_fields_cannot_add_argv_shell_media_install_or_environment(self):
        for field in ["arguments", "shell", "media", "install", "environment", "network"]:
            for path in [(), ("tools", 0), ("limits",)]:
                document = copy.deepcopy(EXAMPLES["configuration"])
                target = document
                for item in path:
                    target = target[item]
                target[field] = "caller cannot extend the fixed probe operation"
                self.assert_rejected("configuration", document)
        for path in [(), ("configuration",), ("tools", 0),
                     ("tools", 0, "commands", 0),
                     ("tools", 0, "commands", 0, "stdout"),
                     ("tools", 0, "version"), ("tools", 0, "observation")]:
            document = report_with_observation()
            target = document
            for item in path:
                target = target[item]
            target["qualified"] = True
            self.assert_rejected("report", document)

    def test_fixed_tool_profiles_explicit_paths_and_exact_pins_are_required(self):
        for field, value in [
            ("profile", "arbitrary-tool"), ("path", "ffmpeg"),
            ("expected_sha256", "a" * 63), ("expected_sha256", "F" * 64),
            ("dependency_id", "two words"),
        ]:
            document = copy.deepcopy(EXAMPLES["configuration"])
            document["tools"][0][field] = value
            self.assert_rejected("configuration", document)
        for field in ["schema", "tools", "limits"]:
            document = copy.deepcopy(EXAMPLES["configuration"])
            del document[field]
            self.assert_rejected("configuration", document)
        document = copy.deepcopy(EXAMPLES["configuration"])
        document["tools"] = []
        self.assert_rejected("configuration", document)

    def test_execution_and_retention_budgets_have_closed_numeric_bounds(self):
        for field in EXAMPLES["configuration"]["limits"]:
            for value in [-1, 0, 2**64, "unlimited"]:
                document = copy.deepcopy(EXAMPLES["configuration"])
                document["limits"][field] = value
                self.assert_rejected("configuration", document)

    def test_capture_hex_and_process_outcomes_do_not_conflate_acceptance(self):
        for field, value in [("retained_hex", "0"), ("retained_hex", "GG"),
                             ("total_bytes", -1), ("sha256", "not-a-digest")]:
            document = report_with_command()
            document["tools"][0]["commands"][0]["stdout"][field] = value
            self.assert_rejected("report", document)
        for outcome in ["succeeded", "failed", "cancelled", "timed_out", "output_limit",
                        "spawn_failed", "capture_failed", "cleanup_failed"]:
            document = report_with_command()
            document["tools"][0]["commands"][0]["outcome"] = outcome
            jsonschema.Draft202012Validator(SCHEMAS["report"]).validate(document)
        document = report_with_command()
        document["tools"][0]["commands"][0]["outcome"] = "qualified"
        self.assert_rejected("report", document)
        document = report_with_command()
        document["tools"][0]["commands"][0]["arguments"] = ["-i", "source.wav"]
        self.assert_rejected("report", document)
        for field, value in [("exit_code", 17), ("signal", 9)]:
            document = report_with_command()
            document["tools"][0]["commands"][0][field] = value
            self.assert_rejected("report", document)

    def test_observation_and_normalized_version_cannot_claim_native_qualification(self):
        for name in NAMES:
            document = copy.deepcopy(EXAMPLES[name])
            document["schema"] = f"aniflow.toolchain-probe-{name}/v2"
            self.assert_rejected(name, document)
        document = report_with_command()
        document["native_qualification"] = True
        self.assert_rejected("report", document)
        document = report_with_observation()
        document["tools"][0]["version"]["normalization"] = "guessed_vendor_version"
        self.assert_rejected("report", document)
        for field in ["executable_sha256", "flags", "features", "provenance"]:
            document = report_with_observation()
            del document["tools"][0]["observation"][field]
            self.assert_rejected("report", document)
        document = report_with_observation()
        del document["tools"][0]["version"]["normalization"]
        self.assert_rejected("report", document)
        document = report_with_observation()
        document["complete"] = False
        self.assert_rejected("report", document)


if __name__ == "__main__":
    unittest.main()
