---
schema: aether.architecture-decision/v1
id: aniflow-adr-0016
title: Validate selected historical spatial frames without inventing prior execution
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-adr-0008, aniflow-adr-0012]
---

# ADR-0016 — Selected historical spatial import

Proposed for #37; no acceptance or native qualification is recorded.

An interrupted directory may contain useful pixels without trustworthy
historical tool/model/settings evidence. Reusing it as a native Upscayl
checkpoint would manufacture that evidence. A current importer instead pins
the original source/index, selected ordinal/time/file identities, processing
dimensions and decoder. It requires an exact supplied generic import decision
before running one bounded ordinary v3 stage. Structure/CRC, full native RGB
decode and immutable copy observations remain separate from historical claims.

The first lane selects 1..128 explicitly named files per stage, permits uniform
2x/3x/4x original geometry within declared byte/pixel bounds, and leaves other
files untouched. Its output is a distinct spatial subset, never a complete
assembly index or recovered native processor report. Several selected imports
can be retained independently; automatic mixed collection/resume remains later
#37 work. The existing runtime owns locks, cancellation and acceptance.

This avoids a second checkpoint database and an invented model identity, at the
cost of not automatically completing a whole interrupted recovery job. Review
before automatic mixing, relaxed geometry, inferred provenance or cross-run
adoption. All execution and qualification remain deferred to flow #88, last.
