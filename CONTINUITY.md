---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T13:13:43Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Add a typed allenk image-repair profile and whole-stage resume for issue #36."
  includes:
  - Explicit visible-overlay profile and threshold with pinned executable and decoder
  - Exact same-run importer provenance and compatible whole-stage reuse
  excludes:
  - conversation transcripts
  - private source media or local paths
  - native qualification, clip processing, external manifest migration and release publication
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
  - docs/frame-repair.md
  - docs/validation/aniflow-36-allenk-checkpoint.md
  - docs/toolchain-upstreams.md
  - docs/temporal-correctness.md
  - ROADMAP.md
work:
  objective: Reuse accepted named repair work without weakening generic repair's fresh-run boundary.
  success_conditions:
  - Existing v3 checkpoints, process groups, writer ownership and immutable publication remain authoritative.
  - Named typed settings and current executable/decoder identities govern reuse; exact importer provenance is required.
  - Unselected frame bytes and complete original source mappings remain unchanged.
  active_issue: "36"
  next:
    kind: action
    id: "36"
    description: "Finish source review and save the named repair draft before selecting #38 setup."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/36
    - https://github.com/egohygiene/aniflow/issues/38
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 2add9a6b230aaf415288e6c128bcf276dc05681c
    ref: refs/heads/main
    verified_at: "2026-10-27T13:13:43Z"
  candidate:
    branch: codex/36-named-frame-repair
    revision: null
    pull_request: null
    handoff_state: in-progress
  live:
    status: partial
    observed_at: "2026-10-27T13:13:43Z"
    default_branch_revision: 2add9a6b230aaf415288e6c128bcf276dc05681c
    issue_state: open
    pull_request_state: not-created
    notes: "PR #125 and #126 are merged. This named repair checkpoint starts independently from main; PR #127 remains a separate open chunk draft. #36 and #64 remain open."
  parallel_changes:
  - "PR #127 bounded Upscayl chunks at 246759871a6b705fbc34615185f9066397d3a8d0 is not included in this branch's base."
review:
  status: partial
  reviewed_at: "2026-10-27T13:13:43Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source and pinned upstream interface inspection
    outcome: limited
    observed_at: "2026-10-27T13:13:43Z"
    notes: Named source and synthetic regressions are being authored and reviewed; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T13:13:43Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Native/tool/model and current-source execution qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Binary identity binds compiled profile assets, not a complete shared-library or host environment.
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
first. Live source and work-tracker state outrank this snapshot. Main is
`2add9a6b230aaf415288e6c128bcf276dc05681c`: generic repair PR #125 merged at
`7b3aa0fc671d853518e382d74f717945940be337`, then per-frame Upscayl recovery PR #126
merged at the current base. Documentation issue #118 remains closed.

This independent branch, `codex/36-named-frame-repair`, adds a typed allenk image
profile and whole-stage resume. Candidate revision stays unset to avoid a
circular self-pin. Re-query the branch and draft before continuing.
[Chunk draft PR #127](https://github.com/egohygiene/aniflow/pull/127) remains
separate at `246759871a6b705fbc34615185f9066397d3a8d0`; its code is not included
in this branch's main base.

The `frame_repair::allenk` facade uses a distinct closed configuration and
capability over the same immutable import and repair stages. An explicit
current/V2 or legacy/V1 visible-overlay profile and threshold generate fixed
single-image argv. Denoising, force, region, snap and automatic cross-profile
fallback are omitted. The installed binary version is observed at execution;
compiled profile assets are bound by the executable digest. Reports retain
unobserved host/shared-library identity and unqualified native/visual behavior.
No binary hash is described as complete environment identity.

Every unselected frame remains byte-identical. Selected PNGs must retain exact
mapping/geometry and fully decode. Original source clocks and cuts stay intact.
The named report wraps complete repair observations with explicit dependency
facts; historical processing remains unknown and source relationship declared.

## Integrity and continuation

Named resume rebuilds and compares the exact plan. Generic v3 run/resume freshly
recheck processor and decoder files and require the exact same-run importer,
matching import configuration and source/artifact bindings. Loose reports cannot
stand in for accepted predecessor work. Ordinary checkpoint compatibility,
exclusive writer ownership, final integrity and failure retention stay in charge.

An accepted import or complete named repair stage can reuse. Interrupted repair
retries all selected frames once; partial per-frame outputs never become
checkpoints. Generic arbitrary-argv repair remains fresh-run only, including
forced generic resume. No new scheduler, cache or process implementation is
introduced. Whole-stage reuse does not imply cross-host deterministic output.

#36 remains open for pinned external manifest migration, GWR integration, clip
limits/splitting/seams, optional detection review and native qualification. No
actual pinned `video-frames/1` schema/example has been supplied, so none is
invented. The next useful sequence is #38 selected repair/Upscayl setup, then
#35 verified assembly/audio and a single workflow recipe. Reconcile parallel
chunk draft #127 before composing the full media path. #37 retains historical
output adoption and native/failure qualification; #111 can proceed independently.

## Qualification and preservation

Tests/compiler/build, lint/format, naming/schema/drift, smoke/package,
native/model/provider/CLI, dependency resolution/installation and hosted checks
remain deferred under #64. No Actions are dispatched or polled. Authored
synthetic regression sources remain unrun. The known alignment cancellation/
start-marker failure remains unresolved with cause unproven. A source merge is
not native safety, creative approval, release or flow consumer proof.

See the [guide](docs/frame-repair.md) and
[named handoff](docs/validation/aniflow-36-allenk-checkpoint.md). Keep the
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
