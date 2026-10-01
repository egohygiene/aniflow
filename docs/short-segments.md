# Short-segment workflows

The #32 [exact temporal contract](temporal-correctness.md) supersedes the older
first-stream/approximate-duration profile. New plans, manifests and reconstruction
reports use v2. Implementation qualification is intentionally deferred to #64;
old v1 state cannot be silently resumed under the new policy.

aniflow owns temporal decomposition and reconstruction through the stable
capability IDs `media.video.segment/v1` and `media.video.reconstruct/v1`.
Segment duration is explicit, measured in milliseconds, and must be between 1
and 29,999 milliseconds.

## Timing modes

| Mode | Streams | Boundary declaration | Intended use |
| --- | --- | --- | --- |
| `stream-copy` | Explicitly selected video and optional audio | `keyframe_aligned` | Source keyframes with compatible non-reordered packet clocks and aligned audio boundaries |
| `transcode-h264-aac` | Explicitly selected video and optional audio | `frame_accurate` | Exact source frame intervals and declared audio sample-boundary rounding |

Before stream-copy execution, aniflow inspects real source keyframes and records
the selected start and end timestamps in the plan. If keyframe spacing would
create a segment of 30 seconds or longer, planning fails with guidance to use
the transcode mode. The requested duration is therefore an explicit target;
the manifest never disguises keyframe-aligned boundaries as exact cuts.

Every omitted stream, including subtitles, requires explicit discard
acknowledgement. Ambiguous moving-video/audio selection is refused. Both modes
require the supported contiguous CFR, zero-presentation-origin profile; VFR and
offsets remain visible observations and are refused for processing. Neither mode
silently chooses streams or normalizes timing.

## Durable layout

```text
run-directory/
├── segments/                 generated segment files
├── state/
│   ├── request.json          exact resumable request
│   ├── plan.json             inspected source and normalized intent
│   └── checkpoint.json       atomically updated verified prefix
├── logs/                     bounded FFmpeg diagnostics
└── delivery/
    ├── segment-manifest.json immutable completed manifest
    └── reconstruction-report.json
```

The manifest records source identity, timing mode, requested boundaries, actual
durations, ordered paths, byte sizes, checksums, FFmpeg identity, and the plan
digest, exact source stream windows and independently observed output timing.
Publication requires source intervals, frame cardinality and clocks to satisfy
the declared temporal policy as well as the existing checksum checks.

## Resume and cleanup

Each segment is written to a temporary sibling and atomically renamed. On
failure, timeout, or cancellation, the temporary file is removed while already
accepted segments and the checkpoint remain. Resume verifies the source and
plan digests and exact stream/timestamp observations, then retains only the
contiguous checksum-and-timing-valid prefix. Missing,
changed, or reordered segments are regenerated from the first invalid entry.

An already-complete workspace is not overwritten by a new run. Reconstruction
also refuses to replace an existing destination.

## Resource and process boundary

FFmpeg and FFprobe run directly with argv; no command is interpreted by a
shell. Standard output and standard error are drained but only 4 MiB of each is
retained. Each media child has an explicit timeout, and the Rust API exposes a
thread-safe cancellation token that terminates the active child. Segments run
sequentially, bounding child concurrency at one.

## Reconstruction validation

Reconstruction first verifies path shape, order, and every segment checksum.
It then uses FFmpeg's concat demuxer with generated safe relative paths. The
result is inspected independently with FFprobe and receives a SHA-256 digest.
Reconstruction restores the original exact CFR video grid and selected source
audio. Every video interval and frame count must match exactly. A/V origins must
match exactly; the declared end tolerance is one observed video frame plus one
observed audio decode block. The original source must remain unchanged and
available; per-segment codec padding does not define the reconstruction clock.

## Rust facade

```rust,no_run
use aniflow::{
    CancellationToken, Result, SegmentMode, SegmentRequest,
    reconstruct_segments, segment_with_progress,
};

fn split_and_join() -> Result<()> {
    let request = SegmentRequest::new(
        "source.mp4",
        ".aniflow/segments/demo",
        10_000,
        SegmentMode::TranscodeH264Aac,
    );
    let cancellation = CancellationToken::default();
    let outcome = segment_with_progress(request, &cancellation, |_| {})?;
    reconstruct_segments(&outcome.run_directory, "reconstructed.mp4")?;
    Ok(())
}
```

flow may invoke this facade or the versioned JSON CLI envelope. renderflow may
consume a complete reconstructed video or provide whole-file transcoding, but
temporal decomposition and reconstruction remain aniflow responsibilities.
