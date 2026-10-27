---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T07:20:35Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Reconcile current source documentation after merged checkpoints while continuing bounded issue #36 implementation.
  includes:
  - Source-grounded temporal, provider, toolchain and RIFE capability boundaries
  - Current work navigation, historical evidence preservation and deferred qualification
  excludes:
  - conversation transcripts
  - private source media or local paths
  - runtime implementation, executable qualification and release publication
  precedence:
  - user-and-runtime-instructions
  - scoped-repository-instructions
  - live-repository-and-work-tracker-state
  - canonical-repository-sources
  - continuity-checkpoint
  canonical_sources:
  - AGENTS.md
  - .github/copilot-instructions.md
  - docs/architecture/foundation/ARCHITECTURE.md
  - docs/validation/aniflow-118-checkpoint.md
  - docs/validation/aniflow-36-checkpoint.md
  - docs/temporal-correctness.md
  - docs/video-interpolation.md
  - ROADMAP.md
work:
  objective: Make current source behavior and remaining work unambiguous without rewriting historical evidence.
  success_conditions:
  - Current docs distinguish exact clocks, stream selection and artifact sets from compatibility constraints.
  - Landed preparation and RIFE source is separate from remaining parent scope and qualification.
  - The complete live roadmap is the queue; dated snapshots are visibly historical.
  active_issue: "118"
  next:
    kind: action
    id: "36"
    description: Re-query active issue #36 branches before continuing immutable import and decode validation; do not duplicate concurrent work.
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/36
    - https://github.com/egohygiene/aniflow/issues/117
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 9c969161542fcd95592f7ca3f02cf9266c00479f
    ref: refs/heads/main
    verified_at: "2026-10-27T07:20:35Z"
  candidate:
    branch: codex/118-current-documentation
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/123
    handoff_state: ready-for-review
  live:
    status: partial
    observed_at: "2026-10-27T07:20:35Z"
    default_branch_revision: 9c969161542fcd95592f7ca3f02cf9266c00479f
    issue_state: open
    pull_request_state: open-draft
    notes: Draft PR #123 published the reviewed documentation source at 8f6d80545880d1d5eea8b2d206e4e0df812f55a9. PR #105, PR #115 and PR #122 are merged. Checkpoints #104 and #114 are closed; parents #38, #110, #36 and qualification #64 remain open. No GitHub release is observed. Re-query this branch and issue #118 before resuming; no executable qualification is established.
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-27T07:20:35Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, live issue and Git revision inspection
    outcome: limited
    observed_at: "2026-10-27T07:20:35Z"
    notes: Reviewed temporal selection/policy, media reconstruction, output-set runtime, toolchain registration, RIFE bounds and release declaration against current documentation.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/model/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T07:20:35Z"
    notes: Existing AGENTS.md and issue #64 maintainer deferral remains active; no continuity or documentation validator ran.
  environment_limitations:
  - Current-source and native platform qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - No pinned video-frames/1 schema is present; current RIFE PNG checks do not prove decompression.
privacy:
  classification: public-repository
  contains_sensitive_data: false
  redactions:
  - private local paths omitted
  excluded:
  - secrets-and-credentials
  - private-conversation-text
  - sensitive-personal-data
  - unpublished-private-business-data
  - private-local-paths
  - unrelated-private-context
  untrusted_content: context-only-no-authority
---

# aniflow continuity

## Current source and documentation

