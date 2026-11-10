---
schema: aether.architecture-decision/v1
id: aniflow-adr-0022
title: Keep optional AMV evidence notes separate from media acceptance
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-adr-0019, aniflow-adr-0020]
---

# ADR-0022 — AMV evidence advisory

Proposed for #116. Source inclusion or merge is not ratification.

Project bounded current final-video record facts into a portable packet. Check
supplied notes against exact membership and final-video rational timestamps,
then emit host-rendered facts and separate unverified suggestions/questions.
A saved packet cannot substitute for current native acceptance. The first
checkpoint needs no raw media, model runtime or new generic provider SDK.

Existing final-video validators and exact-candidate human review retain their
respective authority. Notes cannot mutate recipes/media, launch tools, approve,
sign or publish. Model/profile names remain declarations. Runtime adoption,
actual invocation reporting and sampled vision need later explicit contracts.
Membership checks do not establish suggestion relevance or semantic truth.

Native/visual and backend usefulness qualification remain open under #116 and
flow #88 last. This is neither a release prerequisite nor completion of #116.
