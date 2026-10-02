---
schema: aether.architecture-decision/v1
id: aniflow-adr-0008
title: Keep provider observations separate from layered acceptance
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
  - aniflow-adr-0005
  - aniflow-architecture
  - aniflow-roadmap
---

# ADR-0008 — Keep provider observations separate from layered acceptance

## Status and authority

Proposed for maintainer review with #33. There is no acceptance date.
Implementation, test execution or feature merge does not independently accept
this architectural decision.

## Context

The existing ordered Pipeline v3 executor binds exact providers and observes
artifact integrity, but provider exit and file presence do not establish media
correctness or final delivery. An ordinary artifact-validator stage can emit
JSON without becoming an acceptance authority. Flow needs stable evidence
without importing private scheduler or workspace logic.

## Proposed decision

Resolve explicit validation obligations into exact inline provider locks during
read-only planning. Execute validators inside the producing stage's acceptance
boundary through the existing bounded direct-argument ABI. Observational
providers emit versioned context-bound reports; aniflow independently checks
identity, lineage, provenance and supported temporal invariants before deciding
acceptance. Models and content-changing providers cannot act as gate authority.

Persist content-addressed component, stage, candidate-master and delivery
records. Checkpoints and complete manifests reference the graph. Resume and
read-only status verify the graph and current workspace bytes; no marker,
process exit or provider-supplied accepted flag is sufficient.

Retain mandatory built-in integrity and make extra validation explicit. Limit
the initial temporal profile to #32's admitted exact CFR/zero-origin domain.
Keep one artifact per output port; #69 owns the bounded multi-artifact extension.
Pipeline v2 keeps its compatibility behavior. Cross-run reuse remains #34.

## Rationale and alternatives

Inline gates preserve the ordered scheduler and its checkpoint ownership.
Turning every gate into an ordinary stage would complicate completion and
permit evidence to be confused with acceptance. Trusting a provider boolean
would discard the independently checkable relationship to input/output bytes,
the plan and exact execution identity. Mandatory generic media validation would
claim unsupported semantics for nonmedia artifacts, so additional profiles are
explicit obligations.

## Compatibility and consequences

Optional transport fields preserve historical v1 document readability. New
acceptance-semantics identity prevents historical checkpoints from becoming
current accepted delivery without compatible reexecution. Complete status now
requires the complete evidence graph and matching workspace artifacts. Status
cannot re-read original external inputs because their paths are not durable
identity; run/resume require fresh input bindings.

The evidence graph costs storage and hashing work. An unavailable required
validator blocks acceptance rather than downgrading the obligation. Missing
records may be rebuilt on explicit resume, while conflicting immutable records
are never overwritten. Schema conformance alone is insufficient: cross-record
identity, cardinality, temporal and reference checks belong to the public Rust
acceptance/runtime boundary.

## Security and privacy impact

Source media remains immutable; decode uses private snapshots and explicit
pinned tools. Validators inherit bounded execution, cancellation and output
confinement, without gaining network or publish authority. Declarations and
digests do not sandbox or authenticate local executables. Stronger executable
isolation, authenticity and adversarial qualification remain separate concerns.

## Evidence and review triggers

Synthetic fixtures and schema examples are authored for success, refusal,
cancellation, timeout, tampering and reuse; none has run in this pass under the
maintainer's #64 deferral. The receipt preserves this limitation.

Review before accepting new timing profiles, probabilistic validation,
multi-artifact ports, mutable delivery, cross-run reuse or broader platform
claims. Keep #24 adversarial work and #10 release qualification separate.

## Related artifacts

- [Layered validation guide](../../../layered-validation.md)
- [Implementation specification](../../../specs/layered-validation.md)
- [Checkpoint receipt](../../../validation/aniflow-33-checkpoint.md)
- [Public contracts](../../../contracts/README.md)
- [Flow integration boundary](../../../integrations/flow.md)
