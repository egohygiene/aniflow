#!/usr/bin/env python3
"""Verify that published contract schemas and examples remain inspectable."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
CONTRACTS_DIRECTORY = REPOSITORY_ROOT / "docs" / "contracts"
DRAFT_2020_12 = "https://json-schema.org/draft/2020-12/schema"

PUBLIC_CONTRACTS = {
    "pipeline-v3-configuration-v1.schema.json": "aniflow.pipeline/v3",
    "pipeline-v3-plan-v1.schema.json": "aniflow.pipeline-plan/v1",
    "pipeline-v3-planning-failure-v1.schema.json": "aniflow.pipeline-planning-failure/v1",
    "provider-registration-v1.schema.json": "aniflow.provider-registration/v1",
    "provider-manifest-v1.schema.json": "aniflow.provider-manifest/v1",
    "provider-configuration-v1.schema.json": "aniflow.provider-configuration/v1",
    "compatibility-fingerprint-v1.schema.json": "aniflow.compatibility-fingerprint/v1",
    "provider-lock-v1.schema.json": "aniflow.provider-lock/v1",
    "provider-event-v1.schema.json": "aniflow.provider-event/v1",
    "provider-execution-report-v1.schema.json": "aniflow.provider-execution-report/v1",
}

PUBLIC_EXAMPLES = {
    "pipeline-v3-configuration-v1.example.json": "aniflow.pipeline/v3",
    "pipeline-v3-plan-v1.example.json": "aniflow.pipeline-plan/v1",
    "pipeline-v3-planning-failure-v1.example.json": "aniflow.pipeline-planning-failure/v1",
    "provider-registration-v1.example.json": "aniflow.provider-registration/v1",
    "provider-manifest-v1.example.json": "aniflow.provider-manifest/v1",
    "provider-configuration-v1.example.json": "aniflow.provider-configuration/v1",
    "compatibility-fingerprint-v1.example.json": "aniflow.compatibility-fingerprint/v1",
    "provider-lock-v1.example.json": "aniflow.provider-lock/v1",
    "provider-event-v1.example.json": "aniflow.provider-event/v1",
    "provider-execution-report-v1.example.json": "aniflow.provider-execution-report/v1",
}


def load_json(path: Path, errors: list[str]) -> Any | None:
    try:
        with path.open(encoding="utf-8") as source:
            return json.load(source)
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"{path.relative_to(REPOSITORY_ROOT)}: {error}")
        return None


def walk_schema(node: Any):
    """Yield every nested JSON Schema node."""
    if isinstance(node, dict):
        yield node
        for value in node.values():
            yield from walk_schema(value)
    elif isinstance(node, list):
        for value in node:
            yield from walk_schema(value)


def check_schema_internals(filename: str, document: dict[str, Any], errors: list[str]) -> None:
    definitions = document.get("$defs", {})
    for node in walk_schema(document):
        reference = node.get("$ref")
        if isinstance(reference, str) and reference.startswith("#/$defs/"):
            name = reference.removeprefix("#/$defs/")
            if name not in definitions:
                errors.append(f"{filename}: unresolved local definition {reference}")
        pattern = node.get("pattern")
        if isinstance(pattern, str):
            try:
                re.compile(pattern)
            except re.error as error:
                errors.append(f"{filename}: invalid regular expression {pattern!r}: {error}")
        required = node.get("required")
        properties = node.get("properties")
        if isinstance(required, list):
            if len(required) != len(set(required)):
                errors.append(f"{filename}: required property list contains duplicates")
            if isinstance(properties, dict):
                for name in required:
                    if name not in properties:
                        errors.append(f"{filename}: required property {name!r} is undeclared")


def check_v3_path_pattern(
    filename: str,
    pattern: str,
    valid: str,
    invalid: list[str],
    errors: list[str],
) -> None:
    check_pattern_samples(filename, "portable-path", pattern, valid, invalid, errors)


def check_pattern_samples(
    filename: str,
    label: str,
    pattern: str,
    valid: str,
    invalid: list[str],
    errors: list[str],
) -> None:
    try:
        compiled = re.compile(pattern)
    except re.error:
        return
    if compiled.fullmatch(valid) is None:
        errors.append(f"{filename}: {label} pattern rejects {valid!r}")
    for value in invalid:
        if compiled.fullmatch(value) is not None:
            errors.append(f"{filename}: {label} pattern accepts invalid {value!r}")


def main() -> int:
    errors: list[str] = []
    contracts: dict[str, dict[str, Any]] = {}
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
        contracts[filename] = document
        check_schema_internals(filename, document, errors)

    unsafe_artifact_paths = [
        "artifacts/../escape",
        "artifacts/./output",
        "artifacts//output",
        "artifacts/CON",
        "artifacts/con.txt",
        "artifacts/output.",
        "artifacts/output ",
        "artifacts/output?.bin",
        "artifacts/control\u0085.bin",
    ]
    for filename, definition in [
        ("pipeline-v3-configuration-v1.schema.json", "expectedArtifact"),
        ("pipeline-v3-plan-v1.schema.json", "plannedExpectedArtifact"),
    ]:
        document = contracts.get(filename, {})
        pattern = (
            document.get("$defs", {})
            .get(definition, {})
            .get("properties", {})
            .get("relative_path", {})
            .get("pattern")
        )
        if isinstance(pattern, str):
            check_v3_path_pattern(
                filename,
                pattern,
                "artifacts/stage/output.bin",
                unsafe_artifact_paths,
                errors,
            )

    registration = contracts.get("provider-registration-v1.schema.json", {})
    locator_pattern = (
        registration.get("$defs", {}).get("portableLocator", {}).get("pattern")
    )
    if isinstance(locator_pattern, str):
        check_v3_path_pattern(
            "provider-registration-v1.schema.json",
            locator_pattern,
            "bin/provider",
            [value.removeprefix("artifacts/") for value in unsafe_artifact_paths],
            errors,
        )

    configuration = contracts.get("pipeline-v3-configuration-v1.schema.json", {})
    final_output_required = (
        configuration.get("$defs", {}).get("finalOutput", {}).get("required", [])
    )
    if "required_validations" not in final_output_required:
        errors.append(
            "pipeline-v3-configuration-v1.schema.json: final outputs must require required_validations"
        )

    for filename in [
        "pipeline-v3-configuration-v1.schema.json",
        "pipeline-v3-plan-v1.schema.json",
        "pipeline-v3-planning-failure-v1.schema.json",
    ]:
        printable_pattern = (
            contracts.get(filename, {})
            .get("$defs", {})
            .get("printableText", {})
            .get("pattern")
        )
        if isinstance(printable_pattern, str):
            check_pattern_samples(
                filename,
                "printable-text",
                printable_pattern,
                "safe diagnostic detail",
                ["", "   ", "unsafe\x1b[31m", "line\nbreak", "unsafe\u0085detail"],
                errors,
            )

    semantic_version_patterns: set[str] = set()
    for filename, document in contracts.items():
        semantic_version_pattern = (
            document.get("$defs", {}).get("semanticVersion", {}).get("pattern")
        )
        if isinstance(semantic_version_pattern, str):
            semantic_version_patterns.add(semantic_version_pattern)
            check_pattern_samples(
                filename,
                "semantic-version",
                semantic_version_pattern,
                "1.2.3-alpha.1+build.5",
                ["1.2", "01.2.3", "1.2.3-01", "1.2.3-unsafe\x1b"],
                errors,
            )
    if len(semantic_version_patterns) != 1:
        errors.append("published schemas must share one semantic-version pattern")

    failure = contracts.get("pipeline-v3-planning-failure-v1.schema.json", {})
    failure_definitions = failure.get("$defs", {})
    attempt_properties = failure_definitions.get("resolutionAttempt", {}).get(
        "properties", {}
    )
    if attempt_properties.get("available", {}).get("const") is not False:
        errors.append(
            "pipeline-v3-planning-failure-v1.schema.json: failed resolution attempts must require available=false"
        )
    if attempt_properties.get("reasons", {}).get("minItems") != 1:
        errors.append(
            "pipeline-v3-planning-failure-v1.schema.json: failed resolution attempts must require at least one reason"
        )
    diagnostic = failure_definitions.get("diagnostic", {})
    if not diagnostic.get("allOf"):
        errors.append(
            "pipeline-v3-planning-failure-v1.schema.json: diagnostics must condition provider_unavailable evidence"
        )

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
