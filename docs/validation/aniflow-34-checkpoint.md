# aniflow #34 implementation handoff

The maintainer requested #34 after merging #33 / PR #70. The implementation,
synthetic coverage and consumer documentation are ready for draft review.
Qualification remains deferred under #64. This handoff does not authorize a
merge or assert that the original #34 acceptance criteria have passed.

| Checkpoint | Identity |
| --- | --- |
| Base / merged #33 | `6a63bdbe6dd294e549a5addf2413ac4d264edd14` |
| Branch | `feat/aniflow-34-cache-controls` |
| Contracts and owned storage (#71) | `bdd40fb` |
| Reuse and persistent rerun frontier (#72) | `5e71693` |
| Operations and authored coverage (#73) | `f7b97def0f2150df26d91c5af0cb3a1714471eb3` |
| Implementation tree | `fc1df770ecbdf8d7a63088f118847b0d803fff4a` |

These are local checkpoint identities. A subsequent handoff commit adds this
document, the [structured receipt](aniflow-34-implementation.json), and the
[PR description](aniflow-34-pr.md). Publication can use one commit containing the
exact final local tree; the draft PR records that published commit/tree.

## Delivered implementation

- #71: versioned cache policy, semantic keys, sealed entries, explicit owner
  namespaces, bounded inventory, exclusive locks and atomic publication.
- #72: opt-in cross-run producer reuse with origin proof verification, fresh
  output inodes and current validator execution; targeted rerun frontiers that
  survive interruption and exclude invalidated historical checkpoints.
- #73: public library and CLI inspection, invalidation and preview/apply pruning;
  storage preflight, retention, typed diagnostics, decision receipts, closed
  schemas, synthetic coverage and Flow guidance.

See the [guide](../cache-reuse.md), [specification](../specs/content-addressed-reuse.md),
[Flow contract](../integrations/flow.md) and proposed
[ADR-0009](../architecture/governance/decisions/ADR-0009-owned-stage-cache.md).
Current acceptance remains authoritative; cache receipts only explain reuse.
Pipeline v2 and provider-local model caches are outside this implementation.

## Qualification and limits

**No tests, builds, formatting, lint, schema checks, smoke, packaging,
stable/MSRV or hosted-CI qualification ran during this pass.** The 20 authored
Rust tests and four authored Python schema tests are not passing evidence.
All fixtures are synthetic; no real-media or native-platform qualification is
claimed. The later focused entry point is `task cache:conformance`.

Coverage is authored for compatible reuse and current validation, identity
changes, output isolation, corruption, expiry, explicit invalidation, bounded
pruning, readonly inspection, contention, cancellation, conflicting same-key
results, storage refusal and interrupted reruns. The accumulated #64 audit must
also qualify existing v3/layered validation behavior and preserve the previous
alignment cancellation failure until it is investigated.

Cache writes currently require Unix storage preflight. Coarse namespace locks
serialize writers; crash locks are never stolen automatically. Abandoned
temporary work and ownership-corrupt entries need operator reconciliation.
Physical free-space checks are not quotas. ADR-0009 remains proposed. Multiple
artifacts on one output port remain the separate #69 follow-up.

## Review and continuation

Publish one draft PR, link #34 and #71–#73, and append this exact unqualified
checkpoint to #64 without claiming acceptance or inspecting CI for this pass.
Return to the maintainer; merge only after explicit authorization.

The ordered queue after #34 is bounded #24 adversarial-corpus work, then #10
release conventions, then Flow #51 subject to its current dependencies. Preserve
the #64 deferral unless the maintainer changes it; do not silently start broad
qualification, real-media work, #69 or the next product issue.
