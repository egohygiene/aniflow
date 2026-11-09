---
schema: aether.architecture-decision/v1
id: aniflow-adr-0018
title: Compose complete spatial frames from explicitly accepted origins
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-adr-0005, aniflow-adr-0008, aniflow-adr-0016]
---

# ADR-0018 — Complete spatial recovery carrier

Historical imports are reviewed subsets with unknown prior processing. Current
native recovery may have accepted independent units before the whole run
finishes. Join these explicit origins against one original source/index into a
separate complete carrier, preserving each ordinal/time and uniform geometry.
Require every original ordinal exactly once, current non-excluded v3 acceptance,
exact original/source and output bytes, and stable provider/report/checkpoint
bindings. Later progress on unrelated native units does not invalidate an
unchanged selected checkpoint; rerun/exclusion or changed output does.

Copy only pinned selected PNGs into a fresh private root and publish the
composition marker last. This marker composes current authority rather than
creating another checkpoint database or asserting historical Upscayl execution.
Assembly remains responsible for its own complete decode and temporal gates.
Source, imported and native versions remain distinct; quality requires separate
whole-work review. Partial publication is retained and refused for explicit
reconciliation. No source replacement, model lookup, process, posting or proof
occurs in composition. This proposal is unratified and unqualified; all
execution remains deferred to flow #88, scheduled last.
