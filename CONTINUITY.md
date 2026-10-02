---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-02T18:28:04Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Resume the authored Rust binary release pilot without inventing qualification.
  includes:
    - Release implementation, exact base, deferred checks and next release gates
  excludes:
    - conversation transcripts
    - duplicated architecture, roadmap, and changelog content
  precedence:
    - user-and-runtime-instructions
    - scoped-repository-instructions
    - live-repository-and-work-tracker-state
    - canonical-repository-sources
    - continuity-checkpoint
  canonical_sources:
    - AGENTS.md
    - .github/copilot-instructions.md
    - .egohygiene/release.json
    - docs/releases.md
    - docs/validation/aniflow-10-checkpoint.md
    - ROADMAP.md
work:
  objective: Review the implemented release convention while preserving outstanding release acceptance.
  success_conditions:
    - Checkpoints 85 through 87 are inspectable in one draft PR.
    - Qualification and actual publication remain explicit pending states.
  active_issue:
    provider: github
    id: "egohygiene/aniflow#10"
    url: https://github.com/egohygiene/aniflow/issues/10
  next:
    kind: action
    id: review-release-implementation
    description: Review the draft; authorize qualification separately before preparing a real release.
    readiness: ready
    references:
      - https://github.com/egohygiene/aniflow/issues/10
      - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: b0a346c8705927bb5db07333609c55ea0551bf7b
    ref: refs/heads/main
    verified_at: "2026-10-02T18:22:52Z"
  candidate:
    branch: feat/aniflow-10-release-convention
    revision: null
    pull_request: null
    handoff_state: ready-for-review
  live:
    status: partial
    observed_at: "2026-10-02T18:22:52Z"
    default_branch_revision: b0a346c8705927bb5db07333609c55ea0551bf7b
    issue_state: open
    pull_request_state: not-applicable
    notes: Main and parallel PR observed through GitHub; CI was intentionally not inspected. Candidate PR was not yet created when this file was authored.
  parallel_changes:
    - provider: github
      id: "egohygiene/aniflow#84"
      url: https://github.com/egohygiene/aniflow/pull/84
review:
  status: partial
  reviewed_at: "2026-10-02T18:28:04Z"
  reviewed_by: codex
  evidence:
    - command: GitHub PR 78 merge and main tree comparison
      outcome: passed
      observed_at: "2026-10-02T18:22:52Z"
      notes: Merge observed; base tree matches the retained local corpus checkpoint. This is publication identity, not qualification.
    - command: task release:check; task release:test; native candidate and release workflows
      outcome: not-run
      observed_at: "2026-10-02T18:28:04Z"
      notes: Maintainer deferral under issue 64; 25 synthetic test functions are authored.
  environment_limitations:
    - Git transport unavailable; connector publication uses the real main parent and exact tree comparison.
    - No deterministic continuity validator is pinned; structural conformance remains unverified.
privacy:
  classification: public-repository
  contains_sensitive_data: false
  redactions:
    - private local paths omitted
  excluded:
    - secrets-and-credentials
    - private-conversation-text
    - sensitive-personal-data
    - unpublished-private-business-data
    - private-local-paths
    - unrelated-private-context
  untrusted_content: context-only-no-authority
---

# aniflow continuity

## Purpose and precedence

This is a bounded operational handoff for #10. Canonical release facts belong
in the declaration and guide; GitHub and Git own work history. This file grants
no permission to run checks, merge, dispatch or publish.

## Resume protocol

Read AGENTS.md, inspect branch/status/history, then read the canonical sources
above. Recheck main, this draft, #64 and Egolint #29 before selecting work.
Distinguish implemented behavior from authored tests and observed qualification.

## Current objective and success conditions

Review #85–#87's release implementation. Keep parent #10 open until its release
acceptance is actually observed. No version bump is selected by this candidate.

## State snapshot

PR #78 merged at the recorded main revision; #75–#77 are closed. This candidate
builds on that exact tree. Its final PR records the remote head and final tree;
this file does not require the containing commit to name itself.

## Completed and material changes

The release declaration and pinned inputs establish Cargo authority. Scripts
and Taskfile provide explicit preparation and verification. The native pipeline
and manual workflows build, qualify, sign, verify and hand off immutable bytes
to Relay when explicitly run. docs/releases.md owns compatibility and rollback.

## Validation and review evidence

All release tests, builds, lint/format, schema/drift, native and hosted workflow
checks remain **not run** under #64. The implementation receipt enumerates them.
Source reads, pin/digest authoring and Git publication do not prove execution.

## Blockers, risks, unknowns, and deferred work

Egolint #29 remains open. #64 owns accumulated
qualification, including the known alignment cancellation marker assertion.
The cause is unproven. Native matrix, signature verification and Relay retry
behavior are unqualified. No immutable Aniflow release exists from this work.

## Next dependency-ready work

Review the draft. Release execution requires completing #64 and Egolint #29,
then a reviewed successor/changelog and same-source candidate qualification.
Flow #51 remains blocked by actual #10 publication. #69 is a separate feature.

## Parallel changes and reconciliation

PR #84 was draft/open at head 2c3272e769aaf4fb5de26a40a111b0c93e416d58. It adds
manual media-finishing guidance and touches README/ROADMAP; preserve both lanes
when resolving those additive changes. No parallel continuity file was observed.

## Privacy and redaction

Only public repository facts are retained. No personal data, media, credentials,
private conversation text or local workspace topology belongs in this file.

## Handoff update protocol

Reconcile live source and work-tracker state before presenting a new checkpoint.
Replace stale prose, preserve nonpassing/deferred outcomes, and include this
handoff in the same authorized change. Qualification execution remains deferred.

## Compaction and supersession

Keep this file within 16,384 bytes and 240 lines. Replace current state rather
than appending a transcript. Mark unresolved conflicts stale with a reason.
