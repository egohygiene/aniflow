#!/usr/bin/env python3
"""Prepare and recheck a local provider bundle; delegate execution to aniflow.

No media algorithm or alternate run state lives here. Pipeline v3's public
library (through its CLI adapter) owns execution, status, and compatible resume.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent
FILES = ("provider.py", "configuration.schema.json", "pipeline.yml", "workflow.py")
EFFECTS = ("filesystem-read", "filesystem-write", "environment-read", "subprocess", "ai")


def adapter():
    spec = importlib.util.spec_from_file_location("aniflow_demucs_provider", ROOT / "provider.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_sha256(value) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                                     separators=(",", ":")).encode()).hexdigest()


def write_json(path: Path, value) -> None:
    with path.open("x", encoding="utf-8") as destination:
        json.dump(value, destination, indent=2, ensure_ascii=False)
        destination.write("\n")


def load_json(path: Path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate JSON field")
            result[key] = value
        return result
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 1024 * 1024:
        raise ValueError("bundle JSON must be a bounded regular file")
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique)


def documents(values, schema_sha256):
    schema = {"id": "aniflow.demucs.configuration/v1", "version": "1.0.0",
              "sha256": schema_sha256}
    manifest = load_json(ROOT / "manifest.json")
    manifest["configuration_schemas"] = [schema]
    capability = manifest["capabilities"][0]
    capability["configuration_schema"] = schema
    capability["requirements"]["models"][0]["sha256"] = values["checkpoint_sha256"]
    configuration = {
        "schema": "aniflow.provider-configuration/v1",
        "provider": {"id": "org.egohygiene.aniflow.demucs", "version": "1.0.0"},
        "capability": {"id": "aniflow/audio.separate", "version": "1.0.0"},
        "configuration_schema": schema, "values": values,
        "effective_configuration_sha256": canonical_sha256(values),
    }
    registration = {
        "schema": "aniflow.provider-registration/v1", "registration_id": "demucs-local",
        "manifest": "manifest.json", "configuration": "configuration.json",
        "executable": "provider.py", "implementation_id": "aniflow-demucs-v1",
        "components": {
            "tools": [{"id": "demucs", "version": "4.0.1"}], "codecs": [],
            "models": [{"id": "htdemucs_6s", "version": "4.0.1",
                        "sha256": values["checkpoint_sha256"]}],
        },
    }
    return manifest, configuration, registration


def prepare(arguments) -> None:
    provider = adapter()
    # Preserve a venv interpreter's symlink spelling: resolving it would select
    # the base interpreter and silently lose its installed Demucs environment.
    python_path = Path(os.path.abspath(arguments.python))
    ffmpeg_path = Path(os.path.abspath(arguments.ffmpeg))
    cache = Path(os.path.abspath(arguments.model_repository))
    output = Path(os.path.abspath(arguments.output_directory))
    if output.exists() or output.is_symlink():
        raise ValueError("bundle destination already exists; choose a new directory")
    if not output.parent.is_dir() or output.parent.resolve() != output.parent:
        raise ValueError("bundle destination needs an existing symlink-free parent")
    if output == cache or cache in output.parents or output in cache.parents:
        raise ValueError("model repository and provider bundle must be separate")
    checkpoint = "5c90dfd2-34c22ccb.th"
    values = {
        "python_executable": str(python_path), "python_sha256": sha256(python_path),
        "ffmpeg_executable": str(ffmpeg_path), "ffmpeg_sha256": sha256(ffmpeg_path),
        "demucs_version": "4.0.1", "model_repository": str(cache),
        "model_bag_sha256": sha256(cache / "htdemucs_6s.yaml"),
        "checkpoint_filename": checkpoint, "checkpoint_sha256": sha256(cache / checkpoint),
    }
    provider.validate_configuration(values)
    provider.verify_dependencies(values)
    manifest, configuration, registration = documents(values, sha256(ROOT / FILES[1]))
    # Reserve a unique destination and only remove our own failed preparation.
    output.mkdir(mode=0o700)
    try:
        for name in FILES:
            shutil.copyfile(ROOT / name, output / name)
        (output / "provider.py").chmod(0o755)
        write_json(output / "manifest.json", manifest)
        write_json(output / "configuration.json", configuration)
        write_json(output / "registration.json", registration)
        write_json(output / "bundle.json", {
            "schema": "aniflow.demucs-local-bundle/v1",
            "files": {name: sha256(output / name) for name in
                      (*FILES, "manifest.json", "configuration.json", "registration.json")},
        })
    except BaseException:
        shutil.rmtree(output)
        raise
    print(output / "registration.json")


def check(registration: Path):
    registration = Path(os.path.abspath(registration))
    directory = registration.parent
    if directory.resolve() != directory or registration.name != "registration.json":
        raise ValueError("use a symlink-free prepared bundle registration.json")
    seal = load_json(directory / "bundle.json")
    expected_files = {*FILES, "manifest.json", "configuration.json", "registration.json"}
    if (set(seal) != {"schema", "files"}
            or seal["schema"] != "aniflow.demucs-local-bundle/v1"
            or set(seal["files"]) != expected_files):
        raise ValueError("invalid local bundle receipt")
    for name, digest in seal["files"].items():
        path = directory / name
        if path.is_symlink() or not path.is_file() or sha256(path) != digest:
            raise ValueError(f"prepared bundle changed: {name}; prepare a new bundle")
    for name in FILES:
        if sha256(directory / name) != sha256(ROOT / name):
            raise ValueError(f"bundle differs from this provider version: {name}")
    configuration = load_json(directory / "configuration.json")
    provider = adapter()
    values = provider.validate_configuration(configuration["values"])
    expected = documents(values, sha256(directory / "configuration.schema.json"))
    for name, document in zip(("manifest.json", "configuration.json", "registration.json"), expected):
        if load_json(directory / name) != document:
            raise ValueError(f"incoherent provider bundle: {name}")
    provider.verify_dependencies(values)
    return directory, values


def execute(arguments) -> int:
    directory, values = check(Path(arguments.registration))
    cache = Path(values["model_repository"])
    target = Path(arguments.output_directory if arguments.action == "run"
                  else arguments.run_directory).absolute().resolve()
    if target == cache or target in cache.parents or cache in target.parents:
        raise ValueError("run workspace must be separate from the model repository")
    command = [arguments.aniflow, "--output", "json"]
    if arguments.action == "run":
        command += ["run-v3", "--pipeline", str(directory / "pipeline.yml"),
                    "--output-directory", str(target), "--offline",
                    "--host-cpu-threads", str(arguments.host_cpu_threads),
                    "--host-memory-mib", str(arguments.host_memory_mib),
                    "--host-storage-mib", str(arguments.host_storage_mib)]
        for effect in EFFECTS:
            command += ["--allow-side-effect", effect]
    else:
        command += ["resume-v3", str(target)]
    command += ["--input", f"source_audio={arguments.input}",
                "--provider-registration", str(directory / "registration.json"),
                "--provider-timeout-seconds", str(arguments.timeout_seconds),
                "--maximum-stdout-bytes", "1048576", "--maximum-stderr-bytes", "4194304",
                "--maximum-artifact-files", "3", "--maximum-artifact-bytes", "536870912"]
    # Replace this thin delivery process so cancellation reaches aniflow's
    # existing process-group handler directly, without a second lifecycle.
    os.execvp(command[0], command)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    preparation = commands.add_parser("prepare", help="verify installed local assets; download nothing")
    preparation.add_argument("--python", required=True)
    preparation.add_argument("--ffmpeg", required=True)
    preparation.add_argument("--model-repository", required=True)
    preparation.add_argument("--output-directory", required=True)
    verification = commands.add_parser("check", help="reverify prepared dependencies without processing media")
    verification.add_argument("--registration", required=True)
    for action in ("run", "resume"):
        operation = commands.add_parser(action)
        operation.add_argument("--registration", required=True)
        operation.add_argument("--input", required=True)
        operation.add_argument("--aniflow", default="aniflow")
        operation.add_argument("--timeout-seconds", type=int, default=21600)
        if action == "run":
            operation.add_argument("--output-directory", required=True)
            for resource in ("cpu-threads", "memory-mib", "storage-mib"):
                operation.add_argument(f"--host-{resource}", required=True, type=int)
        else:
            operation.add_argument("--run-directory", required=True)
    arguments = parser.parse_args()
    try:
        if arguments.action == "prepare":
            prepare(arguments)
        elif arguments.action == "check":
            check(Path(arguments.registration))
            print("local provider bundle and dependency identities verified")
        else:
            return execute(arguments)
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Demucs workflow refused: {error}", file=sys.stderr)
        return 1
    except Exception as error:
        # Provider contract failures stay printable without a traceback that
        # could dump invocation context or paths into machine stdout.
        print(f"Demucs workflow refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
