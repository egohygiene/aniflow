# Start a video with managed AI tools

The managed source entry resolves selected Upscayl binary/model files before
lowering to the existing exact source recipe. Selected RIFE uses the same store.
Actual runs acquire missing curated packages; `--plan` and managed doctor never
download, create a store or run inference. Existing complete, explicitly pinned
v1 recipe/RIFE configurations keep their original behavior.

Install compatible **FFmpeg and ffprobe 8.0.1** on PATH first, or supply their
paths below. This checkpoint manages Upscayl/RIFE, not FFmpeg or system GPU
drivers. Native admission still checks the exact FFmpeg version and supported
source media; declaring a pin during setup does not claim a successful probe.
See [source recipe restrictions](music-video-recipe.md) before a long run.

```bash
aniflow video music-video --source-video input.mp4 --plan
aniflow video music-video --source-video input.mp4 --output-directory ./runs
```

The omitted configuration selects source extraction, import without repair,
Upscayl 4x with the anime model, 16-frame resumable chunks, and lossless-master
plus H.264 delivery assembly. RIFE stays disabled. The default source selection
is input-global video stream 0 and audio stream 1. Change these to match your
source (`--video-stream`, `--audio-stream`); use `--no-audio` for silent input
and `--discard-streams 2,3` to acknowledge unwanted streams. Supported sources
and original audio retain their existing exact timing/format gates.

The first plan reports the source file digest, selected packages, expected
network/install bytes, supported platform notes, requested scale, finite bounds
and pending geometry/effects. With installed dependencies it can produce the
ordinary exact source plan. Neither form probes source media or proves native
compatibility, free-space admission or successful output. Native extraction
makes later frame/resource observations; downstream plans remain conditional.

## Configure only what you need

Use [the managed source example](../examples/managed/music-video.json):

```bash
aniflow video music-video --source-video input.mp4 \
  --configuration examples/managed/music-video.json --plan
```

All three new configuration schemas reject unknown fields:

- `aniflow.managed-music-video/v1`: source recipe preset, local FFmpeg paths,
  stream selection, finite resource policy, tools and optional interpolation.
- `aniflow.managed-rife/v1`: selected RIFE tools, observed host memory and device.
- `aniflow.managed-options/v1`: a user defaults document with an `options` object.

For each dependency role, flags override project configuration, which overrides
an explicitly selected user defaults file, followed by managed defaults. No
ambient user settings file is silently read. Select user settings with
`--managed-user-configuration examples/managed/user-options.json`.
An upper-layer model directory overrides both lower-layer model file paths;
an upper-layer individual parameter/weights path overrides only that role.
An active managed package is the default when no explicit version selects
another package; multiple applicable active models require a model choice.
A corrupt active package is reported, never silently replaced by another model.

Common source options:

```bash
aniflow video music-video --source-video input.mp4 --output-directory ./runs \
  --ffmpeg-executable /absolute/bin/ffmpeg \
  --ffprobe-executable /absolute/bin/ffprobe \
  --upscayl-model realesrgan-x4plus --output-scale 2 \
  --managed-cache-directory /absolute/cache/aniflow-managed
```

The general model is an explicit choice. Model suitability and video temporal
quality remain an operator review; no model is automatically substituted.
Upscayl requires a working Vulkan GPU (device 0 in this bounded preset) and has
no automatic CPU fallback. RIFE can explicitly use CPU with device `-1`.
Supported curated artifacts currently cover macOS x86_64/ARM64 and glibc Linux
x86_64. Upscayl requires macOS 13+ on x86_64, macOS 14+ on ARM64, or glibc 2.35+;
RIFE requires macOS 11+ or the catalog's Linux runtime prerequisites. macOS
minimums are checked from the local system plist before acquisition. Native
Mac/GPU and broader release matrix qualification remain pending.

The default cache is `~/Library/Caches/aniflow/managed` on macOS and
`${XDG_CACHE_HOME:-~/.cache}/aniflow/managed` on Linux. Set
`ANIFLOW_MANAGED_CACHE` or `--managed-cache-directory` to change it. Keep the
cache disjoint from source/config/run trees. Package identities and installation
roots are frozen into native configuration and resume identity.

## Reuse a local installation

Executable and model roles can be mixed independently. An explicit executable
requires its declared semantic version; it is hashed without an invented probe.
Bad explicit paths fail before acquisition and are never repaired or replaced.

```bash
aniflow video music-video --source-video input.mp4 --output-directory ./runs \
  --upscayl-executable /absolute/tools/upscayl-bin --upscayl-version 2.15.0
```

