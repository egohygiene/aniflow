#!/usr/bin/env python3
"""Bounded raw Essentia observations for aniflow's native musical provider.

This subprocess is an algorithm adapter, not a run engine. Rust owns source
selection, authorization, timeouts, checkpointing, normalized availability,
and report publication. No classifier models, downloads, resampling, replay
gain, or source writes occur here. Native scores are not probabilities.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import stat
import struct
import sys
import tempfile
import wave


VERSIONS = {
    "essentia": "2.1b6.dev1389", "numpy": "2.3.5",
    "PyYAML": "6.0.3", "six": "1.17.0",
}
SAMPLE_RATE = 44_100
MAXIMUM_BYTES = 256 * 1024 * 1024
MAXIMUM_RUNTIME_BYTES = 1024 * 1024 * 1024
MAXIMUM_RUNTIME_FILES = 25_000
MAXIMUM_OBSERVATIONS = 8192
PACKAGE_TREES = {
    "essentia": ("essentia", "essentia.libs"),
    "numpy": ("numpy", "numpy.libs"),
    "PyYAML": ("yaml", "_yaml"),
    "six": (),
}


class Refusal(Exception):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def refuse(code: str, message: str) -> None:
    raise Refusal(code, message)


def configure_environment() -> tempfile.TemporaryDirectory:
    if not sys.flags.isolated or not sys.dont_write_bytecode:
        refuse("isolation_required", "invoke the pinned interpreter with -I -B")
    # Set these before importing NumPy or Essentia's native libraries.
    for name in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
                 "NUMEXPR_NUM_THREADS", "VECLIB_MAXIMUM_THREADS", "BLIS_NUM_THREADS"):
        os.environ[name] = "1"
    # -B prevents writes, not reads. A fresh private prefix ensures none of the
    # excluded, potentially stale package bytecode can be consumed. This is set
    # before any pinned third-party package import and removed on normal exit.
    bytecode_directory = tempfile.TemporaryDirectory(prefix="aniflow-musical-bytecode-")
    sys.pycache_prefix = bytecode_directory.name
    return bytecode_directory


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    count = 0
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            count += len(block)
            if count > MAXIMUM_RUNTIME_BYTES:
                refuse("runtime_bound_exceeded", "installed runtime file exceeds its byte bound")
            digest.update(block)
    return digest.hexdigest(), count


def installed_distributions():
    distributions = {}
    for name, required_version in VERSIONS.items():
        distribution = importlib.metadata.distribution(name)
        if distribution.version != required_version:
            refuse("unsupported_runtime", f"this profile requires {name}=={required_version}")
        distributions[name] = distribution
    # A new site-packages/six/ package can shadow the recorded six.py without
    # residing beneath that module's tree. Require each top-level import to
    # resolve to an actual recorded, non-cache file of its own distribution.
    root = Path(sys.prefix).resolve(strict=True)
    for module, distribution_name in (("essentia", "essentia"), ("numpy", "numpy"),
                                      ("yaml", "PyYAML"), ("_yaml", "PyYAML"), ("six", "six")):
        distribution = distributions[distribution_name]
        recorded = {Path(os.path.abspath(distribution.locate_file(entry)))
                    for entry in distribution.files or [] if not generated_cache(Path(entry))}
        spec = importlib.util.find_spec(module)
        if spec is None or spec.origin is None:
            refuse("runtime_import_shadow", "required package import is unavailable")
        origin = safe_runtime_file(Path(spec.origin), root)
        if origin not in recorded:
            refuse("runtime_import_shadow", "package import resolves outside its recorded distribution")
    return distributions


def safe_runtime_file(path: Path, root: Path) -> Path:
    path = Path(os.path.abspath(path))
    if not path.is_relative_to(root):
        refuse("runtime_path_escape", "installed distribution path escapes the interpreter prefix")
    relative = path.relative_to(root)
    current = root
    for component in relative.parts:
        current /= component
        if current.is_symlink():
            refuse("runtime_path_escape", "installed distribution paths must not contain symlinks")
    if not stat.S_ISREG(path.lstat().st_mode):
        refuse("runtime_nonregular_file", "installed distribution entries must be regular files")
    return path


def generated_cache(path: Path) -> bool:
    return "__pycache__" in path.parts or path.suffix in {".pyc", ".pyo"}


def runtime_manifest(distributions) -> list[dict]:
    root = Path(sys.prefix).resolve(strict=True)
    files = set()

    def include(path: Path) -> None:
        if generated_cache(path):
            return
        files.add(safe_runtime_file(path, root))
        if len(files) > MAXIMUM_RUNTIME_FILES:
            refuse("runtime_bound_exceeded", "installed runtime exceeds its file bound")

    for distribution in distributions.values():
        if not distribution.files:
            refuse("runtime_inventory_missing", "installed distribution has no RECORD inventory")
        for entry in distribution.files:
            if generated_cache(Path(entry)):
                continue
            include(Path(distribution.locate_file(entry)))
        # Inventory package trees as well as wheel RECORD entries. Otherwise an
        # added importable source/native file could influence execution without
        # changing the runtime pin. Generated caches alone are excluded.
        site_packages = Path(distribution.locate_file(""))
        for tree_name in PACKAGE_TREES[distribution.metadata["Name"]]:
            directory_root = site_packages / tree_name
            if not directory_root.exists() and not directory_root.is_symlink():
                continue
            if directory_root.is_symlink():
                refuse("runtime_path_escape", "installed package root must not be a symlink")
            walked_entries = 0
            for directory, subdirectories, names in os.walk(directory_root, followlinks=False):
                subdirectories[:] = [name for name in subdirectories if name != "__pycache__"]
                walked_entries += len(subdirectories) + len(names)
                if walked_entries > MAXIMUM_RUNTIME_FILES:
                    refuse("runtime_bound_exceeded", "installed package tree exceeds its entry bound")
                if any((Path(directory) / name).is_symlink() for name in subdirectories):
                    refuse("runtime_path_escape", "installed package directories must not be symlinks")
                for name in names:
                    include(Path(directory) / name)
    manifest = []
    total = 0
    for path in sorted(files):
        digest, count = sha256_file(path)
        total += count
        if total > MAXIMUM_RUNTIME_BYTES:
            refuse("runtime_bound_exceeded", "installed runtime exceeds its total byte bound")
        manifest.append({"path": path.relative_to(root).as_posix(),
                         "sha256": digest, "byte_count": count})
    return manifest


def probe() -> dict:
    distributions = installed_distributions()
    files = runtime_manifest(distributions)
    import numpy
    import essentia
    import essentia.standard as standard
    if numpy.__version__ != VERSIONS["numpy"] or essentia.__version__ != "2.1-beta6-dev" or essentia.__version_git_sha__ != "v2.1_beta5-1389-g36ec3d92":
        refuse("unsupported_runtime", "imported native runtime differs from the declared profile")
    # Exercise native extension loading and algorithm registration, without any
    # input audio, classifier models, or inference work during preflight.
    standard.RhythmExtractor2013(method="multifeature", minTempo=40, maxTempo=208)
    for profile in ("krumhansl", "temperley"):
        standard.KeyExtractor(profileType=profile, sampleRate=SAMPLE_RATE)
    encoded = json.dumps(files, sort_keys=True, ensure_ascii=False,
                         separators=(",", ":")).encode("utf-8")
    essentia_metadata = distributions["essentia"].read_text("METADATA")
    numpy_metadata = distributions["numpy"].read_text("METADATA")
    expression = distributions["essentia"].metadata.get("License-Expression")
    if expression != "AGPL-3.0-only" or not essentia_metadata or not numpy_metadata:
        refuse("license_evidence_missing", "required installed license metadata is unavailable")
    return {
        "schema": "aniflow.audio-musical-probe/v1",
        "python_version": platform.python_version(),
        "essentia_version": distributions["essentia"].version,
        "essentia_runtime_version": str(essentia.__version__),
        "essentia_git_sha": str(essentia.__version_git_sha__),
        "numpy_version": distributions["numpy"].version,
        "pyyaml_version": distributions["PyYAML"].version,
        "six_version": distributions["six"].version,
        "runtime_sha256": hashlib.sha256(b"aniflow.essentia-runtime/v1\0" + encoded).hexdigest(),
        "runtime_file_count": len(files),
        "runtime_byte_count": sum(item["byte_count"] for item in files),
        "license": {
            "essentia_expression": expression,
            "essentia_metadata_sha256": hashlib.sha256(essentia_metadata.encode("utf-8")).hexdigest(),
            "numpy_metadata_sha256": hashlib.sha256(numpy_metadata.encode("utf-8")).hexdigest(),
        },
    }


def finite(value, label: str) -> float:
    number = float(value)
    if not math.isfinite(number):
        refuse("invalid_analyzer_output", f"{label} must be finite")
    return number


def finite_vector(values, label: str) -> list[float]:
    if len(values) > MAXIMUM_OBSERVATIONS:
        refuse("invalid_analyzer_output", f"{label} exceeds its observation bound")
    return [finite(value, label) for value in values]


def analyze(path: Path) -> dict:
    installed_distributions()
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        refuse("invalid_input", "input must be an absolute regular non-symlink PCM WAV snapshot")
    size = path.stat().st_size
    if not 44 <= size <= MAXIMUM_BYTES:
        refuse("invalid_input", "input exceeds the bounded PCM WAV profile")
    with path.open("rb") as source:
        header = source.read(12)
    if header[:4] != b"RIFF" or header[8:] != b"WAVE" or struct.unpack("<I", header[4:8])[0] + 8 != size:
        refuse("invalid_input", "input must have a complete RIFF/WAVE envelope")
    with wave.open(str(path), "rb") as source:
        channels = source.getnchannels()
        sample_rate = source.getframerate()
        frames = source.getnframes()
        if source.getsampwidth() != 2 or source.getcomptype() != "NONE" or channels not in (1, 2):
            refuse("unsupported_pcm", "only PCM16 mono/stereo snapshots are supported")
        if sample_rate != SAMPLE_RATE:
            refuse("unsupported_rate", "this musical profile requires 44100 Hz without resampling")
        if frames < SAMPLE_RATE * 8:
            refuse("insufficient_duration", "at least eight seconds are required")
        if frames > SAMPLE_RATE * 600:
            refuse("duration_bound_exceeded", "at most 600 seconds are supported")
        payload = source.readframes(frames)
        if len(payload) != frames * channels * 2:
            refuse("invalid_input", "PCM payload is truncated")
    import numpy as np
    # Convert signed PCM before averaging; otherwise integer addition overflows.
    # Keep half-integer stereo averages exact before the final float32 cast.
    integers = np.frombuffer(payload, dtype="<i2").reshape((frames, channels)).astype(np.int32)
    sums = integers.sum(axis=1, dtype=np.int32)
    if not np.any(sums):
        refuse("silent_input", "arithmetic-average mono signal is exactly silent")
    signal = np.ascontiguousarray(sums.astype(np.float32) / (channels * 32768), dtype=np.float32)
    del integers, sums, payload
    import essentia.standard as standard
    bpm, ticks, confidence, estimates, intervals = standard.RhythmExtractor2013(
        method="multifeature", minTempo=40, maxTempo=208,
    )(signal)
    ticks = finite_vector(ticks, "beat ticks")
    duration = frames / SAMPLE_RATE
    if any(tick < 0 or tick > duration for tick in ticks) or any(
        right <= left for left, right in zip(ticks, ticks[1:])
    ):
        refuse("invalid_analyzer_output", "beat times must be ordered and inside the input")
    profiles = []
    for profile in ("krumhansl", "temperley"):
        key, scale, strength = standard.KeyExtractor(
            profileType=profile, sampleRate=SAMPLE_RATE,
        )(signal)
        strength = finite(strength, "key strength")
        if not (key == scale == "" and strength == 0) and (
            key not in {"A", "A#", "Bb", "B", "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb", "G", "G#", "Ab"}
            or scale not in {"major", "minor"}
        ):
            refuse("invalid_analyzer_output", "key extractor returned an unsupported key or mode")
        profiles.append({"profile": profile, "key": key, "scale": scale,
                         "raw_strength": strength})
    return {
        "schema": "aniflow.audio-musical-observation/v1",
        "sample_rate_hz": SAMPLE_RATE, "channels": channels,
        "sample_frames": frames, "duration_seconds": duration,
        "downmix": "arithmetic_average",
        "bpm": {"value": finite(bpm, "BPM"), "ticks_seconds": ticks,
                "raw_confidence": finite(confidence, "beat confidence metric"),
                "estimates": finite_vector(estimates, "BPM estimates"),
                "bpm_intervals": finite_vector(intervals, "BPM intervals")},
        "key_profiles": profiles,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--probe", action="store_true")
    action.add_argument("--input", type=Path)
    arguments = parser.parse_args()
    try:
        with configure_environment():
            result = probe() if arguments.probe else analyze(arguments.input)
        print(json.dumps(result, sort_keys=True, ensure_ascii=False, allow_nan=False,
                         separators=(",", ":")))
        return 0
    except Refusal as error:
        print(json.dumps({"code": error.code, "message": str(error)}, sort_keys=True), file=sys.stderr)
        return 65
    except (OSError, ValueError, RuntimeError, ImportError, wave.Error, importlib.metadata.PackageNotFoundError) as error:
        print(json.dumps({"code": "adapter_failure", "message": type(error).__name__}), file=sys.stderr)
        return 74


if __name__ == "__main__":
    raise SystemExit(main())
