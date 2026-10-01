#!/usr/bin/env python3
"""Exercise MIDI candidates with generated PCM and a synthetic local adapter ABI.

Real ffmpeg/ffprobe inspect generated tones. A marker model and fixture executable
prove contracts only: no Basic Pitch runtime, trained model or native inference
is installed, downloaded or executed, and no musical accuracy is established.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import struct
import subprocess
import sys
import tempfile
import wave


INSPECTION = runpy.run_path(str(Path(__file__).with_name("smoke-audio-inspection.py")))
sha256 = INSPECTION["sha256"]
local_tool_pin = INSPECTION["local_tool_pin"]
BASIC_PITCH_REVISION = "9991303bba609a3b93089d13ec80d1d495083596"
MODEL_BYTE_SIZE = 230444
ADAPTER_MARKER = "Synthetic MIDI adapter identity; never executed as an analyzer.\n"
MODEL_MARKER = b"Synthetic MIDI model identity; not inference weights.\n"
RUNTIME_SHA256 = hashlib.sha256(b"synthetic installed-distribution inventory").hexdigest()
PROBE = {
    "schema": "aniflow.audio-midi-probe/v1",
    "python_version": "3.11.9", "basic_pitch_version": "0.4.0",
    "basic_pitch_revision": BASIC_PITCH_REVISION,
    "onnxruntime_version": "1.20.1", "numpy_version": "1.26.4",
    "librosa_version": "0.10.2.post1", "scipy_version": "1.13.1",
    "pretty_midi_version": "0.2.10", "runtime_sha256": RUNTIME_SHA256,
    "runtime_file_count": 1, "runtime_byte_count": 100,
    "license": {
        "basic_pitch_expression": "Apache-2.0",
        "basic_pitch_metadata_sha256": hashlib.sha256(b"synthetic Basic Pitch license metadata").hexdigest(),
        "onnxruntime_expression": "MIT",
        "onnxruntime_metadata_sha256": hashlib.sha256(b"synthetic ONNX Runtime license metadata").hexdigest(),
    },
}
SYNTHETIC_PYTHON = r'''
import json, pathlib, sys, wave
root = pathlib.Path(__FIXTURE_ROOT__)
args = sys.argv[1:]
stage = pathlib.Path.cwd()
assert args[:3] == ["-I", "-B", str(stage / "adapter.py")]
assert (stage / "adapter.py").read_text() == __ADAPTER_MARKER__
mode = (root / "mode").read_text()
with (root / "all-launches").open("a") as log:
    log.write(json.dumps(args) + "\n")
if args[3:] == ["--probe"]:
    probe = json.loads((root / "probe.json").read_text())
    if mode == "runtime-drift": probe["runtime_sha256"] = "f" * 64
    print(json.dumps(probe))
    raise SystemExit(0)
assert args[3:] == ["--input", str(stage / "source.wav"), "--model", str(stage / "model.onnx")]
assert (stage / "model.onnx").read_bytes() == __MODEL_MARKER__ + b"\0" * (__MODEL_BYTE_SIZE__ - len(__MODEL_MARKER__))
with wave.open(str(stage / "source.wav"), "rb") as source:
    assert source.getframerate() == 22050 and source.getnchannels() == 1
    assert source.getsampwidth() == 2 and source.getnframes() == 44100
with (root / "inference-launches").open("a") as log:
    log.write(json.dumps(args) + "\n")
notes = [
    {"start_microseconds": 100000, "end_microseconds": 500000, "pitch": 60, "velocity": 102, "activation": 0.8},
    {"start_microseconds": 750000, "end_microseconds": 1250000, "pitch": 64, "velocity": 76, "activation": 0.6},
]
if mode == "polyphony":
    notes.insert(1, {"start_microseconds": 200000, "end_microseconds": 700000, "pitch": 67, "velocity": 89, "activation": 0.7})
if mode == "empty": notes = []
observation = {"schema": "aniflow.audio-midi-observation/v1", "sample_rate_hz": 22050,
               "channels": 1, "sample_frames": 44100, "duration_microseconds": 2000000, "notes": notes}
if mode == "malformed": observation["unexpected"] = True
print(json.dumps(observation))
'''


def invoke(binary, command, *arguments, success=True):
    result = subprocess.run(
        [str(binary), "--output", "json", *map(str, arguments)],
        capture_output=True, text=True, timeout=60, check=False,
    )
    if (result.returncode == 0) != success:
        raise AssertionError(f"{command}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
    envelope = json.loads(result.stdout if success else result.stderr)
    assert envelope["schema_version"] == 1 and envelope["command"] == command
    assert envelope["status"] == ("success" if success else "error")
    return envelope.get("result")


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def pcm(path, rate=22050, channels=1, silence=False):
    samples = b"".join(
        struct.pack("<h", 0 if silence else (1200 if index % 64 < 32 else -1200)) * channels
        for index in range(rate * 2)
    )
    with wave.open(str(path), "wb") as audio:
        audio.setparams((channels, 2, rate, rate * 2, "NONE", "not compressed"))
        audio.writeframes(samples)


def read_midi(path):
    """Independent strict read-back for the explicitly exported type-0 subset."""
    content = path.read_bytes()
    assert content[:4] == b"MThd" and len(content) <= 1024 * 1024
    assert struct.unpack(">IHHH", content[4:14]) == (6, 0, 1, 960)
    assert content[14:18] == b"MTrk"
    track_size = struct.unpack(">I", content[18:22])[0]
    assert len(content) == 22 + track_size
    track = content[22:]
    cursor = 0
    tick = 0
    active = {}
    notes = []
    ended = False
    tempos = []
    programs = []

    def variable_length():
        nonlocal cursor
        value = 0
        for index in range(4):
            assert cursor < len(track)
            byte = track[cursor]
            cursor += 1
            assert index or byte != 0x80
            value = (value << 7) | (byte & 0x7F)
            if not byte & 0x80:
                return value
        raise AssertionError("MIDI variable-length quantity exceeds four bytes")

    while cursor < len(track):
        tick += variable_length()
        assert not ended and cursor < len(track)
        status = track[cursor]
        cursor += 1
        if status == 0xFF:
            assert cursor < len(track)
            kind = track[cursor]
            cursor += 1
            size = variable_length()
            assert cursor + size <= len(track)
            payload = track[cursor:cursor + size]
            cursor += size
            if kind == 0x51:
                assert size == 3
                tempos.append((tick, int.from_bytes(payload, "big")))
            else:
                assert kind == 0x2F and size == 0 and not active
                ended = True
        elif status == 0xC0:
            assert cursor < len(track)
            programs.append((tick, track[cursor]))
            cursor += 1
        else:
            assert status in (0x80, 0x90) and cursor + 2 <= len(track)
            pitch, velocity = track[cursor:cursor + 2]
            cursor += 2
            assert 21 <= pitch <= 108 and 0 <= velocity <= 127
            if status == 0x90:
                assert velocity > 0 and pitch not in active
                active[pitch] = (tick, velocity)
            else:
                assert velocity == 0 and pitch in active
                start, on_velocity = active.pop(pitch)
                assert start < tick
                notes.append((start, tick, pitch, on_velocity))
    assert ended and not active
    assert tempos == [(0, 500000)] and programs == [(0, 0)]
    return sorted(notes)


def exercise(binary, root, receipt):
    tools = root / "tools.json"
    write_json(tools, {
        "schema": "aniflow.audio-inspection.configuration/v1",
        "ffmpeg": local_tool_pin("ffmpeg"), "ffprobe": local_tool_pin("ffprobe"),
        "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    })
    interpreter = Path(sys.executable).resolve(strict=True)
    python = root / "synthetic-python"
    fixture = SYNTHETIC_PYTHON.replace("__FIXTURE_ROOT__", repr(str(root))).replace(
        "__ADAPTER_MARKER__", repr(ADAPTER_MARKER)
    ).replace("__MODEL_MARKER__", repr(MODEL_MARKER)).replace("__MODEL_BYTE_SIZE__", str(MODEL_BYTE_SIZE))
    python.write_text(f"#!{interpreter}\n" + fixture, encoding="utf-8")
    python.chmod(0o755)
    adapter = root / "synthetic-adapter.py"
    adapter.write_text(ADAPTER_MARKER, encoding="utf-8")
    model = root / "synthetic-model.onnx"
    model.write_bytes(MODEL_MARKER + b"\0" * (MODEL_BYTE_SIZE - len(MODEL_MARKER)))
    model_bytes = model.read_bytes()
    write_json(root / "probe.json", PROBE)
    settings = root / "midi.json"
    write_json(settings, {
        "schema": "aniflow.audio-midi.configuration/v1",
        "python": {"executable": str(python), "version": PROBE["python_version"], "sha256": sha256(python)},
        "adapter": {"path": str(adapter), "sha256": sha256(adapter)},
        "runtime": {"sha256": RUNTIME_SHA256, "file_count": 1, "byte_count": 100},
        "model": {"path": str(model), "sha256": sha256(model), "byte_size": model.stat().st_size,
                  "model_id": "basic-pitch-onnx-icassp-2022", "revision": BASIC_PITCH_REVISION},
        "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    })
    source = root / "generated tone.wav"
    pcm(source)
    source_bytes = source.read_bytes()
    immutable = {source: source_bytes, model: model_bytes, adapter: adapter.read_bytes(), settings: settings.read_bytes()}
    shared = ["--input", source, "--configuration", tools, "--midi-configuration", settings]
    cases = []
    documents = []
    export_packages = []

    def mode(value):
        (root / "mode").write_text(value, encoding="utf-8")

    def log_bytes(name):
        path = root / name
        return path.read_bytes() if path.exists() else b""

    def assert_immutable():
        for path, original in immutable.items():
            assert path.read_bytes() == original, f"source/declared input was modified: {path.name}"

    mode("monophonic")
    planned = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "midi-candidates", *shared)
    assert planned["payload"]["policy"]["offline"]
    assert "ai" in planned["payload"]["policy"]["allowed_side_effects"]
    assert "network" not in planned["payload"]["policy"]["allowed_side_effects"]
    assert not log_bytes("inference-launches")
    assert_immutable()
    cases.append("offline-plan-no-inference")

    def run(name, current_source=source, expected="candidate"):
        current_shared = ["--input", current_source, "--configuration", tools, "--midi-configuration", settings]
        outcome = invoke(binary, "audio_midi_extract", "audio", "midi", "extract", *current_shared,
                         "--output-directory", root / name)
        outputs = {item["id"]: item for item in outcome["outputs"]}
        for item in outputs.values():
            path = Path(item["path"])
            assert sha256(path) == item["sha256"]
            documents.append((name + "-" + item["id"] + ".json", path))
        report = json.loads(Path(outputs["midi"]["path"]).read_text())
        assert report["schema"] == "aniflow.audio-midi/v1" and report["result"]["status"] == expected
        assert report["source"]["artifact"]["sha256"] == sha256(current_source)
        assert report["confidence"]["kind"] == "unavailable" and report["provenance"] == "probabilistic"
        assert report["method"]["backend"] == "onnxruntime_cpu" and not report["method"]["gpu"]
        assert report["probe"] == PROBE
        assert report["settings"]["model"]["sha256"] == hashlib.sha256(model_bytes).hexdigest()
        assert_immutable()
        cases.append(name)
        return outcome, outputs, report

    outcome, outputs, report = run("monophonic-candidate")
    notes = report["result"]["observation"]["notes"]
    assert [(note["pitch"], note["velocity"]) for note in notes] == [(60, 102), (64, 76)]
    assert [(note["start_tick"], note["end_tick"]) for note in notes] == [(192, 960), (1440, 2400)]
    launches = log_bytes("all-launches")
    status = invoke(binary, "status_v3", "status-v3", outcome["run_directory"])
    assert status["payload"]["state"] == "complete" and log_bytes("all-launches") == launches
    cases.append("read-only-status-no-launch")
    inference = log_bytes("inference-launches")
    resumed = invoke(binary, "audio_resume", "audio", "resume", outcome["run_directory"],
                     "--analysis", "midi-candidates", *shared)
    assert resumed["executed_stages"] == [] and resumed["reused_stages"] == outcome["executed_stages"]
    assert log_bytes("inference-launches") == inference
    cases.append("exact-resume-no-inference")

    report_path = Path(outputs["midi"]["path"])
    report_bytes = report_path.read_bytes()
    export = root / "candidate export"
    exported = invoke(binary, "audio_midi_export", "audio", "midi", "export", "--candidate", report_path,
                      "--output-directory", export)
    assert exported is not None
    assert (export / "candidate-report.json").read_bytes() == report_bytes
    assert json.loads((export / "notes.json").read_text()) == report["result"]["observation"]
    assert read_midi(export / "candidate.mid") == sorted(
        (note["start_tick"], note["end_tick"], note["pitch"], note["velocity"]) for note in notes
    )
    documents.append(("export-report.json", export / "export-report.json"))
    export_packages.append(("monophonic-export", export))
    assert report_path.read_bytes() == report_bytes
    assert_immutable()
    cases.append("explicit-export-independent-midi-readback")
    export_bytes = {path: path.read_bytes() for path in export.iterdir()}
    invoke(binary, "audio_midi_export", "audio", "midi", "export", "--candidate", report_path,
           "--output-directory", export, success=False)
    assert set(export.iterdir()) == set(export_bytes)
    assert all(path.read_bytes() == content for path, content in export_bytes.items())
    cases.append("existing-export-directory-refusal")

    mode("polyphony")
    _, poly_outputs, polyphonic = run("polyphonic-candidate")
    poly_notes = polyphonic["result"]["observation"]["notes"]
    assert [note["pitch"] for note in poly_notes] == [60, 67, 64]
    assert poly_notes[1]["start_tick"] < poly_notes[0]["end_tick"]
    poly_export = root / "polyphonic export"
    invoke(binary, "audio_midi_export", "audio", "midi", "export", "--candidate", poly_outputs["midi"]["path"],
           "--output-directory", poly_export)
    assert read_midi(poly_export / "candidate.mid") == sorted(
        (note["start_tick"], note["end_tick"], note["pitch"], note["velocity"]) for note in poly_notes
    )
    documents.append(("polyphonic-export-report.json", poly_export / "export-report.json"))
    export_packages.append(("polyphonic-export", poly_export))
    cases.append("polyphonic-export-independent-midi-readback")

    mode("empty")
    _, empty_outputs, empty = run("empty-candidate")
    assert empty["result"]["observation"]["notes"] == []
    assert empty["result"]["observation"]["native"]["notes"] == []
    empty_export = root / "empty export"
    invoke(binary, "audio_midi_export", "audio", "midi", "export", "--candidate", empty_outputs["midi"]["path"],
           "--output-directory", empty_export)
    assert read_midi(empty_export / "candidate.mid") == []
    documents.append(("empty-export-report.json", empty_export / "export-report.json"))
    export_packages.append(("empty-export", empty_export))
    cases.append("empty-export-no-invented-notes")
    mode("monophonic")
    inference = log_bytes("inference-launches")
    for name, rate, channels, silence, reason in [
        ("unsupported-sample-rate", 16000, 1, False, "unsupported_sample_rate"),
        ("unsupported-channels", 22050, 2, False, "unsupported_channels"),
        ("silent-input", 22050, 1, True, "silent_input"),
    ]:
        path = root / f"{name}.wav"
        pcm(path, rate=rate, channels=channels, silence=silence)
        immutable[path] = path.read_bytes()
        _, unavailable_outputs, unavailable = run(name, path, "unavailable")
        assert unavailable["result"]["reason"] == reason and unavailable["raw_observation"] is None
        assert log_bytes("inference-launches") == inference
        destination = root / (name + " export")
        invoke(binary, "audio_midi_export", "audio", "midi", "export", "--candidate", unavailable_outputs["midi"]["path"],
               "--output-directory", destination, success=False)
        assert not destination.exists()
    cases.append("unavailable-export-refusal")

    mode("malformed")
    failure_directory = root / "malformed-output"
    invoke(binary, "audio_midi_extract", "audio", "midi", "extract", *shared,
           "--output-directory", failure_directory, success=False)
    assert not list(failure_directory.glob("**/artifacts/audio-midi/midi.json"))
    cases.append("malformed-native-output-refusal")
    inference = log_bytes("inference-launches")
    mode("runtime-drift")
    refused = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "midi-candidates", *shared, success=False)
    assert any(item["code"] == "runtime_mismatch" for item in refused["diagnostics"])
    refusal_path = root / "runtime-refusal.json"
    write_json(refusal_path, refused)
    documents.append(("runtime-refusal.json", refusal_path))
    assert log_bytes("inference-launches") == inference
    cases.append("stale-runtime-identity-refusal")
    mode("monophonic")
    model.write_bytes(b"changed marker model identity\n" + model_bytes[len(b"changed marker model identity\n"):])
    refused = invoke(binary, "audio_resume", "audio", "resume", outcome["run_directory"],
                     "--analysis", "midi-candidates", *shared, success=False)
    assert any(item["code"] == "model_digest_mismatch" for item in refused["diagnostics"])
    assert log_bytes("inference-launches") == inference
    model.write_bytes(model_bytes)
    assert report_path.read_bytes() == report_bytes
    assert_immutable()
    cases.append("changed-model-resume-refusal")

    result = {
        "schema": "aniflow.audio-midi-smoke/v1", "synthetic_provider": True,
        "real_model_inference": False, "models_downloaded": False, "real_inspection_tools": True,
        "cases": cases, "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "source_unchanged": True, "documents": [], "export_packages": [],
    }
    if receipt:
        receipt = receipt.resolve()
        receipt.parent.mkdir(parents=True, exist_ok=True)
        captured = Path(tempfile.mkdtemp(prefix="midi-reports-", dir=receipt.parent))
        for name, path in documents:
            target = captured / name
            shutil.copyfile(path, target)
            result["documents"].append(str(target))
        for name, directory in export_packages:
            target = captured / name
            target.mkdir()
            package = {"name": name}
            for key, filename in [
                ("midi", "candidate.mid"), ("candidate_report", "candidate-report.json"),
                ("notes", "notes.json"), ("export_report", "export-report.json"),
            ]:
                shutil.copyfile(directory / filename, target / filename)
                package[key] = str(target / filename)
            result["export_packages"].append(package)
        write_json(receipt, result)
    print(json.dumps({"synthetic_midi_cases": len(cases), "captured_documents": len(result["documents"])}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="aniflow midi café ") as temporary:
        exercise(arguments.aniflow.resolve(strict=True), Path(temporary).resolve(), arguments.receipt)


if __name__ == "__main__":
    main()
