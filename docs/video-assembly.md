# Assemble indexed frames into a master and delivery

Use `video assemble-frames` to turn a complete original-clock PNG workspace
into two separate MP4 files: a lossless audiovisual master and a smaller
H.264 delivery. You supply the original video, the chosen frame directory and
its exact matching index. The source and previous runs remain untouched.

This is the selected [#35](https://github.com/egohygiene/aniflow/issues/35)
source checkpoint. All tests, builds, lint/schema checks, native media execution
and qualification remain deferred under
[#64](https://github.com/egohygiene/aniflow/issues/64). These instructions describe
authored behavior; they are not an executed native qualification receipt.

## Choose the frame set

Start with an existing complete frame workspace. Assembly does not extract
frames, detect repairs, upscale images or infer missing timing.

| Frame choice | Directory and index to supply |
| --- | --- |
| Original indexed PNGs | The complete PNG directory and its `aniflow.rife-frame-index/v1` input index |
| Accepted selective repair | `artifacts/frame-repair/frames` and `artifacts/frame-repair/index.json` beneath that repair run |
| Accepted Upscayl output | `artifacts/upscaled/frames` and `artifacts/upscaled/index.json` beneath that upscale run |

Use the existing run's accepted artifact records to locate those outputs. A
matching filename or a leftover directory alone does not establish acceptance.
Keep the index outside the PNG directory: directory membership must exactly
match the indexed frame names. Supply the original source video throughout,
including when the frames have been repaired or upscaled.

The [selected setup guide](toolchain-media-setup.md) prepares the named repair
or Upscayl configuration from explicit local inventory. Use
[selective repair](frame-repair.md) for reviewed ranges, then optionally pass
its complete output directory and updated index to
[Upscayl recovery](upscayl-recovery.md). The setup mapper selects per-frame
Upscayl policy; longer inputs need an explicit bounded-chunk configuration.
Once that run has accepted its complete collected output, pass the collected
frames and spatial output index to assembly. These are explicit separate
commands and runs. A single automated repair/upscale/assembly recipe is still
follow-up work under #35.

Upscayl's output index keeps the original source geometry and timing separately
from scaled output geometry. Assembly respects both. The `rife-frame-index`
name does not imply interpolation: this checkpoint accepts the original input
clock, not RIFE's derived 2x output index.

## Supported media profile

The selected profile is intentionally closed:

| Boundary | Admitted behavior |
| --- | --- |
| Source container | Observed MP4/MOV-family input with the exact declared selected streams |
| Video clock | Progressive, contiguous zero-origin CFR with every original frame interval retained |
| PNGs | Complete opaque RGB8/SDR frame set; 1–100,000 frames, with exact names and digests |
| Geometry | Square pixels, no rotation; even output width and height of at least 2, within configured pixel bounds |
| Color | Explicitly tagged supported BT.709 SDR source; no inferred HDR or untagged-color conversion |
| Audio | No audio, or the selected original AAC-LC stream with exact packet/configuration/timing verification |
| A/V ends | Existing tolerance of one observed video-frame interval plus one observed audio decode block |
| Native interface | Explicitly pinned FFmpeg and ffprobe 8.0.1 |

AAC sample rate must be an observed integer from 8,000 through 192,000 Hz;
channel count and layout, codec extradata and exact sample-grid packet timing
must be observed. No channel-layout normalization is attempted. Unsupported
stream/packet side data is refused. Video observations require progressive
frames with zero crop and stable geometry, pixel format, aspect and color;
unsupported dynamic transform/color side data is refused.

Source stream selection and acknowledged discards come from the bound temporal
index. Assembly freshly probes the source before full PNG decoding or encoding.
A declared index is not a substitute for current source observations.

VFR, nonzero decoded presentation origins, larger A/V tails, unsupported color
or bit depth, alpha, rotation, unsupported audio and derived RIFE clocks are
refused. The command does not silently trim with `-shortest`, pad a short
stream, stretch audio or normalize offsets. AAC priming packets can have
negative coded timestamps; their skip information is compared explicitly and
does not authorize a shifted decoded presentation origin. See
[exact temporal policy](temporal-correctness.md) for the retained tolerance.
Broader media handling remains parent #35/#110 work.

## Pin the tools

Create an explicit assembly configuration. Replace these paths and digest
placeholders with the trusted local tool identities; the placeholders are not
usable pins:

```json
{
  "schema": "aniflow.video-assembly-configuration/v1",
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
  "delivery_crf": 18,
  "preset": "slow"
}
```

The [configuration schema](../providers/video-assembly/configuration.schema.json)
records the closed settings. Arbitrary FFmpeg arguments, codec substitution,
hardware fallback and audio reencoding are not configuration options. The tools
must expose the selected FFV1/MP4, PNG, AAC, H.264/libx264 and conversion
interfaces. A matching version string alone does not establish that the build
contains them or that it works on this host. Execution compares observed version
interfaces and pinned bytes; source-based interface evidence is recorded in
[toolchain upstreams](toolchain-upstreams.md#ffmpeg-801-assembly-profile-for-35--2026-10-27).
No installation or download occurs.

## Plan, run, resume and inspect

Planning reads and binds the source, index, complete frame set and tools. It
launches no tools and creates no run, so it cannot establish full decode,
container suitability or native availability:

```bash
aniflow --output json video assemble-frames \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/upscale-run/artifacts/upscaled/frames" \
  --frame-index "/absolute/upscale-run/artifacts/upscaled/index.json" \
  --configuration "/absolute/assembly.json" \
  --plan
```

Start by replacing `--plan` with
`--output-directory "/absolute/assembly-runs"`. This is a parent for a new
isolated run, not a destination MP4 filename or an existing source workspace.
The result uses the existing `aniflow.pipeline-run-outcome/v1` contract and
identifies the durable run directory.

After an interruption, keep the same source, frame set, index, tools and
settings. Replace the final option with
`--resume-run-directory "/absolute/assembly-runs/RUN_DIRECTORY"`. The facade
reconstructs the exact plan and current provider authority before asking the
ordinary v3 runtime to resume. Changed inputs or settings require a new run;
files from an incompatible run are not adopted automatically.

Inspect without launching assembly:

```bash
aniflow --output json status-v3 "/absolute/assembly-runs/RUN_DIRECTORY"
```

There is no separate assembly status database or command. Pipeline v3 status,
immutable stage checkpoints, output observations and acceptance records remain
the authority. An accepted master can be reused when only delivery needs to
retry. An interrupted stage retries its whole operation; partial MP4s remain
failure evidence and are never treated as completed checkpoints.

The public library exposes `video_assembly::{AssemblyConfiguration,
AssemblyRequest, AssemblyReport}` and `plan`, `run`, `resume`. The request
binds source video, frames, index, configuration and the provider executable.
Run/resume use the existing cancellation token, `PipelineV3RunProgress`
callback and `PipelineV3RunOutcome`; ordinary `status_v3` remains read-only.
The CLI is a thin adapter with machine command name `video_assemble_frames`.

## Outputs and evidence

| Stage | Planned output IDs | Media behavior |
| --- | --- | --- |
| `assemble_master` | `lossless_master`, `master_evidence` | FFV1 version 3, bgr0, MP4 faststart; original AAC copied when present |
| `encode_delivery` | `h264_delivery`, `delivery_evidence` | H.264/libx264, yuv420p, MP4 faststart; original AAC copied when present |

Delivery reads the accepted master for video and copies audio afresh from the
original source. Both stages compare the copied audio against the source.
The fixed paths beneath the run are
`artifacts/assembly/master/master.mp4` and `master.json`, and
`artifacts/assembly/delivery/delivery.mp4` and `delivery.json`. They are declared
in the plan and reported by the existing run/status records. Keep both media
files and their accepted evidence together. MP4 atom observations require the
movie header before media data; fragmented output is outside this profile.

The master is lossless relative to the admitted RGB8 PNG pixels. Full decode
and per-frame pixel observations check that relation, while exact rational
frame intervals and counts check reconstruction timing. That does not prove
the PNGs were extracted from the original video or that earlier repair or
upscaling preserved creative intent. The report retains
`source_relationship: declared_not_verified`,
`historical_processing: unknown` and `backend_qualification: invocation_observed_not_backend_qualified`.

For audio, verification compares packet payload hashes, exact timing from
integer timestamps and their time bases, codec extradata, stream properties
and priming/skip observations. An audio codec label, file checksum or matching
duration alone is insufficient. The delivery intentionally changes video
pixels through BT.709 conversion, 4:2:0 subsampling and lossy H.264 compression;
it is not labeled pixel-identical to the master. Its audio and temporal
requirements remain separate from those declared video losses.

Provider reports are contemporary observations. They become accepted stage
outputs only after the ordinary runtime's artifact, temporal and validation
gates pass. Process exit zero, a provider report or a direct wrapper invocation
does not by itself complete the run. Native qualification of the authored
implementation is still unrun under #64.

## Storage and execution limits

Defaults are finite admission policies, not predicted output sizes:

| Configuration field | Default |
| --- | ---: |
| `timeout_milliseconds` | 21,600,000 (6 hours per bounded native call) |
| `maximum_capture_bytes` | 67,108,864 (64 MiB) |
| `maximum_source_bytes` | 68,719,476,736 (64 GiB) |
| `maximum_output_bytes` | 137,438,953,472 (128 GiB per media output) |
| `maximum_frame_bytes` | 268,435,456 (256 MiB per PNG) |
| `maximum_total_frame_bytes` | 274,877,906,944 (256 GiB) |
| `maximum_pixels` | 134,217,728 |
| `maximum_working_bytes` | 2,199,023,255,552 (2 TiB) |
| `minimum_free_bytes` | 67,108,864 (64 MiB) |
| `delivery_crf` / `preset` | `18` / `slow` |

The existing temporal validation probe also retains its own bounded timeout
and capture policy; the larger assembly setting does not widen every shared
probe. The outer provider request has its own execution/capture/artifact limits.
Whichever applicable bound is reached first can stop acceptance.

Conservative working-space admission accounts for source/frame/tool copies,
private candidates, retained accepted outputs, reports and capture buffers.
The default 128 GiB output ceiling can therefore require substantial free
space even for a much smaller expected output. Choose explicit tighter bounds
for a known workload when appropriate; an output exceeding them is refused.
A fresh run and each actual provider attempt also observe free space on the
system temporary volume, because the existing temporal validator creates its
own bounded media snapshots there. Fully accepted resume does not reserve a
new native attempt's temporary allocation. A roomy run directory does not
make a full temporary volume usable. Admission is not an operating-system disk reservation or an enforced
RSS/VRAM quota. No in-place replacement, source deletion or automatic cleanup of prior
runs occurs. Keep failure evidence until you have reviewed it.

#35 remains open for the automated complete workflow, broader media and tail
profiles, remaining assembly acceptance and native/platform qualification.
Destination copies (#83), optional provenance proofs (#82), and RIFE-derived
clock/audio handling (#110) retain their separate ownership. This checkpoint
does not publish an immutable release.
