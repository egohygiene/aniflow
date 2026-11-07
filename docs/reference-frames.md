# Select reference images from an existing clip

Use `frame_extraction::references` to create a small ordered set for local human
review, then manually hand selected images to a separate creative app. It does
not generate video, call a model, infer character identity, upload or publish.
Original video/audio and earlier runs remain unchanged.

The [SDK/CLI example](../examples/reference_frames.rs) accepts an exact local
source, existing extraction settings JSON, sampling-policy JSON, the actual
Aniflow provider executable, a separate run parent/root and `plan|run|resume`.
The extraction settings use the existing
[source extraction guide](video-frame-extraction.md) and pinned FFmpeg/ffprobe
8.0.1. Set finite byte/count/pixel/time/capture/free-space bounds appropriate to
the clip and selected images. The native profile initially supports the same
unrotated square-pixel BT.709 SDR, zero-origin contiguous CFR MP4/MOV and
selected no-audio/AAC-LC source subset. VFR, unsupported clocks/color/streams,
empty media and source changes refuse explicitly; no timing is invented.

Example sampling policy:

```json
{
  "schema": "aniflow.reference-sampling/v1",
  "every_n_frames": 60,
  "maximum_samples": 32,
  "include_first": true,
  "include_last": true,
  "overflow": "reject"
}
```

At 30 fps this requests a periodic position every two seconds, starting at
frame zero, plus the exact last frame. Selection preserves source ordinals,
timestamps and reasons, deduplicating only an overlapping position. Identical
pixels at different positions remain separate useful references. A one-frame
clip produces one image with first/periodic/last reasons. Cuts are not inferred;
this first selector makes no scene-coverage or semantic-importance claim.

The caller cap is 1–256 images. If periodic plus first/last exceeds it, the
operation refuses; increase the interval or explicit cap. It does not silently
change cadence or drop the end. Count/geometry/clock admission occurs after
native observation; read-only planning binds policy/source/tools and does not
pretend to know source geometry. Original temporal inspection remains intact.

After separately qualified setup, an example manual invocation is:

```sh
cargo run --example reference_frames -- /absolute/clip.mp4 /absolute/extraction.json /absolute/sampling.json /absolute/aniflow /absolute/reference-runs plan
```

Replace `plan` with `run` to request a new isolated run. For `resume`, supply its
exact retained run directory. These are future operator instructions, not
commands executed for this checkpoint. Resume uses the ordinary v3 writer and
immutable whole-stage checkpoints. Accepted artifacts can be reused; an
interrupted attempt reruns that bounded stage, without per-image adoption from
failed scratch. Changing source, policy or pins requires a new run.

Only selected PNGs are materialized, never a complete PNG copy of the clip.
The shared extraction conversion is explicitly full-range RGB24 BT.709.
Independent source framehash and selected PNG generation retain original
PTS/durations through a closed ordinal filter. Every PNG is structurally
inspected, fully decoded and compared to its selected source RGB hash. Fresh
source observations and exact source/tool/image hashes are rechecked. Existing
process runner/runtime own timeout, capture, cancellation, artifact bounds and
acceptance; free-space checks observe availability without reserving it.

Current accepted outputs are `artifacts/references/frames`, `manifest.json` and
`README.md`. Locate them through run output/status records, not leftovers or
guessed filenames. The JSON records source hash/size, original frame clock,
exact selected source intervals/reasons, dimensions, PNG/pixel hashes, policy
and tool identities. Markdown links to generated local files for manual review.
A manifest alone is not runtime acceptance or visual approval. A sparse set is
not an assembly input or complete source frame index. #116 review and future
selectors can consume it without expanding this owner into a generation loop.

Six meaningful synthetic cases are authored and remain uncompiled/unrun. No
compiler/build/dependency/test/lint/format/schema/docs/native/provider/media/
model/CLI/SDK/CI execution was performed. #64 retains qualification and the known
alignment cancellation/start-marker failure (cause unproven). #136 stays open
for full source/native/platform acceptance and broader VFR/selectors. No real
private clip was processed, reviewed or shared.
