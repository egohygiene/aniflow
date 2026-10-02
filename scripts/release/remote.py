"""Read-only GitHub release gates. Mutations belong exclusively to Relay."""

import json
import os
import re

from .common import REPOSITORY, SHA, command, require
from .plan import verify_candidate


def api(path):
    return json.loads(command(["gh", "api", path]))


def guard(version, revision, candidate_run=None):
    plan = verify_candidate(version, revision)
    require(SHA.fullmatch(revision) is not None, "expected source must be a full Git SHA")
    require(os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch" and
            os.environ.get("GITHUB_REPOSITORY") == REPOSITORY and
            os.environ.get("GITHUB_REF") == "refs/heads/main" and os.environ.get("GITHUB_SHA") == revision,
            "release workflow must be manually dispatched on this repository's main branch")
    repo = api("repos/" + REPOSITORY)
    require(repo["default_branch"] == "main", "default branch changed; review release policy first")
    branch = api("repos/" + REPOSITORY + "/branches/main")
    require(branch["commit"]["sha"] == revision, "source is no longer the current default-branch head")
    for gate in plan["build"]["first_release_gates"]:
        issue = api("repos/" + gate["repository"] + "/issues/" + str(gate["issue"]))
        require(issue["state"] == "closed" and issue.get("state_reason") == "completed",
                "release qualification dependency remains incomplete: " + gate["repository"] + "#" + str(gate["issue"]))
    refs = api("repos/" + REPOSITORY + "/git/matching-refs/tags/" + version)
    for ref in refs:
        if ref["ref"] != "refs/tags/" + version:
            continue
        obj = ref["object"]
        for _ in range(4):
            if obj["type"] == "commit":
                break
            require(obj["type"] == "tag", "unsupported release ref")
            obj = api("repos/" + REPOSITORY + "/git/tags/" + obj["sha"])["object"]
        require(obj["type"] == "commit" and obj["sha"] == revision, "immutable tag belongs to another revision")
    if candidate_run is not None:
        require(re.fullmatch(r"[1-9][0-9]{0,19}", candidate_run) is not None, "candidate run must be a numeric run ID")
        run = api("repos/" + REPOSITORY + "/actions/runs/" + candidate_run)
        require(run["head_sha"] == revision and run["head_branch"] == "main" and
                run["event"] == "workflow_dispatch" and run["conclusion"] == "success" and
                run["path"].split("@")[0] == plan["build"]["candidate_workflow"],
                "candidate run is not a successful manual qualification of this source")
    return {"state": "authorized-inputs", "source_revision": revision, "candidate_run": candidate_run,
            "publication": "not-performed"}
