"""Fresh-output ownership and bounded inventory for synthetic corpus tooling."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path, PurePosixPath
import stat

MAX_FILE_BYTES = 2 * 1024 * 1024
MAX_BUNDLE_BYTES = 32 * 1024 * 1024
MAX_FILES = 4096


def digest(data):
    return hashlib.sha256(data).hexdigest()


def relative_path(value):
    path = PurePosixPath(value)
    if not value or "\\" in value or ":" in value or path.is_absolute() or any(
        part in ("", ".", "..") for part in value.split("/")
    ):
        raise ValueError("unsafe corpus-relative path: " + value)
    return path


def regular_bytes(path, maximum=MAX_FILE_BYTES):
    path = Path(path)
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError("expected a regular file: " + str(path))
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("file byte budget exceeded: " + str(path))
    return data


def safe_new_directory(path, repository):
    path = Path(path).expanduser()
    if any(part in (".", "..") for part in str(path).split(os.sep)):
        raise ValueError("output path must not contain traversal components")
    path = path.absolute()
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise ValueError("output path must not contain links")
    if path.exists() or not path.parent.is_dir():
        raise ValueError("output must be absent with an existing parent")
    if path == repository or repository in path.parents:
        raise ValueError("generated media must be outside the source checkout")
    return path


def inventory(root):
    root = Path(root)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("inventory root must be a real directory")
    files, directories, total = [], [], 0
    for current, child_dirs, names in os.walk(root, followlinks=False):
        for name in sorted(child_dirs + names):
            path = Path(current) / name
            relative = path.relative_to(root).as_posix()
            relative_path(relative)
            mode = path.lstat().st_mode
            if stat.S_ISDIR(mode):
                directories.append(relative)
            elif stat.S_ISREG(mode):
                data = regular_bytes(path)
                total += len(data)
                files.append({"path": relative, "size_bytes": len(data), "sha256": digest(data)})
            else:
                raise ValueError("links and special files are not corpus artifacts")
            if len(files) + len(directories) > MAX_FILES or total > MAX_BUNDLE_BYTES:
                raise ValueError("corpus inventory budget exceeded")
    return {"directories": sorted(directories), "files": sorted(files, key=lambda item: item["path"])}
