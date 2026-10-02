"""Two isolated native builds and a normalized archive; no download or signing."""

from __future__ import annotations

import json
import os
from pathlib import Path
import platform
import time

from corpus.process import execute
from .common import (ROOT, checksum_bytes, clean_revision, command, digest, git, inventory,
                     json_bytes, new_directory, regular_bytes, require, write_new)
from .plan import verify_candidate
from .bundle import archive_bytes, binary_identity, payload_files, spdx


def build(target, output, work, version, revision, repository=ROOT):
    plan = verify_candidate(version, revision, repository)
    config = plan["build"]
    require(target in [item["triple"] for item in config["targets"]], "target is outside the declared matrix")
    toolchain = config["rust_toolchain"]
    rustc = command(["rustup", "run", toolchain, "rustc", "--version", "--verbose"], repository=repository).decode()
    cargo = command(["rustup", "run", toolchain, "cargo", "--version"], repository=repository).decode().strip()
    require("host: " + target + "\n" in rustc, "release builds must run on their native target")
    require(rustc.startswith("rustc " + toolchain + " "), "Rust toolchain drift")
    work = new_directory(work, repository)
    output_path = Path(output).absolute()
    require(not output_path.exists() and not output_path.is_relative_to(work), "output must be a fresh separate directory")
    epoch = int(git("show", "--no-patch", "--format=%ct", revision, repository=repository))
    binaries, results = [], []
    deadline = time.monotonic() + 2400
    for index in [1, 2]:
        target_dir = work / ("build-" + str(index))
        env = dict(os.environ)
        env.pop("RUSTFLAGS", None)
        env.update({"CARGO_TARGET_DIR": str(target_dir), "SOURCE_DATE_EPOCH": str(epoch),
                    "CARGO_INCREMENTAL": "0", "RUSTUP_TOOLCHAIN": toolchain,
                    "CARGO_ENCODED_RUSTFLAGS": "--remap-path-prefix=" + str(repository) + "=/aniflow/source\x1f" +
                                              "--remap-path-prefix=" + str(target_dir) + "=/aniflow/target"})
        result = execute(["cargo", "build", "--frozen", "--release", "--bin", "aniflow", "--target", target],
                         cwd=repository, logs=work / "logs", name="build-" + str(index),
                         deadline=deadline, environment=env, budget_root=work,
                         maximum_bytes=4 * 1024 * 1024 * 1024)
        results.append({"state": result["state"], "build": index})
        require(result["state"] == "passed", "native build failed; retain work/logs")
        binary_path = target_dir / target / "release/aniflow"
        data = regular_bytes(binary_path)
        binary_identity(data, target)
        require(command([binary_path, "--version"], repository=repository).decode().strip() == "aniflow " + version[1:],
                "built CLI version differs from Cargo authority")
        require(b"Usage:" in command([binary_path, "--help"], repository=repository), "built CLI help is unavailable")
        binaries.append(data)
    require(binaries[0] == binaries[1], "clean native builds differ; no release artifact accepted")
    first, second = [archive_bytes(payload_files(data, repository)) for data in binaries]
    require(first == second, "archive recipe is not deterministic on this toolchain")
    metadata = json.loads(command(["rustup", "run", toolchain, "cargo", "metadata", "--format-version", "1",
                                   "--frozen", "--filter-platform", target], repository=repository))
    clean_revision(revision, repository)
    output = new_directory(output_path, repository)
    archive_name = "aniflow-" + version + "-" + target + ".tar.gz"
    write_new(output / archive_name, first)
    write_new(output / "build.json", json_bytes({"schema": "aniflow.release-build-evidence/v1",
        "version": version, "source_revision": revision, "target": target, "rustc": rustc, "cargo": cargo,
        "python": platform.python_version(), "system": platform.system(), "machine": platform.machine(),
        "source_date_epoch": epoch, "cargo_lock_sha256": digest(regular_bytes(repository / "Cargo.lock")),
        "archive_sha256": digest(first), "binary_sha256": digest(binaries[0]),
        "archive_members": {name: digest(value[0]) for name, value in payload_files(binaries[0], repository).items()},
        "cli_smoke": "passed", "reproducibility": {"state": "matched", "builds": results,
            "scope": "same-source, native runner and installed toolchain; two isolated Cargo target directories"}}))
    write_new(output / "sbom.spdx.json", json_bytes(spdx(metadata, version, revision, target, epoch, repository)))
    write_new(output / "SHA256SUMS", checksum_bytes(inventory(output)))
    return {"target": target, "archive_sha256": digest(first), "state": "built"}
