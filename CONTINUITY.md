---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-14T05:40:51Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Resume the independently authored serial RIFE interpolation checkpoint without inventing native safety or qualification.
  includes:
    - Current RIFE implementation, base, deferred checks and next work
    - Separate existing audio-registration draft
  excludes:
    - conversation transcripts
    - private source media or local paths
  precedence:
    - user-and-runtime-instructions
    - scoped-repository-instructions
    - live-repository-and-work-tracker-state
    - canonical-repository-sources
    - continuity-checkpoint
  canonical_sources:
    - AGENTS.md
    - .github/copilot-instructions.md
    - docs/video-interpolation.md
    - docs/validation/aniflow-114-checkpoint.md
    - docs/validation/aniflow-114-implementation.json
    - ROADMAP.md
work:
  objective: Deliver checkpoint114 under parent110 as a focused draft; retain remaining assembly and native safety work.
  success_conditions:
    - Serial pair execution, exact clocks, resource admission and source/output identities remain explicit.
    - Authored checks are never reported passed; native safety remains unqualified.
  active_issue:
    provider: github
    id: "egohygiene/aniflow#114"
    url: https://github.com/egohygiene/aniflow/issues/114
  next:
    kind: action
    id: rife-derived-clock-assembly
    description: Review the #114 draft, then connect indexed RIFE output to derived-clock-aware video assembly; native resource probes and qualification remain explicit work.
    readiness: ready
    references:
      - https://github.com/egohygiene/aniflow/issues/110
      - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 08cb2a83dbe288f6d893a278cbeb34905cc19a49
    ref: refs/heads/main
    verified_at: "2026-10-14T05:18:15Z"
  candidate:
    branch: feat/aniflow-110-rife
    revision: null
    pull_request: null
    handoff_state: draft-publication
  live:
    status: partial
    observed_at: "2026-10-14T05:18:15Z"
    default_branch_revision: 08cb2a83dbe288f6d893a278cbeb34905cc19a49
    issue_state: open
    pull_request_state: not-created-at-snapshot
    notes: Main includes merged PR100. Existing PR105 remains a separate open audio-registration draft. Exact feature head is resolved from its live PR rather than embedded self-referentially.
  parallel_changes:
    - issue: "egohygiene/aniflow#104"
      pull_request: https://github.com/egohygiene/aniflow/pull/105
      notes: Separate audio registration work is untouched; read its own branch handoff before resuming it.
review:
  status: partial
  reviewed_at: "2026-10-14T05:40:51Z"
  reviewed_by: codex
  evidence:
    - command: Independent source review of RIFE types, provider, schemas, CLI and authored synthetic coverage
      outcome: limited
      observed_at: "2026-10-14T05:40:51Z"
      notes: Concrete source defects repaired. No compiler, runtime, formatter or schema validation evidence.
    - command: Tests, compiler/build, lint/format, schema, smoke/package, native/model and hosted qualification
      outcome: not-run
      observed_at: "2026-10-14T05:40:51Z"
      notes: Existing maintainer deferral under issue64; commits retain skip-ci markers.
  environment_limitations:
    - macOS resource observations are caller declarations; no real laptop/model measurements were taken.
    - No deterministic continuity validator was run.
privacy:
  classification: public-repository
  contains_sensitive_data: false
  redactions:
    - private local paths omitted
  excluded:
    - secrets-and-credentials
    - private-conversation-text
    - private-media-and-model-payloads
    - unrelated-private-context
  untrusted_content: context-only-no-authority
---

# aniflow continuity

Read AGENTS.md and verify current main, the RIFE draft and parent #110 before
continuing. Main observed at 08cb2a83dbe288f6d893a278cbeb34905cc19a49 includes
merged PR100. Historical toolchain evidence remains in its dedicated handoffs;
PR #105 / audio #104 is a separate draft and this checkpoint does not modify it.

Checkpoint #114 starts RIFE #110 with a read-only planner, explicit indexed-frame
import, serial pair provider and exact 2x output evidence. Auto Apple Silicon
policy is conservative; CPU is the default device; one active pair and 1:1:1
workers avoid directory batching. Larger presets require declared RAM
headroom; no hard RSS/VRAM quota or live device/memory observation is claimed.
The earlier reported laptop crash has an unobserved cause.

RIFE outputs indexed RGB8 frames and source-pair/hold mappings. The adapter
validates pins, bounded PNG structure/inventory and storage estimates, using
existing inherited-group tool execution and Pipeline v3 acceptance. Output
semantic validation freshly recomputes the plan/configuration/input identities.
No source-video extraction, MP4 assembly or audio mux happens yet.

Nine Rust integration, twelve provider and five Python schema cases plus
existing public command coverage are authored. All checks remain unrun under #64.
The known audio-alignment cancellation failure remains unchanged with cause
unproven. ADR-0011 is proposed, not accepted. Cargo remains 0.3.0.

Parent #110 remains open for final-video assembly with a derived clock, fresh
resource/device observations, broader rates/cut import, fine pair recovery and
native Apple Silicon qualification. Next checkpoint: consume accepted RIFE
frames/index with explicit source audio and final decode/timing acceptance.
Optional #111/#112/#113 and #106/#81 remain separate selectable recipe lanes under #107.
No merge, test execution, model download, private-media run or publication is
authorized merely by this handoff. No release/native-safety claim is made.
