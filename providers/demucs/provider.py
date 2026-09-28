#!/usr/bin/env python3
"""Typed, source-preserving Demucs 4.0.1 adapter for the Pipeline v3 ABI.

The owning aniflow runtime bounds process time, descendants, diagnostic capture,
and accepted artifact sizes. This adapter validates the narrow PCM16 WAV profile
independently, pins local dependencies, and publishes evidence only after both
stems pass. Offline flags and local model lookup are not a network sandbox.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import time
from typing import Any


PROVIDER_ID = "org.egohygiene.aniflow.demucs"
PROVIDER_VERSION = "1.0.0"
CAPABILITY_ID = "aniflow/audio.separate"
CONFIGURATION_SCHEMA_ID = "aniflow.demucs.configuration/v1"
CONFIGURATION_SCHEMA_VERSION = "1.0.0"
CONFIGURATION_SCHEMA_SHA256 = "5a6934037afc3cf3f49f1a9d99b1eecb27c6cf69e723c52e79eec396091fbb13"
MODEL_NAME = "htdemucs_6s"
CHECKPOINT_FILENAME = "5c90dfd2-34c22ccb.th"
MAXIMUM_WAV_BYTES = 256 * 1024 * 1024
MAXIMUM_DURATION_SECONDS = 600
DURATION_TOLERANCE_MILLISECONDS = 20
SHA256 = re.compile(r"^[a-f0-9]{64}$")
LOCAL_ID = re.compile(r"^[a-z][a-z0-9_-]*$")
VALUE_KEYS = {
    "python_executable", "python_sha256", "ffmpeg_executable", "ffmpeg_sha256",
    "ffprobe_executable", "ffprobe_sha256",
    "demucs_version", "model_repository", "model_bag_sha256",
    "checkpoint_filename", "checkpoint_sha256",
}
METADATA_SCRIPT = (
    "import importlib.metadata,json,platform;"
    "print(json.dumps({\"demucs_version\":importlib.metadata.version(\"demucs\"),"
    "\"python_version\":platform.python_version()}))"
)


class ContractError(Exception):
    """A stable failure category plus an actionable, non-sensitive diagnostic."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise ContractError(code, message)


def require_object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        fail("invalid_contract", f"{label} must contain exactly its documented fields")
    return value


