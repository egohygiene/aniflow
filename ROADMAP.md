---
schema: aether.architecture-document/v1
id: aniflow-roadmap
title: aniflow Roadmap
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-24
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

# aniflow Roadmap

<!-- BEGIN ROADMAP EXECUTION SNAPSHOT -->
<!-- roadmap-manifest
schema: hygiene.roadmap/v1alpha1
repository: egohygiene/aniflow
visibility: public
publication: central
route: /roadmap/aniflow/
updated: 2026-08-24
-->
## 2026-08-24 execution snapshot

> This evidence-reconciled snapshot is the issue-generation and visual-roadmap handoff. The longer-horizon strategy below remains canonical context; generated HTML, JSON, progress, issue plans, and commit lists are projections.

**Lifecycle:** functional Rust alpha  
**Current gate:** Fix the lowercase-title policy failure, reconcile the claimed v0.3 status, and publish the first verified release.  
**North-star outcome:** A bounded, resumable, temporally correct offline media-analysis pipeline with explicit library and provider contracts.

### Visual roadmap publication

**Mode:** `central`  
**Route:** `/roadmap/aniflow/`  
**Current publication evidence:** Source-only Rust library and CLI; no GitHub release or Pages publication observed.

Publish the public-safe projection through egohygiene.io at /roadmap/aniflow/. This repository owns intent and acceptance evidence; it does not add a second site deployment.

### Quest line

<!-- roadmap-step
id: ANI-Q01
status: complete
depends_on: []
issues: [3, 4, 5, 7]
-->
#### ANI-Q01 — Build the Rust pipeline foundation

**State:** `complete`  
**Depends on:** None

**Outcome:** Library extraction, contracts, and architecture graph form a functional alpha.

**Exit criteria:**

- [x] The library and CLI compile on supported platforms.
- [x] Core contract and architecture artifacts are present.

**Current evidence:**

- Issues #3, #4, #5, and #7 correspond to landed architecture, extraction, contracts, and graph work.
- Latest audited merge 66368c7a61124a46944df79d7c77b17f5c2a11a4 landed on 2026-08-20.

<!-- roadmap-step
id: ANI-Q02
status: blocked
depends_on: [ANI-Q01]
issues: []
-->
#### ANI-Q02 — Restore CI and release truth

**State:** `blocked`  
**Depends on:** `ANI-Q01`

**Outcome:** All required workflows are green and the documented version matches a published artifact.

**Exit criteria:**

- [ ] The lowercase checker accepts the corrected Aniflow Personal Model title.
- [ ] A tagged release backs the documented version or the version claim is removed.

**Current evidence:**

- Linux stable, MSRV, and macOS passed.
- Overall CI failed on title: Aniflow Personal Model, and no release was observed despite a v0.3 claim.

<!-- roadmap-step
id: ANI-Q03
status: planned
depends_on: [ANI-Q02]
issues: []
-->
#### ANI-Q03 — Bound runtime and make Pipeline v3 resumable

**State:** `planned`  
**Depends on:** `ANI-Q02`

**Outcome:** Long-running work has explicit resource limits, checkpoints, and deterministic resume behavior.

**Exit criteria:**

- [ ] CPU, memory, disk, and time limits are enforced in fixtures.
- [ ] Interrupted work resumes without duplicating accepted outputs.

**Current evidence:**

- Bounded runtime and Pipeline v3 resume are identified roadmap gaps.

<!-- roadmap-step
id: ANI-Q04
status: ready
depends_on: [ANI-Q03]
issues: [6]
-->
#### ANI-Q04 — Guarantee temporal correctness

**State:** `ready`  
**Depends on:** `ANI-Q03`

**Outcome:** Issue #6 handles short segments and boundary conditions without timestamp drift.

**Exit criteria:**

- [ ] Short, overlapping, and boundary-segment fixtures pass.
- [ ] Every derived artifact retains source-time provenance.

**Current evidence:**

- Issue #6 opened on 2026-08-19.

<!-- roadmap-step
id: ANI-Q05
status: planned
depends_on: [ANI-Q03, ANI-Q04]
issues: [8]
-->
#### ANI-Q05 — Add offline Demucs and publish v1

**State:** `planned`  
**Depends on:** `ANI-Q03`, `ANI-Q04`

**Outcome:** Issue #8 provides an optional offline separation path within a stable v1 pipeline.

**Exit criteria:**

- [ ] Demucs assets, licensing, resource bounds, and fallback behavior are documented and tested.
- [ ] A tagged v1 release passes the complete supported-platform matrix.

**Current evidence:**

- Issue #8 opened on 2026-08-22.
- No GitHub release was observed.

### Roadmap-to-issue handoff

- A step is complete only when its exit criteria and required evidence are satisfied; commit count never determines progress.
- Ready steps without an issue are candidates for the private, duplicate-aware roadmap.issue-plan.json dry run. Planned steps remain preview-only unless a reviewer explicitly opts them in with issue_policy: propose.
- Issue creation or reconciliation requires human approval or an explicitly authorized Pace operation and returns issue references through a reviewable roadmap pull request.
- Pull requests and commits should include Roadmap-Step: <ID>; historical evidence may be linked through existing issue and pull-request relationships.
- Public rendering uses only allowlisted build-time evidence and never places a GitHub token or private issue plan in the browser artifact.

<!-- END ROADMAP EXECUTION SNAPSHOT -->

## Strategic context

aniflow v0.2.0 proves a real end-to-end music-video path: inspect, extract,
process ordered frame sets, reconstruct, restore audio, apply subtitles, and
emit a master and manifest. The next evolution preserves that working vertical
slice while replacing the contracts that currently limit reuse, temporal
correctness, recovery, and publication.

The v1 destination is a reusable Rust library and standalone CLI for inspecting,
decomposing, processing, validating, reconstructing, and resuming temporally
ordered video workflows. aniflow remains independent of sibling tools; flow
owns cross-tool orchestration.

Horizons advance when their exit evidence exists, not when a date arrives.

## Now — Establish the v1 contract

### PR 1 — Architecture and roadmap

Define identity, canonical language, system ownership, inward dependency
direction, operating method, accepted boundaries, and strategic evolution.
Correct documentation that treats the optional renderflow handoff or binary-
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

**Exit evidence:** aniflow cannot report success until the master passes its
declared stream, timing, synchronization, decodability, and checksum checks.

**Milestone:** publish `0.8.0` as the v1 release candidate for real-world flow
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
workflow; flow consumes a released version rather than a source revision.

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
| Cross-tool convenience recreates coupling | Keep sibling dependencies forbidden and integrate through flow |
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
