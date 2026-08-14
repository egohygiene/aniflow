---
schema: aether.architecture-decision/v1
id: aniflow-adr-0002
title: Preserve polyrepo independence and move cross-tool orchestration to flow
kind: architecture-decision
status: accepted
accepted: 2026-08-13
owners:
  - egohygiene
scope:
  - aniflow
  - flow-suite-boundary
governed_by:
  - architecture-decisions
supersedes: []
superseded_by: []
related:
  - aniflow-architecture
  - aniflow-foundations
---

# ADR-0002 — Preserve polyrepo independence and move cross-tool orchestration to flow

## Context

aniflow v0.2.0 contains an optional renderflow handoff in its pipeline, run
workspace, README, and delivery manifest. The suite direction now keeps aniflow,
optiflow, renderflow, and flow in separate repositories with independent release
lifecycles. Direct sibling selection would create a chain of domain dependencies
and duplicate flow's orchestration responsibility.

## Decision

aniflow ends at a validated temporal master and aniflow-owned run evidence. It
does not depend on, select, configure, or invoke flow, optiflow, or renderflow in
the target architecture.

flow may consume aniflow's public library or CLI and coordinate its master with
sibling capabilities. The pipeline v2 renderflow handoff remains documented as
a deprecated compatibility seam and will be removed from pipeline v3 through an
explicit migration.

## Rationale

Independent repositories preserve clear domain ownership, focused releases,
standalone usability, and acyclic package dependencies. A top-level orchestrator
can compose tools without forcing each tool to understand the suite.

## Evidence and assumptions

Observed: aniflow already produces a master and delivery manifest without
requiring renderflow, and the handoff defaults to disabled. Decided: the four
tools remain independent repositories and flow owns cross-tool orchestration.
Assumed: aniflow's public result will contain sufficient data for a flow adapter;
that assumption must be tested before v1.

## Alternatives considered

- aniflow depends on renderflow: convenient for one sequence but couples release
  cycles and makes aniflow choose downstream policy.
- renderflow depends on aniflow: reverses the coupling without solving
  orchestration ownership.
- Merge all tools into one repository and workspace: simplifies local linking
  but weakens independent lifecycle and was rejected by current suite direction.

## Trade-offs

flow needs adapters and compatibility tests. Users who want a one-command suite
workflow use flow rather than enabling a convenience field inside aniflow.

## Expected consequences

Pipeline v3 has no renderflow section. aniflow release metadata publishes a
stable consumable boundary. flow declares compatible aniflow versions and owns
cross-tool provenance and sequencing.

## Security, privacy, and accessibility impact

Removing implicit downstream invocation reduces executable authority and data
exposure. flow must request any further transformation explicitly.

## Observed outcomes

Architecture ownership is now acyclic; implementation migration is pending.

## Review triggers

Review only if a required capability cannot be composed through a public
artifact and result contract without leaking temporal-domain responsibility.

## Related artifacts

`aniflow-foundations`, `aniflow-architecture`, and the external flow suite
architecture.

## Validation

Dependency checks show no aniflow crate or runtime dependency on sibling tools,
and a flow consumer test composes the released interface externally.
