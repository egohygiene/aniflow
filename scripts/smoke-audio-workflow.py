#!/usr/bin/env python3
"""Reconcile audio contracts through one bounded synthetic workflow.

A real registered separator fixture generates three independent PCM profiles.
All selected-stem analyses retain the original mix and separation-run evidence.
Their clocks are deliberately different: duration-only lineage neither proves
content correspondence nor maps words, beats or notes onto the mix clock.
Native provider ABIs use synthetic executables and marker assets. Only local
ffmpeg/ffprobe perform real media inspection; no trained model is executed.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
from fractions import Fraction
import hashlib
import json
import math
import os
from pathlib import Path
import runpy
import signal
import struct
import subprocess
import sys
import tempfile
import time
import wave


ROOT = Path(__file__).resolve().parents[1]


def fixture(name):
    return runpy.run_path(str(Path(__file__).with_name(name)))


INSPECTION = fixture("smoke-audio-inspection.py")
STEM = fixture("smoke-audio-stem.py")
TRANSCRIPTION = fixture("smoke-audio-transcription.py")
ALIGNMENT = fixture("smoke-audio-alignment.py")
MIDI = fixture("smoke-audio-midi.py")
sha256 = INSPECTION["sha256"]
PROFILES = {"speech_profile": 16000, "midi_profile": 22050, "musical_profile": 44100}
SECONDS = 8

SEPARATOR = r'''
import json, math, pathlib, struct, sys, wave
assert sys.argv[1] == "--aniflow-invocation" and len(sys.argv) == 3
request = json.loads(pathlib.Path(sys.argv[2]).read_text())
with wave.open(request["inputs"][0]["path"], "rb") as source:
    assert source.getnframes() / source.getframerate() == 8
for output in request["outputs"]:
    target = pathlib.Path(output["path"])
    target.parent.mkdir(parents=True, exist_ok=True)
    if output["port"] == "evidence":
        target.write_text("Synthetic generated profiles; independent PCM generation, no separation inference, resampling or content/clock mapping proof.\n")
        continue
    rate = request["configuration"]["values"][output["port"]]
    frames = rate * 8
    samples = b"".join(struct.pack("<h", round(1200 * math.sin(2 * math.pi * 440 * index / rate))) for index in range(frames))
    with wave.open(str(target), "wb") as audio:
        audio.setparams((1, 2, rate, frames, "NONE", "not compressed"))
        audio.writeframes(samples)
'''

MUSICAL = r'''
import json, pathlib, sys, wave
root = pathlib.Path(__FIXTURE_ROOT__)
args = sys.argv[1:]
assert args[:2] == ["-I", "-B"]
if args[3:] == ["--probe"]:
    print((root / "probe.json").read_text())
    raise SystemExit(0)
assert args[3] == "--input" and len(args) == 5
with (root / "launches").open("a") as log: log.write(json.dumps(args) + "\n")
with wave.open(args[4], "rb") as audio:
    rate, channels, frames = audio.getframerate(), audio.getnchannels(), audio.getnframes()
print(json.dumps({
    "schema": "aniflow.audio-musical-observation/v1", "sample_rate_hz": rate,
    "channels": channels, "sample_frames": frames, "duration_seconds": frames / rate,
    "downmix": "arithmetic_average",
    "bpm": {"value": 120.0, "ticks_seconds": [index / 2 for index in range(1, 16)],
            "raw_confidence": 2.5, "estimates": [120.0, 60.0], "bpm_intervals": [0.5]},
    "key_profiles": [
        {"profile": "krumhansl", "key": "C", "scale": "major", "raw_strength": 0.75},
        {"profile": "temperley", "key": "A", "scale": "minor", "raw_strength": 0.6}]}))
'''


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def executable(path, body):
    path.write_text(f"#!{Path(sys.executable).resolve(strict=True)}\n" + body, encoding="utf-8")
    path.chmod(0o755)
    return path


def reference(path):
    return {"path": str(path), "sha256": sha256(path), "byte_size": path.stat().st_size}


def log_bytes(directory, name="launches"):
    path = directory / name
    return path.read_bytes() if path.exists() else b""


class Workflow:
    def __init__(self, binary, root):
        self.binary, self.root = binary, root
        self.documents = {}
        self.cases, self.runs, self.links = [], [], []
        self.declared_review_files = []
        self.schemas = {}
        schema_paths = list((ROOT / "docs/contracts").glob("*.schema.json")) + list((ROOT / "providers").rglob("*.schema.json"))
        for path in sorted(schema_paths):
            document = json.loads(path.read_text())
            tag = document.get("properties", {}).get("schema", {}).get("const")
            if tag:
                self.schemas[tag] = path

    def case(self, identifier, outcome="passed", **details):
        self.cases.append({"id": identifier, "outcome": outcome, "details": details})

    def document(self, identifier, path, schema_path=None):
        path = path.resolve(strict=True)
        if path in self.documents:
            return self.documents[path]["id"]
        item = {"id": identifier, **reference(path)}
        document = json.loads(path.read_text())
        if isinstance(document, dict) and "schema" in document:
            item["schema"] = document["schema"]
            schema_path = self.schemas.get(document["schema"], schema_path)
            digest_fields = {"aniflow.pipeline-plan/v1": "plan_sha256", "aniflow.pipeline-run/v1": "manifest_sha256",
                             "aniflow.provider-lock/v1": "lock_sha256", "aniflow.stage-checkpoint/v1": "checkpoint_sha256"}
            if document["schema"] in digest_fields:
                canonical = json.dumps(document["payload"], sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
                assert hashlib.sha256(canonical).hexdigest() == document[digest_fields[document["schema"]]], identifier
        if schema_path is not None:
            item["schema_path"] = str(schema_path)
            item["schema_sha256"] = sha256(schema_path)
        self.documents[path] = item
        return identifier

    def invoke(self, command, *arguments, success=True, name=None):
        result = subprocess.run([str(self.binary), "--output", "json", *map(str, arguments)],
                                capture_output=True, text=True, timeout=60, check=False)
        return self.decode(command, result.returncode, result.stdout, result.stderr, success, name)

    def decode(self, command, returncode, stdout, stderr, success, name):
        if (returncode == 0) != success:
            raise AssertionError(f"{command}: exit {returncode}\n{stdout}\n{stderr}")
        envelope = json.loads(stdout if success else stderr)
        assert envelope["schema_version"] == 1 and envelope["command"] == command, envelope
        assert envelope["status"] == ("success" if success else "error"), envelope
        self.last_envelope = envelope
        if name and not success:
            path = self.root / (name + "-error-envelope.json")
            write_json(path, envelope)
            self.document(name + "-error-envelope", path, ROOT / "docs/contracts/machine-envelope-v1.schema.json")
        result = envelope.get("result")
        if name and result is not None:
            path = self.root / (name + ".json")
            write_json(path, result)
            self.document(name, path)
        return result

    def run_evidence(self, identifier, outcome, stem=None, expected_state="complete"):
        run = Path(outcome["run_directory"])
        assert run.is_relative_to(self.root)
        manifest_path = Path(outcome["run_manifest"])
        manifest = json.loads(manifest_path.read_text())
        assert manifest["payload"]["plan_sha256"] == outcome["plan_sha256"]
        assert manifest["payload"]["state"] == expected_state
        snapshot = {str(path.relative_to(run)): sha256(path) for path in run.rglob("*") if path.is_file()}
        status = self.invoke("status_v3", "status-v3", run, name=identifier + "-status")
        assert status["payload"]["state"] == expected_state
        assert snapshot == {str(path.relative_to(run)): sha256(path) for path in run.rglob("*") if path.is_file()}
        files = {sha256(path): path for path in run.rglob("*") if path.is_file()}
        files.update({sha256(path): path for path in self.declared_review_files})
        if hasattr(self, "separation"):
            separation = Path(self.separation["run_directory"])
            files.update({sha256(path): path for path in separation.rglob("*") if path.is_file()})
        outputs = {}
        for item in outcome["outputs"]:
            path = Path(item["path"])
            assert path.is_relative_to(run) and sha256(path) == item["sha256"]
            outputs[item["id"]] = item
        for path in sorted(run.rglob("*.json")):
            self.document(identifier + "-" + str(path.relative_to(run)).replace("/", "-"), path)
        self.document(identifier + "-manifest", manifest_path)
        self.runs.append({"id": identifier, "run_directory": str(run), "plan_sha256": outcome["plan_sha256"],
                          "state": expected_state, "executed_stages": outcome["executed_stages"],
                          "reused_stages": outcome["reused_stages"], "outputs": outcome["outputs"]})
        if "analysis" not in outputs:
            return outputs, None
        analysis = json.loads(Path(outputs["analysis"]["path"]).read_text())
        assert analysis["source"]["origin"] == {"numerator": 0, "denominator": 1}
        if stem is not None:
            rate = PROFILES[stem]
            source = analysis["source"]
            assert source["sample_rate_hz"] == rate and source["frame_count"] == rate * SECONDS
            assert source["channels"] == 1 and source["artifact"]["sha256"] == self.stems[stem]["sha256"]
            assert source["stem"]["id"] == stem and source["stem"]["original_mix"]["sha256"] == sha256(self.mix)
            lineage = json.loads(Path(outputs["stem_lineage"]["path"]).read_text())["lineage"]
            assert lineage["selected_stem"] == source and lineage["timing_basis"] == "zero_origin_duration_only"
            assert lineage["source_plan_sha256"] == self.separation["plan_sha256"]
            assert lineage["original_mix"]["artifact"]["sha256"] == sha256(self.mix)
            assert Fraction(lineage["original_mix"]["frame_count"], lineage["original_mix"]["sample_rate_hz"]) == SECONDS
            assert Fraction(source["frame_count"], source["sample_rate_hz"]) == SECONDS
        artifact_ids = {item["id"] for item in analysis["artifacts"]}
        artifact_ids.add(analysis["source"]["artifact"]["id"])
        if analysis["source"]["stem"] is not None:
            artifact_ids.add(analysis["source"]["stem"]["original_mix"]["id"])
        provider_ids = {item["id"] for item in analysis["providers"]}
        for artifact in analysis["artifacts"]:
            assert artifact["sha256"] in files, (identifier, artifact)
            assert files[artifact["sha256"]].stat().st_size == artifact["byte_size"]
        for capability in analysis["capabilities"]:
            assert set(capability["evidence_artifact_ids"]) <= artifact_ids, (identifier, capability, artifact_ids)
            assert set(capability["provider_evidence_ids"]) <= provider_ids
        for observation in analysis["observations"]:
            provenance = observation["provenance"]
            assert provenance["provider_evidence_id"] in provider_ids
            assert set(provenance["evidence_artifact_ids"]) <= artifact_ids
            assert observation["scope"]["stem_id"] == stem
        for timeline in analysis["timelines"]:
            assert timeline["scope"]["stem_id"] == stem
            previous = -1
            for event in timeline["events"]:
                span = event["range"]
                assert 0 <= span["start"] < span["end"] <= analysis["source"]["frame_count"]
                assert span["start"] >= previous
                previous = span["end"] if timeline["kind"] == "disjoint_regions" else span["start"]
                assert event["provenance"]["provider_evidence_id"] in provider_ids
                assert set(event["provenance"]["evidence_artifact_ids"]) <= artifact_ids
        for excerpt in analysis["excerpts"]:
            assert excerpt["artifact_id"] in artifact_ids and set(excerpt["evidence_artifact_ids"]) <= artifact_ids
            assert excerpt["scope"]["stem_id"] == stem
            assert 0 <= excerpt["range"]["start"] < excerpt["range"]["end"] <= analysis["source"]["frame_count"]
        for semantic in analysis["semantic_artifacts"]:
            assert semantic["artifact_id"] in artifact_ids
            assert set(semantic["provenance"]["evidence_artifact_ids"]) <= artifact_ids
            assert semantic["provenance"]["provider_evidence_id"] in provider_ids
        # Report identities must bind actual upstream documents and their locks.
        locks = [json.loads(path.read_text()) for path in run.rglob("*.json")
                 if json.loads(path.read_text()).get("schema") == "aniflow.provider-lock/v1"]
        for key in ["signal", "musical", "transcription", "alignment", "midi"]:
            if key not in outputs:
                continue
            report_path = Path(outputs[key]["path"])
            report = json.loads(report_path.read_text())
            expected_source = copy.deepcopy(analysis["source"])
            if key == "signal":
                # This family measures first, then attaches verified lineage to
                # the final normalized document. Its unchanged companion owns
                # the physical stem bytes/clock without the later stem label.
                expected_source["stem"] = None
            assert report["source"] == expected_source
            upstream_name = "inspection_analysis_artifact" if key == "signal" else "upstream_analysis_artifact"
            for name in ["technical_artifact", upstream_name]:
                artifact = report[name]
                assert artifact["sha256"] in files and files[artifact["sha256"]].stat().st_size == artifact["byte_size"]
                self.links.append({"id": identifier + "-" + name, "from_document": self.documents[report_path.resolve()]["id"],
                                   "to_document": self.documents[files[artifact["sha256"]].resolve()]["id"],
                                   "relation": "digest_bound_upstream_evidence"})
            matching = [lock for lock in locks if lock["lock_sha256"] == report["provider_lock_sha256"]]
            assert len(matching) == 1
            lock = matching[0]["payload"]
            assert lock["provider"] == report["provider"]
            assert lock["effective_configuration_sha256"] == report["configuration_sha256"]
            assert lock["implementation"]["executable_sha256"] == report["implementation_sha256"]
        return outputs, analysis

    def exact_resume(self, identifier, outcome, arguments, logs):
        before = [log_bytes(directory, name) for directory, name in logs]
        resumed = self.invoke("audio_resume", "audio", "resume", outcome["run_directory"], *arguments,
                              name=identifier + "-resume")
        assert resumed["executed_stages"] == []
        assert set(resumed["reused_stages"]) == set(outcome["executed_stages"] + outcome["reused_stages"])
        assert resumed["outputs"] == outcome["outputs"]
        assert [log_bytes(directory, name) for directory, name in logs] == before
        self.case(identifier + "-exact-resume", reused_stages=resumed["reused_stages"], provider_relaunch=False)

    def shared(self, stem):
        return ["--input", self.mix, "--configuration", self.tools,
                "--stem-run", self.separation["run_directory"], "--stem-stage", "separate_profiles", "--stem-id", stem]


def prepare_profiles(workflow):
    root = workflow.root
    mix = root / "synthetic original café 雪.wav"
    with wave.open(str(mix), "wb") as audio:
        audio.setparams((1, 2, 48000, 48000 * SECONDS, "NONE", "not compressed"))
        audio.writeframes(b"".join(struct.pack("<h", round(1200 * math.sin(2 * math.pi * 440 * index / 48000)))
                                   for index in range(48000 * SECONDS)))
    workflow.mix = mix
    tools = root / "inspection.json"
    workflow.tool_pins = {name: INSPECTION["local_tool_pin"](name) for name in ["ffmpeg", "ffprobe"]}
    write_json(tools, {"schema": "aniflow.audio-inspection.configuration/v1", **workflow.tool_pins,
                       "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536})
    workflow.tools = tools
    bundle = root / "separator bundle"
    STEM["generic_bundle"](bundle, 48000, 0)
    manifest = json.loads((bundle / "manifest.json").read_text())
    capability = manifest["capabilities"][0]
    audio_port = capability["outputs"][0]
    evidence = capability["outputs"][2]
    capability["outputs"] = [{**copy.deepcopy(audio_port), "name": name} for name in PROFILES]
    capability["outputs"].append(evidence)
    schema_bytes = (json.dumps({"type": "object", "additionalProperties": False,
                               "properties": {name: {"const": rate} for name, rate in PROFILES.items()},
                               "required": list(PROFILES)}, sort_keys=True) + "\n").encode()
    (bundle / "configuration.schema.json").write_bytes(schema_bytes)
    schema = {"id": "aniflow.synthetic-profiles/v1", "version": "1.0.0", "sha256": hashlib.sha256(schema_bytes).hexdigest()}
    manifest["configuration_schemas"] = [schema]
    capability["configuration_schema"] = schema
    write_json(bundle / "manifest.json", manifest)
    configuration = json.loads((bundle / "configuration.json").read_text())
    configuration.update({"configuration_schema": schema, "values": PROFILES,
                          "effective_configuration_sha256": STEM["canonical_digest"](PROFILES)})
    write_json(bundle / "configuration.json", configuration)
    executable(bundle / "provider.py", SEPARATOR)
    pipeline = json.loads((bundle / "pipeline.yml").read_text())
    stage = pipeline["stages"][0]
    stage["id"] = "separate_profiles"
    bindings = [(name, name, "wav") for name in PROFILES] + [("evidence", "profile_evidence", "txt")]
    stage["outputs"] = [{"port": port, "artifacts": [{"id": name, "relative_path": f"artifacts/profiles/{name}.{suffix}", "kind": "file"}]}
                        for port, name, suffix in bindings]
    stage["validations"] = [{"id": name + "_integrity", "artifact": name,
                              "contract": "aniflow.validation/artifact-integrity/v1"} for _, name, _ in bindings]
    pipeline["outputs"] = [{"id": name, "artifact": name, "required_validations": [name + "_integrity"]} for _, name, _ in bindings]
    write_json(bundle / "pipeline.yml", pipeline)
    arguments = ["--pipeline", bundle / "pipeline.yml", "--input", "source_audio=" + str(mix),
                 "--provider-registration", bundle / "registration.json", "--host-cpu-threads", "2",
                 "--host-memory-mib", "16384", "--host-storage-mib", "16384", "--offline"]
    for effect in ["filesystem-read", "filesystem-write", "environment-read", "subprocess", "ai"]:
        arguments.extend(["--allow-side-effect", effect])
    workflow.separation = workflow.invoke("run_v3", "run-v3", *arguments,
                                         "--output-directory", root / "separation runs", name="profiles-outcome")
    outputs, _ = workflow.run_evidence("profiles", workflow.separation)
    workflow.stems = outputs
    workflow.case("audio-workflow-profiles-lineage", generation="independent_synthetic_pcm_no_resampling",
                  timing_basis="zero_origin_duration_only", content_correspondence_proven=False)


def prepare_transcription(root):
    root.mkdir()
    # Add bounded failure/interruption controls to the existing fixture ABI.
    body = TRANSCRIPTION["WHISPER"].replace("__FIXTURE_ROOT__", json.dumps(str(root)))
    body = body.replace("import json, pathlib, sys", "import json, pathlib, sys, time")
    body = body.replace("segments = [", """mode = (root / \"mode\").read_text()
if mode == \"nonzero\": raise SystemExit(23)
if mode == \"sleep\":
    (root / \"provider-started\").write_text(\"yes\")
    time.sleep(120)
segments = [""", 1)
    whisper = executable(root / "synthetic-whisper", body)
    model = root / "synthetic-tiny.en.bin"
    model.write_bytes(b"Synthetic model marker; no trained inference weights.\n")
    configuration = {"schema": "aniflow.audio-transcription.configuration/v1",
                     "whisper": {"executable": str(whisper), "version": "1.8.7", "sha256": sha256(whisper)},
                     "model": {"path": str(model), "sha256": sha256(model), "byte_size": model.stat().st_size,
                               "model_id": "tiny.en", "revision": "synthetic-workflow-v1"},
                     "language": "en", "threads": 1, "tool_timeout_milliseconds": 30000,
                     "maximum_tool_output_bytes": 65536}
    settings = root / "transcription.json"
    write_json(settings, configuration)
    (root / "mode").write_text("ok")
    return settings, model, whisper


def prepare_alignment(root):
    root.mkdir()
    native = executable(root / "synthetic-pocketsphinx", ALIGNMENT["POCKETSPHINX"].replace("__FIXTURE_ROOT__", json.dumps(str(root))))
    model = root / "synthetic-en-us"
    model.mkdir()
    resources = []
    for name in ALIGNMENT["MODEL_FILES"]:
        path = model / name
        path.write_text({"feat.params": ALIGNMENT["FEATURE_PARAMETERS"], "noisedict": ALIGNMENT["NOISE_DICTIONARY"]}.get(
            name, f"Synthetic acoustic resource {name}; no inference weights.\n"))
        resources.append({"name": name, "sha256": sha256(path), "byte_size": path.stat().st_size})
    dictionary = root / "synthetic-dictionary.dict"
    dictionary.write_text("hello HH AH L OW\nsynthetic S IH N TH EH T IH K\nworld W ER L D\n")
    settings = root / "alignment.json"
    write_json(settings, {"schema": "aniflow.audio-alignment.configuration/v1",
                         "pocketsphinx": {"executable": str(native), "version": "5.1.1", "sha256": sha256(native)},
                         "model": {"directory": str(model), "model_id": "en-us", "revision": "synthetic-workflow-v1", "files": resources},
                         "dictionary": {"path": str(dictionary), "sha256": sha256(dictionary), "byte_size": dictionary.stat().st_size},
                         "language": "en", "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536})
    lyrics = root / "reviewed lyrics.json"
    write_json(lyrics, ALIGNMENT["reviewed_document"](["Hello, synthetic!", "World!"]))
    (root / "declared source text.txt").write_text("Hello, synthetic!\nWorld!")
    (root / "declared review evidence.txt").write_bytes(b"Synthetic review declaration; no human review is asserted.\n")
    (root / "mode").write_text("complete")
    (root / "phrase").write_text("hello synthetic world")
    return settings, lyrics


def prepare_musical(root):
    root.mkdir()
    python = executable(root / "synthetic-python", MUSICAL.replace("__FIXTURE_ROOT__", json.dumps(str(root))))
    adapter = root / "adapter.py"
    adapter.write_text("Synthetic adapter marker; fixture ABI supplies declared estimates.\n")
    probe = copy.deepcopy(json.loads((ROOT / "docs/contracts/examples/audio-musical-analysis-v1.example.json").read_text())["probe"])
    probe.update({"python_version": "3.11.0", "runtime_sha256": hashlib.sha256(b"synthetic workflow dependency inventory").hexdigest(),
                  "runtime_file_count": 1, "runtime_byte_count": 64})
    write_json(root / "probe.json", probe)
    settings = root / "musical.json"
    write_json(settings, {"schema": "aniflow.audio-musical.configuration/v1",
                         "python": {"executable": str(python), "version": probe["python_version"], "sha256": sha256(python)},
                         "adapter": {"path": str(adapter), "sha256": sha256(adapter)},
                         "runtime": {"sha256": probe["runtime_sha256"], "file_count": 1, "byte_count": 64},
                         "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536})
    return settings


def prepare_midi(root):
    root.mkdir()
    body = MIDI["SYNTHETIC_PYTHON"].replace("__FIXTURE_ROOT__", repr(str(root))).replace(
        "__ADAPTER_MARKER__", repr(MIDI["ADAPTER_MARKER"])).replace("__MODEL_MARKER__", repr(MIDI["MODEL_MARKER"])).replace(
        "__MODEL_BYTE_SIZE__", str(MIDI["MODEL_BYTE_SIZE"]))
    body = body.replace("getnframes() == 44100", "getnframes() == 176400").replace(
        '"sample_frames": 44100, "duration_microseconds": 2000000', '"sample_frames": 176400, "duration_microseconds": 8000000')
    python = executable(root / "synthetic-python", body)
    adapter = root / "adapter.py"
    adapter.write_text(MIDI["ADAPTER_MARKER"])
    model = root / "synthetic-model.onnx"
    model.write_bytes(MIDI["MODEL_MARKER"] + b"\0" * (MIDI["MODEL_BYTE_SIZE"] - len(MIDI["MODEL_MARKER"])))
    write_json(root / "probe.json", MIDI["PROBE"])
    (root / "mode").write_text("monophonic")
    settings = root / "midi.json"
    write_json(settings, {"schema": "aniflow.audio-midi.configuration/v1",
                         "python": {"executable": str(python), "version": MIDI["PROBE"]["python_version"], "sha256": sha256(python)},
                         "adapter": {"path": str(adapter), "sha256": sha256(adapter)},
                         "runtime": {"sha256": MIDI["RUNTIME_SHA256"], "file_count": 1, "byte_count": 100},
                         "model": {"path": str(model), "sha256": sha256(model), "byte_size": model.stat().st_size,
                                   "model_id": "basic-pitch-onnx-icassp-2022", "revision": MIDI["BASIC_PITCH_REVISION"]},
                         "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536})
    return settings, model


def exercise(binary, root, receipt):
    workflow = Workflow(binary, root)
    prepare_profiles(workflow)
    transcription_root, alignment_root = root / "transcription fixture", root / "alignment fixture"
    musical_root, midi_root = root / "musical fixture", root / "midi fixture"
    transcription, transcription_model, whisper = prepare_transcription(transcription_root)
    alignment, lyrics = prepare_alignment(alignment_root)
    workflow.declared_review_files = [alignment_root / "declared source text.txt", alignment_root / "declared review evidence.txt", lyrics]
    reviewed = json.loads(lyrics.read_text())
    assert reviewed["source"]["sha256"] == sha256(workflow.declared_review_files[0])
    assert reviewed["provenance"]["evidence"]["sha256"] == sha256(workflow.declared_review_files[1])
    musical = prepare_musical(musical_root)
    midi, midi_model = prepare_midi(midi_root)
    signal_settings = root / "signal.json"
    write_json(signal_settings, {"schema": "aniflow.audio-signal.configuration/v1", "silence_threshold_pcm": 32,
                                "minimum_silence_milliseconds": 100, "clipping_threshold_pcm": 32767})
    immutable_paths = [workflow.mix, workflow.tools, transcription, transcription_model, whisper,
                       alignment, lyrics, musical, midi, midi_model] + [Path(item["path"]) for item in workflow.stems.values()]
    immutable_paths += workflow.declared_review_files + [signal_settings, alignment_root / "synthetic-pocketsphinx",
                         alignment_root / "synthetic-dictionary.dict"] + list((alignment_root / "synthetic-en-us").iterdir())
    for directory in [musical_root, midi_root]:
        immutable_paths += [directory / "synthetic-python", directory / "adapter.py", directory / "probe.json"]
    immutable_paths += list((root / "separator bundle").iterdir())
    immutable = {path: path.read_bytes() for path in immutable_paths}
    requests = {
        "signal": ("speech_profile", ["--analysis", "signal", "--signal-configuration", signal_settings], ["audio", "analyze"], "audio_analyze", []),
        "transcription": ("speech_profile", ["--analysis", "transcription", "--transcription-configuration", transcription], ["audio", "transcribe"], "audio_transcribe", [(transcription_root, "launches")]),
        "alignment": ("speech_profile", ["--analysis", "lyrics-alignment", "--alignment-configuration", alignment, "--lyrics", lyrics], ["audio", "lyrics", "align"], "audio_lyrics_align", [(alignment_root, "launches")]),
        "musical": ("musical_profile", ["--analysis", "musical", "--musical-configuration", musical], ["audio", "analyze"], "audio_analyze", [(musical_root, "launches")]),
        "midi": ("midi_profile", ["--analysis", "midi-candidates", "--midi-configuration", midi], ["audio", "midi", "extract"], "audio_midi_extract", [(midi_root, "inference-launches")]),
    }
    outcomes, analyses, outputs = {}, {}, {}
    for name, (stem, selection, command, command_id, logs) in requests.items():
        shared = workflow.shared(stem)
        before = [log_bytes(directory, log) for directory, log in logs]
        planned = workflow.invoke("audio_plan", "audio", "plan", *shared, *selection, name=name + "-plan")
        assert planned["payload"]["policy"]["offline"] and "network" not in planned["payload"]["policy"]["allowed_side_effects"]
        assert [log_bytes(directory, log) for directory, log in logs] == before
        # Specialized canonical commands imply their analysis selector.
        run_selection = selection if name in ["signal", "musical"] else selection[2:]
        outcome = workflow.invoke(command_id, *command, *shared, *run_selection,
                                  "--output-directory", root / (name + " runs"), name=name + "-outcome")
        assert outcome["plan_sha256"] == planned["plan_sha256"]
        current_outputs, analysis = workflow.run_evidence(name, outcome, stem)
        assert analysis["status"] == "complete" or name == "signal"
        outcomes[name], analyses[name], outputs[name] = outcome, analysis, current_outputs
        workflow.exact_resume("audio-workflow-" + name, outcome, [*shared, *selection], logs)
        workflow.case("audio-workflow-" + name + "-linked", normalized_status=analysis["status"], source_sha256=analysis["source"]["artifact"]["sha256"])
    speech_source = analyses["transcription"]["source"]
    assert speech_source == analyses["alignment"]["source"] == analyses["signal"]["source"]
    technical_hashes = {outputs[name]["technical"]["sha256"] for name in ["signal", "transcription", "alignment"]}
    assert len(technical_hashes) == 1
    transcript_report = json.loads(Path(outputs["transcription"]["transcription"]["path"]).read_text())
    alignment_report = json.loads(Path(outputs["alignment"]["alignment"]["path"]).read_text())
    musical_report = json.loads(Path(outputs["musical"]["musical"]["path"]).read_text())
    midi_report = json.loads(Path(outputs["midi"]["midi"]["path"]).read_text())
    signal_report = json.loads(Path(outputs["signal"]["signal"]["path"]).read_text())
    assert signal_report["tools"] == [{"id": name, "version": pin["version"], "sha256": pin["sha256"]}
                                      for name, pin in workflow.tool_pins.items()]
    assert transcript_report["settings"]["whisper_sha256"] == sha256(whisper)
    assert transcript_report["settings"]["model"]["sha256"] == sha256(transcription_model)
    assert alignment_report["settings"]["pocketsphinx_sha256"] == sha256(alignment_root / "synthetic-pocketsphinx")
    assert alignment_report["settings"]["dictionary"]["sha256"] == sha256(alignment_root / "synthetic-dictionary.dict")
    for resource in alignment_report["settings"]["model"]["files"]:
        path = alignment_root / "synthetic-en-us" / resource["name"]
        assert resource["sha256"] == sha256(path) and resource["byte_size"] == path.stat().st_size
    for report, directory in [(musical_report, musical_root), (midi_report, midi_root)]:
        assert report["settings"]["python_sha256"] == sha256(directory / "synthetic-python")
        assert report["settings"]["adapter_sha256"] == sha256(directory / "adapter.py")
        assert report["settings"]["runtime"]["sha256"] == report["probe"]["runtime_sha256"]
    assert midi_report["settings"]["model"]["sha256"] == sha256(midi_model)
    assert transcript_report["timed_text"]["provenance"]["kind"] == "observed_transcript"
    transcript_timeline = next(item for item in analyses["transcription"]["timelines"] if item["id"] == "transcription-segments")
    for cue, event in zip(transcript_report["timed_text"]["cues"], transcript_timeline["events"], strict=True):
        timing = cue["timing"]
        assert event["range"] == {boundary: int(Fraction(timing[boundary]["numerator"], timing[boundary]["denominator"]) * 16000)
                                  for boundary in ["start", "end"]}
    assert alignment_report["timed_text"]["provenance"]["kind"] == "reviewed_lyrics"
    assert not alignment_report["method"]["review_attestation_independently_verified"]
    assert alignment_report["reviewed_lyrics"]["artifact"]["sha256"] == sha256(lyrics)
    assert [(cue["id"], cue["text"]) for cue in alignment_report["timed_text"]["cues"]] == [
        (cue["id"], cue["text"]) for cue in json.loads(lyrics.read_text())["cues"]]
    assert musical_report["provenance"] == "heuristic" and midi_report["provenance"] == "probabilistic"
    assert all(item["authority"] is None for item in analyses["transcription"]["semantic_artifacts"] + analyses["midi"]["semantic_artifacts"])
    assert any(item["kind"] == "reviewed_lyrics" and item["artifact_id"] == "reviewed_lyrics" and item["authority"] is not None
               for item in analyses["alignment"]["semantic_artifacts"])
    for analysis in analyses.values():
        excerpt = next(item for item in analysis["excerpts"] if item["id"] == "source-span")
        assert excerpt["artifact_id"] == analysis["source"]["artifact"]["id"]
        assert excerpt["range"] == {"start": 0, "end": analysis["source"]["frame_count"]}
    for report in [transcript_report, alignment_report, musical_report, midi_report]:
        assert report["confidence"]["kind"] == "unavailable"
    beats = musical_report["result"]["beats"]
    assert all(0 <= beat["source_frame"] < 44100 * SECONDS and beat["source_frame"] == math.floor(beat["seconds"] * 44100 + 0.5) for beat in beats)
    assert all(left["source_frame"] < right["source_frame"] for left, right in zip(beats, beats[1:]))
    workflow.case("audio-workflow-distinct-clocks-authority", shared_speech_technical_sha256=next(iter(technical_hashes)),
                  content_correspondence_proven=False, mix_alignment_mapping_proven=False,
                  observed_text_grants_reviewed_authority=False, source_span_excerpt_references_verified=True,
                  rendered_excerpt_artifacts_generated=False)

    for name, report_id, command, expected_losses in [
        ("transcription", "transcription", ["audio", "transcript-export"], {"language"}),
        ("alignment", "alignment", ["audio", "lyrics", "export"], {"language", "metadata"}),
    ]:
        report_path = Path(outputs[name][report_id]["path"])
        destination = root / (name + " export")
        command_id = "audio_transcript_export" if name == "transcription" else "audio_lyrics_export"
        argument = "--transcription" if name == "transcription" else "--alignment"
        shared = [argument, report_path, "--to", "webvtt", "--output-directory", destination]
        refused = workflow.invoke(command_id, *command, *shared, success=False, name=name + "-loss-refusal")
        assert {loss["kind"] for loss in refused["losses"]} == expected_losses and not destination.exists()
        exported = workflow.invoke(command_id, *command, *shared, "--allow-loss", ",".join(sorted(expected_losses)), name=name + "-export-outcome")
        bound_report = exported["transcription_report" if name == "transcription" else "alignment_report"]
        assert bound_report["sha256"] == sha256(report_path)
        conversion = json.loads((destination / "conversion.json").read_text())
        assert conversion["input_document_sha256"] == sha256(destination / "normalized-input.json")
        assert conversion["output_document_sha256"] == sha256(destination / "normalized-output.json")
        assert conversion["output"]["sha256"] == sha256(destination / "payload.vtt")
        for path in destination.glob("*.json"):
            workflow.document(name + "-export-" + path.name, path)
        converted = json.loads((destination / "normalized-output.json").read_text())
        assert converted["provenance"] == (transcript_report if name == "transcription" else alignment_report)["timed_text"]["provenance"]
        workflow.case("audio-workflow-" + name + "-explicit-export-loss", losses=sorted(expected_losses), authority_preserved=True)

    midi_export = root / "midi export"
    workflow.invoke("audio_midi_export", "audio", "midi", "export", "--candidate", outputs["midi"]["midi"]["path"],
                    "--output-directory", midi_export, name="midi-export-outcome")
    notes = midi_report["result"]["observation"]["notes"]
    assert MIDI["read_midi"](midi_export / "candidate.mid") == sorted((note["start_tick"], note["end_tick"], note["pitch"], note["velocity"]) for note in notes)
    export_report = json.loads((midi_export / "export-report.json").read_text())
    assert export_report["source_report"]["sha256"] == outputs["midi"]["midi"]["sha256"]
    assert export_report["source_audio"] == midi_report["source"]["artifact"]
    assert export_report["notes"]["sha256"] == sha256(midi_export / "notes.json")
    assert export_report["midi"]["sha256"] == sha256(midi_export / "candidate.mid")
    for path in midi_export.glob("*.json"):
        workflow.document("midi-export-" + path.name, path, ROOT / "docs/contracts/audio-midi-notes-v1.schema.json" if path.name == "notes.json" else None)
    workflow.case("audio-workflow-midi-export-readback", note_count=len(notes), source_tempo_inferred=False, source_instrument_inferred=False)

    # A missing optional provider refuses planning before any run or inference.
    native_bytes = whisper.read_bytes()
    whisper.unlink()
    run_plans = set(root.rglob("plan/plan.json"))
    launches = log_bytes(transcription_root)
    refused = workflow.invoke("audio_plan", "audio", "plan", *workflow.shared("speech_profile"), *requests["transcription"][1],
                              success=False, name="missing-provider-preflight")
    assert any(item["code"] == "missing_tool" for item in refused["diagnostics"])
    assert log_bytes(transcription_root) == launches and set(root.rglob("plan/plan.json")) == run_plans
    executable(whisper, native_bytes.decode().split("\n", 1)[1])
    assert whisper.read_bytes() == native_bytes
    workflow.case("audio-workflow-missing-provider", outcome="unavailable", run_created=False)

    # Content partial/unavailable is distinct from a failed runtime.
    (alignment_root / "mode").write_text("partial")
    partial = workflow.invoke("audio_lyrics_align", "audio", "lyrics", "align", *workflow.shared("speech_profile"),
                              *requests["alignment"][1][2:], "--output-directory", root / "partial alignment runs", name="partial-alignment-outcome")
    partial_outputs, partial_analysis = workflow.run_evidence("partial-alignment", partial, "speech_profile")
    assert partial_analysis["status"] == "partial"
    partial_report = json.loads(Path(partial_outputs["alignment"]["path"]).read_text())
    assert [word["timing"]["status"] for word in partial_report["result"]["observation"]["words"]] == ["candidate", "unmatched", "candidate"]
    partial_export = root / "partial alignment export"
    workflow.invoke("audio_lyrics_export", "audio", "lyrics", "export", "--alignment", partial_outputs["alignment"]["path"],
                    "--to", "webvtt", "--output-directory", partial_export, "--allow-loss", "language,metadata", success=False)
    assert not partial_export.exists()
    (alignment_root / "mode").write_text("complete")
    workflow.case("audio-workflow-partial-alignment", outcome="partial", runtime_state="complete", untimed_words_preserved=True)
    # Reuse the accepted 16 kHz stem in the musical family: no implicit resample.
    unavailable = workflow.invoke("audio_analyze", "audio", "analyze", *workflow.shared("speech_profile"),
                                  *requests["musical"][1], "--output-directory", root / "unavailable musical runs", name="unavailable-musical-outcome")
    unavailable_outputs, unavailable_analysis = workflow.run_evidence("unavailable-musical", unavailable, "speech_profile")
    unavailable_report = json.loads(Path(unavailable_outputs["musical"]["path"]).read_text())
    assert unavailable_report["result"] == {"status": "unavailable", "reason": "unsupported_sample_rate"}
    assert unavailable_analysis["status"] == "partial"
    workflow.case("audio-workflow-unsupported-profile", outcome="unavailable", normalized_status="partial", runtime_state="complete", resampled=False)
    (transcription_root / "mode").write_text("empty")
    empty = workflow.invoke("audio_transcribe", "audio", "transcribe", *workflow.shared("speech_profile"),
                            *requests["transcription"][1][2:], "--output-directory", root / "empty transcription runs", name="empty-transcription-outcome")
    empty_outputs, _ = workflow.run_evidence("empty-transcription", empty, "speech_profile")
    empty_report = json.loads(Path(empty_outputs["transcription"]["path"]).read_text())
    assert empty_report["result"]["status"] == "empty" and empty_report["timed_text"] is None
    workflow.case("audio-workflow-empty-transcript", runtime_state="complete", invented_text=False)

    for mode, expected_state in [("nonzero", "failed"), ("sleep", "cancelled")]:
        (transcription_root / "mode").write_text(mode)
        destination = root / (mode + " transcription runs")
        arguments = ["audio", "transcribe", *workflow.shared("speech_profile"), *requests["transcription"][1][2:], "--output-directory", destination]
        if mode == "nonzero":
            workflow.invoke("audio_transcribe", *arguments, success=False, name="nonzero-provider-error")
            assert workflow.last_envelope["error"]["category"] == "execution"
        else:
            process = subprocess.Popen([str(binary), "--output", "json", *map(str, arguments)],
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
            deadline = time.monotonic() + 30
            marker = transcription_root / "provider-started"
            try:
                while not marker.exists() and process.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.01)
                assert marker.exists(), "provider did not reach bounded interruption marker"
                process.send_signal(signal.SIGINT)
                stdout, stderr = process.communicate(timeout=15)
                workflow.decode("audio_transcribe", process.returncode, stdout, stderr, False, "interrupted-error-result")
            finally:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.communicate()
        directories = [path for path in destination.iterdir() if path.is_dir()]
        assert len(directories) == 1
        run = directories[0]
        status = workflow.invoke("status_v3", "status-v3", run, name=mode + "-status")
        assert status["payload"]["state"] == expected_state
        stage = next(item for item in status["payload"]["stages"] if item["stage_id"] == "inspect_audio")
        assert stage["checkpoint"] is not None
        assert not list(run.glob("**/artifacts/audio-transcription/transcription.json"))
        workflow.case("audio-workflow-" + expected_state + "-provider", outcome=expected_state, technical_checkpoint_retained=True, final_report_published=False)
        (transcription_root / "mode").write_text("ok")
        resumed = workflow.invoke("audio_resume", "audio", "resume", run, *workflow.shared("speech_profile"), *requests["transcription"][1], name=mode + "-recovery-outcome")
        assert resumed["executed_stages"] == ["analyze_transcription"]
        assert set(resumed["reused_stages"]) == {"inspect_audio", "attach_stem_lineage"}
        workflow.run_evidence(mode + "-recovered", resumed, "speech_profile")
        workflow.case("audio-workflow-" + expected_state + "-compatible-resume", reused_stages=resumed["reused_stages"], executed_stages=resumed["executed_stages"])
        workflow.exact_resume("audio-workflow-" + expected_state + "-recovered", resumed,
                              [*workflow.shared("speech_profile"), *requests["transcription"][1]], [(transcription_root, "launches")])

    launches = log_bytes(midi_root, "inference-launches")
    model_bytes = midi_model.read_bytes()
    midi_model.write_bytes(b"X" + model_bytes[1:])
    refused = workflow.invoke("audio_resume", "audio", "resume", outcomes["midi"]["run_directory"],
                              *workflow.shared("midi_profile"), *requests["midi"][1], success=False, name="stale-model-resume")
    assert any(item["code"] == "model_digest_mismatch" for item in refused["diagnostics"])
    assert log_bytes(midi_root, "inference-launches") == launches
    midi_model.write_bytes(model_bytes)
    workflow.case("audio-workflow-stale-model-resume-refusal", provider_relaunch=False)
    launches = log_bytes(alignment_root)
    lyrics_bytes = lyrics.read_bytes()
    changed = json.loads(lyrics_bytes)
    changed["metadata"]["review_revision"] = "synthetic-r2"
    write_json(lyrics, changed)
    workflow.invoke("audio_resume", "audio", "resume", outcomes["alignment"]["run_directory"],
                    *workflow.shared("speech_profile"), *requests["alignment"][1], success=False, name="stale-review-resume")
    error = workflow.last_envelope["error"]
    assert error["category"] == "state" and "differ from the saved run" in error["message"], error
    assert log_bytes(alignment_root) == launches
    lyrics.write_bytes(lyrics_bytes)
    workflow.case("audio-workflow-stale-review-resume-refusal", provider_relaunch=False, review_authority_not_inferred=True)
    assert all(path.read_bytes() == content for path, content in immutable.items())
    assert all(sha256(Path(pin["executable"])) == pin["sha256"] for pin in workflow.tool_pins.values())
    workflow.case("audio-workflow-sources-unchanged", source_and_declared_inputs_unchanged=True)
    for path in [workflow.tools, transcription, alignment, lyrics, musical, midi, signal_settings]:
        workflow.document("configuration-" + path.parent.name + "-" + path.name, path)
    result = {"schema": "aniflow.audio-workflow-smoke/v1", "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "synthetic_provider": True, "real_model_inference": False, "models_downloaded": False,
              "real_inspection_tools": True, "aniflow": reference(binary),
              "source": {**reference(workflow.mix), "sample_rate_hz": 48000, "channels": 1, "frame_count": 48000 * SECONDS},
              "profiles": [{"id": stem, "sample_rate_hz": rate, "channels": 1, "frame_count": rate * SECONDS,
                            "source_sha256": workflow.stems[stem]["sha256"], "timing_basis": "zero_origin_duration_only",
                            "generation": "independent_synthetic_pcm_no_resampling"} for stem, rate in PROFILES.items()],
              "cases": workflow.cases, "tools": workflow.tool_pins, "documents": list(workflow.documents.values()),
              "runs": workflow.runs, "links": workflow.links, "sources_unchanged": True}
    if receipt:
        write_json(receipt, result)
    print(json.dumps({"synthetic_audio_workflow_cases": len(workflow.cases), "documents": len(workflow.documents),
                      "runs": len(workflow.runs), "receipt": str(receipt) if receipt else None}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    binary = arguments.aniflow.resolve(strict=True)
    if arguments.receipt:
        receipt = arguments.receipt.resolve()
        receipt.parent.mkdir(parents=True, exist_ok=True)
        # Retain exact run-local evidence and its referenced outputs for review.
        root = Path(tempfile.mkdtemp(prefix="audio-workflow-café-", dir=receipt.parent)).resolve()
        exercise(binary, root, receipt)
    else:
        with tempfile.TemporaryDirectory(prefix="aniflow audio workflow café ") as temporary:
            exercise(binary, Path(temporary).resolve(), None)


if __name__ == "__main__":
    main()
