---
schema: aether.architecture-document/v1
id: aniflow-decisions
title: aniflow Decisions
kind: architecture-document
version: 0.1.12
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-11-10
governed_by:
  - architecture-decisions
depends_on:
  - aniflow-principles
  - aniflow-foundations
  - aniflow-epistemology
  - aniflow-system
  - aniflow-architecture
related:
  - aniflow-methodology
  - aniflow-roadmap
supersedes: []
---

# aniflow Decisions

## Purpose

This is the canonical index for significant aniflow decisions. Accepted
records preserve why durable boundaries and trade-offs exist without
duplicating their complete rationale. Proposed records are listed separately
and do not establish accepted policy.

## Decision governance

Use indexed ADR mode. Record a decision when it changes temporal semantics,
system ownership, dependency direction, public compatibility, processor or
checkpoint contracts, security posture, or another expensive-to-reverse
boundary. Implementation tasks remain in issues and PRs. A proposed ADR may
be indexed separately while awaiting explicit maintainer acceptance;
implementation or merge of feature code alone does not establish that
acceptance.

Merging an ADR pull request is the normal acceptance authority. The initial
records also reflect maintainer-approved architectural direction established on
2026-08-13. Later outcomes append to records rather than rewriting their
original context.

## Status model

Use `proposed`, `accepted`, `deprecated`, `superseded`, `rejected`, `withdrawn`,
or `historical`. Proposed records have no acceptance date and remain proposals
until the maintainer explicitly accepts the decision through normal ADR
review. Supersession links must exist in both directions and superseded
records remain discoverable.

## Decision index

| ID | Decision | Status | Accepted | Review trigger |
| --- | --- | --- | --- | --- |
| [aniflow-ADR-0001](decisions/ADR-0001-library-first-thin-cli.md) | Expose a public library behind a thin CLI | Accepted | 2026-08-13 | Independent package boundaries become necessary |
| [aniflow-ADR-0002](decisions/ADR-0002-polyrepo-independence.md) | Preserve polyrepo independence and move cross-tool orchestration to flow | Accepted | 2026-08-13 | A capability cannot be composed without domain leakage |
| [aniflow-ADR-0003](decisions/ADR-0003-typed-external-tool-ports.md) | Isolate external tools behind typed ports | Accepted | 2026-08-13 | A supported capability cannot fit the port safely |
| [aniflow-ADR-0004](decisions/ADR-0004-versioned-public-contracts.md) | Version public machine contracts | Accepted | 2026-08-13 | Compatibility costs materially exceed benefits |
| [aniflow-ADR-0005](decisions/ADR-0005-temporal-truth.md) | Model temporal truth instead of average-rate convenience | Accepted | 2026-08-13 | Fixtures show the target model cannot represent supported media |

## Proposed decisions

| ID | Proposal | Status | Review trigger |
| --- | --- | --- | --- |
| [aniflow-ADR-0006](decisions/ADR-0006-audio-analysis-foundation.md) | Bound normalized audio analysis to explicit source-relative evidence; #47 records loss-aware text conversion; #80 records native source-format inspection and signal evidence | Proposed; no acceptance recorded | Review with #42/#47; revisit before a later checkpoint expands payload, timing or review-authority semantics |
| [aniflow-ADR-0007](decisions/ADR-0007-provider-neutral-stem-import.md) | Import declared stem lineage through accepted provider evidence | Proposed; no acceptance recorded | Review with #45; revisit before partial selection, transformed mix mappings or relaxed import authority |
| [aniflow-ADR-0008](decisions/ADR-0008-layered-validation-acceptance.md) | Keep provider observations separate from component/stage/candidate-master/delivery acceptance, including #69's bounded member sets | Proposed; no acceptance recorded | Review #33/#69 together; revisit before new timing profiles, dynamic/optional membership or changes to cross-run reuse |
| [aniflow-ADR-0009](decisions/ADR-0009-owned-stage-cache.md) | Reuse owned stage outputs through current acceptance | Proposed; no acceptance recorded | Review with #34; revisit before shared caches, finer locks or validator reuse |
| [aniflow-ADR-0010](decisions/ADR-0010-plan-bound-toolchain-preflight.md) | Bind optional typed toolchain preflight to immutable plans and require it on resume | Proposed; no acceptance recorded | Review with #103; revisit before new adapter mappings, automatic setup or guard portability |
| [aniflow-ADR-0011](decisions/ADR-0011-serial-rife-derived-clock.md) | Bound RIFE interpolation with serial pairs, explicit resource admission and a derived clock | Proposed; no acceptance recorded | Review with #114/#110 before broader rates, parallel execution, fine checkpoints or final-video assembly |
| [aniflow-ADR-0012](decisions/ADR-0012-fixed-indexed-music-video-recipe.md) | Compose the fixed indexed music-video recipe through existing child runs, immutable intent and honest pending downstream plans | Proposed; no acceptance recorded | Review with #35/#107 before broader phases, mutable intent, concurrency, cross-recipe reuse or review/approval authority |

