#!/usr/bin/env python3
"""Run the bounded synthetic FFmpeg 8.0.1 source-recipe/manual-SRT pilot.

The existing recipe, audio, lyrics and timed-text owners retain authority.
No word/timing/creative approval, release, signing or publication is generated.
The reused watchdog monitors CLI process groups; descendant cancellation is
not qualified by this pilot and native providers retain their own bounds.
"""

from __future__ import annotations

import argparse
from fractions import Fraction
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys
import time

from corpus.process import execute, require_passed, tree_bytes
from corpus.storage import regular_bytes, safe_new_directory

ROOT = Path(__file__).resolve().parents[1]
MAX_FILE = 16 * 1024 * 1024
MAX_OUTPUT = 512 * 1024 * 1024
MAX_TOOL = 128 * 1024 * 1024
MAX_REPORT = 65536
VIDEO_FRAMES = 64
VIDEO_DURATION = Fraction(8, 3)
AUDIO_SAMPLES = 128000
REQUIRED_FEATURES = {
    "filters": {"testsrc2", "sine", "atrim", "format", "scale", "setsar",
                "aformat", "aresample", "ametadata", "ebur128", "apad"},
    "muxers": {"mp4", "wav", "image2", "framehash", "hash", "null"},
    "encoders": {"libx264", "png", "ffv1", "rawvideo", "pcm_f32le", "aac",
                 "wrapped_avframe", "pcm_s16le"},
    "decoders": {"h264", "aac", "png", "ffv1", "pcm_f32le"},
    "demuxers": {"mov", "wav", "image2", "lavfi"},
    "protocols": {"input:file", "input:pipe", "output:file", "output:pipe"},
}
EXPECTED_CUES = [(Fraction(1, 4), Fraction(7, 8), "Synthetic cue one"),
                 (Fraction(9, 8), Fraction(7, 4), "Synthetic cue two — 日本語")]
MANUAL_SRT = ("1\n00:00:00,250 --> 00:00:00,875\nSynthetic cue one\n\n"
              "2\n00:00:01,125 --> 00:00:01,750\nSynthetic cue two — 日本語\n").encode()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


def file_record(path, maximum=MAX_FILE):
    data = regular_bytes(path, maximum)
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def require_preserved_files(records, message):
    for path, expected in records.items():
        require(file_record(path) == expected, message)


def read_json(path):
    return json.loads(regular_bytes(path, 4 * 1024 * 1024))


def contained_file(value, root):
    path = Path(value)
    require(path.is_absolute() and not path.is_symlink(), "expected an absolute retained regular output")
    resolved = path.resolve(strict=True)
    require(root in resolved.parents, "CLI output escaped its pilot workspace")
    file_record(resolved)
    return resolved


def machine_result(path, command):
    value = read_json(path)
    require(value.get("schema_version") == 1 and value.get("command") == command
            and value.get("status") == "success" and isinstance(value.get("result"), dict),
            "CLI did not return the selected successful machine contract")
    return value["result"]


def machine_error(path, command):
    value = read_json(path)
    require(value.get("schema_version") == 1 and value.get("command") == command
            and value.get("status") == "error" and isinstance(value.get("error"), dict),
            "expected the selected structured domain refusal, not a parser or process failure")
    return value["error"]


def feature_names(kind, text):
    """Read FFmpeg's bounded feature tables, excluding headers/descriptions."""
    names, section = set(), None
    for line in text.splitlines():
        parts = line.split()
        if kind == "protocols":
            if line in ("Input:", "Output:"):
                section = line[:-1].lower()
            elif section and len(parts) == 1 and re.fullmatch(r"[a-z0-9_]+", parts[0]):
                names.add(section + ":" + parts[0])
        elif len(parts) >= 3 and parts[1] != "=":
            if kind == "filters" and re.fullmatch(r"[A-Z.]{2,3}", parts[0]):
                names.add(parts[1])
            elif kind in ("encoders", "decoders") and re.fullmatch(r"[VAS][A-Z.]{5}", parts[0]):
                names.add(parts[1])
            elif kind in ("muxers", "demuxers") and parts[0] in ("D", "E", "DE"):
                names.update(parts[2 if parts[1] == "d" else 1].split(","))
    return names


