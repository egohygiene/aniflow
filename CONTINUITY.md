---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-28"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Save the #24 hands-on synthetic video demo above source-entry draft PR #134."
  includes:
  - Watchable synthetic video generation and fresh-checkout recipe walkthrough
  - Source configuration and fixed extraction-to-shared-recipe composition
  - Preserved indexed v1 contracts and existing child acceptance authority
  - Saved base receipts, remaining work and deferred qualification
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
  - docs/validation/aniflow-35-source-recipe-checkpoint.md
  - docs/video-frame-extraction.md
  - docs/video-assembly.md
  - ROADMAP.md
  - docs/video-demo.md
  - docs/validation/aniflow-24-video-demo-checkpoint.md
work:
  objective: Save reproducible video demos and local trial instructions, then return to the planned feature work.
  success_conditions:
  - Watchable synthetic clips and local recipe settings have explicit generation and trial commands.
  - One source intent coordinates extraction and the existing tail under one root.
  - Only accepted extraction supplies downstream frame identities and target geometry.
  - Existing indexed API/wire contracts and child acceptance/recovery remain authoritative.
  - Unrun qualification and parent issue residual scope stay explicit.
  active_issue: "24"
  next:
    kind: action
    id: "24"
    description: "Publish the reviewed demo checkpoint, then sync; execution remains deferred."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/24
    - https://github.com/egohygiene/aniflow/pull/134
    - https://github.com/egohygiene/aniflow/pull/133
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/issues/111
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on:
    - https://github.com/egohygiene/aniflow/pull/134
    - https://github.com/egohygiene/aniflow/pull/133
state:
  base:
    revision: 1b09f835351800a45d570233f4981fdadf7aae99
    ref: refs/heads/codex/35-source-music-video-recipe
    verified_at: "2026-10-28"
  candidate:
    branch: codex/24-synthetic-video-demo
    revision: null
    pull_request: null
    handoff_state: source-reviewed-publication-pending
  live:
    status: partial
    observed_at: "2026-10-28"
    default_branch_revision: e1634df58fa32feda23021565b032a83c5a30d66
    issue_state: open
    pull_request_state: not-published
    notes: "Demo work starts from open draft PR #134 above #133. No merges or execution qualification occur in this continuation. #24/#35/#111 remain open."
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-28"
  reviewed_by: codex
  evidence:
  - command: Synthetic demo source review
    outcome: limited
    observed_at: "2026-10-28"
    notes: Root and independent manual review found no remaining concrete blocker. Five pure regressions are authored and unrun; small demo resource limits and malformed-observation refusal were corrected. Publication is pending.
  - command: Read-only root and independent source review
    outcome: limited
    observed_at: "2026-10-28T05:50:33Z"
    notes: No remaining concrete blocker after exact extraction output-port handoff, root intent re-read before extraction and shared accepted tail status reuse. Seventeen new regressions are authored and unrun (10 public, 5 CLI, 2 exact-child), with existing CLI mode/recovery tests extended.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-28T05:48:31Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Native/tool/model and current-source execution qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Source conversion evidence is not authenticity, visual-quality or native-host qualification.
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

The current #24 slice adds a [watchable synthetic video demo](docs/video-demo.md)
on branch `codex/24-synthetic-video-demo`, based on saved PR #134 head
`1b09f835351800a45d570233f4981fdadf7aae99` (tree
`b34865a681ff4546ccbc751e0fe1700fe2b60ccd`). Generation uses explicitly supplied
FFmpeg/ffprobe 8.0.1 and creates local recipe settings; it never runs aniflow
or tests automatically. Current generation and qualification remain unrun.
See the [demo handoff](docs/validation/aniflow-24-video-demo-checkpoint.md).
The saved source-entry checkpoint below remains the implementation base.

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
Save the #24 demo draft and sync before returning to #111's next feature. #35 remains
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
