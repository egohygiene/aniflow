---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-27T10:15:18Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Continue bounded selective direct-image repair for issue #36 after merged documentation and immutable-import checkpoints.
  includes:
  - Two ordered stages using existing import, provider, workspace and acceptance owners
  - Exact selected ranges, complete output mapping, unselected byte preservation and honest evidence
  excludes:
  - conversation transcripts
  - private source media or local paths
  - clip-only or named-provider qualification, external manifest migration and release publication
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
  - docs/validation/aniflow-36-repair-checkpoint.md
  - docs/validation/aniflow-36-import-checkpoint.md
  - docs/temporal-correctness.md
  - docs/video-interpolation.md
  - ROADMAP.md
work:
  objective: Apply explicit selected-frame repair without changing unselected PNGs or inventing historical processing evidence.
  success_conditions:
  - Library and CLI compose import then direct-image repair under the existing runtime.
  - Complete output names, ordinals, geometry, clocks and cuts are retained; unselected bytes match.
  - Current processor observations and byte changes never imply visual repair quality or native qualification.
  active_issue: "36"
  next:
    kind: action
    id: "36"
    description: Re-query the repair draft and remaining issue scope before selecting named providers, clip handling or migration; #37 follows as a separate recovery lane.
    readiness: ready
    references:
    - https://github.com/egohygiene/aniflow/issues/36
    - https://github.com/egohygiene/aniflow/issues/37
    - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: a717935b49fccf87151124a6e20efeb4281ff11c
    ref: refs/heads/main
    verified_at: "2026-10-27T10:03:42Z"
  candidate:
    branch: codex/36-selective-frame-repair
    revision: null
    pull_request: https://github.com/egohygiene/aniflow/pull/125
    handoff_state: ready-for-review
  live:
    status: partial
    observed_at: "2026-10-27T10:15:18Z"
    default_branch_revision: a717935b49fccf87151124a6e20efeb4281ff11c
    issue_state: open
    pull_request_state: draft
    notes: PR #123 merged at 97076e5cb90a88dcbcc6cc7c32598850bb70da6b and focused issue #118 is closed. PR #124 merged at the base revision. Draft PR #125 first saved repair at 5a17004dc43aff86c162263980b2f02a74256f5b; re-query the current head. #36 and qualification #64 remain open. No executable qualification is established.
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-27T10:15:18Z"
  reviewed_by: codex
  evidence:
  - command: Read-only source, live issue and Git revision inspection
    outcome: limited
    observed_at: "2026-10-27T10:15:18Z"
    notes: Root and independent source review found no concrete blocker in provider, import, temporal, resume and workspace boundaries. Thirteen synthetic regression sources are authored but unrun. Published feature blobs/tree match the local snapshot.
  - command: Tests, compiler/build, lint/format, naming/schema/drift, smoke/package, native/model/provider/CLI and hosted qualification
    outcome: not-run
    observed_at: "2026-10-27T10:03:42Z"
    notes: Existing AGENTS.md and issue #64 maintainer deferral remains active; no continuity or documentation validator ran.
  environment_limitations:
  - Current-source and native platform qualification remains unrun.
  - Known alignment cancellation/start-marker failure remains unresolved, cause unproven.
  - No pinned video-frames/1 schema is present; current RIFE PNG checks do not prove decompression.
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
first. Live source and work-tracker state outrank this snapshot. Base main
`a717935b49fccf87151124a6e20efeb4281ff11c` includes merged documentation
PR #123 and immutable frame-import PR #124. Focused documentation issue #118
is closed; parent #36 and qualification #64 remain open.

This checkpoint adds `frame_repair`: one ordinary Pipeline v3 run with immutable
import followed by explicitly selected direct-image processing. Only selected
ranges reach the processor; every unselected frame is copied byte-identically.
The complete frame index retains names, ordinals, geometry, clocks and cuts.
Reports distinguish requested processing from observed byte changes and leave
visual repair quality unestablished. Generic caller-selected tools do not become
qualified named providers or establish implicit model/environment identities.
This initial capability requires fresh runs and refuses generic v3 resume before
durable transitions; read-only status remains available. Other capabilities'
cache/resume semantics remain unchanged.

Import and repair use the existing provider ABI, ordered run, immutable workspace
and acceptance owners. A standalone import report is not prior accepted-stage
authority. Native children stay in the outer process group; no second scheduler,
arbitrary DAG, historical lock or silent frame padding/truncation is added.
See [the guide](docs/frame-repair.md) and
[checkpoint handoff](docs/validation/aniflow-36-repair-checkpoint.md).

## Continue from live state

Re-query this branch and its draft PR before continuing. #36 still owns explicit
named provider integration, clip-only segment limits and checked seams, optional
review-first detection, pinned external `video-frames/1` migration and executable
qualification. Do not invent that absent external schema. #37 owns bounded
Upscayl recovery; #35 owns the complete music-video recipe. Broader RIFE assembly
and setup remain #110/#38. The complete retained ledger is #117.

[Draft PR #125](https://github.com/egohygiene/aniflow/pull/125) first saved the
feature at `5a17004dc43aff86c162263980b2f02a74256f5b`. The resume regression and
this source handoff follow that initial save; do not treat the initial pin as
the latest head. Candidate revision is deliberately unset to avoid a circular
claim about the commit containing this document.

## Qualification and preservation

Source review only. Tests/compiler/build, lint/format, naming/schema/drift,
smoke/package, native/model/provider/CLI, dependency resolution/installation and
hosted checks remain deferred under #64. No Actions were dispatched or polled.
The known alignment cancellation/start-marker failure remains unresolved with
cause unproven. Source merge is not native safety, creative approval, release
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
