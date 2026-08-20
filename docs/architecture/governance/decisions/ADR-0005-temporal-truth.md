---
schema: aether.architecture-decision/v1
id: aniflow-adr-0005
title: Model temporal truth instead of average-rate convenience
kind: architecture-decision
status: accepted
accepted: 2026-08-13
owners:
  - egohygiene
scope:
  - aniflow
governed_by:
  - architecture-decisions
supersedes: []
superseded_by: []
related:
  - aniflow-foundations
  - aniflow-ontology
  - aniflow-architecture
---

# ADR-0005 — Model temporal truth instead of average-rate convenience

## Context

Version 0.2.0 estimates frame count from duration and average frame rate,
extracts the first video and audio streams, and reassembles images at a fixed
average rate. This is an honest v0.2 CFR constraint, but it cannot serve as the
target model for variable-frame-rate media, explicit stream selection, or
reliable synchronization.

## Decision

aniflow's target temporal model represents rational time bases, stream identity,
timestamps, durations, ordering, and synchronization explicitly. Decomposition
preserves the relation between components and the source timeline, and
reconstruction uses that relation rather than inferring timing from filenames
and an average rate.

Every input class receives one of three explicit outcomes: supported with
validated semantics, rejected before expensive work with an actionable reason,
or declared experimental with visible limitations. Silent timing normalization
is prohibited.

## Rationale

Temporal integrity is aniflow's defining domain responsibility. A fast pipeline
that subtly changes timing is not a valid foundation for reusable processing or
trusted resume.

## Evidence and assumptions

Observed: current inspection stores average frame rate as both text and floating
point and estimated frame count as duration multiplied by rate. Observed: current
assembly supplies a fixed framerate. Assumed: timestamp-preserving decomposition
and fixture-led stream policy can cover the intended creator workflows without
requiring a universal media editor.

## Alternatives considered

- Support CFR forever: simpler and valid for a subset, but unnecessarily narrows
  the product vision and encourages users to pre-normalize without evidence.
- Normalize every source to CFR automatically: predictable internally but may
  duplicate, drop, or retime frames without explicit intent.
- Treat FFmpeg output as inherently correct: delegates policy and validation to
  command defaults that may vary by input and version.

## Trade-offs

The domain model, fixtures, manifests, and reconstruction become more complex.
Some inputs may be rejected until their semantics are implemented. That cost is
accepted in exchange for honest temporal behavior.

## Expected consequences

Inspection captures richer stream evidence. Plans include explicit selection
and timing policy. VFR and multi-stream fixtures become required. Checkpoint
identity includes timeline intent.

## Security, privacy, and accessibility impact

No execution authority changes. More detailed metadata can expose source path
or stream information, so persisted and machine output must apply privacy-aware
field policy.

## Observed outcomes

None yet; v0.2.0 remains explicitly CFR/first-stream constrained.

## Review triggers

Review if representative fixtures demonstrate that the chosen model cannot
round-trip supported media or imposes unacceptable cost on the simple CFR path.

## Related artifacts

`aniflow-foundations`, `aniflow-ontology`, and `aniflow-architecture`.

## Validation

Redistribution-safe CFR, VFR, audio-less, subtitle, and multi-stream fixtures
prove either preservation or exact preflight rejection.
