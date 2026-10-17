#!/usr/bin/env python3
"""Independent closed RIFE wire-shape checks using synthetic observations.

No model, native executable or private media is used. Rust additionally owns
temporal recomputation, canonical identities, resource derivation, sorted cut
boundaries and exact ordinal/midpoint mapping. Schema checks do not establish
those relationships or qualify native execution. Authored; unrun under #64.
"""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import unittest

try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit("RIFE schema checks require an already installed jsonschema package.") from error

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "providers/rife"
CONTRACTS = ROOT / "docs/contracts"
SCHEMAS = {
    "configuration": BUNDLE / "configuration.schema.json",
    "provider_configuration": BUNDLE / "provider-configuration.schema.json",
    "frame_index": CONTRACTS / "rife-frame-index-v1.schema.json",
    "plan": CONTRACTS / "rife-plan-v1.schema.json",
    "output_index": CONTRACTS / "rife-output-index-v1.schema.json",
}


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode()).hexdigest()


def marker_digest(name):
    return hashlib.sha256(("Synthetic RIFE shape fixture: " + name + "; no inference.\n").encode()).hexdigest()


def rational(numerator, denominator=1):
    return {"numerator": numerator, "denominator": denominator}


def configuration(enabled=False):
    value = load(BUNDLE / "configuration.example.json")
    value["enabled"] = enabled
    return value


def provider_configuration():
    return {"schema": "aniflow.rife-provider-configuration/v1", "configuration": configuration(True)}


def frame_index():
    support = {"policy": "aniflow.temporal-policy/cfr-zero-origin/v1", "supported": True, "diagnostics": [], "frame_count": 2, "video_frame_tolerance": rational(1, 24), "audio_boundary_tolerance": None}
    source_time = {"start": rational(0), "end": rational(1, 12)}
    temporal = {
        "schema": "aniflow.temporal-inspection/v1",
        "source_sha256": marker_digest("source"), "source_size_bytes": 128,
        "selection_intent": {"video_stream": 0, "audio_stream": None, "no_audio": True, "discard_streams": []},
        "streams": [{"index": 0, "kind": "video", "codec": "png", "time_base": rational(1, 24), "start_pts": 0, "duration_ticks": 2, "declared_frame_rate": rational(24), "average_frame_rate": rational(24), "width": 64, "height": 64, "pixel_format": "rgb24", "sample_rate_hz": None}],
        "selected": {"video": 0, "audio": None, "explicitly_discarded": [], "automatic_policy": "synthetic explicit selection"},
        "timelines": [{"stream_index": 0, "cadence": "constant", "frame_period": rational(1, 24), "source_time": source_time, "frames": [
            {"ordinal": ordinal, "stream_index": 0, "presentation_ticks": ordinal, "decode_ticks": ordinal, "duration_ticks": 1, "keyframe": True, "source_time": {"start": rational(ordinal, 24), "end": rational(ordinal + 1, 24)}}
            for ordinal in range(2)
        ], "packets": [{"ordinal": ordinal, "stream_index": 0, "presentation_ticks": ordinal, "decode_ticks": ordinal, "duration_ticks": 1} for ordinal in range(2)]}],
        "diagnostics": [], "processing": support,
    }
    return {"schema": "aniflow.rife-frame-index/v1", "temporal_inspection": temporal, "width": 64, "height": 64, "frame_profile": "rgb8_sdr_bt709_square_unrotated", "frames": [{"ordinal": ordinal, "filename": f"source_{ordinal:08}.png", "sha256": marker_digest(f"frame {ordinal}")} for ordinal in range(2)], "cut_before_ordinals": [1]}


def plan():
    index = frame_index()
    return {
        "schema": "aniflow.rife-plan/v1", "enabled": True,
        "configuration_sha256": canonical_digest(configuration(True)), "frame_index_sha256": canonical_digest(index), "source_sha256": index["temporal_inspection"]["source_sha256"],
        "source_frame_count": 2, "output_frame_count": 4, "source_frame_rate": rational(24), "output_frame_rate": rational(48), "duration": rational(1, 12), "cut_before_ordinals": [1],
        "resources": {"preset": "conservative", "maximum_active_pairs": 1, "threads": "1:1:1", "padded_width": 64, "padded_height": 64, "maximum_padded_pixels": 2100000, "estimated_working_memory_mib": 514, "working_memory_budget_mib": 2048, "memory_estimate_method": "uncalibrated:512MiB+512B_per_padded_pixel/v1", "memory_limit_enforced": False},
        "native_qualified": False, "audio_policy": "not_processed; preserve_selected_audio_at_assembly",
    }


