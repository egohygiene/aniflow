# Serial RIFE frame interpolation

The first [#110](https://github.com/egohygiene/aniflow/issues/110) checkpoint,
[#114](https://github.com/egohygiene/aniflow/issues/114), authors an optional
`rife-ncnn-vulkan` wrapper and a read-only resource planner. RIFE is the frame
interpolation tool. This slice accepts already-extracted PNG frames with explicit
temporal evidence and emits an indexed frame directory. It does not yet accept
an MP4 directly, assemble a final video, remux audio, detect scene cuts, or provide
general target-frame-rate conversion.

Tests, builds, formatting, schema checks, native model runs and host qualification
remain unrun under [#64](https://github.com/egohygiene/aniflow/issues/64). The
behavior described here is authored implementation, not an observed guarantee
that a particular Mac, Linux host, Vulkan device or model run is safe or usable.

## Plan before running

```bash
aniflow --output json video interpolation-plan \
  --configuration "/absolute/rife-configuration.json" \
  --frame-index "/absolute/frame-index.json"
```

The planner reads caller JSON and returns `aniflow.rife-plan/v1`. It validates the
configuration and imported temporal profile, derives exact clocks and reports
resource admission. It does not launch a tool, inspect a model, reserve RAM,
probe a GPU, create output directories or authorize execution.

Use [the caller schema](../providers/rife/configuration.schema.json) and
[synthetic shape example](../providers/rife/configuration.example.json). Replace
every synthetic path, digest, byte size, version and host observation with actual
local values before using the configuration. An absent `enabled` means `false`.
A disabled plan preserves the source frame count/rate and has `resources: null`.
The enabled provider profile cannot execute a disabled configuration.
Caller configuration documents are bounded to 64 KiB; input and output frame
indexes are bounded to 16 MiB. An otherwise valid frame count can still exceed
the serialized evidence budget.

Supply operating system (`macos` or `linux`), architecture (`aarch64` or `x86_64`),
total memory and currently available memory in MiB. Enabled configurations need
at least 4 GiB total RAM and positive available RAM no greater than total RAM.
These are explicit operator observations; this checkpoint does not obtain them
automatically or claim that they remain current during processing.

## Conservative defaults and explicit budgets

| Setting | Default or fixed first-profile policy |
| --- | --- |
| Enabled | `false` |
| Preset | `auto`, resolving conservatively except for an explicitly observed higher-memory Linux x86_64 host |
| Device | `-1`, explicitly selecting CPU; one Vulkan device `0..31` requires an explicit setting |
| Concurrent pairs | Exactly one |
| Tool threads | `1:1:1`; no configurable parallel batch |
| Time per child invocation | At most 120,000 ms |
| Captured tool output | At most 1 MiB |
| Individual frame bytes | Configurable up to 64 MiB, with an additional geometry-based bound |
| Artifact budget | 64 GiB by default; configurable from 1 MiB to 1 TiB |
| Spatial/temporal test-time augmentation and UHD mode | Omitted; this profile does not expose these flags |

| Preset | Padded pixel cap | Estimate cap | Host requirement |
| --- | ---: | ---: | --- |
| `conservative` | 2,100,000 | 2,048 MiB | Explicit admitted host and memory observations |
| `balanced` | 8,400,000 | 8,192 MiB | Explicit selection and at least 32,768 MiB total RAM |

`auto` selects `balanced` only on Linux x86_64 with declared total RAM of at
least 32,768 MiB and available RAM of at least 16,384 MiB. Apple Silicon macOS
and every other admitted observation resolve to `conservative`. These are
policy choices, not native host support or device qualification claims.

Width and height are rounded upward to multiples of 32 for admission. The
working-set heuristic is `512 MiB + 512 bytes × padded pixels`, rounded upward
to MiB. The admission budget is the smallest of one quarter of total RAM, one
third of available RAM and the preset estimate cap. Geometry above the pixel
cap or an estimate above the budget is refused before launching RIFE.

This estimate is uncalibrated. It is not measured RSS, a GPU-memory estimate, an
OS-enforced memory limit or a promise that a laptop cannot crash. The plan
records `memory_limit_enforced: false` and `native_qualified: false`. CPU and
Vulkan execution still need synthetic native qualification, including resource
measurements on Apple Silicon. A nonnegative device ID declares GPU use; it
does not prove that the device exists or has sufficient memory.

Interpolate at the smaller source resolution before an optional 4K upscale.
Conservative admission deliberately refuses 4K-sized interpolation rather than
silently increasing concurrency, selecting a larger preset or resizing the
source. On macOS, selecting `balanced` remains explicit even with higher-memory
observations. For an initial preview, prepare a separate short preview clip with
zero-origin temporal inspection and indexed frames. Slicing an existing frame
list without matching temporal evidence is insufficient. This slice has no
automatic preview extractor or implicit resize.

## Exact input and output clocks

The [frame-index contract](contracts/rife-frame-index-v1.schema.json) requires:

- An `aniflow.temporal-inspection/v1` record accepted by the existing
  [zero-origin CFR policy](temporal-correctness.md), with exact rational timing.
- Positive width/height matching the selected video stream and frame count.
- `frame_profile: "rgb8_sdr_bt709_square_unrotated"`.
- One to 100,000 ordered frame records with consecutive zero-based ordinals,
  unique portable PNG basenames and SHA-256 digests.
- Sorted, unique `cut_before_ordinals`: the right-frame ordinal where a new shot
  starts, excluding zero and any ordinal beyond the last frame.

No automatic frame extraction, colorspace conversion, HDR tone mapping, rotation,
alpha removal, sample-aspect-ratio correction, VFR normalization, cut detection,
frame dropping or audio processing occurs. The import profile is an operator
declaration. The provider validates frame bytes and PNG structure, but does not
independently inspect the original video or prove the declared color treatment.

An enabled plan doubles the source frame count and exact rational frame rate,
preserving duration. For example, `24000/1001` becomes `48000/1001` with exactly
`2N` output frames. Each even output ordinal copies its source frame. Each odd
ordinal uses the midpoint of adjacent source frames, except at a declared cut,
where it holds the left frame, or after the last frame, where it holds the final
frame. These holds preserve duration and avoid blending across declared cuts.
The provider generates `frame_00000000.png` onward and records each frame's
digest, source ordinals and origin in `aniflow.rife-output-index/v1`.
The public `RifeOutputIndex::from_json_slice` reader checks the bounded closed
document; `validate_against` additionally recomputes its exact plan from the
configuration and source index and checks every output ordinal and mapping.
Independent file-byte observations remain a provider execution responsibility.

Selected source audio remains outside this provider. The plan records
`not_processed; preserve_selected_audio_at_assembly`; that describes the later
assembly requirement, not a completed audio-sync or remux operation. General
target rates, direct video input/output and assembly remain parent #110 work.

## Explicit provider registration and execution

`RifeProviderConfiguration` wraps an enabled caller configuration. Its
`provider_configuration()` method creates the exact provider values and
`manifest()` creates an inert manifest with the digest of the published
[provider-configuration schema](../providers/rife/provider-configuration.schema.json).
Deserialize and normalize caller defaults before constructing the effective
provider envelope. Its schema requires every settings field: the envelope hash
binds the complete materialized values, and omitted defaults in raw values are
refused rather than silently changing the configured invocation identity.
Generate the manifest for the selected configuration: CPU execution declares
GPU forbidden; an explicit Vulkan device declares GPU required. No placeholder
manifest is supplied as a runnable installation.

The capability `aniflow/rife-interpolation` version `1.0.0` has two immutable
inputs and two exact outputs:

| Port | Direction | Artifact type | Role |
| --- | --- | --- | --- |
| `frames` | Input and output | `application/vnd.aniflow.rife-frames` | Intermediate video frame directory |
| `index` | Input | `application/vnd.aniflow.rife-frame-index+json` | Temporal/frame import evidence |
| `index` | Output | `application/vnd.aniflow.rife-output-index+json` | Output lineage evidence |

Registration and Pipeline v3 selection are separate explicit operations using
the existing [provider authoring contract](provider-authoring.md). The existing
direct-argv ABI routes this provider through
`aniflow --aniflow-invocation "/absolute/invocation.json"`; it is not a new public
standalone video-processing command. A successful frame-directory operation
does not by itself establish accepted final-video delivery.

## Pinned bytes and bounded execution

Supply an absolute normalized executable path, declared exact semantic version
and SHA-256. The model directory basename must be `rife-v4.6` and contain exactly
`flownet.param` (at most 16 MiB) and `flownet.bin` (at most 256 MiB), with explicit
digests and byte sizes. No installation, model download, network inference,
backend fallback or model discovery happens implicitly. The declared tool
version is retained evidence; this slice has no upstream version-probe proof.

The provider stages pinned tool/model/frame bytes into private workspace paths
and invokes adjacent pairs serially through the existing bounded process runner.
Execution requires the declared operating system and architecture to match the
running host; RAM values remain explicit declarations. Supplied input paths must
resolve to canonical nonsymlink files/directories.
It preserves the model-directory name and passes only midpoint `0.5`, explicit
device and `1:1:1` threads. The upstream uses short flags for these options;
Aniflow passes direct argument vectors without a shell.

Source directories must contain exactly their indexed regular PNG files, and
outputs must be new assigned paths. PNG checking streams bounded chunks and
checks signature, CRCs, RGB8 noninterlaced IHDR, expected geometry and supported
ancillary chunks. This is structural checking, not proof that compressed image
data decodes correctly or that interpolation looks good.

The provider calculates a conservative artifact estimate for input snapshots,
worst-case outputs, pair scratch, tool/model snapshots and index evidence before
launch. It does not inspect available filesystem space or reserve disk capacity.
Time, captured output and observed file limits are bounded. The artifact
estimate is not a live disk quota, and subprocess handling is not an OS sandbox
or enforced RSS/VRAM boundary. A pinned external binary remains executable code.
Interrupted work can retain partial artifacts; automatic successful reuse or
resumption of a partial RIFE frame directory is not part of this first slice.

The synthetic Rust/provider/schema fixtures are authored for later #64 checks.
No compiler, test, schema, native-model or platform result is claimed here.
