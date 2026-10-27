---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T17:20:24Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Prepare selected allenk repair and Upscayl configurations from explicit toolchain evidence for #38."
  includes:
  - One explicitly selected capability and dependency mapping with typed profile settings
  - Bounded local inspection, diagnostics and inert existing-format configuration output
  excludes:
  - conversation transcripts
  - private source media or local paths
  - native qualification, dependency acquisition, automatic registration and release publication
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
  - docs/toolchain-media-setup.md
  - docs/validation/aniflow-38-media-setup-checkpoint.md
  - docs/toolchain-upstreams.md
  - docs/frame-repair.md
  - docs/upscayl-recovery.md
  - ROADMAP.md
work:
  objective: Derive existing typed media configurations without adding another runtime or implicit setup.
  success_conditions:
  - Profile effective settings remain the sole options source; dependency roles are explicit.
  - Ready means local inventory consistency, never native qualification or execution authority.
  - Existing media facades retain exact registration, source admission, processing checks and resume.
  active_issue: "38"
  next:
    kind: action
    id: "38"
    description: "Finish source review and publish the selected media setup draft, then sync before #35 composition."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/38
    - https://github.com/egohygiene/aniflow/issues/35
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 221af01edcbe68de56431ec0f39f76b08d6fc7a2
    ref: refs/heads/codex/36-named-frame-repair
    verified_at: "2026-10-27T17:20:24Z"
  candidate:
    branch: codex/38-selected-media-setup
    revision: null
    pull_request: null
    handoff_state: in-progress
  live:
    status: partial
    observed_at: "2026-10-27T17:20:24Z"
    default_branch_revision: 2add9a6b230aaf415288e6c128bcf276dc05681c
    issue_state: open
    pull_request_state: not-created
    notes: "This setup branch is stacked on open repair draft #128. Parallel chunk draft #127 remains separate. No merge occurred; #38 and #64 remain open."
  parallel_changes:
  - "PR #127 bounded Upscayl chunks at 246759871a6b705fbc34615185f9066397d3a8d0 are not included in this branch."
review:
  status: partial
  reviewed_at: "2026-10-27T17:20:24Z"
  reviewed_by: codex
  evidence:
  - command: Read-only selected mapping and pinned upstream interface inspection
    outcome: limited
    observed_at: "2026-10-27T17:20:24Z"
    notes: Source and synthetic regressions are being authored and reviewed; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T17:20:24Z"
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
first. Live source and work-tracker state outrank this snapshot. Main remains
`2add9a6b230aaf415288e6c128bcf276dc05681c`, with generic repair PR #125 and
per-frame Upscayl recovery PR #126 merged. Documentation issue #118 is closed.

This branch, `codex/38-selected-media-setup`, is stacked on named repair
[draft PR #128](https://github.com/egohygiene/aniflow/pull/128) at
`221af01edcbe68de56431ec0f39f76b08d6fc7a2`. Its typed allenk profile and
whole-stage resume are part of this base, not merged main. Candidate revision
stays unset to avoid a circular self-pin; re-query the new draft before continuing.
[Chunk draft PR #127](https://github.com/egohygiene/aniflow/pull/127) remains
separate at `246759871a6b705fbc34615185f9066397d3a8d0`; its code is not included.
No merge occurred in this checkpoint.

## Selected setup boundary

`toolchain::prepare_media_setup` and `toolchain prepare-media` map one explicit
capability and dependency selection to the existing allenk image-repair or
Upscayl recovery configuration. Closed options come only from the selected
capability's `effective_settings`. The shared offline inspector owns bounded
local hashing and attributed inventory facts; typed mapping requires the exact
dependency kinds, selected flags/features, version/package evidence, effects,
backend and native/requested-scale declarations.

A ready result contains one inert `configuration.json` and canonical digest.
Unready dependencies retain diagnostics and no files. Preparation reads no
source media, launches no process, writes no setup, registers no provider and
installs/downloads nothing. The existing media facades retain source-bound
registration, exact locks, component freshness, processing admission and resume.
Retained setup evidence cannot replace those execution owners.

Native qualification remains false and complete host/runtime identity remains
unobserved. Allenk uses explicit current/legacy selection without denoising or
fallback; Upscayl retains separate native4 and requested2/3/4 scales and separate
model-file pins. Its shared model-name guard now includes source-derived 8x/16x
inference while preserving upstream precedence. Local hashes and caller claims
do not establish model origin/terms, hardware suitability or visual quality.

## Continuation and qualification

#38 remains open for other typed mappings, complete runtime/environment identity,
optional tool/device probes, model-locator discovery and native/platform
qualification. Next proposed source work is #35's verified frame-to-video/audio
assembly and a single workflow recipe after reconciling open setup/repair/chunk
drafts. #36 retains manifest migration, other provider/clip limits/seams, optional
detection and native qualification; #37 retains historical-output adoption and
the full failure/native matrix. #111 can proceed independently.

Tests/compiler/build, lint/format, naming/schema/drift, smoke/package,
native/model/provider/CLI, dependency resolution/installation and hosted checks
remain deferred under #64. No Actions are dispatched or polled. Authored
synthetic regressions remain unrun; source review is in progress. The known
alignment cancellation/start-marker failure remains unresolved with cause
unproven. Source inclusion is not native safety, creative approval, release or
flow consumer proof. Source package 0.3.0 remains unreleased.

See the [setup guide](docs/toolchain-media-setup.md) and
[handoff](docs/validation/aniflow-38-media-setup-checkpoint.md). Keep the
historical block below verbatim; it grants no additional authority.

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
