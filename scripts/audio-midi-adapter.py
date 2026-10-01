#!/usr/bin/env python3
"""Bounded Basic Pitch observations; Rust owns evidence and MIDI publication.

This adapter admits one explicit upstream ONNX file and an already installed,
fingerprinted CPU environment. It never installs packages, downloads models,
writes MIDI, or infers authorship, instrument identity, or calibrated confidence.
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
import sysconfig
import tempfile
import wave


VERSIONS = {
    "basic-pitch": "0.4.0", "onnxruntime": "1.20.1", "numpy": "1.26.4",
    "librosa": "0.10.2.post1", "scipy": "1.13.1", "pretty_midi": "0.2.10",
}
MODULES = {
    "basic_pitch": "basic-pitch", "onnxruntime": "onnxruntime", "numpy": "numpy",
    "librosa": "librosa", "scipy": "scipy", "pretty_midi": "pretty_midi",
}
BASIC_PITCH_REVISION = "9991303bba609a3b93089d13ec80d1d495083596"
MODEL_GIT_BLOB = "c30e5f9438e798604b7177aa26be1fe64482f767"
MODEL_BYTES = 230_444
SAMPLE_RATE = 22_050
MAXIMUM_FRAMES = SAMPLE_RATE * 120
MAXIMUM_INPUT_BYTES = 8 * 1024 * 1024
MAXIMUM_NOTES = 8192
MAXIMUM_RUNTIME_FILES = 50_000
MAXIMUM_RUNTIME_BYTES = 2 * 1024 * 1024 * 1024


class Refusal(Exception):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def refuse(code: str, message: str) -> None:
    raise Refusal(code, message)


def configure_environment() -> tempfile.TemporaryDirectory:
    if not sys.flags.isolated or not sys.dont_write_bytecode:
        refuse("isolation_required", "invoke the pinned interpreter with -I -B")
    for name in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
                 "NUMEXPR_NUM_THREADS", "VECLIB_MAXIMUM_THREADS", "BLIS_NUM_THREADS"):
        os.environ[name] = "1"
    os.environ["NUMBA_DISABLE_JIT"] = "1"
    # -B stops cache writes, not reads. Point imports at a fresh private cache.
    temporary = tempfile.TemporaryDirectory(prefix="aniflow-midi-cache-")
    sys.pycache_prefix = temporary.name
    os.environ["NUMBA_CACHE_DIR"] = temporary.name
    return temporary


def generated_cache(path: Path) -> bool:
    return "__pycache__" in path.parts or path.suffix in {".pyc", ".pyo"}


def safe_runtime_file(path: Path, root: Path) -> Path:
    path = Path(os.path.abspath(path))
    if not path.is_relative_to(root):
        refuse("runtime_path_escape", "installed file escapes the interpreter prefix")
    current = root
    for component in path.relative_to(root).parts:
        current /= component
        if current.is_symlink():
            refuse("runtime_path_escape", "runtime paths must not contain symlinks")
    if not stat.S_ISREG(path.lstat().st_mode):
        refuse("runtime_nonregular_file", "runtime entries must be regular files")
    return path


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    count = 0
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            count += len(block)
            if count > MAXIMUM_RUNTIME_BYTES:
                refuse("runtime_bound_exceeded", "runtime file exceeds its byte bound")
            digest.update(block)
    return digest.hexdigest(), count


def installed_distributions() -> dict:
    if sys.version_info[:2] not in {(3, 10), (3, 11)}:
        refuse("unsupported_python", "this profile admits Python 3.10 or 3.11")
    required = {}
    for name, version in VERSIONS.items():
        distribution = importlib.metadata.distribution(name)
        if distribution.version != version:
            refuse("unsupported_runtime", f"this profile requires {name}=={version}")
        required[name] = distribution
    # The upstream Model constructor tries alternative runtimes first. Admit
    # only the explicitly selected ONNX environment, before importing it.
    for module in ("tensorflow", "coremltools", "tflite_runtime"):
        if importlib.util.find_spec(module) is not None:
            refuse("unsupported_runtime", "this profile requires a sole-ONNX environment")
    root = Path(sys.prefix).resolve(strict=True)
    for module, name in MODULES.items():
        distribution = required[name]
        recorded = {Path(os.path.abspath(distribution.locate_file(entry)))
                    for entry in distribution.files or [] if not generated_cache(Path(entry))}
        spec = importlib.util.find_spec(module)
        if spec is None or spec.origin is None:
            refuse("runtime_import_shadow", "required import is unavailable")
        if safe_runtime_file(Path(spec.origin), root) not in recorded:
            refuse("runtime_import_shadow", "import resolves outside its recorded distribution")
    return required


def runtime_manifest() -> list[dict]:
    """Bind installed third-party files, including transitive packages and additions.

    This deliberately does not claim to inventory Python's standard library,
    operating-system libraries, or every environmental influence.
    """
    root = Path(sys.prefix).resolve(strict=True)
    files = set()

    def include(path: Path) -> None:
        if generated_cache(path):
            return
        files.add(safe_runtime_file(path, root))
        if len(files) > MAXIMUM_RUNTIME_FILES:
            refuse("runtime_bound_exceeded", "installed runtime exceeds its file bound")

    for distribution in importlib.metadata.distributions():
        if not distribution.files:
            refuse("runtime_inventory_missing", "every installed distribution needs a RECORD inventory")
        for entry in distribution.files:
            if not generated_cache(Path(entry)):
                include(Path(distribution.locate_file(entry)))
    # RECORD alone misses newly added modules that could shadow imports.
    roots = {Path(sysconfig.get_path(name)) for name in ("purelib", "platlib")}
    for package_root in roots:
        if package_root.is_symlink() or not package_root.is_relative_to(root):
            refuse("runtime_path_escape", "package root escapes the interpreter prefix")
        walked = 0
        for directory, subdirectories, names in os.walk(package_root, followlinks=False):
            subdirectories[:] = [name for name in subdirectories if name != "__pycache__"]
            walked += len(subdirectories) + len(names)
            if walked > MAXIMUM_RUNTIME_FILES * 2:
                refuse("runtime_bound_exceeded", "installed runtime exceeds its entry bound")
            if any((Path(directory) / name).is_symlink() for name in subdirectories):
                refuse("runtime_path_escape", "package directories must not be symlinks")
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
    files = runtime_manifest()
    import onnxruntime
    onnxruntime.disable_telemetry_events()
    if onnxruntime.__version__ != VERSIONS["onnxruntime"]:
        refuse("unsupported_runtime", "imported ONNX Runtime differs from its metadata")
    if "CPUExecutionProvider" not in onnxruntime.get_available_providers():
        refuse("cpu_runtime_unavailable", "ONNX CPU execution provider is unavailable")
    # Import the actual inference/postprocessing modules, including their
    # transitive dependencies. No model/session/audio inference is created.
    from basic_pitch.inference import Model
    from basic_pitch.note_creation import model_output_to_notes
    if not hasattr(Model.MODEL_TYPES, "ONNX") or not callable(model_output_to_notes):
        refuse("unsupported_runtime", "required Basic Pitch API is unavailable")
    encoded = json.dumps(files, sort_keys=True, ensure_ascii=False,
                         separators=(",", ":")).encode("utf-8")
    metadata = {name: distributions[name].read_text("METADATA")
                for name in ("basic-pitch", "onnxruntime")}
    if not all(metadata.values()):
        refuse("license_evidence_missing", "required installed package metadata is absent")
    # These are upstream declarations for the pinned profiles, not a license
    # audit of transitive packages or verification of artifact authenticity.
    return {
        "schema": "aniflow.audio-midi-probe/v1",
        "python_version": platform.python_version(),
        "basic_pitch_version": VERSIONS["basic-pitch"],
        "basic_pitch_revision": BASIC_PITCH_REVISION,
        "onnxruntime_version": VERSIONS["onnxruntime"],
        "numpy_version": VERSIONS["numpy"], "librosa_version": VERSIONS["librosa"],
        "scipy_version": VERSIONS["scipy"], "pretty_midi_version": VERSIONS["pretty_midi"],
        "runtime_sha256": hashlib.sha256(b"aniflow.basic-pitch-runtime/v1\0" + encoded).hexdigest(),
        "runtime_file_count": len(files),
        "runtime_byte_count": sum(item["byte_count"] for item in files),
        "license": {
            "basic_pitch_expression": "Apache-2.0",
            "basic_pitch_metadata_sha256": hashlib.sha256(metadata["basic-pitch"].encode()).hexdigest(),
            "onnxruntime_expression": "MIT",
            "onnxruntime_metadata_sha256": hashlib.sha256(metadata["onnxruntime"].encode()).hexdigest(),
        },
    }


def regular_input(path: Path) -> None:
    if not path.is_absolute() or path.resolve(strict=True) != path or not path.is_file():
        refuse("invalid_input", "input must be an absolute regular non-symlink snapshot")


def inspect_wave(path: Path) -> int:
    regular_input(path)
    size = path.stat().st_size
    if not 44 <= size <= MAXIMUM_INPUT_BYTES:
        refuse("invalid_input", "input exceeds the bounded PCM WAV profile")
    with path.open("rb") as source:
        header = source.read(12)
    if header[:4] != b"RIFF" or header[8:] != b"WAVE" or struct.unpack("<I", header[4:8])[0] + 8 != size:
        refuse("invalid_input", "input must have a complete RIFF/WAVE envelope")
    with wave.open(str(path), "rb") as source:
        frames = source.getnframes()
        if (source.getsampwidth(), source.getcomptype(), source.getnchannels()) != (2, "NONE", 1):
            refuse("unsupported_pcm", "this profile requires mono PCM16")
        if source.getframerate() != SAMPLE_RATE:
            refuse("unsupported_rate", "this profile requires 22050 Hz without resampling")
        if not 1 <= frames <= MAXIMUM_FRAMES:
            refuse("duration_bound_exceeded", "this profile admits at most 120 seconds")
        payload = source.readframes(frames)
        if len(payload) != frames * 2:
            refuse("invalid_input", "PCM payload is truncated")
    if not any(payload):
        refuse("silent_input", "PCM input is exactly silent")
    return frames


def verify_model(path: Path) -> None:
    regular_input(path)
    if path.stat().st_size != MODEL_BYTES:
        refuse("unsupported_model", "this profile admits only the pinned upstream nmp.onnx file")
    # Git object identity can be checked without downloading or importing it.
    # This is source-file identity, not a claim about ONNX external-data parsing.
    digest = hashlib.sha1(f"blob {MODEL_BYTES}\0".encode())
    with path.open("rb") as source:
        digest.update(source.read(MODEL_BYTES + 1))
    if digest.hexdigest() != MODEL_GIT_BLOB:
        refuse("unsupported_model", "model differs from the pinned upstream Git blob")


def finite(value, label: str) -> float:
    if isinstance(value, bool):
        refuse("invalid_analyzer_output", f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number):
        refuse("invalid_analyzer_output", f"{label} must be finite")
    return number


def microseconds(value: float) -> int:
    numerator, denominator = value.as_integer_ratio()
    return (2 * numerator * 1_000_000 + denominator) // (2 * denominator)


def normalize_events(events, frames: int) -> list[dict]:
    if len(events) > MAXIMUM_NOTES:
        refuse("invalid_analyzer_output", "note count exceeds its bound")
    notes = []
    duration = frames / SAMPLE_RATE
    duration_microseconds = frames * 1_000_000 // SAMPLE_RATE
    for event in events:
        if len(event) != 5:
            refuse("invalid_analyzer_output", "native note event must have five fields")
        start, end, pitch, amplitude, bends = event
        start, end = finite(start, "start"), finite(end, "end")
        amplitude, numeric_pitch = finite(amplitude, "activation"), finite(pitch, "pitch")
        if not 0 <= start < end <= duration:
            refuse("invalid_analyzer_output", "native note timing escapes source bounds")
        if not 21 <= numeric_pitch <= 108 or not numeric_pitch.is_integer():
            refuse("invalid_analyzer_output", "pitch must be an integer in the 88-key profile")
        if not 0 <= amplitude <= 1:
            refuse("invalid_analyzer_output", "activation must be in the unit interval")
        if bends is not None:
            refuse("unsupported_pitch_bends", "this profile disables pitch-bend inference")
        velocity = round(127 * amplitude)  # Native NumPy round uses ties-to-even.
        if not 1 <= velocity <= 127:
            refuse("invalid_analyzer_output", "native velocity must be in 1..127")
        start_us = microseconds(start)
        end_us = microseconds(end)
        if not 0 <= start_us < end_us <= duration_microseconds:
            refuse("invalid_analyzer_output", "microsecond timing collapses or escapes source bounds")
        notes.append({"start_microseconds": start_us, "end_microseconds": end_us,
                      "pitch": int(numeric_pitch), "velocity": velocity, "activation": amplitude})
    notes.sort(key=lambda note: (note["start_microseconds"], note["pitch"], note["end_microseconds"]))
    previous_end = {}
    for note in notes:
        pitch = note["pitch"]
        if note["start_microseconds"] < previous_end.get(pitch, 0):
            refuse("invalid_analyzer_output", "same-pitch notes overlap")
        previous_end[pitch] = note["end_microseconds"]
    return notes


def validate_outputs(outputs, frames: int) -> None:
    expected_frames = frames * 86 // SAMPLE_RATE
    if not isinstance(outputs, dict) or set(outputs) != {"note", "onset", "contour"}:
        refuse("invalid_analyzer_output", "native output must contain only note, onset and contour arrays")
    for name, columns in (("note", 88), ("onset", 88), ("contour", 264)):
        values = outputs[name]
        if getattr(values, "shape", None) != (expected_frames, columns):
            refuse("invalid_analyzer_output", "native activation array has an unexpected shape")
        for value in values.flat:
            if not 0 <= finite(value, "model activation") <= 1:
                refuse("invalid_analyzer_output", "native activations must be in the unit interval")


def analyze(path: Path, model_path: Path) -> dict:
    frames = inspect_wave(path)
    verify_model(model_path)
    installed_distributions()
    import onnxruntime
    onnxruntime.disable_telemetry_events()
    from basic_pitch.inference import Model, run_inference
    from basic_pitch.note_creation import model_output_to_notes
    model = Model(model_path)
    if model.model_type != Model.MODEL_TYPES.ONNX or model.model.get_providers() != ["CPUExecutionProvider"]:
        refuse("unsupported_runtime", "this profile requires ONNX CPU-only execution")
    outputs = run_inference(path, model)
    validate_outputs(outputs, frames)
    _, events = model_output_to_notes(
        outputs, onset_thresh=0.5, frame_thresh=0.3, infer_onsets=True,
        min_note_len=11, min_freq=None, max_freq=None, include_pitch_bends=False,
        multiple_pitch_bends=False, melodia_trick=True, midi_tempo=120,
    )
    return {
        "schema": "aniflow.audio-midi-observation/v1", "sample_rate_hz": SAMPLE_RATE,
        "channels": 1, "sample_frames": frames,
        "duration_microseconds": frames * 1_000_000 // SAMPLE_RATE,
        "notes": normalize_events(events, frames),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--probe", action="store_true")
    action.add_argument("--input", type=Path)
    parser.add_argument("--model", type=Path)
    arguments = parser.parse_args()
    if arguments.probe == (arguments.model is not None):
        parser.error("--model is required with --input and forbidden with --probe")
    try:
        with configure_environment():
            result = probe() if arguments.probe else analyze(arguments.input, arguments.model)
        print(json.dumps(result, sort_keys=True, ensure_ascii=False, allow_nan=False,
                         separators=(",", ":")))
        return 0
    except Refusal as error:
        print(json.dumps({"code": error.code, "message": str(error)}, sort_keys=True), file=sys.stderr)
        return 65
    except (OSError, ValueError, RuntimeError, ImportError, TypeError, wave.Error,
            importlib.metadata.PackageNotFoundError) as error:
        print(json.dumps({"code": "adapter_failure", "message": type(error).__name__}), file=sys.stderr)
        return 74


if __name__ == "__main__":
    raise SystemExit(main())