| [aniflow-ADR-0014](decisions/ADR-0014-reviewed-caption-candidates.md) | Bind selected SRT caption candidates to current rendered subset declarations while separating full creative review | Proposed; no acceptance recorded | Review with #108 before rich text, player typography, clocks or automated approval |
| [aniflow-ADR-0015](decisions/ADR-0015-detached-proof-candidates.md) | Retain opaque detached candidates and exact selected subjects in separate immutable roots | Proposed; no acceptance recorded | Review with #82 before proof parsing, network submission, upgrades or trusted verification |

| [aniflow-ADR-0018](decisions/ADR-0018-complete-spatial-carrier.md) | Compose complete spatial frames from current accepted historical/native origins without inventing history | Proposed; no acceptance recorded | Review with #37/#35 before mixed-carrier assembly, selected missing work or broader clock semantics |

## Evidence gaps and open questions

[ADR-0017](decisions/ADR-0017-selected-destination-video.md) proposes selected
destination geometry/encoding over current native acceptance, retaining original
clock/audio/master and separate quality/platform decisions. Proposed; no
acceptance recorded. Review before alternate codecs, audio transforms or clocks.

[ADR-0016](decisions/ADR-0016-historical-spatial-import.md) proposes current
native validation of selected historical spatial frames with a distinct
subset/schema and supplied import decision. Proposed; no acceptance recorded.
Review before mixed native/imported collection or automatic checkpoint adoption.

[ADR-0015](decisions/ADR-0015-detached-proof-candidates.md) proposes separate
offline retention of opaque detached candidates and exact retained subjects.
It is **Proposed; no acceptance recorded**. Review before network submission,
proof parsing/upgrades, trusted verification or stronger authenticity claims.

[ADR-0013](decisions/ADR-0013-local-destination-preparation.md) proposes local
destination preflight over accepted temporal-media facts and explicit requirement
declarations. It remains proposed, with no acceptance date; review with #83/#82
before adding transforms, platform calls, automatic approval or proof authority.

Crate decomposition beyond one library and binary, the exact timeline model,
and shared suite-contract adaptation remain intentionally undecided. They need
consumer or fixture evidence before new ADRs are accepted.

## Validation

Every index entry resolves to one canonical record with stable identity,
status, authority, context, decision, rationale, trade-offs, consequences,
review triggers, and lineage.

- [ADR-0019 — Exact final lossless audio QC](decisions/ADR-0019-final-lossless-audio-qc.md)
  remains proposed. It composes exact final native WAV/FLAC measurements over
  existing signal and meter owners without changing sample identity or granting
  musical/publication authority. The explicit resumable boundary and audio
  publishing v2 extension retain ordinary native checkpoint authority.
  Source inclusion is not ratification.

- [ADR-0020 — Recovered RIFE lineage](decisions/ADR-0020-recovered-rife-lineage.md)
  remains proposed. It binds complete recovered origins to the existing serial
  RIFE owner and distinct assembly carrier without relabeling historical work.

- [ADR-0021 — Explicit short deflicker comparison](decisions/ADR-0021-deflicker-comparison.md)
  remains proposed. Native preview integrity, supplied excerpt review and full
  creative/publication approval remain separate. Source merge is not ratification.

- [ADR-0022 — AMV evidence advisory](decisions/ADR-0022-amv-evidence-advisory.md) remains proposed. Record facts, unverified suggestions, current acceptance and human approval keep separate authority.

- [ADR-0023 — Fixed selectable release stages](decisions/ADR-0023-fixed-release-stages.md)
  remains proposed. Existing domain owners retain execution and acceptance;
  unavailable hooks never satisfy completion or downstream inputs.

- [ADR-0024 — Explicit audio goals and candidate review](decisions/ADR-0024-audio-delivery-goals.md) — proposed; source inclusion is not ratification.