def output_index():
    return {"schema": "aniflow.rife-output-index/v1", "plan": plan(), "frames": [
        {"ordinal": ordinal, "filename": f"frame_{ordinal:08}.png", "sha256": marker_digest(f"output {ordinal}"), "source_left_ordinal": ordinal // 2, "source_right_ordinal": ordinal // 2, "origin": ["original", "cut_hold", "original", "endpoint_hold"][ordinal]}
        for ordinal in range(4)
    ]}


def replace(value, path, replacement):
    target = value
    for part in path[:-1]:
        target = target[part]
    target[path[-1]] = replacement
    return value


class RifeSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = {}
        for name, path in SCHEMAS.items():
            document = load(path)
            Draft202012Validator.check_schema(document)
            cls.validators[name] = Draft202012Validator(document)

    def accepted(self, name, value):
        errors = list(self.validators[name].iter_errors(value))
        self.assertEqual([], errors, "\n".join(error.message for error in errors))

    def refused(self, name, value):
        self.assertTrue(list(self.validators[name].iter_errors(value)))

    def test_synthetic_shapes_are_closed_and_accepted(self):
        fixtures = {"configuration": configuration(), "provider_configuration": provider_configuration(), "frame_index": frame_index(), "plan": plan(), "output_index": output_index()}
        for name, value in fixtures.items():
            with self.subTest(schema=name):
                self.accepted(name, value)
                candidate = copy.deepcopy(value)
                candidate["unrecognized"] = True
                self.refused(name, candidate)

    def test_caller_defaults_and_disabled_host_shape(self):
        candidate = configuration()
        del candidate["enabled"]
        for field in ["preset", "device_id", "tool_timeout_milliseconds", "maximum_tool_output_bytes", "maximum_frame_bytes", "maximum_artifact_bytes"]:
            del candidate["settings"][field]
        candidate["host"] = {"operating_system": "", "architecture": "", "total_memory_mib": 0, "available_memory_mib": 0}
        self.accepted("configuration", candidate)
        candidate["enabled"] = True
        self.refused("configuration", candidate)
        self.refused("provider_configuration", replace(provider_configuration(), ["configuration", "enabled"], False))
        for field in ["preset", "device_id", "tool_timeout_milliseconds", "maximum_tool_output_bytes", "maximum_frame_bytes", "maximum_artifact_bytes"]:
            effective = provider_configuration()
            del effective["configuration"]["settings"][field]
            self.refused("provider_configuration", effective)

    def test_enabled_host_and_resource_settings_refuse_unsupported_values(self):
        mutations = [
            (["host", "operating_system"], "windows"), (["host", "architecture"], "unknown"),
            (["host", "total_memory_mib"], 4095), (["host", "available_memory_mib"], 0),
            (["settings", "device_id"], -2), (["settings", "device_id"], 32),
            (["settings", "tool_timeout_milliseconds"], 120001),
            (["settings", "maximum_tool_output_bytes"], 1048577),
            (["settings", "maximum_frame_bytes"], 67108865),
            (["settings", "maximum_artifact_bytes"], 1048575),
            (["settings", "model", "directory"], "/synthetic/rife-v4.7"),
            (["settings", "tool", "executable"], "/synthetic/../rife"),
            (["settings", "model", "param_sha256"], "bad"),
        ]
        for path, value in mutations:
            with self.subTest(path=path, value=value):
                self.refused("configuration", replace(configuration(True), path, value))
        for field in ["tta", "uhd", "threads", "scale", "target_frame_rate"]:
            candidate = configuration(True)
            candidate["settings"][field] = True
            self.refused("configuration", candidate)

    def test_nested_frame_temporal_and_filename_boundaries(self):
        for filename in ["../frame.png", "/frame.png", ".hidden.png", "frame.png\n", "frame.jpg", "a" * 125 + ".png"]:
            with self.subTest(filename=filename):
                self.refused("frame_index", replace(frame_index(), ["frames", 0, "filename"], filename))
        self.refused("frame_index", replace(frame_index(), ["frame_profile"], "rgba8"))
        self.refused("frame_index", replace(frame_index(), ["cut_before_ordinals"], [0]))
        self.refused("frame_index", replace(frame_index(), ["temporal_inspection", "timelines", 0, "frame_period", "denominator"], 0))
        candidate = frame_index()
        candidate["frames"][0]["alpha"] = True
        self.refused("frame_index", candidate)

    def test_evidence_does_not_claim_memory_enforcement_or_native_qualification(self):
        self.refused("plan", replace(plan(), ["native_qualified"], True))
        self.refused("plan", replace(plan(), ["resources", "memory_limit_enforced"], True))
        self.refused("plan", replace(plan(), ["resources", "maximum_active_pairs"], 2))
        self.refused("plan", replace(plan(), ["resources", "threads"], "2:2:2"))
        self.refused("output_index", replace(output_index(), ["frames", 1, "origin"], "unbounded"))
        self.refused("output_index", replace(output_index(), ["frames", 0, "filename"], "../output.png"))


if __name__ == "__main__":
    unittest.main()
