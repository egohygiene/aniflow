---
schema: aether.architecture-document/v1
id: aniflow-methodology
title: aniflow Methodology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-methodology
depends_on:
  - aniflow-purpose
  - aniflow-vision
  - aniflow-principles
  - aniflow-foundations
  - aniflow-architecture
related:
  - aniflow-decisions
  - aniflow-roadmap
supersedes: []
---

# aniflow Methodology

## Operating method

aniflow evolves through small, evidence-led increments that preserve a working
media vertical slice while replacing weak contracts deliberately. Architecture,
fixtures, implementation, and public documentation advance together.

## Development loop

### Observe

Inspect repository behavior, real media constraints, external tool contracts,
and failure evidence. Distinguish current behavior from intended architecture.

### Specify

Define domain meaning, compatibility, safety, supported and rejected cases, and
objective exit evidence before changing a public contract.

### Plan

Choose the smallest coherent pull-request boundary, identify migration effects,
and keep unrelated behavior outside the change.

### Implement

Move behavior behind the accepted inward dependency direction. Preserve the
working vertical slice and add abstractions only where a real boundary requires
them.

### Validate

Use the narrowest tests first, then contract, fixture, failure-injection, and
synthetic end-to-end validation. Validate outputs and state, not just process
exit.

### Review

Compare implementation, architecture, schemas, examples, security implications,
and user-facing behavior. Keep limitations and disagreement visible.

### Publish and learn

Merge only with named exit evidence. Record compatibility and decisions,
publish an appropriate pre-1.0 increment, exercise it from a real consumer, and
feed discoveries into the next specification.

## Pull-request method

- Keep each pull request independently reviewable and safely revertible.
- Separate behavior-preserving structural work from semantic changes when doing
  so materially improves review.
- Add tests with the contract they protect rather than deferring quality to a
  final hardening pass.
- Change schemas and machine output only with explicit versions and migration.
- Update architecture and decisions when ownership or invariants change.
- Preserve historical release specifications rather than rewriting them as the
  target state.
- Use real or redistribution-safe fixtures for temporal claims.

## Validation loops

| Loop | Evidence |
| --- | --- |
| Local | format, lint, unit, contract, and targeted fixture results |
| Integration | deterministic synthetic media and external-adapter simulations |
| Temporal | CFR, VFR, stream-selection, subtitle, audio, interruption, and resume fixtures |
| Consumer | independent Rust example, CLI script, and later flow adapter |
| Release | clean install, package contents, compatibility, checksums, and documented smoke run |

## Human and AI collaboration

Humans retain product authority, approve significant decisions, and review
public behavior. AI contributors may inspect, draft, implement, and validate,
but must cite repository evidence, preserve uncertainty, avoid expanding scope,
and never treat generated prose as proof.

## Feedback and revision

Failures become regression fixtures when reproducible. Repeated duplication may
justify an abstraction; anticipated duplication does not. An ADR is reviewed
when its trigger occurs, and roadmap horizons advance on exit evidence rather
than elapsed time.

## Assumptions and open questions

The current CI demonstrates one Linux synthetic path. The methodology expects a
broader evidence matrix as contracts mature, but does not claim those fixtures
exist yet.

## Validation

A contributor can follow this method without private conversation history, and
each roadmap increment names the evidence required to advance.
