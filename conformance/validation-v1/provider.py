#!/usr/bin/env python3
"""Synthetic protocol fixture. It does not inspect or qualify real media."""
import hashlib
import json
from pathlib import Path
import shutil
import sys
import time

if len(sys.argv) != 3 or sys.argv[1] != "--aniflow-invocation":
    raise SystemExit(64)
request = json.loads(Path(sys.argv[2]).read_text())
mode = request["configuration"]["values"]["mode"]
output = Path(request["outputs"][0]["path"])
output.parent.mkdir(parents=True, exist_ok=True)
if mode == "copy":
    shutil.copyfile(request["inputs"][0]["path"], output)
    raise SystemExit(0)

counter = Path(__file__).resolve().with_name("validator-launch-count")
counter.write_text(str(int(counter.read_text()) + 1 if counter.exists() else 1))
inputs = {binding["port"]: binding for binding in request["inputs"]}
context = json.loads(Path(inputs["context"]["path"]).read_text())
if mode == "nonzero":
    raise SystemExit(17)
if mode == "missing":
    raise SystemExit(0)
if mode == "slow":
    time.sleep(10)
if mode == "malformed":
    output.write_text("{}")
    raise SystemExit(0)
if mode == "mutate-artifact":
    Path(inputs["artifact"]["path"]).write_bytes(b"changed candidate")

canonical = json.dumps(context, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
report = {
    "schema": "aniflow.validator-observation/v1",
    "context_sha256": hashlib.sha256(canonical.encode()).hexdigest(),
    "artifact_sha256": context["artifact"]["sha256"],
    "disposition": mode if mode in {"partial", "failed", "skipped", "unavailable"} else "passed",
    "checks": [
        {"criterion": criterion, "disposition": "passed"}
        for criterion in ["artifact_identity", "source_lineage", "provenance"]
    ],
}
if mode == "duplicate":
    report["checks"].append(report["checks"][0])
if mode == "stale-context":
    report["context_sha256"] = "f" * 64
if mode == "wrong-artifact":
    report["artifact_sha256"] = "f" * 64
if mode == "contradictory":
    report["checks"][0]["disposition"] = "failed"
output.write_text(json.dumps(report, sort_keys=True, separators=(",", ":")))