That uses the manual binary with managed models. To keep a local model too, add
`--upscayl-model-directory /absolute/models --upscayl-model digital-art-4x`.
Use `--upscayl-model-param` and `--upscayl-model-weights` for separate exact file
paths. The anime model identifier is `realesr-animevideov3`; its 4x files are
`realesr-animevideov3-x4.param` and `.bin`. Manual files remain outside lifecycle
ownership. Complete manual overrides work without a matching catalog entry or
managed installation. Unsupported native profiles still refuse execution.

Managed upstream release IDs such as `20251207-174704` are artifact identities,
not an observed semantic binary version. The legacy native `version` field
encodes these as `0.0.0+upscayl.release.20251207-174704` (similarly for RIFE),
while the exact upstream release/source, artifact digests and catalog revision
remain separately pinned in the managed receipt/runtime identity.

## Select RIFE explicitly

For a source recipe, use `--interpolate` plus operator-observed total/available
RAM, or add a managed `interpolation` object to the project configuration.
Merely specifying a RIFE model/path does not enable interpolation or download it.

```bash
aniflow video music-video --source-video input.mp4 --output-directory ./runs \
  --interpolate --rife-total-memory-mib 16384 \
  --rife-available-memory-mib 8192 --rife-device-id -1
```

For existing indexed RGB PNGs, the standalone and bounded-preview RIFE entries
also resolve managed defaults before the existing native configuration:

```bash
aniflow video rife preview-plan --frames-directory /absolute/frames \
  --frame-index /absolute/index.json --first-source-ordinal 0 --end-source-ordinal 2 \
  --rife-total-memory-mib 16384 --rife-available-memory-mib 8192
aniflow video rife run --frames-directory /absolute/frames \
  --frame-index /absolute/index.json --output-directory ./runs \
  --rife-total-memory-mib 16384 --rife-available-memory-mib 8192 --rife-device-id -1
```

The existing RIFE profile is midpoint 2x, serial, model `rife-v4.6` with exact
`flownet.param`/`.bin`; a manual directory must keep that final directory name.
Corresponding overrides are `--rife-executable`, `--rife-version`,
`--rife-model-directory`, `--rife-model-param`, `--rife-model-weights`, and
`--rife-model`. Valid observed memory is required before actual acquisition;
these values are admission declarations, not memory reservations.

## Offline and exact resume

`--offline` and `--no-download` prohibit transport. Already verified cached
artifacts can be assembled locally; a missing artifact fails with a setup
message. They do not install an alternative provider/model or enable a cloud
service. [Managed lifecycle commands](managed-tool-commands.md) expose doctor,
plan, install, repair, activation, rollback, uninstall and explicit retention.

Resume a managed source run without the original managed defaults document:

```bash
aniflow video music-video --source-video input.mp4 \
  --resume-run-directory /absolute/runs/recipe --offline
aniflow video rife resume --frames-directory /absolute/frames \
  --frame-index /absolute/index.json --run-directory /absolute/runs/rife --offline
```

Resume reads retained exact native configuration; it does not select today's
catalog defaults or acquire replacements. Omit managed settings/model/cache
flags and the managed `--configuration`. Explicit legacy pinned configuration
continues to be supported. Changed source/tool/model/configuration/path bytes
still refuse stale reuse. RIFE retains its exact managed configuration beside
the immutable ordinary run plan and verifies their binding on readback.

Before consumption, selected packages receive a conservative durable
`configuration-<sha256>` retention reference. It protects successful, failed and
resumable work from uninstall/repair; it does not assert run acceptance. Keep it
until all runs with that configuration have been removed/exported, then use the
explicit lifecycle release command. No automatic completion-based cleanup occurs.

## Bound a real video before a long upscale

The preset's caps are worst-case admission limits, not measurements of final PNG
sizes or a promise that a large video fits disk. Set justified finite values
with the source config or `--maximum-working-bytes`, `--minimum-free-bytes`,
`--maximum-source-bytes`, `--maximum-video-output-bytes`, `--maximum-capture-bytes`,
`--maximum-output-frame-bytes`, `--maximum-input-frame-bytes`,
`--maximum-total-frame-bytes`, `--maximum-frames`, `--frames-per-chunk` and
`--chunk-timeout-milliseconds`. Exceeding a frame cap fails safely.
The managed preset caps original source input at 1 GiB, each generated video at 512 MiB and each captured tool
stream at 16 MiB, making short runs bounded; raise the output cap explicitly for
a longer lossless master after checking the complete storage report. The native
validator separately checks its temporary volume using twice the larger of the
source/output caps plus 192 MiB of report space; placing runs on another disk
does not bypass that independent guard. Do not lower
a cap just to bypass admission. Use the measured indexed recovery
resource report once accepted extraction supplies actual frames. Native and
visual qualification on the target Mac/GPU remain separate review work.
