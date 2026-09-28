# Audio inspection checkpoint

Issue [#43](https://github.com/egohygiene/aniflow/issues/43) is checkpoint 2 of
parent [#13](https://github.com/egohygiene/aniflow/issues/13).

## Draft checkpoint 1

Main `885afc86e3b313267099a14f8e91f5c836eb2a73` contains the maintainer's
merge of #52, completing #42. Live scope, prerequisites and repository
instructions were refreshed before branching. Implementation is in progress.

The selected design is a native Rust stream-inspector provider reached through
the existing fixed `--aniflow-invocation` ABI. A focused public library facade
and `audio plan`, `audio inspect`, and `audio resume` CLI operations use ordinary
Pipeline v3 plans, provider locks, run state and checkpoints. Library callers
supply the provider executable explicitly; CLI callers use the current binary.

The initial profile is plain PCM16 RIFF WAV, one or two channels, 8–192 kHz,
at most 600 seconds and 256 MiB. A private bounded source snapshot is inspected
with pinned local ffprobe and decoded with pinned local FFmpeg. Independent
WAV sample parsing and decoded-PCM hashing must agree. No preview is rendered.
Source and tool identities are rechecked before publication and run-local checkpoint reuse.

Two immutable output ports carry normalized `aniflow.audio-analysis/v1` and
companion `aniflow.audio-technical-inspection/v1` evidence. Metadata validation
and provider evidence remain distinct from the runtime's built-in artifact
integrity gate. #32, #33 and #34 are not absorbed into this checkpoint.

## Recovery plan

1. Read the current PR description and this checkpoint before resuming.
2. Fetch the branch and fresh main; inspect existing partial work rather than
   recreating the implementation.
3. Finish the typed configuration, bounded tool execution, WAV validation,
   provider publication, public facade/CLI, schemas and synthetic fixtures.
4. Check missing/version-changed tools, source/profile refusal, hash mismatch,
   timeout, cancellation, output overflow and resume without provider relaunch.
5. Run meaningful local repository checks and a real FFmpeg/ffprobe smoke over
   generated audio. Record exact results and unverified gates in the local
   receipt, then push a final review checkpoint. Do not poll hosted CI.
6. Stop for maintainer review and merge before a dependent issue. Keep parent
   #13 open. All fixtures remain synthetic; real media is never modified.

## Draft checkpoint 2

The native provider, public facade, CLI, schemas, task wrappers and synthetic
process fixtures are implemented. Draft PR #53 preserves this checkpoint.
Compilation, initial strict library/binary Clippy, contract declarations and
independent schema checks pass. Focused integration and real FFmpeg smoke
checks are in progress; this checkpoint is not a qualification receipt.

Review corrected an output-role mismatch and an invalid cacheability declaration.
The provider is environment-dependent and non-cacheable; existing run-local
checkpoint reuse still requires exact pinned identities. Machine failures use
the existing JSON stderr contract. The CLI configuration loader refuses FIFOs.

Next: finish focused/full local checks, record any remaining gaps and the exact
validated tree, update roadmap handoffs, then mark the PR ready for review.
