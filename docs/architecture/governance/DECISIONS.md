---
schema: aether.architecture-document/v1
id: aniflow-decisions
title: Aniflow Decisions
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
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

# Aniflow Decisions

## Purpose

This is the canonical index for significant accepted Aniflow decisions. It
preserves why durable boundaries and trade-offs exist without duplicating their
complete rationale.

## Decision governance

Use indexed ADR mode. Record a decision when it changes temporal semantics,
system ownership, dependency direction, public compatibility, processor or
checkpoint contracts, security posture, or another expensive-to-reverse
boundary. Proposals and implementation tasks remain outside this log.

Merging an ADR pull request is the normal acceptance authority. The initial
records also reflect maintainer-approved architectural direction established on
2026-08-13. Later outcomes append to records rather than rewriting their
original context.

## Status model

Use `accepted`, `deprecated`, `superseded`, `rejected`, `withdrawn`, or
`historical`. Supersession links must exist in both directions and superseded
records remain discoverable.

## Decision index

| ID | Decision | Status | Accepted | Review trigger |
| --- | --- | --- | --- | --- |
| [ANIFLOW-ADR-0001](decisions/ADR-0001-library-first-thin-cli.md) | Expose a public library behind a thin CLI | Accepted | 2026-08-13 | Independent package boundaries become necessary |
| [ANIFLOW-ADR-0002](decisions/ADR-0002-polyrepo-independence.md) | Preserve polyrepo independence and move cross-tool orchestration to Flow | Accepted | 2026-08-13 | A capability cannot be composed without domain leakage |
| [ANIFLOW-ADR-0003](decisions/ADR-0003-typed-external-tool-ports.md) | Isolate external tools behind typed ports | Accepted | 2026-08-13 | A supported capability cannot fit the port safely |
| [ANIFLOW-ADR-0004](decisions/ADR-0004-versioned-public-contracts.md) | Version public machine contracts | Accepted | 2026-08-13 | Compatibility costs materially exceed benefits |
| [ANIFLOW-ADR-0005](decisions/ADR-0005-temporal-truth.md) | Model temporal truth instead of average-rate convenience | Accepted | 2026-08-13 | Fixtures show the target model cannot represent supported media |

## Evidence gaps and open questions

Crate decomposition beyond one library and binary, the exact timeline model,
and shared suite-contract adaptation remain intentionally undecided. They need
consumer or fixture evidence before new ADRs are accepted.

## Validation

Every index entry resolves to one canonical record with stable identity,
status, authority, context, decision, rationale, trade-offs, consequences,
review triggers, and lineage.
