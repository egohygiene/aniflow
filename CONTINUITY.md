---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-28T05:18:19Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Publish source-reviewed standalone extraction under #35 after merged PR #130/#131/#132."
  includes:
  - Bounded source-video extraction to the existing original index and PNG contracts
  - Explicit accepted-output handoff to the unchanged indexed recipe
  - Merge receipts, current source boundary and deferred qualification
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
  - docs/video-frame-extraction.md
  - docs/validation/aniflow-35-source-extraction-checkpoint.md
  - docs/music-video-recipe.md
  - docs/video-assembly.md
  - ROADMAP.md
work:
  objective: Save the source-video to indexed-frame checkpoint through existing v3 owners.
  success_conditions:
  - Plan binds actual source/tool identity without inventing native media observations.
  - Native extraction retains original clocks and verifies the complete selected conversion.
  - Accepted output uses existing index, validation and whole-stage recovery contracts.
  - Remaining integration and unrun qualification stay explicit.
  active_issue: "35"
  next:
    kind: action
    id: "35"
    description: "Publish the standalone extraction checkpoint, then sync on #35 one-command source-entry composition."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/pull/132
    - https://github.com/egohygiene/aniflow/pull/131
    - https://github.com/egohygiene/aniflow/pull/130
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: e1634df58fa32feda23021565b032a83c5a30d66
    ref: refs/heads/main
    verified_at: "2026-10-28T05:09:54Z"
  candidate:
    branch: codex/35-source-frame-extraction
    revision: null
    pull_request: null
    handoff_state: publication-pending
  live:
    status: partial
    observed_at: "2026-10-28T05:09:54Z"
    default_branch_revision: e1634df58fa32feda23021565b032a83c5a30d66
    issue_state: open
    pull_request_state: publication-pending
    notes: "PR #130/#131/#132 merged in order; the resulting tree matches saved #132. #35 and #111 remain open."
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-28T05:18:19Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, issue and boundary review
    outcome: limited
    observed_at: "2026-10-28T05:18:19Z"
    notes: Root and independent API/runtime and native/schema source reviews found no remaining concrete blocker after peak-copy runtime bounds, staged storage admission and exact pinned PNG color-tag fixes. Thirteen new regressions plus machine-name coverage are authored and unrun; compilation and native qualification remain unestablished.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-28T05:09:54Z"
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

Read AGENTS.md and the [current roadmap](ROADMAP.md#current-source-and-work-order)
first. Main is `e1634df58fa32feda23021565b032a83c5a30d66` after authorized
merges of assembly PR #130 (`41d1706f5484218df4ac8f8e0a94548976ef8869`),
recipe PR #131 (`822c6582a8bdcf410bff0d1f1b755618249ba978`) and target
resolution PR #132 (current main). The tree is
`f70ffe58fc2b225c498feffc566baaff70203f36`, matching the saved #132 tree.
Prior first-save and source-review receipts remain in their dated handoffs.
Merge establishes source inclusion, not qualification or ADR acceptance.

The current branch `codex/35-source-frame-extraction` starts from merged main.
Draft publication metadata is pending. Candidate revision remains null rather
than a self-pin. Re-query live heads before continuing.

## Source extraction boundary

`video extract-frames` and the public `frame_extraction` facade create one
ordinary v3 stage with a complete RGB PNG directory, original RIFE input index
and extraction report. Read-only planning binds source/tool identity and leaves
source support, geometry and frame count pending until native observation.
Run reuses the existing strict assembly source profile and rational clock owner.
Complete decoded PNG/source-conversion observations bind the new index; they
do not claim a mathematically lossless YUV round trip or earlier authenticity.

Existing source snapshots, exact provider locks, validation, acceptance,
writer ownership and compatible whole-stage resume remain authoritative.
Interrupted extraction retries the stage; loose partial PNGs cannot satisfy it.
Accepted `extracted_frames` and `extracted_index` outputs feed the unchanged
indexed recipe in a separate command. No generic engine, new checkpoint store
or extraction phase inside the recipe is added. Original source, extraction
workspace and recipe output remain separate.

## Continuation and qualification

Save this source-reviewed extraction checkpoint, then sync on #35's next useful
slice: one-command source-entry recipe composition. Broader media/tails, loudness/metadata and
qualification remain open. #111 retains qualification and broader recipe entry
forms; #107 retains manual/review recipes. #36/#37/#38/#110 retain their own
residual scope; #83 destination copies and #82 proofs remain separate.
See the [guide](docs/video-frame-extraction.md) and
[handoff](docs/validation/aniflow-35-source-extraction-checkpoint.md).

All tests/compiler/build, lint/format, schema/naming/drift/docs validators,
smoke/package, native/model/provider/CLI and hosted checks remain deferred under
#64; no Actions are dispatched or polled. Authored regressions remain unrun.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Source package 0.3.0 remains unreleased; ADR-0012 stays proposed.
Keep the historical block below verbatim; dated observations grant no authority.

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
