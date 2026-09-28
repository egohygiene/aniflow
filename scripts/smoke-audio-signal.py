#!/usr/bin/env python3
"""Check signal analysis with generated PCM truth and explicitly pinned local tools."""

from __future__ import annotations

import argparse
from array import array
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import platform
import runpy
import sys
import tempfile
import wave


INSPECTION = runpy.run_path(str(Path(__file__).with_name("smoke-audio-inspection.py")))
sha256 = INSPECTION["sha256"]
local_tool_pin = INSPECTION["local_tool_pin"]
run_json = INSPECTION["run_json"]


def pcm_bytes(samples: array) -> bytes:
    if sys.byteorder != "little":
        samples = array("h", samples)
        samples.byteswap()
    return samples.tobytes()


def tone(rate: int, seconds: int) -> array:
    # All selected rates are divisible by 1000, so this repeats an exact period.
    period = array("h", (round(8192 * math.sin(2 * math.pi * i / (rate // 1000)))
                         for i in range(rate // 1000)))
    return period * (1000 * seconds)


def fixtures():
    rate = 48000
    yield "signal_silence", rate, 1, array("h", [0]) * (rate * 4)
    yield "signal_tone_1khz", rate, 1, tone(rate, 4)
    # One sample beyond a full 100 ms block catches truncated terminal evidence.
    yield "signal_impulse_tail", rate, 1, array("h", [0]) * (rate * 4) + array("h", [32767])
    tapered = array("h", (round(16384 * math.sin(math.pi * i / 2 + math.pi / 4)
                               * min(1, i / 480, (rate - i - 1) / 480))
                          for i in range(rate)))
    yield "signal_intersample_peak", rate, 1, tapered
    asymmetric = array("h")
    for value in tone(rate, 4):
        asymmetric.extend([0, value])
    yield "signal_channel_asymmetry", rate, 2, asymmetric
    thresholds = (array("h", [32]) * 4800 + array("h", [33]) * 4800
                  + array("h", [0]) * 4799 + array("h", [33])
                  + array("h", [-32]) * 4800
                  + array("h", [32766, 32767, -32768, -32767, 32766]))
    yield "signal_clipping_thresholds", rate, 1, thresholds
    yield "signal_short_input", 8000, 1, tone(8000, 1)[:799]
    yield "signal_constant_lra", rate, 1, tone(rate, 60)
    yield "signal_sparse_lra", rate, 1, array("h", [0]) * (rate * 60 - 1) + array("h", [32767])
    yield "signal_high_rate", 96000, 1, tone(96000, 4)


def measured(measurement: dict) -> float:
    assert measurement["value"]["kind"] == "measured", measurement
    value = measurement["value"]["value"]
    assert isinstance(value, (int, float)) and math.isfinite(value), measurement
    return value


def unavailable(measurement: dict, reason: str) -> None:
    assert measurement["value"] == {"kind": "unavailable", "reason": reason}, measurement


def near(actual: float, expected: float, tolerance: float) -> None:
    assert math.isfinite(actual) and abs(actual - expected) <= tolerance, (
        f"{actual} differs from expected {expected} by more than {tolerance}"
    )


def region(channel: int, start: int, end: int) -> dict:
    return {"channel": channel, "range": {"start": start, "end": end}}


def check_values(name: str, rate: int, channels: int, samples: array, report: dict) -> dict:
    frames = len(samples) // channels
    assert report["settings"] == {
        "schema": "aniflow.audio-signal.configuration/v1",
        "silence_threshold_pcm": 32,
        "minimum_silence_milliseconds": 100,
        "clipping_threshold_pcm": 32767,
    }
    assert report["method"]["short_term_window_frames"] == rate * 3
    assert report["method"]["short_term_hop_frames"] == rate // 10
    assert report["method"]["loudness_range_minimum_frames"] == rate * 60
    assert report["method"]["true_peak_padding_frames"] == (rate // 10 if rate <= 48000 else 0)
    assert report["method"]["tool_summary_decimal_places"] == 1
    assert report["method"]["short_term_decimal_places"] == 3
    assert len(report["channels"]) == channels
    for channel, actual in enumerate(report["channels"]):
        values = samples[channel::channels]
        peak = max(abs(value) for value in values) / 32768
        rms = math.sqrt(sum(value * value for value in values) / frames) / 32768
        assert actual["channel"] == channel
        near(actual["sample_peak_ratio"], peak, 1e-12)
        near(actual["rms_ratio"], rms, 1e-12)
        for key in ["sample_peak", "rms", "crest_factor"]:
            assert actual[key]["scope"] == {"channels": [channel], "stem_id": None}
            assert actual[key]["unit"] == ("decibels" if key == "crest_factor" else "decibels_full_scale")
        if peak:
            near(measured(actual["sample_peak"]), 20 * math.log10(peak), 1e-9)
            near(measured(actual["rms"]), 20 * math.log10(rms), 1e-9)
            near(measured(actual["crest_factor"]), 20 * math.log10(peak / rms), 1e-9)
        else:
            for key in ["sample_peak", "rms", "crest_factor"]:
                unavailable(actual[key], "silent_input")

    full_scope = {"channels": list(range(channels)), "stem_id": None}
    for key in ["integrated_loudness", "loudness_range", "true_peak"]:
        assert report[key]["scope"] == full_scope
    assert report["integrated_loudness"]["unit"] == "loudness_units_full_scale"
    assert report["loudness_range"]["unit"] == "loudness_units"
    assert report["true_peak"]["unit"] == "decibels_true_peak"
    expected_windows = max(0, frames // (rate // 10) - 29)
    assert len(report["short_term"]) == expected_windows
    for index, observation in enumerate(report["short_term"]):
        start = index * (rate // 10)
        assert observation["range"] == {"start": start, "end": start + rate * 3}
        assert observation["measurement"]["scope"] == full_scope
        assert observation["measurement"]["unit"] == "loudness_units_full_scale"
    for key in ["silence_regions", "clipping_regions"]:
        previous = (-1, -1)
        for item in report[key]:
            start, end = item["range"]["start"], item["range"]["end"]
            assert 0 <= item["channel"] < channels and 0 <= start < end <= frames
            assert (item["channel"], start) >= previous
            previous = (item["channel"], end)

    if name == "signal_silence":
        for key in ["integrated_loudness", "loudness_range", "true_peak"]:
            unavailable(report[key], "silent_input")
        assert report["silence_regions"] == [region(0, 0, frames)]
        assert report["clipping_regions"] == []
        for observation in report["short_term"]:
            unavailable(observation["measurement"], "silent_input")
    elif name in {"signal_tone_1khz", "signal_constant_lra", "signal_high_rate"}:
        near(measured(report["integrated_loudness"]), -15.05, 0.2)
        for observation in report["short_term"]:
            near(measured(observation["measurement"]), -15.05, 0.2)
        assert report["silence_regions"] == report["clipping_regions"] == []
        if name == "signal_constant_lra":
            near(measured(report["loudness_range"]), 0, 0.1)
        else:
            unavailable(report["loudness_range"], "insufficient_duration")
        if name == "signal_high_rate":
            unavailable(report["true_peak"], "unsupported_true_peak_rate")
        else:
            near(measured(report["true_peak"]), 20 * math.log10(0.25), 0.2)
    elif name in {"signal_impulse_tail", "signal_sparse_lra"}:
        near(measured(report["true_peak"]), 0, 0.1)
        assert report["silence_regions"] == [region(0, 0, frames - 1)]
        assert report["clipping_regions"] == [region(0, frames - 1, frames)]
        if name == "signal_sparse_lra":
            unavailable(report["loudness_range"], "insufficient_gated_windows")
        else:
            unavailable(report["integrated_loudness"], "below_absolute_gate")
    elif name == "signal_intersample_peak":
        sample_peak = measured(report["channels"][0]["sample_peak"])
        true_peak = measured(report["true_peak"])
        near(sample_peak, -9.03, 0.02)
        near(true_peak, -6.02, 0.2)
        assert true_peak > sample_peak + 2.5
    elif name == "signal_channel_asymmetry":
        assert report["silence_regions"] == [region(0, 0, frames)]
        assert report["clipping_regions"] == []
        near(measured(report["integrated_loudness"]), -15.05, 0.2)
    elif name == "signal_clipping_thresholds":
        assert report["silence_regions"] == [region(0, 0, 4800), region(0, 14400, 19200)]
        assert report["clipping_regions"] == [region(0, 19201, 19204)]
    elif name == "signal_short_input":
        unavailable(report["integrated_loudness"], "insufficient_duration")
        unavailable(report["loudness_range"], "insufficient_duration")
        assert report["short_term_status"] == {"kind": "unavailable", "reason": "insufficient_duration"}
        assert report["short_term"] == []
        measured(report["true_peak"])
    return {
        "integrated_loudness": report["integrated_loudness"]["value"],
        "loudness_range": report["loudness_range"]["value"],
        "true_peak": report["true_peak"]["value"],
        "channel_sample_peak_ratios": [item["sample_peak_ratio"] for item in report["channels"]],
        "short_term_windows": len(report["short_term"]),
        "silence_regions": report["silence_regions"],
        "clipping_regions": report["clipping_regions"],
    }


def exercise(binary: Path, directory: Path, pins_path: Path, settings_path: Path,
             name: str, rate: int, channels: int, samples: array) -> dict:
    directory.mkdir()
    source = directory / f"{name} synthetic source.wav"
    pcm = pcm_bytes(samples)
    with wave.open(str(source), "wb") as output:
        output.setparams((channels, 2, rate, len(samples) // channels, "NONE", "not compressed"))
        output.writeframes(pcm)
    source_hash = sha256(source)
    pcm_hash = hashlib.sha256(pcm).hexdigest()
    shared = ["--input", str(source), "--configuration", str(pins_path),
              "--analysis", "signal", "--signal-configuration", str(settings_path)]
    before = sorted(path.relative_to(directory) for path in directory.rglob("*"))
    plan = run_json(binary, "audio_plan", "audio", "plan", *shared)
    assert plan["schema"] == "aniflow.pipeline-plan/v1"
    assert sorted(path.relative_to(directory) for path in directory.rglob("*")) == before
    assert sha256(source) == source_hash
    outcome = run_json(binary, "audio_analyze", "audio", "analyze", *shared,
                       "--output-directory", str(directory / "runs"))
    assert outcome["executed_stages"] == ["inspect_audio", "measure_audio"]
    assert outcome["reused_stages"] == []
    outputs = {item["id"]: item for item in outcome["outputs"]}
    assert set(outputs) == {"technical", "signal", "analysis"}
    artifacts = {}
    for key, item in outputs.items():
        path = Path(item["path"])
        assert sha256(path) == item["sha256"]
        artifacts[key] = json.loads(path.read_text())
    technical = next(value for value in artifacts.values()
                     if value["schema"] == "aniflow.audio-technical-inspection/v1")
    signal = next(value for value in artifacts.values()
                  if value["schema"] == "aniflow.audio-signal-measurements/v1")
    signal_output = next(outputs[key] for key, value in artifacts.items() if value is signal)
    pins = json.loads(pins_path.read_text())
    assert signal["tools"] == [{"id": tool, "version": pins[tool]["version"],
                                "sha256": pins[tool]["sha256"]}
                               for tool in ["ffmpeg", "ffprobe"]]
    analysis = next(value for value in artifacts.values()
                    if value["schema"] == "aniflow.audio-analysis/v1"
                    and any(item["sha256"] == signal_output["sha256"] for item in value["artifacts"]))
    expected_status = "complete" if name == "signal_constant_lra" else "partial"
    assert analysis["status"] == expected_status
    frames = len(samples) // channels
    assert technical["source"]["sha256"] == source_hash
    assert technical["pcm_sha256"] == technical["decoded_pcm_sha256"] == pcm_hash
    assert technical["decode_complete"] is True and technical["source_unchanged"] is True
    for document in [signal, analysis]:
        assert document["source"]["artifact"]["sha256"] == source_hash
        assert document["source"]["frame_count"] == frames
        assert document["source"]["channels"] == channels
        assert document["source"]["sample_rate_hz"] == rate
    for reference in [signal["technical_artifact"], signal["inspection_analysis_artifact"]]:
        upstream = Path(outputs["technical"]["path"]).with_name(reference["id"] + ".json")
        assert sha256(upstream) == reference["sha256"]
        assert upstream.stat().st_size == reference["byte_size"]
    capability = next(item for item in analysis["capabilities"]
                      if item["capability"]["id"] == "aniflow/audio-signal-measurements")
    assert capability["status"] == expected_status
    assert "signal" in capability["evidence_artifact_ids"]
    values = check_values(name, rate, channels, samples, signal)
    status = run_json(binary, "status_v3", "status-v3", outcome["run_directory"])
    assert status["payload"]["state"] == "complete"
    resumed = False
    if name == "signal_tone_1khz":
        result = run_json(binary, "audio_resume", "audio", "resume", outcome["run_directory"], *shared)
        assert result["executed_stages"] == []
        assert result["reused_stages"] == ["inspect_audio", "measure_audio"]
        assert result["outputs"] == outcome["outputs"]
        resumed = True
    assert sha256(source) == source_hash
    print(f"Audio signal smoke passed: {name} ({rate} Hz, {channels} channel(s), {frames} frames)", flush=True)
    return {"fixture_id": name, "sample_rate_hz": rate, "channels": channels,
            "frame_count": frames, "source_sha256": source_hash, "pcm_sha256": pcm_hash,
            "signal_report_sha256": signal_output["sha256"], "source_unchanged": True,
            "plan_read_only": True, "status_complete": True, "resume_both_stages": resumed,
            "measurements": values}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    arguments = parser.parse_args()
    binary = arguments.aniflow.resolve(strict=True)
    pins = {name: local_tool_pin(name) for name in ["ffmpeg", "ffprobe"]}
    results = []
    with tempfile.TemporaryDirectory(prefix="aniflow-signal-smoke-") as temporary:
        directory = Path(temporary).resolve()
        pins_path = directory / "tool-pins.json"
        pins_path.write_text(json.dumps({"schema": "aniflow.audio-inspection.configuration/v1",
                                        **pins, "tool_timeout_milliseconds": 30000,
                                        "maximum_tool_output_bytes": 1048576}) + "\n")
        settings_path = directory / "signal-settings.json"
        settings_path.write_text(json.dumps({"schema": "aniflow.audio-signal.configuration/v1"}) + "\n")
        for name, rate, channels, samples in fixtures():
            try:
                results.append(exercise(binary, directory / name, pins_path, settings_path,
                                        name, rate, channels, samples))
            except Exception as error:
                raise RuntimeError(f"{name}: {error}") from error
    receipt = {"schema": "aniflow.audio-signal-smoke/v1", "synthetic_only": True,
               "checked_at_utc": datetime.now(timezone.utc).isoformat(),
               "platform": platform.platform(), "aniflow_sha256": sha256(binary),
               "tools": pins, "fixtures": results,
               "tolerances": {"native_ratio_absolute": 1e-12, "native_decibels_absolute": 1e-9,
                              "tone_loudness_lu": 0.2, "true_peak_db": 0.2,
                              "constant_lra_lu": 0.1},
               "qualification": "Local synthetic smoke only; no EBU compliance or release qualification."}
    if arguments.receipt:
        arguments.receipt.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(f"Audio signal smoke passed: {len(results)} synthetic fixtures")


if __name__ == "__main__":
    main()
