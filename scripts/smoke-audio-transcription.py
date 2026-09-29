#!/usr/bin/env python3
"""Run generated PCM through real inspection and a synthetic whisper/model ABI."""

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

# Deliberately synthetic observations: this never loads or executes model weights.
WHISPER = r'''
import json, pathlib, sys
root = pathlib.Path(__FIXTURE_ROOT__)
args = sys.argv[1:]
if args == ['--version']:
    print('whisper.cpp version: 1.8.7')
    raise SystemExit(0)
assert args[args.index('--output-file') + 1] == '-'
assert '--output-json' in args and '--no-gpu' in args and '--no-flash-attn' in args
assert '--no-fallback' in args and '--output-json-full' not in args
with (root / 'launches').open('a') as log: log.write(json.dumps(args) + '\n')
segments = [
    {'timestamps': {'from': '00:00:00,000', 'to': '00:00:00,500'},
     'offsets': {'from': 0, 'to': 500}, 'text': ' synthetic hello'},
    {'timestamps': {'from': '00:00:00,700', 'to': '00:00:01,200'},
     'offsets': {'from': 700, 'to': 1200}, 'text': ' synthetic café'}]
if (root / 'mode').read_text() == 'empty': segments = []
print(json.dumps({
    'systeminfo': 'WHISPER : COREML = 0 | OPENVINO = 0 | CPU : synthetic |',
    'model': {'type': 'tiny', 'multilingual': False, 'vocab': 51864,
        'audio': {'ctx': 1500, 'state': 384, 'head': 6, 'layer': 4},
        'text': {'ctx': 448, 'state': 384, 'head': 6, 'layer': 4}, 'mels': 80, 'ftype': 1},
    'params': {'model': args[args.index('--model') + 1], 'language': 'en', 'translate': False},
    'result': {'language': 'en'}, 'transcription': segments}))
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


def pcm(path, rate):
    samples = b"".join(struct.pack("<h", 1200 if index % 64 < 32 else -1200) for index in range(rate * 2))
    with wave.open(str(path), "wb") as audio:
        audio.setparams((1, 2, rate, rate * 2, "NONE", "not compressed"))
        audio.writeframes(samples)


def exercise(binary, root, receipt):
    tools = root / "tools.json"
    tools.write_text(json.dumps({
        "schema": "aniflow.audio-inspection.configuration/v1",
        "ffmpeg": local_tool_pin("ffmpeg"), "ffprobe": local_tool_pin("ffprobe"),
        "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    }), encoding="utf-8")
    whisper = root / "synthetic-whisper"
    interpreter = Path(sys.executable).resolve(strict=True)
    whisper.write_text(f"#!{interpreter}\n" + WHISPER.replace("__FIXTURE_ROOT__", json.dumps(str(root))), encoding="utf-8")
    whisper.chmod(0o755)
    model = root / "synthetic-tiny.en.bin"
    model.write_bytes(b"Synthetic model identity for ABI conformance; not inference weights.\n")
    settings = root / "transcription.json"
    settings.write_text(json.dumps({
        "schema": "aniflow.audio-transcription.configuration/v1",
        "whisper": {"executable": str(whisper), "version": "1.8.7", "sha256": sha256(whisper)},
        "model": {"path": str(model), "sha256": sha256(model), "byte_size": model.stat().st_size,
                  "model_id": "tiny.en", "revision": "synthetic-fixture-v1"},
        "language": "en", "threads": 1,
        "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    }), encoding="utf-8")
    (root / "mode").write_text("ok", encoding="utf-8")
    source = root / "generated tone.wav"
    pcm(source, 16000)
    source_bytes = source.read_bytes()
    shared = ["--input", source, "--configuration", tools, "--transcription-configuration", settings]
    cases = []
    documents = []

    plan = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "transcription", *shared)
    assert plan["payload"]["policy"]["offline"]
    assert "ai" in plan["payload"]["policy"]["allowed_side_effects"]
    assert "network" not in plan["payload"]["policy"]["allowed_side_effects"]
    assert not (root / "launches").exists()
    cases.append("offline-plan")

    def run(name, expected):
        outcome = invoke(binary, "audio_transcribe", "audio", "transcribe", *shared, "--output-directory", root / name)
        outputs = {item["id"]: item for item in outcome["outputs"]}
        for item in outputs.values():
            path = Path(item["path"])
            assert sha256(path) == item["sha256"]
            documents.append((name + "-" + item["id"] + ".json", path))
        report = json.loads(Path(outputs["transcription"]["path"]).read_text())
        assert report["schema"] == "aniflow.audio-transcription/v1"
        assert report["result"]["status"] == expected
        assert report["source"]["artifact"]["sha256"] == hashlib.sha256(source_bytes).hexdigest()
        assert report["confidence"]["kind"] == "unavailable"
        assert report["word_timing"]["status"] == "unavailable"
        assert report["provenance"] == "probabilistic"
        assert source.read_bytes() == source_bytes
        cases.append(name)
        return outcome, outputs, report

    outcome, outputs, report = run("observed", "observed")
    assert report["timed_text"]["provenance"]["kind"] == "observed_transcript"
    assert report["result"]["observation"]["segments"][0]["end"] == {"numerator": 1, "denominator": 2}
    assert len(report["timed_text"]["cues"]) == 2
    launches = (root / "launches").read_bytes()
    status = invoke(binary, "status_v3", "status-v3", outcome["run_directory"])
    assert status["payload"]["state"] == "complete"
    resumed = invoke(binary, "audio_resume", "audio", "resume", outcome["run_directory"], "--analysis", "transcription", *shared)
    assert resumed["executed_stages"] == [] and resumed["reused_stages"] == outcome["executed_stages"]
    assert (root / "launches").read_bytes() == launches
    cases.append("status-exact-resume")

    report_path = Path(outputs["transcription"]["path"])
    report_sha = sha256(report_path)
    export = root / "export"
    refused = invoke(binary, "audio_transcript_export", "audio", "transcript-export", "--transcription", report_path,
                     "--to", "webvtt", "--output-directory", export, success=False)
    assert any(loss["kind"] == "language" for loss in refused["losses"])
    assert not export.exists()
    cases.append("export-loss-refusal")
    exported = invoke(binary, "audio_transcript_export", "audio", "transcript-export", "--transcription", report_path,
                      "--to", "webvtt", "--output-directory", export, "--allow-loss", "language")
    assert exported["transcription_report"]["sha256"] == report_sha
    conversion = json.loads((export / "conversion.json").read_text())
    assert conversion["output"]["sha256"] == sha256(export / "payload.vtt")
    assert conversion["input_document_sha256"] == sha256(export / "normalized-input.json")
    assert conversion["output_document_sha256"] == sha256(export / "normalized-output.json")
    assert not conversion["review_attestation_independently_verified"]
    assert json.loads((export / "normalized-output.json").read_text())["provenance"]["kind"] == "observed_transcript"
    assert sha256(report_path) == report_sha
    for name in ["normalized-input.json", "normalized-output.json", "conversion.json"]:
        documents.append(("export-" + name, export / name))
    cases.append("observed-export")

    (root / "mode").write_text("empty", encoding="utf-8")
    _, _, empty = run("empty", "empty")
    assert empty["timed_text"] is None
    assert empty["result"]["observation"]["segments"] == []
    launches = (root / "launches").read_bytes()
    model.unlink()
    refused = invoke(binary, "audio_plan", "audio", "plan", "--analysis", "transcription", *shared, success=False)
    assert any(item["code"] == "missing_model" for item in refused["diagnostics"])
    assert (root / "launches").read_bytes() == launches
    cases.append("missing-model-refusal")
    assert source.read_bytes() == source_bytes

    result = {"schema": "aniflow.audio-transcription-smoke/v1", "synthetic_provider": True,
              "real_model_inference": False, "real_inspection_tools": True,
              "cases": cases, "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
              "source_unchanged": True, "documents": []}
    if receipt:
        receipt = receipt.resolve()
        receipt.parent.mkdir(parents=True, exist_ok=True)
        captured = Path(tempfile.mkdtemp(prefix="transcription-reports-", dir=receipt.parent))
        for name, path in documents:
            target = captured / name
            shutil.copyfile(path, target)
            result["documents"].append(str(target))
        receipt.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"synthetic_transcription_cases": len(cases), "captured_documents": len(result["documents"])}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="aniflow transcription café ") as temporary:
        exercise(arguments.aniflow.resolve(strict=True), Path(temporary).resolve(), arguments.receipt)


if __name__ == "__main__":
    main()
