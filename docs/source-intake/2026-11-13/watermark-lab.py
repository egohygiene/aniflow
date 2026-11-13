#!/usr/bin/env python3
"""Short-clip invisible VIDEO watermark experiment, not a production cleaner.

Prepare uses Python 3.8+ and FFmpeg/ffprobe; setup needs Python 3.11-3.14.
Run from the project folder AFTER upscale and finish-video.py complete:

  python3 watermark-lab.py prepare release/akashic-upscaled.mp4 --start-frame 3284
  python3 watermark-lab.py setup
  python3 watermark-lab.py process --frames 2 --attempt tiled-smoke

Then omit --frames and use a new --attempt name to process the whole reference.
Each retry writes WORK/attempts/NAME; existing reference and attempts are retained.

Setup explicitly downloads a pinned backend and a VAE model into an isolated
local runtime. Processing is offline. It requires working Apple MPS by default;
--device cuda or --device cpu are explicit alternatives. No GPU run is claimed
tested by this wrapper's author. Processing uses VAE tiling and a process-group
memory/time/disk watchdog. MPS also has an allocator cap. These are layered
guards, not an OS-wide memory guarantee. Defaults target an 18 GiB Mac.

The reference/candidate are SILENT, limited to at most 240 frames. Your song,
master and upscaled frames remain untouched. Audio-watermark work is out of scope.
The full-resolution reference is losslessly encoded from the decoded source;
the candidate changes pixels and is lossy. Source resolution/rate are requested
explicitly, bypassing upstream's 512-pixel/12-fps defaults. That does NOT transfer
upstream removal claims to this configuration. A compatible external detector
is still needed to evaluate watermark effectiveness on this exact output.

No automatic full-video run, upload, watermark-verdict claim or Aniflow install.
Use --help, or the subcommand followed by --help, for options.
"""

import argparse
from contextlib import contextmanager
from dataclasses import asdict
from datetime import datetime, timezone
import fcntl
from fractions import Fraction
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
import uuid

REVISION = "d25cada76bc39bab59cd7a41c29028b1cc98a5f2"
MODEL = "stabilityai/sd-vae-ft-mse"
RUNTIME = Path.home() / ".local/share/aniflow-watermark-lab" / REVISION[:12]
GIB = 1024 ** 3
WRAPPER_VERSION = "2.0.0"


@contextmanager
def workspace_lock(work):
    # The worker inherits this descriptor, so an orphaned worker keeps the lock.
    with (work / "processing.lock").open("a+") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            fail("Another process still holds this workspace lock. Stop that run before retrying.")
        yield handle.fileno()


def physical_memory():
    if sys.platform == "darwin":
        return int(run(["/usr/sbin/sysctl", "-n", "hw.memsize"]).strip())
    return os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")


def group_rss(pgid):
    # Includes the worker and its ffmpeg children. GPU accounting may differ
    # from RSS; the MPS allocator has an independent limit in the worker.
    result = subprocess.run([tool("ps"), "-axo", "pgid=,rss="], capture_output=True, text=True, timeout=5)
    if result.returncode:
        fail("Cannot monitor process memory: " + result.stderr[-1000:])
    return sum(int(fields[1]) * 1024 for line in result.stdout.splitlines()
               if len(fields := line.split()) == 2 and int(fields[0]) == pgid)


def stop_group(process):
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(process.pid, sig)
        except ProcessLookupError:
            pass
        if sig == signal.SIGTERM:
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
    process.wait()


