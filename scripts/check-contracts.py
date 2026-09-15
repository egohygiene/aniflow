#!/usr/bin/env python3
"""Verify that published contract schemas and examples remain inspectable."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
CONTRACTS_DIRECTORY = REPOSITORY_ROOT / "docs" / "contracts"
DRAFT_2020_12 = "https://json-schema.org/draft/2020-12/schema"

PUBLIC_CONTRACTS = {
    "provider-manifest-v1.schema.json": "aniflow.provider-manifest/v1",
    "provider-configuration-v1.schema.json": "aniflow.provider-configuration/v1",
    "compatibility-fingerprint-v1.schema.json": "aniflow.compatibility-fingerprint/v1",
}

PUBLIC_EXAMPLES = {
    "provider-manifest-v1.example.json": "aniflow.provider-manifest/v1",
    "provider-configuration-v1.example.json": "aniflow.provider-configuration/v1",
    "compatibility-fingerprint-v1.example.json": "aniflow.compatibility-fingerprint/v1",
}


def load_json(path: Path, errors: list[str]) -> Any | None:
    try:
        with path.open(encoding="utf-8") as source:
            return json.load(source)
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"{path.relative_to(REPOSITORY_ROOT)}: {error}")
        return None


def main() -> int:
    errors: list[str] = []
    json_paths = sorted(CONTRACTS_DIRECTORY.rglob("*.json"))

    if not json_paths:
        errors.append("docs/contracts contains no JSON documents")

    for path in json_paths:
        load_json(path, errors)

    for filename, contract_id in PUBLIC_CONTRACTS.items():
        path = CONTRACTS_DIRECTORY / filename
        document = load_json(path, errors)
        if not isinstance(document, dict):
            continue
        if document.get("$schema") != DRAFT_2020_12:
            errors.append(f"{filename}: expected JSON Schema draft 2020-12")
        if document.get("type") != "object":
            errors.append(f"{filename}: root type must be object")
        if document.get("additionalProperties") is not False:
            errors.append(f"{filename}: root must reject unknown properties")
        schema_property = document.get("properties", {}).get("schema", {})
        if schema_property.get("const") != contract_id:
            errors.append(f"{filename}: schema constant must be {contract_id}")

    examples_directory = CONTRACTS_DIRECTORY / "examples"
    for filename, contract_id in PUBLIC_EXAMPLES.items():
        path = examples_directory / filename
        document = load_json(path, errors)
        if not isinstance(document, dict):
            continue
        if document.get("schema") != contract_id:
            errors.append(f"{filename}: schema must be {contract_id}")

    if errors:
        print("Contract validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"Validated {len(json_paths)} published JSON contract documents.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