def cues_from_text(text):
    require(len(text.encode()) <= 65536, "timed-text observation exceeds bound")
    text = text.replace("\r\n", "\n").strip()
    if text.startswith("WEBVTT\n"):
        text = text[len("WEBVTT\n"):].strip()
    pattern = re.compile(r"^(\d{2}):(\d{2}):(\d{2})[,.](\d{3}) --> "
                         r"(\d{2}):(\d{2}):(\d{2})[,.](\d{3})$")
    result = []
    for block in text.split("\n\n"):
        lines = block.splitlines()
        if len(lines) > 1 and " --> " not in lines[0]:
            lines = lines[1:]
        require(len(lines) >= 2, "missing cue interval or supplied words")
        match = pattern.fullmatch(lines[0])
        require(match is not None, "unsupported or changed cue interval")
        fields = [int(part) for part in match.groups()]
        require(all(fields[i] < 60 for i in (1, 2, 5, 6)), "invalid cue clock")
        start, end = [Fraction(h * 3600000 + m * 60000 + s * 1000 + ms, 1000)
                      for h, m, s, ms in (fields[:4], fields[4:])]
        result.append((start, end, "\n".join(lines[1:])))
    return result


def profile_facts(name="aligned"):
    require(name in ("aligned", "ordinary-mp4"), "unknown bounded synthetic source profile")
    return {"frames": 48, "height": 90, "samples": 96000, "decoded_samples": 96256, "duration": Fraction(2)} if name == "ordinary-mp4" else {"frames": 64, "height": 96, "samples": 128000, "decoded_samples": 128000, "duration": Fraction(8, 3)}


def observe_source_geometry(extraction, source_profile, source_record):
    facts = profile_facts(source_profile)
    media = extraction["media"]
    require(extraction.get("schema") == "aniflow.frame-extraction-report/v1"
            and extraction.get("source_sha256") == source_record["sha256"]
            and extraction.get("source_byte_count") == source_record["bytes"]
            and extraction.get("frame_count") == media.get("frame_count") == facts["frames"]
            and extraction.get("width") == media.get("width") == 160
            and extraction.get("height") == media.get("height") == facts["height"],
            "accepted extraction does not bind the selected complete source/display")
    if source_profile == "ordinary-mp4":
        expected = {"coded_width": 160, "coded_height": 96, "display_width": 160, "display_height": 90,
            "crop_left": 0, "crop_right": 0, "crop_top": 0, "crop_bottom": 6}
        require(media.get("profile") == "aniflow.ffmpeg-8.0.1-rgb8-cfr-source-extraction/coded-display-v2"
                and json_bytes(media.get("source_geometry")) == json_bytes(expected),
                "ordinary source did not exercise the exact admitted coded/display rectangle")
    else:
        require(media.get("profile") == "aniflow.ffmpeg-8.0.1-rgb8-cfr-source-extraction/v1"
                and "source_geometry" not in media, "aligned extraction changed its original profile shape")
    return {"profile": media["profile"], "source_geometry": media.get("source_geometry"),
            "frame_count": facts["frames"]}


