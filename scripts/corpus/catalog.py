"""Compile source-owned recipes and existing coverage into a reviewable catalog."""

from __future__ import annotations

import json
from pathlib import Path
import re
import tempfile

from .recipes import ROOT, SEED, VERSION, json_bytes, recipes
from .profiles import NATIVE_PROFILES
from .storage import MAX_BUNDLE_BYTES, MAX_FILE_BYTES, digest, inventory, regular_bytes, relative_path, safe_new_directory

CATALOG = ROOT / "conformance/temporal-v1/catalog.json"
SUITES = {
    "provider_runtime": ["provider_runtime", "provider_invocation_v3", "provider_runtime_contracts", "provider_conformance", "pipeline_v2_provider_runtime"],
    "pipeline_recovery": ["pipeline_v3_execution", "pipeline_v3_state_contracts"],
    "layered_acceptance": ["layered_validation", "validation_contracts"],
    "cache_operations": ["cache_v3", "cache_cli"],
    "temporal_media": ["temporal_contract", "temporal_native"],
    "audio_text": ["timed_text", "audio_inspection", "audio_signal", "audio_stem", "audio_musical", "audio_transcription", "audio_alignment", "audio_midi"],
    "corpus_contracts": ["adversarial_corpus", "corpus_native"],
}


def source_record(path):
    relative_path(path)
    target = ROOT / path
    if any(part.is_symlink() for part in [target, *target.parents] if part != ROOT.parent):
        raise ValueError("source reference must not traverse a link")
    data = regular_bytes(target)
    return {"path": path, "size_bytes": len(data), "sha256": digest(data)}


def describe(recipe):
    total = 0
    files = []
    for path, data in sorted(recipe.files.items()):
        relative_path(path)
        if len(data) > MAX_FILE_BYTES:
            raise ValueError("recipe exceeds file budget: " + recipe.id)
        total += len(data)
        files.append({"path": path, "size_bytes": len(data), "sha256": digest(data)})
    directories = set(recipe.directories)
    for path in recipe.files:
        directories.update(str(parent) for parent in relative_path(path).parents if str(parent) != ".")
    for path in directories:
        relative_path(path)
    if total > MAX_BUNDLE_BYTES:
        raise ValueError("recipe exceeds bundle budget")
    expected = {"directories": sorted(directories), "files": files}
    media_types = {"image": "image/png", "frame_sequence": "application/vnd.aniflow.frame-sequence+directory",
                   "audio": "audio/wav", "container": "video/x-yuv4mpeg", "timed_text": "text/plain",
                   "temporal_protocol": "application/vnd.aniflow.synthetic-probe+json"}
    return {"id": recipe.id, "family": recipe.family, "validity": recipe.validity,
            "media_type": media_types[recipe.family], "seed": SEED,
            "parameters": recipe.parameters, "corruption": recipe.corruption,
            "oracle": recipe.oracle, "inventory": expected, "inventory_sha256": digest(json_bytes(expected)),
            "qualification": "authored_not_run"}


