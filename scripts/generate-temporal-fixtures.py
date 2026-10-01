#!/usr/bin/env python3
"""Author deterministic FFprobe protocol fixtures; does not execute media tools/tests."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "tests" / "fixtures" / "temporal"


def video(index=7, denominator=24000, duration=1001, count=4):
    stream = {"index": index, "codec_type": "video", "codec_name": "h264", "time_base": f"1/{denominator}", "start_pts": 0, "duration_ts": duration * count, "r_frame_rate": f"{denominator}/{duration}", "avg_frame_rate": f"{denominator}/{duration}", "width": 16, "height": 16, "pix_fmt": "yuv420p", "disposition": {"attached_pic": 0}}
    frames = [{"stream_index": index, "pts": n * duration, "pkt_dts": n * duration, "pkt_duration": duration, "key_frame": int(n % 2 == 0)} for n in range(count)]
    packets = [{"stream_index": index, "pts": n * duration, "dts": n * duration, "duration": duration} for n in range(count)]
    return stream, {"stream_index": index, "frames": {"frames": frames}, "packets": {"packets": packets}}


def audio(index=8, duration=2002, count=4):
    stream = {"index": index, "codec_type": "audio", "codec_name": "pcm_s16le", "time_base": "1/48000", "sample_rate": "48000", "start_pts": 0, "duration_ts": duration * count}
    frames = [{"stream_index": index, "pts": n * duration, "pkt_dts": n * duration, "pkt_duration": duration, "nb_samples": duration} for n in range(count)]
    packets = [{"stream_index": index, "pts": n * duration, "dts": n * duration, "duration": duration} for n in range(count)]
    return stream, {"stream_index": index, "frames": {"frames": frames}, "packets": {"packets": packets}}


def base():
    stream, observation = video()
    return {"selection": {"video_stream": None, "audio_stream": None, "no_audio": False, "discard_streams": []}, "inventory": {"streams": [stream]}, "observations": [observation]}


def fixtures():
    result = []
    def add(case_id, fixture, outcome="supported", codes=()):
        fixture = copy.deepcopy(fixture)
        stub = f"synthetic protocol identity for {case_id}; not a media file".encode()
        fixture.update({"case_id": case_id, "qualification": "synthetic protocol fixture only; native media and tests not executed", "source_stub": stub.decode(), "source_sha256": hashlib.sha256(stub).hexdigest(), "source_size_bytes": len(stub), "expected": {"outcome": outcome, "codes": list(codes)}})
        result.append(fixture)

    add("cfr-24000-1001", base())
    f = base(); stream, obs = video(denominator=30000); f["inventory"]["streams"] = [stream]; f["observations"] = [obs]; add("cfr-30000-1001", f)
    f = base(); stream, obs = video(denominator=30, duration=1); f["inventory"]["streams"] = [stream]; f["observations"] = [obs]; add("cfr-integer-30", f)
    f = base(); stream, obs = audio(); f["inventory"]["streams"].append(stream); f["observations"].append(obs); add("synchronized-audio-video", f)
    f = base(); frames = f["observations"][0]["frames"]["frames"]; frames[1]["pkt_duration"] = 2002; frames.pop(); frames[2]["pts"] = 3003; f["observations"][0]["packets"]["packets"].pop(); add("vfr-visible-refusal", f, "refused", ["variable_frame_rate"])
    for name, offset, code in [("positive-origin", 5005, "nonzero_presentation_offset"), ("negative-origin", -2002, "negative_presentation_offset")]:
        f = base(); f["inventory"]["streams"][0]["start_pts"] = offset
        for frame in f["observations"][0]["frames"]["frames"]: frame["pts"] += offset; frame["pkt_dts"] += offset
        for packet in f["observations"][0]["packets"]["packets"]: packet["pts"] += offset; packet["dts"] += offset
        add(name, f, "refused", [code])
    f = base(); frames = f["observations"][0]["frames"]["frames"]; frames[1]["pts"], frames[2]["pts"] = frames[2]["pts"], frames[1]["pts"]; add("reordered-decoded-presentation", f, "refused", ["presentation_order"])
    f = base(); f["observations"][0]["frames"]["frames"][2]["pts"] = 1001; add("duplicate-decoded-presentation", f, "refused", ["presentation_order"])
    f = base(); packets = f["observations"][0]["packets"]["packets"]; packets[1]["pts"], packets[3]["pts"] = packets[3]["pts"], packets[1]["pts"]
    for n, packet in enumerate(packets): packet["dts"] = (n - 2) * 1001
    add("reordered-packet-pts-monotonic-dts", f)
    f = base(); packets = f["observations"][0]["packets"]["packets"]; packets[1]["dts"], packets[2]["dts"] = packets[2]["dts"], packets[1]["dts"]; add("nonmonotonic-decode-order", f, "refused", ["decode_order"])
    f = base(); del f["observations"][0]["packets"]["packets"][0]["dts"]; add("missing-packet-dts", f, "refused", ["missing_timestamp"])
    f = base(); del f["observations"][0]["frames"]["frames"][0]["pts"]; add("missing-decoded-pts", f, "parser_error", ["missing_timestamp"])
    f = base(); f["observations"][0]["frames"]["frames"][0]["pkt_duration"] = 0; add("zero-frame-duration", f, "parser_error", ["invalid_duration"])
    f = base(); f["inventory"]["streams"][0]["duration_ts"] = 5005; add("declared-duration-contradiction", f, "refused", ["stream_duration_mismatch"])
    f = base(); stream, obs = audio(duration=1001); f["inventory"]["streams"].append(stream); f["observations"].append(obs); add("audio-video-end-mismatch", f, "refused", ["synchronization_mismatch"])
    f = base(); f["inventory"]["streams"][0]["time_base"] = "1/0"; add("zero-denominator", f, "parser_error", ["invalid_rational"])
    f = base(); f["inventory"]["streams"][0]["time_base"] = "9223372036854775807/1"; add("time-arithmetic-overflow", f, "parser_error", ["arithmetic_overflow"])
    f = base(); f["inventory"]["streams"].append(copy.deepcopy(f["inventory"]["streams"][0])); add("duplicate-stream-index", f, "parser_error", ["duplicate_stream"])
    f = base(); stream, obs = video(index=9); f["inventory"]["streams"].append(stream); f["observations"] = []; add("ambiguous-multi-video", f, "refused", ["ambiguous_video"])
    f["selection"]["video_stream"] = 9; f["selection"]["discard_streams"] = [7]; f["observations"] = [obs]; add("explicit-second-video", f)
    f = base(); a, ao = audio(index=8); b, bo = audio(index=11); f["inventory"]["streams"] += [a, b]; f["observations"] = []; add("ambiguous-multi-audio", f, "refused", ["ambiguous_audio"])
    f["selection"]["audio_stream"] = 11; f["selection"]["discard_streams"] = [8]; f["observations"] = [video()[1], bo]; add("explicit-second-audio", f)
    f = base(); cover, _ = video(index=0); cover["disposition"]["attached_pic"] = 1; f["inventory"]["streams"].insert(0, cover); add("cover-art-is-not-moving-video", f, "refused", ["unacknowledged_stream"])
    f["selection"]["discard_streams"] = [0]; add("acknowledged-cover-art", f)
    f = base()
    for index, kind in enumerate(["subtitle", "data", "attachment", "unrecognized"]): f["inventory"]["streams"].append({"index": index, "codec_type": kind, "codec_name": "synthetic", "time_base": "1/1000"})
    add("mixed-extra-streams-refused", f, "refused", ["unacknowledged_stream"])
    f["selection"]["discard_streams"] = [0, 1, 2, 3]; add("mixed-extra-streams-explicitly-discarded", f)
    f = base(); stream, obs = audio(); f["inventory"]["streams"].append(stream); f["selection"]["no_audio"] = True; f["selection"]["discard_streams"] = [8]; add("explicit-no-audio", f)
    f = base(); f["selection"]["video_stream"] = 99; f["observations"] = []; add("unknown-selected-index", f, "refused", ["invalid_selection"])
    f = base(); f["selection"]["discard_streams"] = [7]; f["observations"] = []; add("selected-and-discarded-conflict", f, "refused", ["invalid_selection"])
    return result


def main():
    DESTINATION.mkdir(parents=True, exist_ok=True)
    generated = fixtures()
    for fixture in generated:
        (DESTINATION / f"{fixture['case_id']}.json").write_text(json.dumps(fixture, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    index = {"schema": "aniflow.temporal-fixture-index/v1", "qualification": "authored protocol fixtures; test execution deferred", "generator": "scripts/generate-temporal-fixtures.py", "fixtures": [f["case_id"] for f in generated]}
    (DESTINATION / "index.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
    print(f"Authored {len(generated)} temporal protocol fixtures; no tests or media tools executed.")


if __name__ == "__main__":
    main()
