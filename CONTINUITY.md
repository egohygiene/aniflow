---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T10:54:38Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Deliver bounded per-frame Upscayl recovery for issue #37 using existing Pipeline v3 checkpoints.
  includes:
  - Exact source, executable, model-pair and settings bindings with observed decoded geometry
  - Independent frame checkpoints, complete-set collection, explicit resume and frame progress
  excludes:
  - conversation transcripts
  - private source media or local paths
  - native qualification, historical output adoption, automatic quality decisions and release publication
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
  - docs/validation/aniflow-37-checkpoint.md
  - docs/frame-repair.md
  - docs/toolchain-upstreams.md
  - docs/temporal-correctness.md
  - ROADMAP.md
work:
  objective: Reuse compatible validated Upscayl frames after interruption and retry pending work without changing sources or inventing history.
  success_conditions:
  - Existing runtime owns all accepted checkpoints, cancellation, immutable publication and writer exclusion.
  - Exact binary, model pair, scales and execution settings bind each frame; source timing stays unchanged.
  - Complete collection requires every observed frame; source-derived interface support remains separate from native qualification.
  active_issue: "37"
  next:
    kind: action
    id: "37"
    description: Re-query the stacked draft and remaining chunk, historical-import and qualification scope before selecting the next checkpoint.
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/pull/126
    - https://github.com/egohygiene/aniflow/pull/125
    - https://github.com/egohygiene/aniflow/issues/37
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on:
    - https://github.com/egohygiene/aniflow/pull/125
state:
  base:
    revision: 00cb3d0b76018990ef3d28eb5546d6d746eacefd
    ref: refs/heads/codex/36-selective-frame-repair
    verified_at: "2026-10-27T10:54:38Z"
  candidate:
    branch: codex/37-upscayl-frame-recovery
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/126
    handoff_state: ready-for-review
  live:
    status: partial
    observed_at: "2026-10-27T10:54:38Z"
    default_branch_revision: a717935b49fccf87151124a6e20efeb4281ff11c
    issue_state: open
    pull_request_state: draft
    notes: PR #126 is stacked on still-open draft #125 to reuse its decoder. Initial runtime prerequisite saved at 8d84e971d3f7745682bbf78d678431d8fb95b0d8; the feature followed at 35b729a9b98d654f4d637c24c77463ffc023b9c6. Re-query the final head with regression/handoff updates. #36, #37 and #64 remain open. No merge or native qualification is implied.
  parallel_changes:
  - Draft PR #125 contains selective direct-image repair; its generic repair capability remains fresh-run only.
review:
  status: partial
  reviewed_at: "2026-10-27T10:54:38Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, pinned upstream interface and live Git revision inspection
    outcome: limited
    observed_at: "2026-10-27T10:54:38Z"
    notes: Root and independent source review found no remaining concrete blocker in the bounded feature. Exact published feature blobs/tree match the local source snapshot. Fourteen recovery and four integrity regression sources are authored but unrun; no compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/model/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T10:54:38Z"
    notes: Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator.
  environment_limitations:
  - Current-source and native platform qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Pinned Upscayl source requires a functional GPU; planning defers availability admission and provides no CPU fallback or driver qualification.
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
`a717935b49fccf87151124a6e20efeb4281ff11c`; documentation PR #123 and immutable
import PR #124 are merged. Focused documentation issue #118 is closed.

[Draft PR #126](https://github.com/egohygiene/aniflow/pull/126) is stacked on
[repair draft #125](https://github.com/egohygiene/aniflow/pull/125) at
`00cb3d0b76018990ef3d28eb5546d6d746eacefd`. It reuses the shared bounded decoder;
neither draft is merged by this checkpoint. Initial runtime prerequisite
`8d84e971d3f7745682bbf78d678431d8fb95b0d8` is a historical save, not the final
feature head. The feature followed at `35b729a9b98d654f4d637c24c77463ffc023b9c6`,
before the regression and current handoff save. Candidate revision stays unset
to avoid a circular self-pin.

The `upscayl_recovery` facade uses a finite set of independent ordinary v3
frame stages plus a complete-set collector. Exact source/index/time, executable,
model-pair and settings bindings govern reuse. Each selected native process
handles one frame; an explicit resume attempts pending work once and preserves
compatible validated frames. Native model scale and requested output scale are
separate. The output's spatial index preserves original source inspection and
clock while recording new geometry separately; it is not automatically RIFE
input-compatible.

All durable acceptance stays with existing stage checkpoints and the run owner.
No provider-local checkpoint store, automatic historical PNG adoption, global
cache or new scheduler is added. Native children inherit the outer group.
Current execution observations do not prove historical processing, perceptual
quality, native Apple Silicon support or GPU-memory enforcement. See
[the guide](docs/upscayl-recovery.md) and
[handoff](docs/validation/aniflow-37-checkpoint.md).

## Integrity and continuation

Built-in integrity publication rechecks declared stage inputs and accepted
outputs. External validators still trigger broad artifact checks. Before-use
checks, checkpoint compatibility and final whole-run verification remain intact.
Unrelated corruption may be detected at its next use or final delivery instead
of after every independent frame; no completed delivery can rely on it.

Fresh binary/model/decoder preflight also guards generic v3 run and resume for
these capabilities; a stale declared registry inventory cannot justify reuse.
The collector requires the exact expected same-run frame producers; loose source
reports and PNGs cannot be substituted. Resume checks room for new metadata and
each attempted operation rather than reserving all fresh-run outputs again.
This does not relax PR #125's fresh-only rule for generic direct-image repair,
whose model/environment identities remain unobserved.

#37 remains open for bounded chunks, explicit historical output adoption, the
full injected failure matrix and native/NCNN qualification. #36 retains named
repair-provider/clip/migration work. The eventual #35 music-video recipe composes
the useful paths; #110/#38 retain finishing and setup scope. Re-query the full
retained ledger #117 before choosing another issue.

## Qualification and preservation

Source review only. Tests/compiler/build, lint/format, naming/schema/drift,
smoke/package, native/model/provider/CLI, dependency resolution/installation and
hosted checks remain deferred under #64. No Actions were dispatched or polled.
The known alignment cancellation/start-marker failure remains unresolved with
cause unproven. A source merge is not native safety, creative approval, release
or flow consumer proof. First core publication remains #10.

Keep the historical block below verbatim. Prior handoffs and receipts preserve
their dated observations; this snapshot grants no additional authority.

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
