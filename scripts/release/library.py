"""Portable Cargo source archive over the existing pinned release owner.

Packaging is an explicit build-time operation, never invoked by import. The
reader does not extract paths or trust a supplied 'verified' flag. A source
archive is not a crates.io publication or native/installation qualification.
"""
from __future__ import annotations
import gzip
import io
import json
import os
from pathlib import Path
import tarfile
import tomllib
from .common import ROOT, command, digest, regular_bytes, require

MAX_LIBRARY = 134217728
MAX_MEMBERS = 8192


def filename(version):
    return "aniflow-" + version + ".crate"


def observe(data, version, revision, repository=ROOT):
    require(0 < len(data) <= MAX_LIBRARY, "library archive byte bound")
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as stream:
        expanded = stream.read(MAX_LIBRARY + 1)
    require(len(expanded) <= MAX_LIBRARY, "library archive expansion bound")
    tracked = set(command(["git", "-c", "core.hooksPath=/dev/null", "ls-tree", "-r", "--name-only", "-z", revision], repository=repository).decode().split("\0"))
    prefix = "aniflow-" + version.removeprefix("v") + "/"
    files = {}
    with tarfile.open(fileobj=io.BytesIO(expanded), mode="r:") as archive:
        for entry in archive:
            require(entry.isfile() and entry.name.startswith(prefix) and len(files) < MAX_MEMBERS,
                    "library contains an unsafe member or exceeds member bound")
            relative = entry.name[len(prefix):]
            path = Path(relative)
            require(relative and not path.is_absolute() and ".." not in path.parts and relative not in files,
                    "library contains escaped or duplicate paths")
            reader = archive.extractfile(entry)
            require(reader is not None and entry.size <= MAX_LIBRARY, "library member unreadable or oversized")
            content = reader.read(MAX_LIBRARY + 1)
            require(len(content) == entry.size, "library member truncated or oversized")
            if relative not in {"Cargo.toml", "Cargo.toml.orig", ".cargo_vcs_info.json"}:
                require(relative in tracked and content == regular_bytes(repository / relative),
                        "library member is not exact tracked source: " + relative)
            files[relative] = content
    required = {"src/lib.rs", "src/main.rs", "Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "LICENSE", "README.md", ".cargo_vcs_info.json"}
    require(required.issubset(files), "library archive lacks required source/authority files")
    package = tomllib.loads(files["Cargo.toml"].decode())["package"]
    require(package["name"] == "aniflow" and package["version"] == version.removeprefix("v"), "library Cargo authority differs")
    require(files["Cargo.toml.orig"] == regular_bytes(repository / "Cargo.toml"), "library original Cargo authority differs")
    vcs = json.loads(files[".cargo_vcs_info.json"])
    require(vcs.get("git", {}).get("sha1") == revision and vcs["git"].get("dirty", False) is False and vcs.get("path_in_vcs") == "",
            "library does not represent the exact clean repository root")
    require(not any(name.startswith(("target/", ".agents/", ".codex/", ".git/")) for name in files), "development state entered library archive")
    return {"schema_version": "aniflow.library-asset/v1", "name": filename(version), "version": version.removeprefix("v"),
            "source_revision": revision, "sha256": digest(data), "byte_count": len(data),
            "members": {name: digest(value) for name, value in sorted(files.items())},
            "channel": "github-release-source-archive", "registry_publication": "not_performed",
            "qualification": "source_archive_bytes_only"}


def prepare(version, revision, toolchain, work, repository=ROOT):
    archives = []
    for index in [1, 2]:
        target = work / ("library-" + str(index))
        environment = dict(os.environ)
        environment.pop("CARGO_BUILD_TARGET", None)
        environment.update({"CARGO_TARGET_DIR": str(target), "CARGO_NET_OFFLINE": "true", "RUSTUP_TOOLCHAIN": toolchain,
                            "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "core.hooksPath", "GIT_CONFIG_VALUE_0": "/dev/null"})
        command(["cargo", "package", "--frozen", "--no-verify"], repository=repository, env=environment, timeout=300)
        data = regular_bytes(target / "package" / ("aniflow-" + version.removeprefix("v") + ".crate"), MAX_LIBRARY)
        observe(data, version, revision, repository)
        archives.append(data)
    require(archives[0] == archives[1], "isolated library source archives differ")
    return archives[0], observe(archives[0], version, revision, repository)
