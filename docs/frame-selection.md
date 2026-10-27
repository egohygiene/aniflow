# Frame selection preview

Preview specific frame ranges before an external repair. The command takes an
existing `aniflow.rife-frame-index/v1` document and reports exactly which frame
positions and source-time intervals the selection describes. It does not need a
RIFE configuration, model, executable, image directory or source video.

This first [#36](https://github.com/egohygiene/aniflow/issues/36) checkpoint is
authored source, with execution checks deferred under
[#64](https://github.com/egohygiene/aniflow/issues/64). It is not the completed
workspace importer or repair operation.

## Select ranges

```bash
aniflow video frame-selection-plan \
  --frame-index "frames/index.json" \
  --range "1:299" \
  --range "600:898"
```

`FIRST:LAST` means **one-based inclusive positions** in the complete index.
It does not interpret numbers embedded in filenames. `1:299` selects 299
frames, with canonical zero-based ordinals `[0, 299)`. `600:898` selects
another 299, with ordinals `[599, 898)`. The preview displays each range's
actual first/last filename and exact rational, half-open source-time interval.
For a 1,000-frame input this request selects 598 and leaves 402 unselected.

Ranges may be supplied in any order; the result sorts them by source position.
Adjacent ranges stay separate. Repeated or overlapping frames are errors, not
silently deduplicated. Gaps between selected ranges are allowed and count as
unselected frames. Gaps in the complete input mapping are refused, even outside
the selected ranges. Empty, zero-based, reversed and out-of-bounds requests are
refused. The limit is 1,024 ranges over the input profile's 100,000-frame maximum.
Time-string selection is intentionally absent: the operation never guesses
which frame a decimal timestamp means.

For automation:

```bash
aniflow --output json video frame-selection-plan \
  --frame-index "frames/index.json" \
  --range "1:299"
```

Success uses the existing machine envelope, command `video_frame_selection_plan`,
with an `aniflow.frame-selection-plan/v1` result. Existing machine failures
remain on stderr; this checkpoint does not change the product's stream contract.
Shell redirection is an explicit caller action if a saved preview is desired.

## Input and evidence

The [existing frame-index contract](contracts/rife-frame-index-v1.schema.json)
already contains a complete exact `TemporalInspection`, source geometry,
ordered frame filenames/digests and optional cut boundaries. This operation
uses that document through an explicit adapter, sharing its clock and mapping
validation instead of introducing another frame manifest. The accepted input
profile remains CFR, zero-origin, RGB8/SDR, square and unrotated as declared by
that index. The [temporal rules](temporal-correctness.md) are recomputed; saved
support flags do not override inconsistent timestamps. Cut declarations retain
their existing validation and enter the bound index digest; a selected range
may cross a cut because this is selection, not interpolation or repair.

The CLI accepts an index up to 16 MiB and reads at most one additional byte to
detect growth beyond that bound. It initially checks for a regular nonsymlink
JSON file; this is not a race-free file-opening guarantee. The library accepts
a parsed `RifeFrameIndex`. Every declared filename/ordinal/digest is
checked, including unselected frames. Missing/duplicate entries, unsafe
basenames, geometry/count contradictions and a 240-frame mapping for a declared
299-frame timeline fail. This detects inconsistent **declarations**, not a
missing physical file: no PNG or original source is opened by this command.

The result binds the index's canonical JSON SHA-256, including selected and
unselected frame declarations, and the declared original source digest.
Formatting differences in equivalent JSON do not change the canonical digest.
`evidence_basis` is always `index_declarations_only` and
`historical_processing` is always `unknown`. The index's name, dimensions and
hashes do not identify which historical tool edited the frames.

The `unselected_frame_count` field means **intended unselected cardinality**. It
does not assert that any bytes were preserved. No decoded-integrity, native
qualification, import/stage acceptance, provider lock, checkpoint, execution or
visual approval is created. Deserializing a preview grants no authority; future
execution must re-observe the inputs and recompute the selection against them.

## Library

```rust
use aniflow::frame_selection::{plan_from_rife_index, InclusiveFrameRange};
use aniflow::video_interpolation::RifeFrameIndex;

fn preview(index: &RifeFrameIndex) -> aniflow::Result<()> {
    let ranges = ["1:299".parse::<InclusiveFrameRange>()?];
    let plan = plan_from_rife_index(index, &ranges)?;
    assert_eq!(plan.selected_frame_count, 299);
    Ok(())
}
```

`plan_from_rife_index` is pure and leaves its inputs unchanged. It shares the
CLI's validation and count/time arithmetic. The serialized plan's
[schema](contracts/frame-selection-plan-v1.schema.json) defines shape; ordering,
cross-field arithmetic and source-time relationships require the Rust planner.

## Remaining import and repair work

The complete #36 workflow must independently bind the source video bytes,
decode every frame within resource limits, establish dimensions/depth/color/
alpha semantics, and copy into a new immutable stage before acceptance. The
current RIFE PNG structure/CRC checker does not prove successful decompression.
An actual import capability may have its own contemporary execution evidence;
it must retain unknown historical frame-processing provenance instead of
inventing prior provider locks.

The standalone `video-frames/1` format has no pinned specification/example in
this repository, so no migration adapter is claimed. Repair execution must
preserve all unselected bytes, bind returned frames to exact source positions,
reject cardinality drift (including 299 to 240), and handle explicit clip limits
and seams before promotion. None of those operations is performed by this
preview. See the [checkpoint handoff](validation/aniflow-36-checkpoint.md).
