#!/usr/bin/env python3
"""Hermetic reference implementation of provider-invocation v1.

The provider deliberately uses only the Python standard library. It accepts the
single aniflow direct-argv ABI, validates the closed request it consumes, reads
only bound inputs, and writes only beneath bound output paths.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
from typing import Any, NoReturn


PROVIDER_ID = "org.egohygiene.aniflow.reference"
PROVIDER_VERSION = "1.0.0"
CONFIGURATION_SCHEMA_ID = "aniflow.reference-provider.configuration/v1"
CONFIGURATION_SCHEMA_VERSION = "1.0.0"
# Replaced alongside the checked-in provider-owned schema whenever its exact
# bytes change.
CONFIGURATION_SCHEMA_SHA256 = "e1afbc8a40457b98255976814a36297752914c3787ebf3aac534f152bca3a317"

LOCAL_ID = re.compile(r"^[a-z][a-z0-9_-]*$")
SHA256 = re.compile(r"^[a-f0-9]{64}$")

PROFILES = {
    "aniflow/frame.process": {
        "mode": "copy",
        "input": {
            "port": "frames",
            "artifact_type": "application/vnd.aniflow.reference-frame-set+directory",
            "artifact_role": "temporal_component",
            "stream_role": "video",
            "kind": "directory",
        },
        "output": {
            "port": "processed_frames",
            "artifact_type": "application/vnd.aniflow.reference-frame-set+directory",
            "artifact_role": "intermediate",
            "stream_role": "video",
            "kind": "directory",
        },
    },
    "aniflow/audio.process": {
        "mode": "copy",
        "input": {
            "port": "audio",
            "artifact_type": "application/vnd.aniflow.reference-audio",
            "artifact_role": "temporal_component",
            "stream_role": "audio",
            "kind": "file",
        },
        "output": {
            "port": "processed_audio",
            "artifact_type": "application/vnd.aniflow.reference-audio",
            "artifact_role": "intermediate",
            "stream_role": "audio",
            "kind": "file",
        },
    },
    "aniflow/whole-video.process": {
        "mode": "copy",
        "input": {
            "port": "video",
            "artifact_type": "application/vnd.aniflow.reference-video",
            "artifact_role": "temporal_source",
            "stream_role": "video",
            "kind": "file",
        },
        "output": {
            "port": "processed_video",
            "artifact_type": "application/vnd.aniflow.reference-video",
            "artifact_role": "candidate_master",
            "stream_role": "video",
            "kind": "file",
        },
    },
    "aniflow/artifact.validate": {
        "mode": "validation_evidence",
        "input": {
            "port": "artifact",
            "artifact_type": "application/octet-stream",
            "artifact_role": "candidate_master",
            "kind": "file",
        },
        "output": {
            "port": "evidence",
            "artifact_type": "application/vnd.aniflow.validation-evidence+json",
            "artifact_role": "validation_evidence",
            "kind": "file",
        },
    },
}


class ContractError(Exception):
    """A printable, non-sensitive invocation contract error."""


def reject(message: str, exit_code: int = 64) -> NoReturn:
    print(f"reference provider rejected invocation: {message}", file=sys.stderr)
    raise SystemExit(exit_code)


def require_object(
    value: Any,
    *,
    required: set[str],
    optional: set[str] | None = None,
    label: str,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be an object")
    allowed = required | (optional or set())
    missing = sorted(required - value.keys())
    unknown = sorted(value.keys() - allowed)
    if missing:
        raise ContractError(f"{label} is missing required fields: {', '.join(missing)}")
    if unknown:
        raise ContractError(f"{label} contains unknown fields: {', '.join(unknown)}")
    return value


def require_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or not value.isprintable():
        raise ContractError(f"{label} must be a nonempty printable string")
    return value


def require_local_id(value: Any, label: str) -> str:
    text = require_string(value, label)
    if LOCAL_ID.fullmatch(text) is None:
        raise ContractError(f"{label} is not a normalized local identifier")
    return text


def require_sha256(value: Any, label: str) -> str:
    text = require_string(value, label)
    if SHA256.fullmatch(text) is None:
        raise ContractError(f"{label} is not a lowercase SHA-256 digest")
    return text


def require_exact(value: Any, expected: Any, label: str) -> None:
    if value != expected:
        raise ContractError(f"{label} does not match the reference provider contract")


def require_normalized_absolute_path(value: Any, label: str) -> Path:
    text = require_string(value, label)
    path = Path(text)
    if not path.is_absolute() or os.path.normpath(text) != text:
        raise ContractError(f"{label} must be an absolute normalized path")
    if any(part in {".", ".."} for part in path.parts):
        raise ContractError(f"{label} must not contain dot components")
    return path


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def reject_duplicate_fields(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ContractError("invocation request contains a duplicate field")
        value[key] = item
    return value


def validate_configuration(value: Any) -> tuple[str, str]:
    configuration = require_object(
        value,
        required={
            "schema",
            "provider",
            "capability",
            "configuration_schema",
            "values",
            "effective_configuration_sha256",
        },
        label="configuration",
    )
    require_exact(
        configuration["schema"],
        "aniflow.provider-configuration/v1",
        "configuration.schema",
    )

    provider = require_object(
        configuration["provider"],
        required={"id", "version"},
        label="configuration.provider",
    )
    require_exact(provider["id"], PROVIDER_ID, "configuration.provider.id")
    require_exact(
        provider["version"], PROVIDER_VERSION, "configuration.provider.version"
    )

    capability = require_object(
        configuration["capability"],
        required={"id", "version"},
        label="configuration.capability",
    )
    capability_id = require_string(capability["id"], "configuration.capability.id")
    if capability_id not in PROFILES:
        raise ContractError("configuration.capability.id is not supported")
    require_exact(capability["version"], "1.0.0", "configuration.capability.version")

    schema = require_object(
        configuration["configuration_schema"],
        required={"id", "version", "sha256"},
        label="configuration.configuration_schema",
    )
    require_exact(schema["id"], CONFIGURATION_SCHEMA_ID, "configuration schema id")
    require_exact(
        schema["version"], CONFIGURATION_SCHEMA_VERSION, "configuration schema version"
    )
    require_exact(
        schema["sha256"], CONFIGURATION_SCHEMA_SHA256, "configuration schema digest"
    )

    values = require_object(
        configuration["values"], required={"mode"}, label="configuration.values"
    )
    mode = require_string(values["mode"], "configuration.values.mode")
    if mode not in {"copy", "validation_evidence", "missing_output", "nonzero"}:
        raise ContractError("configuration.values.mode is unsupported")
    expected_digest = canonical_sha256(values)
    require_exact(
        require_sha256(
            configuration["effective_configuration_sha256"],
            "configuration.effective_configuration_sha256",
        ),
        expected_digest,
        "effective configuration digest",
    )
    return capability_id, mode


def validate_binding(
    value: Any, expected: dict[str, str], *, label: str, output: bool
) -> tuple[dict[str, Any], Path]:
    binding = require_object(
        value,
        required={
            "port",
            "artifact_id",
            "artifact_type",
            "artifact_role",
            "kind",
            "path",
        },
        optional={"stream_role"},
        label=label,
    )
    require_local_id(binding["port"], f"{label}.port")
    require_local_id(binding["artifact_id"], f"{label}.artifact_id")
    for field in ("port", "artifact_type", "artifact_role", "kind"):
        require_exact(binding[field], expected[field], f"{label}.{field}")
    expected_stream_role = expected.get("stream_role")
    if expected_stream_role is None:
        if "stream_role" in binding:
            raise ContractError(f"{label}.stream_role must be omitted")
    else:
        require_exact(
            binding.get("stream_role"), expected_stream_role, f"{label}.stream_role"
        )
    path = require_normalized_absolute_path(binding["path"], f"{label}.path")
    if output:
        if path.exists() or path.is_symlink():
            raise ContractError(f"{label}.path must not exist before execution")
    else:
        try:
            metadata = path.lstat()
        except OSError as error:
            raise ContractError(f"{label}.path is unavailable") from error
        if stat.S_ISLNK(metadata.st_mode):
            raise ContractError(f"{label}.path must not be a symbolic link")
        kind_matches = (
            expected["kind"] == "file" and stat.S_ISREG(metadata.st_mode)
        ) or (expected["kind"] == "directory" and stat.S_ISDIR(metadata.st_mode))
        if not kind_matches:
            raise ContractError(f"{label}.path has the wrong filesystem kind")
    return binding, path


def paths_overlap(left: Path, right: Path) -> bool:
    left_parts = tuple(os.path.normcase(part) for part in left.parts)
    right_parts = tuple(os.path.normcase(part) for part in right.parts)
    length = min(len(left_parts), len(right_parts))
    return left_parts[:length] == right_parts[:length]


def validate_invocation(value: Any) -> tuple[str, str, dict[str, Any], Path, Path]:
    request = require_object(
        value,
        required={
            "schema",
            "execution_semantics",
            "stage_id",
            "provider_lock_sha256",
            "configuration",
            "inputs",
            "outputs",
        },
        label="invocation",
    )
    require_exact(request["schema"], "aniflow.provider-invocation/v1", "invocation.schema")
    require_exact(
        request["execution_semantics"],
        "aniflow.provider-invocation/direct-argv/v1",
        "invocation.execution_semantics",
    )
    require_local_id(request["stage_id"], "invocation.stage_id")
    require_sha256(request["provider_lock_sha256"], "invocation.provider_lock_sha256")
    capability_id, mode = validate_configuration(request["configuration"])
    profile = PROFILES[capability_id]

    if not isinstance(request["inputs"], list) or len(request["inputs"]) != 1:
        raise ContractError("invocation.inputs must contain exactly one binding")
    if not isinstance(request["outputs"], list) or len(request["outputs"]) != 1:
        raise ContractError("invocation.outputs must contain exactly one binding")
    input_binding, input_path = validate_binding(
        request["inputs"][0], profile["input"], label="invocation.inputs[0]", output=False
    )
    _, output_path = validate_binding(
        request["outputs"][0], profile["output"], label="invocation.outputs[0]", output=True
    )
    if paths_overlap(input_path, output_path):
        raise ContractError("input and output paths must not overlap")
    return capability_id, mode, input_binding, input_path, output_path


def ensure_output_parent(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def transformed_header(capability_id: str) -> bytes:
    return f"aniflow-reference-provider/v1\ncapability={capability_id}\n".encode("utf-8")


def write_transformed_file(source: Path, destination: Path, capability_id: str) -> None:
    ensure_output_parent(destination)
    with source.open("rb") as reader, destination.open("xb") as writer:
        writer.write(transformed_header(capability_id))
        shutil.copyfileobj(reader, writer, length=64 * 1024)


def write_transformed_directory(source: Path, destination: Path, capability_id: str) -> None:
    destination.mkdir(parents=True, exist_ok=False)
    wrote_file = False
    for root, directory_names, file_names in os.walk(source, followlinks=False):
        directory_names.sort()
        file_names.sort()
        root_path = Path(root)
        relative_root = root_path.relative_to(source)
        for directory_name in directory_names:
            source_directory = root_path / directory_name
            if source_directory.is_symlink():
                raise ContractError("directory input contains a symbolic link")
            (destination / relative_root / directory_name).mkdir(exist_ok=False)
        for file_name in file_names:
            source_file = root_path / file_name
            if source_file.is_symlink() or not source_file.is_file():
                raise ContractError("directory input contains an unsupported entry")
            write_transformed_file(
                source_file, destination / relative_root / file_name, capability_id
            )
            wrote_file = True
    if not wrote_file:
        raise ContractError("directory input must contain at least one regular file")


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    byte_count = 0
    with path.open("rb") as source:
        while chunk := source.read(64 * 1024):
            digest.update(chunk)
            byte_count += len(chunk)
    return digest.hexdigest(), byte_count


def artifact_observation(path: Path) -> dict[str, Any]:
    if path.is_file():
        digest, byte_count = sha256_file(path)
        return {
            "artifact_sha256": digest,
            "byte_count": byte_count,
            "file_count": 1,
            "kind": "file",
        }
    entries = []
    byte_count = 0
    for candidate in sorted(path.rglob("*"), key=lambda value: value.as_posix()):
        if candidate.is_symlink():
            raise ContractError("directory input contains a symbolic link")
        if candidate.is_dir():
            continue
        if not candidate.is_file():
            raise ContractError("directory input contains an unsupported entry")
        digest, size = sha256_file(candidate)
        entries.append(
            {"path": candidate.relative_to(path).as_posix(), "sha256": digest, "size": size}
        )
        byte_count += size
    return {
        "artifact_sha256": canonical_sha256(entries),
        "byte_count": byte_count,
        "file_count": len(entries),
        "kind": "directory",
    }


def write_validation_evidence(
    input_binding: dict[str, Any], input_path: Path, output_path: Path
) -> None:
    observation = artifact_observation(input_path)
    evidence = {
        "schema": "aniflow.reference-validation-evidence/v1",
        "accepted": True,
        "artifact_id": input_binding["artifact_id"],
        **observation,
    }
    ensure_output_parent(output_path)
    with output_path.open("x", encoding="utf-8", newline="\n") as destination:
        json.dump(evidence, destination, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
        destination.write("\n")


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] != "--aniflow-invocation":
        reject("expected exactly --aniflow-invocation <absolute-request-path>")
    request_path = require_normalized_absolute_path(argv[2], "invocation request path")
    try:
        with request_path.open("r", encoding="utf-8") as source:
            request = json.load(source, object_pairs_hook=reject_duplicate_fields)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ContractError("invocation request is unavailable or invalid JSON") from error

    capability_id, mode, input_binding, input_path, output_path = validate_invocation(request)
    if mode == "nonzero":
        print("reference provider configured for deterministic nonzero exit", file=sys.stderr)
        return 23
    if mode == "missing_output":
        return 0

    profile = PROFILES[capability_id]
    require_exact(mode, profile["mode"], "configuration mode for capability")
    if mode == "validation_evidence":
        write_validation_evidence(input_binding, input_path, output_path)
    elif profile["input"]["kind"] == "directory":
        write_transformed_directory(input_path, output_path, capability_id)
    else:
        write_transformed_file(input_path, output_path, capability_id)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except ContractError as error:
        reject(str(error))
