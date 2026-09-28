#!/usr/bin/env python3
"""Exercise the actual pinned Essentia adapter using disposable synthetic PCM.

Results qualify only these declared fixtures and tolerances. They do not measure
musical accuracy on real recordings or turn native scores into probabilities.
"""

from __future__ import annotations

import argparse
from array import array
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import random
import runpy
import signal
import subprocess
import sys
import tempfile
import wave


RATE = 44_100
MAXIMUM_CAPTURE = 1024 * 1024
INSPECTION = runpy.run_path(str(Path(__file__).with_name("smoke-audio-inspection.py")))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(python: Path, adapter: Path, arguments: list[str]) -> subprocess.CompletedProcess:
    command = [str(python), "-I", "-B", str(adapter), *arguments]
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=120)
    except BaseException:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise
    if len(stdout) > MAXIMUM_CAPTURE or len(stderr) > MAXIMUM_CAPTURE:
        raise AssertionError("adapter capture exceeded the fixture runner bound")
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def successful_json(output: subprocess.CompletedProcess) -> dict:
    assert output.returncode == 0, output.stderr.decode("utf-8", errors="replace")
    return json.loads(output.stdout)


def click_track(bpm: int, seconds: int = 24) -> array:
    samples = array("h", [0]) * (RATE * seconds)
    random_source = random.Random(46)
    burst = [round(24_000 * random_source.uniform(-1, 1) * math.exp(-index / 180))
             for index in range(RATE // 25)]
    interval = RATE * 60 // bpm
    for start in range(RATE // 2, len(samples) - len(burst), interval):
        samples[start:start + len(burst)] = array("h", burst)
    return samples


def triad(frequencies: tuple[float, ...], seconds: int = 12) -> array:
    frames = RATE * seconds
    samples = array("h")
    for frame in range(frames):
        fade = min(1, frame / (RATE / 50), (frames - frame - 1) / (RATE / 50))
        sample = sum(math.sin(2 * math.pi * frequency * frame / RATE) for frequency in frequencies)
        samples.append(round(7000 * fade * sample))
    return samples


def fixtures():
    for bpm in (90, 120, 150):
        yield f"click_{bpm}", RATE, 1, click_track(bpm), {"tempo_bpm": bpm}
    major = triad((261.625565, 329.627557, 391.995436))
    minor = triad((220.0, 261.625565, 329.627557))
    yield "c_major", RATE, 1, major, {"key": "C", "scale": "major"}
    yield "a_minor", RATE, 1, minor, {"key": "A", "scale": "minor"}
    # Equal-length remote triads intentionally provide no single key truth.
    ambiguous = major[:RATE * 6] + triad((369.994423, 466.163762, 554.365262), 6)
    yield "alternating_triads", RATE, 1, ambiguous, {"ambiguous": True}
    yield "silence_8s", RATE, 1, array("h", [0]) * (RATE * 8), {"refusal": "silent_input"}
    antiphase = array("h")
    for sample in major[:RATE * 8]:
        antiphase.extend((sample, -sample))
    yield "antiphase_8s", RATE, 2, antiphase, {"refusal": "silent_input"}
    yield "short_4s", RATE, 1, major[:RATE * 4], {"refusal": "insufficient_duration"}
    yield "unsupported_rate_48000", 48_000, 1, array("h", [1]) * (48_000 * 8), {"refusal": "unsupported_rate"}


def write_wav(path: Path, rate: int, channels: int, samples: array) -> None:
    if sys.byteorder != "little":
        samples = array("h", samples)
        samples.byteswap()
    with wave.open(str(path), "wb") as destination:
        destination.setnchannels(channels)
        destination.setsampwidth(2)
        destination.setframerate(rate)
        destination.writeframes(samples.tobytes())


def validate_observation(name: str, expected: dict, document: dict, frames: int,
                         channels: int) -> dict:
    assert document["schema"] == "aniflow.audio-musical-observation/v1"
    assert document["sample_rate_hz"] == RATE
    assert document["sample_frames"] == frames
    assert document["channels"] == channels
    assert document["downmix"] == "arithmetic_average"
    assert document["duration_seconds"] == frames / RATE
    bpm = document["bpm"]
    # Beat coordinates remain native seconds. The consumer's explicitly
    # documented nearest-frame policy must produce strict in-source order.
    ticks = bpm["ticks_seconds"]
    quantized = [math.floor(tick * RATE + 0.5) for tick in ticks]
    assert all(0 <= frame < frames for frame in quantized), (name, quantized)
    assert all(left < right for left, right in zip(quantized, quantized[1:])), name
    for value in [bpm["value"], bpm["raw_confidence"], *ticks, *bpm["estimates"], *bpm["bpm_intervals"]]:
        assert isinstance(value, (float, int)) and math.isfinite(value), (name, value)
    profiles = document["key_profiles"]
    assert [item["profile"] for item in profiles] == ["krumhansl", "temperley"]
    assert all(math.isfinite(item["raw_strength"]) for item in profiles)
    assessment = {"native_scores_are_probabilities": False}
    if "tempo_bpm" in expected:
        candidates = [(factor, abs(bpm["value"] - expected["tempo_bpm"] * factor))
                      for factor in (0.5, 1, 2)]
        factor, difference = min(candidates, key=lambda item: item[1])
        assert difference <= 3, (name, bpm["value"], expected)
        assert len(ticks) >= 4, (name, ticks)
        assessment.update({"tempo_tolerance_bpm": 3, "matched_tempo_factor": factor,
                           "tempo_error_bpm": difference,
                           "half_double_tempo_ambiguity": factor != 1})
    if "key" in expected:
        matching = [item["profile"] for item in profiles
                    if item["key"] == expected["key"] and item["scale"] == expected["scale"]]
        assert matching, (name, profiles)
        assessment["matching_key_profiles"] = matching
    if expected.get("ambiguous"):
        assessment["single_ground_truth_key"] = "not_assigned"
        assessment["profiles_agree"] = len({(item["key"], item["scale"]) for item in profiles}) == 1
    return assessment


def bytecode_isolation_check(python: Path, adapter: Path) -> dict:
    # Create a valid timestamp-based poison cache for a wholly disposable module.
    # The normal -B import would consume it. The adapter's fresh prefix must
    # bypass it without altering any installed runtime distribution or cache.
    program = r'''
import importlib.util, pathlib, py_compile, sys, tempfile
spec = importlib.util.spec_from_file_location("musical_adapter", sys.argv[1])
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)
with tempfile.TemporaryDirectory(prefix="aniflow-bytecode-fixture-") as root:
    source = pathlib.Path(root) / "synthetic_bytecode_fixture.py"
    source.write_text("value = 'poison'\n")
    py_compile.compile(str(source), doraise=True)
    original = source.stat()
    source.write_text("value = 'source'\n")
    import os
    os.utime(source, ns=(original.st_atime_ns, original.st_mtime_ns))
    sys.path.insert(0, root)
    import synthetic_bytecode_fixture
    assert synthetic_bytecode_fixture.value == "poison"
    del sys.modules["synthetic_bytecode_fixture"]
    with adapter.configure_environment():
        import synthetic_bytecode_fixture
        assert synthetic_bytecode_fixture.value == "source"
print("bytecode isolation passed")
'''
    output = subprocess.run([str(python), "-I", "-B", "-c", program, str(adapter)],
                            capture_output=True, timeout=20, check=False)
    assert output.returncode == 0, output.stderr.decode("utf-8", errors="replace")
    return {"fixture": "poisoned_disposable_bytecode", "result": "passed"}


def tree_snapshot(directory: Path) -> dict:
    return {str(path.relative_to(directory)): (sha256(path) if path.is_file() else None)
            for path in directory.rglob("*")}


def native_configurations(arguments, root: Path, probe: dict) -> tuple[Path, Path]:
    tools_path = root / "inspection-configuration.json"
    tools_path.write_text(json.dumps({
        "schema": "aniflow.audio-inspection.configuration/v1",
        **{name: INSPECTION["local_tool_pin"](name) for name in ("ffmpeg", "ffprobe")},
        "tool_timeout_milliseconds": 30000, "maximum_tool_output_bytes": 65536,
    }, indent=2) + "\n", encoding="utf-8")
    settings_path = root / "musical-configuration.json"
    settings_path.write_text(json.dumps({
        "schema": "aniflow.audio-musical.configuration/v1",
        "python": {"executable": str(arguments.python), "version": probe["python_version"],
                   "sha256": sha256(arguments.python)},
        "adapter": {"path": str(arguments.adapter), "sha256": sha256(arguments.adapter)},
        "runtime": {"sha256": probe["runtime_sha256"], "file_count": probe["runtime_file_count"],
                    "byte_count": probe["runtime_byte_count"]},
        "tool_timeout_milliseconds": 120000, "maximum_tool_output_bytes": 1048576,
    }, indent=2) + "\n", encoding="utf-8")
    return tools_path, settings_path


def exercise_native(binary: Path, source: Path, configuration: tuple[Path, Path],
                    fixture: str, expected: dict, source_hash: str, frames: int,
                    rate: int, channels: int) -> dict:
    run_json = INSPECTION["run_json"]
    shared = ["--input", str(source), "--configuration", str(configuration[0]),
              "--analysis", "musical", "--musical-configuration", str(configuration[1])]
    runs = source.parent / f"native-{fixture}-runs"
    before_plan = tree_snapshot(source.parent)
    plan = run_json(binary, "audio_plan", "audio", "plan", *shared)
    assert plan["schema"] == "aniflow.pipeline-plan/v1"
    assert tree_snapshot(source.parent) == before_plan, "planning changed the fixture tree"
    assert not runs.exists()
    outcome = run_json(binary, "audio_analyze", "audio", "analyze", *shared,
                       "--output-directory", str(runs))
    assert outcome["executed_stages"] == ["inspect_audio", "analyze_musical"]
    assert outcome["reused_stages"] == []
    run = Path(outcome["run_directory"])
    outputs = {item["id"]: item for item in outcome["outputs"]}
    assert set(outputs) == {"technical", "musical", "analysis"}
    artifacts = {}
    for name, item in outputs.items():
        path = Path(item["path"])
        assert path.is_relative_to(run)
        assert sha256(path) == item["sha256"]
        artifacts[name] = json.loads(path.read_text(encoding="utf-8"))
    report = artifacts["musical"]
    normalized = artifacts["analysis"]
    assert report["schema"] == "aniflow.audio-musical-analysis/v1"
    assert normalized["schema"] == "aniflow.audio-analysis/v1"
    assert report["source"] == normalized["source"]
    assert report["source"]["artifact"]["sha256"] == source_hash
    assert report["source"]["frame_count"] == frames
    assert report["source"]["sample_rate_hz"] == rate
    assert report["scope"] == {"channels": list(range(channels)), "stem_id": None}
    assert report["provenance"] == "heuristic"
    assert report["confidence"]["kind"] == "unavailable"
    result = report["result"]
    observations = [item for item in normalized["observations"]
                    if item["capability_id"] == "aniflow/audio-musical-structure"]
    if "refusal" in expected:
        reason = {"silent_input": "silent_downmix", "insufficient_duration": "insufficient_duration",
                  "unsupported_rate": "unsupported_sample_rate"}[expected["refusal"]]
        assert result == {"status": "unavailable", "reason": reason}
        assert observations == []
        assert normalized["status"] == "partial"
    else:
        assert result["status"] == "estimated"
        validate_observation(fixture, expected, result["observation"], frames, channels)
        assert all(item["provenance"]["class"] == "heuristic"
                   and item["provenance"]["confidence"]["kind"] == "unavailable" for item in observations)
        assert all(item["scope"] == report["scope"] for item in observations)
        previous = -1
        for beat in result["beats"]:
            assert previous < beat["source_frame"] < frames
            assert beat["source_frame"] == math.floor(beat["seconds"] * rate + 0.5)
            previous = beat["source_frame"]
    before_status = tree_snapshot(run)
    status = run_json(binary, "status_v3", "status-v3", str(run))
    assert status["payload"]["state"] == "complete"
    assert tree_snapshot(run) == before_status, "status mutated the run"
    assert all(stage["checkpoint"] for stage in status["payload"]["stages"])
    resumed = run_json(binary, "audio_resume", "audio", "resume", str(run), *shared)
    assert resumed["executed_stages"] == []
    assert resumed["reused_stages"] == ["inspect_audio", "analyze_musical"]
    assert resumed["outputs"] == outcome["outputs"]
    assert sha256(source) == source_hash
    return {
        "result": "passed", "plan_read_only": True, "status_read_only": True,
        "resume_reused_both_stages": True, "source_unchanged": True,
        "plan_sha256": plan["plan_sha256"],
        "artifacts": {name: {"run_relative_path": str(Path(item["path"]).relative_to(run)),
                             "sha256": item["sha256"], "document": artifacts[name]}
                      for name, item in outputs.items()},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", type=Path, required=True, help="pinned local Essentia interpreter")
    parser.add_argument("--adapter", type=Path, default=Path(__file__).with_name("audio-musical-adapter.py").resolve())
    parser.add_argument("--output", type=Path, required=True, help="new JSON evidence file; never overwritten")
    parser.add_argument("--aniflow", type=Path, help="optional absolute native CLI for full Pipeline v3 route")
    arguments = parser.parse_args()
    if not arguments.python.is_absolute() or not arguments.adapter.is_absolute():
        parser.error("--python and --adapter must be absolute paths")
    if arguments.aniflow is not None and not arguments.aniflow.is_absolute():
        parser.error("--aniflow must be an absolute path")
    if arguments.output.exists():
        parser.error("--output already exists; choose a new evidence path")
    adapter_before = sha256(arguments.adapter)
    probe = successful_json(execute(arguments.python, arguments.adapter, ["--probe"]))
    results = []
    with tempfile.TemporaryDirectory(prefix="aniflow-musical-synthetic-") as temporary:
        root = Path(temporary)
        configuration = native_configurations(arguments, root, probe) if arguments.aniflow else None
        for name, rate, channels, samples, expected in fixtures():
            path = root / f"{name}.wav"
            write_wav(path, rate, channels, samples)
            source_before = sha256(path)
            output = execute(arguments.python, arguments.adapter, ["--input", str(path)])
            frames = len(samples) // channels
            record = {"id": name, "source_sha256": source_before, "sample_rate_hz": rate,
                      "channels": channels, "sample_frames": frames, "expected": expected}
            if "refusal" in expected:
                assert output.returncode == 65, (name, output.stderr)
                assert output.stdout == b"", (name, output.stdout)
                refusal = json.loads(output.stderr)
                assert refusal["code"] == expected["refusal"], (name, refusal)
                record["refusal"] = refusal
            else:
                document = successful_json(output)
                record["observation"] = document
                record["assessment"] = validate_observation(name, expected, document, frames, channels)
            assert sha256(path) == source_before, f"{name}: source bytes changed"
            if arguments.aniflow and name not in {"click_90", "click_150", "a_minor"}:
                record["native"] = exercise_native(arguments.aniflow, path, configuration,
                                                   name, expected, source_before, frames, rate, channels)
            record["source_unchanged"] = True
            record["result"] = "passed"
            results.append(record)
            print(f"Synthetic musical fixture passed: {name}", file=sys.stderr, flush=True)
    assert successful_json(execute(arguments.python, arguments.adapter, ["--probe"])) == probe
    assert sha256(arguments.adapter) == adapter_before, "adapter changed during qualification"
    evidence = {
        "schema": "aniflow.audio-musical-synthetic-corpus/v1",
        "observed_at": datetime.now(timezone.utc).isoformat(),
        "adapter_sha256": adapter_before, "probe": probe,
        "native_cli_sha256": sha256(arguments.aniflow) if arguments.aniflow else None,
        "fixtures": results,
        "isolation": bytecode_isolation_check(arguments.python, arguments.adapter),
        "limits": ["Synthetic signals only; no user media was read or changed.",
                   "Click tolerance is 3 BPM and explicitly allows half/double-tempo ambiguity.",
                   "Triad checks require at least one declared key profile to match.",
                   "Alternating triads have no single assigned ground-truth key.",
                   "Scores are native metrics, not calibrated confidence or probabilities.",
                   "This adapter corpus does not qualify native macOS, hosted CI, or real-music accuracy.",
                   "The runtime fingerprint covers installed package trees, not OS libraries or Python standard library."],
    }
    with arguments.output.open("x", encoding="utf-8") as destination:
        json.dump(evidence, destination, ensure_ascii=False, indent=2, allow_nan=False)
        destination.write("\n")
    print(json.dumps({"result": "passed", "fixtures": len(results), "output": str(arguments.output)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