def canonical_sha256(value: Any) -> str:
    return hashlib.sha256(json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def normalized_absolute_path(value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or not value.isprintable():
        fail("invalid_path", f"{label} must be a printable absolute path")
    path = Path(value)
    if not path.is_absolute() or os.path.normpath(value) != value:
        fail("invalid_path", f"{label} must be absolute and normalized")
    return path


def require_digest(value: Any, label: str) -> None:
    if not isinstance(value, str) or not SHA256.fullmatch(value):
        fail("invalid_contract", f"{label} must be a lowercase SHA-256 digest")


def validate_values(values: Any) -> dict[str, Any]:
    """Validate the closed configuration without launching tools or writing files."""
    values = require_object(values, VALUE_KEYS, "configuration values")
    for key in ("python_executable", "ffmpeg_executable", "ffprobe_executable", "model_repository"):
        normalized_absolute_path(values[key], key)
    for key in ("python_sha256", "ffmpeg_sha256", "ffprobe_sha256", "model_bag_sha256", "checkpoint_sha256"):
        require_digest(values[key], key)
    if values["demucs_version"] != "4.0.1":
        fail("unsupported_version", "this profile requires Demucs 4.0.1")
    if values["checkpoint_filename"] != CHECKPOINT_FILENAME:
        fail("unsupported_model", "this profile requires the pinned htdemucs_6s checkpoint name")
    if Path(values["ffmpeg_executable"]).name != "ffmpeg":
        fail("invalid_path", "the configured FFmpeg executable must be named ffmpeg")
    if Path(values["ffprobe_executable"]).name != "ffprobe":
        fail("invalid_path", "the configured FFprobe executable must be named ffprobe")
    if Path(values["ffprobe_executable"]).parent != Path(values["ffmpeg_executable"]).parent:
        fail("invalid_path", "FFmpeg and FFprobe must share one explicit executable directory")
    return dict(values)


# Public alias retained for management tools that spell out configuration.
validate_configuration = validate_values


def offline_environment(values: dict[str, Any]) -> dict[str, str]:
    environment = dict(os.environ)
    environment.update({
        "TORCH_HUB_OFFLINE": "1", "HF_HUB_OFFLINE": "1",
        "PATH": str(Path(values["ffmpeg_executable"]).parent),
        "PYTHONDONTWRITEBYTECODE": "1",
    })
    environment.pop("PYTHONPATH", None)
    environment.pop("PYTHONHOME", None)
    return environment


def checked_regular_file(path: Path, label: str, *, allow_symlink: bool = False) -> None:
    try:
        metadata = path.stat() if allow_symlink else path.lstat()
    except OSError as error:
        raise ContractError("dependency_unavailable", f"{label} is unavailable") from error
    if not stat.S_ISREG(metadata.st_mode):
        fail("invalid_file", f"{label} must be a regular file")


def inspect_dependency_metadata(values: dict[str, Any]) -> dict[str, Any]:
    """Read at most 1024 bytes from one fixed isolated metadata query in 15s."""
    process = subprocess.Popen(
        [values["python_executable"], "-I", "-B", "-c", METADATA_SCRIPT],
        env=offline_environment(values), stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    captured = bytearray()
    deadline = time.monotonic() + 15
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    fail("dependency_unavailable", "isolated dependency inspection exceeded 15 seconds")
                for key, _ in selector.select(timeout=remaining):
                    chunk = os.read(key.fileobj.fileno(), 1025 - len(captured))
                    if not chunk:
                        selector.unregister(key.fileobj)
                    else:
                        captured.extend(chunk)
                        if len(captured) > 1024:
                            fail("dependency_unavailable", "dependency inspection exceeded its 1024 byte output bound")
            remaining = deadline - time.monotonic()
            if remaining <= 0 or process.wait(timeout=remaining) != 0:
                fail("dependency_unavailable", "configured Python cannot inspect its installed Demucs package")
    except subprocess.TimeoutExpired as error:
        raise ContractError("dependency_unavailable", "isolated dependency inspection exceeded 15 seconds") from error
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        process.stdout.close()
    try:
        return require_object(json.loads(captured), {"demucs_version", "python_version"}, "dependency observation")
    except (json.JSONDecodeError, UnicodeError) as error:
        raise ContractError("dependency_unavailable", "dependency inspection returned invalid JSON") from error


def verify_dependencies(values: dict[str, Any]) -> dict[str, Any]:
    """Reobserve dependencies before execution or accepted-checkpoint reuse.

    Preserve the original interpreter path when it is a venv symlink: invoking
    its resolved target would silently select the system Python environment.
    """
    values = validate_values(values)
    for label in ("python", "ffmpeg", "ffprobe"):
        path = Path(values[f"{label}_executable"])
        checked_regular_file(path, f"{label} executable", allow_symlink=True)
        if not os.access(path, os.X_OK):
            fail("dependency_unavailable", f"{label} is not executable")
        if sha256_file(path) != values[f"{label}_sha256"]:
            fail("dependency_changed", f"{label} executable digest changed; prepare a new registration")
    repository = Path(values["model_repository"])
    if not repository.is_dir() or repository.resolve() != repository:
        fail("dependency_unavailable", "model repository must be an existing canonical directory")
    bag = repository / f"{MODEL_NAME}.yaml"
    checkpoint = repository / CHECKPOINT_FILENAME
    for path, key, label in (
        (bag, "model_bag_sha256", "model bag"),
        (checkpoint, "checkpoint_sha256", "model checkpoint"),
    ):
        checked_regular_file(path, label)
        if sha256_file(path) != values[key]:
            fail("dependency_changed", f"{label} digest changed; prepare a new registration")
    if checkpoint.stat().st_size == 0 or bag.stat().st_size > 1024:
        fail("unsupported_model", "model bag or checkpoint has an unsupported size")
    if bag.read_text(encoding="utf-8").strip() not in {
        "models: ['5c90dfd2']", "models: [\"5c90dfd2\"]", "models: [5c90dfd2]",
    }:
        fail("unsupported_model", "model bag must select only the pinned htdemucs_6s checkpoint")
    # Demucs resolves a checkpoint by signature prefix; refuse ambiguous local
    # candidates even if the expected file itself still has the correct digest.
    if sorted(path.name for path in repository.glob("5c90dfd2*.th")) != [CHECKPOINT_FILENAME]:
        fail("unsupported_model", "model repository has an ambiguous checkpoint signature")
    try:
        metadata = inspect_dependency_metadata(values)
    except OSError as error:
        raise ContractError("dependency_unavailable", "isolated Python dependency inspection failed") from error
    if metadata["demucs_version"] != "4.0.1":
        fail("unsupported_version", "configured Python does not have Demucs 4.0.1 installed")
    if not isinstance(metadata["python_version"], str) or not re.fullmatch(r"3\.[0-9]+\.[0-9]+", metadata["python_version"]):
        fail("unsupported_version", "dependency inspection did not identify a Python 3 runtime")
    return {
        "python_sha256": values["python_sha256"],
        "python_version": metadata["python_version"],
        "ffmpeg_sha256": values["ffmpeg_sha256"],
        "ffprobe_sha256": values["ffprobe_sha256"],
        "demucs_version": "4.0.1", "model_name": MODEL_NAME,
        "model_bag_sha256": values["model_bag_sha256"],
        "checkpoint_filename": CHECKPOINT_FILENAME,
        "checkpoint_sha256": values["checkpoint_sha256"],
    }


def inspect_pcm16_wav(path: Path, *, output: bool = False) -> dict[str, Any]:
    """Validate every RIFF chunk and the full PCM payload without trusting Demucs.

    PCM16 has no compressed bitstream: validated frame alignment and complete
    signed sample words establish decodability. RF64, float, extensible formats,
    duplicate fmt/data chunks, trailing bytes, and truncated files are refused.
    """
    checked_regular_file(path, "WAV artifact")
    size = path.stat().st_size
    if size < 44 or size > MAXIMUM_WAV_BYTES:
        fail("invalid_audio", "WAV artifact is empty, truncated, or exceeds the 256 MiB bound")
    format_values = None
    data_bytes = None
    with path.open("rb") as source:
        header = source.read(12)
        if header[:4] != b"RIFF" or header[8:] != b"WAVE" or struct.unpack("<I", header[4:8])[0] + 8 != size:
            fail("invalid_audio", "WAV must have an exact complete RIFF/WAVE envelope")
        position = 12
        while position < size:
            chunk_header = source.read(8)
            if len(chunk_header) != 8:
                fail("invalid_audio", "WAV contains an incomplete chunk header")
            name, count = struct.unpack("<4sI", chunk_header)
            position += 8
            padded_count = count + count % 2
            if position + padded_count > size:
                fail("invalid_audio", "WAV contains a truncated chunk")
            if name == b"fmt ":
                if format_values is not None or count not in (16, 18):
                    fail("invalid_audio", "WAV has duplicate or unsupported format chunks")
                format_chunk = source.read(count)
                if count == 18 and format_chunk[16:] != b"\x00\x00":
                    fail("invalid_audio", "WAV format extension is unsupported")
                format_values = struct.unpack("<HHIIHH", format_chunk[:16])
            else:
                if name == b"data":
                    if data_bytes is not None or format_values is None:
                        fail("invalid_audio", "WAV requires one data chunk after its format chunk")
                    data_bytes = count
                # Read rather than seek so a concurrent truncation cannot be
                # accepted merely because the header declares a plausible size.
                remaining = count
                while remaining:
                    chunk = source.read(min(1024 * 1024, remaining))
                    if not chunk:
                        fail("invalid_audio", "WAV payload was truncated during inspection")
                    remaining -= len(chunk)
            if count % 2 and len(source.read(1)) != 1:
                fail("invalid_audio", "WAV chunk padding is missing")
            position += padded_count
        if source.read(1):
            fail("invalid_audio", "WAV grew during inspection")
    if format_values is None or data_bytes is None:
        fail("invalid_audio", "WAV is missing format or audio data")
    encoding, channels, sample_rate, byte_rate, block_align, bits = format_values
    if encoding != 1 or bits != 16 or channels not in (1, 2) or not 8000 <= sample_rate <= 192000:
        fail("unsupported_audio", "this profile accepts only PCM16 mono/stereo WAV at 8000–192000 Hz")
    if block_align != channels * 2 or byte_rate != sample_rate * block_align or data_bytes == 0 or data_bytes % block_align:
        fail("invalid_audio", "WAV sample frames or byte-rate fields are inconsistent")
    sample_frames = data_bytes // block_align
    if sample_frames > sample_rate * MAXIMUM_DURATION_SECONDS:
        fail("unsupported_audio", "WAV duration exceeds this profile's 600 second bound")
    if output and (sample_rate, channels) != (44100, 2):
        fail("invalid_output", "Demucs stem must be PCM16 stereo WAV at 44100 Hz")
    return {
        "sha256": sha256_file(path), "byte_count": size,
        "sample_rate_hz": sample_rate, "channels": channels,
        "sample_format": "pcm_s16le", "sample_frames": sample_frames,
        "duration": {"numerator": sample_frames, "denominator": sample_rate},
    }


def validate_envelope(configuration: Any) -> dict[str, Any]:
    configuration = require_object(configuration, {
        "schema", "provider", "capability", "configuration_schema", "values",
        "effective_configuration_sha256",
    }, "configuration envelope")
    expected = {
        "schema": "aniflow.provider-configuration/v1",
        "provider": {"id": PROVIDER_ID, "version": PROVIDER_VERSION},
        "capability": {"id": CAPABILITY_ID, "version": "1.0.0"},
        "configuration_schema": {"id": CONFIGURATION_SCHEMA_ID, "version": "1.0.0", "sha256": CONFIGURATION_SCHEMA_SHA256},
    }
    for key, value in expected.items():
        if configuration[key] != value:
            fail("invalid_contract", f"configuration {key} does not match the typed Demucs profile")
    values = validate_values(configuration["values"])
    if configuration["effective_configuration_sha256"] != canonical_sha256(values):
        fail("invalid_contract", "effective configuration digest is stale")
    return values


def validate_binding(binding: Any, port: str) -> tuple[dict[str, Any], Path]:
    evidence = port == "evidence"
    keys = {"port", "artifact_id", "artifact_type", "artifact_role", "kind", "path"}
    if not evidence:
        keys.add("stream_role")
    binding = require_object(binding, keys, f"{port} binding")
    expected = {
        "port": port, "kind": "file",
        "artifact_type": "application/vnd.aniflow.demucs-separation+json" if evidence else "audio/wav",
        "artifact_role": "validation_evidence" if evidence else "temporal_component",
    }
    if not evidence:
        expected["stream_role"] = "audio"
    if any(binding[key] != value for key, value in expected.items()):
        fail("invalid_contract", f"{port} binding does not match the typed Demucs profile")
    if not isinstance(binding["artifact_id"], str) or not LOCAL_ID.fullmatch(binding["artifact_id"]):
        fail("invalid_contract", "artifact identity is invalid")
    return binding, normalized_absolute_path(binding["path"], f"{port} path")


def paths_overlap(left: Path, right: Path) -> bool:
    return left == right or left in right.parents or right in left.parents


def validate_invocation(request: Any) -> tuple[dict[str, Any], Path, dict[str, Path]]:
    request = require_object(request, {
        "schema", "execution_semantics", "stage_id", "provider_lock_sha256",
        "configuration", "inputs", "outputs",
    }, "invocation")
    if request["schema"] != "aniflow.provider-invocation/v1" or request["execution_semantics"] != "aniflow.provider-invocation/direct-argv/v1":
        fail("invalid_contract", "unsupported invocation schema or execution semantics")
    if not isinstance(request["stage_id"], str) or not LOCAL_ID.fullmatch(request["stage_id"]):
        fail("invalid_contract", "stage identity is invalid")
    require_digest(request["provider_lock_sha256"], "provider lock digest")
    values = validate_envelope(request["configuration"])
    if not isinstance(request["inputs"], list) or len(request["inputs"]) != 1:
        fail("invalid_contract", "expected exactly one audio input")
    _, source = validate_binding(request["inputs"][0], "audio")
    checked_regular_file(source, "source audio")
    if source.resolve() != source:
        fail("invalid_path", "source path must have canonical, non-symlink components")
    if not isinstance(request["outputs"], list) or len(request["outputs"]) != 3:
        fail("invalid_contract", "expected vocals, accompaniment, and evidence outputs")
    outputs = {}
    artifact_ids = {request["inputs"][0]["artifact_id"]}
    for binding in request["outputs"]:
        if not isinstance(binding, dict) or binding.get("port") not in {"vocals", "accompaniment", "evidence"}:
            fail("invalid_contract", "unknown separation output port")
        binding, path = validate_binding(binding, binding["port"])
        if binding["port"] in outputs or binding["artifact_id"] in artifact_ids:
            fail("invalid_contract", "duplicate output port or artifact identity")
        artifact_ids.add(binding["artifact_id"])
        if path.exists() or path.is_symlink():
            fail("output_exists", "separation never overwrites an existing output")
        if path.parent.resolve() != path.parent:
            fail("invalid_path", "output parents must have canonical, non-symlink components")
        outputs[binding["port"]] = path
    if len(set(outputs.values())) != 3 or len({path.parent for path in outputs.values()}) != 1:
        fail("invalid_path", "all three unique outputs must share one parent directory")
    parent = outputs["evidence"].parent
    repository = Path(values["model_repository"])
    if paths_overlap(parent, source) or paths_overlap(parent, repository) or paths_overlap(source, repository):
        fail("invalid_path", "source, output workspace, and external model cache must be separate")
    return values, source, outputs


def separation_arguments(values: dict[str, Any], source: Path, destination: Path) -> list[str]:
    return [
        values["python_executable"], "-I", "-B", "-m", "demucs.separate",
        "--name", MODEL_NAME, "--two-stems", "vocals", "--device", "cpu",
        "--shifts", "0", "--jobs", "0", "--repo", values["model_repository"],
        "--out", str(destination), "--", str(source),
    ]


def require_duration_matches(source: dict[str, Any], stem: dict[str, Any]) -> None:
    difference = abs(source["sample_frames"] * stem["sample_rate_hz"] - stem["sample_frames"] * source["sample_rate_hz"])
    if difference * 1000 > DURATION_TOLERANCE_MILLISECONDS * source["sample_rate_hz"] * stem["sample_rate_hz"]:
        fail("duration_mismatch", "stem duration differs from source by more than 20 milliseconds")


def execute(request: dict[str, Any]) -> dict[str, Any]:
    values, source, outputs = validate_invocation(request)
    dependencies = verify_dependencies(values)
    source_observation = inspect_pcm16_wav(source)
    parent = outputs["evidence"].parent
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".demucs-", dir=parent) as temporary:
        temporary_path = Path(temporary)
        # Snapshot the input under a predictable basename; source metadata and
        # digest are checked independently again after separation.
        snapshot = temporary_path / "input.wav"
        shutil.copyfile(source, snapshot)
        if sha256_file(snapshot) != source_observation["sha256"]:
            fail("source_changed", "source changed while its immutable input snapshot was created")
        candidate_directory = temporary_path / "separated"
        processor_temporary = temporary_path / "processor-tmp"
        processor_temporary.mkdir()
        environment = offline_environment(values)
        environment.update({key: str(processor_temporary) for key in ("TMPDIR", "TMP", "TEMP")})
        result = subprocess.run(
            separation_arguments(values, snapshot, candidate_directory),
            env=environment, check=False,
        )
        if result.returncode != 0:
            fail("provider_failed", "Demucs exited unsuccessfully; no separation evidence was published")
        if sha256_file(snapshot) != source_observation["sha256"]:
            fail("source_changed", "Demucs changed its immutable input snapshot")
        if inspect_pcm16_wav(source) != source_observation:
            fail("source_changed", "source audio changed during separation")
        if verify_dependencies(values) != dependencies:
            fail("dependency_changed", "dependency observations changed during separation")
        stem_root = candidate_directory / MODEL_NAME / "input"
        if not stem_root.is_dir() or stem_root.resolve() != stem_root:
            fail("invalid_output", "Demucs stem directory is missing or has symlink components")
        candidates = {"vocals": stem_root / "vocals.wav", "accompaniment": stem_root / "no_vocals.wav"}
        actual_files = set()
        for path in candidate_directory.rglob("*"):
            if path.is_symlink() or not (path.is_file() or path.is_dir()):
                fail("invalid_output", "Demucs produced a symlink or unsupported filesystem entry")
            if path.is_file():
                actual_files.add(path)
        if actual_files != set(candidates.values()):
            fail("invalid_output", "Demucs did not produce exactly the two declared stems")
        observations = {}
        for port, candidate in candidates.items():
            observations[port] = inspect_pcm16_wav(candidate, output=True)
            require_duration_matches(source_observation, observations[port])
        if observations["vocals"]["sample_frames"] != observations["accompaniment"]["sample_frames"]:
            fail("duration_mismatch", "vocal and accompaniment sample counts differ")
        evidence = {
            "schema": "aniflow.demucs-separation/v1", "accepted": True,
            "stage_id": request["stage_id"],
            "provider": {"id": PROVIDER_ID, "version": PROVIDER_VERSION},
            "provider_lock_sha256": request["provider_lock_sha256"],
            "effective_configuration_sha256": request["configuration"]["effective_configuration_sha256"],
            "source": {"artifact_id": request["inputs"][0]["artifact_id"], **source_observation},
            "outputs": [
                {"port": port, "artifact_id": next(item["artifact_id"] for item in request["outputs"] if item["port"] == port), **observations[port]}
                for port in ("vocals", "accompaniment")
            ],
            "dependencies": dependencies,
            "settings": {"device": "cpu", "two_stems": "vocals", "shifts": 0, "jobs": 0, "duration_tolerance_milliseconds": DURATION_TOLERANCE_MILLISECONDS},
            "offline": {"torch_hub_offline": True, "hf_hub_offline": True, "local_model_repository": True, "network_sandbox": False},
            "validation": {"pcm_integrity": "passed", "duration_alignment": "passed", "source_unchanged": "passed", "perceptual_separation_quality": "not_tested"},
        }
        candidate_evidence = temporary_path / "evidence.json"
        candidate_evidence.write_text(json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        # Same-filesystem hard links publish complete files without clobbering.
        # Evidence is last; the owning runtime, not this evidence alone, accepts
        # the complete stage. Interrupted partial publication cannot checkpoint.
        published = []
        try:
            for port, candidate in (*candidates.items(), ("evidence", candidate_evidence)):
                os.link(candidate, outputs[port])
                published.append(outputs[port])
        except OSError:
            for path in published:
                path.unlink()
            raise
    return evidence


def reject_duplicate_fields(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            fail("invalid_contract", "invocation contains a duplicate JSON field")
        result[key] = value
    return result


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] != "--aniflow-invocation":
        fail("invalid_contract", "expected --aniflow-invocation <absolute-request-path>")
    request_path = normalized_absolute_path(argv[2], "invocation request")
    if request_path.stat().st_size > 1024 * 1024:
        fail("invalid_contract", "invocation request exceeds the 1 MiB bound")
    with request_path.open(encoding="utf-8") as handle:
        request = json.load(handle, object_pairs_hook=reject_duplicate_fields)
    execute(request)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except ContractError as error:
        print(json.dumps({"code": error.code, "message": str(error)}, sort_keys=True), file=sys.stderr)
        raise SystemExit(65)
    except (OSError, ValueError, UnicodeError, struct.error) as error:
        # Do not dump invocation paths, private audio names, or native tool logs.
        print(json.dumps({"code": "io_or_contract_failure", "message": type(error).__name__}, sort_keys=True), file=sys.stderr)
        raise SystemExit(74)
