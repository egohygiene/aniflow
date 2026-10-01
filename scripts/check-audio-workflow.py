#!/usr/bin/env python3
"""Independently check the audio support index and optional retained smoke evidence.

This checker reads local files only. Public JSON documents receive Draft 2020-12
validation; untagged private snapshots receive digest checks, never an invented
public schema. It does not execute providers or qualify model/media accuracy.
"""
from __future__ import annotations

import argparse
import copy
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import re
import shlex
import sys
import wave

from jsonschema import Draft202012Validator
from referencing import Registry, Resource
from referencing.exceptions import NoSuchResource

ROOT = Path(__file__).resolve().parents[1]
MATRIX = "docs/validation/aniflow-51-support-matrix.json"
PROFILE_IDS = {"foundation", "technical", "signal", "stem_lineage", "musical",
               "transcription", "lyrics_alignment", "midi", "timed_text"}
AC_IDS = {f"ANI13-AC{index:02}" for index in range(1, 15)}
SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class ConformanceError(ValueError):
    """Retained evidence or a declared support reference is inconsistent."""


def require(condition, message):
    if not condition:
        raise ConformanceError(message)


def reject_nonfinite(value):
    raise ConformanceError(f"Nonfinite JSON number: {value}")


def finite_float(value):
    number = float(value)
    require(math.isfinite(number), f"Nonfinite JSON number: {value}")
    return number


def reject_duplicates(pairs):
    document = {}
    for key, value in pairs:
        require(key not in document, f"Duplicate JSON property: {key}")
        document[key] = value
    return document


def loads(content):
    try:
        return json.loads(content, object_pairs_hook=reject_duplicates,
                          parse_constant=reject_nonfinite, parse_float=finite_float)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ConformanceError(f"Invalid JSON: {error}") from error


def load(path):
    return loads(Path(path).read_bytes())


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def unique(items, key, label):
    result = {}
    for item in items:
        value = item[key]
        require(value not in result, f"Duplicate {label}: {value}")
        result[value] = item
    return result


def repository_path(root, name):
    path = Path(name)
    require(not path.is_absolute() and ".." not in path.parts,
            f"Unsafe repository reference: {name}")
    target = (root / path).resolve(strict=True)
    require(target.is_relative_to(root.resolve()) and target.is_file(),
            f"Reference is not a repository file: {name}")
    return target


def schemas(root):
    by_path, by_tag = {}, {}
    paths = list((root / "docs/contracts").glob("*.schema.json"))
    paths.extend((root / "providers").glob("**/*.schema.json"))
    for path in sorted(paths):
        document = load(path)
        Draft202012Validator.check_schema(document)
        path = path.resolve()
        by_path[path] = document
        tag = document.get("properties", {}).get("schema", {}).get("const")
        if tag:
            require(tag not in by_tag, f"Duplicate public schema tag: {tag}")
            by_tag[tag] = path
    return by_path, by_tag


def offline_registry(root, by_path):
    def refuse_remote(uri):
        raise NoSuchResource(ref=uri)

    resources = []
    for path, schema in by_path.items():
        resource = Resource.from_contents(schema)
        aliases = {path.as_uri(), "https://github.com/egohygiene/aniflow/" + str(path.relative_to(root.resolve()))}
        if "$id" in schema:
            aliases.add(schema["$id"])
        resources.extend((alias, resource) for alias in aliases)
    return Registry(retrieve=refuse_remote).with_resources(resources)


def source_block(text, keyword):
    match = re.search(r"\b" + re.escape(keyword) + r"\s*\{", text)
    require(match is not None, f"Missing Rust declaration: {keyword}")
    start, depth = match.end(), 1
    for position in range(start, len(text)):
        depth += (text[position] == "{") - (text[position] == "}")
        if depth == 0:
            return text[start:position]
    raise ConformanceError(f"Unclosed Rust declaration: {keyword}")