def media_summary(document, subtitles, codec="h264", pixel_format="yuv420p", source_profile="aligned"):
    facts = profile_facts(source_profile)
    streams = document.get("streams")
    require(isinstance(streams, list) and len(streams) == (3 if subtitles else 2),
            "unexpected stream inventory")
    selected = {}
    for kind in ("video", "audio", *(["subtitle"] if subtitles else [])):
        matches = [stream for stream in streams if stream.get("codec_type") == kind]
        require(len(matches) == 1, "missing or ambiguous " + kind + " stream")
        selected[kind] = matches[0]
    video, audio = selected["video"], selected["audio"]
    require(video.get("codec_name") == codec and video.get("width") == 160
            and video.get("height") == facts["height"] and video.get("pix_fmt") == pixel_format
            and int(video.get("nb_read_frames", -1)) == facts["frames"]
            and Fraction(video.get("avg_frame_rate", "0")) == 24
            and Fraction(video.get("start_pts", 1)) * Fraction(video["time_base"]) == 0
            and Fraction(video.get("duration_ts", 0)) * Fraction(video["time_base"]) == facts["duration"],
            "video structure or rational clock differs from the selected tiny pilot")
    require(audio.get("codec_name") == "aac" and audio.get("sample_rate") == "48000"
            and audio.get("channels") == 2 and audio.get("channel_layout") == "stereo"
            and audio.get("profile") == "LC"
            and re.fullmatch(r"SHA256:[0-9a-f]{64}", audio.get("extradata_hash", "")) is not None
            and 0 < int(audio.get("extradata_size", 0)) <= 4096,
            "AAC stereo source/candidate contract differs")
    require(Fraction(audio.get("start_pts", 1)) * Fraction(audio["time_base"]) == 0
            and Fraction(audio.get("duration_ts", 0)) * Fraction(audio["time_base"]) == facts["duration"],
            "AAC start or complete duration differs from the selected exact sample grid")
    if subtitles:
        require(selected["subtitle"].get("codec_name") == "mov_text", "manual SRT was not muxed")
    packets = [packet for packet in document.get("packets", [])
               if packet.get("stream_index") == audio["index"]]
    require(packets and len(packets) <= 128, "missing or unbounded AAC packet evidence")
    clock = Fraction(audio["time_base"])
    audio_packets = []
    for packet in packets:
        digest = packet.get("data_hash", "")
        require(re.fullmatch(r"SHA256:[0-9a-f]{64}", digest) is not None,
                "AAC packet lacks an observed byte digest")
        side_data = packet.get("side_data_list", [])
        require(isinstance(side_data, list) and len(side_data) <= 4, "unbounded AAC side data")
        for entry in side_data:
            require(isinstance(entry, dict) and set(entry) == {"side_data_type", "skip_samples",
                "discard_padding", "skip_reason", "discard_reason"}
                and entry["side_data_type"] == "Skip Samples"
                and all(type(entry[key]) is int and entry[key] >= 0
                        for key in ("skip_samples", "discard_padding", "skip_reason", "discard_reason")),
                "unsupported AAC priming or padding observation")
        audio_packets.append((Fraction(packet["pts"]) * clock,
                              Fraction(packet["dts"]) * clock, Fraction(packet["duration"]) * clock,
                              digest, json.dumps(side_data, sort_keys=True)))
    return {"frames": facts["frames"], "frame_rate": "24/1", "video_duration": str(facts["duration"]),
            "width": 160, "height": facts["height"], "video_codec": codec, "pixel_format": pixel_format,
            "audio_codec": "aac", "sample_rate": 48000,
            "audio_duration": str(facts["duration"]), "audio_duration_samples": facts["samples"],
            "channels": 2, "subtitle_codec": "mov_text" if subtitles else None}, {
                "configuration": (audio["profile"], audio["extradata_hash"], audio["extradata_size"]),
                "packets": audio_packets}


