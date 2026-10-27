---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T17:32:59Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Reconcile bounded native chunks with merged named repair and selected media setup before #35 assembly."
  includes:
  - Existing v3 chunk checkpoints, complete-set collection and weighted progress
  - Named allenk repair and inert selected media setup retained together
  excludes:
  - conversation transcripts
  - private source media or local paths
  - native qualification, dependency acquisition and release publication
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
  - docs/upscayl-recovery.md
  - docs/validation/aniflow-37-chunks-checkpoint.md
  - docs/frame-repair.md
  - docs/toolchain-media-setup.md
  - docs/validation/aniflow-38-media-setup-checkpoint.md
  - ROADMAP.md
work:
  objective: Reconcile the authorized source merges, then start verified assembly through existing owners.
  success_conditions:
  - Complete chunks retain exact source, timing, tool/model, boundary and native-limit bindings.
  - Named repair and selected setup remain compatible; generic repair stays fresh-run only.
  - Setup and processing preserve distinct authority; no second runtime or checkpoint database.
  active_issue: "37"
  next:
    kind: action
    id: "35"
    description: "Publish and merge reconciled PR #127, then implement the authorized frame/audio assembly checkpoint."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/pull/127
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: e71a59ca35340c3beda0efab7838cfbc8718e4e1
    ref: refs/heads/main
    verified_at: "2026-10-27T17:32:59Z"
  candidate:
    branch: codex/37-upscayl-chunks
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/127
    handoff_state: reconciliation-in-progress
  live:
    status: partial
    observed_at: "2026-10-27T17:32:59Z"
    default_branch_revision: e71a59ca35340c3beda0efab7838cfbc8718e4e1
    issue_state: open
    pull_request_state: open-draft
    notes: "PR #128 merged at 52d84f237baafae7cdde36bff40ad2ccf745d9df; PR #129 merged at current main. PR #127 is being reconciled; its merge is pending. Parents #36, #37, #38 and #64 remain open."
  parallel_changes:
  - "PR #127 pre-reconciliation head is 246759871a6b705fbc34615185f9066397d3a8d0; re-query its published candidate before merging."
review:
  status: partial
  reviewed_at: "2026-10-27T17:32:59Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source conflict reconciliation
    outcome: limited
    observed_at: "2026-10-27T17:32:59Z"
    notes: Earlier chunk, named-repair and setup source reviews are retained in their handoffs. Combined-source reconciliation is being reviewed; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T17:32:59Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Native/tool/model and current-source execution qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Setup observations do not establish complete shared-library, host, model-origin or device identity.
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

## Current source and checkpoint

Read AGENTS.md and the [current roadmap](ROADMAP.md#current-source-and-work-order)
first. Live source and work-tracker state outrank this snapshot. Named repair
[PR #128](https://github.com/egohygiene/aniflow/pull/128) merged at
`52d84f237baafae7cdde36bff40ad2ccf745d9df`; selected media setup
[PR #129](https://github.com/egohygiene/aniflow/pull/129) then merged at current main
`e71a59ca35340c3beda0efab7838cfbc8718e4e1`. Earlier PR #125/#126 are merged;
#118 remains closed for its focused documentation scope.

[PR #127](https://github.com/egohygiene/aniflow/pull/127), branch
`codex/37-upscayl-chunks`, is being reconciled with that main before its
authorized merge. Its pre-reconciliation head is
`246759871a6b705fbc34615185f9066397d3a8d0`. Candidate revision remains null to
avoid a circular self-pin. Re-query the published head and live merge state;
this checkpoint does not claim PR #127 is already merged.

## Combined source boundary

Explicit bounded chunks extend the same `upscayl_recovery` facade and
`video upscale-frames` command. Per-frame mode remains capped at 1,024 frames;
chunks use 2–256 frames per native process, at most 1,024 chunks and 100,000
frames, subject to index, plan and storage bounds. A 5,602-frame input at 64
frames per chunk needs 88 processing stages and one complete-set collector.
Every frame fully decodes. Exact source/index/time, binary/model/settings,
chunk boundaries, native timeout and capture bounds govern accepted reuse.
Interrupted chunks retry in full; loose partials remain evidence. Progress counts
accepted/reused frames. Retained copies need disk; replacement is not in-place.

Named allenk repair shares immutable import and repair, with explicit
current/legacy profile, fixed arguments, version observation and whole-stage
resume. Exact same-run importer provenance and fresh component checks are
required; interrupted repair retries all selected frames. Generic arbitrary-argv
repair stays fresh-run only. Source clocks, cuts and unselected bytes are retained.

`toolchain prepare-media` derives one existing configuration from the selected
profile and inventory without media reads, process launch, file writes,
registration, installation or downloads. Its Upscayl mapping explicitly retains
per-frame policy; chunk mode remains a separate explicit recovery setting.
Ready means local inventory consistency, not native suitability or execution
permission. Exact plans, runtime freshness and accepted checkpoints remain the
owners of processing/reuse. No new scheduler, cache or checkpoint database exists.

## Continuation and qualification

After reconciling and merging PR #127, continue the authorized #35 assembly/audio
checkpoint and then the single music-video recipe. #36 retains pinned manifest
migration, other provider/clip limits/seams, optional detection and qualification.
#37 retains historical-output adoption and the full failure/native matrix.
#38 retains other typed mappings, optional probes, model discovery and complete
runtime/native qualification. #111 can proceed independently.

All tests/compiler/build, lint/format, schema/naming/drift/docs validators,
smoke/package, native/model/provider/CLI and hosted checks remain deferred under
#64; no Actions are dispatched or polled. Authored regressions remain unrun.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Source merge is not native safety, creative approval or release evidence.
Source package 0.3.0 remains unreleased. Retain the dated handoffs and receipts.
See the [chunk](docs/upscayl-recovery.md), [repair](docs/frame-repair.md) and
[setup](docs/toolchain-media-setup.md) guides. Keep the historical block below
verbatim; its dated observations grant no additional authority.

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
