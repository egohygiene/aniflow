"""Bounded direct process execution; capture logs and retain non-passing failures."""

from __future__ import annotations

import os
from pathlib import Path
import signal
import subprocess
import time


def tree_bytes(root, limit, excluded=()):
    total = 0
    count = 0
    for current, directories, names in os.walk(root, followlinks=False):
        if Path(current) == Path(root):
            directories[:] = [name for name in directories if name not in excluded]
        count += len(directories) + len(names)
        if count > 250000:
            raise ValueError("qualification output file-count budget exceeded")
        for name in names:
            total += (Path(current) / name).lstat().st_size
            if total > limit:
                raise ValueError("qualification output byte budget exceeded")
    return total


def stop(process):
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    elif process.poll() is None:
        process.kill()
    process.wait(timeout=2)


def execute(argv, *, cwd, logs, name, deadline, environment=None, budget_root=None, maximum_bytes=4 * 1024**3, budget_excluded=(), maximum_build_bytes=None):
    logs = Path(logs)
    logs.mkdir(parents=True, exist_ok=True)
    stdout_path, stderr_path = logs / (name + ".stdout"), logs / (name + ".stderr")
    started = time.monotonic()
    state, code, message = "failed", None, None
    process = None
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        try:
            if time.monotonic() >= deadline:
                raise TimeoutError("tier time budget exhausted before launch")
            process = subprocess.Popen([str(arg) for arg in argv], cwd=cwd, env=environment,
                                       stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                       start_new_session=os.name == "posix")
            while process.poll() is None:
                if time.monotonic() >= deadline:
                    raise TimeoutError("process exceeded time budget")
                if stdout_path.stat().st_size + stderr_path.stat().st_size > 4 * 1024**2:
                    raise ValueError("process capture limit exceeded")
                if budget_root is not None:
                    tree_bytes(budget_root, maximum_bytes, budget_excluded)
                    if maximum_build_bytes is not None:
                        tree_bytes(Path(budget_root) / "build", maximum_build_bytes)
                time.sleep(0.05)
            code = process.returncode
            state = "passed" if code == 0 else "failed"
        except FileNotFoundError as error:
            state, message = "unavailable", str(error)
        except TimeoutError as error:
            state, message = "timed_out", str(error)
        except (ValueError, OSError) as error:
            state, message = "failed", str(error)
        finally:
            if process is not None:
                stop(process)
    capture = stdout_path.stat().st_size + stderr_path.stat().st_size
    if capture > 4 * 1024**2:
        state, message = "failed", "process capture limit exceeded"
    if budget_root is not None:
        try:
            tree_bytes(budget_root, maximum_bytes, budget_excluded)
            if maximum_build_bytes is not None:
                tree_bytes(Path(budget_root) / "build", maximum_build_bytes)
        except ValueError as error:
            state, message = "failed", str(error)
    def seal(path):
        import hashlib
        checksum = hashlib.sha256()
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(65536), b""):
                checksum.update(block)
        return {"path": path.name, "bytes": path.stat().st_size, "sha256": checksum.hexdigest()}
    return {"name": name, "log_directory": "logs", "argv": [str(arg) for arg in argv], "state": state, "exit_code": code,
            "message": message, "elapsed_seconds": round(time.monotonic() - started, 3),
            "stdout": seal(stdout_path), "stderr": seal(stderr_path)}


def require_passed(result):
    if result["state"] != "passed":
        raise ValueError(result["name"] + ": " + result["state"] + "; see retained process logs")