def source_configuration(tools, source_profile="aligned"):
    facts = profile_facts(source_profile)
    pins = {name: {"executable": str(tools[name]), "version": "8.0.1",
                   "sha256": file_record(tools[name], MAX_TOOL)["sha256"]}
            for name in ("ffmpeg", "ffprobe")}
    common = {"maximum_source_bytes": MAX_FILE, "maximum_frame_bytes": 1048576,
              "maximum_total_frame_bytes": 33554432, "maximum_pixels": 57600}
    native = {"maximum_capture_bytes": 4194304, "maximum_working_bytes": 2147483648,
              "minimum_free_bytes": 67108864, "timeout_milliseconds": 120000}
    return {"schema": "aniflow.source-music-video-recipe/v1",
        "extraction": {"schema": "aniflow.frame-extraction-configuration/v1", **pins, **common, **native,
            "maximum_frames": facts["frames"], "source_selection": {"video_stream": 0, "audio_stream": 1,
                "no_audio": False, "discard_streams": []}},
        "recipe": {"schema": "aniflow.music-video-recipe/v1",
            "repair": {"mode": "disabled", "reason": "Synthetic input has no selected repairs.",
                "import": {"schema": "aniflow.frame-import-configuration/v1", "ffmpeg": pins["ffmpeg"],
                    **common, "tool_timeout_milliseconds": 120000}},
            "upscale": {"mode": "disabled", "reason": "Keep generated geometry; no model is selected."},
            "audio_signal": {"maximum_pcm_bytes": 2097152},
            "assembly": {"schema": "aniflow.video-assembly-configuration/v1", **pins, **common, **native,
                "maximum_output_bytes": 33554432, "delivery_crf": 18, "preset": "veryfast"}}}


