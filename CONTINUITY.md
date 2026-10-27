---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T11:35:52Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: "Extend issue #37 recovery to bounded native chunks using existing Pipeline v3 checkpoints."
  includes:
  - Explicit per-frame and chunk policies with exact source, executable, model and settings bindings
  - Complete-chunk reuse, complete-set collection and frame-weighted progress
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
  - docs/validation/aniflow-37-chunks-checkpoint.md
  - docs/frame-repair.md
  - docs/toolchain-upstreams.md
  - docs/temporal-correctness.md
  - ROADMAP.md
work:
  objective: Process longer declared video workspaces without thousands of per-frame stages or a second checkpoint owner.
  success_conditions:
  - Existing runtime owns accepted chunks, cancellation, immutable publication and writer exclusion.
  - Source/index/time, binary/model pins, chunk boundaries and explicit timeout govern compatibility.
  - Interrupted chunks retry in full; loose partial outputs never gain accepted-checkpoint authority.
  active_issue: "37"
  next:
    kind: action
    id: "37"
    description: "Review draft PR #127, then select the bounded #36 named-provider checkpoint."
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/pull/127
    - https://github.com/egohygiene/aniflow/issues/37
    - https://github.com/egohygiene/aniflow/issues/36
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 2add9a6b230aaf415288e6c128bcf276dc05681c
    ref: refs/heads/main
    verified_at: "2026-10-27T11:31:31Z"
  candidate:
    branch: codex/37-upscayl-chunks
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/127
    handoff_state: ready-for-review
  live:
    status: partial
    observed_at: "2026-10-27T11:31:31Z"
    default_branch_revision: 2add9a6b230aaf415288e6c128bcf276dc05681c
    issue_state: open
    pull_request_state: draft
    notes: "PR #125 merged at 7b3aa0fc671d853518e382d74f717945940be337, then PR #126 merged at the current base. #36, #37 and #64 remain open. PR #127 initially saved at 987f97f6fcb226f3d2f2163889a58159a046d7a8; re-query its final head, including explicit native capture bounds. Source work does not establish native qualification."
  parallel_changes:
  - Generic direct-image repair remains fresh-run only because its external model/environment identities are unobserved.
review:
  status: partial
  reviewed_at: "2026-10-27T11:36:55Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, pinned upstream interface and live Git revision inspection
    outcome: limited
    observed_at: "2026-10-27T11:31:31Z"
    notes: The initial published tree matches the reviewed local snapshot. Root and independent source review, including explicit native capture bounds, found no remaining concrete blocker. Ten chunk regression cases and two contract cases are authored but unrun. No compilation or native qualification is established.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/model/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T11:31:31Z"
    notes: "Maintainer deferral under #64 remains active; no Actions dispatch/polling, dependency resolution or documentation validator."
  environment_limitations:
  - Current-source and native platform qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - Pinned Upscayl source requires a functional GPU; planning does not qualify driver/device availability.
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
first. Live source and work-tracker state outrank this snapshot. PR #125 merged
at `7b3aa0fc671d853518e382d74f717945940be337`; PR #126 then merged at current main
`2add9a6b230aaf415288e6c128bcf276dc05681c`. Selective direct-image repair and
bounded per-frame Upscayl recovery are included. #118 remains closed for its
focused documentation scope. Candidate revision stays unset to avoid a circular
self-pin; re-query [draft PR #127](https://github.com/egohygiene/aniflow/pull/127)
before continuing. Its initial saved head is
`987f97f6fcb226f3d2f2163889a58159a046d7a8`, with exact published tree
`905b4c94d0e74ef5e5d1790d91757668241965e0` matching the local source snapshot.
The subsequent native-capture follow-up is included in source review; the
initial pin does not identify the final head.

The `codex/37-upscayl-chunks` checkpoint adds explicit native chunks to
`upscayl_recovery` and the existing `video upscale-frames` command. Default
per-frame behavior remains bounded at 1,024 frames. Chunk mode uses 2–256 frames
per native process, at most 1,024 chunks and 100,000 frames, subject to existing
index, plan and storage bounds. A 5,602-frame video at 64 frames per chunk uses
88 processing stages and one collector. Source timing stays unchanged in the
separate spatial output index; it is not automatically RIFE-input compatible.

One ordinary v3 stage accepts each complete chunk directory/report. Every input
and output fully decodes, and exact source/index/time, tool/model pins, settings,
chunk boundaries, timeout and native capture bound govern reuse. Completed chunks reuse as whole units;
interrupted chunks retry in full once per explicit resume. Private partials are
retained evidence, never implicit accepted history. The collector requires exact
same-run producers. No new scheduler, checkpoint database or global cache exists.

Separate native timeout and stdout/stderr capture bounds share the existing
process owner and outer group; FFmpeg decoder limits remain unchanged. Progress counts completed/reused chunk
cardinalities with ETA measured only from newly accepted work. Metadata budget
conservatively accounts for repeated stage records and frame-level plan/report
data. Retained PNG copies still require substantial disk; chunking does not imply in-place replacement.
See the [guide](docs/upscayl-recovery.md) and
[chunk handoff](docs/validation/aniflow-37-chunks-checkpoint.md).

## Integrity and continuation

Generic run/resume freshly rechecks component files and producer topology.
Built-in integrity postchecks, before-use checks, checkpoint compatibility and
final whole-run integrity retain their existing owners. External validators
still trigger broad artifact checks. These rules do not relax generic repair's
fresh-only policy or infer historical processing, visual quality, GPU-memory
enforcement or actual native Apple Silicon/NCNN support.

#37 remains open for explicit historical-output adoption, the complete injected
failure matrix and native qualification. The proposed next useful sequence is
#36 one explicit repair provider with dependency-bound resume, #38 selected
repair/Upscayl setup, then #35 verified assembly and a single workflow recipe.
#111 target-resolution policy can proceed independently. Re-query the full
retained ledger #117 before choosing another issue.

## Qualification and preservation

Tests/compiler/build, lint/format, naming/schema/drift, smoke/package,
native/model/provider/CLI, dependency resolution/installation and hosted checks
remain deferred under #64. No Actions were dispatched or polled. Authored
synthetic regression sources remain unrun. The known alignment cancellation/
start-marker failure remains unresolved with cause unproven. A source merge is
not native safety, creative approval, release or flow consumer proof. First
core publication remains #10.

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
