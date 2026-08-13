---
schema: aether.architecture-document/v1
id: aniflow-principles
title: Aniflow Principles
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-principles
depends_on:
  - aniflow-purpose
  - aniflow-vision
related:
  - aniflow-foundations
  - aniflow-decisions
supersedes: []
---

# Aniflow Principles

## Temporal truth over convenience

Represent observed time bases, timestamps, stream identity, and ordering rather
than silently substituting an easier average-rate model. Explicitly rejecting an
unsupported input is better than producing a plausible but mistimed master.

## Preserve before transforming

Treat source bytes as immutable. Capture identity and relevant observations
before deriving new artifacts.

## Plan before expensive execution

Resolve configuration, capabilities, inputs, outputs, resource constraints,
and expected validation before launching expensive processors.

## Validate outcomes, not existence

A zero exit status, marker file, or nonempty output is evidence, not proof of a
valid stage. Completion follows contract-specific validation.

## Recover honestly

Reuse requires compatible identity across inputs, configuration, processor,
tool version, and verified outputs. When compatibility cannot be proven, rerun
or refuse rather than guess.

## Keep processors specialized and bounded

Aniflow owns temporal ordering and verification. External processors own their
specialized transformation and interact through declared inputs, outputs, and
capabilities.

## One public meaning across library and CLI

The CLI presents the library's behavior; it does not define a parallel engine.
Machine contracts are versioned and human output remains presentation.

## Make failure observable and containable

Preserve raw diagnostics, structured events, partial artifacts, and clear state.
Cancellation must stop owned child work and must not become success.

## Keep holons independent

Aniflow does not depend on sibling tools. Cross-tool selection, sequencing, and
suite provenance belong to Flow.

## Require authorization for transformation

Capabilities that remove or alter marks, metadata, captions, or authenticity
signals are documented for authorized media use and do not imply permission.

## Conflicts and exceptions

Source preservation, authorization, temporal integrity, and evidence integrity
outrank speed and convenience. Correctness outranks cache reuse. Independence
outranks short-term sibling coupling. Exceptions require a scoped decision,
consequence analysis, and review trigger.

## Assumptions and open questions

No current exception is accepted. Hardware-specific processors may require
capability-specific compromises, but those do not silently weaken the common
contract.

## Validation

Every significant design or pull request identifies the principles governing
its trade-offs and records deviations.
