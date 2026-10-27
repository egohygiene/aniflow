# Resume bounded per-frame Upscayl work

Use `video upscale-frames` to process a complete declared PNG workspace one
frame at a time, with an immutable checkpoint after each accepted frame. An
interruption leaves completed frames available for compatible run-local reuse.
An explicit resume retries missing, failed or invalid frame work once; it does
not infer success from a filename, size or process exit code.

This is an authored [#37](https://github.com/egohygiene/aniflow/issues/37)
checkpoint. Tests, compiler/build, lint/format/schema checks, native execution
and qualification remain deferred under [#64](https://github.com/egohygiene/aniflow/issues/64).

## Choose the bounded mode

The first recovery profile is serial **per-frame** processing, capped at 1,024
frames per run. Each frame launches a fresh Upscayl process, so one process does
not need to remain alive for the complete workspace. There is one attempt per
explicit run or resume call and no background retry loop or automatic settings
change. The existing Pipeline v2 `upscayl_ncnn` directory-batch adapter remains
available separately. Bounded multi-frame chunks and their native qualification
remain future #37 work.

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
  "output_scale": 2,
  "tile_size": 32,
  "gpu_id": 0,
  "threads": "1:1:1",
  "tta": false,
  "maximum_output_frame_bytes": 67108864,
  "maximum_output_pixels": 134217728,
  "maximum_working_bytes": 274877906944,
  "minimum_free_bytes": 67108864
}
```

`native_scale` is the model's declared scale, passed as `-z`; this initial
profile admits native 4× models. `output_scale` separately requests 2×, 3× or
4× dimensions through `-s`. A native 4× model requested at 2× must produce
exactly `input_width × 2` by `input_height × 2`, and that output must fully
decode before its frame checkpoint can be accepted. The source-derived argv
contract does not establish that a particular installed model works.
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

Arguments are passed directly, without a shell. The model files are copied into
a short private relative `models` directory. The special
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
setting requires a new run. It never mixes settings by silently adopting old
PNG files.

Each frame has an independent stage. A bad or missing frame invalidates its
own accepted work and the final collector, without making unrelated completed
frames depend on it. The collector runs only after every required frame has
accepted output. The normal exclusive run-writer lock remains authoritative;
concurrent writers are refused and abandoned ownership requires explicit
reconciliation under the existing runtime policy.

The library's `upscayl_recovery` facade provides the same plan, run and resume
operations. Frame progress reports total, completed, reused, pending and failed
counts separately from overall runtime progress. Any remaining-time estimate
comes from completed-frame durations measured during the current attempt;
reused frames do not contribute throughput samples. `completed` includes
`reused`; `pending` is total minus completed, so failed work is also pending.
An estimate is unavailable before a newly completed frame or while a frame
failure is present. Zero pending frames can precede final collector acceptance.
Progress is presentation, not checkpoint authority. The CLI prints these
observations to stderr in human mode; machine mode keeps the final plan/run
outcome and does not introduce a progress stream. Machine command spelling is
`video_upscale_frames`.

## Output and evidence

For each frame the provider privately snapshots and fully decodes the input,
runs the pinned Upscayl process, checks exact PNG membership and geometry, and
fully decodes the result through the pinned FFmpeg RGB24 helper. Structure and
CRC checks supplement decoding. Source files remain unchanged; output is
published through the existing runtime's confined, fresh-inode acceptance path.

The [per-frame report](contracts/upscayl-frame-report-v1.schema.json) binds
source digest/index/time, input identity, exact provider configuration, binary,
decoder and both model files. It records output dimensions, output bytes,
decoded input/output hashes and measured processor elapsed time. A zero exit
with an invalid or missing output does not establish a frame checkpoint.

The collector produces a complete separate frame directory, a
[spatial output index](contracts/upscayl-output-index-v1.schema.json), and a
[recovery report](contracts/upscayl-recovery-report-v1.schema.json). It checks
every ordinal, name, frame digest and report against the same-run accepted
frame dependencies. Original rational times and cut boundaries are retained.

| Artifact | Relative path |
| --- | --- |
| Accepted per-frame PNG and report | `artifacts/upscayl/000000/frame.png` and `frame.json`, with the six-digit zero-based ordinal |
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

The 1,024-frame cap also bounds the number of immutable stage checkpoints and
append-only run manifests. The existing runtime retains a stage-record set in
each manifest revision, so metadata can grow quadratically with frame count.
Working-data admission includes this overhead, source/input data, retained
frame and collector outputs, bounded reports, tools, models and private work.
The default working-data budget is 256 GiB, with a 64 MiB free-space reserve.
Shared storage observations are rechecked before frame and collector work.
Admission is an estimate, not a reservation, quota or OS process sandbox.

Per-invocation tool timeout and capture limits remain explicit. Failed or
interrupted candidates and the existing provider/runtime evidence remain
available under their owned retention rules; compatible accepted frames are
not discarded because a later frame fails. Resume performs one new attempt
for work that needs execution. Termination, cancellation, timeout, storage
failure and invalid output do not become an unbounded retry loop. SIGKILL alone
does not diagnose an out-of-memory failure.

Review tile boundaries and temporal consistency whenever separately processed
results are compared or deliberately mixed. This checkpoint does not qualify
Apple Silicon, Vulkan/NCNN, particular models or perceptual quality. It does not
download dependencies, change power settings, add a global cache, reconstruct
video, remux audio, or create a second process/checkpoint runtime.

Bounded chunk mode, explicit historical-output adoption, the full injected
failure matrix, native platform/model qualification and downstream video
assembly remain open. The known alignment cancellation/start-marker failure
is unchanged and its cause remains unproven. See the
[checkpoint handoff](validation/aniflow-37-checkpoint.md) for deferred work.
