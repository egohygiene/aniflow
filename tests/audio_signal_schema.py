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
import math
import struct
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
EXAMPLE_V2 = CONTRACTS / "examples" / "audio-signal-measurements-v2.example.json"
NATIVE_EXAMPLES = {profile: CONTRACTS / "examples" / f"audio-signal-measurements-v3-{profile}.example.json" for profile in ["pcm24", "float32"]}


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def digest(label):
    return hashlib.sha256(f"Synthetic signal contract fixture: {label}".encode()).hexdigest()


def artifact(identifier, label):
    payload = f"Synthetic signal contract fixture: {label}".encode()
    return {"id": identifier, "sha256": hashlib.sha256(payload).hexdigest(), "byte_size": len(payload)}


def settings():
    return {"schema": "aniflow.audio-signal.configuration/v1", "silence_threshold_pcm": 32, "minimum_silence_milliseconds": 100, "clipping_threshold_pcm": 32767}


def source_bytes(rate=48000):
    output = io.BytesIO()
    with wave.open(output, "wb") as source:
        source.setnchannels(1)
        source.setsampwidth(2)
        source.setframerate(rate)
        source.writeframes(bytes(rate * 2))
    return output.getvalue()


def source_identity(rate=48000):
    content = source_bytes(rate)
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



def example_v2(rate=192000):
    document = copy.deepcopy(example())
    document["schema"] = "aniflow.audio-signal-measurements/v2"
    document["provider"]["version"] = "2.0.0"
    document["source"]["artifact"] = source_identity(rate)
    document["source"]["sample_rate_hz"] = rate
    document["source"]["frame_count"] = rate
    document["silence_regions"][0]["range"]["end"] = rate
    method = document["method"]
    method.update({"short_term_window_frames": 3 * rate, "short_term_hop_frames": rate // 10, "integrated_minimum_frames": rate * 2 // 5, "loudness_range_minimum_frames": 60 * rate, "true_peak_max_sample_rate_hz": 192000})
    if rate <= 48000:
        method.update({"true_peak_algorithm": {"kind": "legacy_ebur128"}, "true_peak_target_sample_rate_hz": 192000, "true_peak_padding_frames": rate // 10})
        document["commands"][1]["arguments"][20] = f"apad=pad_len={rate // 10},ebur128=peak=true:framelog=verbose"
    elif rate in [88200, 96000, 176400, 192000]:
        method.update({"true_peak_algorithm": {"kind": "swr_4x_astats", "oversampling_factor": 4, "resampler": "swr", "output_sample_format": "dblp", "internal_sample_format": "dblp", "filter_size": 64, "phase_shift": 10, "linear_interp": False, "exact_rational": True, "cutoff": 1, "filter_type": "kaiser", "kaiser_beta": 9, "dither_method": 0, "async_compensation": 0, "peak_decimal_places": 6}, "true_peak_target_sample_rate_hz": 4 * rate, "true_peak_padding_frames": rate // 10})
        document["commands"][1]["arguments"][20] = f"apad=pad_len={rate // 10},aresample={4 * rate}:resampler=swr:osf=dblp:tsf=dblp:filter_size=64:phase_shift=10:linear_interp=0:exact_rational=1:cutoff=1:filter_type=kaiser:kaiser_beta=9:dither_method=0:async=0,astats=metadata=0:reset=0:measure_perchannel=none:measure_overall=Peak_level+Number_of_samples+Number_of_NaNs+Number_of_Infs"
        document["commands"][1]["arguments"][10:10] = ["-filter_threads", "1"]
    else:
        method.update({"true_peak_algorithm": {"kind": "unsupported_rate"}, "true_peak_target_sample_rate_hz": 0, "true_peak_padding_frames": 0})
        document["commands"] = document["commands"][:1]
        document["true_peak"]["value"] = {"kind": "unavailable", "reason": "unsupported_true_peak_rate"}
    return document


def encoded(document):
    return json.dumps(document, indent=2, allow_nan=False) + "\n"


def native_settings():
    return {"schema": "aniflow.audio-signal.configuration/v2", "silence_threshold_ratio": 32 / 32768, "minimum_silence_milliseconds": 100, "clipping_threshold_ratio": 1.0}


def native_source_bytes(profile):
    # Hand-authored uncompressed RIFF/WAV fixture bytes, not tool output.
    tag, bits, sample = {"pcm16": (1, 16, bytes(2)), "pcm24": (1, 24, bytes(3)), "float32": (3, 32, struct.pack("<f", 1.5))}[profile]
    payload = sample * 48000
    fmt = struct.pack("<HHIIHH", tag, 1, 48000, 48000 * (bits // 8), bits // 8, bits)
    body = b"WAVEfmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(payload)) + payload
    return b"RIFF" + struct.pack("<I", len(body)) + body


def native_example(profile="pcm24"):
    """Sample-statistics shape fixtures; provider/tools were never executed."""
    document = example()
    for field in ["lra_threshold_lufs", "lra_qualifying_windows", "short_term"]:
        del document[field]
    source = native_source_bytes(profile)
    document["schema"] = "aniflow.audio-signal-measurements/v3"
    document["source"]["artifact"] = {"id": "source_audio", "sha256": hashlib.sha256(source).hexdigest(), "byte_size": len(source)}
    document["provider"]["version"] = "3.0.0"
    document["settings"] = native_settings()
    document["source_format"] = profile
    document["method"] = {"algorithm": "aniflow.native-sample-statistics/v1", "full_scale_reference": {"pcm16": 32768.0, "pcm24": 8388608.0, "float32": 1.0}[profile], "accumulation": "compensated_f64_sum_of_squares"}
    document["commands"] = []
    for field, unit in [("integrated_loudness", "loudness_units_full_scale"), ("loudness_range", "loudness_units"), ("true_peak", "decibels_true_peak")]:
        document[field] = metric(unit, reason="unsupported_native_signal_profile")
    document["short_term_status"] = {"kind": "unavailable", "reason": "unsupported_native_signal_profile"}
    if profile == "float32":
        document["channels"][0].update({"sample_peak_ratio": 1.5, "rms_ratio": 1.5, "sample_peak": metric("decibels_full_scale", 20 * math.log10(1.5)), "rms": metric("decibels_full_scale", 20 * math.log10(1.5)), "crest_factor": metric("decibels", 0.0)})
        document["silence_regions"] = []
        document["clipping_regions"] = [{"channel": 0, "range": {"start": 0, "end": 48000}}]
    return document


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
        cls.native_provider_schema = load(BUNDLE / "provider-configuration-native.schema.json")
        cls.report_schema = load(CONTRACTS / "audio-signal-measurements-v1.schema.json")
        cls.report_schema_v2 = load(CONTRACTS / "audio-signal-measurements-v2.schema.json")
        cls.report_schema_v3 = load(CONTRACTS / "audio-signal-measurements-v3.schema.json")
        for schema in [cls.configuration_schema, cls.provider_schema, cls.native_provider_schema, cls.report_schema, cls.report_schema_v2, cls.report_schema_v3]:
            Draft202012Validator.check_schema(schema)
        cls.configuration_validator = Draft202012Validator(cls.configuration_schema)
        cls.provider_validator = Draft202012Validator(cls.provider_schema)
        cls.native_provider_validator = Draft202012Validator(cls.native_provider_schema)
        cls.report_validator = Draft202012Validator(cls.report_schema)
        cls.report_validator_v2 = Draft202012Validator(cls.report_schema_v2)
        cls.report_validator_v3 = Draft202012Validator(cls.report_schema_v3)

    def test_native_examples_preserve_float_over_full_scale_and_integer_limits(self):
        for profile, path in NATIVE_EXAMPLES.items():
            with self.subTest(profile=profile):
                document = native_example(profile)
                self.assertEqual(load(path), document)
                self.report_validator_v3.validate(document)
                self.assertFalse(self.report_validator.is_valid(document))
                self.assertFalse(self.report_validator_v2.is_valid(document))
        self.report_validator_v3.validate(native_example("pcm16"))
        self.report_validator_v3.validate(changed(native_example("float32"), ("source", "sample_rate_hz"), 48001))
        subnormal_shape = native_example("float32")
        tiny = math.ldexp(1.0, -149)
        subnormal_shape["channels"][0].update({"sample_peak_ratio": tiny, "rms_ratio": tiny, "sample_peak": metric("decibels_full_scale", 20 * math.log10(tiny)), "rms": metric("decibels_full_scale", 20 * math.log10(tiny))})
        subnormal_shape["clipping_regions"] = []
        # Structural scalar range coverage only; this modified report does not
        # claim its source identity was measured or relationally accepted.
        self.report_validator_v3.validate(subnormal_shape)
        for profile in ["pcm16", "pcm24"]:
            self.assertFalse(self.report_validator_v3.is_valid(changed(native_example(profile), ("channels", 0, "sample_peak_ratio"), 1.5)))
            self.assertFalse(self.report_validator_v3.is_valid(changed(native_example(profile), ("channels", 0, "sample_peak", "value"), {"kind": "measured", "value": 3.5})))
        for path, value in [(("channels", 0, "sample_peak_ratio"), 3.5e38), (("channels", 0, "rms_ratio"), -0.1), (("channels", 0, "sample_peak", "value"), {"kind": "measured", "value": 1201}), (("channels", 0, "crest_factor", "value"), {"kind": "measured", "value": -0.1})]:
            self.assertFalse(self.report_validator_v3.is_valid(changed(native_example("float32"), path, value)))

    def test_native_profile_refuses_fake_loudness_legacy_thresholds_and_wrong_scale(self):
        document = native_example("float32")
        for path, value in [(("source_format",), "float64"), (("provider", "version"), "2.0.0"), (("settings",), settings()), (("settings", "silence_threshold_ratio"), 1.01), (("settings", "clipping_threshold_ratio"), 0), (("method", "full_scale_reference"), 32768), (("method", "accumulation"), "integer_sum_of_squares"), (("commands",), commands()), (("short_term_status",), {"kind": "measured"}), (("integrated_loudness", "value"), {"kind": "measured", "value": -12}), (("true_peak", "value"), {"kind": "unavailable", "reason": "silent_input"}), (("short_term",), []), (("lra_threshold_lufs",), 0)]:
            with self.subTest(path=path, value=value):
                self.assertFalse(self.report_validator_v3.is_valid(changed(document, path, value)))
        for path in [(), ("source",), ("settings",), ("method",), ("channels", 0), ("true_peak", "value")]:
            self.assertFalse(self.report_validator_v3.is_valid(changed(document, (*path, "undeclared"), True)))

    def test_native_provider_has_a_separate_source_bound_contract(self):
        document = wrapper()
        document["schema"] = "aniflow.audio-signal.provider-configuration/v2"
        document["settings"] = native_settings()
        self.native_provider_validator.validate(document)
        self.assertFalse(self.provider_validator.is_valid(document))
        self.assertFalse(self.native_provider_validator.is_valid(wrapper()))
        for path, value in [(("settings",), settings()), (("settings", "silence_threshold_ratio"), 1.1), (("settings", "clipping_threshold_ratio"), 0), (("source", "sha256"), "invalid"), (("source", "id"), "unrelated_audio"), (("unknown",), True)]:
            self.assertFalse(self.native_provider_validator.is_valid(changed(document, path, value)))
        manifest = load(BUNDLE / "manifest-native.json")
        Draft202012Validator(load(CONTRACTS / "provider-manifest-v1.schema.json")).validate(manifest)
        reference = {"id": "aniflow.audio-signal.provider-configuration/v2", "version": "2.0.0", "sha256": hashlib.sha256((BUNDLE / "provider-configuration-native.schema.json").read_bytes()).hexdigest()}
        self.assertEqual(manifest["configuration_schemas"], [reference])
        self.assertEqual(manifest["capabilities"][0]["configuration_schema"], reference)
        self.assertEqual(manifest["provider"]["version"], "3.0.0")

    def test_reproducible_example_and_source_identity(self):
        self.assertEqual(EXAMPLE.read_text(encoding="utf-8"), encoded(example()))
        self.report_validator.validate(load(EXAMPLE))
        self.assertEqual(example()["source"]["artifact"]["byte_size"], 44 + 48000 * 2)
        self.assertEqual(example()["source"]["artifact"]["sha256"], hashlib.sha256(source_bytes()).hexdigest())

    def test_frozen_v1_documents_and_reproducible_v2_example(self):
        self.assertEqual(hashlib.sha256((CONTRACTS / "audio-signal-measurements-v1.schema.json").read_bytes()).hexdigest(), "47e37ce33fbd9f263dd6619c1743b7ba33cbd662a01e00f9a6e314c1bfdccde5")
        self.assertEqual(hashlib.sha256(EXAMPLE.read_bytes()).hexdigest(), "74124ec5888136a27e1fd8d3ac2cfa532b8d2c810346de6d7808eff29cc47019")
        self.assertEqual(EXAMPLE_V2.read_text(encoding="utf-8"), encoded(example_v2()))
        self.report_validator_v2.validate(load(EXAMPLE_V2))

    def test_v2_exact_rate_selection(self):
        for rate in [8000, 44100, 48000, 88200, 96000, 176400, 192000, 48010, 64000, 88210, 96010, 176410, 191990]:
            with self.subTest(rate=rate):
                self.report_validator_v2.validate(example_v2(rate))
        for rate in [48010, 64000, 88210, 96010, 176410, 191990]:
            with self.subTest(unqualified_rate=rate):
                document = example_v2(rate)
                document["method"]["true_peak_algorithm"] = example_v2()["method"]["true_peak_algorithm"]
                document["method"]["true_peak_target_sample_rate_hz"] = 4 * rate
                document["method"]["true_peak_padding_frames"] = rate // 10
                document["true_peak"]["value"] = {"kind": "measured", "value": -6.0}
                self.assertFalse(self.report_validator_v2.is_valid(document))

    def test_v2_rejects_mixed_versions_unknown_algorithms_and_unpinned_filters(self):
        cases = [
            (("provider", "version"), "1.0.0"),
            (("schema",), "aniflow.audio-signal-measurements/v1"),
            (("method", "true_peak_algorithm"), None),
            (("method", "true_peak_algorithm", "kind"), "future_algorithm"),
            (("method", "true_peak_algorithm", "unknown"), True),
            (("method", "true_peak_algorithm", "oversampling_factor"), 2),
            (("method", "true_peak_algorithm", "filter_size"), 32),
            (("method", "true_peak_algorithm", "phase_shift"), 9),
            (("method", "true_peak_algorithm", "linear_interp"), True),
            (("method", "true_peak_algorithm", "output_sample_format"), "s16"),
            (("method", "true_peak_algorithm", "filter_type"), "cubic"),
            (("method", "true_peak_algorithm", "peak_decimal_places"), 1),
            (("method", "true_peak_target_sample_rate_hz"), 192000),
            (("method", "true_peak_padding_frames"), 0),
        ]
        for path, value in cases:
            with self.subTest(path=path, value=value):
                self.assertFalse(self.report_validator_v2.is_valid(changed(example_v2(), path, value)))
        missing = example_v2()
        del missing["method"]["true_peak_algorithm"]
        self.assertFalse(self.report_validator_v2.is_valid(missing))
        for algorithm in [None, {"kind": "legacy_ebur128"}, example_v2()["method"]["true_peak_algorithm"]]:
            with self.subTest(v1_extension=algorithm):
                self.assertFalse(self.report_validator.is_valid(changed(example(), ("method", "true_peak_algorithm"), algorithm)))
        self.assertFalse(self.report_validator_v2.is_valid(changed(example_v2(48000), ("method", "true_peak_algorithm"), {"kind": "legacy_ebur128", "extra": True})))

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
    parser.add_argument("--write-example", action="store_true", help="Regenerate the v2 synthetic shape example; frozen v1 files remain unchanged.")
    arguments = parser.parse_args()
    if arguments.write_example:
        EXAMPLE_V2.write_text(encoded(example_v2()), encoding="utf-8")
    if arguments.signal_report is not None:
        report = load(arguments.signal_report)
        versions = {"aniflow.audio-signal-measurements/v1": "audio-signal-measurements-v1.schema.json", "aniflow.audio-signal-measurements/v2": "audio-signal-measurements-v2.schema.json", "aniflow.audio-signal-measurements/v3": "audio-signal-measurements-v3.schema.json"}
        if report.get("schema") not in versions:
            raise SystemExit("Unsupported signal report schema.")
        Draft202012Validator(load(CONTRACTS / versions[report["schema"]])).validate(report)
        print(f"Validated generated signal report: {arguments.signal_report}")
    unittest.main(argv=[__file__])
