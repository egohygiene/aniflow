---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T18:20:11Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Retain published recipe draft PR #131 stacked on open assembly draft PR #130."
  includes:
  - Existing indexed inputs, optional named repair, optional Upscayl and assembly
  - Explicit disabled reasons and honest pending downstream exact plans
  - Existing child v3 runs as checkpoint, validation and resume authority
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
  - docs/music-video-recipe.md
  - docs/validation/aniflow-35-recipe-checkpoint.md
  - docs/video-assembly.md
  - docs/validation/aniflow-35-assembly-checkpoint.md
  - docs/upscayl-recovery.md
  - docs/frame-repair.md
  - ROADMAP.md
work:
  objective: Review the published recipe stack and discuss the next source checkpoint.
  success_conditions:
  - Optional steps retain explicit intent and disabled reasons.
  - Exact downstream plans wait for accepted frame bytes and updated indices.
  - Child runs retain current source, tool, plan, output and acceptance authority.
  - Broader recipe scope and unrun qualification remain explicit.
  active_issue: "35"
  next:
    kind: action
    id: "111"
    description: "Discuss opt-in target-resolution policy as the next separate source proposal; no new issue work has started."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/pull/131
    - https://github.com/egohygiene/aniflow/pull/130
    - https://github.com/egohygiene/aniflow/issues/111
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/issues/107
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 1893bfb994d79e75b08d32739ed61aa23e58cde9
    ref: refs/heads/codex/35-verified-frame-assembly
    verified_at: "2026-10-27T17:58:01Z"
  candidate:
    branch: codex/35-music-video-recipe
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/131
    handoff_state: published-draft
  live:
    status: partial
    observed_at: "2026-10-27T18:20:11Z"
    default_branch_revision: 8cb00a323b1829de1e4b958236b6aa8529eb7045
    issue_state: open
    pull_request_state: open-draft
    notes: "PR #130/#131 remain open drafts; recipe #131 targets the assembly branch, not merged main. PR #128/#129/#127 are merged. Parents #35/#36/#37/#38/#64/#107 remain open."
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-27T18:20:11Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source and boundary review
    outcome: limited
    observed_at: "2026-10-27T18:20:11Z"
    notes: Root and independent media/API source review found no remaining concrete blocker after bounded hashing, original-index binding, child locks, exact-plan handoffs and recovery-root fixes. Sixteen regressions plus machine-name mapping are authored and unrun; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T18:20:11Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Native/tool/model and current-source execution qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Fixed composition does not broaden child media, host, device or resource qualification.
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
first. Live source and work-tracker state outrank this snapshot. Main remains
`8cb00a323b1829de1e4b958236b6aa8529eb7045`, including merged named repair
PR #128, selected media setup PR #129 and bounded chunks PR #127.

Assembly [draft PR #130](https://github.com/egohygiene/aniflow/pull/130) remains
open on `codex/35-verified-frame-assembly`. Its first saved implementation
`ca37e01be4d366c3262391f6405e06aebf4683c2` and tree
`786580c491c1d049a2ac03a42b2a1f039fb13d72` remain dated historical receipts.
Its later saved head `1893bfb994d79e75b08d32739ed61aa23e58cde9` is the base
of `codex/35-music-video-recipe`, published as
[draft PR #131](https://github.com/egohygiene/aniflow/pull/131) at 18:20:11Z on
2026-10-27. First saved commit: `913e2e03b1b135043079dc88b259e15b6a69b690`;
tree: `b84b046868d8f8cfa6fe233285aea5614cf1203a`. This is a dated first-save
receipt; candidate revision remains null, not a self-pin. Both drafts remain
open and main unchanged. Re-query live heads; retain the prior handoffs.

## Fixed recipe boundary

The recipe starts with an original source video and complete indexed PNGs.
Its order is fixed: frame preparation, optional Upscayl, then assembly. Named
allenk repair includes import; disabled repair selects standalone import.
Optional work is explicitly enabled or disabled with a reason.

Each enabled operation delegates to its existing child v3 run. Immutable recipe
intent and fixed child paths connect requested work to those runs; they are
not new checkpoint or acceptance authority. Initial planning binds the available
inputs, with later exact plans pending until accepted predecessor output and
its matching index exist. Execution resolves those plans in order. Existing
child locks, validation, immutable checkpoints and resume govern acceptance.
Before pending native work, the shared assembly source observer checks support.
Plan/status and fully accepted resume launch no native source probe. Status
still reads bounded recorded inputs/components for compatibility.

Assembly retains original-clock progressive zero-origin CFR, explicit opaque
8-bit BT.709 SDR and optional original AAC-LC passthrough. Source and upscaled
geometry remain distinct; offsets, long tails, HDR, VFR, unsupported side data
and RIFE-derived clocks are refused. Imported PNG/source lineage remains
declared and earlier processing unknown. Recipe composition does not upgrade
those provenance facts or qualify tools, hosts, playback or visual quality.

The recipe does not extract frames or implement arbitrary stages, manual
review/approval automation, a general DAG, a new runtime or a global cache.
#107 retains broader manual/review recipes. #36/#37/#38 retain their existing
migration, provider, adoption, environment and qualification scope.

## Continuation and qualification

Review the published stack. The next source proposal is separate opt-in
target-resolution policy #111; no new issue work has started. #35 retains initial
extraction, broader media profiles, loudness/metadata and qualification. #110
retains RIFE clock/audio; #83 destination copies and #82 proofs remain separate.
The [recipe guide](docs/music-video-recipe.md) and
[handoff](docs/validation/aniflow-35-recipe-checkpoint.md) describe this boundary.

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
