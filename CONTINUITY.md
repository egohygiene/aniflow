---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T04:05:34Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Continue the first bounded frame-selection preview for issue36 with frequent draft checkpoints and execution qualification deferred.
  includes:
  - Exact read-only range preview over existing complete frame-index declarations
  - Public library, CLI, schema, source-authored regressions and remaining import boundaries
  excludes:
  - conversation transcripts
  - private source media or local paths
  - accepted imports, frame repair and provider execution
  precedence:
  - user-and-runtime-instructions
  - scoped-repository-instructions
  - live-repository-and-work-tracker-state
  - canonical-repository-sources
  - continuity-checkpoint
  canonical_sources:
  - AGENTS.md
  - .github/copilot-instructions.md
  - docs/frame-selection.md
  - docs/validation/aniflow-36-checkpoint.md
  - docs/temporal-correctness.md
  - docs/video-interpolation.md
  - docs/layered-validation.md
  - ROADMAP.md
work:
  objective: Deliver a small read-only frame-selection preview without inventing historical processing or accepting unobserved images.
  success_conditions:
  - CLI and library share one exact range planner and retain unknown historical processing.
  - Complete declared mapping and rational clocks bind the plan; selection counts are not byte-preservation evidence.
  - Draft publication and source-only review remain distinguishable from execution qualification.
  active_issue: "36"
  next:
    kind: action
    id: "36"
    description: Finish source review and save the preview draft; then scope real immutable import and decode validation without inventing video-frames/1 compatibility.
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/36
    - https://github.com/egohygiene/aniflow/issues/117
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 7a808ff99ca8f557f6a3ba7fd373a393de0feee6
    ref: refs/heads/main
    verified_at: "2026-10-27T04:05:34Z"
  candidate:
    branch: codex/36-frame-selection-preview
    revision: null
    pull_request: null
    handoff_state: in-progress
  live:
    status: partial
    observed_at: "2026-10-27T04:05:34Z"
    default_branch_revision: 7a808ff99ca8f557f6a3ba7fd373a393de0feee6
    issue_state: open
    pull_request_state: not-created
    notes: Fresh checkout matches main after audit PR121; current source includes the merged PR105/PR115 checkpoints. New frame-preview work is authored locally, not yet published or qualified. Re-query before publication.
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-27T04:05:34Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, live issue and Git revision inspection
    outcome: limited
    observed_at: "2026-10-27T04:05:34Z"
    notes: Reviewed existing temporal, RIFE-index, acceptance and CLI boundaries; authored ten focused unrun regressions.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/model/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T04:05:34Z"
    notes: Existing AGENTS.md and issue64 maintainer deferral remains active; no continuity validator was run.
  environment_limitations:
  - Current-source and native platform qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - No pinned video-frames/1 schema is present; current PNG checkers do not prove decompression.
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

## Purpose and precedence

This is the current issue36 source checkpoint. Instructions, live evidence and
canonical sources outrank the snapshot; it grants no additional authority.
Read AGENTS.md, the checkpoint handoff and the live issue before continuing.

## Current objective and state

Branch `codex/36-frame-selection-preview` starts from main
`7a808ff99ca8f557f6a3ba7fd373a393de0feee6`. The first source slice adds the
pure frame-selection library and `video frame-selection-plan` CLI, with an
additive schema and ten unrun synthetic regressions. Parent36 remains open.
No draft PR ID or execution result is invented before publication.

The command reads the existing RIFE frame-index document without requiring
RIFE tools/configuration. Inclusive one-based ranges become sorted half-open
ordinals and rational source spans. Complete mappings and counts are checked;
source/PNG bytes are not opened. Unknown historical processing stays explicit.
No import, accepted stage, native decode, repair or untouched-byte proof occurs.

## Continuation and remaining work

Finish source review, publish the narrow draft checkpoint and bind its actual
PR/head here. The detailed [handoff](docs/validation/aniflow-36-checkpoint.md)
owns the remaining real import/decode, explicit video-frames/1 migration,
selective provider repair and qualification sequence. No new frame runtime,
false provider checkpoint or image-header-only acceptance should be introduced.

The complete retained roadmap remains [#117](https://github.com/egohygiene/aniflow/issues/117).
Documentation repair118 and naming119 retain their independent scope. The
October21 audit is preserved in `audits/flow-suite-20261021T025401Z.md` and its
receipt. It is planning evidence, not current product qualification.

## Verification and release boundaries

Source reads and review only. Tests, compiler/build, lint/format, naming/schema,
native/model/provider/CLI, package and hosted checks remain deferred under #64.
The known alignment cancellation/start-marker failure remains unresolved with
unproven cause. No native safety, passing conformance, release or Flow consumer
proof is claimed. Provider setup38, broader RIFE assembly110 and release10 keep
their full remaining scope. A merged source checkpoint does not qualify them.

## Preservation and future updates

Keep within the declared continuity bounds. Update current state after actual
publication; Git, issues and historical receipts own prior observations. Do not
rewrite #32/#33/#104/#114 historical evidence or private consumer content.
The prior merge handoff below remains historical and is preserved verbatim.

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
