"""Read one Cargo authority; prepare only an explicitly chosen successor."""

from __future__ import annotations

import datetime
from pathlib import Path
import re
import tomllib

from .common import ROOT, TAG, SHA, REPOSITORY, clean_revision, digest, json_bytes, no_links, read_json, regular_bytes, require

DECLARATION = ".egohygiene/release.json"
CANDIDATE = ".egohygiene/release-candidate.json"
HEADING = re.compile(r"^## \[([0-9]+\.[0-9]+\.[0-9]+)\] - (\d{4}-\d{2}-\d{2})$", re.MULTILINE)


def version_tuple(version):
    match = TAG.fullmatch(version)
    require(match is not None, "version must be exact vMAJOR.MINOR.PATCH")
    return tuple(int(value) for value in match.groups())


def check(repository=ROOT):
    # Full schema validation is required; missing dependencies are not a pass.
    from jsonschema import Draft202012Validator

    lock = read_json(repository / ".egohygiene/release-policy-lock.json")
    require(lock["schema"] == "aniflow.release-policy-lock/v1", "unsupported policy lock")
    for source in lock["sources"]:
        path = Path(source["local_path"])
        require(not path.is_absolute() and ".." not in path.parts, "unsafe policy snapshot path")
        require(digest(regular_bytes(repository / path)) == source["sha256"], "policy snapshot drift")
        require(SHA.fullmatch(source["revision"]) is not None, "policy revision is not immutable")
    declaration = read_json(repository / DECLARATION)
    Draft202012Validator(read_json(repository / "third_party/release/aether/schema.json")).validate(declaration)
    require(declaration["repository"] == {"id": REPOSITORY, "lifecycle": "active", "release_profile": "cli-library"},
            "unexpected repository release profile")
    require(declaration["release"]["state"] != "frozen", "frozen repositories cannot prepare releases")
    channels = [channel for channel in declaration["delivery"]["channels"] if channel.get("relay_profile") == "binary"]
    require(len(channels) == 1 and channels[0]["kind"] == "github-release" and channels[0]["state"] == "configured",
            "exactly one configured Relay binary channel is required")
    require(declaration["components"] == [{"id": "aniflow", "kind": "crate", "version_authority": {
        "kind": "cargo-manifest", "path": "Cargo.toml", "selector": "package.version"}}],
        "Cargo.toml package.version must be the sole aniflow authority")
    package = tomllib.loads(regular_bytes(repository / "Cargo.toml").decode())["package"]
    version = "v" + package["version"]
    version_tuple(version)
    require(package["name"] == "aniflow", "unexpected Cargo package")
    cargo_lock = tomllib.loads(regular_bytes(repository / "Cargo.lock").decode())
    roots = [p for p in cargo_lock["package"] if p["name"] == "aniflow" and "source" not in p]
    require(len(roots) == 1 and roots[0]["version"] == package["version"], "Cargo.lock version drift")
    text = regular_bytes(repository / "CHANGELOG.md").decode("utf-8")
    require(text.count("## [Unreleased]\n") == 1, "retain exactly one Unreleased heading")
    promoted = HEADING.findall(text)
    require(len({v for v, _ in promoted}) == len(promoted), "duplicate changelog versions")
    for entry, date in promoted:
        version_tuple("v" + entry)
        datetime.date.fromisoformat(date)
    require(promoted and promoted[0][0] == version[1:], "Cargo and latest promoted changelog version drift")
    taskfile = regular_bytes(repository / "Taskfile.yml").decode()
    for task in declaration["automation"]["tasks"].values():
        require("  " + task + ":\n" in taskfile, "missing standard release task: " + task)
    workflow = regular_bytes(repository / declaration["automation"]["github"]["workflow_path"]).decode()
    require("  workflow_dispatch:" in workflow, "manual publication workflow is missing")
    require(not re.search(r"^  (push|pull_request|schedule):", workflow, re.MULTILINE),
            "publication must have a manual trigger only")
    build = read_json(repository / ".egohygiene/release-build.json")
    require(build["schema"] == "aniflow.release-build/v1", "unsupported build contract")
    require(build["targets"] == [{"triple": "x86_64-unknown-linux-gnu", "runner": "ubuntu-24.04"},
                                  {"triple": "aarch64-apple-darwin", "runner": "macos-15"}],
            "release matrix requires a reviewed native adapter change")
    relay = [source for source in lock["sources"] if source["repository"] == "egohygiene/relay"]
    require(len(relay) == 1 and build["relay_revision"] == relay[0]["revision"] and
            "semantic-release.yml@" + build["relay_revision"] in workflow, "Relay publication pin drift")
    for filename in ["release.yml", "release-candidate.yml", "release-check.yml"]:
        content = regular_bytes(repository / ".github/workflows" / filename).decode()
        for reference in re.findall(r"^\s*(?:- )?uses:\s*([^\s#]+)", content, re.MULTILINE):
            require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}", reference) is not None,
                    "release workflow action must be pinned to a full SHA")
    if (repository / CANDIDATE).exists():
        candidate = read_json(repository / CANDIDATE)
        require(candidate == {"schema": "aniflow.release-candidate/v1", "version": version,
                              "date": candidate.get("date"), "component": "aniflow"},
                "release candidate version or shape drift")
        require(promoted and promoted[0] == (version[1:], candidate["date"]),
                "Cargo, candidate and latest promoted changelog must agree")
    return {"schema": "aniflow.release-plan/v1", "version": version, "declaration": declaration,
            "candidate": read_json(repository / CANDIDATE) if (repository / CANDIDATE).exists() else None,
            "publication": "not-performed", "qualification": "not-established",
            "build": build}


