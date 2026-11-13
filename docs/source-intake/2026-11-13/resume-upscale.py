#!/usr/bin/env python3
"""Resume a video-frames PNG upscale on macOS/Linux. Python 3.8+, no packages.

Run from your project directory with the original Upscayl batch stopped:
    caffeinate -i python3 ~/Downloads/resume-upscale.py

Reuses your installed upscayl-bin and model files. Sources are never modified.
Existing outputs are checked for PNG structure, CRCs, size and RGB24 format.
This detects incomplete/corrupt files, not black images or visual artifacts.
It cannot infer which model produced an existing image: use the SAME model and
scale as the original run. Defaults match the Akashic digital-art-4x project.
"""

import argparse
import fcntl
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import uuid
import zlib


def say(message):
    print(message, flush=True)


def fail(message):
    raise RuntimeError(message)


def png_problem(path, expected):
    """Validate structure/CRCs with bounded memory; do not decompress pixels."""
    if not path.exists():
        return "missing"
    try:
        with path.open("rb") as handle:
            if handle.read(8) != b"\x89PNG\r\n\x1a\n":
                return "invalid PNG signature"
            first, idat = True, False
            while True:
                header = handle.read(8)
                if len(header) != 8:
                    return "truncated PNG (no complete IEND)"
                length, kind = struct.unpack(">I4s", header)
                if first and (kind != b"IHDR" or length != 13):
                    return "missing IHDR"
                if not first and kind == b"IHDR":
                    return "duplicate IHDR"
                crc, remaining, data = zlib.crc32(kind), length, b""
                while remaining:
                    chunk = handle.read(min(remaining, 1024 * 1024))
                    if not chunk:
                        return "truncated PNG chunk"
                    crc = zlib.crc32(chunk, crc)
                    if kind == b"IHDR":
                        data += chunk
                    remaining -= len(chunk)
                stored = handle.read(4)
                if len(stored) != 4 or struct.unpack(">I", stored)[0] != (crc & 0xFFFFFFFF):
                    return "PNG checksum mismatch"
                if first:
                    w, h, depth, color, compression, filters, interlace = struct.unpack(">IIBBBBB", data)
                    if (w, h, depth, color, compression, filters, interlace) != (*expected, 8, 2, 0, 0, 0):
                        return "expected {}x{} noninterlaced RGB24; found {}x{}, depth {}, type {}".format(
                            *expected, w, h, depth, color)
                    first = False
                elif kind == b"IDAT" and length:
                    idat = True
                elif kind == b"IEND":
                    if length or not idat or handle.read(1):
                        return "invalid PNG ending or missing image data"
                    return None
    except (OSError, struct.error) as error:
        return str(error)


def run_engine(command, log):
    with log.open("x", encoding="utf-8") as handle:
        handle.write("ARGV: " + json.dumps(list(map(str, command))) + "\n\n")
        handle.flush()
        process = subprocess.Popen(command, stdout=handle, stderr=subprocess.STDOUT)
        try:
            result = process.wait()
        except BaseException:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            raise
        if result:
            reason = "signal {}".format(-result) if result < 0 else "exit status {}".format(result)
            fail("Upscayl stopped with {}. Saved frames are retained.\nLog: {}\n"
                 "You can rerun this script to resume again.".format(reason, log))


