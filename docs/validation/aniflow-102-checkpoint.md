# Issue #102 — macOS core probe checkpoint

Authored 2026-10-04. This extends draft PR #100 and the #101 core diagnostic
probe checkpoint. Parent #38 remains open. No merge occurred in this turn.

## Implementation and publication identity

Main remained `0e5773062a7a3d2d07e05d47d698a38002bd670b`, tree
`650a3376f687311b10556c07ce200eae6458d269`. The starting PR #100 remote head was
`179db95dc8d0c7836c5ecbb13dac411c3bb72e4d`, tree
`40da5bc41330d5a3f4b6debbb1bd13c9eda11096`.

Local implementation commit: `cb3091b4bbb83877fe02dca4fd31a7a7f2a41965`.
Implementation tree: `08e6f3331443baf384a02f154b09e01c3c32c794`.
Publication extends the actual remote PR head. Its final tree must equal the
final local handoff tree; the PR records the resulting remote commit/tree.
This establishes publication fidelity only, not execution qualification.

## Delivered boundary

- macOS now has an authored process adapter for the same pinned FFmpeg/ffprobe
  queries, with no request/report version or command changes. Linux excluding
  uClibc retains its non-reaping wait path; unsupported hosts remain explicit.
- Safe nix `Kqueue`/`KEvent` wrappers observe the exact direct child's exit
  without releasing its PID. The existing locked nix 0.31.3 gains its `event`
  feature; no dependency version or lockfile changes and no unsafe Rust are
  introduced.
- A child already exiting before registration has a separate state. Capture
  continues until natural pipe closure or a bounded stop condition. Only then
  are final group signals sent, signaling disarmed and the actual status reaped.
  EOF alone never means a still-running child has exited.
- Process/capture/hash budgets, cancellation, evidence parsing, before/after
  pins, offline doctor/plan and explicit provider-selection boundaries remain
  intact. Compiled hardware methods do not establish GPU/device availability.
  `native_qualification` stays false.
- The caller must retain exclusive child-wait ownership and must not enable
  automatic `SIGCHLD` reaping. Group cleanup does not contain intentionally
  escaped descendants. Original-path hashes remain observations, not atomic
  executed-file attestation; programs are explicitly trusted.
- The shared synthetic fixture accepts `ANIFLOW_TEST_PYTHON` as an absolute
  interpreter path, with `/usr/bin/python3` as its fixed default. It fails
  actionably on missing prerequisites, never installs or searches PATH, and
  preserves the tested executable's literal spaces/Unicode/shell characters.
  Python 3.10+ is a test prerequisite, not a production probe dependency.

The [probe guide](../toolchain-probes.md) records host and interpreter limits.
[Primary source references](../toolchain-upstreams.md) distinguish the inferred
XNU registration behavior from actual platform execution evidence.

## Authored evidence and source review

Five new Rust cases are authored: three macOS internal cases for strict event
decoding, forced delayed observer registration and delayed registration with
inherited pipes; two shared integration cases for rapid exits and live leaders
closing both streams. The existing inherited-pipe case now also checks leader
exit, truncated capture and descendant creation/cleanup separately. The shared
integration file has fourteen live process cases plus one portable document
case. All these cases remain unrun.

Read-only review caught and repaired premature cleanup in the late-registration
path, which could cut off descendant output and produce different completion
behavior from Linux. Re-review found no remaining concrete source issue. That
is not compiler or runtime evidence.

**All execution checks remain unrun under #64:** tests, compiler/build, format
and lint, schema/drift, smoke/package, real FFmpeg/ffprobe, macOS/Linux native
qualification and hosted CI dispatch/polling. No real/private media, models,
download, release or tag occurred. Cargo remains 0.3.0.

Known failure unchanged:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`,
`tests/audio_alignment.rs:547` (`marker.exists()`), joining thread561;
cause unproven.

## Next boundary

Continue #38 by connecting selected toolchain evidence to reviewable adapter
setup and processing preflight while preserving exact registrations and locks.
Other selected-tool/device probes, environment identity, model discovery and
actual qualification remain open. After #38: #36 → #37 → #35 → #83 → #82.
Flow #78 captures the AMV preset; actual integration still needs the qualified
immutable release and downstream adapter work.