def kebab(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def enum_names(text, name):
    return {kebab(value) for value in re.findall(r"^    ([A-Z]\w*)\s*(?:,|\{)",
                                                source_block(text, "enum " + name), re.MULTILINE)}


def exported_module(root, name):
    require(name.startswith("aniflow::") and name.count("::") == 1,
            f"Unknown public module reference: {name}")
    short = name.split("::")[1]
    lib = (root / "src/lib.rs").read_text()
    require(re.search(r"\bpub mod " + re.escape(short) + r"\s*;", lib),
            f"Module is not publicly exported: {name}")
    direct = root / "src" / (short + ".rs")
    entry = direct if direct.exists() else root / "src" / short / "mod.rs"
    text = entry.read_text()
    sources = {"entry": text}
    # Only named public reexports make a private child part of this surface.
    for match in re.finditer(r"\bpub use (\w+)::(\*|\{[^}]+\}|\w+)\s*;", text, re.DOTALL):
        child = entry.parent / (match[1] + ".rs")
        require(child.is_file(), f"Missing reexport source: {name}::{match[1]}")
        sources[match[1]] = child.read_text()
    return sources


def public_symbol(sources, symbol, function=False):
    declaration = r"\bpub\s+(?:const\s+|async\s+)?fn\s+" if function else r"\bpub\s+(?:struct|enum|type)\s+"
    pattern = declaration + re.escape(symbol) + r"\b"
    entry = sources["entry"]
    if re.search(pattern, entry):
        return True
    for child, text in sources.items():
        if child == "entry" or not re.search(pattern, text):
            continue
        match = re.search(r"\bpub use " + re.escape(child) + r"::(\*|\{[^}]+\}|\w+)\s*;", entry, re.DOTALL)
        if match and (match[1] == "*" or re.search(r"\b" + re.escape(symbol) + r"\b", match[1])):
            return True
    return False


def public_method(sources, owner, operation):
    for text in sources.values():
        pattern = r"\bimpl\s+" + re.escape(owner) + r"\s*\{"
        for match in re.finditer(pattern, text):
            block = source_block(text[match.start():], "impl " + owner)
            if re.search(r"\bpub\s+(?:const\s+)?fn\s+" + re.escape(operation) + r"\b", block):
                return True
    return False


def check_matrix(root, path=None):
    root = root.resolve()
    path = path or root / MATRIX
    matrix = load(path)
    require(matrix["schema"] == "aniflow.audio-support-matrix/v1", "Unknown support matrix schema")
    acceptance = unique(matrix["acceptance_items"], "id", "acceptance item")
    require(set(acceptance) == AC_IDS, "The original 14 acceptance IDs must all be reconciled")
    checkpoints = unique(matrix["checkpoint_evidence"], "issue", "checkpoint issue")
    known = {int(item["issue"].rsplit("/", 1)[1]): item for item in checkpoints.values()}
    require(set(range(42, 51)) <= set(known), "Missing #42–#50 checkpoint evidence")
    references = set()
    for item in checkpoints.values():
        require(item["state"] == "merged", "Historical checkpoint must identify merged evidence")
        require(re.fullmatch(r"https://github.com/egohygiene/aniflow/pull/\d+", item["pull_request"]),
                "Checkpoint has an invalid pull request reference")
        references.add(item["receipt"])
        receipt = load(repository_path(root, item["receipt"]))
        require(isinstance(receipt, dict) and bool(receipt), "Empty checkpoint receipt")
    for item in acceptance.values():
        require(item["original_item"].strip() and item["limitations"].strip(),
                f"Acceptance wording/limitations missing: {item['id']}")
        require(set(item["checkpoints"]) <= set(known) | {51}, "Unknown acceptance checkpoint")
        require(bool(item["evidence"]) and any(item["evidence"].values()), "Acceptance item lacks evidence")
        for paths in item["evidence"].values():
            references.update(paths)
    by_path, by_tag = schemas(root)
    profiles = unique(matrix["profiles"], "id", "profile")
    require(set(profiles) == PROFILE_IDS, "Support matrix must contain foundation and eight families")
    task_text = (root / "Taskfile.yml").read_text()
    task_names = set(re.findall(r"^  ([\w:-]+):\s*$", task_text, re.MULTILINE))
    cli = (root / "src/cli.rs").read_text()
    audio_commands = enum_names(cli, "AudioCommands")
    timed_commands = enum_names(cli, "TimedTextCommands")
    nested = {"midi": enum_names(cli, "MidiCommands"), "lyrics": enum_names(cli, "LyricsCommands")}
    analysis_selectors = enum_names(cli, "AudioAnalysisKind")
    estimate_selectors = enum_names(cli, "AudioEstimateKind")
    capability_source = (root / "src/audio_analysis.rs").read_text()
    capability_declaration = capability_source.split("AUDIO_CAPABILITY_IDS_V1", 1)[1].split("];", 1)[0]
    capabilities = set(re.findall(r'"(aniflow/audio-[^" ]+)"', capability_declaration))
    declared_families = set()
    api_count = cli_count = task_count = 0
    for profile in profiles.values():
        api = profile["public_api"]
        sources = exported_module(root, api["module"])
        for name in api["types"]:
            require(public_symbol(sources, name), f"Missing public type: {api['module']}::{name}")
            api_count += 1
        for name in api["operations"]:
            if "::" in name:
                owner, operation = name.split("::")
                method_sources = sources
                if owner == "AudioInspectionRequest":
                    method_sources = exported_module(root, "aniflow::audio_inspection")
                require(public_symbol(method_sources, owner), f"Unknown method owner: {owner}")
                require(public_method(method_sources, owner, operation), f"Missing public method: {name}")
            else:
                require(public_symbol(sources, name, function=True), f"Missing public operation: {api['module']}::{name}")
            api_count += 1
        for command in profile["cli"]:
            tokens = shlex.split(command)
            require(len(tokens) >= 2, f"Invalid CLI reference: {command}")
            if tokens[0] == "audio":
                require(tokens[1] in audio_commands, f"Unknown audio command: {command}")
                if tokens[1] in nested:
                    require(len(tokens) >= 3 and tokens[2] in nested[tokens[1]], f"Unknown nested command: {command}")
                if "--analysis" in tokens:
                    selector = tokens[tokens.index("--analysis") + 1]
                    allowed = estimate_selectors if tokens[1] == "analyze" else analysis_selectors
                    require(selector in allowed, f"Unknown analysis selector: {command}")
            elif tokens[0] == "timed-text":
                require(tokens[1] in timed_commands, f"Unknown timed-text command: {command}")
            else:
                raise ConformanceError(f"Unknown CLI family: {command}")
            for token in tokens:
                if token.startswith("--"):
                    field = token[2:].replace("-", "_")
                    require(re.search(r"\b" + re.escape(field) + r"\s*:", cli), f"Unknown CLI flag: {token}")
            cli_count += 1
        for task in profile["tasks"]:
            require(task in task_names, f"Unknown Task reference: {task}")
            task_count += 1
        for name in profile["schemas"]:
            target = repository_path(root, name)
            require(target in by_path, f"Not an indexed public schema: {name}")
        references.update(profile["schemas"] + profile["docs"])
        require(set(profile["checkpoints"]) <= set(known), "Unknown family checkpoint")
        require(set(profile["capability_ids"]) <= capabilities, "Unknown normalized capability ID")
        if profile["id"] != "foundation":
            declared_families.update(profile["capability_ids"])
        require(profile["normalized_family_version"] == "1.0.0", "Unknown normalized family version")
        require(profile["qualification"].strip(), "Family qualification must remain explicit")
    require(declared_families == capabilities, "Families do not cover the normalized capability set")
    musical_schema = load(root / "docs/contracts/audio-musical-analysis-v1.schema.json")
    unsupported = musical_schema["properties"]["unsupported_families"]["const"]
    require(set(profiles["musical"]["unsupported"]) == set(unsupported), "Musical unsupported families were dropped or promoted")
    dispositions = unique(matrix["explicit_dispositions"], "id", "scope disposition")
    expected = {"wider_musical_families": "unsupported", "calibrated_model_confidence": "unavailable",
                "transcription_word_timing": "unavailable", "model_inference_and_quality": "unverified",
                "native_platform_qualification": "unverified", "hosted_ci": "unverified",
                "rendered_previews": "not_implemented_by_audio_roadmap",
                "adr_acceptance": "proposed_not_accepted_by_implementation"}
    for name, status in expected.items():
        require(name in dispositions and dispositions[name]["status"] == status,
                f"Unsupported/unverified scope was promoted: {name}")
    require(set(dispositions["wider_musical_families"]["values"]) == set(unsupported), "Unsupported disposition differs from public contract")
    for name in references:
        repository_path(root, name)
    for field in ["real_media_read_or_mutated", "models_downloaded", "paid_apis", "hosted_ci_pass_claimed", "release_claimed"]:
        require(matrix["completion"][field] is False, f"Unqualified completion claim: {field}")
    return {"path": str(path), "sha256": digest(path), "acceptance_items": len(acceptance),
            "profiles": len(profiles), "capability_families": len(capabilities),
            "repository_references": len(references), "public_api_references": api_count,
            "cli_references": cli_count, "task_references": task_count,
            "indexed_schemas": len(by_path), "indexed_schema_tags": len(by_tag)}


def check_reference(item, label):
    path = Path(item["path"]).resolve(strict=True)
    require(path.is_file(), f"Not a regular evidence file: {label}")
    require(SHA256.fullmatch(item["sha256"]) and digest(path) == item["sha256"], f"Digest mismatch: {label}")
    if "byte_size" in item:
        require(path.stat().st_size == item["byte_size"], f"Byte size mismatch: {label}")
    return path


def check_fixture_index(root, path=None):
    path = path or root / "docs/validation/aniflow-51-fixture-index.json"
    index = load(path)
    require(index["schema"] == "aniflow.audio-fixture-index/v1", "Unknown audio fixture index")
    families = unique(index["families"], "family", "fixture family")
    require(set(families) == set(index["family_counts"]), "Fixture family counts differ from index")
    identifiers, references = set(), set()
    for name, family in families.items():
        local = unique(family["fixtures"], "local_id", "local fixture")
        require(len(local) == index["family_counts"][name], "Fixture family count differs from declarations")
        references.add(family["generator"])
        references.update(family["test_paths"] + family["receipt_paths"])
        for item in local.values():
            require(item["id"].startswith("ani.audio.") and item["id"] not in identifiers,
                    "Duplicate or invalid namespaced fixture identity")
            identifiers.add(item["id"])
            require(item["purpose"].strip(), "Fixture purpose missing")
            if "fixture_path" in item:
                references.add(item["fixture_path"])
    require(len(identifiers) == index["fixture_count"], "Total fixture count differs from declarations")
    for name in references:
        repository_path(root, name)
    require(index["safety"]["synthetic_only"] is True and all(index["safety"][field] is False for field in
            ["real_media_read_or_mutated", "model_downloads_required", "paid_apis_required"]), "Unqualified fixture safety claim")
    return {"path": str(path), "sha256": digest(path), "families": len(families),
            "fixtures": len(identifiers), "repository_references": len(references)}


def artifact_file(artifact, files, label):
    matches = files.get(artifact["sha256"], [])
    require(bool(matches), f"Unresolved artifact digest: {label}")
    require(any(path.stat().st_size == artifact["byte_size"] for path in matches), f"Artifact size mismatch: {label}")
    return next(path for path in matches if path.stat().st_size == artifact["byte_size"])


def verify_source(source, files):
    path = artifact_file(source["artifact"], files, "source")
    with wave.open(str(path), "rb") as audio:
        require((audio.getframerate(), audio.getnchannels(), audio.getnframes()) ==
                (source["sample_rate_hz"], source["channels"], source["frame_count"]), "Source PCM clock differs from retained bytes")
        require(audio.getsampwidth() == 2, "Workflow source is not PCM16")
    require(source["stream_index"] == 0 and source["origin"] == {"numerator": 0, "denominator": 1},
            "Unsupported source clock in synthetic workflow")
    if source.get("stem"):
        artifact_file(source["stem"]["original_mix"], files, "original mix")


def verify_analysis(analysis, files):
    source = analysis["source"]
    verify_source(source, files)
    artifact_ids = {source["artifact"]["id"]}
    if source.get("stem"):
        artifact_ids.add(source["stem"]["original_mix"]["id"])
    artifacts = unique(analysis["artifacts"], "id", "normalized artifact")
    artifact_ids.update(artifacts)
    for artifact in artifacts.values():
        artifact_file(artifact, files, artifact["id"])
    providers = unique(analysis["providers"], "id", "normalized provider")
    capabilities = {}
    for item in analysis["capabilities"]:
        name = item["capability"]["id"]
        require(name not in capabilities, "Duplicate normalized capability")
        capabilities[name] = item
    for capability in capabilities.values():
        require(set(capability["evidence_artifact_ids"]) <= artifact_ids, "Unknown capability artifact reference")
        require(set(capability["provider_evidence_ids"]) <= set(providers), "Unknown capability provider reference")

    def provenance(item):
        value = item["provenance"]
        require(value["provider_evidence_id"] in providers, "Unknown provenance provider")
        require(set(value["evidence_artifact_ids"]) <= artifact_ids, "Unknown provenance artifact")

    def scope(item):
        value = item["scope"]
        require(value["stem_id"] == (source["stem"]["id"] if source.get("stem") else None), "Scope stem differs from source")
        channels = value["channels"]
        require(bool(channels) and channels == sorted(set(channels)) and all(0 <= channel < source["channels"] for channel in channels), "Invalid channel scope")

    def span(value):
        require(0 <= value["start"] < value["end"] <= source["frame_count"], "Timeline/excerpt outside source clock")

    for observation in analysis["observations"]:
        provenance(observation)
        scope(observation)
        require(observation["capability_id"] in capabilities, "Observation refers to unknown capability")
    for timeline in analysis["timelines"]:
        scope(timeline)
        require(timeline["capability_id"] in capabilities, "Timeline refers to unknown capability")
        previous = -1
        for event in timeline["events"]:
            span(event["range"])
            require(event["range"]["start"] >= previous, "Unordered timeline")
            previous = event["range"]["end"] if timeline["kind"] == "disjoint_regions" else event["range"]["start"]
            provenance(event)
    for item in analysis["excerpts"]:
        scope(item)
        span(item["range"])
        require(item["artifact_id"] in artifact_ids and set(item["evidence_artifact_ids"]) <= artifact_ids, "Unknown excerpt artifact")
    for item in analysis["semantic_artifacts"]:
        provenance(item)
        require(item["artifact_id"] in artifact_ids, "Unknown semantic artifact")
        if item["kind"] in ["observed_transcript", "midi_candidate"]:
            require(item["authority"] is None, "Observed candidate was promoted to authored authority")


def verify_midi(path, report, notes):
    try:
        import mido
    except ImportError as error:
        raise ConformanceError("Independent MIDI readback requires an already installed mido package") from error
    midi = mido.MidiFile(path)
    require(midi.type == 0 and midi.ticks_per_beat == 960 and len(midi.tracks) == 1, "Unexpected MIDI serialization profile")
    absolute, active, observed, tempos, programs = 0, {}, [], [], []
    for message in midi.tracks[0]:
        absolute += message.time
        if message.type == "set_tempo":
            tempos.append(message.tempo)
        elif message.type == "program_change":
            programs.append((message.channel, message.program))
        elif message.type == "note_on" and message.velocity:
            key = (message.channel, message.note)
            require(key not in active, "Ambiguous same-pitch MIDI overlap")
            active[key] = (absolute, message.velocity)
        elif message.type in ["note_on", "note_off"]:
            key = (message.channel, message.note)
            require(key in active, "Unpaired MIDI note end")
            start, velocity = active.pop(key)
            observed.append({"start_tick": start, "end_tick": absolute, "pitch": message.note, "velocity": velocity})
            require(message.channel == 0, "Unexpected MIDI channel")
    require(not active and tempos == [500000] and programs == [(0, 0)], "Incomplete MIDI or unexpected tempo/program mapping")
    order = lambda value: (value["start_tick"], value["end_tick"], value["pitch"], value["velocity"])
    expected = [{key: note[key] for key in ["start_tick", "end_tick", "pitch", "velocity"]} for note in notes["notes"]]
    require(sorted(observed, key=order) == sorted(expected, key=order) == sorted(report["read_back"]["notes"], key=order), "Mido readback differs from candidate/export evidence")
    require(len(observed) == report["note_count"] and report["empty_candidate"] == (len(observed) == 0), "MIDI note count differs from evidence")
    return {"sha256": digest(path), "notes": len(observed), "reader": "mido", "version": mido.version.__version__}


def verify_midi_package(directory, report, files):
    for key in ["source_report", "source_audio", "notes", "midi"]:
        artifact_file(report[key], files, "MIDI export:" + key)
    candidate_path = directory / "candidate-report.json"
    midi_path, notes_path = directory / "candidate.mid", directory / "notes.json"
    for key, target in [("source_report", candidate_path), ("midi", midi_path), ("notes", notes_path)]:
        check_reference({"path": str(target), **report[key]}, "MIDI package:" + key)
    candidate, notes = load(candidate_path), load(notes_path)
    require(candidate["source"]["artifact"] == report["source_audio"], "MIDI export source audio differs from package candidate")
    require(candidate["result"]["observation"] == notes, "MIDI notes companion differs from package candidate observation")
    return verify_midi(midi_path, report, notes)


def check_receipt(root, path):
    receipt = load(path)
    require(receipt["schema"] == "aniflow.audio-workflow-smoke/v1", "Unknown workflow receipt schema")
    for field, expected in [("synthetic_provider", True), ("real_model_inference", False), ("models_downloaded", False), ("sources_unchanged", True)]:
        require(receipt[field] is expected, f"Unsupported workflow qualification: {field}")
    source_path = check_reference(receipt["source"], "original source")
    check_reference(receipt["aniflow"], "aniflow executable")
    root_directory = source_path.parent
    # All generated assets are beneath the explicit retained workflow root.
    files = {}
    for file in root_directory.rglob("*"):
        if file.is_file():
            files.setdefault(digest(file), []).append(file.resolve())
    for declared in [receipt["source"], *receipt.get("profiles", [])]:
        if not all(field in declared for field in ["sample_rate_hz", "channels", "frame_count"]):
            continue  # Digest-only test receipts do not claim a PCM clock.
        sha = declared.get("source_sha256", declared.get("sha256"))
        require(sha in files, "Declared profile source digest is unresolved")
        with wave.open(str(files[sha][0]), "rb") as audio:
            require((audio.getframerate(), audio.getnchannels(), audio.getnframes()) ==
                    (declared["sample_rate_hz"], declared["channels"], declared["frame_count"]),
                    "Declared workflow profile clock differs from retained PCM bytes")
    if "profiles" in receipt:
        unique(receipt["profiles"], "id", "declared PCM profile")
    by_path, by_tag = schemas(root)
    registry = offline_registry(root, by_path)
    documents = unique(receipt["documents"], "id", "captured document")
    parsed, paths, counts = {}, {}, {"schema_validated": 0, "digest_only_private": 0,
                                    "digest_and_semantics_internal_contract": 0}
    notes_schema = (root / "docs/contracts/audio-midi-notes-v1.schema.json").resolve()
    envelope_schema = (root / "docs/contracts/machine-envelope-v1.schema.json").resolve()
    for identifier, item in documents.items():
        target = check_reference(item, identifier)
        require(target.is_relative_to(root_directory), f"Captured document escapes retained root: {identifier}")
        document = load(target)
        parsed[identifier], paths[identifier] = document, target
        tag = document.get("schema") if isinstance(document, dict) else None
        require(item.get("schema") == tag, f"Captured schema tag differs: {identifier}")
        schema_path = by_tag.get(tag) if tag else None
        if tag == "aniflow.artifact-integrity-validation/v1":
            # Existing private struct in run_v3.rs has no published JSON schema.
            # Preserve that distinction while reobserving its actual artifact.
            require("schema_path" not in item and "schema_sha256" not in item,
                    "Internal integrity evidence claims a nonexistent public schema")
            require(set(document) == {"schema", "validation_id", "contract", "artifact_id", "artifact_sha256", "accepted"}
                    and document["contract"] == "aniflow.validation/artifact-integrity/v1"
                    and document["accepted"] is True and document["artifact_sha256"] in files,
                    "Internal integrity evidence differs from accepted retained artifact")
            counts["digest_and_semantics_internal_contract"] += 1
            continue
        if tag:
            require(schema_path is not None, f"Unknown public document tag: {tag}")
        elif isinstance(document, dict) and "schema_version" in document:
            schema_path = envelope_schema
        elif "schema_path" in item:
            schema_path = Path(item["schema_path"]).resolve(strict=True)
            require(schema_path == notes_schema and target.name == "notes.json", "Unrecognized untagged schema companion")
        if schema_path:
            require(Path(item.get("schema_path", "")).resolve() == schema_path, f"Missing/wrong public schema reference: {identifier}")
            require(item.get("schema_sha256") == digest(schema_path), f"Schema digest mismatch: {identifier}")
            errors = list(Draft202012Validator(by_path[schema_path], registry=registry).iter_errors(document))
            require(not errors, f"Public schema validation failed: {identifier}: {errors[0].message if errors else ''}")
            counts["schema_validated"] += 1
        else:
            require("schema_sha256" not in item, "Private snapshot claims unknown schema identity")
            counts["digest_only_private"] += 1
    runs = unique(receipt["runs"], "id", "captured run")
    output_count = 0
    expected_links = set()
    for identifier, run in runs.items():
        directory = Path(run["run_directory"]).resolve(strict=True)
        require(directory.is_relative_to(root_directory), "Run escapes retained workflow root")
        require(SHA256.fullmatch(run["plan_sha256"]), "Invalid run plan identity")
        outputs = unique(run["outputs"], "id", "run output")
        for item in outputs.values():
            target = check_reference(item, identifier + ":" + item["id"])
            require(target.is_relative_to(directory), "Run output locator escapes owning run")
            output_count += 1
        manifests = [value for key, value in parsed.items() if paths[key].is_relative_to(directory) and isinstance(value, dict) and value.get("schema") == "aniflow.pipeline-run/v1"]
        require(any(value["payload"]["plan_sha256"] == run["plan_sha256"] and value["payload"]["state"] == run["state"] for value in manifests), "Run state/plan differs from retained manifest")
        if "analysis" not in outputs:
            continue
        analysis = load(outputs["analysis"]["path"])
        verify_analysis(analysis, files)
        if "stem_lineage" in outputs:
            lineage = load(outputs["stem_lineage"]["path"])["lineage"]
            require(lineage["selected_stem"] == analysis["source"], "Final normalized source differs from selected lineage clock")
            require(lineage["timing_basis"] == "zero_origin_duration_only", "Workflow silently invented a mix clock mapping")
            verify_source(lineage["original_mix"], files)
            mix, selected = lineage["original_mix"], lineage["selected_stem"]
            require(Fraction(mix["frame_count"], mix["sample_rate_hz"]) == Fraction(selected["frame_count"], selected["sample_rate_hz"]), "Stem/mix duration differs")
            require(mix["artifact"]["sha256"] == receipt["source"]["sha256"], "Stem lineage original mix differs from workflow source")
            require(lineage["source_plan_sha256"] in {value["plan_sha256"] for value in runs.values()}, "Lineage refers to uncaptured separation plan")
        for family in ["signal", "musical", "transcription", "alignment", "midi"]:
            if family not in outputs:
                continue
            report = load(outputs[family]["path"])
            expected_source = copy.deepcopy(analysis["source"])
            if family == "signal":
                expected_source["stem"] = None
            require(report["source"] == expected_source, "Family companion source identity/clock differs from normalized evidence")
            locks = [value for key, value in parsed.items() if paths[key].is_relative_to(directory) and isinstance(value, dict) and value.get("schema") == "aniflow.provider-lock/v1" and value["lock_sha256"] == report["provider_lock_sha256"]]
            require(len(locks) == 1, "Family companion provider lock is unresolved/ambiguous")
            require(locks[0]["payload"]["provider"] == report["provider"] and locks[0]["payload"]["effective_configuration_sha256"] == report["configuration_sha256"], "Family companion provider/configuration differs from lock")
            upstream = "inspection_analysis_artifact" if family == "signal" else "upstream_analysis_artifact"
            for name in ["technical_artifact", upstream]:
                artifact_file(report[name], files, family + ":" + name)
                expected_links.add((digest(outputs[family]["path"]), report[name]["sha256"]))
    links = unique(receipt["links"], "id", "evidence link")
    observed_links = set()
    for item in links.values():
        require(item["from_document"] in parsed and item["to_document"] in parsed, "Evidence link refers to unknown captured document")
        require(item["relation"] == "digest_bound_upstream_evidence", "Unknown evidence relation")
        source, target = parsed[item["from_document"]], documents[item["to_document"]]
        require(any(isinstance(value, dict) and value.get("sha256") == target["sha256"] and value.get("byte_size") == target["byte_size"] for value in source.values()), "Recorded upstream link is not bound by source report")
        observed_links.add((documents[item["from_document"]]["sha256"], target["sha256"]))
    require(expected_links <= observed_links, "Required upstream evidence link is missing")
    midi_checks = []
    for identifier, report in parsed.items():
        if not isinstance(report, dict) or report.get("schema") != "aniflow.audio-midi-export/v1":
            continue
        directory = paths[identifier].parent
        if paths[identifier].name != "export-report.json":
            continue  # CLI outcome copies are not package completion markers.
        midi_checks.append(verify_midi_package(directory, report, files))
    return {"path": str(path), "sha256": digest(path), "documents": len(documents), **counts,
            "runs": len(runs), "run_outputs": output_count, "upstream_links": len(links),
            "retained_assets": sum(map(len, files.values())), "midi_readbacks": midi_checks,
            "qualification": "synthetic contract, identity and clock conformance; real model accuracy, native platforms and release remain unverified"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--matrix", type=Path)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    try:
        result = {"schema": "aniflow.audio-workflow-conformance/v1", "status": "passed",
                  "matrix": check_matrix(arguments.root, arguments.matrix),
                  "fixture_index": check_fixture_index(arguments.root)}
        if arguments.receipt:
            result["receipt"] = check_receipt(arguments.root, arguments.receipt)
        content = json.dumps(result, indent=2, ensure_ascii=False, allow_nan=False) + "\n"
        if arguments.output:
            arguments.output.write_text(content, encoding="utf-8")
        else:
            print(content, end="")
    except (ConformanceError, OSError, KeyError, TypeError, ValueError) as error:
        print(json.dumps({"schema": "aniflow.audio-workflow-conformance/v1", "status": "failed", "error": str(error)}), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
