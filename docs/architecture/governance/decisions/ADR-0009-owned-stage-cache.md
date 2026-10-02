---
schema: aether.architecture-decision/v1
id: aniflow-adr-0009
title: Reuse owned stage outputs through current acceptance
kind: architecture-decision
status: proposed
owners:
  - egohygiene
scope:
  - aniflow
governed_by:
  - architecture-decisions
supersedes: []
superseded_by: []
related:
  - aniflow-adr-0003
  - aniflow-adr-0004
  - aniflow-adr-0008
  - aniflow-architecture
  - aniflow-roadmap
---

# ADR-0009 — Reuse owned stage outputs through current acceptance

## Status and authority

Proposed with #34. Implementation and merge do not independently accept the
proposal. No acceptance date or executed qualification is recorded.

## Context

Run-local checkpoints cannot amortize expensive producers across isolated runs.
#33 validation evidence binds exact plan and invocation context. Moving old
acceptance documents to a new plan would falsely imply current validation.
Cleanup and shared writers also introduce ownership and partial-publication
risks that ordinary per-run output directories do not address.

## Proposed decision

Introduce an explicit owned local cache behind the public library, with optional
run/resume configuration and versioned machine contracts. Cache only producers
whose provider contracts allow reuse. Separate the semantic lookup key from the
sealed accepted-result entry. Bind both to exact source, provider, dependency,
validation and execution identity, while excluding run paths and timestamps from
the lookup key.

Verify retained origin proof before copying bytes to new run-owned inodes. Keep
producer reports unchanged and run current validators before current-plan
acceptance. Persist reuse decisions and pending rerun obligations. Original
acceptance can be inspected, but cannot simply be re-signed for another plan.

Use conservative namespace-wide exclusive writers and separate workspace locks.
Publish sealed snapshots with same-filesystem rename. Preflight free storage and
explicit limits; inspect is read-only, invalidation is explicit, prune is bounded
and defaults to preview. Unknown or incomplete data is never adopted for cleanup.
Crash locks require operator reconciliation instead of guessed PID recovery.

## Consequences and alternatives

Current validators incur cost on cache hits; producer reuse still avoids the
expensive transformation. Whole-plan keys would unnecessarily invalidate
independent stages. Rewriting provider reports would falsify historical process
evidence. A distributed cache, per-entry leases and automatic stale-lock cleanup
would broaden the concurrency/security boundary beyond this checkpoint.

Coarse locks serialize runs sharing a namespace and fail immediately on
contention. Fresh copies consume storage but prevent a caller editing run output
from mutating the cache. Unix storage observation has an explicit unsupported
boundary elsewhere; platform support must follow actual qualification.

## Review triggers

Revisit before shared remote caches, concurrent per-entry writers, automatic
crash recovery, alternate native storage backends, validator-result reuse across
plans, or multi-artifact ports (#69). Validate normalized outcomes, refusal,
source immutability, retention and lock behavior during #64.
