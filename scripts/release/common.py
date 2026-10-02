"""Bounded file and direct-command helpers; no publication authority."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "egohygiene/aniflow"
TAG = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z")
SHA = re.compile(r"[0-9a-f]{40}\Z")
MAX_FILE = 256 * 1024 * 1024
MAX_TOTAL = 512 * 1024 * 1024
MAX_FILES = 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def no_links(path):
    path = Path(path).absolute()
    require(not any(item.is_symlink() for item in [path, *path.parents]),
            "path must not traverse a symlink: " + str(path))
    return path


def regular_bytes(path, maximum=MAX_FILE):
    path = no_links(path)
    info = path.stat()
    require(stat.S_ISREG(info.st_mode) and info.st_size <= maximum,
            "expected a bounded regular file: " + str(path))
    with path.open("rb") as handle:
        data = handle.read(maximum + 1)
    require(len(data) <= maximum, "file grew beyond the byte budget")
    return data


def digest(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode("utf-8")


def read_json(path):
    return json.loads(regular_bytes(path, 16 * 1024 * 1024))


def write_new(path, data):
    path = no_links(path)
    with path.open("xb") as handle:
        handle.write(data)


def new_directory(path, repository=ROOT):
    path = no_links(path)
    require(not path.is_relative_to(repository.resolve()), "output must be outside the checkout")
    require(path.parent.is_dir(), "output parent must already exist")
    path.mkdir()  # Exclusive ownership; never adopt or clean another directory.
    return path


def inventory(directory):
    directory = no_links(directory)
    require(directory.is_dir(), "bundle directory is missing")
    records = {}
    total = 0
    for path in sorted(directory.rglob("*")):
        no_links(path)
        if path.is_dir():
            continue
        name = path.relative_to(directory).as_posix()
        require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]*", name) is not None,
                "unsupported artifact path")
        data = regular_bytes(path)
        total += len(data)
        records[name] = {"sha256": digest(data), "bytes": len(data)}
        require(total <= MAX_TOTAL and len(records) <= MAX_FILES, "bundle exceeds budget")
    return records


def checksum_bytes(records):
    return "".join(records[name]["sha256"] + "  " + name + "\n"
                   for name in sorted(records) if name != "SHA256SUMS").encode("utf-8")


def verify_inventory(directory):
    records = inventory(directory)
    require(regular_bytes(directory / "SHA256SUMS") == checksum_bytes(records),
            "SHA256SUMS must cover every file exactly once with matching bytes")
    return records


def command(arguments, *, repository=ROOT, timeout=120, env=None):
    result = subprocess.run([str(arg) for arg in arguments], cwd=repository,
                            env=env, check=False, capture_output=True, timeout=timeout)
    require(len(result.stdout) + len(result.stderr) <= 16 * 1024 * 1024,
            "command capture exceeds budget")
    require(result.returncode == 0, "command failed: " + str(arguments[0]) + "\n" +
            result.stderr[-4000:].decode("utf-8", errors="replace"))
    return result.stdout


def git(*arguments, repository=ROOT):
    return command(["git", *arguments], repository=repository).decode("utf-8").strip()


def clean_revision(expected=None, repository=ROOT):
    revision = git("rev-parse", "HEAD^{commit}", repository=repository)
    require(SHA.fullmatch(revision) is not None, "source revision must be a full Git SHA")
    if expected is not None:
        require(revision == expected, "expected source does not match HEAD")
    require(not git("status", "--porcelain", "--untracked-files=all", repository=repository),
            "release operation requires a clean checkout")
    return revision
