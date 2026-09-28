#!/usr/bin/env python3
"""Closed musical schemas and a reproducible synthetic shape example.

The example's PCM is generated locally; provider/runtime/other artifact identities
are synthetic marker bytes. It represents contract shape, never a measured run
or accuracy result. Actual reports can be checked with --musical-report PATH.
Requires an already installed jsonschema package; nothing is installed or run.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import unittest
import wave
try:
    from jsonschema import Draft202012Validator
except ImportError as error:
    raise SystemExit("Musical schema checks require an already installed jsonschema package.") from error

ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "docs/contracts"
BUNDLE = ROOT / "providers/audio-musical"
EXAMPLE = CONTRACTS / "examples/audio-musical-analysis-v1.example.json"
CONFIGURATION_EXAMPLE = BUNDLE / "configuration.example.json"
SCHEMAS = {name: CONTRACTS / f"audio-musical-{name}-v1.schema.json" for name in ["analysis", "observation", "probe"]}
SCHEMAS.update(settings=BUNDLE / "configuration.schema.json", provider_configuration=BUNDLE / "provider-configuration.schema.json")

def reject_nonfinite(value):
    raise ValueError(f"Non-JSON numeric token: {value}")

def load(path):
    return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject_nonfinite)

def marker(label):
    return f"Synthetic musical contract shape only: {label}; no analyzer was executed.\n".encode()

def digest(label):
    return hashlib.sha256(marker(label)).hexdigest()

def artifact(identifier):
    content = marker(identifier)
    return {"id": identifier, "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}

def settings():
    return {"schema": "aniflow.audio-musical.configuration/v1", "python": {"executable": "/synthetic/python3", "version": "3.12.14", "sha256": digest("python")}, "adapter": {"path": "/synthetic/audio-musical-adapter.py", "sha256": digest("adapter")}, "runtime": {"sha256": digest("runtime"), "file_count": 4, "byte_count": 1024}, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}

def observation():
    return {"schema": "aniflow.audio-musical-observation/v1", "sample_rate_hz": 44100, "channels": 2, "sample_frames": 352800, "duration_seconds": 8.0, "downmix": "arithmetic_average", "bpm": {"value": 120.0, "ticks_seconds": [index / 2 for index in range(1, 16)], "raw_confidence": 3.25, "estimates": [120.0, 60.0], "bpm_intervals": [0.5]}, "key_profiles": [{"profile": "krumhansl", "key": "C", "scale": "major", "raw_strength": 0.7}, {"profile": "temperley", "key": "A", "scale": "minor", "raw_strength": 0.6}]}

def example():
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(2)
        audio.setsampwidth(2)
        audio.setframerate(44100)
        # A small synthetic impulse at every half-second; no inference is run.
        audio.writeframes(b"".join(struct.pack("<hh", *([1000 if frame % 22050 < 8 else 0] * 2)) for frame in range(352800)))
    pcm = output.getvalue()
    configuration = settings()
    raw = observation()
    probe = {"schema": "aniflow.audio-musical-probe/v1", "python_version": "3.12.14", "essentia_version": "2.1b6.dev1389", "essentia_runtime_version": "2.1-beta6-dev", "essentia_git_sha": "v2.1_beta5-1389-g36ec3d92", "numpy_version": "2.3.5", "pyyaml_version": "6.0.3", "six_version": "1.17.0", "runtime_sha256": configuration["runtime"]["sha256"], "runtime_file_count": 4, "runtime_byte_count": 1024, "license": {"essentia_expression": "AGPL-3.0-only", "essentia_metadata_sha256": digest("essentia metadata"), "numpy_metadata_sha256": digest("numpy metadata")}}
    probe_command = {"tool": "python", "arguments": ["-I", "-B", "{adapter}", "--probe"]}
    return {
        "schema": "aniflow.audio-musical-analysis/v1",
        "source": {"artifact": {"id": "source_audio", "sha256": hashlib.sha256(pcm).hexdigest(), "byte_size": len(pcm)}, "stream_index": 0, "sample_rate_hz": 44100, "channels": 2, "frame_count": 352800, "origin": {"numerator": 0, "denominator": 1}, "stem": None},
        "scope": {"channels": [0, 1], "stem_id": None},
        "technical_artifact": artifact("technical"), "upstream_analysis_artifact": artifact("analysis"),
        "provider": {"id": "org.egohygiene.aniflow.audio-musical", "version": "1.0.0"},
        "implementation_sha256": digest("implementation"), "configuration_sha256": digest("effective configuration"), "provider_lock_sha256": digest("provider lock"),
        "probe": probe,
        "settings": {"python_version": configuration["python"]["version"], "python_sha256": configuration["python"]["sha256"], "adapter_sha256": configuration["adapter"]["sha256"], "runtime": configuration["runtime"], "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576},
        "commands": [probe_command, {"tool": "python", "arguments": ["-I", "-B", "{adapter}", "--input", "{snapshot}"]}, copy.deepcopy(probe_command)],
        "method": {"downmix": "arithmetic_average", "beat_quantization": "nearest_source_frame_ties_up", "sample_rate_hz": 44100, "minimum_frames": 352800, "rhythm_method": "rhythm_extractor2013_multifeature", "minimum_tempo_bpm": 40, "maximum_tempo_bpm": 208, "key_method": "key_extractor_default_parameters"},
        "provenance": "heuristic", "confidence": {"kind": "unavailable", "reason": "Native estimator scores are not calibrated probabilities."},
        "result": {"status": "estimated", "observation": raw, "beats": [{"id": f"beat_{index:06}", "seconds": seconds, "source_frame": round(seconds * 44100)} for index, seconds in enumerate(raw["bpm"]["ticks_seconds"])], "key_disagreement": True, "tempo_status": {"status": "estimated"}, "beats_status": {"status": "estimated"}, "key_status": {"status": "estimated"}, "tempo_candidates": [{"id": "tempo_primary", "method": "rhythm_extractor2013_multifeature", "value": 120.0}, {"id": "tempo_estimate_000000", "method": "rhythm_extractor2013_estimate", "value": 120.0}, {"id": "tempo_estimate_000001", "method": "rhythm_extractor2013_estimate", "value": 60.0}], "key_candidates": [{"id": "key_krumhansl", "profile": "krumhansl", "pitch_class": 0, "mode": "major", "raw_strength": 0.7}, {"id": "key_temperley", "profile": "temperley", "pitch_class": 9, "mode": "minor", "raw_strength": 0.6}]},
        "unsupported_families": ["downbeats", "meter", "tempo_changes", "chords", "onsets", "rhythm_density", "sections", "spectral", "timbre"],
    }

def validators():
    result = {}
    for name, path in SCHEMAS.items():
        document = load(path)
        Draft202012Validator.check_schema(document)
        result[name] = Draft202012Validator(document)
    return result

class MusicalSchemas(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = validators()
        cls.report = load(EXAMPLE)

    def test_published_examples_and_raw_payloads(self):
        self.assertEqual(self.report, example())
        self.assertEqual(load(CONFIGURATION_EXAMPLE), settings())
        self.validators["analysis"].validate(self.report)
        self.validators["probe"].validate(self.report["probe"])
        self.validators["observation"].validate(self.report["result"]["observation"])
        self.validators["settings"].validate(settings())
        tool = {"executable": "/synthetic/tool", "version": "6.1.1", "sha256": digest("tool")}
        self.validators["provider_configuration"].validate({"schema": "aniflow.audio-musical.provider-configuration/v1", "settings": settings(), "tools": {"schema": "aniflow.audio-inspection.configuration/v1", "ffmpeg": tool, "ffprobe": tool, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}, "source": self.report["source"]["artifact"], "upstream_analysis_artifact_id": "analysis"})

    def test_closed_nested_shapes(self):
        for path in [[], ["probe"], ["probe", "license"], ["settings", "runtime"], ["method"], ["result"], ["result", "observation", "bpm"], ["result", "observation", "key_profiles", 0], ["result", "beats", 0], ["result", "key_status"]]:
            changed = copy.deepcopy(self.report)
            target = changed
            for key in path: target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["analysis"].is_valid(changed), path)

    def test_units_confidence_profiles_and_bounds(self):
        cases = [( ["confidence"], {"kind": "calibrated", "score": 0.9}), (["method", "sample_rate_hz"], 48000), (["method", "minimum_tempo_bpm"], 0), (["result", "observation", "bpm", "value"], -1), (["result", "observation", "key_profiles", 0, "raw_strength"], 1.1), (["result", "key_candidates", 0, "pitch_class"], 12), (["result", "beats", 0, "source_frame"], -1), (["result", "observation", "key_profiles", 1, "profile"], "krumhansl"), (["probe", "essentia_git_sha"], "other-build"), (["unsupported_families"], ["chords"])]
        for path, replacement in cases:
            changed = copy.deepcopy(self.report)
            target = changed
            for key in path[:-1]: target = target[key]
            target[path[-1]] = replacement
            self.assertFalse(self.validators["analysis"].is_valid(changed), path)

    def test_partial_and_unavailable_shapes(self):
        changed = copy.deepcopy(self.report)
        changed["result"]["tempo_candidates"] = []
        changed["result"]["beats"] = []
        for family in ["tempo_status", "beats_status"]:
            changed["result"][family] = {"status": "unavailable", "reason": "no_estimate"}
        changed["result"]["observation"]["bpm"].update(value=0, estimates=[], ticks_seconds=[])
        self.validators["analysis"].validate(changed)
        for reason in ["silent_downmix", "insufficient_duration", "unsupported_sample_rate"]:
            changed["result"] = {"status": "unavailable", "reason": reason}
            changed["commands"] = [changed["commands"][0], changed["commands"][-1]]
            self.validators["analysis"].validate(changed)

    def test_stem_shape_and_structural_identity(self):
        changed = copy.deepcopy(self.report)
        changed["source"]["stem"] = {"id": "voice", "original_mix": artifact("original_mix"), "relationship_evidence_id": "stem_lineage"}
        changed["scope"]["stem_id"] = "voice"
        changed["upstream_analysis_artifact"]["id"] = "stem_analysis"
        self.validators["analysis"].validate(changed)
        for path in [["source", "artifact", "sha256"], ["source", "artifact", "id"], ["settings", "python_version"]]:
            bad = copy.deepcopy(changed)
            target = bad
            for key in path[:-1]: target = target[key]
            target[path[-1]] += "\n"
            self.assertFalse(self.validators["analysis"].is_valid(bad), path)

    def test_rust_owns_relational_validation(self):
        # These are legal JSON shapes; Rust rejects candidate/raw and clock drift.
        changed = copy.deepcopy(self.report)
        changed["result"]["beats"][0]["source_frame"] += 1
        self.validators["analysis"].validate(changed)
        changed["scope"]["channels"] = [0]
        self.validators["analysis"].validate(changed)

    def test_manifest_configuration_hash(self):
        manifest = load(BUNDLE / "manifest.json")
        expected = hashlib.sha256(SCHEMAS["provider_configuration"].read_bytes()).hexdigest()
        self.assertEqual(manifest["configuration_schemas"][0]["sha256"], expected)
        self.assertEqual(manifest["capabilities"][0]["configuration_schema"]["sha256"], expected)
        self.assertFalse(manifest["capabilities"][0]["behavior"]["cacheable"])

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-example", action="store_true")
    parser.add_argument("--musical-report", action="append", type=Path, default=[])
    args, unittest_args = parser.parse_known_args()
    if args.write_example:
        EXAMPLE.write_text(json.dumps(example(), indent=2, allow_nan=False) + "\n")
        CONFIGURATION_EXAMPLE.write_text(json.dumps(settings(), indent=2, allow_nan=False) + "\n")
    for report in args.musical_report:
        validators()["analysis"].validate(load(report))
        print(f"Validated musical report: {report}")
    unittest.main(argv=[__file__, *unittest_args])
