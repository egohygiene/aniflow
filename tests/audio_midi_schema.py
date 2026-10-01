#!/usr/bin/env python3
"""Independent strict MIDI schemas with reproducible synthetic shape fixtures.

No model is downloaded or executed. Generated PCM and dependency marker bytes
bind example identities. Rust owns cross-field source timing, exact ordering,
source-relative note bounds, tick derivation and MIDI-byte equality. Schema
validation alone does not qualify inference accuracy or exported musical intent.
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
    raise SystemExit("MIDI schema checks require an already installed jsonschema package.") from error

ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "docs/contracts"
BUNDLE = ROOT / "providers/audio-midi"
REVISION = "9991303bba609a3b93089d13ec80d1d495083596"
SCHEMAS = {
    "notes": CONTRACTS / "audio-midi-notes-v1.schema.json",
    "export": CONTRACTS / "audio-midi-export-v1.schema.json",
    "report": CONTRACTS / "audio-midi-v1.schema.json",
    "preflight": CONTRACTS / "audio-midi-preflight-v1.schema.json",
    "configuration": BUNDLE / "configuration.schema.json",
    "provider_configuration": BUNDLE / "provider-configuration.schema.json",
    "observation": CONTRACTS / "audio-midi-observation-v1.schema.json",
    "probe": CONTRACTS / "audio-midi-probe-v1.schema.json",
}
EXAMPLES = {name: CONTRACTS / "examples" / path.name.replace(".schema.", ".example.") for name, path in SCHEMAS.items() if name not in ["configuration", "provider_configuration"]}
EXAMPLES["configuration"] = BUNDLE / "configuration.example.json"


def reject_nonfinite(value):
    raise ValueError(f"Invalid JSON numeric token: {value}")


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"Duplicate JSON property: {key}")
        result[key] = value
    return result


def load(path):
    return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject_nonfinite, object_pairs_hook=reject_duplicates)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def marker(label):
    return f"Synthetic MIDI contract fixture: {label}; no model was executed.\n".encode()


def digest(label):
    return hashlib.sha256(marker(label)).hexdigest()


def artifact(identifier, content=None):
    content = marker(identifier) if content is None else content
    return {"id": identifier, "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}


def rational(numerator, denominator=1):
    return {"numerator": numerator, "denominator": denominator}


def audio_source(rate=22050, channels=1, silent=False):
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(channels)
        audio.setsampwidth(2)
        audio.setframerate(rate)
        audio.writeframes(b"".join(struct.pack("<" + "h" * channels, *([0 if silent else (1200 if frame % 100 < 50 else -1200)] * channels)) for frame in range(rate * 3)))
    return {"artifact": artifact("source_audio", output.getvalue()), "stream_index": 0, "sample_rate_hz": rate, "channels": channels, "frame_count": rate * 3, "origin": rational(0), "stem": None}


def observation():
    return {"schema": "aniflow.audio-midi-observation/v1", "sample_rate_hz": 22050, "channels": 1, "sample_frames": 66150, "duration_microseconds": 3000000, "notes": [
        {"start_microseconds": 0, "end_microseconds": 500000, "pitch": 60, "velocity": 102, "activation": 0.8},
        {"start_microseconds": 0, "end_microseconds": 500000, "pitch": 64, "velocity": 89, "activation": 0.7},
        {"start_microseconds": 1000000, "end_microseconds": 2000000, "pitch": 67, "velocity": 76, "activation": 0.6},
    ]}


def probe():
    return {"schema": "aniflow.audio-midi-probe/v1", "python_version": "3.11.13", "basic_pitch_version": "0.4.0", "basic_pitch_revision": REVISION, "onnxruntime_version": "1.20.1", "numpy_version": "1.26.4", "librosa_version": "0.10.2.post1", "scipy_version": "1.13.1", "pretty_midi_version": "0.2.10", "runtime_sha256": digest("runtime"), "runtime_file_count": 4, "runtime_byte_count": 1024, "license": {"basic_pitch_expression": "Apache-2.0", "basic_pitch_metadata_sha256": digest("basic-pitch metadata"), "onnxruntime_expression": "MIT", "onnxruntime_metadata_sha256": digest("onnxruntime metadata")}}


def replace(value, path, replacement):
    target = value
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = replacement
    return value


def validators():
    result = {}
    for name, path in SCHEMAS.items():
        value = load(path)
        Draft202012Validator.check_schema(value)
        result[name] = Draft202012Validator(value)
    return result


def model_bytes():
    return marker("model").ljust(230444, b"\0")


def configuration():
    return {"schema": "aniflow.audio-midi.configuration/v1", "python": {"executable": "/synthetic/python3", "version": "3.11.13", "sha256": digest("python")}, "adapter": {"path": "/synthetic/audio-midi-adapter.py", "sha256": digest("adapter")}, "runtime": {"sha256": digest("runtime"), "file_count": 4, "byte_count": 1024}, "model": {"path": "/synthetic/nmp.onnx", "sha256": hashlib.sha256(model_bytes()).hexdigest(), "byte_size": 230444, "model_id": "basic-pitch-onnx-icassp-2022", "revision": REVISION}, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 2097152}


def provider_configuration():
    tool = {"executable": "/synthetic/tool", "version": "6.1.1", "sha256": digest("inspection tool")}
    return {"schema": "aniflow.audio-midi.provider-configuration/v1", "settings": configuration(), "tools": {"schema": "aniflow.audio-inspection.configuration/v1", "ffmpeg": tool, "ffprobe": tool, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}, "source": audio_source()["artifact"], "upstream_analysis_artifact_id": "analysis"}


def preflight():
    return {"schema": "aniflow.audio-midi-preflight/v1", "ready": False, "diagnostics": [{"code": "missing_model", "component": "basic-pitch-onnx-icassp-2022", "message": "Synthetic fixture: the explicitly pinned ONNX model is absent."}], "probe": None}


def commands(candidate=True):
    probe_command = {"tool": "python", "arguments": ["-I", "-B", "{staged_adapter}", "--probe"]}
    analysis_command = {"tool": "python", "arguments": ["-I", "-B", "{staged_adapter}", "--input", "{snapshot}", "--model", "{staged_model}"]}
    return [copy.deepcopy(probe_command), analysis_command, copy.deepcopy(probe_command)] if candidate else [copy.deepcopy(probe_command), copy.deepcopy(probe_command)]


def report():
    settings = configuration()
    observed = observation()
    return {
        "schema": "aniflow.audio-midi/v1", "source": audio_source(), "scope": {"channels": [0], "stem_id": None}, "technical_artifact": artifact("technical"), "upstream_analysis_artifact": artifact("analysis"), "raw_observation": artifact("midi_observation", canonical(observed)),
        "provider": {"id": "org.egohygiene.aniflow.audio-midi", "version": "1.0.0"}, "implementation_sha256": digest("implementation"), "configuration_sha256": hashlib.sha256(canonical(provider_configuration())).hexdigest(), "provider_lock_sha256": digest("provider lock"),
        "settings": {"python_version": settings["python"]["version"], "python_sha256": settings["python"]["sha256"], "adapter_sha256": settings["adapter"]["sha256"], "runtime": settings["runtime"], "model": {key: value for key, value in settings["model"].items() if key != "path"}, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 2097152}, "probe": probe(), "commands": commands(),
        "method": {"sample_rate_hz": 22050, "downmix": "none", "backend": "onnxruntime_cpu", "gpu": False, "pitch_bends": False, "instrument": "unavailable", "native_time_quantization": "nearest_microsecond_ties_up", "maximum_native_time_error_nanoseconds": 500, "velocity_mapping": "round_ties_even_127_times_activation", "minimum_pitch": 21, "maximum_pitch": 108}, "provenance": "probabilistic", "confidence": {"kind": "unavailable", "reason": "Note activations and MIDI velocities are not calibrated confidence; inferred notes are unreviewed candidates."},
        "result": {"status": "candidate", "observation": {"native": observed, "notes": [
            {"id": "note_000000", "start": rational(0), "end": rational(1, 2), "start_tick": 0, "end_tick": 960, "pitch": 60, "velocity": 102, "activation": 0.8},
            {"id": "note_000001", "start": rational(0), "end": rational(1, 2), "start_tick": 0, "end_tick": 960, "pitch": 64, "velocity": 89, "activation": 0.7},
            {"id": "note_000002", "start": rational(1), "end": rational(2), "start_tick": 1920, "end_tick": 3840, "pitch": 67, "velocity": 76, "activation": 0.6},
        ]}},
    }


def midi_bytes():
    # Fixed synthetic type-0 track: tempo/program, two simultaneous notes, then
    # one later note. Explicit statuses and shortest delta encodings only.
    track = bytes.fromhex("00ff510307a12000c000 00903c66 00904059 8740803c00 00804000 874090434c 8f00804300 00ff2f00")
    return bytes.fromhex("4d546864000000060000000103c04d54726b") + struct.pack(">I", len(track)) + track


def export_report():
    candidate = report()
    clock = {"format": 0, "tracks": 1, "ticks_per_quarter": 960, "microseconds_per_quarter": 500000, "channel": 0, "program": 0}
    source_bytes = (json.dumps(candidate, indent=2, ensure_ascii=False, allow_nan=False) + "\n").encode()
    notes = candidate["result"]["observation"]
    return {"schema": "aniflow.audio-midi-export/v1", "authority": "candidate", "source_report": artifact("midi", source_bytes), "source_audio": candidate["source"]["artifact"], "notes": artifact("midi_notes", canonical(notes)), "midi": artifact("midi_candidate", midi_bytes()), "profile": {**clock, "tick_rounding": "nearest_tick_ties_up", "maximum_tick_error_seconds": rational(1, 3840), "inferred_source_tempo": False, "inferred_source_instrument": False}, "mappings": ["native_times_rounded_to_microseconds", "candidate_times_rounded_to_midi_ticks", "activation_retained_in_companion", "placeholder_channel_and_program", "pitch_bends_unavailable", "fixed_tempo_is_clock_only"], "read_back": {**clock, "notes": [{key: note[key] for key in ["start_tick", "end_tick", "pitch", "velocity"]} for note in notes["notes"]]}, "note_count": len(notes["notes"]), "empty_candidate": False}


BUILDERS = {"observation": observation, "probe": probe, "configuration": configuration, "report": report, "preflight": preflight, "export": export_report, "notes": lambda: report()["result"]["observation"]}


class MidiSchemas(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = validators()

    def test_published_examples_and_reproducible_identities(self):
        for name, build in BUILDERS.items():
            value = load(EXAMPLES[name])
            self.assertEqual(value, build(), name)
            self.validators[name].validate(value)
        self.validators["provider_configuration"].validate(provider_configuration())
        self.assertEqual(report()["raw_observation"], artifact("midi_observation", canonical(observation())))
        self.assertEqual(report()["configuration_sha256"], hashlib.sha256(canonical(provider_configuration())).hexdigest())

    def test_closed_nested_report_and_provider_shapes(self):
        for path in [[], ["source"], ["scope"], ["settings"], ["settings", "model"], ["settings", "runtime"], ["probe"], ["probe", "license"], ["method"], ["confidence"], ["raw_observation"], ["result"], ["result", "observation"], ["result", "observation", "native"], ["result", "observation", "native", "notes", 0], ["result", "observation", "notes", 0], ["result", "observation", "notes", 0, "start"]]:
            value = report()
            target = value
            for key in path:
                target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["report"].is_valid(value), path)
        for path in [[], ["settings"], ["settings", "python"], ["settings", "adapter"], ["tools"], ["source"]]:
            value = provider_configuration()
            target = value
            for key in path:
                target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["provider_configuration"].is_valid(value), path)

    def test_candidate_authority_and_command_identity(self):
        for path, value in [(["provenance"], "deterministic"), (["confidence"], {"kind": "calibrated", "score": 0.8}), (["result", "status"], "verified"), (["method", "gpu"], True), (["method", "pitch_bends"], True), (["method", "instrument"], "piano"), (["method", "maximum_native_time_error_nanoseconds"], 0), (["commands"], commands(False)), (["commands", 1, "arguments", 0], "--isolated"), (["raw_observation"], None), (["source", "sample_rate_hz"], 48000), (["result", "observation", "notes", 0, "end_tick"], 230401)]:
            self.assertFalse(self.validators["report"].is_valid(replace(report(), path, value)), path)

    def test_configuration_pins_paths_capture_and_model_bounds(self):
        for path, value in [(["model", "revision"], "main"), (["model", "byte_size"], 230445), (["model", "model_id"], "other"), (["model", "path"], "/synthetic/../model"), (["python", "executable"], "python3"), (["adapter", "path"], "/synthetic/adapter\n.py"), (["python", "version"], "3.12.1"), (["maximum_tool_output_bytes"], 2097153), (["tool_timeout_milliseconds"], 120001), (["runtime", "file_count"], 50001)]:
            self.assertFalse(self.validators["configuration"].is_valid(replace(configuration(), path, value)), path)
        value = provider_configuration()
        value["source"]["id"] = "other_audio"
        self.assertFalse(self.validators["provider_configuration"].is_valid(value))

    def test_preflight_readiness_is_bound_to_probe_and_diagnostics(self):
        ready = {"schema": "aniflow.audio-midi-preflight/v1", "ready": True, "diagnostics": [], "probe": probe()}
        self.validators["preflight"].validate(ready)
        for value in [replace(copy.deepcopy(ready), ["probe"], None), replace(copy.deepcopy(ready), ["ready"], False), replace(preflight(), ["ready"], True), replace(preflight(), ["probe"], probe()), replace(preflight(), ["diagnostics"], [])]:
            self.assertFalse(self.validators["preflight"].is_valid(value))
        for code in ["model_size_mismatch", "runtime_mismatch", "adapter_digest_mismatch", "tool_output_limit", "unsupported_platform"]:
            self.validators["preflight"].validate(replace(preflight(), ["diagnostics", 0, "code"], code))

    def test_unavailable_and_empty_candidate_shapes(self):
        for reason, source in [("unsupported_sample_rate", audio_source(48000)), ("unsupported_channels", audio_source(22050, 2)), ("silent_input", audio_source(silent=True)), ("duration_limit", audio_source())]:
            if reason == "duration_limit":
                source["frame_count"] = 2646001
                source["artifact"]["byte_size"] = 44 + 2646001 * 2
            value = report()
            value.update(source=source, scope={"channels": list(range(source["channels"])), "stem_id": None}, raw_observation=None, result={"status": "unavailable", "reason": reason}, commands=commands(False))
            self.validators["report"].validate(value)
            value["raw_observation"] = report()["raw_observation"]
            self.assertFalse(self.validators["report"].is_valid(value))
        value = report()
        value["result"]["observation"]["native"]["notes"] = []
        value["result"]["observation"]["notes"] = []
        self.validators["report"].validate(value)

    def test_manifest_exact_schema_identity_and_port_shapes(self):
        manifest = load(BUNDLE / "manifest.json")
        expected = hashlib.sha256(SCHEMAS["provider_configuration"].read_bytes()).hexdigest()
        self.assertEqual(manifest["configuration_schemas"][0]["sha256"], expected)
        cap = manifest["capabilities"][0]
        self.assertEqual(cap["configuration_schema"]["sha256"], expected)
        self.assertEqual(cap["id"], "aniflow/audio-midi-extraction")
        self.assertEqual([item["name"] for item in cap["inputs"]], ["audio", "technical", "upstream_analysis"])
        self.assertEqual([item["name"] for item in cap["outputs"]], ["midi", "analysis"])
        self.assertEqual(cap["outputs"][0]["artifact_type"], "application/vnd.aniflow.audio-midi+json")
        self.assertFalse(cap["behavior"]["cacheable"])
        self.assertFalse(cap["behavior"]["content_changes"])
        Draft202012Validator(load(CONTRACTS / "provider-manifest-v1.schema.json")).validate(manifest)

    def test_export_authority_loss_clock_and_information_boundaries(self):
        self.assertEqual(export_report()["source_report"], artifact("midi", EXAMPLES["report"].read_bytes()))
        self.assertEqual(export_report()["notes"], artifact("midi_notes", canonical(report()["result"]["observation"])))
        self.assertEqual(export_report()["midi"], artifact("midi_candidate", midi_bytes()))
        for path, value in [(["authority"], "reviewed"), (["profile", "format"], 1), (["profile", "tracks"], 2), (["profile", "ticks_per_quarter"], 480), (["profile", "inferred_source_tempo"], True), (["profile", "inferred_source_instrument"], True), (["profile", "maximum_tick_error_seconds"], rational(0)), (["mappings"], []), (["read_back", "program"], 42), (["read_back", "notes", 0, "pitch"], 109), (["read_back", "notes", 0, "end_tick"], 230401), (["midi", "byte_size"], 262145), (["source_report", "id"], "other"), (["empty_candidate"], True)]:
            self.assertFalse(self.validators["export"].is_valid(replace(export_report(), path, value)), path)
        value = export_report()
        value["mappings"].reverse()
        self.assertFalse(self.validators["export"].is_valid(value))
        for path in [[], ["profile"], ["read_back"], ["read_back", "notes", 0], ["midi"]]:
            value = export_report()
            target = value
            for key in path:
                target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["export"].is_valid(value), path)

    def test_notes_companion_is_exact_closed_normalized_observation(self):
        value = report()["result"]["observation"]
        self.validators["notes"].validate(value)
        self.assertEqual(value, load(EXAMPLES["notes"]))
        for path, replacement in [(["notes", 0, "start_tick"], -1), (["notes", 0, "activation"], 1.1), (["native", "notes", 0, "velocity"], 128), (["notes", 0, "start"], rational(121)), (["notes", 0, "start"], rational(1, 3))]:
            self.assertFalse(self.validators["notes"].is_valid(replace(copy.deepcopy(value), path, replacement)), path)
        changed = copy.deepcopy(value)
        changed["schema"] = "aniflow.audio-midi-notes/v1"
        self.assertFalse(self.validators["notes"].is_valid(changed))
        changed = copy.deepcopy(value)
        changed["notes"][0]["reviewed"] = True
        self.assertFalse(self.validators["notes"].is_valid(changed))

    def test_empty_export_has_an_explicit_empty_track(self):
        value = export_report()
        value.update(note_count=0, empty_candidate=True)
        value["read_back"]["notes"] = []
        value["midi"]["byte_size"] = 36
        self.validators["export"].validate(value)
        for path, replacement in [(["note_count"], 1), (["midi", "byte_size"], 37), (["empty_candidate"], False)]:
            self.assertFalse(self.validators["export"].is_valid(replace(copy.deepcopy(value), path, replacement)), path)

    def test_native_profile_numeric_and_note_bounds(self):
        cases = [(["sample_rate_hz"], 44100), (["channels"], 2), (["sample_frames"], 2646001), (["duration_microseconds"], 120000001), (["notes", 0, "start_microseconds"], -1), (["notes", 0, "end_microseconds"], 0), (["notes", 0, "pitch"], 20), (["notes", 0, "pitch"], 109), (["notes", 0, "pitch"], 60.5), (["notes", 0, "velocity"], 0), (["notes", 0, "velocity"], 128), (["notes", 0, "activation"], 1.001), (["notes", 0, "activation"], -0.1), (["notes", 0, "start_microseconds"], 0.5)]
        for path, value in cases:
            self.assertFalse(self.validators["observation"].is_valid(replace(observation(), path, value)), path)
        value = observation()
        value["notes"] = [value["notes"][0]] * 8193
        self.assertFalse(self.validators["observation"].is_valid(value))

    def test_native_zero_note_and_polyphonic_shapes(self):
        self.validators["observation"].validate(observation())
        value = observation()
        value["notes"] = []
        self.validators["observation"].validate(value)

    def test_probe_pins_runtime_bounds_and_license_identity(self):
        for path, value in [(["python_version"], "3.12.1"), (["python_version"], "3.11.13\n"), (["basic_pitch_version"], "0.4.1"), (["basic_pitch_revision"], "latest"), (["onnxruntime_version"], "1.21.0"), (["numpy_version"], "2.0.0"), (["runtime_file_count"], 0), (["runtime_file_count"], 50001), (["runtime_byte_count"], 2147483649), (["license", "basic_pitch_expression"], "MIT"), (["runtime_sha256"], "0" * 64 + "\n")]:
            self.assertFalse(self.validators["probe"].is_valid(replace(probe(), path, value)), path)

    def test_rust_owns_cross_field_source_note_semantics(self):
        # Legal JSON shapes. Rust additionally rejects these relational errors.
        value = observation()
        value["notes"][0]["end_microseconds"] = 4000000
        self.validators["observation"].validate(value)
        value = observation()
        value["notes"].reverse()
        self.validators["observation"].validate(value)
        value = observation()
        value["notes"][1]["pitch"] = 60
        self.validators["observation"].validate(value)
        value["duration_microseconds"] += 1
        self.validators["observation"].validate(value)

    def test_fixture_loader_rejects_duplicate_keys_and_nonfinite(self):
        with self.assertRaises(ValueError):
            json.loads('{"pitch":60,"pitch":61}', object_pairs_hook=reject_duplicates)
        for value in ["NaN", "Infinity", "-Infinity"]:
            with self.assertRaises(ValueError):
                json.loads(value, parse_constant=reject_nonfinite)

    def test_schemas_prune_unreferenced_definitions(self):
        for name, path in SCHEMAS.items():
            schema = load(path)
            definitions, seen, pending = schema.get("$defs", {}), set(), []
            def walk(node):
                if isinstance(node, dict):
                    if "$ref" in node:
                        pending.append(node["$ref"].removeprefix("#/$defs/"))
                    for key, value in node.items():
                        if key != "$defs":
                            walk(value)
                elif isinstance(node, list):
                    for value in node:
                        walk(value)
            walk(schema)
            while pending:
                key = pending.pop()
                if key not in seen:
                    seen.add(key)
                    walk(definitions[key])
            self.assertEqual(seen, set(definitions), name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-examples", action="store_true")
    parser.add_argument("--midi-report", type=Path, action="append", default=[])
    parser.add_argument("--preflight-report", type=Path, action="append", default=[])
    parser.add_argument("--export-report", type=Path, action="append", default=[])
    parser.add_argument("--notes-report", type=Path, action="append", default=[])
    args, unittest_args = parser.parse_known_args()
    if args.write_examples:
        for name, builder in BUILDERS.items():
            EXAMPLES[name].write_text(json.dumps(builder(), indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    for kind, paths in [("report", args.midi_report), ("preflight", args.preflight_report), ("export", args.export_report), ("notes", args.notes_report)]:
        for path in paths:
            validators()[kind].validate(load(path))
            print(f"Validated MIDI {kind}: {path}")
    unittest.main(argv=[__file__, *unittest_args])
