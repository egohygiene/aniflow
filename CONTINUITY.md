---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-11-06"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Retain merged source checkpoints for #111/#35/#112/#113 and their remaining gates."
  includes:
  - Prior accepted spatial recipe entry
  - Exact audio extent and accepted signal-evidence view
  - Selected delivery metadata and private receipts
  - Read-only accepted completion projection
  - Historical source receipts and deferred qualification
  excludes:
  - private media and local paths
  - qualification execution and release publication
  precedence:
  - user-and-runtime-instructions
  - scoped-repository-instructions
  - live-repository-and-work-tracker-state
  - canonical-repository-sources
  - continuity-checkpoint
  canonical_sources:
  - AGENTS.md
  - ROADMAP.md
  - docs/music-video-recipe.md
  - docs/video-assembly.md
  - docs/delivery-metadata.md
  - docs/final-video-handoff.md
work:
  objective: "Complete the approved source batch while preserving explicit residual work."
  success_conditions:
  - Four feature source checkpoints are merged through separate PRs.
  - Existing acceptance and immutable source rules remain authoritative.
  - No qualification, approval or release is fabricated.
  active_issue: "113"
  next:
    kind: action
    id: "35"
    description: "Sync on merged source; automatic source-bound PCM/meter composition remains a possible next bounded feature."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 020d4324fba793217435d97dcad7fe9db02bd055
    ref: refs/heads/main
    verified_at: "2026-11-06"
  candidate:
    branch: codex/flow-batch5-roadmap-receipt
    revision: null
    pull_request: null
    handoff_state: source-prepared
  live:
    status: partial
    observed_at: "2026-11-06"
    default_branch_revision: 020d4324fba793217435d97dcad7fe9db02bd055
    issue_state: open
    pull_request_state: merged
    notes: "PRs #137/#138/#139/#140 merged. Full issue acceptance/qualification remains open; verify later live state."
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-11-06"
  reviewed_by: codex
  evidence:
  - command: "Read-only source and Git metadata/content inspection"
    outcome: limited
    observed_at: "2026-11-06"
    notes: "Eighteen new focused regression cases authored, uncompiled/unrun. Source inclusion is not qualification."
  environment_limitations:
  - "All tests/compiler/build/dependency/lint/format/schema/docs/native/media/provider/CI execution deferred under #64."
  - "Known alignment cancellation/start-marker failure unresolved, cause unproven."
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

## November 6, 2026 source continuation

PR #137 (prior accepted spatial input), #138 (exact audio extents/signal view),
#139 (selected metadata policy) and #140 (read-only completion) are merged in
order. Main was observed at `020d4324fba793217435d97dcad7fe9db02bd055`, tree
`ee665b0c5ea7cb3d96696dd6cbd77da5d5860eda`. This documentation continuation
retains frontmatter and current merge receipts; query live main before continuing. See the new
[handoff guide](docs/final-video-handoff.md) and each dated validation checkpoint.
#111/#35/#112/#113 retain explicit residual source/integration/qualification work;
no acceptance boxes, proposed ADR status or releases are silently advanced.
All checks/execution remain deferred under #64, including the known unresolved
alignment cancellation/start-marker failure (cause unproven). The older snapshot
and prose below are retained historical observations, superseded where stated
by these current receipts and the live roadmap.

## Retained October 28 source checkpoint

The current #24 [demo](docs/video-demo.md) is saved in [draft PR #135](https://github.com/egohygiene/aniflow/pull/135)
on `codex/24-synthetic-video-demo`, based on saved PR #134 head
`1b09f835351800a45d570233f4981fdadf7aae99` (tree
`b34865a681ff4546ccbc751e0fe1700fe2b60ccd`). Generation uses explicitly supplied
FFmpeg/ffprobe 8.0.1 and creates local recipe settings; it never runs aniflow
or tests automatically. Current generation and qualification remain unrun.
See the [demo handoff](docs/validation/aniflow-24-video-demo-checkpoint.md).
First saved head `7dcb367a9541294a31ec0fa85ef2e7f343199338`, tree
`1a4709ba04253847d4b8adbb0d8a761560a8afca`: all nine blobs/tree matched local source.
The saved source-entry checkpoint below remains the implementation base; query the live draft head before continuation.

Read AGENTS.md and the [current roadmap](ROADMAP.md#current-source-and-work-order)
first. Main is `e1634df58fa32feda23021565b032a83c5a30d66` after authorized
merges of assembly PR #130 (`41d1706f5484218df4ac8f8e0a94548976ef8869`),
recipe PR #131 (`822c6582a8bdcf410bff0d1f1b755618249ba978`) and target
resolution PR #132 (current main). Its tree is
`f70ffe58fc2b225c498feffc566baaff70203f36`. Prior first-save and source-review
receipts remain in their dated handoffs. Merge establishes source inclusion,
not qualification or ADR acceptance.

Branch `codex/35-source-music-video-recipe` starts at the saved extraction
[draft PR #133](https://github.com/egohygiene/aniflow/pull/133) head
`9e05b05574ad03cb94137f90e3525cbf51a5ebed`, tree
`ab6c2d0b4cfb0cee7d81bc2ccc30e20a055385a7`. PR #133 remains open draft;
this continuation does not merge it. The new
[draft PR #134](https://github.com/egohygiene/aniflow/pull/134) was created on
2026-10-28 at 05:53:28Z (UTC), targeting the extraction branch. First saved head:
`55a0ba322ec914b486dd233e00c4166b1d40f2bb`; tree:
`e84c2383a7b4dc7d0ea54d88901e037c1573281a`. All 25 published blobs and the
exact tree matched reviewed local source. Candidate revision remains null;
these first-save receipts do not self-pin later documentation changes. Re-query
the PR live head before continuation.

## Source-entry boundary

Source entry owns one immutable intent and root; accepted extraction supplies
the shared indexed tail in memory. Indexed v1 contracts remain unchanged.
Geometry and conditional neural admission await accepted frames. Existing v3
plans, writer ownership, validation and recovery remain authoritative; partial
state is retained. Completed compatible resume avoids fresh native work.
See the [recipe guide](docs/music-video-recipe.md) for the retained boundary.

## Continuation and qualification

Root and independent source review found no remaining concrete blocker.
Review demo draft #135 and sync before returning to #111's next feature. #35 remains
open for broader media/audio-tail profiles,
loudness/metadata and complete workflow qualification. #111 retains prior
Upscayl spatial-index recipe entry and qualification. #107 retains manual/review
recipes; #36/#37/#38/#110 keep their residual scope. #83 destination copies and
#82 proofs remain separate. See the [recipe guide](docs/music-video-recipe.md)
and [handoff](docs/validation/aniflow-35-source-recipe-checkpoint.md).

All tests/compiler/build, lint/format, schema/naming/drift/docs validators,
smoke/package, native/model/provider/CLI and hosted checks remain deferred under
#64; no Actions are dispatched or polled. Authored regressions remain unrun.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Source package 0.3.0 remains unreleased; ADR-0012 and its appended
source-entry extension stay proposed. Keep the historical block below verbatim;
dated observations grant no authority.

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
