"""Pure byte recipes. Generation is independent of codecs, locale and wall time."""

from __future__ import annotations

import binascii
import copy
import importlib.util
import json
import struct
import zlib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = "1.0.0"
SEED = 240034


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")


@dataclass
class Recipe:
    name: str
    family: str
    validity: str
    files: dict[str, bytes]
    oracle: dict
    parameters: dict = field(default_factory=dict)
    corruption: str | None = None
    directories: list[str] = field(default_factory=list)

    @property
    def id(self):
        return "ani.corpus." + self.name


def png(width=2, height=2, alpha=False, variant=0, metadata=False):
    """PNG with stored DEFLATE blocks, so zlib compressor versions cannot drift."""
    channels = 4 if alpha else 3
    rows = b"".join(b"\x00" + bytes((SEED + variant + x * 31 + y * 17 + c * 71) % 256
                                   for x in range(width) for c in range(channels))
                    for y in range(height))
    chunks = []
    for offset in range(0, len(rows), 65535):
        block = rows[offset:offset + 65535]
        chunks.append(bytes([int(offset + len(block) == len(rows))])
                      + struct.pack("<HH", len(block), 65535 - len(block)) + block)
    compressed = b"\x78\x01" + b"".join(chunks) + struct.pack(">I", zlib.adler32(rows) & 0xffffffff)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data) & 0xffffffff)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6 if alpha else 2, 0, 0, 0))
    if metadata:
        data += chunk(b"sRGB", b"\x00") + chunk(b"tEXt", b"Description\x00aniflow synthetic seed 240034")
    return data + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")


