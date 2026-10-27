# #36 frame-selection preview checkpoint

## Scope and source

Base: main `7a808ff99ca8f557f6a3ba7fd373a393de0feee6`.
Branch: `codex/36-frame-selection-preview`. The containing draft PR identifies
the published candidate; this handoff does not invent its own commit or PR ID.

The maintainer authorized continued implementation and frequent draft saves,
with tests/build/lint/format/schema/native/provider/CLI/CI execution deferred.
This is the first bounded source checkpoint for
[#36](https://github.com/egohygiene/aniflow/issues/36), not completion of that
issue or authorization to process real media. No dependencies or Cargo files
change. Use `[skip ci]` and disabled Git hooks for checkpoint commits.

## Authored behavior

- `frame_selection::plan_from_rife_index` is the shared pure library operation.
  It adapts the existing `RifeFrameIndex`; no second frame-input format is added.
- `InclusiveFrameRange` parses `FIRST:LAST` as one-based inclusive positions,
  with no whitespace, signs, decimal timestamps or inferred filename numbering.
  Explicitly constructed library ranges undergo the same bound checks.
- Input ranges are sorted, duplicates/overlaps refused, and adjacency retained.
  Half-open zero-based ordinals and exact `TimeRange` spans derive from the
  existing complete source timeline, without floating-point conversion.
- The full existing index validator checks geometry/cardinality, portable
  unique filenames, consecutive ordinals, digests, cuts and temporal support.
  Unselected entries must remain complete. Selected gaps are allowed and count
  as unselected intent. A 240-entry index against a 299-frame timeline fails.
- `video frame-selection-plan --frame-index FILE --range FIRST:LAST` prints
  selected/unselected totals, ranges, exact times and endpoint filenames.
  `--output json` uses command `video_frame_selection_plan` and additive
  `aniflow.frame-selection-plan/v1` under the existing machine envelope.
- The CLI shares one bounded regular-file index reader with the existing RIFE
  planning command. It does not load RIFE configuration or tools for selection.
- The preview binds canonical complete-index and declared source digests.
  `evidence_basis=index_declarations_only` and
  `historical_processing=unknown` are explicit. No source/PNG bytes are read or
  decoded; no file is created by the operation. Counts cannot attest preserved
  bytes. JSON deserialization is not an acceptance gate.

The [user guide](../frame-selection.md), public schema, machine enum, contract
registry, README and roadmap describe that same boundary. Current machine
failures remain on stderr; the separate suite observability migration owns any
change to global output behavior.

## Review and verification

Source-level review covers API/CLI/schema names, exact range arithmetic,
complete-input admission, preserved RIFE dispatch and evidence claims. Ten
synthetic Rust regressions are authored in `tests/frame_selection.rs`, including
one actual-binary test **for later execution**. They cover two 299-frame ranges,
fractional clocks, gap counts, sorting, endpoints, invalid selections, truncated
or malformed mappings, digest binding, parser behavior and index-only CLI
side effects. Existing command-enum coverage is extended.

No tests, compiler/build/check, formatting/lint, schema validation, CLI or
native/provider execution, image decoding, dependency installation, downloads
or CI ran. No Actions were dispatched or polled. Source review is not passing
execution, decoded fidelity, platform or release evidence. All #36 acceptance
criteria and #64's existing debt remain open, including the known alignment
cancellation/start-marker failure with unproven cause.

## Remaining coherent checkpoints

1. **Immutable frame import:** bind actual source bytes to the temporal
   inspection, observe every declared file with confined paths and strict
   count/name matching, decode bounded PNGs and establish dimensions/depth/
   color/alpha facts, then copy into an exclusive immutable output stage.
   Reobserve mutable inputs before acceptance and retain failures/cancellation.
   The current PNG header and RIFE chunk/CRC checks are not decompression proof.
2. **Historical provenance:** import is a new operation with its own real
   contemporary evidence. Unknown external processing remains unknown. Do not
   fabricate a prior provider lock, run manifest or accepted checkpoint to make
   imported frames look natively generated. A genuine import capability can use
   the existing Pipeline v3 provider/acceptance runtime.
3. **`video-frames/1` migration:** obtain and pin the actual standalone schema
   or representative source. It is absent from this checkout; no guessed adapter
   or compatibility promise is included. Refuse unsupported inputs explicitly.
4. **Selective repair:** invoke only the explicit selected provider/profile,
   write edited outputs separately, copy unselected frames byte-for-byte, bind
   every return to its exact ordinal/time, and reject missing/extra/renamed or
   truncated output before promotion. Clip-only providers require verified
   cardinality/timing and explicit bounded splitting/seam handling.
5. **Qualification:** when the maintainer resumes #64, compile/run the focused
   and existing temporal/RIFE/machine/schema coverage, then the broader required
   gates. Native fixtures and actual import/repair evidence belong to their
   implementation checkpoints, not this sidecar-only preview.

## Architectural impact

No new runtime, workspace or authority model is selected. The pure preview
reuses #32's source clock and the existing declared frame index; #33 acceptance
and Pipeline v3 locks remain unchanged. No ADR is needed for this additive
selection projection. A future import acceptance/migration decision must be
reviewed against the existing canonical architecture before implementation.

Keep this draft narrow. #36 and parent #35 remain open; broader provider setup,
RIFE assembly, release and Flow integration keep their existing owners.
