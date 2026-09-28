#!/usr/bin/env python3
"""Qualify four high-rate true-peak profiles through the CLI using synthetic PCM."""

from __future__ import annotations

import argparse
from array import array
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import platform
import runpy
import tempfile


SIGNAL = runpy.run_path(str(Path(__file__).with_name("smoke-audio-signal.py")))
RATES = (88200, 96000, 176400, 192000)
PEAK_TOLERANCE_DB = 0.02


def fixtures():
    for rate in RATES:
        for case in ("sine", "tail", "silence", "short_one_frame",
                     "short_partial_hop", "asymmetric", "over_full_scale"):
            channels = 2 if case == "asymmetric" else 1
            if case == "silence":
                samples = array("h", [0]) * rate
            elif case in {"tail", "short_one_frame", "short_partial_hop"}:
                frames = {"tail": rate + 1, "short_one_frame": 1,
                          "short_partial_hop": rate // 10 - 1}[case]
                samples = array("h", [0]) * (frames - 1) + array("h", [32767])
            else:
                # A quarter-rate sine sampled at 45 degrees has sample maxima
                # A/sqrt(2) but reconstructed maxima A. The 10 ms edge ramps
                # suppress truncation transients; PCM16 never exceeds full scale
                # even for A=1.4, which tests floating-point interpolation headroom.
                amplitude = 1.4 if case == "over_full_scale" else 0.5
                samples = array("h")
                for frame in range(rate):
                    taper = min(1.0, frame / (rate // 100),
                                (rate - frame - 1) / (rate // 100))
                    value = round(32768 * amplitude * math.sin(math.pi * frame / 2 + math.pi / 4) * taper)
                    if channels == 2:
                        samples.append(0)
                    samples.append(value)
            yield f"true_peak_{rate}_{case}", case, rate, channels, samples


def check_measurements(name, rate, channels, samples, report):
    case = name.removeprefix(f"true_peak_{rate}_")
    method = report["method"]
    assert method["true_peak_algorithm"]["kind"] == "swr_4x_astats"
    assert method["true_peak_padding_frames"] == rate // 10
    assert method["true_peak_target_sample_rate_hz"] == rate * 4
    expected_filter = (
        f"apad=pad_len={rate // 10},aresample={rate * 4}:resampler=swr:osf=dblp:tsf=dblp:"
        "filter_size=64:phase_shift=10:linear_interp=0:exact_rational=1:cutoff=1:"
        "filter_type=kaiser:kaiser_beta=9:dither_method=0:async=0,"
        "astats=metadata=0:reset=0:measure_perchannel=none:"
        "measure_overall=Peak_level+Number_of_samples+Number_of_NaNs+Number_of_Infs"
    )
    assert len(report["commands"]) == 2
    assert expected_filter in report["commands"][1]["arguments"]
    frames = len(samples) // channels
    assert report["true_peak"]["unit"] == "decibels_true_peak"
    assert report["true_peak"]["scope"] == {"channels": list(range(channels)), "stem_id": None}
    for index, channel in enumerate(report["channels"]):
        native_peak = max(abs(value) for value in samples[index::channels]) / 32768
        SIGNAL["near"](channel["sample_peak_ratio"], native_peak, 1e-12)
    if case == "silence":
        SIGNAL["unavailable"](report["true_peak"], "silent_input")
        expected_peak = None
    else:
        expected_amplitude = (1.4 if case == "over_full_scale" else 0.5) if case in {
            "sine", "asymmetric", "over_full_scale"} else 32767 / 32768
        expected_peak = 20 * math.log10(expected_amplitude)
        actual = SIGNAL["measured"](report["true_peak"])
        SIGNAL["near"](actual, expected_peak, PEAK_TOLERANCE_DB)
        sample_peak = 20 * math.log10(max(item["sample_peak_ratio"] for item in report["channels"]))
        if case in {"sine", "asymmetric", "over_full_scale"}:
            assert actual > sample_peak + 2.9, (actual, sample_peak)
        if case == "over_full_scale":
            assert actual > 2.9, "interpolated floating-point peaks were clipped"
    assert report["short_term"] == []
    assert report["short_term_status"] == {"kind": "unavailable", "reason": "insufficient_duration"}
    if case.startswith("short_"):
        SIGNAL["unavailable"](report["integrated_loudness"], "insufficient_duration")
    if case == "asymmetric":
        assert report["channels"][0]["sample_peak_ratio"] == 0.0
        SIGNAL["unavailable"](report["channels"][0]["sample_peak"], "silent_input")
    return {"true_peak": report["true_peak"]["value"], "expected_true_peak_dbtp": expected_peak,
            "sample_peak_ratios": [item["sample_peak_ratio"] for item in report["channels"]],
            "expected_interpolated_frames": 4 * (frames + rate // 10),
            "method": method["true_peak_algorithm"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--check-schema", action="store_true",
                        help="Also check every emitted report using an already installed jsonschema package.")
    arguments = parser.parse_args()
    binary = arguments.aniflow.resolve(strict=True)
    validator = None
    representative_report = None
    if arguments.check_schema:
        from jsonschema import Draft202012Validator
        schema = Path(__file__).resolve().parents[1] / "docs/contracts/audio-signal-measurements-v2.schema.json"
        validator = Draft202012Validator(json.loads(schema.read_text()))

    def check(name, rate, channels, samples, report):
        nonlocal representative_report
        if validator is not None:
            validator.validate(report)
        if representative_report is None:
            representative_report = report
        return check_measurements(name, rate, channels, samples, report)

    pins = {tool: SIGNAL["local_tool_pin"](tool) for tool in ("ffmpeg", "ffprobe")}
    results = []
    with tempfile.TemporaryDirectory(prefix="aniflow-true-peak-smoke-") as temporary:
        directory = Path(temporary).resolve()
        pins_path = directory / "tools.json"
        pins_path.write_text(json.dumps({"schema": "aniflow.audio-inspection.configuration/v1",
                                        **pins, "tool_timeout_milliseconds": 30000,
                                        "maximum_tool_output_bytes": 1048576}) + "\n")
        settings_path = directory / "settings.json"
        settings_path.write_text(json.dumps({"schema": "aniflow.audio-signal.configuration/v1"}) + "\n")
        for name, case, rate, channels, samples in fixtures():
            try:
                result = SIGNAL["exercise"](binary, directory / name, pins_path, settings_path,
                                             name, rate, channels, samples,
                                             measurement_check=check, resume=case == "sine")
                results.append(result)
            except Exception as error:
                raise RuntimeError(f"{name}: {error}") from error
    receipt = {"schema": "aniflow.high-rate-true-peak-smoke/v1", "synthetic_only": True,
               "checked_at_utc": datetime.now(timezone.utc).isoformat(), "platform": platform.platform(),
               "aniflow_sha256": SIGNAL["sha256"](binary), "tools": pins, "fixtures": results,
               "true_peak_absolute_tolerance_db": PEAK_TOLERANCE_DB,
               "representative_report": representative_report,
               "independent_schema_checked": validator is not None,
               "qualification": "Only these generated fixtures on these pinned local tools; no EBU compliance or release qualification."}
    if arguments.receipt:
        arguments.receipt.write_text(json.dumps(receipt, indent=2, allow_nan=False) + "\n")
    print(f"High-rate true-peak smoke passed: {len(results)} synthetic fixtures", flush=True)


if __name__ == "__main__":
    main()
