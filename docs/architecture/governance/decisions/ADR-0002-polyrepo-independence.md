---
schema: aether.architecture-decision/v1
id: aniflow-adr-0002
title: Preserve polyrepo independence and move cross-tool orchestration to Flow
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

# ADR-0002 — Preserve polyrepo independence and move cross-tool orchestration to Flow

## Context

Aniflow v0.2.0 contains an optional Renderflow handoff in its pipeline, run
workspace, README, and delivery manifest. The suite direction now keeps Aniflow,
Optiflow, Renderflow, and Flow in separate repositories with independent release
lifecycles. Direct sibling selection would create a chain of domain dependencies
and duplicate Flow's orchestration responsibility.

## Decision

Aniflow ends at a validated temporal master and Aniflow-owned run evidence. It
does not depend on, select, configure, or invoke Flow, Optiflow, or Renderflow in
the target architecture.

Flow may consume Aniflow's public library or CLI and coordinate its master with
sibling capabilities. The pipeline v2 Renderflow handoff remains documented as
a deprecated compatibility seam and will be removed from pipeline v3 through an
explicit migration.

## Rationale

Independent repositories preserve clear domain ownership, focused releases,
standalone usability, and acyclic package dependencies. A top-level orchestrator
can compose tools without forcing each tool to understand the suite.

## Evidence and assumptions

Observed: Aniflow already produces a master and delivery manifest without
requiring Renderflow, and the handoff defaults to disabled. Decided: the four
tools remain independent repositories and Flow owns cross-tool orchestration.
Assumed: Aniflow's public result will contain sufficient data for a Flow adapter;
that assumption must be tested before v1.

## Alternatives considered

- Aniflow depends on Renderflow: convenient for one sequence but couples release
  cycles and makes Aniflow choose downstream policy.
- Renderflow depends on Aniflow: reverses the coupling without solving
  orchestration ownership.
- Merge all tools into one repository and workspace: simplifies local linking
  but weakens independent lifecycle and was rejected by current suite direction.

## Trade-offs

Flow needs adapters and compatibility tests. Users who want a one-command suite
workflow use Flow rather than enabling a convenience field inside Aniflow.

## Expected consequences

Pipeline v3 has no Renderflow section. Aniflow release metadata publishes a
stable consumable boundary. Flow declares compatible Aniflow versions and owns
cross-tool provenance and sequencing.

## Security, privacy, and accessibility impact

Removing implicit downstream invocation reduces executable authority and data
exposure. Flow must request any further transformation explicitly.

## Observed outcomes

Architecture ownership is now acyclic; implementation migration is pending.

## Review triggers

Review only if a required capability cannot be composed through a public
artifact and result contract without leaking temporal-domain responsibility.

## Related artifacts

`aniflow-foundations`, `aniflow-architecture`, and the external Flow suite
architecture.

## Validation

Dependency checks show no Aniflow crate or runtime dependency on sibling tools,
and a Flow consumer test composes the released interface externally.
