#!/usr/bin/env python3
"""Independent transcription schemas and deterministic synthetic shape examples.

The source PCM and native-shaped observations are generated. Tool/model/provider
identities are synthetic marker bytes; no model is downloaded or executed and no
speech recognition accuracy is claimed. Rust validates reduced times, ordering,
source duration, exact command settings and the derived observed-text projection.
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
    raise SystemExit("Transcription schema checks require an already installed jsonschema package.") from error

ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "docs/contracts"
BUNDLE = ROOT / "providers/audio-transcription"
SCHEMAS = {"report": CONTRACTS / "audio-transcription-v1.schema.json", "preflight": CONTRACTS / "audio-transcription-preflight-v1.schema.json", "configuration": BUNDLE / "configuration.schema.json", "provider_configuration": BUNDLE / "provider-configuration.schema.json"}
EXAMPLES = {"report": CONTRACTS / "examples/audio-transcription-v1.example.json", "preflight": CONTRACTS / "examples/audio-transcription-preflight-v1.example.json", "configuration": BUNDLE / "configuration.example.json"}
CONFIDENCE_REASON = "Basic whisper.cpp JSON does not report calibrated segment confidence."
WORD_REASON = "Basic whisper.cpp JSON reports segment timing, not word timing."
PRODUCER = "org.egohygiene.aniflow.audio-transcription@1.0.0"
MODEL_PARAMETERS = {"type": "tiny", "multilingual": False, "vocab": 51864, "audio": {"ctx": 1500, "state": 384, "head": 6, "layer": 4}, "text": {"ctx": 448, "state": 384, "head": 6, "layer": 4}, "mels": 80, "ftype": 1}

def reject_nonfinite(value):
    raise ValueError(f"Invalid JSON numeric token: {value}")

def load(path):
    return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject_nonfinite)

def marker(label):
    return f"Synthetic transcription shape fixture: {label}; no model was executed.\n".encode()

def digest(label):
    return hashlib.sha256(marker(label)).hexdigest()

def artifact(identifier, content=None):
    content = marker(identifier) if content is None else content
    return {"id": identifier, "sha256": hashlib.sha256(content).hexdigest(), "byte_size": len(content)}

def rational(numerator, denominator=1):
    return {"numerator": numerator, "denominator": denominator}

def audio_source(rate=16000, channels=1, silent=False):
    output = io.BytesIO()
    with wave.open(output, "wb") as audio:
        audio.setnchannels(channels)
        audio.setsampwidth(2)
        audio.setframerate(rate)
        audio.writeframes(b"".join(struct.pack("<" + "h" * channels, *([0 if silent else (1200 if frame % 100 < 50 else -1200)] * channels)) for frame in range(rate * 3)))
    return {"artifact": artifact("source_audio", output.getvalue()), "stream_index": 0, "sample_rate_hz": rate, "channels": channels, "frame_count": rate * 3, "origin": rational(0), "stem": None}

def configuration():
    return {"schema": "aniflow.audio-transcription.configuration/v1", "whisper": {"executable": "/synthetic/whisper-cli", "version": "1.8.7", "sha256": digest("whisper executable")}, "model": {"path": "/synthetic/tiny.en.bin", "sha256": digest("model marker"), "byte_size": len(marker("model marker")), "model_id": "tiny.en", "revision": "synthetic fixture revision"}, "language": "en", "threads": 1, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}

def observation():
    return {"detected_language": "en", "model": copy.deepcopy(MODEL_PARAMETERS), "segments": [
        {"id": "transcription_segment_000001", "start": rational(0), "end": rational(1), "text": " Synthetic observed words."},
        {"id": "transcription_segment_000002", "start": rational(5, 4), "end": rational(5, 2), "text": " Café e\u0301 — synthetic Unicode. "},
    ]}

def raw_bytes():
    native = {"systeminfo": "Synthetic native-shaped observation; no whisper process or model executed.", "model": copy.deepcopy(MODEL_PARAMETERS), "params": {"model": "{staged_model}", "language": "en", "translate": False}, "result": {"language": "en"}, "transcription": [
        {"timestamps": {"from": "00:00:00,000", "to": "00:00:01,000"}, "offsets": {"from": 0, "to": 1000}, "text": observation()["segments"][0]["text"]},
        {"timestamps": {"from": "00:00:01,250", "to": "00:00:02,500"}, "offsets": {"from": 1250, "to": 2500}, "text": observation()["segments"][1]["text"]},
    ]}
    return json.dumps(native, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()

def commands(analyzed=True, threads=1):
    probe = {"tool": "whisper", "arguments": ["--version"]}
    result = [probe]
    if analyzed:
        result.append({"tool": "whisper", "arguments": ["--model", "{staged_model}", "--file", "{snapshot}", "--language", "en", "--threads", str(threads), "--processors", "1", "--no-gpu", "--no-flash-attn", "--no-fallback", "--temperature", "0", "--best-of", "1", "--beam-size", "1", "--output-json", "--output-file", "-", "--no-prints"]})
    return [*result, copy.deepcopy(probe)]

def timed_text(source, raw, observed):
    return {"schema": "aniflow.timed-text/v1", "source": {**raw, "id": "source_text"}, "provenance": {"kind": "observed_transcript", "producer": PRODUCER}, "language": "en", "audio_source": copy.deepcopy(source), "overlap_policy": "reject", "metadata": {}, "cues": [{"id": segment["id"], "source_label": None, "text": segment["text"], "timing": {"kind": "interval", "start": segment["start"], "end": segment["end"]}, "speaker": None} for segment in observed["segments"]]}

def report():
    source, settings, observed = audio_source(), configuration(), observation()
    raw = artifact("transcription_observation", raw_bytes())
    model = {key: value for key, value in settings["model"].items() if key != "path"}
    return {"schema": "aniflow.audio-transcription/v1", "source": source, "scope": {"channels": [0], "stem_id": None}, "technical_artifact": artifact("technical"), "upstream_analysis_artifact": artifact("analysis"), "raw_observation": raw, "provider": {"id": "org.egohygiene.aniflow.audio-transcription", "version": "1.0.0"}, "implementation_sha256": digest("implementation"), "configuration_sha256": digest("effective configuration"), "provider_lock_sha256": digest("provider lock"), "settings": {"whisper_version": "1.8.7", "whisper_sha256": settings["whisper"]["sha256"], "model": model, "language": "en", "threads": 1, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}, "licenses": {"tool_expression": "MIT", "tool_source_url": "https://github.com/ggml-org/whisper.cpp/blob/48f628a84833905ee4a0658ee6d4a5c915ce1997/LICENSE", "model_expression": "MIT", "model_source_url": "https://huggingface.co/ggerganov/whisper.cpp/blob/5359861c739e955e79d9a303bcbc70fb988958b1/README.md"}, "commands": commands(), "method": {"sample_rate_hz": 16000, "downmix": "none", "timestamp_grid_milliseconds": 10, "translation": False, "vad": False, "word_timestamps": False, "gpu": False}, "provenance": "probabilistic", "confidence": {"kind": "unavailable", "reason": CONFIDENCE_REASON}, "word_timing": {"status": "unavailable", "reason": WORD_REASON}, "result": {"status": "observed", "observation": observed}, "timed_text": timed_text(source, raw, observed)}

def preflight():
    return {"schema": "aniflow.audio-transcription-preflight/v1", "ready": False, "diagnostics": [{"code": "missing_model", "component": "model", "message": "Synthetic fixture: the configured model file is absent."}]}

BUILDERS = {"report": report, "preflight": preflight, "configuration": configuration}

def validators():
    result = {}
    for name, path in SCHEMAS.items():
        schema = load(path)
        Draft202012Validator.check_schema(schema)
        result[name] = Draft202012Validator(schema)
    return result

class TranscriptionSchemas(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = validators()
        cls.report = load(EXAMPLES["report"])

    def test_published_synthetic_shapes_and_captured_identity(self):
        for name, build in BUILDERS.items():
            value = load(EXAMPLES[name])
            self.assertEqual(value, build(), name)
            self.validators[name].validate(value)
        self.assertEqual(self.report["raw_observation"], artifact("transcription_observation", raw_bytes()))
        timed = load(CONTRACTS / "timed-text-v1.schema.json")
        Draft202012Validator(timed).validate(self.report["timed_text"])
        tool = {"executable": "/synthetic/tool", "version": "6.1.1", "sha256": digest("inspection tool")}
        self.validators["provider_configuration"].validate({"schema": "aniflow.audio-transcription.provider-configuration/v1", "settings": configuration(), "tools": {"schema": "aniflow.audio-inspection.configuration/v1", "ffmpeg": tool, "ffprobe": tool, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}, "source": self.report["source"]["artifact"], "upstream_analysis_artifact_id": "analysis"})

    def test_unknown_nested_fields_are_closed(self):
        for path in [[], ["source"], ["settings"], ["settings", "model"], ["method"], ["confidence"], ["word_timing"], ["raw_observation"], ["result"], ["result", "observation"], ["result", "observation", "model", "audio"], ["result", "observation", "segments", 0], ["timed_text"], ["timed_text", "provenance"], ["timed_text", "cues", 0, "timing"]]:
            value = copy.deepcopy(self.report)
            target = value
            for key in path: target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["report"].is_valid(value), path)

    def test_authority_confidence_word_timing_and_methods_cannot_be_promoted(self):
        cases = [(["provenance"], "deterministic"), (["confidence"], {"kind": "calibrated", "score": 0.9}), (["word_timing"], {"status": "available"}), (["timed_text", "provenance"], {"kind": "reviewed_lyrics"}), (["timed_text", "provenance", "producer"], "human-reviewer"), (["method", "word_timestamps"], True), (["method", "translation"], True), (["method", "downmix"], "arithmetic_average"), (["licenses", "model_expression"], "unknown")]
        for path, replacement in cases:
            value = copy.deepcopy(self.report)
            target = value
            for key in path[:-1]: target = target[key]
            target[path[-1]] = replacement
            self.assertFalse(self.validators["report"].is_valid(value), path)

    def test_valid_reviewed_text_cannot_be_promoted_into_transcription(self):
        value = copy.deepcopy(self.report)
        value["timed_text"]["provenance"] = {"kind": "reviewed_lyrics", "authority": {"supplied_by": "synthetic-reviewer", "provenance_artifact_id": "review_evidence"}, "evidence": artifact("review_evidence"), "source_sha256": value["timed_text"]["source"]["sha256"]}
        Draft202012Validator(load(CONTRACTS / "timed-text-v1.schema.json")).validate(value["timed_text"])
        self.assertFalse(self.validators["report"].is_valid(value))

    def test_observed_empty_and_unavailable_are_distinct(self):
        value = copy.deepcopy(self.report)
        value["result"]["status"] = "empty"
        self.assertFalse(self.validators["report"].is_valid(value))
        value["result"]["observation"]["segments"] = []
        value["timed_text"] = None
        self.validators["report"].validate(value)
        value["raw_observation"] = None
        self.assertFalse(self.validators["report"].is_valid(value))
        for reason, source in [("unsupported_sample_rate", audio_source(48000)), ("unsupported_channels", audio_source(16000, 2)), ("silent_input", audio_source(silent=True))]:
            value = copy.deepcopy(self.report)
            value.update(source=source, scope={"channels": list(range(source["channels"])), "stem_id": None}, result={"status": "unavailable", "reason": reason}, raw_observation=None, timed_text=None, commands=commands(False))
            self.validators["report"].validate(value)
            value["raw_observation"] = self.report["raw_observation"]
            self.assertFalse(self.validators["report"].is_valid(value))
        value = copy.deepcopy(self.report)
        value["timed_text"] = None
        self.assertFalse(self.validators["report"].is_valid(value))

    def test_native_timing_grid_and_model_bounds(self):
        for invalid in [rational(-1), rational(1, 1000), rational(1, 3), rational(601), rational(60001, 100), {"numerator": 0.5, "denominator": 1}]:
            value = copy.deepcopy(self.report)
            value["result"]["observation"]["segments"][0]["start"] = invalid
            self.assertFalse(self.validators["report"].is_valid(value), invalid)
        for field, replacement in [("multilingual", True), ("vocab", 51865), ("ftype", 0), ("mels", 128)]:
            value = copy.deepcopy(self.report)
            value["result"]["observation"]["model"][field] = replacement
            self.assertFalse(self.validators["report"].is_valid(value), field)
        value = copy.deepcopy(self.report)
        value["source"]["frame_count"] = 9600001
        self.assertFalse(self.validators["report"].is_valid(value))

    def test_configuration_pins_paths_and_unsupported_language_preflight(self):
        for path, replacement in [(["whisper", "version"], "1.8.6"), (["whisper", "sha256"], "0" * 64 + "\n"), (["model", "model_id"], "base.en"), (["model", "byte_size"], 268435457), (["model", "path"], "/synthetic/../model.bin"), (["threads"], 17), (["language"], "EN")]:
            value = configuration()
            target = value
            for key in path[:-1]: target = target[key]
            target[path[-1]] = replacement
            self.assertFalse(self.validators["configuration"].is_valid(value), path)
        value = configuration()
        value["language"] = "fr"
        self.validators["configuration"].validate(value)  # Typed preflight refuses unsupported language.
        value = preflight()
        value["diagnostics"][0]["code"] = "unsupported_language"
        self.validators["preflight"].validate(value)
        value["ready"] = True
        self.assertFalse(self.validators["preflight"].is_valid(value))
        self.validators["preflight"].validate({"schema": "aniflow.audio-transcription-preflight/v1", "ready": True, "diagnostics": []})

    def test_stem_scope_and_unicode_text_remain_observed(self):
        value = copy.deepcopy(self.report)
        original = copy.deepcopy(value["source"]["artifact"])
        original["id"] = "original_mix"
        value["source"]["stem"] = {"id": "voice_layer", "original_mix": original, "relationship_evidence_id": "stem_lineage"}
        value["scope"]["stem_id"] = "voice_layer"
        value["upstream_analysis_artifact"]["id"] = "stem_analysis"
        value["timed_text"]["audio_source"] = copy.deepcopy(value["source"])
        self.validators["report"].validate(value)
        for text in ["", "bad\0", "bad\u0085"]:
            value["result"]["observation"]["segments"][0]["text"] = text
            self.assertFalse(self.validators["report"].is_valid(value), repr(text))

    def test_exact_commands_and_capture_bounds(self):
        value = copy.deepcopy(self.report)
        value["commands"][1]["arguments"].append("--output-json-full")
        self.assertFalse(self.validators["report"].is_valid(value))
        value = copy.deepcopy(self.report)
        value["raw_observation"]["byte_size"] = 1048577
        self.assertFalse(self.validators["report"].is_valid(value))
        value = copy.deepcopy(self.report)
        value["raw_observation"]["persisted"] = True
        self.assertFalse(self.validators["report"].is_valid(value))

    def test_rust_owns_source_bounds_order_and_derived_equality(self):
        value = copy.deepcopy(self.report)
        value["result"]["observation"]["segments"][0]["end"] = rational(4)  # source is 3 seconds
        self.validators["report"].validate(value)
        value["result"]["observation"]["segments"].reverse()
        self.validators["report"].validate(value)
        value["timed_text"]["cues"][0]["text"] = "Different text"
        self.validators["report"].validate(value)
        value["commands"][1]["arguments"][7] = "2"  # settings still selects one thread
        self.validators["report"].validate(value)

    def test_schemas_prune_unreferenced_definitions(self):
        for name, path in SCHEMAS.items():
            schema = load(path)
            definitions, seen, pending = schema.get("$defs", {}), set(), []
            def walk(node):
                if isinstance(node, dict):
                    if "$ref" in node: pending.append(node["$ref"].removeprefix("#/$defs/"))
                    for key, value in node.items():
                        if key != "$defs": walk(value)
                elif isinstance(node, list):
                    for value in node: walk(value)
            walk(schema)
            while pending:
                key = pending.pop()
                if key in seen: continue
                seen.add(key)
                walk(definitions[key])
            self.assertEqual(seen, set(definitions), name)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-examples", action="store_true")
    parser.add_argument("--transcription-report", type=Path, action="append", default=[])
    parser.add_argument("--preflight-report", type=Path, action="append", default=[])
    args, unittest_args = parser.parse_known_args()
    if args.write_examples:
        for name, builder in BUILDERS.items():
            EXAMPLES[name].write_text(json.dumps(builder(), indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    for kind, paths in [("report", args.transcription_report), ("preflight", args.preflight_report)]:
        for path in paths:
            validators()[kind].validate(load(path))
            print(f"Validated transcription {kind}: {path}")
    unittest.main(argv=[__file__, *unittest_args])
