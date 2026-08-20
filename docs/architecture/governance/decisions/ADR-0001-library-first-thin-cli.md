---
schema: aether.architecture-decision/v1
id: aniflow-adr-0001
title: Expose a public library behind a thin CLI
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
  - aniflow-architecture
  - aniflow-roadmap
---

# ADR-0001 — Expose a public library behind a thin CLI

## Context

aniflow v0.2.0 is a binary-only Cargo package whose modules are private to
`main.rs`. flow and other Rust consumers cannot reuse planning or execution
without invoking a process or extracting internals. The CLI also risks becoming
the accidental behavioral contract.

## Decision

aniflow's behavior will be exposed through an intentionally small public Rust
library. The CLI will parse and present user intent while delegating inspection,
planning, running, resuming, status, and diagnostics to that library.

The first extraction will retain one Cargo package with library and binary
targets. Additional packages require a demonstrated compile, release, feature,
or independent-consumer boundary.

## Rationale

A library makes aniflow independently composable while one behavioral path
prevents CLI and embedded use from diverging. Starting with one package keeps
the refactor reviewable and avoids inventing premature crate boundaries.

## Evidence and assumptions

Observed: all current behavior is reachable only through the binary and private
modules. Decided: the product must work both as a library and CLI. Assumed: an
initial single-package split can expose the needed facade without creating an
unstable public surface across every internal type.

## Alternatives considered

- Remain CLI-only: preserves simplicity but forces every in-process consumer
  through process and serialization boundaries.
- Create core, FFmpeg, processor SDK, and CLI crates immediately: offers strong
  physical boundaries without evidence that each deserves independent release.
- Let flow import private source: creates cross-repository coupling and makes
  aniflow cease to be independently authoritative.

## Trade-offs

Public API compatibility becomes an explicit maintenance responsibility.
Keeping one package provides weaker physical isolation than a workspace, but
reduces migration complexity while the API is pre-1.0.

## Expected consequences

The binary becomes a delivery adapter. Library examples and consumer tests
become release gates. Internal modules may change freely until intentionally
promoted into the public facade.

## Security, privacy, and accessibility impact

No authority expands. Library callers must receive the same path, process,
redaction, and safety semantics as CLI users. Structured errors improve
automation accessibility.

## Observed outcomes

The package now builds library and binary targets from one behavioral path. A
small crate-root facade exposes diagnostics, inspection, planning, execution,
resume, and status while keeping pipeline, state, process, and workspace modules
private. The CLI owns Clap argument mapping and console rendering, and an
external-style integration test plus example compile against only the public
facade. Run progress is provisional pending the dedicated observable-runtime
increment.

## Review triggers

Review when an adapter needs independent versioning, compile isolation, feature
selection, or multiple external consumers depend directly on the same internal
boundary.

## Related artifacts

`aniflow-architecture` and `aniflow-roadmap`.

## Validation

A Rust example and CLI contract tests perform equivalent operations through the
same application path.
