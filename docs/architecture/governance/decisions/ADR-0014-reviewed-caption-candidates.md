---
schema: aether.architecture-decision/v1
id: aniflow-adr-0014
title: Gate selected caption candidates with exact rendered preview declarations
kind: architecture-decision
status: proposed
owners:
  - egohygiene
scope:
  - aniflow
governed_by:
  - architecture-decisions
supersedes: []
superseded_by: []
related:
  - aniflow-architecture
  - aniflow-adr-0005
  - aniflow-adr-0008
  - aniflow-adr-0012
  - aniflow-adr-0013
---

# ADR-0014 — Reviewed caption candidates

## Status and authority

Proposed for #108. Source or merge does not ratify this decision. No acceptance
date, actual human content decision or native qualification is recorded.

## Context

Reviewed words and timing do not establish typography, contrast or safe picture
placement. A rendered subset cannot establish approval of every final frame.
Soft subtitles also depend on player behavior rather than burned-in font pixels.

## Proposed decision

Read current exact manual export/reviews through their existing owner. Use the
actual selected SRT wire times on the exact accepted picture clock. Compose
fixed v3 stages around the existing native process/media/validator machinery.
Native preview PNGs and full context bind a supplied generic manual declaration;
no approval is generated. Preserve source and renderer/font/tool identities.

Explicitly distinguish encoded-video-preserving subtitle mux from pixel-changing
H.264 burn-in. Preserve original AAC packets, codec configuration and clock in
both. Project a muxed subtitle candidate as a separate variant, which existing
destination policies refuse. Burn-in remains a separate candidate and makes no
original-frame identity assertion. Both require current native acceptance.

## Consequences and review triggers

Bound plain text/cues/frame selections and finite native resources. Keep private
words/paths/lineage outside public summaries. A subset declaration permits
candidate generation only; final creative review, sanitation and publication
remain separate. Video/lyrics audio relationship is operator-declared. Revisit
before rich markup, multi-face fallback, karaoke, player typography guarantees,
new clocks or automated review. No ASR, model or platform port is introduced.

## Evidence

Source-only authoring; focused synthetic regressions are uncompiled/unrun.
Native font, codec, packet/timing, pixel/color, cancellation, corruption and
platform/consumer qualification remain in flow #88, scheduled last. Preserve
the known unrelated alignment cancellation/start-marker failure, cause unproven.

## Proposed fixed finishing delivery (November 9, 2026)

The fixed recipe continuation retains the exact generic preview declaration
and composes existing native caption facades before selected metadata remux.
Preview review stays a candidate-generation gate. See
[scope](../../../recipe-finishing.md); proposal and qualification status are unchanged.
