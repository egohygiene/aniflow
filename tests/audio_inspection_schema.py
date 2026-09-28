#!/usr/bin/env python3
"""Independent JSON Schema checks for the bounded audio inspection contracts.

Requires already installed jsonschema. Synthetic report values below exercise
structure only: no analyzer result is fabricated as execution evidence. Optional
--technical-report/--analysis-report paths validate actual locally generated
reports. Relational timing, digests, command equality and source binding remain
public Rust validator responsibilities.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import unittest
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit("Audio inspection schema checks require an already installed Python jsonschema package; no installation was attempted.") from error

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "providers" / "audio-inspection"
CONTRACTS = ROOT / "docs" / "contracts"
PROBE_ARGUMENTS = ["-v", "error", "-protocol_whitelist", "file,pipe", "-show_entries", "stream=index,codec_type,codec_name,sample_fmt,sample_rate,channels,bits_per_sample,time_base,duration_ts,start_pts,bit_rate:format=format_name,nb_streams", "-of", "json", "-i", "{snapshot}"]
DECODE_ARGUMENTS = ["-v", "error", "-nostdin", "-xerror", "-err_detect", "explode", "-threads", "1", "-protocol_whitelist", "file,pipe", "-i", "{snapshot}", "-map", "0:a:0", "-vn", "-sn", "-dn", "-codec:a", "pcm_s16le", "-f", "hash", "-hash", "sha256", "pipe:1"]


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def synthetic_digest(label: str) -> str:
    return hashlib.sha256(f"Synthetic schema fixture: {label}".encode()).hexdigest()


def configuration():
    return {
        "schema": "aniflow.audio-inspection.configuration/v1",
        "ffmpeg": {"executable": "/synthetic/tools/ffmpeg", "version": "6.1.1-3ubuntu5", "sha256": synthetic_digest("ffmpeg")},
        "ffprobe": {"executable": "/synthetic/tools/ffprobe", "version": "6.1.1-3ubuntu5", "sha256": synthetic_digest("ffprobe")},
        "tool_timeout_milliseconds": 30000,
        "maximum_tool_output_bytes": 65536,
    }


def provider_configuration():
    return {
        "schema": "aniflow.audio-inspection.provider-configuration/v1",
        "settings": configuration(),
        "source": {"id": "source_audio", "sha256": synthetic_digest("source"), "byte_size": 192044},
    }


def technical_report():
    return {
        "schema": "aniflow.audio-technical-inspection/v1",
        "source": {"id": "source_audio", "sha256": synthetic_digest("source"), "byte_size": 192044},
        "provider": {"id": "org.egohygiene.aniflow.audio-inspection", "version": "1.0.0"},
        "implementation_sha256": synthetic_digest("implementation"),
        "configuration_sha256": synthetic_digest("configuration"),
        "provider_lock_sha256": synthetic_digest("lock"),
        "tools": [{"id": tool, "version": "6.1.1-3ubuntu5", "sha256": synthetic_digest(tool)} for tool in ["ffmpeg", "ffprobe"]],
        "container": "wav",
        "codec": "pcm_s16le",
        "sample_format": "s16",
        "stream_index": 0,
        "sample_rate_hz": 48000,
        "channels": 2,
        "frame_count": 48000,
        "duration": {"numerator": 1, "denominator": 1},
        "pcm_bitrate_bits_per_second": 1536000,
        "pcm_sha256": synthetic_digest("pcm"),
        "decoded_pcm_sha256": synthetic_digest("pcm"),
        "decode_complete": True,
        "source_unchanged": True,
        "commands": [{"tool": tool, "arguments": arguments} for tool, arguments in [("ffprobe", PROBE_ARGUMENTS), ("ffmpeg", DECODE_ARGUMENTS)]],
    }


def changed(document, path, value):
    result = copy.deepcopy(document)
    node = result
    for component in path[:-1]:
        node = node[component]
    node[path[-1]] = value
    return result


class AudioInspectionSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.configuration_schema = load(BUNDLE / "configuration.schema.json")
        cls.provider_configuration_schema = load(BUNDLE / "provider-configuration.schema.json")
        cls.technical_schema = load(CONTRACTS / "audio-technical-inspection-v1.schema.json")
        cls.preflight_schema = load(CONTRACTS / "audio-inspection-preflight-v1.schema.json")
        for schema in [cls.configuration_schema, cls.provider_configuration_schema, cls.technical_schema, cls.preflight_schema]:
            Draft202012Validator.check_schema(schema)
        cls.configuration_validator = Draft202012Validator(cls.configuration_schema)
        cls.provider_configuration_validator = Draft202012Validator(cls.provider_configuration_schema)
        cls.technical_validator = Draft202012Validator(cls.technical_schema)
        cls.preflight_validator = Draft202012Validator(cls.preflight_schema)

    def test_synthetic_configuration_and_report_structures(self):
        self.configuration_validator.validate(configuration())
        self.provider_configuration_validator.validate(provider_configuration())
        self.technical_validator.validate(technical_report())

    def test_provider_schema_embeds_identical_settings(self):
        expected = {key: value for key, value in self.configuration_schema.items() if key not in ["$schema", "$id", "$defs", "title", "description"]}
        self.assertEqual(self.provider_configuration_schema["$defs"]["settings"], expected)
        for name, definition in self.configuration_schema["$defs"].items():
            self.assertEqual(self.provider_configuration_schema["$defs"][name], definition)

    def test_manifest_uses_exact_provider_schema_bytes(self):
        manifest = load(BUNDLE / "manifest.json")
        Draft202012Validator(load(CONTRACTS / "provider-manifest-v1.schema.json")).validate(manifest)
        expected = {"id": "aniflow.audio-inspection.provider-configuration/v1", "version": "1.0.0", "sha256": hashlib.sha256((BUNDLE / "provider-configuration.schema.json").read_bytes()).hexdigest()}
        self.assertEqual(manifest["configuration_schemas"], [expected])
        self.assertEqual(manifest["capabilities"][0]["configuration_schema"], expected)

    def test_configuration_rejects_unknown_fields_invalid_pins_and_bounds(self):
        cases = [
            (("extra",), True),
            (("schema",), "aniflow.audio-inspection.configuration/v2"),
            (("ffmpeg", "unknown"), True),
            (("ffmpeg", "executable"), "ffmpeg"),
            (("ffmpeg", "executable"), "/tools/../ffmpeg"),
            (("ffmpeg", "executable"), "/tools/./ffmpeg"),
            (("ffmpeg", "executable"), "/tools/ffmpeg\n"),
            (("ffmpeg", "version"), "nightly"),
            (("ffmpeg", "version"), "6.1.1\n"),
            (("ffmpeg", "version"), "06.1.1"),
            (("ffmpeg", "sha256"), "a" * 64 + "\n"),
            (("ffmpeg", "sha256"), "not-a-digest"),
            (("tool_timeout_milliseconds",), 0),
            (("tool_timeout_milliseconds",), 120001),
            (("maximum_tool_output_bytes",), 1023),
            (("maximum_tool_output_bytes",), 1048577),
            (("maximum_tool_output_bytes",), "65536"),
        ]
        for path, value in cases:
            with self.subTest(path=path, value=value):
                document = changed(configuration(), path, value)
                self.assertFalse(self.configuration_validator.is_valid(document))
                self.assertFalse(self.provider_configuration_validator.is_valid({**provider_configuration(), "settings": document}))

    def test_provider_wrapper_requires_exact_source_identity(self):
        for path, value in [(("source", "id"), "another_source"), (("source", "sha256"), "invalid"), (("source", "byte_size"), 43), (("source", "byte_size"), 268435457), (("source", "extra"), True), (("extra",), True)]:
            with self.subTest(path=path):
                self.assertFalse(self.provider_configuration_validator.is_valid(changed(provider_configuration(), path, value)))
        document = provider_configuration()
        del document["source"]
        self.assertFalse(self.provider_configuration_validator.is_valid(document))

    def test_report_rejects_unsupported_profile_and_malformed_evidence(self):
        cases = [
            (("schema",), "aniflow.audio-technical-inspection/v2"),
            (("source", "id"), "source_audio\n"),
            (("source", "byte_size"), 43),
            (("source", "byte_size"), 268435457),
            (("source", "sha256"), "invalid"),
            (("implementation_sha256",), "a" * 64 + "\n"),
            (("provider", "version"), "2.0.0"),
            (("container",), "mp4"),
            (("codec",), "aac"),
            (("sample_format",), "fltp"),
            (("stream_index",), 1),
            (("channels",), 3),
            (("sample_rate_hz",), 7999),
            (("sample_rate_hz",), 192001),
            (("frame_count",), 0),
            (("frame_count",), 115200001),
            (("duration", "denominator"), 0),
            (("duration", "numerator"), -1),
            (("pcm_bitrate_bits_per_second",), 0),
            (("decode_complete",), False),
            (("source_unchanged",), False),
            (("tools", 0, "id"), "ffprobe"),
            (("tools", 0, "version"), "nightly"),
            (("commands", 0, "tool"), "ffmpeg"),
            (("commands", 0, "arguments"), []),
        ]
        for path, value in cases:
            with self.subTest(path=path, value=value):
                self.assertFalse(self.technical_validator.is_valid(changed(technical_report(), path, value)))
        for path in [(), ("source",), ("provider",), ("tools", 0), ("duration",), ("commands", 0)]:
            with self.subTest(unknown_field=path):
                self.assertFalse(self.technical_validator.is_valid(changed(technical_report(), (*path, "undeclared"), True)))

    def test_preflight_ready_and_unavailable_evidence(self):
        ready = {"schema": "aniflow.audio-inspection-preflight/v1", "ready": True, "tools": technical_report()["tools"], "diagnostics": []}
        unavailable = {"schema": "aniflow.audio-inspection-preflight/v1", "ready": False, "tools": [], "diagnostics": [{"code": "missing_tool", "tool": "ffmpeg", "message": "Synthetic missing-tool diagnostic."}]}
        self.preflight_validator.validate(ready)
        self.preflight_validator.validate(unavailable)
        for document, path, value in [
            (ready, ("tools",), []),
            (ready, ("diagnostics",), unavailable["diagnostics"]),
            (ready, ("tools", 0, "id"), "ffprobe"),
            (ready, ("tools", 0, "sha256"), "invalid"),
            (ready, ("tools", 0, "unknown"), True),
            (unavailable, ("diagnostics",), []),
            (unavailable, ("diagnostics", 0, "code"), "unknown_failure"),
            (unavailable, ("diagnostics", 0, "unknown"), True),
            (unavailable, ("unknown",), True),
        ]:
            with self.subTest(path=path, value=value):
                self.assertFalse(self.preflight_validator.is_valid(changed(document, path, value)))

    def test_schema_success_is_not_relational_acceptance(self):
        # These cannot be checked by structural JSON Schema. The public Rust
        # validator and provider compare actual bytes, duration and decode hash.
        for path, value in [(("decoded_pcm_sha256",), synthetic_digest("mismatch")), (("duration", "numerator"), 2), (("pcm_bitrate_bits_per_second",), 128000)]:
            with self.subTest(path=path):
                self.technical_validator.validate(changed(technical_report(), path, value))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--technical-report", type=Path, help="Validate an actual locally generated technical report.")
    parser.add_argument("--analysis-report", type=Path, help="Validate its normalized audio-analysis companion.")
    parser.add_argument("--preflight-report", type=Path, help="Validate a locally generated dependency preflight report.")
    arguments = parser.parse_args()
    for path, schema_name in [(arguments.technical_report, "audio-technical-inspection-v1.schema.json"), (arguments.analysis_report, "audio-analysis-v1.schema.json"), (arguments.preflight_report, "audio-inspection-preflight-v1.schema.json")]:
        if path is not None:
            Draft202012Validator(load(CONTRACTS / schema_name)).validate(load(path))
            print(f"Validated generated report: {path}")
    unittest.main(argv=[__file__])
