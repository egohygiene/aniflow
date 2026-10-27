---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T22:48:55Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Author opt-in target-resolution policy #111 on top of open recipe draft PR #131."
  includes:
  - Closed target geometry, exact/minimum intent and separate force policy
  - Actual stage input identity, unchanged bypass and existing Upscayl recovery
  - Explicit preview scope, remaining acceptance and deferred qualification
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
  - docs/target-resolution.md
  - docs/validation/aniflow-111-target-resolution-checkpoint.md
  - docs/music-video-recipe.md
  - docs/validation/aniflow-35-recipe-checkpoint.md
  - docs/upscayl-recovery.md
  - ROADMAP.md
work:
  objective: Complete the selected target-resolution checkpoint through existing recipe and v3 owners.
  success_conditions:
  - Actual stage geometry and policy identity govern execute/bypass decisions.
  - Bypass keeps accepted input identity and needs no neural tool/model files.
  - Executing work retains exact Upscayl dependencies, scales and recovery.
  - Unsupported transformations and unrun qualification remain explicit.
  active_issue: "111"
  next:
    kind: action
    id: "111"
    description: "Publish the target-resolution checkpoint as a draft stacked on open recipe PR #131."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/111
    - https://github.com/egohygiene/aniflow/pull/131
    - https://github.com/egohygiene/aniflow/pull/130
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 31352771bcb6e6d043bf1f19bd57b87be3853e20
    ref: refs/heads/codex/35-music-video-recipe
    verified_at: "2026-10-27T22:48:55Z"
  candidate:
    branch: codex/111-target-resolution
    revision: null
    pull_request: null
    handoff_state: implementation-in-progress
  live:
    status: partial
    observed_at: "2026-10-27T22:48:55Z"
    default_branch_revision: 8cb00a323b1829de1e4b958236b6aa8529eb7045
    issue_state: open
    pull_request_state: publication-pending
    notes: "Issue #111 read live. PR #130/#131 remain open drafts; target policy stacks on recipe source. Main and prior merged PR #128/#129/#127 remain unchanged."
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-27T22:48:55Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, issue and boundary review
    outcome: limited
    observed_at: "2026-10-27T22:48:55Z"
    notes: Target policy is being authored against existing recipe and Upscayl owners. Prior review receipts remain in dated handoffs; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T22:48:55Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Native/tool/model and current-source execution qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Target geometry is not proof of quality, native detail or device suitability.
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
first. Main remains `8cb00a323b1829de1e4b958236b6aa8529eb7045`, including
merged named repair PR #128, selected setup PR #129 and bounded chunks PR #127.
Assembly draft PR #130 remains open at
`1893bfb994d79e75b08d32739ed61aa23e58cde9`; stacked recipe draft PR #131
remains open at `31352771bcb6e6d043bf1f19bd57b87be3853e20`.

The authorized `codex/111-target-resolution` checkpoint is based on recipe
source and will target that branch. Publication metadata is pending; candidate
revision remains null rather than a circular self-pin. None of this stack has
merged. Prior first-save pins, tree receipts, source-review findings and unrun
coverage remain in the assembly/recipe handoffs. Live state outranks this snapshot.

## Target-resolution boundary

The opt-in closed policy compares actual stage input against explicit target
dimensions and exact/minimum intent, with a separate force choice. Both axes,
aspect and supported orientation matter. Unsupported crop/stretch/letterbox,
non-square pixels, rotation and model/output scale combinations are refused.
Model-native scale, requested output scale and final geometry stay distinct.

Bypass passes accepted frames unchanged and records geometry, reason and policy
identity without loading neural tools/models or inventing model execution or
quality evidence. It does not bypass required source/import/assembly validation.
Executing paths retain existing Upscayl tool/model/device/tile/worker bindings
and per-frame/chunk recovery. Actual stage identity is rechecked for handoff and
resume; a retained decision cannot authorize changed inputs or configuration.

The recipe still uses immutable intent and fixed child paths. Existing child
v3 plans, locks, acceptance and checkpoints remain authoritative; no conditional
workflow engine or new provider/checkpoint framework is added. Initial exact
downstream plans remain pending until accepted predecessor outputs exist.
Geometry declarations do not establish extraction lineage, perceptual quality,
creative approval or native qualification. ADR-0012 remains proposed.

## Continuation and qualification

Finish this draft checkpoint; #111 remains open for qualification and residual
integration. The separate preview is bounded to 256 frames/ten source seconds,
uses the same target/settings, and cannot replace full-output acceptance.
#35 retains initial extraction, broader media profiles, loudness/metadata and
qualification; #107 retains broader manual/review recipes. #110 RIFE clocks,
#83 destination copies and #82 proofs remain separate. See the
[policy guide](docs/target-resolution.md) and
[handoff](docs/validation/aniflow-111-target-resolution-checkpoint.md).

All tests/compiler/build, lint/format, schema/naming/drift/docs validators,
smoke/package, native/model/provider/CLI and hosted checks remain deferred under
#64; no Actions are dispatched or polled. Authored regressions remain unrun.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Source package 0.3.0 remains unreleased. Keep the historical block below
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