def unreleased_body(text):
    return text.split("## [Unreleased]\n", 1)[1].split("\n## [", 1)[0]


def prepare(version, date, repository=ROOT):
    plan = check(repository)
    clean_revision(repository=repository)
    require(version_tuple(version) > version_tuple(plan["version"]), "prepare requires an explicit successor version")
    datetime.date.fromisoformat(date)
    require(re.fullmatch(r"\d{4}-\d{2}-\d{2}", date) is not None, "date must be YYYY-MM-DD")
    changelog_path = repository / "CHANGELOG.md"
    text = regular_bytes(changelog_path).decode()
    require(re.search(r"^[-*] \S", unreleased_body(text), re.MULTILINE) is not None,
            "Unreleased has no reviewed entries to promote")
    require(not any(v == version[1:] for v, _ in HEADING.findall(text)), "version already in changelog")
    manifest = regular_bytes(repository / "Cargo.toml").decode()
    manifest, count = re.subn(r'(?m)^(version = ")[^"]+("\s*)$',
                             lambda m: m.group(1) + version[1:] + m.group(2), manifest, count=1)
    require(count == 1, "unsupported Cargo manifest version layout")
    lock_path = repository / "Cargo.lock"
    cargo_lock = regular_bytes(lock_path).decode()
    cargo_lock, count = re.subn(r'(\[\[package\]\]\nname = "aniflow"\nversion = ")[^"]+("\n)',
                               lambda m: m.group(1) + version[1:] + m.group(2), cargo_lock)
    require(count == 1, "unsupported Cargo.lock root version layout")
    changes = {
        "Cargo.toml": manifest.encode(), "Cargo.lock": cargo_lock.encode(),
        "CHANGELOG.md": text.replace("## [Unreleased]\n", "## [Unreleased]\n\n## [" + version[1:] + "] - " + date + "\n", 1).encode(),
        CANDIDATE: json_bytes({"schema": "aniflow.release-candidate/v1", "version": version, "date": date, "component": "aniflow"}),
    }
    # All inputs preflight before mutation. An interrupted write remains an
    # inspectable dirty tree and cannot pass verify; never commit or tag here.
    for path, data in changes.items():
        no_links(repository / path).write_bytes(data)
    return {"prepared_version": version, "changed_paths": sorted(changes), "next": "review diff and open a release preparation PR"}


def verify_candidate(version, source_revision, repository=ROOT):
    plan = check(repository)
    require(plan["candidate"] is not None, "prepare and review a successor release first")
    require(version == plan["version"], "requested tag disagrees with Cargo authority")
    clean_revision(source_revision, repository)
    body = unreleased_body(regular_bytes(repository / "CHANGELOG.md").decode())
    require(re.search(r"^[-*] \S", body, re.MULTILINE) is None, "unpromoted changes remain in Unreleased")
    return plan
