---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T17:49:28Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Author the selected original-clock audiovisual assembly checkpoint after the authorized source merges."
  includes:
  - Two ordinary v3 stages for an FFV1 MP4 master and H.264 MP4 delivery
  - Original AAC packet verification and explicit indexed-frame provenance limits
  - Manual repair/upscale handoff, existing plan/run/resume/status and parent scope
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
  - docs/video-assembly.md
  - docs/validation/aniflow-35-assembly-checkpoint.md
  - docs/upscayl-recovery.md
  - docs/frame-repair.md
  - docs/toolchain-media-setup.md
  - ROADMAP.md
work:
  objective: Review the published assembly checkpoint and select the remaining parent scope.
  success_conditions:
  - Source and output geometry stay distinct while exact original clocks are retained.
  - Both MP4 stages verify original AAC payload, timing, extradata and priming.
  - Master pixel observations and current component identities govern acceptance.
  - Broader parent scope and all unrun qualification remain explicit.
  active_issue: "35"
  next:
    kind: action
    id: "35"
    description: "Review draft PR #130, then select the remaining full-workflow and media-profile work."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/pull/130
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 8cb00a323b1829de1e4b958236b6aa8529eb7045
    ref: refs/heads/main
    verified_at: "2026-10-27T17:46:40Z"
  candidate:
    branch: codex/35-verified-frame-assembly
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/130
    handoff_state: published-draft
  live:
    status: partial
    observed_at: "2026-10-27T17:49:28Z"
    default_branch_revision: 8cb00a323b1829de1e4b958236b6aa8529eb7045
    issue_state: open
    pull_request_state: open-draft
    notes: "PR #128 merged at 52d84f237baafae7cdde36bff40ad2ccf745d9df; PR #129 at e71a59ca35340c3beda0efab7838cfbc8718e4e1; PR #127 at current main. Parents #35, #36, #37, #38 and #64 remain open."
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-27T17:49:28Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source and interface review
    outcome: limited
    observed_at: "2026-10-27T17:49:28Z"
    notes: Source review corrected exact validator substitution and resume storage overreservation. Ten public, four media and three CLI regressions plus stable machine-name coverage are authored and unrun; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T17:49:28Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Native/tool/model and current-source execution qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Source-supported media behavior does not qualify installed builds, shared libraries or hosts.
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
PR #128 merged at `52d84f237baafae7cdde36bff40ad2ccf745d9df`; selected media
setup PR #129 merged at `e71a59ca35340c3beda0efab7838cfbc8718e4e1`; bounded
native chunks PR #127 merged at current main
`8cb00a323b1829de1e4b958236b6aa8529eb7045`. Earlier PR #125/#126 are merged.

The authorized #35 checkpoint is published as
[draft PR #130](https://github.com/egohygiene/aniflow/pull/130) on
`codex/35-verified-frame-assembly`, observed open on 2026-10-27 at 17:49:28Z.
First saved implementation: `ca37e01be4d366c3262391f6405e06aebf4683c2`;
tree: `786580c491c1d049a2ac03a42b2a1f039fb13d72`. This dated receipt is not a
self-pin for later edits; candidate revision remains null. Re-query the live
head before continuing. The [handoff](docs/validation/aniflow-35-assembly-checkpoint.md)
records source review and unrun coverage. Parent #35 remains open.

## Assembly boundary

`video_assembly` and `video assemble-frames` bind the original video, complete
PNG directory/index and explicit FFmpeg/ffprobe 8.0.1 pins. Original input
indices and Upscayl spatial output indices retain source geometry and clocks
separately from output geometry. Imported PNG/source lineage remains declared;
earlier processing stays unknown. The profile does not accept RIFE-derived clocks.

Two existing v3 stages produce an audiovisual FFV1/bgr0 MP4 master and an
H.264/yuv420p/faststart MP4 delivery. When audio is selected, both copy original
admitted AAC and compare packet payloads, exact integer timestamps, codec
extradata and priming. The
master additionally compares decoded RGB pixels and exact frame clocks. Strict
zero-origin CFR/SDR admission refuses unsupported offsets, long A/V tails,
VFR, HDR, alpha and rotation rather than silently normalizing them.

Plan/run/resume/status use the existing provider/runtime, exact locks and
immutable acceptance. No new scheduler, mux service or checkpoint database is
introduced. Interrupted stages retry whole; compatible accepted master work can
be reused. Source data and prior accepted runs remain immutable. Resource
bounds do not certify host suitability or enforce an RSS/VRAM quota.

The guide connects selected setup, repair/upscale and assembly through explicit
manual artifact handoffs. It does not claim an automated complete recipe.
Selected setup still maps Upscayl to per-frame policy; chunks are separately
explicit. #36/#37/#38 retain their migration, provider, adoption, tool/model,
environment and qualification scope. #111 can proceed independently.

## Continuation and qualification

Review draft PR #130, keeping #35 open for the complete workflow,
broader profiles and acceptance. Destination copies #83 and optional proofs #82
remain separate. RIFE derived-clock/audio work remains #110. Dated prior handoffs
retain their exact historical observations.

All tests/compiler/build, lint/format, schema/naming/drift/docs validators,
smoke/package, native/model/provider/CLI and hosted checks remain deferred under
#64; no Actions are dispatched or polled. Authored regressions remain unrun.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Source merge is not native safety, creative approval or release evidence.
Source package 0.3.0 remains unreleased. Keep the historical block below verbatim;
its dated observations grant no additional authority.

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
