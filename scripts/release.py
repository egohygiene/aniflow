#!/usr/bin/env python3
"""Explicit release handoffs; only the remote Relay workflow publishes."""

import argparse
import json
from pathlib import Path
import re
import sys

from release import bundle, plan
from release.common import REPOSITORY, require


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="operation", required=True)
    commands.add_parser("check")
    commands.add_parser("plan")
    prepare = commands.add_parser("prepare")
    prepare.add_argument("--version", required=True)
    prepare.add_argument("--date", required=True)
    for name in ["verify", "guard", "build", "assemble", "seal", "publish"]:
        child = commands.add_parser(name)
        child.add_argument("--version", required=True)
        child.add_argument("--source-revision", required=True)
        if name in {"verify", "seal"}:
            child.add_argument("--bundle", type=Path, required=name == "seal")
        if name == "seal":
            child.add_argument("--attestation", type=Path, required=True)
        if name == "build":
            child.add_argument("--target", required=True)
            child.add_argument("--work-directory", type=Path, required=True)
            child.add_argument("--output", type=Path, required=True)
        if name == "assemble":
            child.add_argument("--parts", type=Path, required=True)
            child.add_argument("--output", type=Path, required=True)
        if name in {"guard", "publish"}:
            child.add_argument("--candidate-run", required=name == "publish")
    args = parser.parse_args()
    try:
        if args.operation in {"check", "plan"}:
            result = plan.check()
        elif args.operation == "prepare":
            result = plan.prepare(args.version, args.date)
        elif args.operation == "verify":
            result = plan.verify_candidate(args.version, args.source_revision)
            if args.bundle:
                result = bundle.verify_bundle(args.bundle, args.version, args.source_revision)
        elif args.operation == "guard":
            from release.remote import guard
            result = guard(args.version, args.source_revision, args.candidate_run)
        elif args.operation == "build":
            from release.build import build
            result = build(args.target, args.output, args.work_directory, args.version, args.source_revision)
        elif args.operation == "assemble":
            result = bundle.assemble(args.parts, args.output, args.version, args.source_revision)
        elif args.operation == "seal":
            plan.verify_candidate(args.version, args.source_revision)
            result = bundle.seal(args.bundle, args.attestation, args.version, args.source_revision)
        else:
            plan.verify_candidate(args.version, args.source_revision)
            require(re.fullmatch(r"[1-9][0-9]{0,19}", args.candidate_run) is not None, "invalid candidate run ID")
            result = {"state": "handoff-only", "dispatched": False,
                "review": "Inspect the successful candidate run and its retained signed bundle before explicitly dispatching.",
                "argv": ["gh", "workflow", "run", "release.yml", "--repo", REPOSITORY, "--ref", "main",
                         "--field", "version=" + args.version, "--field", "source_revision=" + args.source_revision,
                         "--field", "candidate_run_id=" + args.candidate_run, "--field", "reviewed=true"]}
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except Exception as error:
        # Keep missing schema/tool dependencies and invalid inputs non-passing.
        print(json.dumps({"state": "failed", "operation": args.operation, "error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
