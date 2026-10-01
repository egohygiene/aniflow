# aniflow #50 checkpoint

Status: implementation and local validation complete in PR #62. The current
closeout below supersedes the historical in-progress savepoint notes.

Issue: <https://github.com/egohygiene/aniflow/issues/50>
Parent: <https://github.com/egohygiene/aniflow/issues/13> (remains open).
PR: <https://github.com/egohygiene/aniflow/pull/62>

The user authorized direct merge after local checks on 2026-10-01 UTC. This
instruction supersedes the original maintainer-only merge plan below for PR #62.
Confirm the PR's live merged state before starting #51; no #51 implementation is
included in this checkpoint.

## Starting evidence

Fresh main is `b81c91e1fe57130764c536ad4c59bb1c27a73e61`, the maintainer's
merge of PR #61; #49 is closed. Required #42/#43 are closed with merged evidence.
No aniflow PR was open before this branch. The live flow #11 top handoff and
linked holistic graph, all four suite issue/PR/release inventories, repository
instructions and current provider contracts were refreshed before branching.
Older roadmap snapshots remain historical. The workspace was recreated from
GitHub after maintenance; the complete #49 implementation and receipt were
already pushed and merged.

## Original bounded implementation plan

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

## Original recovery and review discipline

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

## Historical implementation savepoint

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

## 2026-10-01 local closeout

The fresh rerun validates pushed implementation
`b434b1ffd7e4cd31fa96aa620bd237b64b3e2ef5`, tree
`02f907d191c4b4a3ecca4abb96ed047d21cc00cb`, against base
`b81c91e1fe57130764c536ad4c59bb1c27a73e61`. The final handoff changes only
documentation and adds the [local receipt](aniflow-50-local.json); production
Rust, adapter, schemas, tests and scripts retain their validated contents.

Passed: 364 stable Rust tests across 31 binaries; 26 unit and 10 integration/CLI
tests on Rust 1.85.1; 12 adapter tests; 16 schema tests; 82 published JSON
documents; 18 unchanged Demucs adapter tests; four actual Task argument checks;
formatting, strict Clippy, naming and diff checks; strict rustdoc, doctests (zero
examples) and a compiled source package with 357 files before this docs closeout.
The complete repository smoke passed synthetic video/provider/recovery and
technical, signal, high-rate, stem, timed-text, transcription, alignment and MIDI
scenarios. The optional musical analyzer was skipped because its explicit
environment was not configured.

All 17 MIDI smoke cases passed. An independent strict JSON loader and Draft
2020-12 validator checked 31 captured reports/companions. External Mido 1.3.3
read back monophonic, polyphonic and empty exports: format/clock, event pairing,
note ticks/pitch/velocity and companion identities agree with normalized
evidence. Source bytes remained unchanged. Independent runtime/export and
documentation/schema/Task reviews found no remaining material blocker.

The earlier PR checkpoint retains the generated-cache corruption observations.
This fresh rerun used isolated stable/package/smoke and focused MSRV targets;
the receipt records exact environment, hashes and successful checks. No product
retry or weakened assertion was introduced. Actual Basic Pitch inference and
accuracy, native-platform support, complete MSRV/dependency closure, OS
isolation, hosted CI and release qualification remain unverified.

After the user-authorized PR #62 merge, #50 is complete and #51 is the next
checkpoint. #51 reconciles the merged audio capabilities through a bounded
synthetic workflow, capability matrix, failure/interruption/resume evidence and
renderflow/flow consumption docs. Parent #13 stays open for that closeout.
No #51 implementation, tag or release is included here.
