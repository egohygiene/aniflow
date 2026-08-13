---
schema: aether.architecture-document/v1
id: aniflow-architecture-index
title: Aniflow Architecture Index
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-document
depends_on: []
related:
  - aniflow-purpose
  - aniflow-roadmap
supersedes: []
---

# Aniflow Architecture Index

This directory is the canonical architecture record for Aniflow. It defines
the product's identity, domain language, system ownership, internal dependency
rules, working method, and significant accepted decisions.

The baseline is grounded in `egohygiene/aniflow` package v0.2.0 at revision
`20783d7374298e5fd44c782348a3c944c9318656`, inspected on 2026-08-13. Target
statements describe accepted direction; current implementation gaps remain
explicit in the documents and roadmap.

## Document graph

Read in dependency order:

1. [Purpose](identity/PURPOSE.md)
2. [Vision](identity/VISION.md)
3. [Principles](identity/PRINCIPLES.md)
4. [Pillars](identity/PILLARS.md)
5. [Foundations](foundation/FOUNDATIONS.md)
6. [Epistemology](meta/EPISTEMOLOGY.md)
7. [Ontology](domain/ONTOLOGY.md)
8. [System](foundation/SYSTEM.md)
9. [Architecture](foundation/ARCHITECTURE.md)
10. [Methodology](foundation/METHODOLOGY.md)
11. [Decisions](governance/DECISIONS.md)
12. [Roadmap](../../ROADMAP.md)

## Canonical ownership

Each document owns one concern. The repository root `ARCHITECTURE.md` is a
compatibility link to this graph rather than a second architecture authority.
Implementation specifications under `docs/specs/` remain historical release
contracts and do not override this baseline.

Aniflow is one independently usable holon in the Flow suite. Suite-wide
orchestration and cross-tool contracts belong to Flow. This repository remains
authoritative for Aniflow's temporal-media domain and public behavior.

## Status and change

The documents begin in `draft` status so the baseline can be reviewed through
normal pull-request governance. A material change to purpose, domain ownership,
dependency direction, public compatibility, or temporal invariants requires a
decision record and downstream document review.

## Validation

Architecture review verifies valid frontmatter, resolvable identifiers, an
acyclic dependency graph, one H1 per document, consistent canonical terms,
explicit assumptions, and no conflicting ownership.
