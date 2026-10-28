# Extract an indexed frame workspace

Use `video extract-frames` to turn a supported source video into a complete
PNG directory and an original `aniflow.rife-frame-index/v1` index. The accepted
outputs can be passed directly to the existing [music-video recipe](music-video-recipe.md)
for selected repair, optional upscaling and verified assembly. The original
video remains unchanged and supplies the original audio to later assembly.

This is the standalone #35 extraction checkpoint. It uses one ordinary
Pipeline v3 stage and the existing execution, validation, status and resume
owners. It does not add extraction as a new phase inside the recipe. Source
implementation and useful regression cases are authored; all execution and
native qualification remain deferred under
[#64](https://github.com/egohygiene/aniflow/issues/64).

## Pin the source tools and stream selection

Create an extraction configuration using the exact installed FFmpeg and ffprobe
8.0.1 identities. Paths and digest placeholders below must be replaced with
your trusted local executable identities. This example selects input-global
video stream 0 and audio stream 1; it does not infer those positions for you.

```json
{
  "schema": "aniflow.frame-extraction-configuration/v1",
  "ffmpeg": {
    "executable": "/absolute/bin/ffmpeg",
    "version": "8.0.1",
    "sha256": "REPLACE_WITH_OBSERVED_FFMPEG_SHA256"
  },
  "ffprobe": {
    "executable": "/absolute/bin/ffprobe",
    "version": "8.0.1",
    "sha256": "REPLACE_WITH_OBSERVED_FFPROBE_SHA256"
  },
  "source_selection": {
    "video_stream": 0,
    "audio_stream": 1,
    "no_audio": false,
    "discard_streams": []
  }
}
```

The configuration requires an explicit video stream and either an explicit
audio stream or `no_audio: true`; it does not automatically select streams.
For an intentionally video-only result, use `audio_stream: null` and
`no_audio: true`; acknowledge any omitted source streams in `discard_streams`.
Stream choices and discards follow the existing [temporal contract](temporal-correctness.md).
Extraction does not write a separate audio file or select an audio reencode.

The [closed caller schema](../providers/frame-extraction/configuration.schema.json)
also exposes finite source, frame, aggregate, pixel, frame-count, native-time,
capture, working-space and free-space limits. Defaults include 100,000 frames,
8,400,000 pixels per frame and 64 MiB per compressed PNG. The pixel ceiling can
be explicitly raised as far as 33,554,432; this is not an 8K qualification claim,
and every other bound still applies. Lower configured bounds can refuse work
that otherwise fits the supported media profile.

## Plan, run and resume

First request a read-only plan:

```bash
aniflow --output json video extract-frames \
  --source-video "/absolute/source video.mp4" \
  --configuration "/absolute/extraction.json" \
  --plan
```

Planning hashes the current source and selected tool files, resolves the exact
provider and freezes the configuration. It launches no probe or decoder and
creates no run workspace. Source support, selected geometry and actual frame
count therefore remain pending until execution observes the source. A plan is
not a claim that the video will pass native admission or that disk is reserved.

Replace `--plan` with `--output-directory "/absolute/extraction-runs"` to
start. This is a parent directory for a new isolated v3 run, not the PNG output
directory. Execution snapshots the exact inputs, observes the supported source
profile, extracts and verifies the complete frame set, then lets the existing
runtime accept the declared outputs.

The result uses the ordinary v3 outcome. Resolve these accepted artifact IDs
from that result; do not guess stage paths or treat a discovered PNG directory
as completion evidence:

| Artifact ID | Content |
| --- | --- |
| `extracted_frames` | Complete RGB PNG directory in source-frame order |
| `extracted_index` | Original RIFE input index with exact source identity, frame digests, geometry and rational source times |
| `extraction_evidence` | Source/conversion observations and complete-set identities for this extraction |

After interruption, retain the same source, configuration and tool files and
use `--resume-run-directory "/absolute/extraction-runs/RUN_DIRECTORY"` in
place of `--plan` or `--output-directory`. Recovery is whole-stage: an accepted
compatible extraction can be reused; an interrupted extraction retries the
complete stage. Partial PNGs are retained as failure evidence and are not
adopted as accepted frames. Changed source, tools or configuration cannot reuse
an incompatible checkpoint.

Read status without launching media tools:

```bash
aniflow --output json status-v3 "/absolute/extraction-runs/RUN_DIRECTORY"
```

The ordinary v3 writer-lock and incomplete-initialization rules still apply.
An abandoned lock or a directory without complete initialization is not
automatically overwritten. Keep the run and its accepted artifacts together.

## Continue into the music-video recipe

After extraction completes, use the reported paths for `extracted_frames` and
`extracted_index` with your separately selected recipe configuration:

```bash
aniflow --output json video music-video \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/REPORTED_EXTRACTED_FRAMES_PATH" \
  --frame-index "/absolute/REPORTED_EXTRACTED_INDEX_PATH" \
  --configuration "/absolute/music-video.json" \
  --plan
```

Then replace `--plan` with `--output-directory "/absolute/music-video-runs"`
to run the recipe. Its immutable import or selected repair, optional Upscayl
and verified assembly retain their own acceptance and compatibility checks.
Extraction evidence does not bypass those checks. The original source video,
extraction workspace and recipe workspace remain distinct; this workflow does
not replace or prune them automatically.

The two commands expose an explicit handoff. A single source-only recipe
command, automatic choices for optional steps and broader manual/review
composition are not supplied by this checkpoint.

## What is observed

The native profile shares the assembly owner's strict MP4/MOV source admission:
contiguous zero-origin CFR, a selected moving-video stream, even square-pixel
unrotated progressive geometry, explicitly tagged opaque 8-bit BT.709 SDR, and
no audio or supported selected AAC-LC. Unsupported VFR, offsets, longer A/V
tails, HDR, alpha, rotation, other color profiles and derived RIFE clocks are
refused rather than normalized.

Extraction converts the selected source to full-range RGB24 with explicit
BT.709 matrix/range handling. Each PNG is fully decoded and its RGB pixels
are compared with the same conversion of the corresponding source frame.
The complete set retains original ordinals and rational source times in the
existing index contract. The report distinguishes observed extraction from an
arbitrary imported frame workspace's declared source relationship.

Extraction does not detect shot cuts. The new index's `cut_before_ordinals`
is empty; that is not evidence that the source has no cuts and does not qualify
RIFE interpolation across them. This checkpoint's practical continuation is
the original-clock repair/upscale/assembly path.

PNG compression is lossless for those RGB values. A YUV-to-RGB conversion can
round values; it is not a mathematically lossless round trip to the encoded
source. These observations establish the selected conversion and exact indexed
frame relationship, not earlier authenticity, authorship, visual quality or
host/native qualification. `native_qualified` remains false.

Configured byte/time/capture limits and conservative storage admission remain
bounded, but are not filesystem quotas, RSS/VRAM guarantees or measured device
suitability. Initial admission covers source/tool snapshots and observations;
after source geometry and count are observed, a conservative PNG allowance
must fit the configured frame, aggregate and working-space limits and available
disk. A highly compressible input can still be refused by that allowance; the
planner does not predict its eventual PNG compression ratio.
Retained source/tool snapshots, generated frames, observations and
failed attempts can consume substantial disk. No tool/model download, private
media processing or release is implicit in planning this workflow.

The public `frame_extraction` module provides
`FrameExtractionConfiguration`, `FrameExtractionRequest`, `plan`, `run` and
`resume` with standard v3 plans, outcomes and progress. The CLI machine command
is `video_extract_frames`. Parent #35 remains open for source-only recipe
composition, broader media profiles, loudness/metadata and qualification.
