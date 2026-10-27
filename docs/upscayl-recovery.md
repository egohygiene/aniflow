# Resume bounded Upscayl work

Use `video upscale-frames` to process a complete declared PNG workspace with
immutable checkpoints. Choose per-frame processing for individual recovery or
bounded native chunks for longer videos. An interruption leaves compatible
accepted work available for run-local reuse. Explicit resume attempts pending
work once; a filename, size or process exit code alone never proves success.

This is an authored [#37](https://github.com/egohygiene/aniflow/issues/37)
checkpoint. Tests, compiler/build, lint/format/schema checks, native execution
and qualification remain deferred under [#64](https://github.com/egohygiene/aniflow/issues/64).

## Choose the bounded mode

The configuration selects the recovery unit explicitly:

| Policy | Native process and checkpoint | Bounds |
| --- | --- | --- |
| `per_frame` (default) | One process and one accepted checkpoint per frame | Up to 1,024 frames |
| `bounded_chunk` | One native directory process and one accepted checkpoint per chunk | 2–256 frames per chunk, up to 1,024 chunks and 100,000 total frames |

Index, plan and storage admission can impose tighter bounds. For a 5,602-frame
video, a chunk size of 64 creates 88 chunk stages plus the final collector.
The final chunk contains the remaining 34 frames. Chunk mode avoids expanding
that video into thousands of independent runtime stages.

Add the `execution` field to the configuration to select chunks:

```json
{
  "execution": {
    "mode": "bounded_chunk",
    "frames_per_chunk": 64,
    "timeout_milliseconds": 1800000,
    "maximum_tool_output_bytes": 16777216
  }
}
```

Omit `execution`, or set it to `{ "mode": "per_frame" }`, to retain per-frame
processing. Chunk size is explicit; the native chunk timeout defaults to
30 minutes and may be set from 1 millisecond through 6 hours. Each FFmpeg
decode still uses the import configuration's separate limit, at most 120 seconds.
The existing outer provider timeout also applies to the complete invocation,
including decode and copying; its request default remains 6 hours. These are
upper time bounds, not throughput or native-resource guarantees.

`maximum_tool_output_bytes` separately caps captured native stdout and stderr.
It defaults to 16 MiB per stream, with an explicit range from 1 KiB through
64 MiB. The larger chunk-specific budget accommodates the pinned tool's tile
progress output; it does not widen FFmpeg/audio capture limits. Exceeding the
selected cap fails the chunk with bounded capture evidence. Changing the cap,
like changing the timeout or chunk size, requires a new exact run.

Completed chunks are reused as complete units. If a chunk fails, is interrupted
or loses an accepted output, the entire chunk is pending and is retried once on
explicit resume. Private partial outputs remain failure evidence; they are not
silently adopted as frame checkpoints. Smaller chunks reduce repeated work
following interruption but require more checkpoints. There is no background
retry loop or automatic settings change. The existing Pipeline v2 `upscayl_ncnn`
native directory-batch adapter remains available separately.

Supply the original source video, its complete `aniflow.rife-frame-index/v1`,
and the corresponding PNG directory. Keep the index outside that directory;
directory membership must exactly match the index. The admitted input is the
existing RGB8 SDR, square-pixel, unrotated, zero-origin CFR profile described in
the [import guide](frame-import.md). This command does not extract frames or
infer missing timing.

## Pin the executable and model

The [configuration schema](../providers/upscayl-recovery/configuration.schema.json)
requires explicit Upscayl and FFmpeg executable pins plus the exact model
`.param` and `.bin` files. This is a template; replace the paths, declared
versions and digest placeholders with the actual trusted local dependencies:

```json
{
  "schema": "aniflow.upscayl-recovery-configuration/v1",
  "import": {
    "schema": "aniflow.frame-import-configuration/v1",
    "ffmpeg": {
      "executable": "/absolute/bin/ffmpeg",
      "version": "8.0.0",
      "sha256": "REPLACE_WITH_OBSERVED_FFMPEG_SHA256"
    }
  },
  "upscayl": {
    "executable": "/absolute/bin/upscayl-bin",
    "version": "2.15.0",
    "sha256": "REPLACE_WITH_OBSERVED_UPSCAYL_SHA256"
  },
  "model": {
    "name": "realesr-animevideov3",
    "native_scale": 4,
    "param": {
      "path": "/absolute/models/realesr-animevideov3-x4.param",
      "sha256": "REPLACE_WITH_OBSERVED_PARAM_SHA256"
    },
    "weights": {
      "path": "/absolute/models/realesr-animevideov3-x4.bin",
      "sha256": "REPLACE_WITH_OBSERVED_WEIGHTS_SHA256"
    }
  },
  "execution": {
    "mode": "bounded_chunk",
    "frames_per_chunk": 64,
    "timeout_milliseconds": 1800000,
    "maximum_tool_output_bytes": 16777216
  },
  "output_scale": 2,
  "tile_size": 32,
  "gpu_id": 0,
  "threads": "1:1:1",
  "tta": false,
  "maximum_output_frame_bytes": 8388608,
  "maximum_output_pixels": 134217728,
  "maximum_working_bytes": 274877906944,
  "minimum_free_bytes": 67108864
}
```

This example selects an 8 MiB maximum output PNG size to keep admission for a
longer video below the default 256 GiB working-data policy when the other
inputs and overhead fit. It does not predict compression: any output above
8 MiB is refused. Choose an explicit larger bound and working-data budget when
required; see [storage admission](#storage-interruption-and-remaining-work).

`native_scale` is the model's declared scale, passed as `-z`; this initial
profile admits native 4× models. `output_scale` separately requests 2×, 3× or
4× dimensions through `-s`. A native 4× model requested at 2× must produce
exactly `input_width × 2` by `input_height × 2`, and that output must fully
decode before its frame or chunk checkpoint can be accepted. The source-derived
argv contract does not establish that a particular installed model works.
`maximum_output_pixels` bounds both the native 4× intermediate and the requested
output geometry, so requesting 2× does not bypass the larger intermediate's
pixel admission. This bound is not an enforced GPU-memory quota.

The adapter follows the pinned
[Upscayl NCNN interface](toolchain-upstreams.md#interface-facts-that-affect-profiles)
at revision `0beb39028a0ddd83250e845b4c3333c0675e3b97`. Its flags are short-form
only. A per-frame invocation has this shape inside private work:

```text
-i input.png -o output.png -m models -n realesr-animevideov3 -z 4 -s 2 -t 32 -g 0 -j 1:1:1 -f png
```

For bounded chunks, `-i input` and `-o output` select private directories that
already exist. Only that chunk's exact PNG members are supplied, and the result
must contain exactly those names. Each native process exits before the next
chunk starts. Arguments are passed directly, without a shell. Model files are
copied into a short private relative `models` directory. The special
`realesr-animevideov3` model uses `-x4.param` and `-x4.bin` filenames there;
other supported model names use unsuffixed `.param`/`.bin` companions. Model
names that would make upstream override the declared native scale are refused.
The adapter neither discovers models nor guesses a CPU fallback.

Tile size is explicit, with 32 as the conservative default; automatic tile
selection is not admitted. One GPU is selected explicitly, and load/process/save
workers are fixed at `1:1:1`. `tta: true` adds the explicit `-x` option. These
settings, both model-file digests and executable pins participate in checkpoint
compatibility. The pinned upstream source requires GPU/Vulkan execution. No
enforced GPU-memory quota or observed device suitability is claimed.
Planning does not probe GPU availability; an explicit execution attempt can
still fail because the selected native device or backend is unavailable.

Upscayl's configured version is a caller declaration. The pinned interface has
no exact version probe: `-v` means verbose. Binary bytes and model bytes are
observed, while driver/backend qualification remains unestablished. FFmpeg
retains its strict decoder-version observation. Reusing an already observed,
compatible frame does not claim bitwise reproducibility across GPUs or drivers.

## Plan, run and resume

Planning binds the source video, complete index, every original frame and the
explicit dependencies without launching tools or creating a run:

```bash
aniflow --output json video upscale-frames \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/frames" \
  --frame-index "/absolute/frame-index.json" \
  --configuration "/absolute/upscayl-recovery.json" \
  --plan
```

To start, replace `--plan` with
`--output-directory "/absolute/upscale-runs"`. That directory is a parent for
a new isolated run, not an existing source or output-frame workspace. The
result uses the existing `aniflow.pipeline-run-outcome/v1` contract and reports
the durable run directory. Keep that directory and its owned state intact.

To resume, use the same source, frames, index and settings, replacing the final
option with `--resume-run-directory "/absolute/upscale-runs/RUN_DIRECTORY"`.
The facade reconstructs the same plan and current provider authority before
calling the existing v3 resume path. A changed source, frame, model, binary or
setting, including execution mode, chunk size, timeout or native capture bound,
requires a new run.
It never mixes settings by silently adopting old PNG files.

Generic v3 run/resume also freshly checks this capability's component files.
The complete-set collector requires the exact same-run frame or chunk producers
and dependencies; source-bound loose PNGs and reports cannot stand in for them.
Low-level frame operations carry declared index/time mapping until the collector
compares it with the complete original index. Calling a native wrapper directly
does not establish runtime acceptance.

Each processing unit has an independent stage. A bad or missing frame
invalidates its own per-frame checkpoint, or its whole chunk checkpoint, and
the final collector. Unrelated completed units do not depend on it. The
collector runs only after every required frame belongs to accepted output.
The normal exclusive run-writer lock remains authoritative;
concurrent writers are refused and abandoned ownership requires explicit
reconciliation under the existing runtime policy.

The library's `upscayl_recovery` facade provides the same plan, run and resume
operations. Frame progress reports total, completed, reused, pending and failed
counts separately from overall runtime progress. Any remaining-time estimate
comes from newly accepted processing-unit durations measured during the current
attempt, weighted by their frame counts; reused frames do not contribute
throughput samples. In chunk mode counts advance only when the whole chunk is
accepted or reused, and the full failed chunk remains pending. No intra-chunk
progress or partial checkpoint authority is inferred from native output.
`completed` includes `reused`; `pending` is total minus completed, so failed work is also pending.
An estimate is unavailable before newly completed work or while a processing
unit failure is present. Zero pending frames can precede final collector
acceptance.
Progress is presentation, not checkpoint authority. The CLI prints these
observations to stderr in human mode; machine mode keeps the final plan/run
outcome and does not introduce a progress stream. Machine command spelling is
`video_upscale_frames`.

## Output and evidence

For each frame the provider privately snapshots and fully decodes the input,
runs the pinned Upscayl process for its frame or chunk, checks exact PNG
membership and geometry, and fully decodes the result through the pinned FFmpeg RGB24 helper. Structure and
CRC checks supplement decoding. Source files remain unchanged; output is
published through the existing runtime's confined, fresh-inode acceptance path.

The [per-frame report](contracts/upscayl-frame-report-v1.schema.json) binds
source digest/index/time, input identity, exact provider configuration, binary,
decoder and both model files. It records output dimensions, output bytes,
decoded input/output hashes and measured processor elapsed time. A zero exit
with an invalid or missing output does not establish a frame checkpoint.

Chunk mode instead writes a [chunk report](contracts/upscayl-chunk-report-v1.schema.json)
with exact contiguous frame specifications and decoded observations for every
member. Processor elapsed time belongs to the complete native invocation; it
is not misrepresented as a separately measured duration for each frame. The
chunk directory and report must both be accepted by the existing runtime.

The collector produces a complete separate frame directory, a
[spatial output index](contracts/upscayl-output-index-v1.schema.json), and a
[per-frame recovery report](contracts/upscayl-recovery-report-v1.schema.json) or
[chunk recovery report](contracts/upscayl-chunk-recovery-report-v1.schema.json),
according to the selected policy. It checks every ordinal, name, frame digest
and report against the same-run accepted producer dependencies. Original
rational times and cut boundaries are retained.

| Artifact | Relative path |
| --- | --- |
| Accepted per-frame PNG and report | `artifacts/upscayl/000000/frame.png` and `frame.json`, with the six-digit zero-based ordinal |
| Accepted chunk directory and report | `artifacts/upscayl-chunks/000000/frames` and `chunk.json`, with the six-digit zero-based chunk ordinal |
| Complete upscaled PNG directory | `artifacts/upscaled/frames` |
| Spatial output index | `artifacts/upscaled/index.json` |
| Collected observations | `artifacts/upscaled/upscale.json` |

The spatial index embeds the original `source_index` unchanged, records scaled
output geometry, and maps the output PNG digests. It does not rewrite the
source video's observed dimensions to pretend the original video was upscaled.
This is a distinct output contract, not an automatically interchangeable RIFE
input index; downstream assembly and consumer integration remain later work.

The source-to-frame relationship remains `declared_not_verified`, historical
processing remains `unknown`, and `backend_qualification` is `unqualified`.
Schema validity, a loose report or an existing PNG cannot grant
accepted-stage authority. Imported outputs from older native batches still
need a separate explicit import decision and exact historical evidence.

## Storage, interruption and remaining work

Chunking bounds the number of processing stages, not the size of the complete
video. The existing runtime repeats stage records in append-only run manifests,
so that component grows quadratically with processing-unit count. The complete
source plan, operations and reports separately retain frame-level mappings.
The conservative metadata admission estimate accounts for both counts:
`(units + 1) × (frames + units + 1) × 16 KiB`. It is not a claim that every
manifest contains the source inventory. Per-frame metadata remains bounded by
its 1,024-frame limit. Plan documents remain bounded at 64 MiB.

Working-data admission includes source/input data, retained frame and collector
copies, bounded reports, tools, models, metadata and private work. The default
working-data budget is 256 GiB with a 64 MiB free-space reserve. Shared storage
observations are rechecked before processing and collection. Admission is an
estimate, not a reservation, quota or OS process sandbox. Source frames and
accepted intermediate outputs are retained; chunk mode does not provide
in-place replacement or zero-additional-storage processing.

Admission uses the configured maximum PNG size, not a compression prediction.
For 5,602 frames, three retained copies at the default 64 MiB bound alone need
about 1.03 TiB, exceeding the default budget. The example's 8 MiB bound reduces
that component to about 131.3 GiB; input data, metadata and scratch still count.
An output that exceeds the chosen maximum is refused. A larger explicit
working-data budget also requires enough observed free storage. Complete-set
artifact-count limits account for the selected frame total rather than the old
1,024-frame ceiling; byte and caller-selected execution limits still apply.

Fresh runs admit the complete conservative working-data estimate. Resume checks
space for bounded new metadata and the free-space reserve; it does not charge
all retained frames a second full-run allocation. Each attempted frame, chunk
or collector then checks its own immediate storage need. This does not infer
checkpoint validity from files found on disk or promise enough space for every
future manifest revision or retry.

Per-invocation tool timeout and capture limits remain explicit. Failed or
interrupted candidates and the existing provider/runtime evidence remain
available under their owned retention rules; compatible accepted units are
not discarded because a later unit fails. Resume performs one new attempt
for work that needs execution. Termination, cancellation, timeout, storage
failure and invalid output do not become an unbounded retry loop. SIGKILL alone
does not diagnose an out-of-memory failure.

Review tile boundaries and temporal consistency whenever separately processed
results are compared or deliberately mixed. This checkpoint does not qualify
Apple Silicon, Vulkan/NCNN, particular models or perceptual quality. It does not
download dependencies, change power settings, add a global cache, reconstruct
video, remux audio, or create a second process/checkpoint runtime.

Explicit historical-output adoption, the full injected failure matrix, native
platform/model qualification and downstream video assembly remain open. The known alignment cancellation/start-marker failure
is unchanged and its cause remains unproven. See the
[chunk checkpoint handoff](validation/aniflow-37-chunks-checkpoint.md) for
deferred work; the [per-frame handoff](validation/aniflow-37-checkpoint.md)
retains its original dated observations.