def guarded_run(command, log, env, lock_fd, args, budget):
    peak, started, next_heartbeat = 0, time.monotonic(), 0.0
    status, reason, process = "failed", None, None
    try:
        with log.open("x", encoding="utf-8") as handle:
            handle.write("ARGV: " + json.dumps(list(map(str, command))) + "\n\n")
            handle.flush()
            process = subprocess.Popen(command, stdout=handle, stderr=subprocess.STDOUT,
                                       env=env, start_new_session=True, pass_fds=(lock_fd,))
            while process.poll() is None:
                elapsed = time.monotonic() - started
                rss = group_rss(process.pid)
                peak = max(peak, rss)
                if rss > budget["max_rss_bytes"]:
                    fail("Memory watchdog stopped the run: {:.2f} GiB RSS exceeded the {:.2f} GiB limit.".format(rss / GIB, budget["max_rss_bytes"] / GIB))
                if elapsed > args.timeout_seconds:
                    fail("Time watchdog stopped the run after {} seconds.".format(args.timeout_seconds))
                if shutil.disk_usage(log.parent).free < args.min_free_disk_gib * GIB:
                    fail("Disk watchdog stopped the run: less than {} GiB free.".format(args.min_free_disk_gib))
                if elapsed >= next_heartbeat:
                    say("Worker: {:.0f}s elapsed | RSS {:.2f} GiB | peak {:.2f} GiB".format(elapsed, rss / GIB, peak / GIB))
                    next_heartbeat = elapsed + 15
                try:
                    process.wait(timeout=0.5)
                except subprocess.TimeoutExpired:
                    pass
            if process.returncode:
                fail("Worker stopped (status {}). See {}. No candidate is approved.".format(process.returncode, log))
            status = "completed"
    except BaseException as exc:
        reason = str(exc) or type(exc).__name__
        if process is not None:
            stop_group(process)
        raise
    finally:
        save(log.with_name("resources.json"), {"status": status, "reason": reason,
             "peak_sampled_group_rss_bytes": peak, "elapsed_seconds": time.monotonic() - started,
             "budget": budget, "sampling_seconds": 0.5,
             "limitations": "RSS sampling can miss spikes and is not an OS-wide memory limit."})


def process_budget(args):
    total = physical_memory()
    if total < 8 * GIB:
        fail("This full-resolution experiment requires at least 8 GiB RAM even with tiling.")
    for name in ("max_rss_gib", "max_mps_gib", "min_free_disk_gib", "timeout_seconds"):
        value = getattr(args, name)
        if not math.isfinite(value) or value <= 0:
            fail(name.replace("_", "-") + " must be finite and positive.")
    return {"physical_memory_bytes": total,
            "max_rss_bytes": int(min(args.max_rss_gib * GIB, total / 3)),
            "max_mps_bytes": int(min(args.max_mps_gib * GIB, total / 4)),
            "note": "Requested budgets are further limited by installed RAM."}


def tiled_worker(config_path):
    """Run in the isolated backend interpreter; do not modify installed files."""
    config = read(Path(config_path))
    import logging
    import torch
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    device = config["device"]
    actual_mps_cap = None
    if device == "mps":
        if not torch.backends.mps.is_available():
            fail("MPS is unavailable.")
        recommended = torch.mps.recommended_max_memory()
        fraction = min(0.5, config["budget"]["max_mps_bytes"] / recommended)
        torch.mps.set_per_process_memory_fraction(fraction)
        actual_mps_cap = int(recommended * fraction)
        say("MPS allocator cap: {:.2f} GiB".format(actual_mps_cap / GIB))

    # This module is pinned to REVISION. Avoid the broad CLI/video imports.
    from remove_ai_watermarks import video_invisible as vi
    from remove_ai_watermarks import video_encoding as ve
    source = Path(config["reference"])
    expected = config["expected"]
    stream = video(probe(source))
    if stream.get("pix_fmt") != "yuv420p":
        fail("Worker supports only the validated 8-bit yuv420p reference.")
    def known(key):
        value = stream.get(key)
        return value if value and value != "unknown" else None
    profile = ve.VideoEncodeProfile(pixel_format="yuv420p", source_pixel_format="yuv420p",
        component_depth=8, color_range=known("color_range"), color_space=known("color_space"),
        color_transfer=known("color_transfer"), color_primaries=known("color_primaries"),
        time_base=stream.get("time_base"), start_pts=stream.get("start_pts"))

    # Upstream's profile probe imports PyAV alongside OpenCV. Our already
    # validated SDR profile uses ffprobe only, avoiding that known collision.
    vi.probe_video_encode_profile = lambda _source: profile
    original_encoder_command = vi.raw_video_command
    def bounded_encoder(*args, **kwargs):
        command = original_encoder_command(*args, **kwargs)
        # Remove frame lookahead and constrain encoder threading/memory.
        return command[:-1] + ["-threads:v", "2", "-filter_threads", "1",
                               "-tune", "zerolatency"] + command[-1:]
    vi.raw_video_command = bounded_encoder
    runtime = vi.load_video_vae_runtime(model=MODEL, device=device)
    vae = runtime.vae
    scale = 2 ** (len(vae.config.block_out_channels) - 1)
    if scale != 8:
        fail("Unexpected VAE spatial scale; refusing unvalidated tile geometry.")
    vae.enable_tiling()
    vae.tile_sample_min_size = config["tile_size"]
    vae.tile_latent_min_size = config["tile_size"] // scale
    vae.tile_overlap_factor = 0.25
    if not vae.use_tiling:
        fail("VAE tiling did not enable.")
    say("VAE tiles: {} pixels, 25% overlap. Batch size: 1.".format(config["tile_size"]))
    if "av" in sys.modules:
        fail("PyAV was unexpectedly loaded. Stop and inspect the runtime before retrying.")
    original_stream_batches = vi._stream_batches
    def progress_batches(frames, batch_size):
        for number, batch in enumerate(original_stream_batches(frames, batch_size), 1):
            say("Processing frame {}/{}".format(number, expected["frames"]))
            yield batch
            if device == "mps":
                torch.mps.synchronize()
                torch.mps.empty_cache()
    vi._stream_batches = progress_batches
    metrics = vi.regenerate_video_candidate(source, Path(config["candidate"]),
        noise_std=config["noise_std"], long_side=max(expected["width"], expected["height"]),
        fps=float(Fraction(expected["fps"])), batch_size=1, seed=0, model=MODEL,
        device=device, duration=expected["frames"] / float(Fraction(expected["fps"])), runtime=runtime)
    save(Path(config["candidate"]).with_name("backend-metrics.json"),
         {"metrics": asdict(metrics), "mps_allocator_limit_bytes": actual_mps_cap,
          "tile_size": config["tile_size"], "tile_overlap": 0.25,
          "profile": "experimental tiled VAE, ffprobe-only SDR profile, bounded encoder",
          "watermark_verification": "not tested"})


