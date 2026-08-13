---
schema: aether.architecture-document/v1
id: aniflow-foundations
title: Aniflow Foundations
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-foundations
depends_on:
  - aniflow-purpose
  - aniflow-vision
  - aniflow-principles
  - aniflow-pillars
related:
  - aniflow-system
  - aniflow-architecture
supersedes: []
---

# Aniflow Foundations

## Foundational assumptions

### Media is ordered in time

A video is not merely a directory of images. Its meaning includes streams,
time bases, timestamps, ordering, duration, synchronization, and declared
relationships among components.

### The source is evidence

Source bytes and observable media structure exist before any processing intent.
They remain immutable within a run and are identified before derivation.

### Paths are locations, not identity

Artifact identity derives from content and relevant typed context. A pathname
may change without changing content, and content may change at the same path.

### Plans and results are distinct

A plan records resolved intent before execution. A result records observations,
validation, and consequences after execution. Neither substitutes for the
other.

### Processors are fallible boundaries

A configured command, discovered executable, reported version, exit status,
log, and produced artifact may disagree. The runtime preserves these distinct
observations.

### Validation is layered

Configuration validity, process completion, artifact existence, structural
integrity, temporal semantics, and delivery fitness are different checks.

### A derivative is a new artifact

Processing never turns the output into the original. Every transformation
creates a new artifact with its own identity and run evidence.

## Invariants

- Source content is never modified by an Aniflow run.
- Ordering and timing intent remain explicit from inspection through delivery.
- Processor outputs do not overwrite their inputs.
- A stage is complete only after its declared outputs pass validation.
- Checkpoint reuse requires compatible inputs, plan, implementation, and
  verified outputs.
- Cancellation and failure cannot be recorded as success.
- The CLI and Rust library expose the same domain semantics.
- Aniflow has no runtime or crate dependency on Flow, Optiflow, or Renderflow.
- A master is named as such only after declared master validation succeeds.

## Baseline constraints

Aniflow is local-first, source-preserving, automation-friendly, and safe for
paths containing spaces and Unicode. Rust is the primary implementation
language; external tools remain valid behind typed, direct-process adapters.
Large frame sets require bounded resource use and storage-aware behavior.

## Current constraints that are not foundations

Pipeline v2 currently uses average frame rate for CFR reconstruction, processes
the first video and audio streams, exchanges PNG frames, encodes AAC audio,
trusts run-local completion markers too broadly, and includes an optional
Renderflow handoff. These are observed v0.2.0 constraints to migrate, not
enduring truths.

## Falsified or revised foundations

The earlier assumption that a direct downstream Renderflow seam belonged in
Aniflow has been revised. Cross-holon handoff selection belongs to Flow; the v2
field remains only as a compatibility concern until pipeline v3.

## Assumptions and open questions

The precise internal timeline representation and minimum cross-platform support
matrix require fixture evidence. Those gaps do not weaken the invariants above.

## Validation

Architecture and contract tests verify invariants, and changes to a foundation
require an ADR plus review of every dependent document.
