"""Deterministic native archives, dependency inventory and signed bundle checks."""

from __future__ import annotations

import datetime
import gzip
import io
import json
from pathlib import Path
import tarfile
import tomllib

from .common import (ROOT, REPOSITORY, MAX_TOTAL, checksum_bytes, command, digest,
                     inventory, json_bytes, new_directory, read_json, regular_bytes,
                     require, verify_inventory, write_new)
from .plan import verify_candidate


def binary_identity(data, target):
    if target == "x86_64-unknown-linux-gnu":
        require(len(data) >= 64 and data[:6] == b"\x7fELF\x02\x01" and
                int.from_bytes(data[18:20], "little") == 62, "expected x86_64 ELF binary")
    elif target == "aarch64-apple-darwin":
        require(len(data) >= 32 and data[:4] == b"\xcf\xfa\xed\xfe" and
                int.from_bytes(data[4:8], "little") == 0x100000c, "expected arm64 Mach-O binary")
    else:
        raise ValueError("unsupported release target")


def payload_files(binary, repository=ROOT):
    files = {"bin/aniflow": (binary, 0o755)}
    for name in ["LICENSE", "README.md", "CHANGELOG.md"]:
        files[name] = (regular_bytes(repository / name), 0o644)
    for path in sorted((repository / "docs/contracts").rglob("*.schema.json")):
        files["contracts/" + path.relative_to(repository / "docs/contracts").as_posix()] = (regular_bytes(path), 0o644)
    require(len(files) <= 1024 and sum(len(data) for data, _ in files.values()) <= MAX_TOTAL,
            "archive payload exceeds budget")
    return files


def archive_bytes(files):
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode="wb", filename="", mtime=0, compresslevel=9) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for name, (data, mode) in sorted(files.items()):
                entry = tarfile.TarInfo(name)
                entry.size, entry.mode = len(data), mode
                entry.uid = entry.gid = entry.mtime = 0
                entry.uname = entry.gname = ""
                archive.addfile(entry, io.BytesIO(data))
    return buffer.getvalue()


def archive_inventory(data, target):
    # Bound decompression before tar parsing; never extract untrusted paths.
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as compressed:
        expanded = compressed.read(MAX_TOTAL + 1)
    require(len(expanded) <= MAX_TOTAL, "archive expands beyond budget")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(expanded), mode="r:") as archive:
        for entry in archive:
            require(entry.isfile() and entry.name not in files and len(files) < 1024,
                    "archive contains non-regular or duplicate members")
            path = Path(entry.name)
            require(not path.is_absolute() and ".." not in path.parts, "archive path escapes")
            require(entry.uid == entry.gid == entry.mtime == 0 and entry.uname == entry.gname == "",
                    "archive ownership or timestamp is not normalized")
            require(entry.mode == (0o755 if entry.name == "bin/aniflow" else 0o644), "archive mode drift")
            handle = archive.extractfile(entry)
            require(handle is not None, "archive file is unreadable")
            content = handle.read(MAX_TOTAL + 1)
            require(len(content) == entry.size and len(content) <= MAX_TOTAL, "invalid archive member size")
            files[entry.name] = content
    require("bin/aniflow" in files, "archive lacks executable")
    binary_identity(files["bin/aniflow"], target)
    return files