def say(message):
    print(message, flush=True)


def fail(message):
    raise RuntimeError(message)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(4 * 1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def save(path, value):
    with path.open("x", encoding="utf-8") as f:
        json.dump(value, f, indent=2)
        f.write("\n")


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def tool(name):
    found = shutil.which(name)
    if not found:
        fail("Required command not found: " + name)
    return found


def run(command, log=None, env=None):
    if log is None:
        p = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        if p.returncode or p.stderr.strip():
            fail("Command failed or reported an error: " + p.stderr[-6000:])
        return p.stdout
    with log.open("x", encoding="utf-8") as handle:
        handle.write("ARGV: " + json.dumps(list(map(str, command))) + "\n\n")
        handle.flush()
        p = subprocess.Popen(command, stdout=handle, stderr=subprocess.STDOUT, env=env)
        try:
            result = p.wait()
        except BaseException:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    p.kill()
                    p.wait()
            raise
        if result:
            fail("Command stopped (status {}). See {}. No output is approved for release.".format(result, log))


def probe(path, frames=False):
    command = [tool("ffprobe"), "-loglevel", "error", "-show_streams", "-show_format", "-output_format", "json"]
    if frames:
        command += ["-select_streams", "v:0", "-show_frames", "-show_entries",
                    "stream:format:frame=best_effort_timestamp"]
    return json.loads(run(command + [str(path)]))


def video(info):
    streams = [s for s in info["streams"] if s["codec_type"] == "video"]
    if len(streams) != 1:
        fail("This lab requires exactly one video stream.")
    return streams[0]


def validate_clip(path, expected):
    info = probe(path, frames=True)
    stream, rows = video(info), info.get("frames", [])
    if (stream["width"], stream["height"]) != (expected["width"], expected["height"]):
        fail("Candidate dimensions changed. Retained for inspection; not accepted.")
    if len(rows) != expected["frames"]:
        fail("Frame count changed: expected {}, got {}.".format(expected["frames"], len(rows)))
    tb, fps = Fraction(stream["time_base"]), Fraction(expected["fps"])
    tolerance = max(2 * tb, Fraction(1, 500))
    for index, row in enumerate(rows):
        if "best_effort_timestamp" not in row or abs(Fraction(row["best_effort_timestamp"]) * tb - Fraction(index, 1) / fps) > tolerance:
            fail("Frame timestamps changed or are missing; candidate is not accepted.")
    if abs(Fraction(stream["avg_frame_rate"]) - fps) > Fraction(1, 1000000):
        fail("Output frame rate changed.")
    return info


def model_environment(runtime, offline):
    env = dict(os.environ)
    env["HF_HUB_CACHE"] = str(runtime / "model-cache")
    env["HF_HUB_DISABLE_TELEMETRY"] = "1"
    env["DO_NOT_TRACK"] = "1"
    env["HF_HUB_OFFLINE"] = "1" if offline else "0"
    env["TRANSFORMERS_OFFLINE"] = "1" if offline else "0"
    return env


def setup(runtime):
    if not (3, 11) <= sys.version_info[:2] < (3, 15):
        fail("Setup requires Python 3.11 through 3.14. Run setup with a compatible Python.")
    if runtime.exists():
        fail("Runtime directory already exists: {}. Reuse it, or choose a new --runtime directory.".format(runtime))
    runtime.parent.mkdir(parents=True, exist_ok=True)
    say("Creating a dedicated runtime and downloading the pinned backend/model. No video is uploaded.")
    run([sys.executable, "-m", "venv", str(runtime)], runtime.parent / (runtime.name + "-venv.log"))
    python = runtime / "bin/python"
    requirement = "remove-ai-watermarks[video,diffusion] @ git+https://github.com/wiltodelta/remove-ai-watermarks.git@" + REVISION
    run([str(python), "-m", "pip", "install", requirement], runtime / "install.log")
    code = ("from huggingface_hub import snapshot_download; "
            "p=snapshot_download(repo_id=" + repr(MODEL) + ", "
            "allow_patterns=[\"config.json\",\"diffusion_pytorch_model.safetensors\"]); print(p)")
    run([str(python), "-c", code], runtime / "model-download.log", model_environment(runtime, False))
    cache = runtime / "model-cache/models--stabilityai--sd-vae-ft-mse"
    revision = (cache / "refs/main").read_text().strip()
    snapshot = cache / "snapshots" / revision
    files = {name: digest(snapshot / name) for name in ("config.json", "diffusion_pytorch_model.safetensors")}
    save(runtime / "runtime.json", {"backend_commit": REVISION, "model": MODEL,
                                   "model_revision": revision, "model_sha256": files})
    freeze = subprocess.run([str(python), "-m", "pip", "freeze"], capture_output=True, text=True, check=True)
    (runtime / "requirements-observed.txt").write_text(freeze.stdout, encoding="utf-8")
    say("Setup complete: " + str(runtime))


def prepare(args, work):
    source = Path(args.input).expanduser().resolve()
    if work.exists():
        fail("Choose a new --work-dir; this directory already exists.")
    if not 1 <= args.frames <= 240 or args.start_frame < 1:
        fail("Use 1-based --start-frame and 1-240 --frames for this short-clip lab.")
    src_info = probe(source)
    s = video(src_info)
    fps = Fraction(s["avg_frame_rate"])
    if fps < 1 or s.get("pix_fmt") != "yuv420p" or s.get("color_transfer") in ("smpte2084", "arib-std-b67"):
        fail("This lab accepts 8-bit SDR yuv420p video only; finish-video.py's MP4 matches this profile.")
    if s["width"] % 8 or s["height"] % 8:
        fail("VAE alignment would change these dimensions; both must be multiples of 8.")
    if s.get("tags", {}).get("rotate") or any(x.get("rotation", 0) for x in s.get("side_data_list", [])):
        fail("Rotated input is unsupported by this lab.")
    work.mkdir(parents=True, exist_ok=False)
    reference = work / "reference.mp4"
    first = args.start_frame - 1
    say("Preparing a silent, full-resolution reference excerpt; source stays untouched.")
    run([tool("ffmpeg"), "-hide_banner", "-nostdin", "-loglevel", "warning", "-xerror", "-n",
         "-i", str(source), "-map", "0:v:0", "-an", "-filter:v",
         "trim=start_frame={}:end_frame={},setpts=PTS-STARTPTS".format(first, first + args.frames),
         "-frames:v", str(args.frames), "-fps_mode", "passthrough", "-codec:v", "libx264", "-crf", "0", "-preset", "fast",
         "-map_metadata", "-1", str(reference)], work / "prepare.log")
    expected = {"width": s["width"], "height": s["height"], "fps": str(fps), "frames": args.frames}
    ref_info = validate_clip(reference, expected)
    save(work / "source.ffprobe.json", src_info)
    save(work / "reference.ffprobe.json", ref_info)
    save(work / "experiment.json", {"schema": "watermark-lab/1", "source": str(source),
         "source_sha256": digest(source), "start_frame_1based": args.start_frame,
         "reference_sha256": digest(reference), "expected": expected,
         "watermark_verification": "not tested", "audio": "omitted from this isolated visual experiment"})
    say("Prepared {} frames at {}x{}, {} fps in {}.".format(args.frames, s["width"], s["height"], fps, work))


def process(args, work, runtime):
    with workspace_lock(work) as lock_fd:
        process_locked(args, work, runtime, lock_fd)


def process_locked(args, work, runtime, lock_fd):
    budget = process_budget(args)
    plan = read(work / "experiment.json")
    state = read(runtime / "runtime.json")
    if plan.get("schema") != "watermark-lab/1" or state["backend_commit"] != REVISION:
        fail("Unsupported experiment/runtime identity.")
    reference = work / "reference.mp4"
    if digest(reference) != plan["reference_sha256"]:
        fail("Reference file changed.")
    if args.frames is not None and not 2 <= args.frames <= plan["expected"]["frames"]:
        fail("--frames must be between 2 and the prepared reference frame count.")
    if args.tile_size not in (128, 256, 512):
        fail("--tile-size must be 128, 256 or 512.")
    attempt = args.attempt or (datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:6])
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,79}", attempt):
        fail("--attempt must be a simple name: letters, digits, hyphens, underscores.")
    output = work / "attempts" / attempt
    if output.exists():
        fail("Attempt already exists. Choose a new --attempt name; prior files are retained.")
    if shutil.disk_usage(work).free < args.min_free_disk_gib * GIB:
        fail("Need at least {} GiB free in the workspace.".format(args.min_free_disk_gib))
    cache = runtime / "model-cache/models--stabilityai--sd-vae-ft-mse"
    if (cache / "refs/main").read_text().strip() != state["model_revision"]:
        fail("Cached model revision changed.")
    for name, expected in state["model_sha256"].items():
        if digest(cache / "snapshots" / state["model_revision"] / name) != expected:
            fail("Model file digest changed: " + name)
    python = runtime / "bin/python"
    code = ("import importlib.metadata as m,json,torch; "
            "d=m.distribution(\"remove-ai-watermarks\"); "
            "print(json.dumps({\"version\":d.version,\"source\":json.loads(d.read_text(\"direct_url.json\") or \"{}\"),"
            "\"mps\":bool(hasattr(torch.backends,\"mps\") and torch.backends.mps.is_available()),"
            "\"cuda\":torch.cuda.is_available(),\"torch\":torch.__version__,"
            "\"mps_recommended\":torch.mps.recommended_max_memory() if torch.backends.mps.is_available() else 0}))")
    env = model_environment(runtime, True)
    # Apply conservative allocator settings even during the lightweight host probe.
    env["PYTORCH_MPS_HIGH_WATERMARK_RATIO"] = "0.5"
    env["PYTORCH_MPS_LOW_WATERMARK_RATIO"] = "0.4"
    host = json.loads(run([str(python), "-c", code], env=env))
    if host["source"].get("vcs_info", {}).get("commit_id") != REVISION:
        fail("Installed backend does not match the pinned Git revision.")
    if args.device != "cpu" and not host.get(args.device):
        fail("Requested {} backend is unavailable on this host.".format(args.device))
    validate_clip(reference, plan["expected"])
    expected = dict(plan["expected"])
    expected["frames"] = args.frames or expected["frames"]
    output.mkdir(parents=True, exist_ok=False)
    candidate = output / "candidate.mp4"
    if args.device == "mps":
        fraction = min(0.5, budget["max_mps_bytes"] / host["mps_recommended"])
        env["PYTORCH_MPS_HIGH_WATERMARK_RATIO"] = str(fraction)
        env["PYTORCH_MPS_LOW_WATERMARK_RATIO"] = str(fraction * 0.8)
    env["PYTHONUNBUFFERED"] = "1"
    env["OMP_NUM_THREADS"] = "2"
    env["OPENBLAS_NUM_THREADS"] = "2"
    env["VECLIB_MAXIMUM_THREADS"] = "2"
    save(output / "runtime-observed.json", {"runtime": state, "host": host, "device": args.device,
         "wrapper_version": WRAPPER_VERSION, "wrapper_sha256": digest(Path(__file__).resolve()),
         "noise_std": args.noise_std, "seed": 0, "batch_size": 1,
         "tile_size": args.tile_size, "tile_overlap": 0.25, "budget": budget,
         "resolution_and_rate": "source preserved; differs from upstream calibration defaults"})
    config = {"reference": str(reference), "candidate": str(candidate), "expected": expected,
              "device": args.device, "noise_std": args.noise_std, "tile_size": args.tile_size, "budget": budget}
    save(output / "worker.json", config)
    say("Running {} frames at {}x{} on {}, with {}px VAE tiles.".format(expected["frames"], expected["width"], expected["height"], args.device, args.tile_size))
    say("RSS stop limit: {:.2f} GiB; MPS cap at most {:.2f} GiB. Limits are not additive.".format(budget["max_rss_bytes"] / GIB, budget["max_mps_bytes"] / GIB))
    say("Attempt: " + str(output))
    say("Progress log: " + str(output / "process.log"))
    command = [str(python), "-u", str(Path(__file__).resolve()), "_worker", str(output / "worker.json")]
    guarded_run(command, output / "process.log", env, lock_fd, args, budget)
    checked = validate_clip(candidate, expected)
    save(output / "candidate.ffprobe.json", checked)
    filters = "[0:v]scale=540:-2,setsar=1[l];[1:v]scale=540:-2,setsar=1[r];[l][r]hstack=inputs=2:shortest=1[v]"
    run([tool("ffmpeg"), "-hide_banner", "-nostdin", "-loglevel", "warning", "-xerror", "-n",
         "-threads", "2", "-i", str(reference), "-threads", "2", "-i", str(candidate),
         "-filter_complex_threads", "1", "-filter_complex", filters, "-map", "[v]", "-frames:v", str(expected["frames"]),
         "-an", "-codec:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart",
         "-threads:v", "2", str(output / "reference-left_candidate-right.mp4")], output / "comparison.log")
    save(output / "report.json", {"render_status": "structurally validated", "expected": expected,
        "wrapper_version": WRAPPER_VERSION, "reference": str(reference), "candidate": str(candidate),
        "configuration": "experimental tiled VAE and bounded encoder; not an upstream calibrated profile",
        "reference_sha256": digest(reference), "candidate_sha256": digest(candidate),
        "backend_commit": REVISION, "quality_review": "pending human review at full resolution and in motion",
        "watermark_verification": "not tested; no local per-file SynthID detector was run",
        "audio_watermark_verification": "not tested; visual experiment is silent",
        "release_approved": False})
    say("Candidate passed frame-count, dimension, rate, timestamp and decode checks.")
    say("Created: " + str(candidate))
    say("Comparison: " + str(output / "reference-left_candidate-right.mp4"))
    say("Watermark effectiveness remains UNVERIFIED. This is not an approved replacement for your master.")


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "_worker":
        tiled_worker(sys.argv[2])
        return
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="action", required=True)
    prep = sub.add_parser("prepare", help="Create a 2-second silent reference without installing or running AI")
    prep.add_argument("input")
    prep.add_argument("--start-frame", type=int, default=1, help="1-based first source frame")
    prep.add_argument("--frames", type=int, default=60, help="1-240 frames (default: 60)")
    prep.add_argument("--work-dir", default="watermark-test")
    install = sub.add_parser("setup", help="Download a pinned backend and model into a new isolated runtime")
    install.add_argument("--runtime", default=str(RUNTIME))
    proc = sub.add_parser("process", help="Run a prepared short experiment using the offline model cache")
    proc.add_argument("--work-dir", default="watermark-test")
    proc.add_argument("--runtime", default=str(RUNTIME))
    proc.add_argument("--device", choices=("mps", "cuda", "cpu"), default="mps")
    proc.add_argument("--noise-std", type=float, default=0.15)
    proc.add_argument("--attempt", help="New folder name inside WORK/attempts; default is a unique timestamp")
    proc.add_argument("--frames", type=int, help="Process only the first N prepared frames; start with 2 as a memory smoke test")
    proc.add_argument("--tile-size", type=int, choices=(128, 256, 512), default=256)
    proc.add_argument("--max-mps-gib", type=float, default=4, help="MPS allocator ceiling, also limited to RAM/4 and half Metal's recommended maximum")
    proc.add_argument("--max-rss-gib", type=float, default=6, help="Worker + child process RSS stop threshold, also limited to RAM/3")
    proc.add_argument("--min-free-disk-gib", type=float, default=2)
    proc.add_argument("--timeout-seconds", type=float, default=1800)
    args = p.parse_args()
    if args.action == "setup":
        setup(Path(args.runtime).expanduser().resolve())
    elif args.action == "prepare":
        prepare(args, Path(args.work_dir).expanduser().resolve())
    else:
        if not 0 <= args.noise_std <= 1:
            p.error("--noise-std must be between 0 and 1")
        process(args, Path(args.work_dir).expanduser().resolve(), Path(args.runtime).expanduser().resolve())


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped. Source media and experiment artifacts are retained.", file=sys.stderr)
        sys.exit(130)
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.CalledProcessError) as e:
        print("\nSTOP: " + str(e), file=sys.stderr)
        sys.exit(1)
