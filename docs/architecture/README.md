---
schema: aether.architecture-document/v1
id: aniflow-architecture-index
title: aniflow Architecture Index
kind: architecture-document
version: 0.1.3
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-10-27
governed_by:
  - architecture-document
depends_on: []
related:
  - aniflow-purpose
  - aniflow-roadmap
supersedes: []
---

# aniflow Architecture Index

This directory is the canonical architecture record for aniflow. It defines
the product's identity, domain language, system ownership, internal dependency
rules, working method, and significant accepted decisions.

The original baseline was grounded in package v0.2.0 at revision
`20783d7374298e5fd44c782348a3c944c9318656` on 2026-08-13. That is historical
context. The current source reconciliation uses main
`2add9a6b230aaf415288e6c128bcf276dc05681c` on 2026-10-27, with selective
repair and per-frame Upscayl recovery merged. This #38 checkpoint builds on
named repair draft #128 and adds inert allenk/Upscayl setup mapping. Separate
draft PR #127 contains Upscayl chunks. Source package 0.3.0 remains unreleased, with execution
qualification deferred to #64.
Begin with the [current roadmap](../../ROADMAP.md#current-source-and-work-order),
[system inventory](foundation/SYSTEM.md) and
[implementation boundaries](foundation/ARCHITECTURE.md). The complete open-work
ledger is [#117](https://github.com/egohygiene/aniflow/issues/117).

Each decision retains its recorded proposed/accepted status. Authored code,
source merge, executed qualification and release publication are separate facts;
the historical baseline is not evidence that every later proposal is accepted.

## Document graph

Read in dependency order:

1. [Purpose](identity/PURPOSE.md)
2. [Vision](identity/VISION.md)
3. [Principles](identity/PRINCIPLES.md)
4. [Pillars](identity/PILLARS.md)
5. [Manifesto](identity/MANIFESTO.md)
6. [Epistemology](meta/EPISTEMOLOGY.md)
7. [AI constitution](meta/AI_CONSTITUTION.md)
8. [Ontology](domain/ONTOLOGY.md)
9. [Personal model](domain/PERSONAL_MODEL.md)
10. [Foundations](foundation/FOUNDATIONS.md)
11. [System](foundation/SYSTEM.md)
12. [Architecture](foundation/ARCHITECTURE.md)
13. [Methodology](foundation/METHODOLOGY.md)
14. [Design](experience/DESIGN.md)
15. [Design system](experience/DESIGN_SYSTEM.md)
16. [Decisions](governance/DECISIONS.md)
17. [Roadmap](../../ROADMAP.md)
18. [Meta architecture](meta/META.md)

## Canonical ownership

Each document owns one concern. The repository root `ARCHITECTURE.md` is a
compatibility link to this graph rather than a second architecture authority.
Implementation specifications under `docs/specs/` remain historical release
contracts and do not override this baseline.

aniflow is one independently usable holon in the flow suite. Suite-wide
orchestration and cross-tool contracts belong to flow. This repository remains
authoritative for aniflow's temporal-media domain and public behavior.

## Status and change

The documents begin in `draft` status so the baseline can be reviewed through
normal pull-request governance. A material change to purpose, domain ownership,
dependency direction, public compatibility, or temporal invariants requires a
decision record and downstream document review.

## Validation

Architecture review verifies valid frontmatter, resolvable identifiers, an
acyclic dependency graph, one H1 per document, consistent canonical terms,
explicit assumptions, and no conflicting ownership.
