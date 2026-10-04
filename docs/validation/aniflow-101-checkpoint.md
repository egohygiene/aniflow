# Issue #101 — bounded FFmpeg probe checkpoint

Authored 2026-10-04. This extends draft PR #100 and the historical offline
#97–#99 checkpoint. Parent #38 remains open; no merge occurred in this turn.

## Implementation and publication identity

Main remained `0e5773062a7a3d2d07e05d47d698a38002bd670b`, tree
`650a3376f687311b10556c07ce200eae6458d269`. PR #100 was open and draft at
remote head `d9b007b9e1438f5de17f61fb295e2dbd85138c43`, tree
`709b2176addb14ed0c6cab27187cee704d4bd376`.

Local implementation commit: `278220c828b346b01ce625412b9adf6054794e5e`.
Implementation tree: `590291dc07faebd3797f8aa1af33e727dd27a7d0`.
Publication extends the actual remote PR head and compares its final tree with
the final local handoff tree. The PR records the subsequent remote commit/tree;
this comparison establishes publication fidelity only, never qualification.

## Delivered boundary

- `probe_tools` and `toolchain probe --configuration PATH` perform only fixed
  queries against explicitly pinned FFmpeg/ffprobe executables. The closed v1
  request accepts no arbitrary argv, media input, installation or registration.
- The report retains exact argv, process outcomes, bounded raw stdout/stderr,
  retained-byte digests, elapsed telemetry and parsed facts separately. Report
  loading rebinds parsed facts to captured bytes and the complete request.
- FFmpeg supplies version, encoder, decoder, filter, compiled hardware method
  and full-help evidence; FFprobe supplies version and full-help evidence.
  Unknown vendor/development version tokens are retained without guessing a
  compatible release. Numeric X.Y normalization to X.Y.0 is explicit.
- `complete` concerns diagnostic collection and parsing. Caller package revision
  remains separately attributed and optional; omitting it still leaves the
  corresponding offline inventory readiness requirement unmet. Compiled backend
  lists do not supply GPU/device evidence. Native qualification stays false.
- Linux except uClibc is the first live host adapter. Non-reaping process
  observation reserves the leader PID through group cleanup, with bounded
  nonblocking captures, deadlines, cancellation and cleanup. Other hosts reject
  live probing explicitly; offline doctor/plan retain existing behavior.
- Executable hashes are observed before and after each command, within size and
  aggregate hash budgets. Original-path invocation is not atomic executed-file
  attestation, and environment control is not an OS or network sandbox. Selected
  programs must be trusted by the caller.
- Human previews are bounded; machine output retains all captured evidence.
  Incomplete probes retain their report in the dependency-error envelope.
  Probe observations are reviewable inventory inputs, never automatic inventory
  writes, provider selection, registration or processing authorization.

See [probe guidance](../toolchain-probes.md) and
[immutable grammar references](../toolchain-upstreams.md).

## Authored coverage and source review

Four parser unit tests, thirteen Rust integration/document cases (twelve gated
to the Linux live adapter), two new CLI tests, an extended command-name case and
six Python schema cases are authored. Cases cover fixed argv and literal paths,
pin changes, malformed output, version normalization, capture identity,
timeouts/cancellation/overflow, inherited pipes, cleanup, missing device/package
evidence and retained CLI failures. **All remain unrun.**

Read-only review repaired the process-group ID reuse risk, package/completion
semantics, diagnostic bounds, synthetic version-banner grammar and stable
observation provenance. Duration remains separate telemetry. Review of source
is not compiler or runtime evidence.

Under #64, no tests, compiler/build, formatting/lint, schema/drift, smoke/package,
native tools/models/platforms or hosted CI dispatch/polling ran. No real/private
media was read or processed. No binary/model download, release or tag occurred.
Cargo remains 0.3.0.

The known failure remains unchanged:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`,
`tests/audio_alignment.rs:547` (`marker.exists()`), joining thread561.
The cause is unproven.

## Remaining #38 work

1. A safe macOS live process adapter, other selected-tool probes and actual
   device/backend evidence; do not invent unsupported version commands.
2. Complete package/runtime/native dependency and adapter identity, then explicit
   registration setup and processing preflight using the existing exact locks.
3. Reviewable model-locator discovery and selected model origin/terms evidence.
4. Actual platform, codec, image/video and native behavior qualification under
   #64 when authorized. Include the new probe suites in that accumulated debt.

After #38: #36 → #37 → #35 → #83 → #82. Flow #78 captures the AMV preset.
Release qualification remains #64/Egolint #29 before actual #10 publication and
Flow #51; later music-video release uses #40/Flow #75.
