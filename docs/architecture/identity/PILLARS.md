---
schema: aether.architecture-document/v1
id: aniflow-pillars
title: Aniflow Pillars
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-pillars
depends_on:
  - aniflow-purpose
  - aniflow-vision
  - aniflow-principles
related:
  - aniflow-foundations
  - aniflow-roadmap
supersedes: []
---

# Aniflow Pillars

## Temporal fidelity

Aniflow preserves and validates the relationships among time, ordering,
streams, and reconstructed output. This pillar excludes ownership of a
processor's visual or audio algorithm.

## Deterministic and recoverable execution

Plans, isolated runs, atomic state, bounded work, cancellation, and compatible
checkpoints make expensive processing understandable and recoverable. This
pillar excludes distributed scheduling as an assumed requirement.

## Processor interoperability

Typed capabilities and narrow external boundaries let specialized processors
participate without coupling them to orchestration internals. This pillar does
not turn arbitrary command execution into an unvalidated plugin contract.

## Validation and evidence

Layered validation and traceable observations explain what Aniflow saw, did,
and accepted. This pillar does not make Aniflow the suite-wide provenance or
authenticity authority.

## Independent usability

A coherent library and CLI allow direct human use and composition by Flow or
other consumers. This pillar excludes direct dependencies on sibling holons.

## Relationships and health signals

Temporal fidelity defines correctness. Recoverable execution and processor
interoperability perform work without weakening it. Validation establishes
whether results qualify, and independent usability exposes those semantics.

Health is demonstrated by temporal fixtures, deterministic invalidation,
library/CLI contract parity, safe cancellation, validated masters, and external
consumer tests. These are evidence signals rather than release dates.

## Assumptions and open questions

The five-pillar set reflects both implemented capabilities and accepted v1
direction. Operational experience may reveal whether storage-awareness merits
its own pillar or remains part of recoverable execution.

## Validation

Roadmap initiatives map to at least one pillar without redefining the pillar
around a temporary tool or feature.
