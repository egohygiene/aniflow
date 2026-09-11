# Short-segment workflows

Aniflow owns temporal decomposition and reconstruction through the stable
capability IDs `media.video.segment/v1` and `media.video.reconstruct/v1`.
Segment duration is explicit, measured in milliseconds, and must be between 1
and 29,999 milliseconds.

## Timing modes

| Mode | Streams | Boundary declaration | Intended use |
| --- | --- | --- | --- |
| `stream-copy` | Copies all source streams | `keyframe_aligned` | Fast, lossless cuts when exact requested boundaries are not required |
| `transcode-h264-aac` | First video and optional first audio stream | `frame_accurate` | Predictable MP4 segments at requested boundaries |

Before stream-copy execution, Aniflow inspects real source keyframes and records
the selected start and end timestamps in the plan. If keyframe spacing would
create a segment of 30 seconds or longer, planning fails with guidance to use
the transcode mode. The requested duration is therefore an explicit target;
the manifest never disguises keyframe-aligned boundaries as exact cuts.

The transcode preset rejects sources containing subtitles instead of silently
discarding them. Stream copy retains source codecs and streams but can start on
the nearest usable keyframe. Neither mode claims more accuracy than it can
establish.

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
digest. Publication happens only after every segment exists and matches its
checksum.

## Resume and cleanup

Each segment is written to a temporary sibling and atomically renamed. On
failure, timeout, or cancellation, the temporary file is removed while already
accepted segments and the checkpoint remain. Resume verifies the source and
plan digests, then retains only the contiguous checksum-valid prefix. Missing,
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
Frame-accurate mode allows one frame (at least 50 ms) of duration difference;
keyframe-aligned mode visibly allows one requested segment plus one frame.

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

Flow may invoke this facade or the versioned JSON CLI envelope. Renderflow may
consume a complete reconstructed video or provide whole-file transcoding, but
temporal decomposition and reconstruction remain Aniflow responsibilities.
