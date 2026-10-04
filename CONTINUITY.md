---
schema_version: aether.repository-continuity/v1
repository:
  id: egohygiene/aniflow
  visibility: public
  default_branch: main
  continuity_path: CONTINUITY.md
document:
  status: active
  updated_at: "2026-10-04T16:39:45Z"
  max_bytes: 16384
  max_lines: 240
  stale_reason: null
  superseded_by: null
scope:
  purpose: Resume the toolchain planning and bounded probe checkpoints without inventing qualification.
  includes:
    - Current implementation, immutable base, deferred checks and next work
  excludes:
    - conversation transcripts
    - duplicated architecture, roadmap, and changelog content
  precedence:
    - user-and-runtime-instructions
    - scoped-repository-instructions
    - live-repository-and-work-tracker-state
    - canonical-repository-sources
    - continuity-checkpoint
  canonical_sources:
    - AGENTS.md
    - .github/copilot-instructions.md
    - docs/toolchain-profiles.md
    - docs/toolchain-upstreams.md
    - docs/toolchain-probes.md
    - docs/validation/aniflow-102-checkpoint.md
    - ROADMAP.md
work:
  objective: Extend draft PR100 with macOS checkpoint102; keep remaining parent38 scope explicit.
  success_conditions:
    - Checkpoints97 through99,101 and102 preserve explicit setup and evidence boundaries.
    - The draft distinguishes inventory consistency from native qualification and runtime integration.
  active_issue:
    provider: github
    id: "egohygiene/aniflow#102"
    url: https://github.com/egohygiene/aniflow/issues/102
  next:
    kind: action
    id: review-toolchain-draft
    description: Review PR100 including checkpoint102, then connect toolchain evidence to explicit setup and processing preflight; qualification stays deferred.
    readiness: ready
    references:
      - https://github.com/egohygiene/aniflow/issues/38
      - https://github.com/egohygiene/aniflow/issues/64
    depends_on: []
state:
  base:
    revision: 0e5773062a7a3d2d07e05d47d698a38002bd670b
    ref: refs/heads/main
    verified_at: "2026-10-04T16:39:45Z"
  candidate:
    branch: feat/aniflow-38-toolchain-profiles
    revision: cb3091b4bbb83877fe02dca4fd31a7a7f2a41965
    pull_request: https://github.com/egohygiene/aniflow/pull/100
    handoff_state: ready-for-review
  live:
    status: partial
    observed_at: "2026-10-04T16:39:45Z"
    default_branch_revision: 0e5773062a7a3d2d07e05d47d698a38002bd670b
    issue_state: open
    pull_request_state: open-draft
    notes: PR100 remained open/draft at 179db95dc8d0c7836c5ecbb13dac411c3bb72e4d before this update. Candidate revision is local implementation; GitHub records the later publication head. No merge or hosted CI inspection occurred.
  parallel_changes: []
review:
  status: partial
  reviewed_at: "2026-10-04T16:39:45Z"
  reviewed_by: codex
  evidence:
    - command: Read-only source review of bounded probe lifecycle, parsers, contracts and authored fixtures
      outcome: limited
      observed_at: "2026-10-04T16:39:45Z"
      notes: macOS kqueue API and exit-registration source reviewed; inherited-pipe early cleanup repaired; no compiler or execution evidence.
    - command: Tests, compiler/build, lint/format, schema/drift, smoke/package, native and hosted qualification
      outcome: not-run
      observed_at: "2026-10-04T16:39:45Z"
      notes: Maintainer deferral under issue64; authored coverage is not qualification.
  environment_limitations:
    - Git transport previously unavailable; publish through connector using the actual remote PR head as parent and exact tree comparison.
    - No deterministic continuity validator was run.
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

Read AGENTS.md and verify current main, PR100 and issues102/38 before resuming.
Live GitHub state supersedes this pre-publication snapshot. No future merge,
check execution, release or media-processing authority is granted here.

PR96 remains merged at `0e5773062a7a3d2d07e05d47d698a38002bd670b`.
PR100 remained open/draft at `179db95dc8d0c7836c5ecbb13dac411c3bb72e4d`
before this update. No merge occurred. Native audio80 retains its bounded
PCM24/float32 semantics; native loudness/true peak remain unavailable.

Local implementation `cb3091b4bbb83877fe02dca4fd31a7a7f2a41965`, tree
`08e6f3331443baf384a02f154b09e01c3c32c794`, adds #102 to #97–#99/#101:
macOS kqueue observation for the existing fixed FFmpeg/ffprobe probes. The
leader remains unreaped through final group signals; the late-registration
path preserves natural capture closure before reaping. EOF alone is not exit.
Linux non-uClibc retains its existing path; other live hosts remain refused.
The caller must own child waits exclusively and disable automatic reaping.

Five new Rust cases are authored: three macOS internal and two shared process
cases. The shared suite now contains fourteen macOS/Linux process cases and
one portable document case. Test-only Python3.10+ is explicitly resolved via
ANIFLOW_TEST_PYTHON or /usr/bin/python3; no installer, PATH discovery or silent
skip. All checks and native/platform execution remain unrun under #64.

Complete diagnostic evidence does not establish media processing, available
GPU/device support, authenticated package identity or provider registration.
Before/after original-path hashes are not atomic executed-file attestation.
See the #102 handoff/receipt and probe guide; preserve historical #38/#101
receipts. The known alignment cancellation failure remains unchanged at
tests/audio_alignment.rs:547 (joining thread561), cause unproven. No real
media/model or release was executed. Cargo remains0.3.0.

Next under #38: explicit adapter setup and processing preflight from selected
evidence, complete environment identity, other tool/device probes and model
locator discovery, plus actual qualification when authorized. Then proceed
#36 → #37 → #35 → #83 → #82. Optional #81/#39 remain separate. Flow#78 captures
an AMV preset over released capabilities; Flow's general workflow CLI and
Aniflow adapter integration are not delivered by these local probe changes.
#79 still needs source-file recovery and human review; #24 retains corpus work.

Release path: #64 and Egolint#29 qualification, then actual immutable #10
publication before Flow#51; later music-video release uses #40 and Flow#75.
Preserve historical failures and unrun results.
