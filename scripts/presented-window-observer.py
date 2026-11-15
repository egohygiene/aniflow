#!/usr/bin/env python3
"""Reobserve saved presented-window bytes and scope; never launch media tools.

Native acceptance/current owner readback and meter qualification remain separate.
Supplying a QC file additionally checks its explicit request selection and subject.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
from pathlib import Path
import struct

SPEC = importlib.util.spec_from_file_location(
    "tail_observer", Path(__file__).with_name("publishing-audio-tail-observer.py"))
tail = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(tail)
SCOPE = "exact_presented_audio_window"
METHOD = "exact_float32_sample_bytes_from_complete_pcm; no_decode_gain_resample_or_delivery_edit"
FIELDS = {"schema", "metric_scope", "source_sha256", "source_bytes", "temporal_sha256",
          "complete_pcm_sha256", "complete_sample_frames", "source_audio_extent",
          "start_sample_frame", "presented_sample_frames", "sample_rate_hz", "channels",
          "window_wav_sha256", "window_wav_bytes", "window_pcm_sha256", "method",
          "loudness", "normalization", "runtime_qualified"}


def record(data):
    return {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


def payload(data):
    # Called only after the existing bounded complete-WAV parser succeeds.
    cursor = 12
    while cursor < len(data):
        size = struct.unpack_from("<I", data, cursor + 4)[0]
        if data[cursor:cursor + 4] == b"data":
            return data[cursor + 8:cursor + 8 + size]
        cursor += 8 + size + size % 2
    raise ValueError("complete WAV data is missing")


def observe_bytes(source, complete, presented, signal, qc=None):
    require = tail.require
    integer = tail.receipt_field
    source_pin = record(source)
    require(signal.get("schema") == "aniflow.video-audio-signal/v1"
            and signal.get("normalization") == "none"
            and signal.get("runtime_qualified") is False, "unsupported signal authority")
    window = signal.get("presented_window")
    require(isinstance(window, dict) and set(window) == FIELDS,
            "missing or open presented-window receipt")
    channels = integer(signal, "channels")
    count = integer(window, "presented_sample_frames")
    full_count = integer(signal, "frames")
    require(channels in (1, 2) and 0 < count <= full_count <= 28_800_000,
            "unsupported presented/full sample bounds")
    native = {"channels": channels, "decoded_sample_frames": full_count,
              "presented_sample_frames": count}
    full = tail.float32_wav(complete, native)
    view = tail.float32_wav(presented, {**native, "decoded_sample_frames": count})
    full_pcm, view_pcm = payload(complete), payload(presented)
    stride = channels * 4
    expected_header = (b"RIFF" + struct.pack("<I", len(view_pcm) + 48) + b"WAVEfmt "
        + struct.pack("<IHHIIHH", 16, 3, channels, 48000, 48000 * stride, stride, 32)
        + b"fact" + struct.pack("<II", 4, count) + b"data" + struct.pack("<I", len(view_pcm)))
    require(presented[:56] == expected_header and len(presented) == len(view_pcm) + 56
            and view_pcm == full_pcm[:count * stride],
            "presented WAV is not the exact canonical source sample-byte view")
    require(window.get("schema") == "aniflow.presented-audio-window/v1"
            and window.get("metric_scope") == SCOPE and window.get("method") == METHOD
            and window.get("normalization") == "none"
            and window.get("runtime_qualified") is False,
            "wrong presented-window method, scope or authority")
    require(window.get("source_sha256") == signal.get("source_sha256") == source_pin["sha256"]
            and integer(window, "source_bytes") == integer(signal, "source_bytes") == source_pin["bytes"]
            and window.get("temporal_sha256") == signal.get("temporal_sha256")
            and isinstance(window["temporal_sha256"], str)
            and re.fullmatch(r"[0-9a-f]{64}", window["temporal_sha256"]) is not None,
            "presented-window source or temporal identity differs")
    require(integer(window, "complete_sample_frames") == full_count
            and integer(window, "start_sample_frame") == 0
            and integer(window, "sample_rate_hz") == integer(signal, "sample_rate_hz") == 48000
            and integer(window, "channels") == channels
            and tail.exact_json(window["source_audio_extent"], tail.sample_extent(count))
            and tail.exact_json(signal["source_audio_extent"], tail.sample_extent(count)),
            "presented-window rational interval or sample scope differs")
    require(window.get("complete_pcm_sha256") == signal.get("decoded_pcm_sha256") == full["decoded_pcm_sha256"]
            and signal.get("decoded_wav_sha256") == full["wav_sha256"]
            and integer(signal, "decoded_wav_bytes") == full["wav_bytes"]
            and window.get("window_pcm_sha256") == view["decoded_pcm_sha256"]
            and window.get("window_wav_sha256") == view["wav_sha256"]
            and integer(window, "window_wav_bytes") == view["wav_bytes"],
            "presented/full PCM or container identity differs")
    for meter, frames, digest in ((signal["loudness"], full_count, full["decoded_pcm_sha256"]),
                                  (window["loudness"], count, view["decoded_pcm_sha256"])):
        require(integer(meter, "frames") == frames and meter.get("pcm_sha256") == digest
                and integer(meter, "sample_rate_hz") == 48000
                and integer(meter, "channels") == channels
                and meter.get("runtime_qualified") is False,
                "meter is not bound to its separate exact PCM subject")
    if qc is not None:
        require(qc.get("schema") == "aniflow.final-audio-qc/v1"
                and qc.get("normalization") == "none" and qc.get("runtime_qualified") is False
                and qc.get("publication_authority") == "none"
                and qc.get("creative_approval") == "not_performed; measurements are not musical quality review"
                and tail.exact_json(qc.get("signal"), signal), "QC signal or authority differs")
        intent = qc["intent"]
        require(intent["request"].get("presented_window") == "exact_presented_audio"
                and intent["signal_request"].get("presented_window") == "exact_presented_audio"
                and intent["artifact"]["sha256"] == source_pin["sha256"]
                and integer(intent["artifact"], "byte_count") == source_pin["bytes"]
                and intent["signal_request"]["source_sha256"] == source_pin["sha256"]
                and integer(intent["signal_request"], "source_bytes") == source_pin["bytes"],
                "QC did not explicitly select this exact presented source")
    return {"schema": "aniflow.retained-presented-window-observation/v1",
            "source": source_pin, "complete_pcm": full, "presented_pcm": view,
            "exact_sample_byte_identity": True, "metric_scope": SCOPE,
            "request_selection_checked": qc is not None, "policy_comparison_performed": False,
            "native_meter_qualified": False, "human_review_performed": False,
            "publication_authority": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-video", "complete-wav", "presented-wav", "signal"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--qc", type=Path)
    args = parser.parse_args()
    files = {name: tail.regular_bytes(getattr(args, name), 4194304 if name == "signal" else 268435456)
             for name in ("source_video", "complete_wav", "presented_wav", "signal")}
    qc = tail.strict_json(tail.regular_bytes(args.qc, 4194304)) if args.qc else None
    result = observe_bytes(files["source_video"], files["complete_wav"], files["presented_wav"],
                           tail.strict_json(files["signal"]), qc)
    result["observer"] = record(Path(__file__).read_bytes())
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
