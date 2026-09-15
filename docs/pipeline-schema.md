# Pipeline Schema v2

Pipeline files are YAML documents. Root objects reject unknown fields so
misspellings cannot silently alter a run. The new processor `limits` object is
also closed to unknown fields.

## Root

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `version` | integer | yes | Must be `2` |
| `name` | string | yes | Lower-level run name |
| `description` | string | no | Pipeline purpose |
| `frame_processors` | array | no | Ordered frame-processing chain |
| `validation` | object | no | Frame integrity policy |
| `audio_processors` | array | no | Ordered external audio chain |
| `subtitles` | object | no | Prepared subtitle input |
| `video_processors` | array | no | Ordered whole-video chain |
| `renderflow` | object | no | Optional downstream handoff |
| `output` | object | no | Master encoding and path |

## `frame_processors`

Every enabled processor requires a unique lowercase `id`. Each stage receives
the previous stage's PNG directory.

### `kind: gemini_watermark_remover`

| Field | Default | Description |
| --- | --- | --- |
| `command` | `gwr` | CLI executable |
| `json` | `true` | Request machine-readable CLI output |
| `additional_arguments` | `[]` | Arguments appended to the adapter command |
| `limits` | bounded defaults | Common provider-execution limits described below |

The adapter invokes `gwr` once with the input directory and `--out-dir`.

Generated command:

```text
gwr remove <input-directory> --out-dir <output-directory> --overwrite --json
```

### `kind: upscayl_ncnn`

| Field | Default | Description |
| --- | --- | --- |
| `command` | `upscayl-bin` | Upscayl backend executable |
| `model` | `realesr-animevideov3` | Model name |
| `model_path` | none | Directory containing `.param` and `.bin` files |
| `scale` | `2` | Output scale: `2`, `3`, or `4` |
| `tile_size` | none | NCNN tile size; `0` enables automatic selection |
| `gpu_id` | none | Upscayl GPU selector |
| `tta` | `false` | Enable test-time augmentation |
| `additional_arguments` | `[]` | Arguments appended to the adapter command |
| `limits` | bounded defaults | Common provider-execution limits described below |

The adapter invokes `upscayl-bin` once with input and output directories so the
model and GPU runtime are not reloaded for every frame.

Upscayl uses short flags because its upstream CLI does not expose long-form
equivalents.

### `kind: external`

| Field | Default | Description |
| --- | --- | --- |
| `command` | required | Executable name or path |
| `arguments` | `[]` | Direct arguments containing `{input}` and `{output}` |
| `concurrency` | `1` | Simultaneous frames |
| `limits` | bounded defaults | Common provider-execution limits described below |

Supported placeholders are `{input}`, `{output}`, `{frame}`, and `{run_dir}`.

## `validation`

| Field | Default | Description |
| --- | --- | --- |
| `require_uniform_dimensions` | `true` | Reject dimension changes between adjacent output frames |
| `minimum_frame_bytes` | `64` | Reject empty or suspiciously small PNG outputs |

Frame names and total count must always match the extracted source sequence.

## `audio_processors` and `video_processors`

Both use the same direct external-command shape:

| Field | Default | Description |
| --- | --- | --- |
| `id` | required | Unique lowercase stage identifier |
| `enabled` | `true` | Whether the stage executes |
| `command` | required | Executable name or path |
| `arguments` | `[]` | Must contain `{input}` and `{output}` |
| `output_extension` | `wav` / `mp4` | Extension without a dot |
| `limits` | bounded defaults | Common provider-execution limits described below |

No shell is involved. A successful command must create the exact, non-empty
output file; process exit alone never establishes stage completion.

## Processor resolution and limits

Pipeline v2 continues to accept either an executable path or a bare command
name. At the compatibility boundary, aniflow resolves a bare name once from the
caller's `PATH`, converts relative paths to absolute paths, and explicitly
registers that candidate. Provider resolution does not perform implicit `PATH`
discovery. Missing executables and explicit upscayl model files produce typed
availability evidence before expensive media work begins.

Every enabled frame, audio, and video processor accepts this optional object:

| `limits` field | Default | Constraint |
| --- | ---: | ---: |
| `timeout_seconds` | `21600` | at least `1` |
| `termination_grace_milliseconds` | `2000` | any nonnegative integer |
| `maximum_stdout_bytes` | `67108864` | at least `1` |
| `maximum_stderr_bytes` | `67108864` | at least `1` |
| `maximum_artifact_files` | `1000000` | at least `1` |
| `maximum_artifact_bytes` | `1099511627776` | at least `1` |

The resolved provider uses direct arguments, cancellation, a wall timeout,
independent stdout/stderr limits, Unix descendant termination, live artifact
limits, strict relative-output confinement, symlink rejection, and
deterministic output digests. One self-validating provider lock is retained per
processor stage and one execution report per invocation under `providers/` in
the run workspace.

Runtime artifact validation is followed by processor-specific validation.
Per-frame and batch outputs must be valid PNG files. Batch output must preserve
the exact frame count and names before it replaces the stage directory. Audio
and whole-video adapters must produce the configured exact non-empty file.
Invalid temporary output is never promoted into a stage.

The provider-owned normalized configuration shape is published as
[`pipeline-v2-processor-configuration-v1.schema.json`](contracts/pipeline-v2-processor-configuration-v1.schema.json).

## `subtitles`

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Whether subtitles are applied |
| `source` | required | SRT or ASS path relative to the pipeline |
| `mode` | `burn` | `burn` or `mux` |

Enabled subtitle sources are copied into the run before processing.

## `renderflow`

> **Deprecated:** this pipeline v2 compatibility field remains executable in
> `0.3.x`, but it is no longer part of aniflow's target architecture. Pipeline
> v3 will remove cross-tool selection; flow will orchestrate renderflow or other
> downstream tools from aniflow's validated master and versioned result.

The renderflow compatibility handoff is intentionally not adapted as a
processor provider. Its removal remains assigned to Pipeline v3.

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | `false` | Whether the handoff executes |
| `command` | `renderflow` | Future CLI executable |
| `arguments` | built-in stub | Direct args containing `{input}` and `{output}` |

`{input}` is the completed master and `{output}` is the run's `renderflow/`
directory.

## `output`

| Field | Default |
| --- | --- |
| `file` | `output/master.mp4` |
| `video_codec` | `libx264` |
| `crf` | `18` |
| `preset` | `slow` |
| `pixel_format` | `yuv420p` |

The configured master path must remain beneath the run's `output/` directory.
