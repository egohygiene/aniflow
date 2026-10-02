#!/usr/bin/env python3
"""Author, generate or qualify the bounded aniflow adversarial corpus."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True

from corpus.catalog import CATALOG, catalog_bytes, check_catalog, generate, verify_bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    catalog = commands.add_parser("catalog", help="author a catalog or check drift without rewriting it")
    modes = catalog.add_mutually_exclusive_group(required=True)
    modes.add_argument("--write", action="store_true", help="explicitly regenerate the reviewed catalog; does not run tests")
    modes.add_argument("--check", action="store_true", help="read-only recipe and source-binding drift check")
    materialize = commands.add_parser("generate", help="generate bytes into an absent directory outside the checkout")
    materialize.add_argument("--output", type=Path, required=True)
    verify = commands.add_parser("verify", help="read-only exact inventory verification")
    verify.add_argument("--directory", type=Path, required=True)
    qualify = commands.add_parser("qualify", help="explicit test execution; never installs dependencies")
    qualify.add_argument("--tier", choices=["pr", "scheduled", "extended"], default="pr")
    qualify.add_argument("--output", type=Path, required=True)
    qualify.add_argument("--ffmpeg", type=Path)
    qualify.add_argument("--ffprobe", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "catalog":
            if args.write:
                data = catalog_bytes()
                CATALOG.parent.mkdir(parents=True, exist_ok=True)
                if CATALOG.is_symlink():
                    raise ValueError("refuse catalog symlink")
                CATALOG.write_bytes(data)
                result = json.loads(data)
                print(json.dumps({"state": "authored_not_run", "cases": len(result["cases"]), "catalog": str(CATALOG)}))
            else:
                result = check_catalog()
                print(json.dumps({"state": "catalog_matches", "cases": len(result["cases"]), "qualification": "not_executed"}))
        elif args.command == "generate":
            result = generate(args.output)
            print(json.dumps({"state": "generated", "cases": len(result["cases"]), "qualification": "not_executed"}))
        elif args.command == "verify":
            result = verify_bundle(args.directory)
            print(json.dumps({"state": "inventory_matches", "cases": len(result["cases"]), "qualification": "not_executed"}))
        else:
            from corpus.runner import qualify
            return qualify(args)
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(json.dumps({"schema": "aniflow.corpus-error/v1", "code": "corpus_refused", "message": str(error)}), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
