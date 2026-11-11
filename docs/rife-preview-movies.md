# Watchable accepted RIFE previews

`video_interpolation::preview::movie` creates a silent H.264 MP4 viewing copy
from one **currently accepted** ordinary v3 PNG preview. It reuses the existing
FFmpeg/FFprobe 8.0.1 observer and v3 execution, checkpoints, cancellation,
recovery and acceptance. It does not rerun RIFE, interpolate additional frames,
assemble a full video, mux original audio or approve the creative result.

This is an implementation checkpoint for #110. Source and ten synthetic cases
(eight library, two CLI) are authored **uncompiled/unrun**. FFmpeg interfaces,
clock/decode/faststart observations, cancellation, resume, playback, resource
behavior and image/color quality still require the deferred qualification pass.
No native run or usable MP4 is claimed by this source checkpoint.

## Select exact accepted input

First retain the original source/index/model/tool inputs and complete an
ordinary [bounded PNG preview](rife-preview.md). Read its current completion
using `video rife preview-status` and explicitly choose its `report_sha256`.
The movie request requires that digest, not an automatically selected latest
preview or a loose report that merely claims completion.

```json
{
  "schema": "aniflow.rife-preview-movie-request/v1",
  "preview_run_directory": "/private/accepted-png-preview",
  "expected_preview_report_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
  "settings": {
    "schema": "aniflow.video-assembly-configuration/v1",
    "ffmpeg": {
      "executable": "/private/tools/ffmpeg",
      "version": "8.0.1",
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    },
    "ffprobe": {
      "executable": "/private/tools/ffprobe",
      "version": "8.0.1",
      "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    },
    "timeout_milliseconds": 30000,
    "maximum_capture_bytes": 1048576,
    "maximum_source_bytes": 134217728,
    "maximum_output_bytes": 16777216,
    "maximum_frame_bytes": 1048576,
    "maximum_total_frame_bytes": 134217728,
    "maximum_pixels": 8294400,
    "maximum_working_bytes": 1073741824,
    "minimum_free_bytes": 67108864,
    "delivery_crf": 18,
    "preset": "medium"
  },
  "provider_executable": "/private/tools/aniflow"
}
```

Replace every zero digest with the selected observed digest, use canonical
absolute nonsymlink paths, and choose bounds that admit the actual source and
tools. Example bounds are not a claim that a particular movie fits. No tool or
model is downloaded. Planning hashes current inputs and components; it launches
no tools. Delivery metadata policy and arbitrary FFmpeg arguments are refused.

## Library and CLI lifecycle

`MovieRequest::from_json_slice` reads a closed request of at most 64 KiB.
The public library exposes `plan`, `run`, `resume`, ordinary native `status`,
and accepted `observe`. The CLI adapts these same owners:

```text
aniflow video rife preview-movie plan --request movie.json
aniflow video rife preview-movie run --request movie.json --output-directory /private/movie-runs
aniflow video rife preview-movie resume --request movie.json --run-directory /private/movie-runs/RETURNED-RUN
aniflow video rife preview-movie status --run-directory /private/movie-runs/RETURNED-RUN
```

The output parent must already be a canonical directory disjoint from the
upstream preview, original frames/index and tools/models. `run` creates an
ordinary isolated child; retain the returned recovery path after failure.
`resume` refuses any changed source, preview receipt, pin, settings or provider
plan. No partial directory or preexisting file is adopted. CLI movie `status`
requires complete acceptance; generic `status-v3` can inspect incomplete runs.
Neither readback launches media tools or repairs work.

## Picture, timing and provenance boundaries

The three immutable native inputs are the accepted preview PNG directory,
its exact selected report and the complete original frame index. The worker
revalidates the report against that original index, checks exact PNG inventory
and digests, then copies pictures to a private local sequence. Its report maps
each local ordinal and rational interval back to the unchanged global preview
frame, source pair/hold origin, PNG digest and original presentation interval.
The facade reobserves actual upstream native acceptance before planning and
after execution/readback; a worker report alone does not establish it.

The MP4 starts at local time zero. Its report retains the original global
window; the container does **not** preserve the original source PTS. The encoder
uses the exact rational rate and timebase, one thread and no B frames. It does
not round rates, pad, truncate, resize, infer cuts or change frame membership.
The native observer requires every local interval, count, dimensions and silent
stream inventory to match, then performs full decode and MP4 faststart checks.

Only even-sized, opaque RGB8 PNGs are admitted. This viewing copy explicitly
interprets RGB8 as full range and encodes BT.709 limited-range yuv420p. These
tags describe the chosen viewing conversion; they do not authenticate original
source color, qualify SDR/HDR appearance, establish decoded pixel equality or
replace visual review. Unsupported geometry/clock/color input is refused rather
than resized or normalized silently. The original audio is untouched and
unassessed; this clip has no audio stream.

The window remains at most 120 source frames / ten exact seconds / 240 derived
pictures. Request bounds cap PNG totals at 128 MiB, each PNG at 64 MiB, output at
256 MiB, pixels at 4K-equivalent area, probe capture at 16 MiB, each tool call at
120 seconds, outer stage at five minutes and admitted working storage at 4 GiB.
The preview report must fit 512 KiB; an oversized original cut/mapping report is
refused without truncation. Storage admission includes native snapshots,
private copies, output copies, tools, captures and evidence plus the selected
free reserve. These are finite admission bounds, not measured RAM/GPU quotas or
OS disk reservations. Failed private work and ordinary native evidence remain
available for explicit reconciliation; no automatic cleanup or source mutation.

`preview_movie` is an **intermediate** `video/mp4` with a distinct provider and
report contract. It is not a candidate master, full RIFE index, derived assembly
carrier or final-video handoff. Ordinary integrity acceptance records the new
preview bytes. `native_qualified` stays false, `creative_approval` stays
`not_performed`, and `publication_authority` stays `none`. No synthetic or loose
file establishes acceptance, Identity adoption or publication authority.

The proposed [ADR-0011](architecture/governance/decisions/ADR-0011-serial-rife-derived-clock.md)
records this extension without ratification. Parent #110 remains open for its
remaining evidence; deep qualification remains in flow #88, scheduled last.
