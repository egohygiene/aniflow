#!/usr/bin/env python3
"""Finish a video-frames/Upscayl project. Python 3.8+, standard library only.

After Upscayl exits successfully, run from the project folder:
    python3 ~/Downloads/finish-video.py akashic.mp4

Requires ffmpeg, ffprobe, and your existing video-frames command on PATH.
No installs, downloads, uploads, watermark regeneration, or audio normalization.
Use --help for options. Outputs and logs are retained; inputs are never edited.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
from fractions import Fraction

VERSION = "1.0.0"
PNG_END = b"\x00\x00\x00\x00IEND\xaeB`\x82"


def fail(message):
    raise RuntimeError(message)


def say(message):
    print(message, flush=True)


def read_json(path):
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def write_json(path, value):
    with path.open("x", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, ensure_ascii=False)
        handle.write("\n")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(4 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def capture(command):
    result = subprocess.run(command, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True)
    if result.returncode or result.stderr.strip():
        fail("Command failed or reported an error:\n" +
             " ".join(map(str, command)) + "\n" + result.stderr[-8000:])
    return result.stdout


def run_logged(command, log, echo=True):
    # No shell: spaces and special characters in paths are passed literally.
    with log.open("x", encoding="utf-8") as handle:
        handle.write("ARGV: " + json.dumps(list(map(str, command))) + "\n\n")
        handle.flush()
        process = subprocess.Popen(command, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True,
                                   errors="replace")
        try:
            for line in process.stdout:
                handle.write(line)
                handle.flush()
                if echo:
                    print(line, end="", flush=True)
            code = process.wait()
        except BaseException:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            raise
        finally:
            process.stdout.close()
        if code:
            fail("Command exited with status {}. See {}".format(code, log))


def resolve_tool(name):
    found = shutil.which(name)
    if not found:
        fail("Required command not found: {}. Put it on PATH first.".format(name))
    # Do not resolve symlinks: Homebrew commands may rely on their invoked name.
    return os.path.abspath(found)


def preflight(frames, images, source, scale):
    for name in ("manifest.json", "source.ffprobe.json", "timing.ffprobe.json"):
        if not (frames / name).is_file():
            fail("Missing extraction metadata: " + str(frames / name))
    m = read_json(frames / "manifest.json")
    if m.get("schema") != "video-frames/1" or not m.get("complete"):
        fail("The frame extraction is incomplete or its manifest is unsupported.")
    if not m.get("fps") or Fraction(m["fps"]) <= 0:
        fail("This finishing preset requires verified constant frame rate.")
    if m.get("hdr") or m.get("alpha") or m["source_bit_depth"] > 8:
        fail("This delivery preset supports 8-bit SDR without alpha only.")
    if m["png_bit_depth"] != 8:
        fail("This delivery preset requires 8-bit RGB PNGs.")
    if not source.is_file() or not images.is_dir():
        fail("Source video or upscaled frame directory does not exist.")
    expected = ["{:08d}_frame.png".format(i)
                for i in range(1, m["frame_count"] + 1)]
    actual = sorted(p.name for p in images.iterdir() if p.suffix.lower() == ".png")
    if actual != expected or not expected:
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        fail("Upscale is incomplete or filenames differ: expected {} PNGs, found {}.\n"
             "Missing (first 5): {}\nUnexpected (first 5): {}\n"
             "Wait for Upscayl to finish; do not rename frames.".format(
                 len(expected), len(actual), missing[:5], extra[:5]))
    target = (m["width"] * scale, m["height"] * scale)
    if target[0] % 2 or target[1] % 2:
        fail("H.264 yuv420p requires even output dimensions.")
    snapshot = {}
    for name in expected:
        path = images / name
        with path.open("rb") as handle:
            header = handle.read(33)
            if (len(header) != 33 or header[:8] != b"\x89PNG\r\n\x1a\n"
                    or header[12:16] != b"IHDR"):
                fail("Invalid or unfinished PNG: " + str(path))
            width, height, depth, kind = struct.unpack(">IIBB", header[16:26])
            if (width, height, depth, kind) != (*target, 8, 2):
                fail("Expected {}x{} RGB24; got {}x{}, depth {}, color type {}: {}".format(
                    *target, width, height, depth, kind, path))
            handle.seek(-12, os.SEEK_END)
            if handle.read() != PNG_END:
                fail("PNG is unfinished or has unexpected trailing data: " + str(path))
        stat = path.stat()
        snapshot[name] = (stat.st_size, stat.st_mtime_ns)
    say("Checking the original source SHA-256...")
    if sha256(source) != m["source_sha256"]:
        fail("The supplied source differs from the original extraction source.")
    say("Ready: {} frames | {}x{} | {} fps | {} audio stream(s).".format(
        len(expected), *target, m["fps"], m["audio_streams"]))
    return m, target, snapshot


def check_unchanged(images, snapshot):
    actual = {p.name for p in images.iterdir() if p.suffix.lower() == ".png"}
    if actual != set(snapshot):
        fail("The image sequence changed during encoding. Wait for all editing to finish.")
    for name, before in snapshot.items():
        stat = (images / name).stat()
        if (stat.st_size, stat.st_mtime_ns) != before:
            fail("A frame changed during encoding: " + name)


def probe(ffprobe, path):
    return json.loads(capture([ffprobe, "-v", "error", "-show_streams",
                              "-show_format", "-show_chapters", "-of", "json", str(path)]))


def packets(ffprobe, path, audio_only=False):
    command = [ffprobe, "-v", "error"]
    if audio_only:
        command += ["-select_streams", "a"]
    command += ["-show_packets", "-show_data_hash", "sha256", "-show_entries",
                "packet=stream_index,pts,dts,duration,size,data_hash", "-of", "json", str(path)]
    grouped = {}
    for packet in json.loads(capture(command)).get("packets", []):
        grouped.setdefault(packet["stream_index"], []).append(packet)
    return grouped


def packet_digest(rows):
    digest = hashlib.sha256()
    for row in rows:
        if not row.get("data_hash"):
            fail("ffprobe did not supply packet hashes; cannot verify stream copying.")
        digest.update((str(row["size"]) + ":" + row["data_hash"] + "\n").encode("ascii"))
    return digest.hexdigest()


def verify_remux(ffprobe, encoded, delivery, source, m, target):
    before, after = probe(ffprobe, encoded), probe(ffprobe, delivery)
    left, right = packets(ffprobe, encoded), packets(ffprobe, delivery)
    a_streams, b_streams = before["streams"], after["streams"]
    if len(a_streams) != len(b_streams):
        fail("Stream count changed during metadata cleanup.")
    video = next(s for s in b_streams if s["codec_type"] == "video")
    if (video["width"], video["height"]) != target or video["pix_fmt"] != "yuv420p":
        fail("Unexpected final dimensions or pixel format.")
    if int(video.get("nb_frames", -1)) != m["frame_count"]:
        fail("Final MP4 frame count differs from the extraction manifest.")
    checks = []
    for a, b in zip(a_streams, b_streams):
        for field in ("codec_type", "codec_name", "width", "height", "pix_fmt",
                      "color_range", "color_space", "color_transfer", "color_primaries",
                      "sample_aspect_ratio", "sample_rate", "channels"):
            if a.get(field) != b.get(field):
                fail("Stream property changed during cleanup: " + field)
        ap, bp = left.get(a["index"], []), right.get(b["index"], [])
        if not ap or len(ap) != len(bp) or packet_digest(ap) != packet_digest(bp):
            fail("Encoded audio/video packet content changed during metadata cleanup.")
        atb, btb = Fraction(a["time_base"]), Fraction(b["time_base"])
        tolerance = 2 * max(atb, btb)
        for p, q in zip(ap, bp):
            for field in ("pts", "dts", "duration"):
                if field not in p or field not in q:
                    if p.get(field) != q.get(field):
                        fail("Packet timing information changed during cleanup.")
                elif abs(p[field] * atb - q[field] * btb) > tolerance:
                    fail("Packet timing changed during metadata cleanup.")
        checks.append({"stream": b["index"], "type": b["codec_type"],
                       "packets": len(bp), "packet_content_sha256": packet_digest(bp)})
    # Independently verify copied audio payloads against the ORIGINAL source.
    original = probe(ffprobe, source)
    original_audio = [s for s in original["streams"] if s["codec_type"] == "audio"]
    final_audio = [s for s in b_streams if s["codec_type"] == "audio"]
    if len(original_audio) != len(final_audio):
        fail("Original audio stream count was not preserved.")
    source_packets = packets(ffprobe, source, audio_only=True) if original_audio else {}
    for a, b in zip(original_audio, final_audio):
        ap, bp = source_packets.get(a["index"], []), right.get(b["index"], [])
        if not ap or len(ap) != len(bp) or packet_digest(ap) != packet_digest(bp):
            fail("Copied audio packets differ from the original. Review the retained files; "
                 "this can also indicate a container/codec compatibility issue.")
    return after, checks


def main():
    parser = argparse.ArgumentParser(
        description="Finish an Upscayl PNG sequence using your existing video-frames workspace.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""Example (run AFTER Upscayl has exited successfully):
  python3 finish-video.py akashic.mp4
  python3 finish-video.py akashic.mp4 --check-only
  python3 finish-video.py akashic.mp4 --output-dir release-v2 --title Akashic

Default output: release/<source-name>-upscaled.mp4
Also retained: work/encoded.mp4, logs, ffprobe reports, SHA256SUMS, summary.json.
The MP4 is lossy H.264; --lossless-master also creates an FFV1 RGB MKV (large).
Frames, source video, and your previous exports are never modified.
Use a NEW output directory for each run; existing output directories are refused.

Metadata scope: suppress copied container/stream tags and chapters. The muxer
can add structural tags such as brands and handler names. Color information is
preserved. This is NOT a forensic scrub of all codec/container fields or a
verified removal of C2PA, SynthID, invisible pixel/audio watermarks, or AI cues.
No watermark remover, re-upscaling, sharpening, denoising, or normalization runs.
Upload the delivery MP4; local reports can contain source tags and local paths.
""")
    parser.add_argument("source", nargs="?", default="akashic.mp4")
    parser.add_argument("--frames-dir", default=".frames", help="Original extraction workspace")
    parser.add_argument("--images-dir", default=".frames-4k", help="Completed upscaled PNG directory")
    parser.add_argument("--output-dir", default="release", help="New output directory (default: release)")
    parser.add_argument("--scale", type=int, choices=(1, 2, 3, 4), default=2,
                        help="Expected output scale relative to source (default: 2)")
    parser.add_argument("--crf", type=int, default=17, help="H.264 quality, 0-51 (default: 17)")
    parser.add_argument("--preset", default="medium", choices=("ultrafast", "superfast", "veryfast",
                        "faster", "fast", "medium", "slow", "slower", "veryslow"))
    parser.add_argument("--title", help="Optional title to add to the delivery MP4")
    parser.add_argument("--artist", help="Optional artist to add to the delivery MP4")
    parser.add_argument("--lossless-master", action="store_true", help="Also encode a large FFV1 RGB master")
    parser.add_argument("--skip-loudness", action="store_true", help="Skip read-only audio loudness measurement")
    parser.add_argument("--check-only", action="store_true", help="Validate frames/source without creating outputs")
    parser.add_argument("--video-frames", default="video-frames", help="Path or command name of installed video-frames")
    parser.add_argument("--version", action="version", version=VERSION)
    args = parser.parse_args()
    if not 0 <= args.crf <= 51:
        parser.error("--crf must be between 0 and 51")
    source = Path(args.source).expanduser().resolve()
    frames = Path(args.frames_dir).expanduser().resolve()
    images = Path(args.images_dir).expanduser().resolve()
    output = Path(args.output_dir).expanduser().resolve()
    ffmpeg, ffprobe, stitcher = [resolve_tool(n) for n in ("ffmpeg", "ffprobe", args.video_frames)]
    if not args.check_only and output.exists():
        fail("Output directory already exists: {}. Choose a new --output-dir.".format(output))
    for protected in (frames, images):
        if output == protected or protected in output.parents:
            fail("Put the output directory outside the frame directories.")
    m, target, snapshot = preflight(frames, images, source, args.scale)
    if args.check_only:
        say("Preflight passed. No files created. PNG headers/end markers were checked; "
            "the actual encode also decodes every image.")
        return
    output.mkdir(parents=True, exist_ok=False)
    work, reports = output / "work", output / "reports"
    work.mkdir()
    reports.mkdir()
    encoded = work / "encoded.mp4"
    delivery = output / (source.stem + "-upscaled.mp4")
    base = [stitcher, "stitch", "--frames-dir", str(frames), "--images-dir", str(images),
            "--source", str(source)]
    say("Encoding the upscaled frames; the existing stitcher verifies frame timing and audio sync...")
    run_logged(base + ["--output", str(encoded), "--crf", str(args.crf), "--preset", args.preset],
               reports / "stitch.log")
    check_unchanged(images, snapshot)
    say("Creating the delivery MP4 with minimal descriptive metadata (no re-encoding)...")
    command = [ffmpeg, "-hide_banner", "-nostdin", "-loglevel", "warning", "-stats", "-xerror", "-n",
               "-copyts", "-i", str(encoded), "-map", "0:v:0", "-map", "0:a?", "-codec", "copy",
               "-map_metadata", "-1", "-map_metadata:s", "-1", "-map_chapters", "-1",
               "-metadata", "encoder=", "-metadata:s", "encoder=",
               "-avoid_negative_ts", "disabled", "-movflags", "+faststart"]
    for key in ("title", "artist"):
        if getattr(args, key) is not None:
            command += ["-metadata", key + "=" + getattr(args, key)]
    run_logged(command + [str(delivery)], reports / "metadata-cleanup.log")
    say("Verifying final dimensions, frame count, packet timing, and unchanged audio/video payloads...")
    final_probe, checks = verify_remux(ffprobe, encoded, delivery, source, m, target)
    write_json(reports / "delivery.ffprobe.json", final_probe)
    masters = []
    if args.lossless_master:
        master = output / (source.stem + "-upscaled-master.mkv")
        say("Encoding the optional FFV1 master, lossless relative to the RGB PNG pixels...")
        run_logged(base + ["--output", str(master), "--lossless"], reports / "master-stitch.log")
        check_unchanged(images, snapshot)
        masters.append(master)
    loudness_logs = []
    if not args.skip_loudness:
        audio_streams = [s for s in final_probe["streams"] if s["codec_type"] == "audio"]
        for i, _ in enumerate(audio_streams):
            log = reports / "audio-{}-loudness.log".format(i)
            say("Measuring audio stream {} loudness/true peak without changing it...".format(i))
            run_logged([ffmpeg, "-hide_banner", "-nostdin", "-nostats", "-loglevel", "info", "-xerror",
                        "-i", str(delivery), "-map", "0:a:{}".format(i), "-filter:a",
                        "ebur128=peak=true:framelog=verbose", "-f", "null", "-"], log, echo=False)
            loudness_logs.append(str(log.relative_to(output)))
            text = log.read_text(encoding="utf-8")
            if "Summary:" in text:
                say(text[text.rfind("Summary:"):].strip())
    say("Writing delivery checksums and the final report...")
    hashes = {p.name: sha256(p) for p in [delivery] + masters}
    with (output / "SHA256SUMS").open("x", encoding="utf-8") as handle:
        for name, digest in hashes.items():
            handle.write(digest + "  " + name + "\n")
    write_json(output / "summary.json", {
        "status": "verified", "finisher_version": VERSION, "source": str(source),
        "source_sha256": m["source_sha256"], "images": str(images),
        "frame_count": m["frame_count"], "fps": m["fps"], "width": target[0], "height": target[1],
        "delivery": delivery.name, "settings": {"crf": args.crf, "preset": args.preset,
        "codec": "libx264", "pixel_format": "yuv420p", "audio": "original packets copied"},
        "verification": {"stitcher": "frame count, decoded frame timing, audio layout and start offset",
        "metadata_remux": "identical compressed payloads and packet timing", "streams": checks,
        "original_audio_payloads_identical": True, "frame_files_unchanged_during_encode": True},
        "loudness_reports": loudness_logs, "sha256": hashes,
        "metadata_scope": "Copied container/stream tags and chapters suppressed; muxer may add structural tags.",
        "invisible_watermark_status": "Not tested or removed. No detector-evasion claim.",
        "notes": ["Delivery MP4 is lossy; original and upscaled PNGs remain unchanged.",
                  "Only share the delivery MP4; reports/work files are local production artifacts."]})
    say("\nDone: " + str(delivery))
    say("Verified {} frames at {}x{}; original audio preserved. Reports: {}".format(
        m["frame_count"], *target, reports))
    say("Watch the final MP4 for motion, repair transitions, color, and lip sync before posting.")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped. Inputs are untouched; partial outputs/logs are retained. "
              "Use a new --output-dir for a fresh run.", file=sys.stderr)
        sys.exit(130)
    except (RuntimeError, OSError, ValueError, KeyError, StopIteration) as error:
        print("\nSTOP: " + str(error), file=sys.stderr)
        print("Inputs are untouched. Any partial outputs/logs are retained; "
              "a failed run does not produce summary.json with status verified.", file=sys.stderr)
        sys.exit(1)
