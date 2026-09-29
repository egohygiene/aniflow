---
schema: aether.architecture-document/v1
id: aniflow-roadmap
title: aniflow Roadmap
kind: architecture-document
version: 0.1.16
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-09-29
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

## 2026-09-29 MIDI candidate checkpoint

The live [#13 mini roadmap](https://github.com/egohygiene/aniflow/issues/13)
remains the parent for ten bounded audio checkpoints. This section supersedes
older execution ordering below. The maintainer merged
[PR #61](https://github.com/egohygiene/aniflow/pull/61) at
`b81c91e1fe57130764c536ad4c59bb1c27a73e61`; fresh main and the merged
#42/#43 prerequisites were rechecked before starting #50.
Implementation, synthetic validation and a feature merge remain distinct from
model accuracy, native-platform support and release qualification.

| Checkpoint | Current state | Merged prerequisites |
| --- | --- | --- |
| [#42](https://github.com/egohygiene/aniflow/issues/42) | Normalized analysis contract; merged via [#52](https://github.com/egohygiene/aniflow/pull/52) | None; #8 is merged |
| [#43](https://github.com/egohygiene/aniflow/issues/43) | Offline technical inspection; merged via [#53](https://github.com/egohygiene/aniflow/pull/53) | #42 |
| [#44](https://github.com/egohygiene/aniflow/issues/44) | Loudness, peaks, silence and clipping; merged via [#54](https://github.com/egohygiene/aniflow/pull/54) | #42, #43 |
| [#45](https://github.com/egohygiene/aniflow/issues/45) | Provider-neutral stem lineage; merged via [#57](https://github.com/egohygiene/aniflow/pull/57) | #42, #43, #8 |
| [#46](https://github.com/egohygiene/aniflow/issues/46) | Tempo, beat and key estimates; merged via [#58](https://github.com/egohygiene/aniflow/pull/58) | #42, #43 |
| [#47](https://github.com/egohygiene/aniflow/issues/47) | Loss-aware lyrics and timed-text conversion; merged via [#59](https://github.com/egohygiene/aniflow/pull/59) | #42 |
| [#48](https://github.com/egohygiene/aniflow/issues/48) | Offline timestamped transcription; merged via [#60](https://github.com/egohygiene/aniflow/pull/60) | #42, #43, #47 |
| [#49](https://github.com/egohygiene/aniflow/issues/49) | Reviewed-lyrics alignment; merged via [PR #61](https://github.com/egohygiene/aniflow/pull/61) | #42, #43, #47 |
| [#50](https://github.com/egohygiene/aniflow/issues/50) | Probabilistic MIDI candidates; active in [draft PR #62](https://github.com/egohygiene/aniflow/pull/62) | #42, #43 |
| [#51](https://github.com/egohygiene/aniflow/issues/51) | Integrated workflow and consumer closeout; waiting | #42–#50 |

The merged features retain their own exact scope and local evidence:

- [Foundation](docs/audio-analysis.md) and [#42 receipt](docs/validation/aniflow-42-local.json).
- [Technical inspection](docs/audio-inspection.md) and [#43 receipt](docs/validation/aniflow-43-local.json).
- [Signal analysis](docs/audio-signal-analysis.md), [#44 receipt](docs/validation/aniflow-44-local.json) and the separately merged [#55 high-rate receipt](docs/validation/aniflow-55-local.json). PR #56 added only four qualified high-rate true-peak profiles; historical v1 evidence remains readable.
- [Stem lineage](docs/audio-stem-lineage.md) and [#45 receipt](docs/validation/aniflow-45-local.json), building on the merged [offline Demucs profile](docs/offline-demucs.md). Exact duration comparison does not prove onset or phase alignment.
- [Musical estimates](docs/audio-musical-analysis.md) and [#46 receipt](docs/validation/aniflow-46-local.json), with retained heuristic uncertainty and separately recorded synthetic real-analyzer observations.
- [Timed-text conversion](docs/timed-text.md) and [#47 receipt](docs/validation/aniflow-47-local.json), with supplied timing, explicit losses and unchanged review authority.

The merged [transcription guide](docs/audio-transcription.md) and
[#48 receipt](docs/validation/aniflow-48-local.json) retain the exact synthetic
checks, earlier validation failures and follow-up results. Actual whisper.cpp
inference and recognition quality remain unverified; a feature merge does not
supply that evidence.

The merged [alignment guide](docs/audio-lyrics-alignment.md) describes #49's
optional pinned local PocketSphinx profile. It preserves the original reviewed
text and its supplied provenance while proposing separate candidate word/cue
timing. Native forced alignment does not establish that the words were spoken,
that the timing is accurate, or that the supplied reviewer is authentic.
The [recovery checkpoint](docs/validation/aniflow-49-checkpoint.md) and
[local receipt](docs/validation/aniflow-49-local.json) record the pushed
savepoints, 328 Rust tests, 29 focused MSRV tests, schema/docs/package and
synthetic smoke checks, development corrections and remaining gates. Local synthetic contract
and refusal checks do not qualify real forced alignment, singing accuracy,
native-platform support or release readiness. Parent #13 remains open; only the
reconciled #51 closeout owns its completion.

The current [MIDI guide](docs/audio-midi.md) describes #50's optional pinned
Basic Pitch 0.4.0 CPU profile. It produces probabilistic note candidates and
a bounded Standard MIDI File export with normalized evidence. Native activation
strength is not calibrated confidence, an instrument assignment or score truth.
The [recovery checkpoint](docs/validation/aniflow-50-checkpoint.md) records
pushed savepoints and remaining validation. Actual model inference/accuracy,
native-platform support and release qualification require separate evidence.

Stop for maintainer review and merge of #50's PR before starting #51. Re-query
live state before choosing the next bounded issue. #51 now waits only for #50
among the audio checkpoints and owns parent #13 reconciliation. Preserve early
draft and validation checkpoints, record local evidence, and do not wait for
hosted CI. The maintainer owns reviews and merges.

The downstream product order remains #13 → #32 → #33 → #34 → bounded #24
closeout → #10 → [flow #51](https://github.com/egohygiene/flow/issues/51).
Generalized stream timing belongs to #32, layered completion gates to #33,
and cross-run reuse to #34. Provider-owned analysis evidence is not any of those
gates. Provider audit #17 and ADR-history reconciliation #14 remain separate
later lanes. No aniflow release is created by this checkpoint.

## 2026-09-25 live suite handoff

> [!IMPORTANT]
> This section supersedes older current-gate ordering below where it conflicts.
> The newer decomposition recorded in #24 is the near-term execution authority.
> Re-query live issue, release, and CI state before starting a branch.

### Immediate aniflow chain

```text
#8 → #13 → #32 → #33 → #34 → bounded #24 closeout → #10 → flow #51
```

1. [#8](https://github.com/egohygiene/aniflow/issues/8) — integrate the
   offline-first Demucs vocal-stem workflow through aniflow's bounded provider
   model.
2. [#13](https://github.com/egohygiene/aniflow/issues/13) — add typed,
   time-indexed audio feature, lyrics/timed-text, and MIDI candidate analysis.
3. [#32](https://github.com/egohygiene/aniflow/issues/32) — finish
   stream-aware temporal correctness: rational time, explicit stream identity,
   timestamp semantics, synchronization, and source-time provenance.
4. [#33](https://github.com/egohygiene/aniflow/issues/33) — make layered
   validation and evidence-rich delivery a real completion gate.
5. [#34](https://github.com/egohygiene/aniflow/issues/34) — add explainable
   content-addressed cross-run reuse and operational controls.
6. Close the remaining bounded fixture families under
   [#24](https://github.com/egohygiene/aniflow/issues/24) without turning the
   corpus into one giant blocking implementation issue.
7. [#10](https://github.com/egohygiene/aniflow/issues/10) — publish the first
   immutable independently installable aniflow release after re-checking its
   external release-convention dependencies.
8. Hand that immutable release to
   [flow #51](https://github.com/egohygiene/flow/issues/51).

The provider SDK/runtime/Pipeline-v3 planning, execution, resume, and conformance
foundation is already landed. The current lane should build temporal/audio
capability and release truth on top of those contracts rather than inventing a
second execution system.

[#14](https://github.com/egohygiene/aniflow/issues/14) remains the ADR-history
lane, and [#17](https://github.com/egohygiene/aniflow/issues/17) remains the
post-roadmap repository/backlog/Identity audit. Neither should silently replace
the active product sequence above.

<!-- BEGIN ROADMAP EXECUTION SNAPSHOT -->
<!-- roadmap-manifest
schema: hygiene.roadmap/v1alpha1
repository: egohygiene/aniflow
visibility: public
publication: central
route: /roadmap/aniflow/
updated: 2026-09-25
-->
## 2026-09-20 execution snapshot

> This evidence-reconciled snapshot is the issue-generation and visual-roadmap handoff. The longer-horizon strategy below remains canonical context; generated HTML, JSON, progress, issue plans, and commit lists are projections.

**Lifecycle:** functional Rust alpha  
**Current gate:** Publish the first verified release in #10 while provider
capabilities and adversarial-corpus work continue in #8, #13, and #24.
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
status: active
depends_on: [ANI-Q01]
issues: [10]
-->
#### ANI-Q02 — Restore CI and release truth

**State:** `active`
**Depends on:** `ANI-Q01`

**Outcome:** All required workflows are green and the documented version matches a published artifact.

**Exit criteria:**

- [x] The lowercase checker accepts the corrected personal-model title.
- [ ] A tagged release backs the documented version or the version claim is removed.

**Current evidence:**

- PR #16 corrected the personal-model title; main at
  e036ca23c3ee81ea1bd962c7a066012a0d74d887 passes Linux stable, MSRV,
  macOS, and repository validation.
- Issue #10 remains the release-publication checkpoint; no release is claimed
  by this roadmap.

<!-- roadmap-step
id: ANI-Q03
status: complete
depends_on: [ANI-Q01]
issues: [18, 20, 22, 25, 27, 29]
-->
#### ANI-Q03 — Bound runtime and make Pipeline v3 resumable

**State:** `complete`
**Depends on:** `ANI-Q01`

**Outcome:** Long-running work has explicit resource limits, checkpoints, and deterministic resume behavior.

**Exit criteria:**

- [x] Required host CPU, memory, and storage are preflighted, while time,
  capture, artifact, and cancellation bounds are enforced in hermetic fixtures.
- [x] Interrupted work resumes without duplicating accepted outputs.

**Current evidence:**

- Issue #18 established the provider manifest, effective-configuration, and
  compatibility-fingerprint foundation.
- Issue #20 established explicit local registration, deterministic resolution, exact
  provider locks, declared host-resource preflight, process time and capture
  bounds, process-tree cancellation, artifact limits, and strict output
  validation.
- Issue #22 adapted pipeline v2 frame, batch, audio, and whole-video processors
  to that registry/runtime while retaining processor-specific validation and
  invocation evidence.
- Issue #25 and merged PR #26 established deterministic, read-only Pipeline v3
  planning and capability resolution.
- Issue #27 and merged PR #28 established bounded Pipeline v3 execution and
  deterministic resume: typed direct-argument invocation, exact-lock
  re-resolution, append-only run manifests, and immutable content-aware stage
  checkpoints.
- Issue #29 and merged PR #30 delivered the provider-conformance and authoring
  checkpoint: one coherent local bundle proves frame, audio, whole-video, and
  validator-evidence stages through registration, resolution, execution,
  checkpoint, status, and reuse. Issue #24 retains the exhaustive adversarial
  corpus.
- Kernel CPU/memory quotas, cross-run content-addressed reuse, multi-artifact
  output ports, and general validator-provider selection remain roadmap gaps.

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

**Current checkpoint:** issues #18 and #20 provide provider-native declarations,
configuration identity, compatibility fingerprints, exact standalone
resolution, locks, and bounded local execution. Issue #22 applies those
facilities to the existing pipeline v2 processor families without removing the
deprecated renderflow compatibility handoff ahead of Pipeline v3.

### PR 6 — Deterministic Pipeline v3 planning

Normalize configuration into a serializable execution plan with a stable
digest, schema artifacts, capability resolution, typed argument expansion, and
actionable migration. Remove cross-holon selection from the new schema.

**Exit evidence:** identical resolved intent produces identical plans and
digests; unsupported capabilities and versions fail before expensive work.

**Current checkpoint:** issue #25 owns the read-only, versioned configuration,
resolved-plan, typed-diagnostic, migration, and library/CLI parity boundary;
merged PR #26 supplies its implementation evidence. Provider conformance
corpus work remains separately tracked by issue #24.

### PR 7 — Content-aware run state and resume

Replace blind completion markers with atomic, versioned stage evidence and
explainable compatibility decisions. Separate read-only inspection from
workspace mutation and represent complete lifecycle states.

**Exit evidence:** changing any relevant input, timeline, configuration,
processor, implementation, or validated output deterministically invalidates
the affected stage.

**Current checkpoint:** issue #27 and merged PR #28 add a bounded Pipeline v3 executor with
separate run, resume, and read-only status boundaries. It re-resolves every
selected provider to the exact plan lock, invokes it through a typed
direct-argument request, and publishes an immutable checkpoint only after the
declared output and built-in artifact-integrity validation pass. Append-only
run-manifest revisions explain compatible reuse and affected/downstream
invalidation. Pipeline v2 behavior remains unchanged.

This checkpoint deliberately supports one artifact per output port and only
`aniflow.validation/artifact-integrity/v1`. Unsupported cardinality,
validation contracts, and replay-unsafe effects such as publish fail before
workspace mutation or provider launch. Cross-run reuse and arbitrary DAG
execution remain later work.

Issue #29 and merged PR #30 published the first coherent provider conformance
bundle and extension-authoring guide. Its four profiles prove the supported
direct process boundary for frame, audio, whole-video, and evidence-producing
artifact validator stages. Provider-backed validation gates, broader platform
evidence, and the exhaustive malicious-provider corpus remain later work;
issue #24 owns that corpus.

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
