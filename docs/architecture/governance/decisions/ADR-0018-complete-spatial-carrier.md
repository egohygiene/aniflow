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

The #35 continuation adds a fixed recovered entry over those owners. Freeze
accepted selections and missing original input identities, process only the
missing ordinals in serial sparse native v2 children, create the complete
carrier, then invoke existing native assembly through a distinct v2 operation.
The sparse run is bounded to 128 original ordinals below 100000, with no v1
collector or renumbering. Old v1 schemas and serialized meanings remain intact.
The original source/index and every actual child remain the authorities; no
new graph, checkpoint store, implicit approval or historical attribution is
introduced. The proposal still requires review and native qualification.
