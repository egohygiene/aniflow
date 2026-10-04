#!/usr/bin/env python3
"""Verify that published contract schemas and examples remain inspectable."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
CONTRACTS_DIRECTORY = REPOSITORY_ROOT / "docs" / "contracts"
DRAFT_2020_12 = "https://json-schema.org/draft/2020-12/schema"

PUBLIC_CONTRACTS = {
    "toolchain-profile-v1.schema.json": "aniflow.toolchain-profile/v1",
    "toolchain-inventory-v1.schema.json": "aniflow.toolchain-inventory/v1",
    "toolchain-report-v1.schema.json": "aniflow.toolchain-report/v1",
    "cache-policy-v1.schema.json": "aniflow.cache-policy/v1",
    "cache-entry-v1.schema.json": "aniflow.cache-entry/v1",
    "cache-inspection-v1.schema.json": "aniflow.cache-inspection/v1",
    "cache-operation-v1.schema.json": "aniflow.cache-operation/v1",
    "cache-decision-v1.schema.json": "aniflow.cache-decision/v1",
    "validation-context-v1.schema.json": "aniflow.validation-context/v1",
    "validator-observation-v1.schema.json": "aniflow.validator-observation/v1",
    "validation-report-v1.schema.json": "aniflow.validation-report/v1",
    "acceptance-record-v1.schema.json": "aniflow.acceptance-record/v1",
    "temporal-inspection-v1.schema.json": "aniflow.temporal-inspection/v1",
    "temporal-artifact-index-v1.schema.json": "aniflow.temporal-artifact-index/v1",
    "segment-plan-v2.schema.json": "aniflow.segment-plan/v2",
    "segment-manifest-v2.schema.json": "aniflow.segment-manifest/v2",
    "reconstruction-report-v2.schema.json": "aniflow.reconstruction-report/v2",
    "audio-midi-export-v1.schema.json": "aniflow.audio-midi-export/v1",
    "audio-midi-preflight-v1.schema.json": "aniflow.audio-midi-preflight/v1",
    "audio-midi-probe-v1.schema.json": "aniflow.audio-midi-probe/v1",
    "audio-midi-observation-v1.schema.json": "aniflow.audio-midi-observation/v1",
    "audio-midi-v1.schema.json": "aniflow.audio-midi/v1",
    "audio-transcription-v1.schema.json": "aniflow.audio-transcription/v1",
    "audio-alignment-v1.schema.json": "aniflow.audio-alignment/v1",
    "audio-alignment-preflight-v1.schema.json": "aniflow.audio-alignment-preflight/v1",
    "audio-transcription-preflight-v1.schema.json": "aniflow.audio-transcription-preflight/v1",
    "timed-text-v1.schema.json": "aniflow.timed-text/v1",
    "timed-text-context-v1.schema.json": "aniflow.timed-text-context/v1",
    "timed-text-conversion-v1.schema.json": "aniflow.timed-text-conversion/v1",
    "timed-text-registry-v1.schema.json": "aniflow.timed-text-registry/v1",
    "audio-musical-analysis-v1.schema.json": "aniflow.audio-musical-analysis/v1",
    "audio-musical-observation-v1.schema.json": "aniflow.audio-musical-observation/v1",
    "audio-musical-probe-v1.schema.json": "aniflow.audio-musical-probe/v1",
    "audio-stem-lineage-v1.schema.json": "aniflow.audio-stem-lineage/v1",
    "audio-signal-measurements-v2.schema.json": "aniflow.audio-signal-measurements/v2",
    "audio-signal-measurements-v3.schema.json": "aniflow.audio-signal-measurements/v3",
    "audio-signal-measurements-v1.schema.json": "aniflow.audio-signal-measurements/v1",
    "audio-inspection-preflight-v1.schema.json": "aniflow.audio-inspection-preflight/v1",
    "audio-technical-inspection-v1.schema.json": "aniflow.audio-technical-inspection/v1",
    "audio-technical-inspection-v2.schema.json": "aniflow.audio-technical-inspection/v2",
    "audio-analysis-v1.schema.json": "aniflow.audio-analysis/v1",
    "demucs-separation-v1.schema.json": "aniflow.demucs-separation/v1",
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
    "provider-execution-report-v2.schema.json": "aniflow.provider-execution-report/v2",
    "provider-invocation-v1.schema.json": "aniflow.provider-invocation/v1",
    "provider-invocation-v2.schema.json": "aniflow.provider-invocation/v2",
    "pipeline-run-outcome-v1.schema.json": "aniflow.pipeline-run-outcome/v1",
    "pipeline-run-recovery-v1.schema.json": "aniflow.pipeline-run-recovery/v1",
    "pipeline-run-v1.schema.json": "aniflow.pipeline-run/v1",
    "stage-checkpoint-v1.schema.json": "aniflow.stage-checkpoint/v1",
}

# This untagged companion inherits its dialect from the versioned MIDI export.
# It is still a closed, independently published schema; adding a document tag
# would change the exact normalized observation preserved in notes.json.
PUBLIC_COMPANION_CONTRACTS = {
    "audio-midi-notes-v1.schema.json": "aniflow.audio-midi-notes/v1",
}

PUBLIC_EXAMPLES = {
    "toolchain-profile-v1.example.json": "aniflow.toolchain-profile/v1",
    "toolchain-inventory-v1.example.json": "aniflow.toolchain-inventory/v1",
    "toolchain-report-v1.example.json": "aniflow.toolchain-report/v1",
    "cache-policy-v1.example.json": "aniflow.cache-policy/v1",
    "cache-entry-v1.example.json": "aniflow.cache-entry/v1",
    "cache-inspection-v1.example.json": "aniflow.cache-inspection/v1",
    "cache-operation-v1.example.json": "aniflow.cache-operation/v1",
    "cache-decision-v1.example.json": "aniflow.cache-decision/v1",
    "validation-context-v1.example.json": "aniflow.validation-context/v1",
    "validator-observation-v1.example.json": "aniflow.validator-observation/v1",
    "validation-report-v1.example.json": "aniflow.validation-report/v1",
    "acceptance-record-v1.example.json": "aniflow.acceptance-record/v1",
    "audio-midi-export-v1.example.json": "aniflow.audio-midi-export/v1",
    "audio-midi-preflight-v1.example.json": "aniflow.audio-midi-preflight/v1",
    "audio-midi-probe-v1.example.json": "aniflow.audio-midi-probe/v1",
    "audio-midi-observation-v1.example.json": "aniflow.audio-midi-observation/v1",
    "audio-midi-v1.example.json": "aniflow.audio-midi/v1",
    "audio-transcription-v1.example.json": "aniflow.audio-transcription/v1",
    "audio-alignment-v1.example.json": "aniflow.audio-alignment/v1",
    "audio-alignment-preflight-v1.example.json": "aniflow.audio-alignment-preflight/v1",
    "audio-transcription-preflight-v1.example.json": "aniflow.audio-transcription-preflight/v1",
    "timed-text-v1.example.json": "aniflow.timed-text/v1",
    "timed-text-context-v1.example.json": "aniflow.timed-text-context/v1",
    "timed-text-conversion-v1.example.json": "aniflow.timed-text-conversion/v1",
    "timed-text-registry-v1.example.json": "aniflow.timed-text-registry/v1",
    "audio-musical-analysis-v1.example.json": "aniflow.audio-musical-analysis/v1",
    "audio-stem-lineage-v1.example.json": "aniflow.audio-stem-lineage/v1",
    "audio-signal-measurements-v2.example.json": "aniflow.audio-signal-measurements/v2",
    "audio-signal-measurements-v3-pcm24.example.json": "aniflow.audio-signal-measurements/v3",
    "audio-signal-measurements-v3-float32.example.json": "aniflow.audio-signal-measurements/v3",
    "audio-signal-measurements-v1.example.json": "aniflow.audio-signal-measurements/v1",
    "audio-technical-inspection-v2-pcm24.example.json": "aniflow.audio-technical-inspection/v2",
    "audio-technical-inspection-v2-float32.example.json": "aniflow.audio-technical-inspection/v2",
    "audio-analysis-technical-v1.example.json": "aniflow.audio-analysis/v1",
    "audio-analysis-estimated-v1.example.json": "aniflow.audio-analysis/v1",
    "audio-analysis-unavailable-v1.example.json": "aniflow.audio-analysis/v1",
    "audio-analysis-timeline-v1.example.json": "aniflow.audio-analysis/v1",
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
    "provider-execution-report-v2.example.json": "aniflow.provider-execution-report/v2",
    "provider-invocation-v1.example.json": "aniflow.provider-invocation/v1",
    "provider-invocation-v2.example.json": "aniflow.provider-invocation/v2",
    "pipeline-run-outcome-v1.example.json": "aniflow.pipeline-run-outcome/v1",
    "pipeline-run-recovery-v1.example.json": "aniflow.pipeline-run-recovery/v1",
    "pipeline-run-v1.example.json": "aniflow.pipeline-run/v1",
    "stage-checkpoint-v1.example.json": "aniflow.stage-checkpoint/v1",
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


def canonical_sha256(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def main() -> int:
    errors: list[str] = []
    contracts: dict[str, dict[str, Any]] = {}
    json_paths = sorted(CONTRACTS_DIRECTORY.rglob("*.json"))

    if not json_paths:
        errors.append("docs/contracts contains no JSON documents")

    for path in json_paths:
        load_json(path, errors)

    for filename, contract_id in {**PUBLIC_CONTRACTS, **PUBLIC_COMPANION_CONTRACTS}.items():
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
        if filename in PUBLIC_COMPANION_CONTRACTS:
            if document.get("$id") != contract_id or "schema" in document.get("properties", {}):
                errors.append(f"{filename}: untagged companion must retain schema ID {contract_id}")
        elif schema_property.get("const") != contract_id:
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
        artifact = document.get("$defs", {}).get(definition, {})
        properties = artifact.get("properties", {})
        pattern = properties.get("relative_path", {}).get("pattern")
        if isinstance(pattern, str):
            check_v3_path_pattern(
                filename,
                pattern,
                "artifacts/stage/output.bin",
                unsafe_artifact_paths,
                errors,
            )
        if "kind" in artifact.get("required", []):
            errors.append(
                f"{filename}: expected artifact kind must remain optional for planning compatibility"
            )
        if properties.get("kind", {}).get("enum") != ["file", "directory"]:
            errors.append(
                f"{filename}: expected artifact kind must enumerate file and directory"
            )

    for filename in [
        "pipeline-run-v1.schema.json",
        "stage-checkpoint-v1.schema.json",
    ]:
        document = contracts.get(filename, {})
        definitions = document.get("$defs", {})
        canonicalization = document.get("properties", {}).get(
            "canonicalization", {}
        )
        if canonicalization.get("const") != "aniflow.canonical-json/v1":
            errors.append(
                f"{filename}: canonicalization must be aniflow.canonical-json/v1"
            )
        artifact_pattern = (
            definitions.get("artifactEvidence", {})
            .get("properties", {})
            .get("relative_path", {})
            .get("$ref")
        )
        if artifact_pattern != "#/$defs/artifactPath":
            errors.append(f"{filename}: artifact evidence must use the confined artifact path")
        portable_pattern = definitions.get("portableRelativePath", {}).get("pattern")
        if isinstance(portable_pattern, str):
            check_v3_path_pattern(
                filename,
                portable_pattern,
                "providers/enhance-8a262138003ebe3f600f6ebb83b7d0686109508a9aece27532c4a7bae8fe5129.report.json",
                [
                    "../escape.json",
                    "/absolute.json",
                    "providers//report.json",
                    "providers/CON",
                    "providers/report?.json",
                ],
                errors,
            )

    invocation = contracts.get("provider-invocation-v1.schema.json", {})
    invocation_properties = invocation.get("properties", {})
    if invocation_properties.get("execution_semantics", {}).get("const") != (
        "aniflow.provider-invocation/direct-argv/v1"
    ):
        errors.append(
            "provider-invocation-v1.schema.json: execution semantics must be fixed to direct-argv v1"
        )
    absolute_path_pattern = (
        invocation.get("$defs", {}).get("absolutePath", {}).get("pattern")
    )
    if isinstance(absolute_path_pattern, str):
        check_pattern_samples(
            "provider-invocation-v1.schema.json",
            "absolute-path",
            absolute_path_pattern,
            "/var/lib/aniflow/source/frames",
            ["relative/path", "../escape", "C:relative", "/unsafe\npath"],
            errors,
        )
    normalized_path_exclusion = (
        invocation.get("$defs", {})
        .get("absolutePath", {})
        .get("not", {})
        .get("pattern")
    )
    if isinstance(normalized_path_exclusion, str):
        try:
            compiled_exclusion = re.compile(normalized_path_exclusion)
        except re.error:
            pass
        else:
            if compiled_exclusion.search("/var/lib/aniflow/../escape.json") is None:
                errors.append(
                    "provider-invocation-v1.schema.json: absolute-path exclusion accepts a parent component"
                )
            if compiled_exclusion.search("/var/lib/aniflow/source/frames") is not None:
                errors.append(
                    "provider-invocation-v1.schema.json: absolute-path exclusion rejects a normalized path"
                )
    else:
        errors.append(
            "provider-invocation-v1.schema.json: absolute paths must exclude dot components"
        )

    artifact_set_invocation = contracts.get("provider-invocation-v2.schema.json", {})
    artifact_set_properties = artifact_set_invocation.get("properties", {})
    if artifact_set_properties.get("execution_semantics", {}).get("const") != (
        "aniflow.provider-invocation/direct-argv/v2"
    ):
        errors.append("provider-invocation-v2.schema.json: execution semantics must be direct-argv v2")
    if artifact_set_properties.get("outputs", {}).get("minItems") != 1:
        errors.append("provider-invocation-v2.schema.json: exact output sets must be nonempty")
    if artifact_set_invocation.get("$defs", {}).get("absolutePath") != invocation.get("$defs", {}).get("absolutePath"):
        errors.append("provider-invocation-v2.schema.json: preserve the normalized absolute-path boundary")

    recovery = contracts.get("pipeline-run-recovery-v1.schema.json", {})
    recovery_required = set(recovery.get("required", []))
    if recovery_required != {"schema", "run_directory"}:
        errors.append(
            "pipeline-run-recovery-v1.schema.json: only schema and run_directory must be required"
        )
    recovery_properties = recovery.get("properties", {})
    for field in ["run_directory", "run_manifest"]:
        if recovery_properties.get(field, {}).get("$ref") != "#/$defs/absolutePath":
            errors.append(
                f"pipeline-run-recovery-v1.schema.json: {field} must use the absolute-path definition"
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
        "pipeline-run-v1.schema.json",
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

    provider_manifest = contracts.get("provider-manifest-v1.schema.json", {})
    provenance_required = (
        provider_manifest.get("properties", {})
        .get("provenance", {})
        .get("properties", {})
        .get("required", {})
    )
    provenance_fields = provenance_required.get("items", {}).get("enum", [])
    if (
        len(provenance_fields) != 10
        or provenance_required.get("minItems") != len(provenance_fields)
        or provenance_required.get("maxItems") != len(provenance_fields)
    ):
        errors.append(
            "provider-manifest-v1.schema.json: provenance must require all 10 declared fields"
        )

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

    manifest_example = load_json(
        examples_directory / "provider-manifest-v1.example.json", errors
    )
    invocation_example = load_json(
        examples_directory / "provider-invocation-v1.example.json", errors
    )
    configuration_example = load_json(
        examples_directory / "provider-configuration-v1.example.json", errors
    )
    if (
        isinstance(manifest_example, dict)
        and isinstance(invocation_example, dict)
        and isinstance(configuration_example, dict)
    ):
        invocation_configuration = invocation_example.get("configuration", {})
        if invocation_configuration != configuration_example:
            errors.append(
                "provider-invocation-v1.example.json: configuration must match "
                "provider-configuration-v1.example.json"
            )
        manifest_provider = manifest_example.get("provider", {})
        if invocation_configuration.get("provider") != {
            "id": manifest_provider.get("id"),
            "version": manifest_provider.get("version"),
        }:
            errors.append(
                "provider-invocation-v1.example.json: provider identity must match the published manifest"
            )

        capability_reference = invocation_configuration.get("capability", {})
        capability = next(
            (
                candidate
                for candidate in manifest_example.get("capabilities", [])
                if candidate.get("id") == capability_reference.get("id")
                and candidate.get("version") == capability_reference.get("version")
            ),
            None,
        )
        if not isinstance(capability, dict):
            errors.append(
                "provider-invocation-v1.example.json: capability must exist in the published manifest"
            )
        else:
            for direction in ["inputs", "outputs"]:
                declared_ports = {
                    port.get("name"): port
                    for port in capability.get(direction, [])
                    if isinstance(port, dict)
                }
                for binding in invocation_example.get(direction, []):
                    declared = declared_ports.get(binding.get("port"))
                    if not isinstance(declared, dict):
                        errors.append(
                            f"provider-invocation-v1.example.json: {direction} port {binding.get('port')!r} "
                            "must exist in the published capability"
                        )
                        continue
                    for field in ["artifact_type", "artifact_role", "stream_role"]:
                        if binding.get(field) != declared.get(field):
                            errors.append(
                                f"provider-invocation-v1.example.json: {direction} port "
                                f"{binding.get('port')!r} {field} must match the published capability"
                            )

    for version in (1, 2):
        filename = f"provider-execution-report-v{version}.example.json"
        document = load_json(examples_directory / filename, errors)
        if not isinstance(document, dict) or not isinstance(document.get("payload"), dict):
            continue
        subject = document["payload"] if version == 1 else {
            "schema": document.get("schema"), "payload": document["payload"]
        }
        expected = canonical_sha256(subject)
        if document.get("report_sha256") != expected:
            errors.append(f"{filename}: report_sha256 does not bind the versioned canonical subject; expected {expected}")

    for filename, digest_field in [
        ("pipeline-run-v1.example.json", "manifest_sha256"),
        ("stage-checkpoint-v1.example.json", "checkpoint_sha256"),
    ]:
        document = load_json(examples_directory / filename, errors)
        if not isinstance(document, dict) or not isinstance(document.get("payload"), dict):
            continue
        expected = canonical_sha256(document["payload"])
        if document.get(digest_field) != expected:
            errors.append(
                f"{filename}: {digest_field} must cover the canonical payload; expected {expected}"
            )

    configuration_example = load_json(
        examples_directory / "pipeline-v3-configuration-v1.example.json", errors
    )
    plan_example = load_json(
        examples_directory / "pipeline-v3-plan-v1.example.json", errors
    )
    if isinstance(configuration_example, dict) and isinstance(plan_example, dict):
        semantic_configuration = {
            key: configuration_example[key]
            for key in ["schema", "name", "inputs", "stages", "outputs"]
            if key in configuration_example
        }
        configuration_sha256 = canonical_sha256(semantic_configuration)
        payload = plan_example.get("payload")
        if not isinstance(payload, dict):
            errors.append("pipeline-v3-plan-v1.example.json: payload must be an object")
        else:
            if payload.get("configuration_sha256") != configuration_sha256:
                errors.append(
                    "pipeline-v3-plan-v1.example.json: configuration_sha256 must identify the published configuration; "
                    f"expected {configuration_sha256}"
                )
            plan_sha256 = canonical_sha256(payload)
            if plan_example.get("plan_sha256") != plan_sha256:
                errors.append(
                    "pipeline-v3-plan-v1.example.json: plan_sha256 must cover the canonical payload; "
                    f"expected {plan_sha256}"
                )

    if errors:
        print("Contract validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"Validated {len(json_paths)} published JSON contract documents.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
