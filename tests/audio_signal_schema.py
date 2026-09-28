#!/usr/bin/env python3
"""Independent schemas and reproducible synthetic shape example for audio signals.

The published example is one second of synthetic mono silence. Source bytes are
reproducible; provider/tool/upstream hashes deliberately identify fixture marker
strings, not executed analyzers. Use --signal-report for an actual local report.
An already installed jsonschema package is required; nothing is downloaded.
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
    raise SystemExit("Audio signal schema checks require an already installed Python jsonschema package; no installation was attempted.") from error

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "providers" / "audio-signal"
CONTRACTS = ROOT / "docs" / "contracts"
EXAMPLE = CONTRACTS / "examples" / "audio-signal-measurements-v1.example.json"


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def digest(label):
    return hashlib.sha256(f"Synthetic signal contract fixture: {label}".encode()).hexdigest()


def artifact(identifier, label):
    payload = f"Synthetic signal contract fixture: {label}".encode()
    return {"id": identifier, "sha256": hashlib.sha256(payload).hexdigest(), "byte_size": len(payload)}


def settings():
    return {"schema": "aniflow.audio-signal.configuration/v1", "silence_threshold_pcm": 32, "minimum_silence_milliseconds": 100, "clipping_threshold_pcm": 32767}


def source_bytes():
    output = io.BytesIO()
    with wave.open(output, "wb") as source:
        source.setnchannels(1)
        source.setsampwidth(2)
        source.setframerate(48000)
        source.writeframes(bytes(48000 * 2))
    return output.getvalue()


def source_identity():
    content = source_bytes()
    return {"id": "source_audio", "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}


def wrapper():
    tools = {"schema": "aniflow.audio-inspection.configuration/v1", "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 1048576}
    for tool in ["ffmpeg", "ffprobe"]:
        tools[tool] = {"executable": f"/synthetic/tools/{tool}", "version": "0.0.0-synthetic", "sha256": digest(tool)}
    return {"schema": "aniflow.audio-signal.provider-configuration/v1", "settings": settings(), "tools": tools, "source": source_identity()}


def metric(unit, value=None, reason="silent_input"):
    payload = {"kind": "unavailable", "reason": reason} if value is None else {"kind": "measured", "value": value}
    return {"unit": unit, "scope": {"channels": [0], "stem_id": None}, "value": payload}


def commands():
    prefix = ["-hide_banner", "-nostdin", "-nostats", "-loglevel", "info", "-xerror", "-err_detect", "explode", "-threads", "1", "-protocol_whitelist", "file,pipe", "-i", "{snapshot}", "-map", "0:a:0", "-vn", "-sn", "-dn", "-af"]
    filters = ["ebur128=metadata=1:peak=true:framelog=verbose,ametadata=print:key=lavfi.r128.S:file=-", "apad=pad_len=4800,ebur128=peak=true:framelog=verbose"]
    return [{"tool": "ffmpeg", "arguments": prefix + [filter_value, "-f", "null", "-"]} for filter_value in filters]


def example():
    return {
        "schema": "aniflow.audio-signal-measurements/v1",
        "source": {"artifact": source_identity(), "stream_index": 0, "sample_rate_hz": 48000, "channels": 1, "frame_count": 48000, "origin": {"numerator": 0, "denominator": 1}, "stem": None},
        "technical_artifact": artifact("technical", "upstream technical shape only; no analyzer executed"),
        "inspection_analysis_artifact": artifact("analysis", "upstream normalized shape only; no analyzer executed"),
        "provider": {"id": "org.egohygiene.aniflow.audio-signal", "version": "1.0.0"},
        "implementation_sha256": digest("implementation placeholder"),
        "configuration_sha256": digest("configuration placeholder"),
        "provider_lock_sha256": digest("lock placeholder"),
        "tools": [{"id": tool, "version": "0.0.0-synthetic", "sha256": digest(tool)} for tool in ["ffmpeg", "ffprobe"]],
        "settings": settings(),
        "method": {"short_term_window_frames": 144000, "short_term_hop_frames": 4800, "integrated_minimum_frames": 19200, "loudness_range_minimum_frames": 2880000, "true_peak_padding_frames": 4800, "loudness_range_minimum_gated_windows": 10, "true_peak_max_sample_rate_hz": 48000, "true_peak_target_sample_rate_hz": 192000, "tool_summary_decimal_places": 1, "short_term_decimal_places": 3, "short_term_floor_lufs": -70, "true_peak_sample_peak_tolerance_millidecibels": 200},
        "commands": commands(),
        "channels": [{"channel": 0, "sample_peak_ratio": 0.0, "rms_ratio": 0.0, "sample_peak": metric("decibels_full_scale"), "rms": metric("decibels_full_scale"), "crest_factor": metric("decibels")}],
        "integrated_loudness": metric("loudness_units_full_scale"),
        "loudness_range": metric("loudness_units"),
        "lra_threshold_lufs": 0.0,
        "lra_qualifying_windows": 0,
        "true_peak": metric("decibels_true_peak"),
        "short_term_status": {"kind": "unavailable", "reason": "insufficient_duration"},
        "short_term": [],
        "silence_regions": [{"channel": 0, "range": {"start": 0, "end": 48000}}],
        "clipping_regions": [],
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


class AudioSignalSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.configuration_schema = load(BUNDLE / "configuration.schema.json")
        cls.provider_schema = load(BUNDLE / "provider-configuration.schema.json")
        cls.report_schema = load(CONTRACTS / "audio-signal-measurements-v1.schema.json")
        for schema in [cls.configuration_schema, cls.provider_schema, cls.report_schema]:
            Draft202012Validator.check_schema(schema)
        cls.configuration_validator = Draft202012Validator(cls.configuration_schema)
        cls.provider_validator = Draft202012Validator(cls.provider_schema)
        cls.report_validator = Draft202012Validator(cls.report_schema)

    def test_reproducible_example_and_source_identity(self):
        self.assertEqual(EXAMPLE.read_text(encoding="utf-8"), encoded(example()))
        self.report_validator.validate(load(EXAMPLE))
        self.assertEqual(example()["source"]["artifact"]["byte_size"], 44 + 48000 * 2)
        self.assertEqual(example()["source"]["artifact"]["sha256"], hashlib.sha256(source_bytes()).hexdigest())

    def test_authored_defaults_and_normalized_wrapper(self):
        self.configuration_validator.validate({"schema": "aniflow.audio-signal.configuration/v1"})
        self.configuration_validator.validate(settings())
        self.provider_validator.validate(wrapper())
        missing_effective_setting = wrapper()
        del missing_effective_setting["settings"]["clipping_threshold_pcm"]
        self.assertFalse(self.provider_validator.is_valid(missing_effective_setting))

    def test_provider_schema_and_manifest_identities(self):
        manifest = load(BUNDLE / "manifest.json")
        Draft202012Validator(load(CONTRACTS / "provider-manifest-v1.schema.json")).validate(manifest)
        reference = {"id": "aniflow.audio-signal.provider-configuration/v1", "version": "1.0.0", "sha256": hashlib.sha256((BUNDLE / "provider-configuration.schema.json").read_bytes()).hexdigest()}
        self.assertEqual(manifest["configuration_schemas"], [reference])
        self.assertEqual(manifest["capabilities"][0]["configuration_schema"], reference)
        self.assertEqual(manifest["capabilities"][0]["behavior"]["determinism"], "environment_dependent")
        self.assertFalse(manifest["capabilities"][0]["behavior"]["cacheable"])
        inspection = load(ROOT / "providers" / "audio-inspection" / "configuration.schema.json")
        for name, definition in inspection["$defs"].items():
            self.assertEqual(self.provider_schema["$defs"][name], definition)

    def test_configuration_rejects_unknown_fields_and_out_of_bounds_settings(self):
        for path, value in [(("unknown",), True), (("schema",), "aniflow.audio-signal.configuration/v2"), (("silence_threshold_pcm",), -1), (("silence_threshold_pcm",), 32769), (("minimum_silence_milliseconds",), 0), (("minimum_silence_milliseconds",), 600001), (("clipping_threshold_pcm",), 0), (("clipping_threshold_pcm",), 32769), (("clipping_threshold_pcm",), "32767")]:
            with self.subTest(path=path, value=value):
                malformed = changed(settings(), path, value)
                self.assertFalse(self.configuration_validator.is_valid(malformed))
                self.assertFalse(self.provider_validator.is_valid({**wrapper(), "settings": malformed}))
        for path, value in [(("source", "id"), "another_source"), (("source", "sha256"), "invalid"), (("tools", "ffmpeg", "executable"), "ffmpeg"), (("tools", "ffmpeg", "version"), "nightly"), (("unknown",), True)]:
            with self.subTest(wrapper_path=path):
                self.assertFalse(self.provider_validator.is_valid(changed(wrapper(), path, value)))

    def test_unknown_nested_report_fields_are_rejected(self):
        paths = [(), ("source",), ("source", "artifact"), ("source", "origin"), ("technical_artifact",), ("provider",), ("tools", 0), ("settings",), ("method",), ("commands", 0), ("channels", 0), ("channels", 0, "sample_peak"), ("channels", 0, "sample_peak", "scope"), ("channels", 0, "sample_peak", "value"), ("integrated_loudness",), ("short_term_status",), ("silence_regions", 0), ("silence_regions", 0, "range")]
        for path in paths:
            with self.subTest(path=path):
                self.assertFalse(self.report_validator.is_valid(changed(example(), (*path, "undeclared"), True)))

    def test_report_rejects_invalid_units_values_bounds_and_identities(self):
        cases = [
            (("schema",), "aniflow.audio-signal-measurements/v2"),
            (("source", "sample_rate_hz"), 48001),
            (("source", "channels"), 3),
            (("source", "origin", "numerator"), 1),
            (("source", "stem"), {"id": "vocals"}),
            (("source", "artifact", "sha256"), "a" * 64 + "\n"),
            (("technical_artifact", "id"), "signal"),
            (("inspection_analysis_artifact", "byte_size"), 0),
            (("tools", 0, "id"), "ffprobe"),
            (("method", "loudness_range_minimum_gated_windows"), 1),
            (("method", "true_peak_max_sample_rate_hz"), 192000),
            (("method", "short_term_floor_lufs"), -120),
            (("channels", 0, "sample_peak_ratio"), 1.01),
            (("channels", 0, "rms_ratio"), -0.1),
            (("channels", 0, "sample_peak", "unit"), "decibels_true_peak"),
            (("channels", 0, "sample_peak", "value"), {"kind": "measured", "value": 1.0}),
            (("channels", 0, "sample_peak", "value"), {"kind": "measured", "value": "-6.0"}),
            (("channels", 0, "sample_peak", "value"), {"kind": "unavailable", "reason": "unknown"}),
            (("integrated_loudness", "value"), {"kind": "measured", "value": -70.0}),
            (("loudness_range", "value"), {"kind": "measured", "value": -1.0}),
            (("true_peak", "unit"), "decibels_full_scale"),
            (("short_term_status",), {"kind": "measured", "reason": "silent_input"}),
            (("silence_regions", 0, "range", "start"), -1),
            (("silence_regions", 0, "range", "end"), 0),
        ]
        for path, value in cases:
            with self.subTest(path=path, value=value):
                self.assertFalse(self.report_validator.is_valid(changed(example(), path, value)))

    def test_finite_measurement_and_structured_unavailability_variants(self):
        measured = example()
        measured["channels"][0]["sample_peak"] = metric("decibels_full_scale", -6.020599913279624)
        measured["integrated_loudness"] = metric("loudness_units_full_scale", -9.0)
        measured["loudness_range"] = metric("loudness_units", 0.0)
        measured["true_peak"] = metric("decibels_true_peak", -6.0)
        self.report_validator.validate(measured)
        for reason in ["silent_input", "unsupported_true_peak_rate"]:
            with self.subTest(reason=reason):
                self.report_validator.validate(changed(example(), ("true_peak",), metric("decibels_true_peak", reason=reason)))

    def test_structural_success_is_not_relational_acceptance(self):
        # Public Rust validation rejects the inconsistent scope, timing, method,
        # linear/logarithmic identities and actual evidence that schemas cannot
        # compare. Structural validity never establishes pipeline completion.
        for path, value in [(("source", "artifact", "byte_size"), 44), (("silence_regions", 0, "range", "end"), 96000), (("method", "short_term_window_frames"), 96000), (("channels", 0, "sample_peak_ratio"), 0.5)]:
            with self.subTest(path=path):
                self.report_validator.validate(changed(example(), path, value))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--signal-report", type=Path, help="Validate an actual locally generated signal report.")
    parser.add_argument("--write-example", action="store_true", help="Regenerate the published synthetic shape example deterministically.")
    arguments = parser.parse_args()
    if arguments.write_example:
        EXAMPLE.write_text(encoded(example()), encoding="utf-8")
    if arguments.signal_report is not None:
        Draft202012Validator(load(CONTRACTS / "audio-signal-measurements-v1.schema.json")).validate(load(arguments.signal_report))
        print(f"Validated generated signal report: {arguments.signal_report}")
    unittest.main(argv=[__file__])
