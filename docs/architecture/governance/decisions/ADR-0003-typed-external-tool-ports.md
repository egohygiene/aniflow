---
schema: aether.architecture-decision/v1
id: aniflow-adr-0003
title: Isolate external tools behind typed ports
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
  - aniflow-principles
---

# ADR-0003 — Isolate external tools behind typed ports

## Context

Aniflow relies on FFmpeg, FFprobe, and optional processors. Version 0.2.0 uses
direct child processes, but tool discovery, version reporting, argument
expansion, execution, output buffering, cancellation, and validation remain
partly coupled to orchestration. External tools can report success while
producing absent, stale, incompatible, or semantically invalid artifacts.

## Decision

Application use cases depend on typed inward-facing ports for media operations
and processor execution. Adapters declare capability, identity, configuration,
inputs, outputs, and resource expectations.

External processes use direct argument arrays without a shell. The runtime
captures stdout and stderr independently, reports structured lifecycle events,
redacts sensitive diagnostics, forwards cancellation, records tool identity,
and validates declared output observations separately from process exit.

## Rationale

Typed ports keep temporal semantics testable without executing real tools and
let adapter implementations change without changing the application contract.
Separating exit from validation prevents external behavior from becoming
unearned stage success.

## Evidence and assumptions

Observed: current direct execution already avoids shell interpolation and typed
Upscayl/Gemini adapters demonstrate integration value. Observed: the generic
runner buffers complete output and has limited cancellation and capability
semantics. Assumed: a common process runtime can serve built-in and generic
adapters without erasing tool-specific behavior.

## Alternatives considered

- Embed every algorithm: reduces process variability but is infeasible,
  licensing-sensitive, and duplicates specialized projects.
- Keep ad hoc command invocation inside stages: initially simple but duplicates
  safety, logging, cancellation, and result interpretation.
- Use shell command strings: flexible but unsafe for paths, quoting, and
  untrusted expansion.

## Trade-offs

Adapters require more explicit code and contract tests. A generic adapter
remains less informative than a typed integration and must expose that reduced
capability honestly.

## Expected consequences

Tool simulation becomes straightforward, child-process behavior becomes
consistent, and processor-specific argument generation remains isolated.

## Security, privacy, and accessibility impact

Direct execution avoids shell expansion but does not make untrusted executables
safe. Pipeline-selected programs still run with user authority. Redaction and
structured diagnostics reduce accidental secret exposure and improve tooling
accessibility.

## Observed outcomes

None beyond the current direct-process precedent; full port extraction is
pending.

## Review triggers

Review when a supported in-process, remote, or sandboxed capability cannot
express its lifecycle through the port without losing essential semantics.

## Related artifacts

`aniflow-principles` and `aniflow-architecture`.

## Validation

Contract tests simulate success, failure, malformed output, timeout,
cancellation, missing artifacts, paths with spaces and Unicode, and redaction.