def compile_catalog():
    cases = [describe(recipe) for recipe in recipes()]
    if len({case["id"] for case in cases}) != len(cases):
        raise ValueError("duplicate corpus fixture IDs")
    sources = {"scripts/generate-temporal-fixtures.py", "docs/validation/aniflow-51-fixture-index.json", "LICENSE"}
    sources.update(path.relative_to(ROOT).as_posix() for path in (ROOT / "scripts/corpus").glob("*.py"))
    sources.add("scripts/temporal-corpus.py")
    sources.update(["tests/corpus_tools.py", "tests/corpus_schema.py", "conformance/temporal-v1/catalog.schema.json"])
    coverage = []
    for family, targets in SUITES.items():
        for target in targets:
            path = "tests/" + target + ".rs"
            text = regular_bytes(ROOT / path).decode("utf-8")
            names = re.findall(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(\w+)\s*\(", text)
            if not names:
                raise ValueError("registered suite has no authored test locators: " + path)
            sources.add(path)
            coverage.append({"id": "ani.suite." + target, "family": family, "kind": "rust_suite",
                             "path": path, "members": names, "qualification": "authored_not_run"})
    audio = json.loads(regular_bytes(ROOT / "docs/validation/aniflow-51-fixture-index.json"))
    for family in audio["families"]:
        coverage.append({"id": "ani.index.audio." + family["family"], "family": "audio_text",
                         "kind": "historical_index", "path": "docs/validation/aniflow-51-fixture-index.json",
                         "members": [fixture["id"] for fixture in family["fixtures"]],
                         "qualification": "historical_locator_only"})
        for key in ("generator", "executable"):
            candidate = family.get(key)
            if isinstance(candidate, str) and (ROOT / candidate).is_file():
                sources.add(candidate)
        sources.update(family.get("test_paths", []))
        sources.update(family.get("receipt_paths", []))
    for directory in ["conformance/provider-v1", "conformance/validation-v1", "tests/fixtures/temporal"]:
        sources.update(path.relative_to(ROOT).as_posix() for path in (ROOT / directory).rglob("*") if path.is_file())
    return {"schema": "aniflow.adversarial-corpus/v1", "version": VERSION,
            "generator": {"version": VERSION, "path": "scripts/temporal-corpus.py", "seed": SEED,
                          "runtime": "Python 3.10+ standard library; exact runtime recorded only when qualification executes"},
            "provenance": {"license": "MIT", "license_path": "LICENSE", "origin": "repository-authored synthetic recipes",
                           "external_media": False, "personal_data": False},
            "qualification": "authored_not_run", "cases": cases, "native_profiles": NATIVE_PROFILES,
            "coverage": coverage, "sources": [source_record(path) for path in sorted(sources)],
            "limits": {"maximum_file_bytes": MAX_FILE_BYTES, "maximum_bundle_bytes": MAX_BUNDLE_BYTES,
                       "maximum_files": 4096},
            "tiers": {
                "pr": {"timeout_seconds": 900, "native_tools": False, "maximum_output_bytes": 67108864, "maximum_build_bytes": 4294967296},
                "scheduled": {"timeout_seconds": 1800, "native_tools": True, "maximum_output_bytes": 134217728, "maximum_build_bytes": 4294967296},
                "extended": {"timeout_seconds": 3600, "native_tools": True, "maximum_output_bytes": 268435456, "maximum_build_bytes": 4294967296}},
            "residuals": [
                {"family": "multi_artifact_output_ports", "owner": "#64; implementation #69",
                 "status": "authored_not_run: exact nonempty One/OneOrMore/Many sets; Optional and empty output sets remain unsupported"},
                {"family": "real_models_and_optional_processors", "owner": "#64", "status": "unqualified"},
                {"family": "native_codec_platform_matrix", "owner": "#64", "status": "unqualified"},
                {"family": "full_issue_24_fixture_breadth", "owner": "#24", "status": "bounded_corpus_not_exhaustive"}]}


def catalog_bytes():
    return json_bytes(compile_catalog())


def check_catalog():
    expected = catalog_bytes()
    if regular_bytes(CATALOG, 8 * 1024 * 1024) != expected:
        raise ValueError("corpus catalog drift; review recipes and explicitly regenerate the catalog")
    return json.loads(expected)


def generate(destination):
    destination = safe_new_directory(destination, ROOT)
    catalog = check_catalog()
    with tempfile.TemporaryDirectory(prefix=".aniflow-corpus-", dir=destination.parent) as scratch:
        staging = Path(scratch) / "bundle"
        staging.mkdir()
        (staging / "catalog.json").write_bytes(json_bytes(catalog))
        (staging / "owner.json").write_bytes(json_bytes({"schema": "aniflow.corpus-owner/v1", "purpose": "synthetic qualification only"}))
        for recipe in recipes():
            case_root = staging / recipe.id
            case_root.mkdir()
            for directory in recipe.directories:
                (case_root / directory).mkdir(parents=True, exist_ok=True)
            for path, data in recipe.files.items():
                target = case_root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                with target.open("xb") as stream:
                    stream.write(data)
        # Enforce aggregate storage bounds before publication, including metadata.
        inventory(staging)
        # Exclusive creation prevents replacing an output appearing after preflight.
        # Interrupted publication is refused by verify_bundle; ownership is last.
        destination.mkdir()
        for item in sorted(staging.iterdir(), key=lambda path: (path.name == "owner.json", path.name)):
            item.rename(destination / item.name)
    return catalog


def verify_bundle(root):
    root = Path(root)
    expected = check_catalog()
    if regular_bytes(root / "catalog.json") != json_bytes(expected):
        raise ValueError("bundle catalog differs from canonical recipes")
    names = {"catalog.json", "owner.json"} | {case["id"] for case in expected["cases"]}
    if {path.name for path in root.iterdir()} != names:
        raise ValueError("unexpected or missing bundle entries")
    if json.loads(regular_bytes(root / "owner.json")) != {"schema": "aniflow.corpus-owner/v1", "purpose": "synthetic qualification only"}:
        raise ValueError("corpus ownership mismatch")
    for case in expected["cases"]:
        if inventory(root / case["id"]) != case["inventory"]:
            raise ValueError("fixture inventory drift: " + case["id"])
    inventory(root)
    return expected