def run(output, aniflow, ffmpeg, ffprobe, source_profile="aligned"):
    facts = profile_facts(source_profile)
    require(os.name == "posix", "pilot requires Unix process-group control")
    output = safe_new_directory(output, ROOT)
    tools = {name: Path(value).resolve(strict=True) for name, value in
             (("aniflow", aniflow), ("ffmpeg", ffmpeg), ("ffprobe", ffprobe))}
    require(tools["ffmpeg"].name == "ffmpeg" and tools["ffprobe"].name == "ffprobe"
            and tools["ffmpeg"].parent == tools["ffprobe"].parent,
            "selected ffmpeg/ffprobe must use their expected names in one directory")
    for path in tools.values():
        require(path.is_absolute() and os.access(path, os.X_OK), "explicit executable tool required")
        file_record(path, MAX_TOOL)
    output.mkdir(mode=0o700)
    environment = dict(os.environ)
    environment["PATH"] = str(tools["ffmpeg"].parent) + os.pathsep + environment.get("PATH", "")
    started = time.monotonic()
    deadline = started + 120
    steps = []
    report = {"schema": "aniflow.synthetic-publishing-pilot/v1", "state": "failed",
              "selected_profile": "ffmpeg801-source-recipe-manual-srt-cpu", "source_profile": source_profile,
              "product_release_qualified": False, "typed_ffmpeg_8_0_1_profile_qualified": False,
              "human_review_performed": False, "publication_performed": False,
              "harness": file_record(Path(__file__)),
              "steps": steps, "tools": {}, "outputs": {}, "failure": None,
              "limits": {"seconds": 120, "output_bytes": MAX_OUTPUT},
              "remaining": ["broader typed profile/caption coverage", "native supported-host matrix",
                            "manual visual/word/timing/audio approval", "release/install/publication evidence"]}

    def execute_step(name, argv, *, expected_success=True):
        step = execute(argv, cwd=output, logs=output / "logs", name=name, deadline=deadline,
                       environment=environment, budget_root=output, maximum_bytes=MAX_OUTPUT - MAX_REPORT)
        steps.append(step)
        if expected_success:
            require_passed(step)
        return step

    def observe(name, path):
        execute_step(name, [tools["ffprobe"], "-v", "error", "-count_frames", "-show_streams",
                            "-show_format", "-show_packets", "-show_data_hash", "sha256", "-of", "json", path])
        return read_json(output / "logs" / (name + ".stdout"))

    try:
        for name, path in tools.items():
            record = file_record(path, MAX_TOOL)
            execute_step("version-" + name, [path, "--version" if name == "aniflow" else "-version"])
            banner = regular_bytes(output / "logs" / ("version-" + name + ".stdout"), 65536).decode()
            report["tools"][name] = {**record, "version_banner": banner.splitlines()[0]}
            if name in ("ffmpeg", "ffprobe"):
                require(banner.splitlines()[0].split()[:3] == [name, "version", "8.0.1"],
                        "the selected typed pilot requires real FFmpeg/ffprobe 8.0.1")
        report["required_ffmpeg_features"] = {}
        for kind, baseline_required in REQUIRED_FEATURES.items():
            required = baseline_required | ({"crop"} if kind == "filters" and source_profile == "ordinary-mp4" else set())
            name = "features-" + kind
            execute_step(name, [tools["ffmpeg"], "-hide_banner", "-" + kind])
            observed = feature_names(kind, regular_bytes(output / "logs" / (name + ".stdout"),
                                                         4 * 1024 * 1024).decode())
            missing = required - observed
            report["required_ffmpeg_features"][kind] = {"required": sorted(required),
                                                         "missing": sorted(missing)}
            require(not missing, "selected FFmpeg lacks required " + kind + ": " + ", ".join(sorted(missing)))
        source = output / "synthetic source.mp4"
        srt = output / "manual lyrics 日本語.srt"
        srt.write_bytes(MANUAL_SRT)
        execute_step("generate-source", [tools["ffmpeg"], "-hide_banner", "-nostdin", "-v", "error",
                     "-xerror", "-n", "-max_alloc", "33554432", "-f", "lavfi", "-i",
                     "testsrc2=size=160x" + str(facts["height"]) + ":rate=24:duration=3", "-f", "lavfi", "-i",
                     "sine=frequency=440:sample_rate=48000:duration=3", "-map", "0:v:0", "-map", "1:a:0",
                     "-frames:v", str(facts["frames"]), "-af", "atrim=end_sample=" + str(facts["samples"]),
                     "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-bf", "0", "-g", "24",
                     "-x264-params", "colorprim=bt709:transfer=bt709:colormatrix=bt709:range=tv",
                     "-threads:v", "1", "-pix_fmt", "yuv420p", "-color_range", "tv", "-colorspace", "bt709",
                     "-color_primaries", "bt709", "-color_trc", "bt709", "-c:a", "aac", "-ac", "2",
                     "-b:a", "128k", "-movie_timescale", "48000",
                     "-map_metadata", "-1", "-map_chapters", "-1", source])
        originals = {path.name: file_record(path) for path in (source, srt)}
        configuration = output / "source-recipe.json"
        configuration.write_bytes(json_bytes(source_configuration(tools, source_profile)))
        recipe_args = [tools["aniflow"], "--output", "json", "video", "music-video",
                       "--source-video", source, "--configuration", configuration]
        execute_step("run-source-recipe", [*recipe_args, "--output-directory", output / "recipes"])
        outcome = machine_result(output / "logs/run-source-recipe.stdout", "video_music_video")
        require(outcome.get("schema") == "aniflow.source-music-video-outcome/v1",
                "CLI did not return the selected typed source recipe")
        recipe_root = Path(outcome["run_directory"]).resolve(strict=True)
        require(output in recipe_root.parents, "recipe escaped the synthetic workspace")
        extracted = [p for p in outcome["phases"] if p.get("phase") == "extract_frames" and p.get("state") == "complete"]
        require(len(extracted) == 1, "one complete extraction owner required")
        extraction_root = Path(extracted[0]["run_directory"]).resolve(strict=True)
        require(recipe_root in extraction_root.parents, "extraction owner escaped accepted recipe")
        records = [p for p in extracted[0]["manifest"]["payload"]["outputs"] if p.get("id") == "extraction_evidence"]
        require(len(records) == 1 and records[0].get("kind") == "file", "one accepted extraction evidence record required")
        extraction_path = contained_file(str(extraction_root / records[0]["relative_path"]), output)
        extraction_record = file_record(extraction_path)
        require(extraction_record == {"sha256": records[0]["sha256"], "bytes": records[0]["byte_count"]}, "accepted extraction evidence byte identity differs")
        report["source_extraction"] = {**observe_source_geometry(read_json(extraction_path), source_profile, file_record(source)), "evidence": extraction_record}

        outputs = outcome.get("outputs", [])
        report_outputs = {}
        accepted_records = {}
        for artifact_id, codec, pixel in (("h264_delivery", "h264", "yuv420p"),
                                          ("lossless_master", "ffv1", "bgr0")):
            matches = [item for item in outputs if item.get("id") == artifact_id]
            require(len(matches) == 1, "missing or ambiguous accepted recipe output: " + artifact_id)
            artifact = matches[0]
            path = contained_file(artifact["path"], output)
            record = file_record(path)
            accepted_records[path] = record
            require(record["sha256"] == artifact.get("sha256"), "accepted output byte identity differs")
            observed, audio = media_summary(observe("probe-" + artifact_id, path), False, codec, pixel, source_profile)
            report_outputs[artifact_id] = {"path": path.relative_to(output).as_posix(), **record,
                                          "media": observed}
            if artifact_id == "h264_delivery":
                after, after_audio = observed, audio
            else:
                master_audio = audio
            execute_step("decode-" + artifact_id, [tools["ffmpeg"], "-hide_banner", "-nostdin", "-v", "error",
                         "-xerror", "-i", path, "-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
        before, before_audio = media_summary(observe("probe-source", source), False, source_profile=source_profile)
        require(before_audio == after_audio == master_audio,
                "typed recipe changed AAC packet bytes or rational timestamps")
        if source_profile == "ordinary-mp4":
            spec = importlib.util.spec_from_file_location("tail_observer", Path(__file__).with_name("publishing-audio-tail-observer.py"))
            tail = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(tail)
            execute_step("probe-source-audio-frames", [tools["ffprobe"], "-v", "error", "-select_streams", "a:0", "-show_streams", "-show_packets", "-show_frames", "-show_data_hash", "sha256", "-of", "json", source])
            native = tail.native_audio(read_json(output / "logs/probe-source-audio-frames.stdout"))
            signal_path = contained_file(outcome["audio_signal"]["receipt"], output)
            signal = read_json(signal_path)
            require(file_record(signal_path)["sha256"] == outcome["audio_signal"]["receipt_sha256"], "source signal receipt pin changed")
            source_wav = signal_path.parent / "selected-audio.wav"
            pcm = tail.float32_wav(regular_bytes(source_wav, MAX_FILE), native)
            tail.bind_signal(native, pcm, signal)
            require((native["presented_sample_frames"], native["decoded_sample_frames"], native["nonpresented_tail_sample_frames"]) == (96000, 96256, 256), "ordinary source does not exercise selected observed terminal AAC block")
            for artifact_id in ("lossless_master", "h264_delivery"):
                decoded = output / (artifact_id + "-complete-audio.wav")
                execute_step("decode-complete-audio-" + artifact_id, [tools["ffmpeg"], "-hide_banner", "-nostdin", "-v", "error", "-xerror", "-n", "-i", output / report_outputs[artifact_id]["path"], "-map", "0:a:0", "-vn", "-sn", "-dn", "-c:a", "pcm_f32le", "-flags:a", "+bitexact", "-fflags", "+bitexact", "-threads:a", "1", "-f", "wav", decoded])
                decoded_pcm = tail.float32_wav(regular_bytes(decoded, MAX_FILE), native)
                require(decoded_pcm["decoded_pcm_sha256"] == pcm["decoded_pcm_sha256"], "complete original AAC decoder signal changed in master or delivery")
            report["complete_decoded_audio"] = {"native": native, "pcm": pcm, "source_signal_receipt": file_record(signal_path), "source_master_delivery_pcm_equal": True, "presented_delivery_loudness_verified": False}
        assembly = [phase for phase in outcome.get("phases", [])
                    if phase.get("phase") == "assembly" and phase.get("state") == "complete"]
        require(len(assembly) == 1, "recipe lacks one accepted assembly child")
        execute_step("observe-completion", [tools["aniflow"], "--output", "json", "video", "completion",
                     assembly[0]["run_directory"], "--public-summary"])
        completion = machine_result(output / "logs/observe-completion.stdout", "video_completion")
        require(completion.get("schema") == "aniflow.final-video-public-summary/v1"
                and completion.get("publication_authority") == "none"
                and completion.get("proof_state") == "not_asserted"
                and completion.get("creative_approval") == "not_recorded_by_fixed_automated_profile",
                "completion projection must preserve its approval/publication limits")
        current = {item["artifact_id"]: item["sha256"] for item in completion.get("artifacts", [])}
        require(current == {key: value["sha256"] for key, value in report_outputs.items()},
                "completion metadata does not bind the selected accepted outputs")
        execute_step("convert-manual-cues", [tools["aniflow"], "--output", "json", "timed-text", "convert",
                     "--input", srt, "--from", "srt", "--to", "webvtt", "--output-directory", output / "webvtt"])
        converted = machine_result(output / "logs/convert-manual-cues.stdout", "timed_text_convert")
        webvtt = contained_file(converted["payload_path"], output)
        require(cues_from_text(regular_bytes(webvtt, 65536).decode()) == EXPECTED_CUES,
                "converted supplied cue words or intervals changed")
        webvtt_record = file_record(webvtt)
        options = output / "lyrics-options.json"
        options.write_bytes(json_bytes({"input_format": "srt", "language": None,
            "overlap_policy": "reject", "targets": ["srt", "webvtt"], "conversion": {"allow_losses": []}}))
        execute_step("prepare-manual-request", [tools["aniflow"], "--output", "json", "video", "lyrics",
                     "prepare-request", "--recipe-run-directory", recipe_root, "--options", options])
        request = machine_result(output / "logs/prepare-manual-request.stdout", "video_lyrics_prepare_request")
        request_path = output / "manual-lyrics-request.json"
        request_path.write_bytes(json_bytes(request))
        workflow = output / "manual-lyrics"
        lyrics_args = [tools["aniflow"], "--output", "json", "video", "lyrics"]
        bound = ["--request", request_path, "--work-directory", workflow]
        execute_step("import-manual-lyrics", [*lyrics_args, "initialize", *bound, "--input", srt])
        imported = machine_result(output / "logs/import-manual-lyrics.stdout", "video_lyrics_initialize")
        require(imported.get("words", {}).get("state") == "awaiting_review"
                and imported.get("timing", {}).get("state") == "awaiting_review"
                and imported.get("ready_for_sidecar_export") is False,
                "supplied SRT import must not invent either review")
        blocked = output / "unreviewed-delivery"
        refused = execute_step("refuse-unreviewed-export", [*lyrics_args, "export", *bound,
            "--output-directory", blocked], expected_success=False)
        require(refused["state"] == "failed" and refused["exit_code"] == 8
                and not blocked.exists(), "unreviewed lyrics export was not refused without output")
        refusal = machine_error(output / "logs/refuse-unreviewed-export.stderr", "video_lyrics_export")
        require(refusal.get("category") == "state" and refusal.get("message") ==
                "lyrics await separate exact words and timing review", "export failed for an unrelated reason")
        execute_step("resume-completed", [*recipe_args, "--resume-run-directory", recipe_root])
        resumed = machine_result(output / "logs/resume-completed.stdout", "video_music_video")
        require(Path(resumed["run_directory"]) == recipe_root,
                "completed compatible resume selected another recipe")
        require_preserved_files(accepted_records, "completed compatible resume changed an accepted video")
        changed_configuration = output / "changed-source-recipe.json"
        changed = source_configuration(tools, source_profile)
        changed["recipe"]["assembly"]["delivery_crf"] = 19
        changed_configuration.write_bytes(json_bytes(changed))
        refused = execute_step("refuse-changed-resume", [tools["aniflow"], "--output", "json", "video",
            "music-video", "--source-video", source, "--configuration", changed_configuration,
            "--resume-run-directory", recipe_root], expected_success=False)
        require(refused["state"] == "failed" and refused["exit_code"] == 8,
                "changed settings were accepted for the retained recipe")
        refusal = machine_error(output / "logs/refuse-changed-resume.stderr", "video_music_video")
        require(refusal.get("category") == "state" and refusal.get("message") ==
                "source recipe input, configuration, selected component pins or provider changed; start a new recipe run",
                "changed resume failed for an unrelated reason")
        require_preserved_files(accepted_records, "refused resume changed an accepted video")
        refused = execute_step("refuse-existing-conversion", [tools["aniflow"], "--output", "json", "timed-text",
            "convert", "--input", srt, "--from", "srt", "--to", "webvtt", "--output-directory", output / "webvtt"],
            expected_success=False)
        require(refused["state"] == "failed" and refused["exit_code"] == 4,
                "existing sidecar destination was not refused")
        refusal = machine_error(output / "logs/refuse-existing-conversion.stderr", "timed_text_convert")
        require(refusal.get("category") == "configuration" and refusal.get("message") ==
                "conversion destination must be a new directory; existing paths are never replaced",
                "existing conversion failed for an unrelated reason")
        require(cues_from_text(regular_bytes(webvtt, 65536).decode()) == EXPECTED_CUES,
                "refused conversion mutated retained sidecars")
        require_preserved_files({webvtt: webvtt_record}, "refused conversion rewrote retained sidecar bytes")
        for path in (source, srt):
            require(file_record(path) == originals[path.name], "synthetic source was mutated")
        for name, path in tools.items():
            require(file_record(path, MAX_TOOL)["sha256"] == report["tools"][name]["sha256"], "selected tool changed")
        require_preserved_files(accepted_records, "final observation changed an accepted video")
        report.update(state="passed", media=after, source_media=before, originals=originals,
                      aac_packets_preserved=len(before_audio["packets"]), manual_cues_preserved=len(EXPECTED_CUES),
                      aac_codec_configuration_preserved=True, aac_priming_padding_preserved=True,
                      source_preserved=True, completed_resume_preserved=True, existing_conversion_refused=True,
                      changed_configuration_resume_refused=True,
                      selected_typed_cpu_subset_observed=True, words_review="awaiting_review",
                      timing_review="awaiting_review", unreviewed_export_refused=True)
        report["outputs"] = {**report_outputs,
            "webvtt": {"path": webvtt.relative_to(output).as_posix(), **webvtt_record},
            "manual_lyrics_request": {"path": request_path.relative_to(output).as_posix(), **file_record(request_path)}}
    except (ValueError, OSError, KeyError, TypeError, ZeroDivisionError) as error:
        report["failure"] = str(error)[:2000]
    finally:
        report["elapsed_seconds"] = round(time.monotonic() - started, 3)
        try:
            report["observed_payload_bytes"] = tree_bytes(output, MAX_OUTPUT - MAX_REPORT)
        except ValueError as error:
            report["state"] = "failed"
            report["observed_payload_bytes"] = None
            report["failure"] = str(error)[:2000]
        encoded = json_bytes(report)
        if len(encoded) > MAX_REPORT:
            report.update(state="failed", failure="pilot report exceeds 64 KiB; full process logs retained")
            report["steps"] = [{"name": step["name"], "state": step["state"], "exit_code": step["exit_code"]}
                               for step in steps]
            encoded = json_bytes(report)
        require(len(encoded) <= MAX_REPORT, "bounded failure report cannot be serialized")
        with (output / "pilot-report.json").open("xb") as stream:
            stream.write(encoded)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-profile", choices=("aligned", "ordinary-mp4"), default="aligned")
    parser.add_argument("--aniflow", type=Path, required=True)
    parser.add_argument("--ffmpeg", type=Path, required=True)
    parser.add_argument("--ffprobe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="Fresh path outside the source checkout")
    args = parser.parse_args()
    try:
        result = run(args.output, args.aniflow, args.ffmpeg, args.ffprobe, args.source_profile)
    except (ValueError, OSError) as error:
        print(str(error), file=sys.stderr)
        return 2
    print(json.dumps({"state": result["state"], "selected_profile": result["selected_profile"],
                      "report": str(args.output / "pilot-report.json"), "release_qualified": False}))
    return 0 if result["state"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
