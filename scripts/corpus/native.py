"""Explicit, bounded codec checks over generated sources. No downloads."""

from __future__ import annotations

import json
from pathlib import Path
import struct
import time

from .process import execute, require_passed
from .storage import digest, regular_bytes
from .profiles import NATIVE_PROFILES

MAX_ALLOC = str(32 * 1024 * 1024)


def tool_identity(path, expected_name, output, deadline, steps):
    if path is None or not path.is_absolute():
        raise ValueError(expected_name + " requires an explicit absolute executable path")
    # Resolve once and pin actual executable bytes; retain requested locator separately.
    resolved = path.resolve(strict=True)
    data = regular_bytes(resolved, 256 * 1024 * 1024)
    result = execute([resolved, "-version"], cwd=output, logs=output / "logs", name=expected_name + "-version",
                     deadline=min(deadline, time.monotonic() + 10), budget_root=output)
    steps.append(result)
    require_passed(result)
    version = regular_bytes(output / "logs" / result["stdout"]["path"]).decode("utf-8").splitlines()[0]
    if not version.lower().startswith(expected_name + " version "):
        raise ValueError("unexpected tool version banner")
    return {"requested_path": str(path), "resolved_path": str(resolved), "sha256": digest(data), "version": version}


def inspect_case(case, source, output, tools, deadline, steps):
    name = case["id"]
    ffmpeg = tools["ffmpeg"]["resolved_path"]
    ffprobe = tools["ffprobe"]["resolved_path"]
    oracle = case["oracle"]
    before = regular_bytes(source)
    probe = execute([ffprobe, "-v", "error", "-max_alloc", MAX_ALLOC, "-count_frames", "-show_streams", "-of", "json", source],
                    cwd=output, logs=output / "logs", name=name + "-probe", deadline=min(deadline, time.monotonic() + 15), budget_root=output)
    steps.append(probe)
    is_audio = case["family"] == "audio"
    decoded = output / (name + (".pcm" if is_audio else ".rgba"))
    command = [ffmpeg, "-v", "error", "-nostdin", "-xerror", "-max_alloc", MAX_ALLOC, "-threads", "1", "-i", source,
               "-map", "0:a:0" if is_audio else "0:v:0", "-threads", "1"]
    if is_audio:
        command += ["-f", "s16le", "-c:a", "pcm_s16le", decoded]
    else:
        command += ["-frames:v", "16", "-f", "rawvideo", "-pix_fmt", "rgba", decoded]
    decode = execute(command, cwd=output, logs=output / "logs", name=name + "-decode",
                     deadline=min(deadline, time.monotonic() + 15), budget_root=output)
    steps.append(decode)
    if regular_bytes(source) != before:
        raise ValueError("native tool changed source: " + name)
    if oracle["outcome"] == "reject":
        # Failure must be a completed decoder refusal; hangs/missing tools are not a pass.
        if (decode["state"] != "failed" or decode["exit_code"] is None
                or decode["exit_code"] <= 0 or decode["stderr"]["bytes"] == 0
                or decode["message"] is not None or probe["message"] is not None
                or probe["state"] in ("unavailable", "timed_out")
                or (probe["exit_code"] is not None and probe["exit_code"] < 0)):
            raise ValueError("malformed input did not yield bounded decoder refusal: " + name)
        return {"id": name, "state": "passed", "diagnostic": "native_decode_refused", "source_sha256": digest(before)}
    require_passed(probe)
    require_passed(decode)
    report = json.loads(regular_bytes(output / "logs" / probe["stdout"]["path"]))
    streams = report.get("streams", [])
    if len(streams) != 1:
        raise ValueError("fixture must contain exactly one stream: " + name)
    for key, expected in oracle["properties"].items():
        if streams[0].get(key) != expected:
            raise ValueError(f"{name}: {key} expected {expected!r}, got {streams[0].get(key)!r}")
    data = regular_bytes(decoded)
    params = case["parameters"]
    if is_audio:
        import io
        import wave
        with wave.open(io.BytesIO(before), "rb") as waveform:
            samples = waveform.readframes(waveform.getnframes())
        bits = params["bits"]
        if bits == 8:
            expected = b"".join(struct.pack("<h", (sample - 128) << 8) for sample in samples)
        elif bits == 24:
            expected = b"".join(struct.pack("<h", int.from_bytes(samples[index:index + 3], "little", signed=True) >> 8)
                                for index in range(0, len(samples), 3))
        else:
            expected = samples
        if data != expected:
            raise ValueError("decoded sample identity mismatch: " + name)
    else:
        width, height = streams[0]["width"], streams[0]["height"]
        count = params.get("frames", 1)
        if len(data) != width * height * 4 * count:
            raise ValueError("decoded frame cardinality/size mismatch: " + name)
        if case["family"] == "image":
            import zlib
            offset, compressed, color = 8, b"", before[25]
            while offset < len(before):
                length = struct.unpack(">I", before[offset:offset + 4])[0]
                if before[offset + 4:offset + 8] == b"IDAT":
                    compressed += before[offset + 8:offset + 8 + length]
                offset += length + 12
            rows = zlib.decompress(compressed)
            channels = 4 if color == 6 else 3
            stride = width * channels + 1
            expected_pixels = bytearray()
            for row in range(height):
                for column in range(width):
                    start = row * stride + 1 + column * channels
                    expected_pixels.extend(rows[start:start + channels])
                    if channels == 3:
                        expected_pixels.append(255)
            if data != expected_pixels:
                raise ValueError("decoded pixel identity mismatch: " + name)
    return {"id": name, "state": "passed", "diagnostic": None, "source_sha256": digest(before),
            "decoded_sha256": digest(data), "decoded_bytes": len(data), "stream": streams[0]}