def main():
    parser = argparse.ArgumentParser(
        description="Check existing Upscayl outputs and resume missing/corrupt frames.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""Examples (run from the project folder):
  python3 resume-upscale.py --check-only
  caffeinate -i python3 resume-upscale.py
  caffeinate -i python3 resume-upscale.py --limit 5

Defaults match Akashic: digital-art-4x, native 4x model, final 2x output.
One Upscayl process per frame, 128-pixel tiles, load/proc/save threads 1:1:1.
These settings aim to reduce peak memory and avoid growth across a long batch;
they cannot guarantee a driver/system issue will not recur. Smaller tiles and
reloading the model can be slower. Check motion around the first resumed frame:
changing tile size can slightly change results at tile boundaries.

Valid existing files are skipped. Replacements are checked before publication;
invalid originals and failed temporary files remain in .upscale-resume inside
the output directory. Source PNGs are never modified. Logs are kept per frame.
PNG checking verifies structure/checksums, not full decoding or visual quality.
Run only one upscaler on this output directory at a time.
""")
    parser.add_argument("--input-dir", default=".frames")
    parser.add_argument("--output-dir", default=".frames-4k")
    parser.add_argument("--models-dir", default="/Applications/Upscayl.app/Contents/Resources/models")
    parser.add_argument("--model", default="digital-art-4x")
    parser.add_argument("--model-scale", type=int, choices=(2, 3, 4), default=4)
    parser.add_argument("--scale", type=int, choices=(2, 3, 4), default=2)
    parser.add_argument("--tile-size", type=int, default=128)
    parser.add_argument("--upscayl-bin", default="upscayl-bin")
    parser.add_argument("--limit", type=int, help="Process at most this many pending frames")
    parser.add_argument("--check-only", action="store_true", help="Print saved/pending counts without changing files")
    args = parser.parse_args()
    if args.tile_size < 32:
        parser.error("--tile-size must be at least 32")
    if args.limit is not None and args.limit < 1:
        parser.error("--limit must be positive")
    inputs = Path(args.input_dir).expanduser().resolve()
    outputs = Path(args.output_dir).expanduser().resolve()
    if inputs == outputs or inputs in outputs.parents or outputs in inputs.parents:
        fail("Input and output must be separate, non-nested directories.")
    with (inputs / "manifest.json").open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("schema") != "video-frames/1" or not manifest.get("complete"):
        fail("Expected a completed video-frames workspace.")
    if manifest.get("png_bit_depth") != 8 or manifest.get("alpha"):
        fail("This resume preset expects 8-bit RGB source PNGs without alpha.")
    count = manifest["frame_count"]
    names = ["{:08d}_frame.png".format(i) for i in range(1, count + 1)]
    actual = sorted(p.name for p in inputs.iterdir() if p.suffix.lower() == ".png")
    if not names or actual != names:
        fail("Source PNG sequence differs from its extraction manifest.")
    if outputs.exists():
        extra = {p.name for p in outputs.iterdir() if p.suffix.lower() == ".png"} - set(names)
        if extra:
            fail("Unexpected output PNG filenames (first 5): " + repr(sorted(extra)[:5]))
    target = (manifest["width"] * args.scale, manifest["height"] * args.scale)
    say("Checking saved PNG checksums and dimensions; this may take a few minutes...")
    pending, invalid = [], 0
    for i, name in enumerate(names, 1):
        problem = png_problem(outputs / name, target)
        if problem:
            pending.append((name, problem))
            if problem != "missing":
                invalid += 1
        if i % 250 == 0:
            say("Checked {}/{}; {} valid so far.".format(i, count, i - len(pending)))
    saved = count - len(pending)
    say("Saved and valid: {}/{} | Missing: {} | Invalid/incomplete: {}".format(
        saved, count, len(pending) - invalid, invalid))
    if pending:
        say("First pending frame: " + pending[0][0])
    if args.check_only or not pending:
        if not pending:
            say("All frames passed PNG checks. You can run finish-video.py.")
        return
    executable = shutil.which(args.upscayl_bin)
    if not executable:
        fail("Cannot find upscayl-bin on PATH; use --upscayl-bin to supply its location.")
    models = Path(args.models_dir).expanduser().resolve()
    for ext in (".param", ".bin"):
        if not (models / (args.model + ext)).is_file():
            fail("Missing model file: " + str(models / (args.model + ext)))
    outputs.mkdir(parents=True, exist_ok=True)
    state = outputs / ".upscale-resume"
    state.mkdir(exist_ok=True)
    with (state / "lock").open("a") as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            fail("Another resume-upscale process is using this output directory.")
        completed, started = 0, time.monotonic()
        for name, _ in pending:
            dest = outputs / name
            if png_problem(dest, target) is None:
                saved += 1
                continue
            if shutil.disk_usage(outputs).free < 1024 ** 3:
                fail("Less than 1 GiB of disk space remains. Free space, then rerun to resume.")
            problem = png_problem(inputs / name, (manifest["width"], manifest["height"]))
            if problem:
                fail("Invalid source frame {}: {}".format(name, problem))
            job = Path(tempfile.mkdtemp(prefix=name[:-4] + "-", dir=str(state)))
            temporary, log = job / "output.png", job / "upscayl.log"
            say("[{}/{} saved] Upscaling {} -> {}x{} | Log: {}".format(
                saved, count, name, *target, log))
            command = [executable, "-i", str(inputs / name), "-o", str(temporary),
                       "-m", str(models), "-n", args.model, "-z", str(args.model_scale),
                       "-s", str(args.scale), "-t", str(args.tile_size), "-j", "1:1:1", "-f", "png"]
            run_engine(command, log)
            problem = png_problem(temporary, target)
            if problem:
                fail("New output failed validation: {}. Retained at {}".format(problem, temporary))
            # Recheck in case another program wrote a valid destination during our work.
            if png_problem(dest, target) is None:
                say("Destination is already valid; retaining this extra render at " + str(temporary))
            else:
                if dest.exists() or dest.is_symlink():
                    backup = job / ("previous-invalid-" + uuid.uuid4().hex + ".png")
                    dest.rename(backup)
                # Atomic, no-overwrite publication on the same filesystem.
                os.link(temporary, dest)
                temporary.unlink()
            completed += 1
            saved += 1
            average = (time.monotonic() - started) / completed
            say("Saved {}/{} ({:.1f}%). Average {:.1f}s/frame this run.\n".format(
                saved, count, 100 * saved / count, average))
            if args.limit is not None and completed >= args.limit:
                break
        if saved == count:
            say("Upscale complete. Run finish-video.py next, then review the video in motion.")
        else:
            say("Stopped at --limit with {} frames remaining. Rerun the same command without --limit.".format(count - saved))


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped. Completed frames and partial work are retained. Rerun to resume.", file=sys.stderr)
        sys.exit(130)
    except (RuntimeError, OSError, ValueError, KeyError) as error:
        print("\nSTOP: " + str(error), file=sys.stderr)
        sys.exit(1)
