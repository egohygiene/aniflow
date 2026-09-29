#!/usr/bin/env python3
"""Independent alignment contract shapes and reproducible synthetic examples.

Generated PCM, reviewed lyric text, marker dependencies and native-shaped output
exercise data contracts. No model is downloaded or executed. The review authority
is a synthetic caller claim, not independently verified human review. Rust owns
cross-field identity, UTF-8 byte offsets, source-time bounds, reduced timing,
resource inventory ordering and exact candidate derivation.
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
    raise SystemExit("Alignment schema checks require an already installed jsonschema package.") from error

ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ROOT / "docs/contracts"
BUNDLE = ROOT / "providers/audio-alignment"
SCHEMAS = {
    "report": CONTRACTS / "audio-alignment-v1.schema.json",
    "preflight": CONTRACTS / "audio-alignment-preflight-v1.schema.json",
    "configuration": BUNDLE / "configuration.schema.json",
    "provider_configuration": BUNDLE / "provider-configuration.schema.json",
}
EXAMPLES = {
    "report": CONTRACTS / "examples/audio-alignment-v1.example.json",
    "preflight": CONTRACTS / "examples/audio-alignment-preflight-v1.example.json",
    "configuration": BUNDLE / "configuration.example.json",
}
MODEL_FILES = ["feat.params", "mdef", "means", "noisedict", "sendump", "transition_matrices", "variances"]


def reject_nonfinite(value):
    raise ValueError(f"Invalid JSON numeric token: {value}")


def load(path):
    return json.loads(path.read_text(encoding="utf-8"), parse_constant=reject_nonfinite)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def marker(label):
    return f"Synthetic alignment fixture: {label}; no model was executed.\n".encode()


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
    resources = [{"name": name, "sha256": digest(name), "byte_size": len(marker(name))} for name in MODEL_FILES]
    return {
        "schema": "aniflow.audio-alignment.configuration/v1",
        "pocketsphinx": {"executable": "/synthetic/pocketsphinx", "version": "5.1.1", "sha256": digest("pocketsphinx executable")},
        "model": {"directory": "/synthetic/en-us", "model_id": "en-us", "revision": "synthetic fixture revision", "files": resources},
        "dictionary": {"path": "/synthetic/cmudict-en-us.dict", "sha256": digest("dictionary"), "byte_size": len(marker("dictionary"))},
        "language": "en", "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576,
    }


def preflight():
    return {"schema": "aniflow.audio-alignment-preflight/v1", "ready": False, "diagnostics": [{"code": "missing_model", "component": "en-us", "message": "Synthetic fixture: an explicitly pinned acoustic resource is absent."}]}


def validators():
    result = {}
    for name, path in SCHEMAS.items():
        schema = load(path)
        Draft202012Validator.check_schema(schema)
        result[name] = Draft202012Validator(schema)
    return result


def reviewed_lyrics():
    source = artifact("source_text", b"Hello, world!\nStay here.\n")
    document = {
        "schema": "aniflow.timed-text/v1", "source": source,
        "provenance": {"kind": "reviewed_lyrics", "authority": {"supplied_by": "synthetic-reviewer", "provenance_artifact_id": "review_evidence"}, "evidence": artifact("review_evidence"), "source_sha256": source["sha256"]},
        "language": "en", "audio_source": None, "overlap_policy": "reject", "metadata": {},
        "cues": [
            {"id": "cue_000001", "source_label": None, "text": "Hello, world!", "timing": {"kind": "untimed"}, "speaker": None},
            {"id": "cue_000002", "source_label": None, "text": "Stay here.", "timing": {"kind": "untimed"}, "speaker": None},
        ],
    }
    return {"artifact": artifact("reviewed_lyrics", canonical(document)), "document": document}


def observation():
    native = [
        {"text": "hello", "start": rational(0), "end": rational(1, 2)},
        {"text": "world", "start": rational(1, 2), "end": rational(1)},
        {"text": "stay", "start": rational(3, 2), "end": rational(2)},
        {"text": "here", "start": rational(2), "end": rational(5, 2)},
    ]
    tokens = [
        {"cue_id": "cue_000001", "byte_start": 0, "byte_end": 5, "text": "Hello", "normalized": "hello"},
        {"cue_id": "cue_000001", "byte_start": 7, "byte_end": 12, "text": "world", "normalized": "world"},
        {"cue_id": "cue_000002", "byte_start": 0, "byte_end": 4, "text": "Stay", "normalized": "stay"},
        {"cue_id": "cue_000002", "byte_start": 5, "byte_end": 9, "text": "here", "normalized": "here"},
    ]
    words = [{"token": token, "timing": {"status": "candidate", "start": word["start"], "end": word["end"]}} for token, word in zip(tokens, native)]
    cues = [
        {"cue_id": "cue_000001", "timing": {"status": "candidate", "start": rational(0), "end": rational(1)}},
        {"cue_id": "cue_000002", "timing": {"status": "candidate", "start": rational(3, 2), "end": rational(5, 2)}},
    ]
    return {"native_words": native, "words": words, "cues": cues}


def raw_bytes():
    # Synthetic captured bytes: no actual alignment inference is represented.
    return canonical({"b": 0.0, "d": 3.0, "p": 1.0, "t": "hello world stay here", "w": [{"b": start, "d": 0.5, "p": 0.8, "t": word} for start, word in [(0.0, "hello"), (0.5, "world"), (1.5, "stay"), (2.0, "here")]]})


def commands(candidate=True):
    if not candidate:
        return []
    return [{"tool": "pocketsphinx", "arguments": ["-hmm", "{staged_model}", "-dict", "{staged_dictionary}", "-lm", "{staged_disabled_lm}", "-samprate", "16000", "-frate", "100", "-phone_align", "no", "-state_align", "no", "-fsgusealtpron", "no", "-loglevel", "ERROR", "align", "{snapshot}", "{reviewed_phrase}"]}]


def report():
    source, settings, reviewed, observed = audio_source(), configuration(), reviewed_lyrics(), observation()
    projected = copy.deepcopy(reviewed["document"])
    projected["audio_source"] = copy.deepcopy(source)
    projected["metadata"]["aniflow_alignment_timing"] = "candidate"
    for cue, alignment in zip(projected["cues"], observed["cues"]):
        cue["timing"] = {"kind": "interval", "start": alignment["timing"]["start"], "end": alignment["timing"]["end"]}
    revision = "511126b492dcb267cf30d49d631946d7b61a9530"
    return {
        "schema": "aniflow.audio-alignment/v1", "source": source, "scope": {"channels": [0], "stem_id": None},
        "technical_artifact": artifact("technical"), "upstream_analysis_artifact": artifact("analysis"),
        "reviewed_lyrics": reviewed, "raw_observation": artifact("alignment_observation", raw_bytes()),
        "provider": {"id": "org.egohygiene.aniflow.audio-alignment", "version": "1.0.0"},
        "implementation_sha256": digest("implementation"), "configuration_sha256": hashlib.sha256(canonical(provider_configuration())).hexdigest(), "provider_lock_sha256": digest("provider lock"),
        "settings": {"pocketsphinx_version": "5.1.1", "pocketsphinx_sha256": settings["pocketsphinx"]["sha256"], "model": {key: value for key, value in settings["model"].items() if key != "directory"}, "dictionary": {key: value for key, value in settings["dictionary"].items() if key != "path"}, "language": "en", "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576},
        "licenses": {"tool_expression": "BSD-2-Clause; bundled components retain their own notices", "tool_source_url": f"https://github.com/cmusphinx/pocketsphinx/blob/{revision}/LICENSE", "model_expression": "BSD-2-Clause (Alpha Cephei declaration)", "model_source_url": f"https://github.com/cmusphinx/pocketsphinx/blob/{revision}/model/en-us/en-us/README", "dictionary_statement": "CMU dictionary attribution and upstream terms are recorded, not independently verified.", "dictionary_source_url": f"https://github.com/cmusphinx/pocketsphinx/blob/{revision}/model/en-us/cmudict-en-us.dict"},
        "commands": commands(), "method": {"sample_rate_hz": 16000, "downmix": "none", "timestamp_grid_milliseconds": 10, "tokenization": "ascii-english-byte-offsets/v1", "alternate_pronunciations": False, "gpu": False, "review_attestation_independently_verified": False, "timing_reviewed": False},
        "provenance": "probabilistic", "confidence": {"kind": "unavailable", "reason": "Forced alignment scores are not calibrated confidence and do not establish that the supplied words occur in the audio."},
        "result": {"status": "candidate", "observation": observed}, "timed_text": projected,
    }


def provider_configuration():
    tool = {"executable": "/synthetic/tool", "version": "6.1.1", "sha256": digest("inspection tool")}
    return {"schema": "aniflow.audio-alignment.provider-configuration/v1", "settings": configuration(), "tools": {"schema": "aniflow.audio-inspection.configuration/v1", "ffmpeg": tool, "ffprobe": tool, "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576}, "source": audio_source()["artifact"], "reviewed_lyrics": reviewed_lyrics()["artifact"], "upstream_analysis_artifact_id": "analysis"}


BUILDERS = {"report": report, "preflight": preflight, "configuration": configuration}


def replace(value, path, replacement):
    target = value
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = replacement
    return value


class AlignmentSchemas(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validators = validators()
        cls.report = load(EXAMPLES["report"])

    def test_published_examples_and_canonical_synthetic_identities(self):
        for name, build in BUILDERS.items():
            value = load(EXAMPLES[name])
            self.assertEqual(value, build(), name)
            self.validators[name].validate(value)
        reviewed = self.report["reviewed_lyrics"]
        self.assertEqual(reviewed["artifact"], artifact("reviewed_lyrics", canonical(reviewed["document"])))
        self.assertEqual(self.report["raw_observation"], artifact("alignment_observation", raw_bytes()))
        self.validators["provider_configuration"].validate(provider_configuration())
        self.assertEqual(self.report["configuration_sha256"], hashlib.sha256(canonical(provider_configuration())).hexdigest())
        validator = Draft202012Validator(load(CONTRACTS / "timed-text-v1.schema.json"))
        validator.validate(reviewed["document"])
        validator.validate(self.report["timed_text"])

    def test_closed_nested_report_objects(self):
        for path in [[], ["source"], ["scope"], ["reviewed_lyrics"], ["reviewed_lyrics", "document"], ["reviewed_lyrics", "document", "provenance"], ["reviewed_lyrics", "document", "provenance", "authority"], ["settings"], ["settings", "model"], ["settings", "model", "files", 0], ["settings", "dictionary"], ["method"], ["confidence"], ["raw_observation"], ["result"], ["result", "observation"], ["result", "observation", "words", 0], ["result", "observation", "words", 0, "token"], ["result", "observation", "words", 0, "timing"], ["timed_text", "cues", 0]]:
            value = copy.deepcopy(self.report)
            target = value
            for key in path:
                target = target[key]
            target["unknown"] = True
            self.assertFalse(self.validators["report"].is_valid(value), path)

    def test_authority_cannot_be_invented_or_promoted(self):
        cases = [(["provenance"], "deterministic"), (["confidence"], {"kind": "calibrated", "score": 0.9}), (["method", "review_attestation_independently_verified"], True), (["method", "timing_reviewed"], True), (["reviewed_lyrics", "document", "provenance"], {"kind": "unreviewed"}), (["timed_text", "provenance"], {"kind": "observed_transcript", "producer": "aligner"}), (["timed_text", "metadata", "aniflow_alignment_timing"], "reviewed"), (["result", "status"], "verified")]
        for path, replacement in cases:
            value = replace(copy.deepcopy(self.report), path, replacement)
            self.assertFalse(self.validators["report"].is_valid(value), path)
        value = copy.deepcopy(self.report)
        del value["timed_text"]["metadata"]["aniflow_alignment_timing"]
        self.assertFalse(self.validators["report"].is_valid(value))

    def test_fixed_profile_commands_and_capture_bounds(self):
        cases = [(["settings", "pocketsphinx_version"], "5.1.0"), (["settings", "language"], "fr"), (["settings", "maximum_tool_output_bytes"], 1048577), (["settings", "tool_timeout_milliseconds"], 120001), (["method", "gpu"], True), (["method", "alternate_pronunciations"], True), (["raw_observation", "byte_size"], 1048577), (["reviewed_lyrics", "artifact", "byte_size"], 65537), (["commands", 0, "arguments", 15], "yes"), (["commands"], [])]
        for path, replacement in cases:
            self.assertFalse(self.validators["report"].is_valid(replace(copy.deepcopy(self.report), path, replacement)), path)

    def test_configuration_resources_pins_and_paths(self):
        cases = [(["model", "model_id"], "other"), (["model", "files", 0, "name"], "README"), (["model", "files", 0, "byte_size"], 67108865), (["dictionary", "byte_size"], 16777217), (["pocketsphinx", "version"], "5.1.0"), (["pocketsphinx", "sha256"], "0" * 64 + "\n"), (["dictionary", "path"], "/synthetic/../dictionary"), (["model", "directory"], "relative"), (["language"], "EN")]
        for path, replacement in cases:
            self.assertFalse(self.validators["configuration"].is_valid(replace(configuration(), path, replacement)), path)
        value = configuration()
        value["model"]["files"].reverse()
        self.assertFalse(self.validators["configuration"].is_valid(value))
        for field in ["model", "dictionary", "pocketsphinx"]:
            value = configuration()
            value[field]["unknown"] = True
            self.assertFalse(self.validators["configuration"].is_valid(value), field)
        value = provider_configuration()
        value["reviewed_lyrics"]["id"] = "source_text"
        self.assertFalse(self.validators["provider_configuration"].is_valid(value))

    def test_candidate_partial_unmatched_ambiguous_shapes(self):
        for status in ["unmatched", "ambiguous"]:
            value = copy.deepcopy(self.report)
            value["result"]["observation"]["words"][0]["timing"] = {"status": status}
            self.validators["report"].validate(value)
            value["result"]["observation"]["words"][0]["timing"]["start"] = rational(0)
            self.assertFalse(self.validators["report"].is_valid(value))
        for status in ["partial", "unmatched", "ambiguous"]:
            value = copy.deepcopy(self.report)
            value["result"]["observation"]["cues"][0]["timing"] = {"status": status}
            self.validators["report"].validate(value)
        value = copy.deepcopy(self.report)
        value["result"]["observation"]["native_words"] = []
        self.validators["report"].validate(value)  # Rust checks matching candidate derivation.

    def test_timing_grid_and_token_bounds(self):
        for invalid in [rational(-1), rational(1, 1000), rational(1, 3), rational(601), rational(60001, 100), {"numerator": 0.5, "denominator": 1}]:
            value = replace(copy.deepcopy(self.report), ["result", "observation", "native_words", 0, "start"], invalid)
            self.assertFalse(self.validators["report"].is_valid(value), invalid)
        for path, replacement in [(["result", "observation", "words", 0, "token", "byte_start"], -1), (["result", "observation", "words", 0, "token", "normalized"], "HELLO"), (["source", "frame_count"], 9600001)]:
            self.assertFalse(self.validators["report"].is_valid(replace(copy.deepcopy(self.report), path, replacement)), path)

    def test_unavailable_outcomes_cannot_invent_candidates(self):
        for reason, source in [("unsupported_sample_rate", audio_source(48000)), ("unsupported_channels", audio_source(16000, 2)), ("silent_input", audio_source(silent=True))]:
            value = copy.deepcopy(self.report)
            value.update(source=source, scope={"channels": list(range(source["channels"])), "stem_id": None}, result={"status": "unavailable", "reason": reason}, raw_observation=None, timed_text=None, commands=[])
            self.validators["report"].validate(value)
            value["raw_observation"] = self.report["raw_observation"]
            self.assertFalse(self.validators["report"].is_valid(value))
        value = copy.deepcopy(self.report)
        value["timed_text"] = None
        self.assertFalse(self.validators["report"].is_valid(value))

    def test_preflight_readiness_and_typed_refusals(self):
        self.validators["preflight"].validate({"schema": "aniflow.audio-alignment-preflight/v1", "ready": True, "diagnostics": []})
        for code in ["missing_dictionary", "dictionary_digest_mismatch", "dictionary_size_mismatch", "unsupported_lexeme", "unsupported_language"]:
            value = preflight()
            value["diagnostics"][0]["code"] = code
            self.validators["preflight"].validate(value)
            value["ready"] = True
            self.assertFalse(self.validators["preflight"].is_valid(value))
        value = configuration()
        value["language"] = "fr"
        self.validators["configuration"].validate(value)  # Typed preflight refuses supported-shape unsupported language.

    def test_rust_owns_cross_field_semantic_validation(self):
        value = copy.deepcopy(self.report)
        value["result"]["observation"]["native_words"][0]["end"] = rational(4)  # Source is three seconds.
        self.validators["report"].validate(value)
        value["result"]["observation"]["words"][0]["token"]["byte_end"] = 1  # Exact slice differs.
        self.validators["report"].validate(value)
        value["timed_text"]["cues"][0]["text"] = "Different reviewed text"
        self.validators["report"].validate(value)
        value["reviewed_lyrics"]["document"]["provenance"]["source_sha256"] = "0" * 64
        self.validators["report"].validate(value)
        config = configuration()
        for resource in config["model"]["files"]:
            resource["byte_size"] = 67108864
        self.validators["configuration"].validate(config)  # Rust checks the 64 MiB total.

    def test_manifest_owns_exact_published_schema_bytes_and_ports(self):
        manifest = load(BUNDLE / "manifest.json")
        digest = hashlib.sha256(SCHEMAS["provider_configuration"].read_bytes()).hexdigest()
        self.assertEqual(manifest["configuration_schemas"][0]["sha256"], digest)
        capability = manifest["capabilities"][0]
        self.assertEqual(capability["configuration_schema"]["sha256"], digest)
        self.assertEqual([port["name"] for port in capability["inputs"]], ["audio", "technical", "upstream_analysis", "reviewed_lyrics"])
        self.assertEqual([port["name"] for port in capability["outputs"]], ["alignment", "analysis"])
        Draft202012Validator(load(CONTRACTS / "provider-manifest-v1.schema.json")).validate(manifest)

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
                if key in seen:
                    continue
                seen.add(key)
                walk(definitions[key])
            self.assertEqual(seen, set(definitions), name)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-examples", action="store_true")
    parser.add_argument("--alignment-report", type=Path, action="append", default=[])
    parser.add_argument("--preflight-report", type=Path, action="append", default=[])
    args, unittest_args = parser.parse_known_args()
    if args.write_examples:
        for name, builder in BUILDERS.items():
            EXAMPLES[name].write_text(json.dumps(builder(), indent=2, ensure_ascii=False, allow_nan=False) + "\n")
    for kind, paths in [("report", args.alignment_report), ("preflight", args.preflight_report)]:
        for path in paths:
            validators()[kind].validate(load(path))
            print(f"Validated alignment {kind}: {path}")
    unittest.main(argv=[__file__, *unittest_args])