def qualify_native(catalog, bundle, output, ffmpeg, ffprobe, deadline, steps):
    if __import__("os").name != "posix":
        raise ValueError("native qualification requires process-group termination support on Unix")
    tools = {"ffmpeg": tool_identity(ffmpeg, "ffmpeg", output, deadline, steps),
             "ffprobe": tool_identity(ffprobe, "ffprobe", output, deadline, steps)}
    results = []
    for case in catalog["cases"]:
        if case["oracle"]["kind"] != "native_decode":
            continue
        source = bundle / case["id"] / case["inventory"]["files"][0]["path"]
        results.append(inspect_case(case, source, output, tools, deadline, steps))
    results.extend(encode_profiles(bundle, output, tools, deadline, steps))
    # Pin executables both before and after use; version banners alone are insufficient.
    for identity in tools.values():
        if digest(regular_bytes(Path(identity["resolved_path"]), 256 * 1024 * 1024)) != identity["sha256"]:
            raise ValueError("native tool changed during qualification")
    return tools, results


def encode_profiles(bundle, output, tools, deadline, steps):
    video = bundle / "ani.corpus.video.cfr-fractional/input.y4m"
    audio = bundle / "ani.corpus.audio.fractional-video-span/input.wav"
    source_digests = {str(path): digest(regular_bytes(path)) for path in (video, audio)}
    results = []
    for profile in NATIVE_PROFILES:
        name = profile["id"]
        replicas = []
        for iteration in range(2):
            encoded = output / f"{name}-{iteration}.media"
            command = [tools["ffmpeg"]["resolved_path"], "-v", "error", "-nostdin", "-xerror",
                       "-max_alloc", MAX_ALLOC, "-threads", "1", "-i", video]
            if profile["audio_streams"]:
                command += ["-i", audio]
            for _ in range(profile["video_streams"]):
                command += ["-map", "0:v:0"]
            if profile["audio_streams"]:
                command += ["-map", "1:a:0", "-c:a", "pcm_s16le"]
            command += ["-map_metadata", "-1", "-fflags", "+bitexact", "-flags:v", "+bitexact",
                        "-c:v", "libx264", "-threads", "1", "-bf", str(profile["b_frames"]),
                        "-pix_fmt", "yuv420p", "-vf", "setsar=" + profile["sample_aspect_ratio"].replace(":", "/"),
                        "-f", profile["container"], encoded]
            step = execute(command, cwd=output, logs=output / "logs", name=f"{name}-encode-{iteration}",
                           deadline=min(deadline, time.monotonic() + 20), budget_root=output, maximum_bytes=128 * 1024**2)
            steps.append(step)
            require_passed(step)
            replicas.append(regular_bytes(encoded))
        if replicas[0] != replicas[1]:
            raise ValueError("same-toolchain encoded generation drift: " + name)
        encoded = output / f"{name}-0.media"
        step = execute([tools["ffprobe"]["resolved_path"], "-v", "error", "-max_alloc", MAX_ALLOC,
                        "-count_frames", "-show_streams", "-of", "json", encoded],
                       cwd=output, logs=output / "logs", name=name + "-probe", deadline=min(deadline, time.monotonic() + 15),
                       budget_root=output, maximum_bytes=128 * 1024**2)
        steps.append(step)
        require_passed(step)
        streams = json.loads(regular_bytes(output / "logs" / step["stdout"]["path"]))["streams"]
        if (sum(stream["codec_type"] == "video" for stream in streams) != profile["video_streams"]
                or sum(stream["codec_type"] == "audio" for stream in streams) != profile["audio_streams"]):
            raise ValueError("encoded stream cardinality mismatch: " + name)
        decoded_records = []
        for stream in streams:
            index = stream["index"]
            target = output / f"{name}-stream-{index}.raw"
            command = [tools["ffmpeg"]["resolved_path"], "-v", "error", "-nostdin", "-xerror",
                       "-max_alloc", MAX_ALLOC, "-threads", "1", "-i", encoded, "-map", "0:" + str(index), "-threads", "1"]
            if stream["codec_type"] == "video":
                if (stream.get("width"), stream.get("height"), stream.get("nb_read_frames"), stream.get("sample_aspect_ratio")) != (4, 4, "4", profile["sample_aspect_ratio"]):
                    raise ValueError("encoded video geometry/cardinality/SAR mismatch: " + name)
                command += ["-frames:v", "8", "-f", "rawvideo", "-pix_fmt", "rgba", target]
                expected_bytes = 4 * 4 * 4 * 4
            else:
                command += ["-f", "s16le", "-c:a", "pcm_s16le", target]
                expected_bytes = 8008 * 2
            step = execute(command, cwd=output, logs=output / "logs", name=f"{name}-decode-{index}",
                           deadline=min(deadline, time.monotonic() + 15), budget_root=output, maximum_bytes=128 * 1024**2)
            steps.append(step)
            require_passed(step)
            data = regular_bytes(target)
            if len(data) != expected_bytes:
                raise ValueError("encoded artifact decode length mismatch: " + name)
            if stream["codec_type"] == "audio" and data != b"\x00" * expected_bytes:
                raise ValueError("encoded silence changed PCM identity")
            decoded_records.append({"stream_index": index, "sha256": digest(data), "size_bytes": len(data)})
        results.append({"id": name, "state": "passed", "encoded_sha256": digest(replicas[0]),
                        "same_toolchain_repeat_equal": True, "streams": streams, "decoded": decoded_records})
    for path, expected in source_digests.items():
        if digest(regular_bytes(path)) != expected:
            raise ValueError("encoding changed a source fixture")
    return results
