# aniflow #50 checkpoint

Status: implementation complete in draft PR #62; final local validation in progress.

Issue: <https://github.com/egohygiene/aniflow/issues/50>
Parent: <https://github.com/egohygiene/aniflow/issues/13> (remains open).
PR: <https://github.com/egohygiene/aniflow/pull/62>

## Starting evidence

Fresh main is `b81c91e1fe57130764c536ad4c59bb1c27a73e61`, the maintainer's
merge of PR #61; #49 is closed. Required #42/#43 are closed with merged evidence.
No aniflow PR was open before this branch. The live flow #11 top handoff and
linked holistic graph, all four suite issue/PR/release inventories, repository
instructions and current provider contracts were refreshed before branching.
Older roadmap snapshots remain historical. The workspace was recreated from
GitHub after maintenance; the complete #49 implementation and receipt were
already pushed and merged.

## Bounded implementation plan

- Integrate one optional local Basic Pitch 0.4.0 ONNX CPU profile, pinned to
  upstream revision `9991303bba609a3b93089d13ec80d1d495083596`. The caller must
  supply explicit local Python, adapter, runtime and model identities. Never
  install a model or provider implicitly.
- Admit mono 22,050 Hz PCM16 WAV, at most 120 seconds. Retain source-relative
  timing and optional existing stem lineage. Unsupported profiles remain
  explicit. Native estimates and derived MIDI never become an authored score.
- Use a bounded adapter around the pinned upstream inference/note APIs. Disable
  pitch-bend inference explicitly. Preserve activation separately from confidence;
  activation is not a calibrated probability. Instrument/channel are not inferred.
- Validate strict note evidence independently of process exit: finite values,
  bounded counts/bytes, source bounds, ordering, pitch/velocity, ambiguous
  same-pitch overlap and dependency/source identity. Preserve polyphony across
  distinct pitches.
- Export a declared Standard MIDI File type-0 subset with fixed 120 BPM and
  960 ticks per quarter note. Record timing quantization and placeholder mapping,
  independently read back the bytes, and refuse collapsed/contradictory notes.
  Publish only to a new output directory with normalized evidence companions.
- Reuse Pipeline v3 planning/run/resume and the existing bounded direct process
  runner, cancellation and dependency rechecks. Ship public library, canonical
  CLI/tasks, strict schemas and synthetic conformance/refusal fixtures together.

## Recovery and review discipline

Push this scope checkpoint as an early draft PR, then implementation and
validation checkpoints to the same branch. Record exact commit/tree identities,
local checks, corrections and remaining gates before review. No hosted-CI
polling. The maintainer owns review and merge.

Synthetic fixtures only. No real-media access/mutation, implicit model downloads,
paid APIs, merges, tags or releases. Actual Basic Pitch/model execution and
accuracy, native macOS/other platforms and release qualification require separate
evidence. No such qualification is claimed by synthetic adapter or MIDI tests.

Stop for this PR's review/merge before #51 reconciliation. Parent #13 remains
open. Downstream product order stays #13 → #32 → #33 → #34 → bounded #24 → #10
→ flow #51; provider audits and final suite release closeout remain later.

## Implementation savepoint

Public contracts, native adapter/runtime, Pipeline v3 lifecycle, CLI/tasks,
strict schemas, MIDI writer/independent reader, immutable export and synthetic
fixtures are implemented. Six public contracts include the versioned schema for
untagged `notes.json` companion.

The first scope savepoint is remote commit
`a2f788902cca7abd3366e6a5c1175248f075c5bb`, tree
`e61277eb0f1521fa3e22eac63e7f42e6e9f70ed0`; it matches its local tree.

Initial all-target compilation, twelve standard-library adapter tests, sixteen
independent schema tests, 82 published JSON documents, four actual Task literal
argument checks, strict Clippy, 26 focused unit tests, formatting and naming
passed. Focused integration verification and
final repository-wide checks are in progress.

Review corrected expected read-back ordering when distinct native onsets round
to the same MIDI tick. The first integration run found that an empty candidate
must not advertise a semantic artifact under an unavailable capability; the
projection is corrected and its unchanged regression is being rerun. Strict
Clippy required moving an implementation block before its test module. No test
assertions were weakened and no retry behavior was introduced.

Remaining: finish corrected focused and full Rust checks, synthetic CLI smoke
and captured-document validation, focused MSRV, docs/package verification,
final evidence receipt and review handoffs. Actual Basic Pitch inference,
accuracy and native-platform/release qualification remain unverified.