def wav(rate=8000, channels=1, bits=16, frames=257, pattern="silence"):
    payload = bytearray()
    for frame in range(frames):
        for channel in range(channels):
            if pattern == "silence":
                value = 0
            elif pattern == "impulse":
                value = 16000 if frame == frames - 1 else 0
            elif pattern == "clipped":
                value = 32767 if frame % 2 else -32768
            elif pattern == "discontinuity":
                value = 12000 if frame >= frames // 2 else 0
            else:
                # Integer square tone; no platform-dependent trigonometry.
                value = (12000 if frame % 8 < 4 else -12000) * (-1 if channel % 2 else 1)
            if bits == 8:
                payload.extend(struct.pack("<B", (value + 32768) // 256))
            elif bits == 16:
                payload.extend(struct.pack("<h", value))
            else:
                payload.extend((value << 8).to_bytes(3, "little", signed=True))
    alignment = channels * bits // 8
    fmt = struct.pack("<HHIIHH", 1, channels, rate, rate * alignment, alignment, bits)
    body = b"WAVEfmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(payload)) + payload
    if len(payload) % 2:
        body += b"\x00"
    return b"RIFF" + struct.pack("<I", len(body)) + body


def y4m(rate="24000:1001", frames=4, width=4, height=4):
    header = f"YUV4MPEG2 W{width} H{height} F{rate} Ip A1:1 C420jpeg\n".encode()
    return header + b"".join(b"FRAME\n" + bytes([16 + n * 10]) * (width * height)
                             + b"\x80" * (width * height // 2) for n in range(frames))


def native_oracle(outcome, **properties):
    return {"kind": "native_decode", "outcome": outcome,
            "codes": ["native_decode_refused"] if outcome == "reject" else [], "properties": properties}


def media_recipes():
    cases = []
    for name, width, height, alpha, metadata in [
        ("rgb", 2, 2, False, False), ("rgba", 2, 2, True, False),
        ("color-metadata", 2, 2, False, True), ("wide", 256, 1, False, False),
        ("tall", 1, 256, False, False),
    ]:
        cases.append(Recipe("image." + name, "image", "boundary" if width != height else "valid",
                            {"input.png": png(width, height, alpha, metadata=metadata)},
                            native_oracle("decode", width=width, height=height, codec_name="png"),
                            {"width": width, "height": height, "alpha": alpha, "metadata": metadata}))
    good = png()
    for name, data, corruption in [
        ("truncated", good[:37], "Truncate after IHDR; no pixel payload."),
        ("bad-signature", b"NOTPNG!!" + good[8:], "Replace the eight-byte signature."),
        ("empty", b"", "Truncate the entire file."),
    ]:
        cases.append(Recipe("image." + name, "image", "malformed", {"input.png": data},
                            native_oracle("reject"), corruption=corruption))
    cases.append(Recipe("image.wrong-extension", "image", "boundary", {"input.jpg": good},
                        native_oracle("decode", width=2, height=2, codec_name="png"),
                        {"actual_type": "image/png", "declared_extension": "jpg"}))
    # Header-only pathological dimensions never carry a large pixel allocation.
    for name, width in [("zero-width", 0), ("huge-width", 2147483647)]:
        header = bytearray(good[:33])
        header[16:20] = struct.pack(">I", width)
        header[29:33] = struct.pack(">I", binascii.crc32(header[12:29]) & 0xffffffff)
        cases.append(Recipe("image." + name, "image", "resource_stress", {"input.png": bytes(header)},
                            native_oracle("reject"), {"declared_width": width, "payload_pixels": 0},
                            "Rewrite width and IHDR CRC, then omit all pixel chunks."))
    for name, names, dimensions in [
        ("empty", [], []), ("single", ["0001.png"], [(2, 2)]),
        ("ordered", ["0001.png", "0002.png", "0003.png"], [(2, 2)] * 3),
        ("gap", ["0001.png", "0003.png"], [(2, 2)] * 2),
        ("nonzero-start", ["0042.png", "0043.png"], [(2, 2)] * 2),
        ("mixed-dimensions", ["0001.png", "0002.png"], [(2, 2), (4, 2)]),
        ("ambiguous-names", ["1.png", "01.png", "0001.png"], [(2, 2)] * 3),
        ("lexical-order", ["2.png", "10.png", "1.png"], [(2, 2)] * 3),
    ]:
        files = {"frames/" + name: png(*size, variant=i) for i, (name, size) in enumerate(zip(names, dimensions))}
        cases.append(Recipe("frames." + name, "frame_sequence", "valid" if name in ("single", "ordered") else "boundary",
                            files, {"kind": "inventory", "outcome": "observe", "codes": [],
                                    "properties": {"frame_count": len(files), "lexical_order": sorted(files)}},
                            {"names": names, "dimensions": dimensions}, directories=["frames"]))
    duplicate = {"frames/0001.png": good, "frames/0002.png": good}
    cases.append(Recipe("frames.duplicate-content", "frame_sequence", "boundary", duplicate,
                        {"kind": "inventory", "outcome": "observe", "codes": [], "properties": {"frame_count": 2}},
                        directories=["frames"]))
    for name, rate, channels, bits, frames, pattern in [
        ("silence", 8000, 1, 16, 257, "silence"), ("tail-impulse", 8000, 1, 16, 257, "impulse"),
        ("square-tone", 8000, 1, 16, 256, "tone"), ("stereo", 44100, 2, 16, 257, "tone"),
        ("six-channels", 48000, 6, 16, 64, "tone"), ("unsigned-8", 8000, 1, 8, 257, "tone"),
        ("signed-24", 48000, 1, 24, 257, "tone"), ("clipped", 8000, 1, 16, 257, "clipped"),
        ("discontinuity", 8000, 1, 16, 257, "discontinuity"), ("single-sample", 8000, 1, 16, 1, "impulse"),
        ("fractional-video-span", 48000, 1, 16, 8008, "silence"),
    ]:
        cases.append(Recipe("audio." + name, "audio", "valid" if name in ("silence", "square-tone") else "boundary",
                            {"input.wav": wav(rate, channels, bits, frames, pattern)},
                            native_oracle("decode", sample_rate=str(rate), channels=channels,
                                          codec_name={8: "pcm_u8", 16: "pcm_s16le", 24: "pcm_s24le"}[bits]),
                            {"sample_rate": rate, "channels": channels, "bits": bits, "frames": frames, "pattern": pattern}))
    for name, data, description in [
        ("bad-riff", b"BAD!" + wav()[4:], "Replace RIFF signature."),
        ("truncated-header", wav()[:15], "Truncate in the fmt chunk header."),
    ]:
        cases.append(Recipe("audio." + name, "audio", "malformed", {"input.wav": data},
                            native_oracle("reject"), corruption=description))
    for name, rate, frames in [("cfr-24", "24:1", 4), ("cfr-fractional", "24000:1001", 4),
                              ("single", "30:1", 1), ("odd-duration", "30000:1001", 5)]:
        cases.append(Recipe("video." + name, "container", "boundary", {"input.y4m": y4m(rate, frames)},
                            native_oracle("decode", width=4, height=4, codec_name="rawvideo", nb_read_frames=str(frames)),
                            {"rate": rate, "frames": frames}))
    cases.append(Recipe("video.corrupt-header", "container", "malformed", {"input.y4m": b"BADMAGIC\nFRAME\n"},
                        native_oracle("reject"), corruption="Replace Y4M signature and omit payload."))
    return cases


def text_recipes():
    srt = "1\n00:00:00,250 --> 00:00:01,500\nHello café 雪 🌍\n\n2\n00:00:02,000 --> 00:00:03,750\nمرحبا שלום\n"
    values = [
        ("unicode-srt", "srt", srt.encode(), "valid", 2, None),
        ("bom-crlf", "srt", b"\xef\xbb\xbf" + srt.replace("\n", "\r\n").encode(), "boundary", 2, None),
        ("empty-cue", "srt", b"1\n00:00:00,000 --> 00:00:01,000\n", "malformed", None, "Remove cue text."),
        ("reverse-interval", "srt", b"1\n00:00:02,000 --> 00:00:01,000\nx\n", "malformed", None, "Reverse start and end."),
        ("overlap", "srt", srt.replace("00:00:02,000", "00:00:01,000").encode(), "unsupported", None, None),
        ("out-of-order", "srt", ("1\n00:00:02,000 --> 00:00:03,000\nx\n\n2\n00:00:00,000 --> 00:00:01,000\ny\n").encode(), "malformed", None, "Swap temporal order."),
        ("invalid-utf8", "srt", b"\xff\xfe\x80", "malformed", None, "Replace UTF-8 with invalid bytes."),
        ("nul", "plain", b"text\x00text", "malformed", None, "Insert NUL."),
        ("unicode-plain", "plain", "café\n雪 🌍\nمرحبا שלום\n".encode(), "valid", 1, None),
        ("unicode-lrc", "lrc", "[00:00.25]café\n[00:02.00]雪 🌍\n".encode(), "valid", 2, None),
        ("webvtt", "webvtt", b"WEBVTT\n\n00:00:00.250 --> 00:00:01.500\nhello\n", "valid", 1, None),
        ("webvtt-style", "webvtt", b"WEBVTT\n\nSTYLE\n::cue {color:red}\n\n00:00:00.250 --> 00:00:01.500\nx\n", "unsupported", None, None),
        ("ttml", "ttml", b'<tt xmlns="http://www.w3.org/ns/ttml" xml:space="preserve"><body><div><p begin="00:00:00.250" end="00:00:01.500">hello</p></div></body></tt>', "valid", 1, None),
        ("ttml-doctype", "ttml", b'<!DOCTYPE tt [<!ENTITY x "bounded">]><tt><body><div><p begin="00:00:00.250" end="00:00:01.500">&x;</p></div></body></tt>', "unsupported", None, None),
        ("byte-limit", "plain", b"x" * 1_048_577, "resource_stress", None, None),
        ("cue-limit", "plain", b"x" * 65_537, "resource_stress", None, None),
    ]
    return [Recipe("text." + name, "timed_text", validity, {"input." + fmt: data},
                   {"kind": "timed_text", "outcome": "accept" if count is not None else "reject",
                    "codes": [] if count is not None else ["configuration"],
                    "properties": {"format": fmt, "cue_count": count}}, corruption=corruption)
            for name, fmt, data, validity, count, corruption in values]


def protocol_recipes():
    spec = importlib.util.spec_from_file_location("temporal_recipes", ROOT / "scripts/generate-temporal-fixtures.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    results = []
    # Import the owning generator, preserving its expected typed diagnostics.
    fixtures = module.fixtures()
    for name, streams in [("no-streams", []), ("audio-only", [module.audio()[0]])]:
        value = copy.deepcopy(fixtures[0])
        value["case_id"] = name
        value["inventory"]["streams"] = streams
        value["observations"] = []
        value["expected"] = ({"outcome": "parser_error", "codes": ["probe_limit"]} if not streams
                             else {"outcome": "refused", "codes": ["missing_video"]})
        fixtures.append(value)
    for value in fixtures:
        expected = value["expected"]
        results.append(Recipe("temporal." + value["case_id"], "temporal_protocol",
                              "valid" if expected["outcome"] == "supported" else
                              "malformed" if expected["outcome"] == "parser_error" else "unsupported",
                              {"probe.json": json_bytes(value)},
                              {"kind": "temporal_protocol", "outcome": expected["outcome"],
                               "codes": expected["codes"], "properties": {}},
                              {"generator_case": value["case_id"]},
                              "Owning temporal generator authors the named malformed probe fields."
                              if expected["outcome"] == "parser_error" else None))
    return results


def recipes():
    return sorted(media_recipes() + text_recipes() + protocol_recipes(), key=lambda item: item.id)
