#!/usr/bin/env python3
"""Reobserve saved AAC-LC frame/packet captures and complete float32 PCM.

This is a read-only observer, not a decoder, approval, or qualification tool.
It never invokes a process. All inputs must already have been captured by the
selected native tools. This is not fresh CLI execution proof. A decoded-tail meter is not a presented-window meter.
"""

from __future__ import annotations

import argparse
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import re
import stat
import struct

MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_WAV_BYTES = 256 * 1024 * 1024
MAX_BLOCKS = 1_000_000
DECODED_SCOPE = "complete_decoded_pcm_including_nonpresented_tail"
PRESENTED_SCOPE = "complete_presented_audio"


def require(value, message):
    if not value:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(",", ":"), allow_nan=False).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def exact_json(actual, expected):
    """JSON equality preserving numbers versus booleans and numeric strings."""
    return canonical(actual) == canonical(expected)


def regular_bytes(path, maximum):
    path = Path(path)
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and not path.is_symlink()
            and 0 < before.st_size <= maximum, "input is not a bounded regular file")
    data = path.read_bytes()
    after = path.lstat()
    require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
            == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
            and len(data) == before.st_size, "input changed during observation")
    return data


def strict_json(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate JSON object field")
            result[key] = value
        return result

    def constant(value):
        raise ValueError("nonfinite JSON number: " + value)

    return json.loads(data, object_pairs_hook=pairs, parse_constant=constant)


def integer(value, name):
    if type(value) is int:
        number = value
    elif isinstance(value, str) and re.fullmatch(r"-?(0|[1-9][0-9]*)", value):
        number = int(value)
    else:
        raise ValueError(name + " is not a native integer")
    require(-(2 ** 63) <= number < 2 ** 63, name + " is outside i64")
    return number


def field(value, name):
    require(isinstance(value, dict) and name in value, "missing " + name)
    return integer(value[name], name)


def receipt_field(value, name):
    require(isinstance(value, dict) and type(value.get(name)) is int,
            "receipt " + name + " is not a JSON integer")
    return integer(value[name], name)


def native_audio(document):
    """Close the admitted ordinary 48 kHz AAC-LC block/packet relationship.

    The initially discarded priming packet is separate from decoded frames.
    Only the final block may contain a nonpresented decoded tail. This is not
    a tolerance: every observed duration/start/sample count is checked.
    """
    require(isinstance(document, dict), "native probe is not an object")
    streams = document.get("streams")
    events = document.get("packets_and_frames")
    require(isinstance(streams, list) and len(streams) == 1,
            "require exactly one selected audio stream")
    require(isinstance(events, list) and 1 <= len(events) <= 2 * MAX_BLOCKS + 1,
            "require bounded complete frame and packet events")
    stream = streams[0]
    require(stream.get("codec_type") == "audio" and stream.get("codec_name") == "aac"
            and stream.get("profile") == "LC" and stream.get("time_base") == "1/48000"
            and field(stream, "sample_rate") == 48000 and field(stream, "start_pts") == 0,
            "require zero-origin 48 kHz AAC-LC sample-grid source")
    channels = field(stream, "channels")
    require((channels, stream.get("channel_layout")) in ((1, "mono"), (2, "stereo")),
            "unsupported native channel layout")
    require(not stream.get("side_data_list"), "unsupported audio stream side data")
    index = field(stream, "index")
    require(0 <= index <= 2 ** 32 - 1, "invalid selected stream index")
    presented = field(stream, "duration_ts")
    require(0 < presented <= MAX_BLOCKS * 1024, "invalid presented extent")
    frames, packets = [], []
    for event in events:
        require(isinstance(event, dict) and field(event, "stream_index") == index,
                "native event changed selected stream")
        if event.get("type") == "frame":
            require(event.get("media_type") == "audio", "non-audio decoded frame")
            frames.append(event)
        elif event.get("type") == "packet":
            require(event.get("codec_type") == "audio", "non-audio packet")
            packets.append(event)
        else:
            raise ValueError("unknown native event type")
    require(1 <= len(frames) <= MAX_BLOCKS and len(packets) == len(frames) + 1,
            "complete AAC block and packet counts disagree")
    require(field(stream, "nb_read_frames") == len(frames)
            and field(stream, "nb_read_packets") == len(packets),
            "stream observation counts disagree")
    priming = packets[0]
    require(field(priming, "pts") == -1024 and field(priming, "dts") == -1024
            and field(priming, "duration") == 1024
            and exact_json(priming.get("side_data_list"), [{"side_data_type": "Skip Samples",
                 "skip_samples": 1024, "discard_padding": 0,
                 "skip_reason": 0, "discard_reason": 0}]),
            "initial AAC priming/skip evidence changed")
    end, decoded = 0, 0
    native_frames, packet_facts = [], []
    for position, (frame, packet) in enumerate(zip(frames, packets[1:])):
        start = field(frame, "pts")
        duration = field(frame, "duration")
        samples = field(frame, "nb_samples")
        require(start == end and field(frame, "pkt_dts") == start,
                "decoded audio has a gap, overlap or changed decode clock")
        require(samples == 1024 and 0 < duration <= 1024
                and (position == len(frames) - 1 or duration == samples),
                "only the final AAC-LC block may have a shorter presented duration")
        require(field(frame, "channels") == channels
                and frame.get("channel_layout") == stream.get("channel_layout")
                and frame.get("sample_fmt") == "fltp" and not frame.get("side_data_list"),
                "decoded frame sample/channel profile changed")
        require(field(packet, "pts") == start and field(packet, "dts") == start
                and field(packet, "duration") == duration and not packet.get("side_data_list"),
                "native packet and decoded presentation clocks/side data disagree")
        native_frames.append({"pts": start, "duration": duration, "nb_samples": samples})
        end += duration
        decoded += samples
    require(end == presented, "complete presented timeline differs from stream extent")
    tail = decoded - presented
    require(0 <= tail < 1024 and decoded == len(frames) * 1024,
            "decoded extent is not exactly the observed complete AAC blocks")
    for packet in packets:
        packet_hash = packet.get("data_hash")
        flags = packet.get("flags")
        require(isinstance(packet_hash, str)
                and re.fullmatch(r"SHA256:[a-f0-9]{64}", packet_hash)
                and isinstance(flags, str) and re.fullmatch(r"[A-Z_]{1,16}", flags)
                and field(packet, "size") > 0, "packet identity is incomplete")
        packet_facts.append({"pts": field(packet, "pts"), "dts": field(packet, "dts"),
            "duration": field(packet, "duration"), "size": field(packet, "size"),
            "flags": flags, "data_hash": packet_hash,
            "side_data_list": packet.get("side_data_list", [])})
    extra = stream.get("extradata_hash")
    require(isinstance(extra, str) and re.fullmatch(r"SHA256:[a-f0-9]{64}", extra)
            and field(stream, "extradata_size") > 0, "AAC codec configuration is unbound")
    return {"audio_stream": index, "sample_rate_hz": 48000, "channels": channels,
        "presented_sample_frames": presented, "decoded_sample_frames": decoded,
        "nonpresented_tail_sample_frames": tail, "decoded_block_count": len(frames),
        "packet_count": len(packets), "priming_sample_frames": 1024,
        "final_presented_sample_frames": field(frames[-1], "duration"),
        "native_frame_samples_sha256": digest(canonical(native_frames)),
        "aac_packet_identity_sha256": digest(canonical({"extradata_hash": extra,
            "extradata_size": field(stream, "extradata_size"), "packets": packet_facts})),
        "required_meter_scope": DECODED_SCOPE if tail else PRESENTED_SCOPE}


def float32_wav(data, native):
    data = bytes(data)
    require(len(data) >= 44 and data[:4] == b"RIFF" and data[8:12] == b"WAVE"
            and struct.unpack_from("<I", data, 4)[0] + 8 == len(data),
            "not a complete RIFF/WAVE container")
    cursor, chunks = 12, {}
    while cursor < len(data):
        require(cursor + 8 <= len(data), "partial WAV chunk header")
        name, size = data[cursor:cursor + 4], struct.unpack_from("<I", data, cursor + 4)[0]
        end = cursor + 8 + size
        require(end <= len(data) and name not in chunks, "partial or duplicate WAV chunk")
        require(name in (b"fmt ", b"fact", b"data"), "unexpected bitexact WAV chunk")
        chunks[name] = data[cursor + 8:end]
        cursor = end + size % 2
    require(cursor == len(data) and set(chunks) == {b"fmt ", b"fact", b"data"},
            "incomplete WAV chunk set")
    fmt = chunks[b"fmt "]
    require(len(fmt) in (16, 18), "unsupported WAV native format shape")
    tag, channels, rate, byte_rate, align, bits = struct.unpack_from("<HHIIHH", fmt)
    require(tag == 3 and bits == 32 and rate == 48000 and channels == native["channels"]
            and align == channels * 4 and byte_rate == rate * align
            and (len(fmt) == 16 or fmt[16:] == b"\0\0"),
            "WAV is not unconverted selected-channel 48 kHz float32 PCM")
    pcm = chunks[b"data"]
    require(len(pcm) % align == 0, "partial interleaved PCM frame")
    count = len(pcm) // align
    require(len(chunks[b"fact"]) == 4
            and struct.unpack("<I", chunks[b"fact"])[0] == count
            and count == native["decoded_sample_frames"],
            "full WAV sample count differs from actual complete native decode")
    require(all(math.isfinite(sample[0]) for sample in struct.iter_unpack("<f", pcm)),
            "complete PCM contains a nonfinite sample")
    tail = pcm[native["presented_sample_frames"] * align:]
    tail_values = [sample[0] for sample in struct.iter_unpack("<f", tail)]
    return {"wav_sha256": digest(data), "wav_bytes": len(data),
        "decoded_pcm_sha256": digest(pcm), "decoded_sample_frames": count,
        "nonpresented_tail_sha256": digest(tail),
        "nonpresented_tail_sample_frames": len(tail) // align,
        "nonpresented_tail_nonzero_values": sum(value != 0 for value in tail_values),
        "nonpresented_tail_peak_absolute": max(map(abs, tail_values), default=0.0)}


def sample_extent(count):
    end = Fraction(count, 48000)
    return {"start": {"numerator": 0, "denominator": 1},
        "end": {"numerator": end.numerator, "denominator": end.denominator}}


def bind_signal(native, pcm, signal):
    """Join a real signal receipt to the native presentation and full decode.

    This does not authenticate a supplied receipt or qualify its meter. The
    caller must bind the original source/tool/receipt files separately.
    """
    require(isinstance(signal, dict) and signal.get("schema") == "aniflow.video-audio-signal/v1"
            and signal.get("normalization") == "none" and signal.get("runtime_qualified") is False,
            "unsupported or authority-claiming signal receipt")
    require(exact_json(signal.get("source_audio_extent"), sample_extent(native["presented_sample_frames"]))
            and receipt_field(signal, "audio_stream") == native["audio_stream"]
            and receipt_field(signal, "sample_rate_hz") == 48000
            and receipt_field(signal, "channels") == native["channels"]
            and receipt_field(signal, "frames") == native["decoded_sample_frames"]
            and signal.get("decoded_pcm_sha256") == pcm["decoded_pcm_sha256"]
            and signal.get("decoded_wav_sha256") == pcm["wav_sha256"]
            and receipt_field(signal, "decoded_wav_bytes") == pcm["wav_bytes"],
            "signal receipt changed the exact presented or complete decoded subject")
    tail = native["nonpresented_tail_sample_frames"]
    if tail:
        require(exact_json(signal.get("decoded_audio_tail"), {
            "schema": "aniflow.aac-decoded-tail/v1", "metric_scope": DECODED_SCOPE,
            "presented_sample_frames": native["presented_sample_frames"],
            "decoded_sample_frames": native["decoded_sample_frames"],
            "nonpresented_tail_sample_frames": tail,
            "decoded_block_count": native["decoded_block_count"],
            "final_presented_sample_frames": native["final_presented_sample_frames"],
            "priming_sample_frames": native["priming_sample_frames"],
            "decoded_extent": sample_extent(native["decoded_sample_frames"])}),
            "decoded tail evidence is incomplete, changed, or mislabeled as presented metrics")
    else:
        require("decoded_audio_tail" not in signal,
                "aligned legacy subject must preserve its absent tail field")
    meter = signal.get("loudness")
    require(isinstance(meter, dict) and receipt_field(meter, "frames") == native["decoded_sample_frames"]
            and receipt_field(meter, "sample_rate_hz") == 48000
            and receipt_field(meter, "channels") == native["channels"]
            and meter.get("pcm_sha256") == pcm["decoded_pcm_sha256"]
            and meter.get("runtime_qualified") is False,
            "meter is not bound to the same complete decoded PCM")
    return {"metric_scope": native["required_meter_scope"],
        "exact_complete_pcm_join": True,
        "presented_delivery_loudness_verified": False}


def bind_qc(native, pcm, qc):
    require(isinstance(qc, dict) and qc.get("schema") == "aniflow.final-audio-qc/v1"
            and qc.get("normalization") == "none" and qc.get("runtime_qualified") is False
            and qc.get("publication_authority") == "none"
            and qc.get("creative_approval") == "not_performed; measurements are not musical quality review",
            "QC receipt fabricates normalization, qualification, publication or creative review")
    joined = bind_signal(native, pcm, qc.get("signal"))
    accepted = qc.get("accepted_pcm")
    require(isinstance(accepted, dict) and accepted.get("report_schema") == "aniflow.audio-signal-measurements/v3"
            and accepted.get("normalization") == "none", "wrong accepted PCM report")
    source = accepted.get("source")
    require(isinstance(source, dict) and exact_json(source.get("artifact"), {
        "id": "source_audio", "sha256": pcm["wav_sha256"], "byte_size": pcm["wav_bytes"]})
        and receipt_field(source, "frame_count") == native["decoded_sample_frames"]
        and receipt_field(source, "sample_rate_hz") == 48000 and receipt_field(source, "channels") == native["channels"]
        and receipt_field(source, "stream_index") == 0 and source.get("stem") is None
        and exact_json(source.get("origin"), {"numerator": 0, "denominator": 1}),
        "native accepted PCM is not the same full decode subject")
    return joined


def observe(probe_path, wav_path):
    probe_bytes = regular_bytes(probe_path, MAX_JSON_BYTES)
    native = native_audio(strict_json(probe_bytes))
    wav_bytes = regular_bytes(wav_path, MAX_WAV_BYTES)
    pcm = float32_wav(wav_bytes, native)
    require(pcm["nonpresented_tail_sample_frames"] == native["nonpresented_tail_sample_frames"],
            "decoded tail sample count differs")
    return {"schema": "aniflow.retained-aac-decoded-tail-observation/v1",
        "observer": {"sha256": digest(Path(__file__).read_bytes())},
        "native_probe": {"sha256": digest(probe_bytes), "bytes": len(probe_bytes)},
        "native": native, "complete_pcm": pcm,
        "presented_delivery_loudness_verified": False,
        "runtime_qualified": False, "publication_authority": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--wav", required=True, type=Path)
    parser.add_argument("--signal", type=Path, help="optional exact source/final signal receipt")
    parser.add_argument("--qc", type=Path, help="optional exact-final QC receipt")
    args = parser.parse_args()
    report = observe(args.probe, args.wav)
    for name, path, bind in (("signal", args.signal, bind_signal), ("qc", args.qc, bind_qc)):
        if path is not None:
            data = regular_bytes(path, MAX_JSON_BYTES)
            report[name] = {"file": {"sha256": digest(data), "bytes": len(data)},
                "join": bind(report["native"], report["complete_pcm"], strict_json(data))}
    print(json.dumps(report, ensure_ascii=False,
                     indent=2, sort_keys=True, allow_nan=False))


if __name__ == "__main__":
    main()
