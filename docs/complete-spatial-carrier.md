# Compose a complete recovered frame set

`upscayl_recovery::carrier::{inspect,plan,create,observe}` combines explicit
current historical-import and native per-frame/chunk selections. Every selected
frame must match the same original source/index, ordinal and rational interval,
with uniform explicit 2x/3x/4x dimensions. Each frame keeps its own origin:
reviewed historical processing is unknown; current native processing is bound
to its exact checkpoint/provider/report/settings. This never calls historical
frames native Upscayl output or approves the complete movie's visual quality.

Supply `aniflow.spatial-carrier-request/v1` with original `source_video`,
`source_index`, selected `sources`, explicit `output_width`/`output_height` and
finite source/frame/total/working/free-space limits. A historical source names
its native import `run_directory` and 1..128 selected increasing `ordinals`.
A native source names its `run_directory`, exact existing recovery
`configuration`, and selected `units` (`stage_id`, increasing `ordinals`). A
chunk must be wholly accepted before any member can be selected. A partial run
can contribute currently accepted non-excluded units; counts or loose PNG/report
files alone never grant that authority. At most 100000 frames are selected.

```bash
aniflow --output json video spatial-carrier inspect --request recovered.json
aniflow --output json video spatial-carrier plan --request recovered.json
aniflow --output json video spatial-carrier create --request recovered.json --output-directory /private/recovered
aniflow --output json video spatial-carrier status --directory /private/recovered --carrier-sha256 KNOWN_CARRIER_JSON_DIGEST
```

Inspect is read only and lists missing original ordinals. Plan/create require
every original ordinal exactly once; gaps, overlaps, renamed/retimed frames,
different source/index/geometry and stale/changed origins refuse. Create copies
only exact pinned PNGs into fresh private `frames/`, then writes `carrier.json`
last. Preserve that raw file digest separately for status. No external media
process is launched during composition. Partial copies are retained and never
adopted automatically; use explicit reconciliation or a different fresh root.

Readback uses current ordinary v3 acceptance, exact original source/index,
retained origin/checkpoint/report identities, strict copied inventory and PNG
structure/CRC/identity. Unrelated native progress can advance without changing a
selected checkpoint; rerun/exclusion, changed selected outputs or changed source
invalidate the binding. Original files stay immutable. The carrier composes
existing acceptance; assembly must perform its own complete decode/clock gates.

Six SDK coverage/clock/bounds/loose-evidence/read-only refusal cases and one CLI
parser case are authored, uncompiled/unrun. Native/platform/visual qualification,
all tests/build/compiler/dependencies/validators/CLI/SDK/media/provider/model/CI
execution remain deferred to [flow #88](https://github.com/egohygiene/flow/issues/88),
scheduled last. Known alignment cancellation/start-marker failure unresolved;
cause unproven. Proposed ADR-0018 is not ratified by this source checkpoint.
