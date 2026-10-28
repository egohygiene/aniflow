#!/usr/bin/env python3
"""Generate small, watchable synthetic inputs; never run aniflow or its tests."""

from __future__ import annotations

import argparse
from fractions import Fraction
import json
import os
from pathlib import Path
import sys
import time

from corpus.native import tool_identity
from corpus.process import execute, require_passed, tree_bytes
from corpus.storage import digest, regular_bytes, safe_new_directory

ROOT = Path(__file__).resolve().parents[1]
RATE = 24
FRAMES = 48
CLIPS = (("landscape", 320, 180), ("portrait", 180, 320))
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_TOOL_BYTES = 128 * 1024 * 1024
MAX_OUTPUT_BYTES = 64 * 1024 * 1024
MAX_ALLOC = str(32 * 1024 * 1024)
COLOR = {"pix_fmt": "yuv420p", "sample_aspect_ratio": "1:1", "color_range": "tv",
         "color_space": "bt709", "color_primaries": "bt709", "color_transfer": "bt709"}


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


def y4m_bytes(width, height):
    """Integer-only limited-range YUV bars, moving square and progress strip."""
    if (not isinstance(width, int) or not isinstance(height, int)
            or not 2 <= width <= 320 or not 2 <= height <= 320
            or width % 2 or height % 2 or width * height > 57_600):
        raise ValueError("demo dimensions must be bounded, positive and even")
    palette = ((63, 102, 240), (173, 42, 26), (32, 240, 118), (219, 16, 138),
               (188, 154, 16), (78, 214, 230), (16, 128, 128), (235, 128, 128))
    data = bytearray(f"YUV4MPEG2 W{width} H{height} F{RATE}:1 Ip A1:1 C420jpeg XCOLORRANGE=LIMITED\n".encode())
    size = max(2, min(width, height) // 5 // 2 * 2)
    strip = max(2, min(height, 12))
    for ordinal in range(FRAMES):
        left = ((width - size) * ordinal // (FRAMES - 1)) // 2 * 2
        top = ((height - size) * (FRAMES - 1 - ordinal) // (FRAMES - 1)) // 2 * 2

        def pixel(x, y):
            if y >= height - strip:
                return (235 if x * FRAMES < width * (ordinal + 1) else 16, 128, 128)
            if left <= x < left + size and top <= y < top + size:
                return (235, 128, 128)
            return palette[min(7, x * 8 // width)]

        data.extend(b"FRAME\n")
        data.extend(pixel(x, y)[0] for y in range(height) for x in range(width))
        for channel in (1, 2):
            data.extend(pixel(x, y)[channel] for y in range(0, height, 2) for x in range(0, width, 2))
    if len(data) > MAX_FILE_BYTES:
        raise ValueError("synthetic raw video exceeds the demo bound")
    return bytes(data)


def encode_arguments(ffmpeg, source, destination):
    return [str(ffmpeg), "-hide_banner", "-nostdin", "-v", "error", "-xerror", "-n",
            "-max_alloc", MAX_ALLOC, "-threads", "1", "-protocol_whitelist", "file,pipe",
            "-f", "yuv4mpegpipe", "-color_range", "tv", "-colorspace", "bt709",
            "-color_primaries", "bt709", "-color_trc", "bt709", "-i", str(source),
            "-map", "0:v:0", "-an", "-sn", "-dn", "-map_metadata", "-1", "-map_chapters", "-1",
            "-vf", "setsar=1,setfield=prog", "-c:v", "libx264", "-preset", "veryfast", "-crf", "18",
            "-threads:v", "1", "-bf", "0", "-g", str(RATE), "-pix_fmt", "+yuv420p",
            "-color_range", "tv", "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
            "-fps_mode:v", "passthrough", "-enc_time_base:v", "1/24", "-video_track_timescale", "24000",
            "-avoid_negative_ts", "disabled", "-use_editlist", "1", "-fflags", "+bitexact",
            "-flags:v", "+bitexact", "-movflags", "+faststart", "-f", "mp4", str(destination)]


def integer(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        raise ValueError(label + " must be an integer observation")
    if isinstance(value, str) and (len(value) > 32 or not value.lstrip("-").isdigit()):
        raise ValueError(label + " must be an integer observation")
    return int(value)


def rational(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, str)) or len(str(value)) > 64:
        raise ValueError(label + " must be a bounded rational observation")
    try:
        return Fraction(value)
    except (ValueError, ZeroDivisionError) as error:
        raise ValueError(label + " is not a rational observation") from error


def require_side_data(document, permitted):
    entries = document.get("side_data_list", [])
    if not isinstance(entries, list) or any(not isinstance(entry, dict)
            or entry.get("side_data_type") not in permitted for entry in entries):
        raise ValueError("generated video has unsupported transform/color side data")


def validate_observation(report, width, height):
    """Check the generated fixture's declared/decoded shape, not aniflow acceptance."""
    if not isinstance(report, dict):
        raise ValueError("FFprobe observation must be an object")
    streams = report.get("streams", [])
    frames = report.get("frames", [])
    if not isinstance(streams, list) or len(streams) != 1 or not isinstance(frames, list) or len(frames) != FRAMES:
        raise ValueError("demo must have exactly one video stream and 48 decoded frames")
    stream = streams[0]
    if not isinstance(stream, dict) or any(not isinstance(frame, dict) for frame in frames):
        raise ValueError("FFprobe stream and frame observations must be objects")
    expected = {"index": 0, "codec_type": "video", "codec_name": "h264", "width": width,
                "height": height, "field_order": "progressive", **COLOR}
    if any(stream.get(key) != value for key, value in expected.items()):
        raise ValueError("generated video differs from the intended geometry, codec or color profile")
    if (integer(stream.get("nb_read_frames"), "frame count") != FRAMES
            or rational(stream.get("r_frame_rate"), "frame rate") != RATE
            or rational(stream.get("avg_frame_rate"), "average frame rate") != RATE):
        raise ValueError("generated video frame count or frame rate differs")
    time_base = rational(stream.get("time_base"), "video time base")
    if (time_base <= 0 or integer(stream.get("start_pts"), "video start") != 0
            or integer(stream.get("duration_ts"), "video duration") * time_base != Fraction(FRAMES, RATE)):
        raise ValueError("generated video has an offset or different duration")
    tags = stream.get("tags", {})
    if not isinstance(tags, dict) or ("rotate" in tags and tags["rotate"] != "0"):
        raise ValueError("generated video must not be rotated")
    require_side_data(stream, {"CPB properties"})
    for ordinal, frame in enumerate(frames):
        if (integer(frame.get("stream_index"), "frame stream") != 0
                or integer(frame.get("pts"), "frame timestamp") * time_base != Fraction(ordinal, RATE)
                or integer(frame.get("duration", frame.get("pkt_duration")), "frame duration") * time_base != Fraction(1, RATE)
                or integer(frame.get("interlaced_frame"), "frame scan") != 0
                or frame.get("width") != width or frame.get("height") != height
                or any(frame.get(key) != value for key, value in COLOR.items())):
            raise ValueError("decoded frame geometry, clock or color differs from the demo profile")
        for field in ("crop_top", "crop_bottom", "crop_left", "crop_right"):
            if integer(frame.get(field), field) != 0:
                raise ValueError("generated video contains cropping")
        require_side_data(frame, {"H.26[45] User Data Unregistered SEI message"})
    container = report.get("format", {})
    if (not isinstance(container, dict) or container.get("format_name") != "mov,mp4,m4a,3gp,3g2,mj2"
            or rational(container.get("start_time"), "container start") != 0
            or rational(container.get("duration"), "container duration") != Fraction(FRAMES, RATE)
            or report.get("chapters", []) != []):
        raise ValueError("generated MP4 has an unexpected container clock or chapters")
    return {"width": width, "height": height, "frames": FRAMES, "frame_rate": "24/1",
            "duration": "2/1", "audio": "none", "color": COLOR.copy(),
            "scope": "FFprobe stream and decoded-frame observations; aniflow acceptance is not run"}


def source_configuration(tools):
    pins = {name: {"executable": tools[name]["resolved_path"], "version": "8.0.1",
                   "sha256": tools[name]["sha256"]} for name in ("ffmpeg", "ffprobe")}
    # Small explicit bounds avoid reserving the production profile's very large
    # output ceilings for two-second demos. Existing report/tool allowances apply.
    common = {"maximum_source_bytes": MAX_FILE_BYTES, "maximum_frame_bytes": 1_048_576,
              "maximum_total_frame_bytes": 33_554_432, "maximum_pixels": 57_600}
    native = {"maximum_capture_bytes": 4_194_304, "maximum_working_bytes": 2_147_483_648,
              "minimum_free_bytes": 67_108_864, "timeout_milliseconds": 120_000}
    return {"schema": "aniflow.source-music-video-recipe/v1",
            "extraction": {"schema": "aniflow.frame-extraction-configuration/v1", **pins, **common, **native,
                           "maximum_frames": FRAMES,
                           "source_selection": {"video_stream": 0, "audio_stream": None, "no_audio": True, "discard_streams": []}},
            "recipe": {"schema": "aniflow.music-video-recipe/v1",
                       "repair": {"mode": "disabled", "reason": "Synthetic demo needs no frame repair.",
                                  "import": {"schema": "aniflow.frame-import-configuration/v1",
                                             "ffmpeg": pins["ffmpeg"], **common, "tool_timeout_milliseconds": 120_000}},
                       "upscale": {"mode": "disabled", "reason": "Keep demo geometry; no neural tools or models are selected."},
                       "assembly": {"schema": "aniflow.video-assembly-configuration/v1", **pins, **common, **native,
                                    "maximum_output_bytes": 33_554_432, "delivery_crf": 18, "preset": "veryfast"}}}


def file_record(path):
    data = regular_bytes(path, MAX_FILE_BYTES)
    return {"path": path.name, "size_bytes": len(data), "sha256": digest(data)}


def require_tools_unchanged(tools):
    for identity in tools.values():
        if digest(regular_bytes(Path(identity["resolved_path"]), MAX_TOOL_BYTES)) != identity["sha256"]:
            raise ValueError("selected tool changed during demo generation")


def generate(output, ffmpeg, ffprobe):
    output = safe_new_directory(output, ROOT)
    if os.name != "posix":
        raise ValueError("demo encoding requires Unix process-group support; no tools were launched")
    for name, path in (("ffmpeg", ffmpeg), ("ffprobe", ffprobe)):
        if not Path(path).is_absolute():
            raise ValueError(name + " requires an explicit absolute executable path")
    generator = Path(__file__).resolve()
    generator_sha256 = digest(regular_bytes(generator, MAX_FILE_BYTES))
    output.mkdir()
    deadline = time.monotonic() + 120
    steps = []
    report = {"schema": "aniflow.synthetic-video-demo/v1", "state": "failed", "qualification": "not_run",
              "aniflow_execution": "not_run", "origin": "repository-authored integer Y4M recipes; no external media",
              "license": "MIT", "recipe_version": 1, "steps": steps, "tools": {}, "clips": [], "failure": None,
              "generator": {"path": generator.relative_to(ROOT).as_posix(), "sha256": generator_sha256},
              "python_version": sys.version,
              "limits": {"seconds": 120, "maximum_output_bytes": MAX_OUTPUT_BYTES, "maximum_file_bytes": MAX_FILE_BYTES},
              "evidence_scope": "Generation and FFprobe observations only. Process success does not qualify aniflow, native providers or visual quality."}
    try:
        for name, path in (("ffmpeg", ffmpeg), ("ffprobe", ffprobe)):
            resolved = Path(path).resolve(strict=True)
            if any(ord(char) < 32 or 127 <= ord(char) <= 159 for char in str(resolved)):
                raise ValueError("tool paths must not contain control characters")
            regular_bytes(resolved, MAX_TOOL_BYTES)
            identity = tool_identity(Path(path), name, output, deadline, steps)
            report["tools"][name] = identity
            if identity["version"].split()[:3] != [name, "version", "8.0.1"]:
                raise ValueError(name + " must report the exact 8.0.1 version token")
        require_tools_unchanged(report["tools"])
        for name, width, height in CLIPS:
            raw = output / (name + ".y4m")
            with raw.open("xb") as stream:
                stream.write(y4m_bytes(width, height))
            before = file_record(raw)
            encoded = output / (name + ".mp4")
            step = execute(encode_arguments(report["tools"]["ffmpeg"]["resolved_path"], raw, encoded),
                           cwd=output, logs=output / "logs", name=name + "-encode",
                           deadline=min(deadline, time.monotonic() + 30), budget_root=output, maximum_bytes=MAX_OUTPUT_BYTES)
            steps.append(step)
            require_passed(step)
            if step["stderr"]["bytes"] or file_record(raw) != before:
                raise ValueError("encoding emitted an error diagnostic or changed its raw input")
            media = file_record(encoded)
            command = [report["tools"]["ffprobe"]["resolved_path"], "-v", "error", "-max_alloc", MAX_ALLOC,
                       "-protocol_whitelist", "file,pipe", "-count_frames", "-show_streams", "-show_format", "-show_chapters",
                       "-show_frames", "-show_entries",
                       "frame=stream_index,pts,duration,pkt_duration,width,height,pix_fmt,sample_aspect_ratio,color_range,color_space,color_primaries,color_transfer,interlaced_frame,crop_top,crop_bottom,crop_left,crop_right:frame_side_data:stream:format:chapter",
                       "-of", "json", "-i", str(encoded)]
            step = execute(command, cwd=output, logs=output / "logs", name=name + "-probe",
                           deadline=min(deadline, time.monotonic() + 20), budget_root=output, maximum_bytes=MAX_OUTPUT_BYTES)
            steps.append(step)
            require_passed(step)
            if step["stderr"]["bytes"]:
                raise ValueError("FFprobe emitted an error diagnostic")
            observation = json.loads(regular_bytes(output / "logs" / step["stdout"]["path"], 4_194_304))
            properties = validate_observation(observation, width, height)
            if file_record(encoded) != media or file_record(raw) != before:
                raise ValueError("demo inputs changed during observation")
            require_tools_unchanged(report["tools"])
            report["clips"].append({"id": name, "raw": before, "media": media, "observation": properties,
                                    "probe_stdout": step["stdout"]})
        # Publish usable configurations only after both requested clips have the
        # expected generated observations. The manifest is written last.
        configuration = json_bytes(source_configuration(report["tools"]))
        for clip in report["clips"]:
            path = output / (clip["id"] + ".recipe.json")
            with path.open("xb") as stream:
                stream.write(configuration)
            clip["configuration"] = file_record(path)
        require_tools_unchanged(report["tools"])
        if digest(regular_bytes(generator, MAX_FILE_BYTES)) != generator_sha256:
            raise ValueError("demo generator source changed during generation")
        tree_bytes(output, MAX_OUTPUT_BYTES)
        report["state"] = "generated"
    except (ValueError, OSError, KeyError, TypeError, IndexError, UnicodeError) as error:
        report["failure"] = str(error)
    except KeyboardInterrupt:
        report["failure"] = "Generation interrupted; partial output and logs are retained."
    finally:
        with (output / "manifest.json").open("xb") as stream:
            stream.write(json_bytes(report))
    print(json.dumps({"state": report["state"], "output": str(output), "manifest": str(output / "manifest.json"),
                      "qualification": "not_run", "guide": str(ROOT / "docs/video-demo.md")}, ensure_ascii=False))
    return 0 if report["state"] == "generated" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="Absent output directory outside the checkout; parent must exist.")
    parser.add_argument("--ffmpeg", required=True, type=Path, help="Explicit local FFmpeg 8.0.1 executable; no download.")
    parser.add_argument("--ffprobe", required=True, type=Path, help="Explicit local ffprobe 8.0.1 executable; no download.")
    args = parser.parse_args()
    try:
        return generate(args.output, args.ffmpeg, args.ffprobe)
    except (ValueError, OSError) as error:
        print("Demo generation refused: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
