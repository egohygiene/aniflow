"""Qualification receipts distinguish authored expectations from executed evidence."""

from __future__ import annotations

import json
import os
from pathlib import Path
import platform
import shutil
import sys
import time

from .catalog import ROOT, check_catalog, generate, verify_bundle
from .native import qualify_native, tool_identity
from .process import execute, require_passed, tree_bytes
from .recipes import json_bytes
from .storage import digest, regular_bytes, safe_new_directory

TARGETS = ["adversarial_corpus", "temporal_contract", "timed_text", "provider_runtime",
           "provider_invocation_v3", "provider_conformance", "pipeline_v3_execution",
           "pipeline_v3_state_contracts", "layered_validation", "validation_contracts", "cache_v3", "cache_cli"]


def qualify(args):
    output = safe_new_directory(args.output, ROOT)
    catalog = check_catalog()
    output.mkdir()
    budget = catalog["tiers"][args.tier]
    deadline = time.monotonic() + budget["timeout_seconds"]
    steps = []
    report = {"schema": "aniflow.corpus-qualification/v1", "tier": args.tier, "state": "failed",
              "catalog_sha256": digest(json_bytes(catalog)), "environment": {"python": sys.version, "platform": platform.platform()},
              "steps": steps, "native_tools": {}, "native_cases": [], "failure": None,
              "scope": "Only executed suites/cases; no model, release or universal codec qualification."}
    bundle = output / "fixtures"
    env = os.environ.copy()
    env.update({"CARGO_NET_OFFLINE": "true", "CARGO_TARGET_DIR": str(output / "build"),
                "PYTHONDONTWRITEBYTECODE": "1", "ANIFLOW_CORPUS_PYTHON": sys.executable})
    try:
        if os.name != "posix":
            report["state"] = "unavailable"
            raise ValueError("this qualification tier requires Unix process-group termination; portable recipe generation remains available")
        generate(bundle)
        verify_bundle(bundle)
        if args.tier != "pr":
            if args.ffmpeg is None or args.ffprobe is None:
                report["state"] = "unavailable"
                raise ValueError("native tier requires explicit --ffmpeg and --ffprobe paths")
            for name in ("ffmpeg", "ffprobe"):
                report["native_tools"][name] = tool_identity(getattr(args, name), name, output, deadline, steps)
            binaries = output / "tools"
            binaries.mkdir()
            for name, identity in report["native_tools"].items():
                (binaries / name).symlink_to(identity["resolved_path"])
            env["PATH"] = str(binaries) + os.pathsep + env.get("PATH", "")
        cargo = shutil.which("cargo")
        if cargo is None:
            report["state"] = "unavailable"
            raise ValueError("cargo is unavailable; no dependency installation attempted")
        for name, command in [("source-revision", ["git", "rev-parse", "HEAD"]),
                              ("source-status", ["git", "status", "--porcelain"]),
                              ("rustc-version", ["rustc", "--version", "--verbose"]),
                              ("cargo-version", [cargo, "--version"])]:
            step = execute(command, cwd=ROOT, logs=output / "logs", name=name,
                           deadline=min(deadline, time.monotonic() + 10), environment=env)
            steps.append(step)
            require_passed(step)
            report["environment"][name] = regular_bytes(output / "logs" / step["stdout"]["path"]).decode("utf-8").strip()
        commands = [
            [sys.executable, "-B", "tests/corpus_tools.py"],
            [sys.executable, "-B", "tests/corpus_schema.py"],
            [cargo, "test", "--locked", "--offline"] + [arg for target in TARGETS for arg in ("--test", target)],
        ]
        if args.tier != "pr":
            commands.append([cargo, "test", "--locked", "--offline", "--test", "temporal_native", "--test", "corpus_native"])
        if args.tier == "extended":
            commands += [[cargo, "test", "--locked", "--offline", "--all-targets"],
                         [sys.executable, "-B", "scripts/check-contracts.py"]]
        for number, command in enumerate(commands):
            step = execute(command, cwd=ROOT, logs=output / "logs", name=f"suite-{number:02}",
                           deadline=deadline, environment=env, budget_root=output,
                           maximum_bytes=budget["maximum_output_bytes"], budget_excluded=("build",),
                           maximum_build_bytes=budget["maximum_build_bytes"])
            steps.append(step)
            require_passed(step)
        if args.tier != "pr":
            native = output / "native"
            native.mkdir()
            first_native_step = len(steps)
            try:
                native_tools, report["native_cases"] = qualify_native(
                    catalog, bundle, native, args.ffmpeg, args.ffprobe, deadline, steps)
            finally:
                for step in steps[first_native_step:]:
                    step["log_directory"] = "native/logs"
            if native_tools != report["native_tools"]:
                raise ValueError("native identity changed between suite and corpus execution")
        verify_bundle(bundle)
        check_catalog()
        tree_bytes(output, budget["maximum_output_bytes"], excluded=("build",))
        tree_bytes(output / "build", budget["maximum_build_bytes"])
        for identity in report["native_tools"].values():
            if digest(regular_bytes(identity["resolved_path"], 256 * 1024 * 1024)) != identity["sha256"]:
                raise ValueError("native executable changed during qualification")
        report["state"] = "passed"
    except (ValueError, OSError, KeyError, TypeError) as error:
        report["failure"] = str(error)
        if any(step["state"] == "unavailable" for step in steps):
            report["state"] = "unavailable"
    finally:
        (output / "receipt.json").write_bytes(json_bytes(report))
    print(json.dumps({"state": report["state"], "receipt": str(output / "receipt.json")}))
    return 0 if report["state"] == "passed" else 1
