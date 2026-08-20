---
schema: aether.architecture-decision/v1
id: aniflow-adr-0004
title: Version public machine contracts
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

# ADR-0004 — Version public machine contracts

## Context

aniflow already versions pipeline and manifest schemas, but the CLI has
inconsistent structured output and human plan/status text is not a stable
integration contract. flow and scripts need explicit compatibility behavior as
the pre-1.0 model changes.

## Decision

Pipeline schemas, execution plans, run manifests, delivery results, events,
errors, and machine-readable CLI envelopes are explicit versioned contracts.
Unknown incompatible versions fail actionably. Breaking changes provide an
intentional migration path or rejection message rather than silent fallback.

Human console output is presentation and may evolve without becoming the
machine contract. Public Rust API compatibility follows SemVer and is kept
intentionally small before 1.0.

## Rationale

Versioned semantics allow independent releases and deterministic consumers.
They make compatibility a tested property instead of an assumption based on
matching field names.

## Evidence and assumptions

Observed: pipeline v1 and v2 and run manifest v2 already establish versioning
precedent. Observed: only `inspect` currently offers a dedicated JSON flag.
Assumed: early publication and a real flow consumer will reveal which API
surface should stabilize for v1.

## Alternatives considered

- Stabilize only the CLI command text: fragile for automation and localization.
- Share internal Rust structs without schema versions: convenient in one build,
  unsafe across independent releases.
- Delay contracts until v1: prevents meaningful pre-1.0 integration feedback.

## Trade-offs

Every public contract carries migration and test obligations. Some duplication
between domain types and wire representations is accepted to preserve domain
meaning and compatibility independently.

## Expected consequences

Pipeline v3, plan and result envelopes, stable exit categories, schema fixtures,
and consumer compatibility tests become roadmap gates.

## Security, privacy, and accessibility impact

Schemas define which paths, logs, and diagnostics may be persisted or emitted.
Machine contracts avoid leaking secrets and give assistive automation stable
error categories.

## Observed outcomes

Version `0.3.0` introduces machine envelope schema v1 for all six CLI
operations, typed public error categories, stable exit-code mapping, a JSON
Schema, golden fixtures, and end-to-end library/CLI parity tests. Unknown
envelope versions are rejected by the public parser. The v0.2 `inspect --json`
shape and `run --output-dir` spelling remain compatibility paths throughout
`0.3.x`, while new consumers use `--output json` and `--output-directory`.

## Review triggers

Review if version negotiation creates substantial complexity without a real
independent consumer, or if a contract exposes domain internals that prevent
safe evolution.

## Related artifacts

`aniflow-architecture` and `aniflow-roadmap`.

## Validation

Golden fixtures, schema validation, unknown-version rejection, migration tests,
SemVer checks, and external consumer tests cover each public contract.
