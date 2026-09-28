#!/usr/bin/env python3
"""Exercise local pinned tools through audio plan/inspect/status/resume on synthetic WAV."""

from __future__ import annotations

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import wave


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def local_tool_pin(name: str) -> dict[str, str]:
    # Discovery is confined to this smoke helper; normal inspection requires pins.
    discovered = shutil.which(name)
    if discovered is None:
        raise RuntimeError(f"audio inspection smoke requires installed {name}")
    executable = Path(discovered).resolve(strict=True)
    result = subprocess.run(
        [str(executable), "-version"],
        check=True,
        capture_output=True,
        text=True,
        timeout=15,
    )
    banner = result.stdout.splitlines()[0].split()
    if len(banner) < 3 or banner[:2] != [name, "version"]:
        raise RuntimeError(f"unrecognized {name} version banner")
    return {
        "executable": str(executable),
        "version": banner[2],
        "sha256": sha256(executable),
    }


def run_json(binary: Path, command: str, *arguments: str) -> dict:
    result = subprocess.run(
        [str(binary), "--output", "json", *arguments],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"{command} failed ({result.returncode}): {result.stderr[-4096:]}"
        )
    envelope = json.loads(result.stdout)
    assert envelope["schema_version"] == 1
    assert envelope["command"] == command
    assert envelope["status"] == "success"
    return envelope["result"]


def exercise_profile(
    binary: Path, directory: Path, configuration: Path, rate: int, channels: int
) -> None:
    frames = 257
    source = directory / f"synthetic {rate} Hz {channels} channels.wav"
    pcm = b"".join(
        struct.pack(
            "<h",
            0 if channels == 1 else (1200 if (frame // 8 + channel) % 2 == 0 else -1200),
        )
        for frame in range(frames)
        for channel in range(channels)
    )
    with wave.open(str(source), "wb") as audio:
        audio.setnchannels(channels)
        audio.setsampwidth(2)
        audio.setframerate(rate)
        audio.writeframes(pcm)
    source_bytes = source.read_bytes()
    source_sha256 = hashlib.sha256(source_bytes).hexdigest()
    pcm_sha256 = hashlib.sha256(pcm).hexdigest()
    shared = ["--input", str(source), "--configuration", str(configuration)]
    output_directory = directory / f"runs-{rate}-{channels}"

    plan = run_json(binary, "audio_plan", "audio", "plan", *shared)
    assert plan["schema"] == "aniflow.pipeline-plan/v1"
    assert not output_directory.exists(), "planning created a run directory"
    assert source.read_bytes() == source_bytes

    outcome = run_json(
        binary,
        "audio_inspect",
        "audio",
        "inspect",
        *shared,
        "--output-directory",
        str(output_directory),
    )
    assert outcome["schema"] == "aniflow.pipeline-run-outcome/v1"
    assert outcome["executed_stages"] == ["inspect_audio"]
    assert outcome["reused_stages"] == []
    outputs = {output["id"]: output for output in outcome["outputs"]}
    assert set(outputs) == {"technical", "analysis"}
    for output in outputs.values():
        assert sha256(Path(output["path"])) == output["sha256"]
    technical = json.loads(Path(outputs["technical"]["path"]).read_text())
    analysis = json.loads(Path(outputs["analysis"]["path"]).read_text())
    duration = Fraction(frames, rate)
    assert technical["schema"] == "aniflow.audio-technical-inspection/v1"
    assert technical["source"]["sha256"] == source_sha256
    assert technical["source"]["byte_size"] == len(source_bytes)
    assert technical["container"] == "wav"
    assert technical["codec"] == "pcm_s16le"
    assert technical["sample_format"] == "s16"
    assert technical["stream_index"] == 0
    assert technical["frame_count"] == frames
    assert technical["sample_rate_hz"] == rate
    assert technical["channels"] == channels
    assert technical["duration"] == {
        "numerator": duration.numerator,
        "denominator": duration.denominator,
    }
    assert technical["pcm_bitrate_bits_per_second"] == rate * channels * 16
    assert technical["pcm_sha256"] == pcm_sha256
    assert technical["decoded_pcm_sha256"] == pcm_sha256
    assert technical["decode_complete"] is True
    assert technical["source_unchanged"] is True
    assert analysis["schema"] == "aniflow.audio-analysis/v1"
    assert analysis["status"] == "complete"
    assert analysis["source"]["artifact"]["sha256"] == source_sha256
    assert analysis["source"]["sample_rate_hz"] == rate
    assert analysis["source"]["channels"] == channels
    assert analysis["source"]["frame_count"] == frames
    assert analysis["source"]["origin"] == {"numerator": 0, "denominator": 1}
    assert source.read_bytes() == source_bytes

    run_directory = outcome["run_directory"]
    status = run_json(binary, "status_v3", "status-v3", run_directory)
    assert status["payload"]["state"] == "complete"
    resumed = run_json(
        binary, "audio_resume", "audio", "resume", run_directory, *shared
    )
    assert resumed["executed_stages"] == []
    assert resumed["reused_stages"] == ["inspect_audio"]
    assert resumed["outputs"] == outcome["outputs"]
    assert source.read_bytes() == source_bytes
    print(f"Audio inspection smoke passed: {rate} Hz, {channels} channels, {frames} frames")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aniflow", type=Path, required=True)
    arguments = parser.parse_args()
    binary = arguments.aniflow.resolve(strict=True)
    pins = {name: local_tool_pin(name) for name in ["ffmpeg", "ffprobe"]}
    with tempfile.TemporaryDirectory(prefix="aniflow-audio-smoke-") as temporary:
        directory = Path(temporary).resolve()
        configuration = directory / "tool-pins.json"
        configuration.write_text(
            json.dumps(
                {
                    "schema": "aniflow.audio-inspection.configuration/v1",
                    **pins,
                    "tool_timeout_milliseconds": 30000,
                    "maximum_tool_output_bytes": 65536,
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        for rate, channels in [(8000, 1), (44100, 2)]:
            exercise_profile(binary, directory, configuration, rate, channels)


if __name__ == "__main__":
    main()
