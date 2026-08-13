---
schema: aether.architecture-document/v1
id: aniflow-roadmap
title: Aniflow Roadmap
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-roadmap
depends_on:
  - aniflow-vision
  - aniflow-principles
  - aniflow-pillars
  - aniflow-foundations
  - aniflow-system
  - aniflow-architecture
  - aniflow-methodology
related:
  - aniflow-decisions
supersedes: []
---

# Aniflow Roadmap

## Strategic context

Aniflow v0.2.0 proves a real end-to-end music-video path: inspect, extract,
process ordered frame sets, reconstruct, restore audio, apply subtitles, and
emit a master and manifest. The next evolution preserves that working vertical
slice while replacing the contracts that currently limit reuse, temporal
correctness, recovery, and publication.

The v1 destination is a reusable Rust library and standalone CLI for inspecting,
decomposing, processing, validating, reconstructing, and resuming temporally
ordered video workflows. Aniflow remains independent of sibling tools; Flow
owns cross-tool orchestration.

Horizons advance when their exit evidence exists, not when a date arrives.

## Now — Establish the v1 contract

### PR 1 — Architecture and roadmap

Define identity, canonical language, system ownership, inward dependency
direction, operating method, accepted boundaries, and strategic evolution.
Correct documentation that treats the optional Renderflow handoff or binary-
only structure as the target architecture.

**Exit evidence:** the architecture graph is complete and acyclic; current
constraints differ visibly from target invariants; all accepted decisions have
review triggers; no runtime behavior changes.

## Next — Become a dependable reusable product

### PR 2 — Library extraction

Expose a deliberately small public Rust facade while reducing the binary to
delivery concerns. Preserve current behavior and pipeline compatibility.

**Exit evidence:** an independent Rust example can inspect, plan, run, resume,
and query status without invoking the CLI parser.

### PR 3 — Stable human and machine contracts

Give commands consistent structured output, typed error categories, documented
exit behavior, long-form naming, compatibility policy, and contract tests.
Establish the supported Rust baseline and broaden core CI evidence.

**Exit evidence:** scripts consume every supported command without parsing
human prose, and library and CLI results remain semantically equivalent.

**Milestone:** publish a `0.3.0` library preview so an external consumer can
exercise the boundary before v1 freezes it.

## Next — Make execution and recovery trustworthy

### PR 4 — Observable process runtime

Introduce the common lifecycle for safe direct execution, independent logs,
events, capability discovery, resource bounds, cancellation, and diagnostic
redaction.

**Exit evidence:** failure, timeout, interruption, malformed output, and missing
artifacts retain truthful state and actionable evidence.

### PR 5 — Typed processor model

Formalize processor identity, capabilities, configuration, declared artifacts,
and result validation. Place first-party integrations and the generic command
escape hatch behind the same bounded runtime semantics.

**Exit evidence:** processors are replaceable adapters rather than orchestration
special cases, and none can establish completion through exit status alone.

### PR 6 — Deterministic Pipeline v3 planning

Normalize configuration into a serializable execution plan with a stable
digest, schema artifacts, capability resolution, typed argument expansion, and
actionable migration. Remove cross-holon selection from the new schema.

**Exit evidence:** identical resolved intent produces identical plans and
digests; unsupported capabilities and versions fail before expensive work.

### PR 7 — Content-aware run state and resume

Replace blind completion markers with atomic, versioned stage evidence and
explainable compatibility decisions. Separate read-only inspection from
workspace mutation and represent complete lifecycle states.

**Exit evidence:** changing any relevant input, timeline, configuration,
processor, implementation, or validated output deterministically invalidates
the affected stage.

**Milestone:** publish `0.5.0` with reusable planning, execution, and recovery
contracts.

## Next — Fulfill the temporal domain promise

### PR 8 — Stream-aware temporal correctness

Represent rational time, timestamps, stream identity, selection, and
synchronization explicitly. Preserve supported variable-frame-rate behavior and
define exact handling or rejection for multiple video, audio, and subtitle
streams.

**Exit evidence:** redistribution-safe CFR, VFR, audio-less, subtitle, and
multi-stream fixtures are correctly processed or rejected before expensive work
with an exact reason.

### PR 9 — Layered validation and evidence-rich delivery

Validate components, intermediates, candidate masters, and delivered artifacts
against declared structural and temporal contracts. Emit a versioned artifact
and observation record suitable for external composition without overstating
authenticity.

**Exit evidence:** Aniflow cannot report success until the master passes its
declared stream, timing, synchronization, decodability, and checksum checks.

**Milestone:** publish `0.8.0` as the v1 release candidate for real-world Flow
consumer testing.

## Later — Improve efficiency without weakening proof

### PR 10 — Content-addressed reuse and operational controls

Add opt-in cross-run reuse, targeted reruns, explicit invalidation, cache
inspection and pruning, storage preflight, retention policy, and concurrent-
writer protection using the trusted identities established earlier.

**Exit evidence:** fresh and reused execution produce equivalent validated
results, corrupted entries are detected, and every reuse decision is explained.

## Release — Publish a supportable v1

### PR 11 — Portability, documentation, packaging, and v1 publication

Complete the supported platform matrix, failure-injection coverage, public API
documentation, installation path, release artifacts, checksums, software bill
of materials, migration guides, and compatibility process.

**Exit evidence:** a clean Rust consumer uses the released crate; a clean
supported machine installs the CLI and completes the documented synthetic
workflow; Flow consumes a released version rather than a source revision.

**Milestone:** publish `1.0.0` and begin normal SemVer governance.

## Maybe

- Scene- or segment-aware targeted invalidation after the timeline model proves
  a stable concept.
- An arbitrary temporal DAG only when real branching or fan-in cannot be
  expressed safely through ordered chains.
- Remote or distributed processor execution with explicit privacy, residency,
  cancellation, and artifact-transfer policy.
- Real-time or interactive processing after offline correctness and recovery
  are mature.
- Perceptual continuity analysis when methods can report calibrated limits
  instead of presenting heuristics as proof.

These are possibilities rather than v1 commitments.

## Dependencies and risks

| Risk | Strategic response |
| --- | --- |
| Public API freezes internal mistakes | Keep the pre-1.0 facade small and test it from a real consumer |
| External tools behave inconsistently | Probe capabilities, isolate adapters, retain raw evidence, and validate artifacts |
| Temporal model expands uncontrollably | Drive it from representative fixtures and explicit support policy |
| Resume reuses incompatible work | Bind reuse to complete stage identity and verified output observations |
| Machine schemas churn | Version contracts and test migrations and unknown-version rejection |
| Cross-tool convenience recreates coupling | Keep sibling dependencies forbidden and integrate through Flow |
| Large runs exhaust local resources | Bound concurrency and add storage-aware preflight before cross-run caching |
| Watermark features imply unauthorized use | Preserve explicit authorization language and avoid authenticity-bypass claims |

## Assumptions and open questions

The target versions are compatibility waypoints rather than calendar promises.
The exact public facade, timeline representation, platform matrix, and cache
retention defaults remain subject to evidence from their respective increments.

## Validation

Roadmap changes remain aligned with Purpose, Vision, Principles, Pillars, and
accepted ADRs. Tactical work is derived into pull requests or issues without
turning this document into a backlog.