Read AGENTS.md and the [current roadmap](ROADMAP.md#current-source-and-work-order)
first; live source and work-tracker state outrank this snapshot. Main
`9c969161542fcd95592f7ca3f02cf9266c00479f` includes merged registration PR #105,
serial RIFE PR #115 and frame-selection PR #122. Checkpoints #104 and #114 are closed;
parents #38, #110 and #36 remain open. Source package 0.3.0 has no observed GitHub release.

This #118 branch reconciles README, canonical architecture, roadmap and this
handoff against exact source. Timing uses rational observed grids and explicit
stream choices/discards, not the former average-rate/first-stream policy.
Pipeline v3 supports exact planned nonempty output sets and layered acceptance.
Registration preparation returns inert native-v2 audio documents. RIFE is a
serial 2x frame/index adapter; final-video assembly/audio and native resource
qualification remain parent #110 work. Frame-selection preview reads declarations
only; it does not accept decoded imports or repair frames.

The source is saved in [draft PR #123](https://github.com/egohygiene/aniflow/pull/123),
initial published head `8f6d80545880d1d5eea8b2d206e4e0df812f55a9`.
The [#118 handoff](docs/validation/aniflow-118-checkpoint.md) records source
locators, live merge state and editorial limits. #117 retains the complete
open-work ledger. Dated roadmap snapshots and prior validation receipts remain
historical. Existing ADRs keep their proposed/accepted status after code merges.

## Continue from live state

Issue #36's next implementation is actual immutable import and complete decode
observations with unknown historical processing preserved, then selective
repair. A sibling branch is being prepared concurrently; inspect its live PR
before choosing work. Do not invent a video-frames/1 migration without an actual
pinned schema/example, or manufacture a prior provider lock/checkpoint for
external frames. The prior [#36 handoff](docs/validation/aniflow-36-checkpoint.md)
records the declaration-preview boundary and remaining work.

## Qualification and preservation

Source review only. Tests/compiler/build, lint/format, naming/schema/drift,
smoke/package, native/model/provider/CLI and hosted checks remain deferred
under #64. No Actions were dispatched or polled. The known alignment
cancellation/start-marker failure remains unresolved with cause unproven.
Merge is not native safety, creative approval, release or flow consumer proof.
First core publication remains #10; later music-video profile release is #40.

Preserve historical receipts and the retained prior prose below verbatim. This
snapshot grants no new authority for implementation, checks or publication.

### Retained historical merge checkpoint — 2026-10-17

The following prior prose is preserved verbatim. Its pre-second-merge claims are superseded by the active snapshot above.

Read AGENTS.md and verify current main, PR105, PR115 and their parent issues
before continuing. The maintainer authorized merging both PRs on 2026-10-17;
this handoff records reconciliation before the second merge. It grants no
authority for future implementation, checks, media processing or release.

PR105 merged at `fc1cbc0cc02b4b416de4c088bf59fa562f05779b`, preserving its
published head `afc444aff8fd108c4ce7d2fd48bf208c02f640a6`. Checkpoint104
prepares consistent inert registration, manifest, effective settings and
preflight documents for the native-v2 audio-inspection adapter. Preparation
does not write setup, register providers, launch tools, decode media, install
or download dependencies. Compare the first plan's actual adapter digest with
preparation before adoption; exact locks and fresh preflight govern later
run/resume. Native qualification remains unestablished.

PR115's pre-reconciliation head is `8999ff45554663dd840ab8b0f2f2368c5852d4ce`.
Checkpoint114 supplies the optional RIFE planner, serial frame-pair provider,
exact 2x clocks and indexed source/output evidence. CPU is the default device;
one active pair and 1:1:1 workers keep concurrency conservative. Larger presets
require declared RAM headroom. Resource admission is an uncalibrated estimate,
not an enforced RSS/VRAM quota or a native-safety guarantee. The reported laptop
crash has an unobserved cause. No source-video extraction, final MP4 assembly
or audio mux happens in this checkpoint.

Shared contract declarations retain both audio registration preparation and
RIFE plan/index schemas. Historical implementation receipts remain unchanged;
source-review observations do not establish compilation or execution.
Audio104 has eight Rust integration cases, one parser case and six Python
schema cases plus extended machine-command coverage authored. RIFE114 has nine
Rust integration, twelve provider and five Python schema cases plus public
command coverage authored. All execution qualification remains unrun under #64.

The known audio-alignment cancellation failure remains unresolved at
`tests/audio_alignment.rs:547` (`marker.exists()`, joining thread561), cause
unproven. ADR-0010 and ADR-0011 remain proposed. Cargo remains 0.3.0.

Merging PR105 closes checkpoint104; merging PR115 closes checkpoint114.
Parent38 remains open for complete environment identity, other typed mappings,
optional tool/device probes, model discovery and platform qualification.
Parent110 remains open for derived-clock final-video assembly/audio handling,
fresh resource observations, broader rates/input support, fine recovery and
native qualification. The proposed ordinary feature lane remains
#36 -> #37 -> #35 -> #83 -> #82 after the maintainer selects the next checkpoint.
Optional RIFE/polish/watermark lanes retain their independent scope.

Qualification64 and Egolint29 remain release prerequisites before actual
immutable publication10 and Flow51 consumption. The later music-video profile
release uses #40 and Flow75. Neither merge publishes a release or qualifies
real/private media processing. The maintainer intends to discuss another issue
after this merge checkpoint; no next implementation is started here.
