---
schema: aether.architecture-decision/v1
id: aniflow-adr-0017
title: Bind selected destination copies to exact source and explicit geometry
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-adr-0008, aniflow-adr-0013]
---

# ADR-0017 — Selected destination video

Proposed for #83; source inclusion is not acceptance or native qualification.

A selected destination can need different dimensions from the clean accepted
master. Plan one isolated derivative with an exact current source, supplied
dated manual-route requirements, closed codec settings, explicit metadata and
geometry policies. Keep exact aspect or explicitly fit/pad; do not infer crop,
trim, frame-rate conversion, audio gain, resampling or source replacement.

The first proposed native lane is CPU H.264/yuv420p SDR MP4 from an accepted
FFV1 RGB8 master or H.264 delivery, with unchanged 48 kHz mono/stereo AAC-LC
packets when present. Current native validation owns actual structure, full
clock, audio payloads, decode and final bytes; output size and platform checks
cannot be inferred from planning. Unsupported subtitle-bearing source is
refused until an explicit preserving path exists. Quality needs separate review.

Exact-aspect integer geometry avoids silent pixel-aspect distortion, at the
cost of sometimes using fewer pixels than a bounding box. Opaque black padding
is explicit and chroma-aligned. No tool/model lookup, download or execution
occurs during source intent planning. The proposal adds no new runtime or
general graph engine. Review before broader color, codecs, audio or timing.
All execution and qualification remain deferred to flow #88, scheduled last.

The following source checkpoint composes final-file audio QC using the existing
complete selected-video decode and current native PCM signal acceptance. Bind
the whole final file as well as its accepted audio packet/configuration/timing
identity. Even metadata-only byte changes invalidate that QC receipt. Keep the
separately pinned native loudness observation unqualified, unavailable values
typed, and musical review independent. Preflight requires an explicit QC input;
there is no inference from earlier original PCM or automatic gain adjustment.