def spdx(metadata, version, revision, target, epoch, repository=ROOT):
    """Describe Cargo's target-filtered resolved graph; never claim OS completeness."""
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root_id = metadata["resolve"]["root"]
    require(root_id in nodes, "Cargo metadata lacks root dependency graph")
    wanted, pending = set(), [root_id]
    while pending:
        key = pending.pop()
        if key in wanted:
            continue
        wanted.add(key)
        pending.extend(dependency["pkg"] for dependency in nodes[key]["deps"])
    by_id = {package["id"]: package for package in metadata["packages"]}
    identifiers = {key: "SPDXRef-Package-" + digest((by_id[key]["name"] + "@" + by_id[key]["version"] +
                   "|" + (by_id[key]["source"] or "repository")).encode())[:24] for key in wanted}
    lock = tomllib.loads(regular_bytes(repository / "Cargo.lock").decode())
    checksums = {(p["name"], p["version"], p.get("source")): p.get("checksum") for p in lock["package"]}
    packages = []
    for key in sorted(wanted, key=lambda item: identifiers[item]):
        package = by_id[key]
        checksum = checksums.get((package["name"], package["version"], package["source"]))
        source = package["source"]
        require(key == root_id or source is not None, "undeclared local Cargo package cannot be released")
        record = {"SPDXID": identifiers[key], "name": package["name"], "versionInfo": package["version"],
                  "downloadLocation": "NOASSERTION", "filesAnalyzed": False,
                  "licenseConcluded": "NOASSERTION", "licenseDeclared": package["license"] or "NOASSERTION",
                  "copyrightText": "NOASSERTION"}
        if checksum:
            record["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        if source and source.startswith("registry+"):
            record["sourceInfo"] = source
        elif key == root_id:
            record["downloadLocation"] = "git+https://github.com/" + REPOSITORY + ".git@" + revision
        else:
            record["sourceInfo"] = source
        packages.append(record)
    relationships = [{"spdxElementId": "SPDXRef-DOCUMENT", "relationshipType": "DESCRIBES",
                      "relatedSpdxElement": identifiers[root_id]}]
    relationships.extend({"spdxElementId": identifiers[key], "relationshipType": "DEPENDS_ON",
                          "relatedSpdxElement": identifiers[dependency["pkg"]]}
                         for key in sorted(wanted) for dependency in nodes[key]["deps"])
    return {"spdxVersion": "SPDX-2.3", "dataLicense": "CC0-1.0", "SPDXID": "SPDXRef-DOCUMENT",
            "name": "aniflow-" + version + "-" + target,
            "documentNamespace": "https://github.com/" + REPOSITORY + "/sbom/" + revision + "/" + target,
            "creationInfo": {"created": datetime.datetime.fromtimestamp(epoch, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                             "creators": ["Tool: aniflow-release-v1"],
                             "comment": "Cargo target-filtered resolved graph, including build/dev dependencies. OS libraries, compiler, external media tools and models are not a complete transitive SBOM; toolchain observations are in build.json."},
            "packages": packages, "relationships": sorted(relationships, key=lambda item: json.dumps(item, sort_keys=True))}


def assemble(parts, output, version, revision, repository=ROOT):
    plan = verify_candidate(version, revision, repository)
    targets = [item["triple"] for item in plan["build"]["targets"]]
    require(sorted(path.name for path in parts.iterdir()) == sorted("native." + target for target in targets),
            "target set is incomplete or unexpected")
    collected, builds, packages, relationships = {}, [], {}, []
    for target in targets:
        part = parts / ("native." + target)
        records = verify_inventory(part)
        archive_name = "aniflow-" + version + "-" + target + ".tar.gz"
        require(set(records) == {archive_name, "build.json", "sbom.spdx.json", "SHA256SUMS"}, "unexpected target payload")
        build = read_json(part / "build.json")
        require(build["schema"] == "aniflow.release-build-evidence/v1" and build["version"] == version and
                build["source_revision"] == revision and build["target"] == target and
                build["reproducibility"]["state"] == "matched" and build["cli_smoke"] == "passed",
                "target evidence does not describe this successful candidate")
        raw = regular_bytes(part / archive_name)
        require(digest(raw) == build["archive_sha256"], "archive identity differs from build evidence")
        archived = archive_inventory(raw, target)
        require(digest(archived["bin/aniflow"]) == build["binary_sha256"], "binary identity differs from build evidence")
        require({name: digest(data) for name, data in archived.items()} == build["archive_members"], "archive member drift")
        require({name: data for name, data in archived.items() if name != "bin/aniflow"} ==
                {name: value[0] for name, value in payload_files(b"", repository).items() if name != "bin/aniflow"},
                "archive contracts/docs differ from represented source")
        collected[archive_name] = raw
        collected["targets/" + target + "/build.json"] = regular_bytes(part / "build.json")
        collected["targets/" + target + "/sbom.spdx.json"] = regular_bytes(part / "sbom.spdx.json")
        sbom = read_json(part / "sbom.spdx.json")
        require(sbom["spdxVersion"] == "SPDX-2.3" and sbom["documentNamespace"].endswith(revision + "/" + target),
                "target SBOM identity mismatch")
        for package in sbom["packages"]:
            identifier = package["SPDXID"]
            require(identifier not in packages or packages[identifier] == package, "target SBOM package disagreement")
            packages[identifier] = package
        relationships.extend(sbom["relationships"])
        builds.append(build)
    aggregate = dict(sbom)
    aggregate.update({"name": "aniflow-" + version, "documentNamespace": "https://github.com/" + REPOSITORY + "/sbom/" + revision,
                      "packages": [packages[key] for key in sorted(packages)],
                      "relationships": [json.loads(value) for value in sorted({json.dumps(value, sort_keys=True) for value in relationships})]})
    collected["sbom.spdx.json"] = json_bytes(aggregate)
    collected["provenance.json"] = json_bytes({"schema": "aniflow.binary-release/v1", "repository": REPOSITORY,
        "version": version, "source_revision": revision, "profile": "binary", "targets": targets,
        "artifact_digests": {name: digest(data) for name, data in collected.items() if name.endswith(".tar.gz")},
        "reproducibility": "two clean builds matched per target; cross-toolchain reproducibility is not claimed",
        "external_dependencies": plan["build"]["native_dependency_policy"], "delivery": plan["declaration"]["delivery"],
        "rollback": plan["declaration"]["evidence"]["rollback"]})
    for source, destination in [("CHANGELOG.md", "CHANGELOG.md"), ("docs/releases.md", "RELEASE.md"),
                                (".egohygiene/release.json", "release-declaration.json"),
                                (".egohygiene/release-policy-lock.json", "release-policy-lock.json")]:
        collected[destination] = regular_bytes(repository / source)
    output = new_directory(output, repository)
    for name, data in sorted(collected.items()):
        (output / name).parent.mkdir(parents=True, exist_ok=True)
        write_new(output / name, data)
    write_new(output / "payload-SHA256SUMS", checksum_bytes(inventory(output)))
    return {"state": "unsigned", "output": str(output), "targets": targets}


def verify_payload(bundle, version, revision):
    records = inventory(bundle)
    payload = {name: info for name, info in records.items()
               if name not in {"SHA256SUMS", "payload-SHA256SUMS", "signature.json", "attestation.sigstore.json"}}
    require(regular_bytes(bundle / "payload-SHA256SUMS") == checksum_bytes(payload), "signed payload inventory mismatch")
    provenance = read_json(bundle / "provenance.json")
    require(provenance["schema"] == "aniflow.binary-release/v1" and provenance["repository"] == REPOSITORY and
            provenance["version"] == version and provenance["source_revision"] == revision,
            "bundle source or version mismatch")
    expected = [item["triple"] for item in read_json(ROOT / ".egohygiene/release-build.json")["targets"]]
    require(provenance["targets"] == expected, "release target coverage drift")
    archives = {"aniflow-" + version + "-" + target + ".tar.gz" for target in expected}
    required = archives | {"provenance.json", "sbom.spdx.json", "CHANGELOG.md", "RELEASE.md", "release-declaration.json", "release-policy-lock.json"}
    required.update("targets/" + target + "/" + name for target in expected for name in ["build.json", "sbom.spdx.json"])
    require(set(payload) == required, "release payload inventory is not exact")
    require(provenance["artifact_digests"] == {name: records[name]["sha256"] for name in sorted(archives)}, "archive provenance drift")
    return provenance


def verify_signature(bundle, revision):
    # Verify the retained signature against the source and signer, not a supplied
    # 'verified' flag. This may retrieve Sigstore trust roots; no offline claim.
    return command(["gh", "attestation", "verify", bundle / "payload-SHA256SUMS",
        "--bundle", bundle / "attestation.sigstore.json", "--repo", REPOSITORY,
        "--signer-workflow", REPOSITORY + "/.github/workflows/release-candidate.yml",
        "--signer-digest", revision, "--source-digest", revision, "--source-ref", "refs/heads/main",
        "--deny-self-hosted-runners", "--format", "json"])


def seal(bundle, attestation, version, revision):
    verify_payload(bundle, version, revision)
    require(not any((bundle / name).exists() for name in ["SHA256SUMS", "signature.json", "attestation.sigstore.json"]),
            "bundle is already sealed or partially sealed; do not overwrite it")
    write_new(bundle / "attestation.sigstore.json", regular_bytes(attestation, 16 * 1024 * 1024))
    verify_signature(bundle, revision)
    write_new(bundle / "signature.json", json_bytes({"schema": "aniflow.release-signature/v1", "state": "verified",
        "method": "github-actions-sigstore", "subject": "payload-SHA256SUMS", "bundle": "attestation.sigstore.json",
        "source_revision": revision, "signer_workflow": REPOSITORY + "/.github/workflows/release-candidate.yml"}))
    write_new(bundle / "SHA256SUMS", checksum_bytes(inventory(bundle)))
    return {"state": "sealed", "source_revision": revision}


def verify_bundle(bundle, version, revision):
    verify_inventory(bundle)
    provenance = verify_payload(bundle, version, revision)
    signature = read_json(bundle / "signature.json")
    require(signature["state"] == "verified" and signature["source_revision"] == revision,
            "signature evidence is incomplete")
    verify_signature(bundle, revision)
    return {"state": "verified", "source_revision": revision, "version": version, "targets": provenance["targets"]}
