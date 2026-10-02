"""Retain a bounded workflow outcome even when preparation fails early."""

import argparse
import json
import os
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--stage", choices=["authorize", "native", "seal", "review"], required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
record = {"schema": "aniflow.release-workflow-outcome/v1", "stage": args.stage,
          "job_status": os.environ.get("RELEASE_JOB_STATUS", "unknown"),
          "requested_source_revision": os.environ.get("RELEASE_SOURCE", "")[:64],
          "requested_version": os.environ.get("RELEASE_VERSION", "")[:64],
          "run_id": os.environ.get("GITHUB_RUN_ID", ""),
          "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", ""),
          "publication": "not-performed-by-this-job"}
with args.output.open("x", encoding="utf-8") as handle:
    json.dump(record, handle, indent=2, sort_keys=True)
    handle.write("\n")
