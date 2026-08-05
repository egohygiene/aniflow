# Pipeline Schema v2

Pipeline files are YAML documents. Root objects reject unknown fields so
misspellings cannot silently alter a run.

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

No shell is involved. A successful command must create the exact output file.

## `subtitles`

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Whether subtitles are applied |
| `source` | required | SRT or ASS path relative to the pipeline |
| `mode` | `burn` | `burn` or `mux` |

Enabled subtitle sources are copied into the run before processing.

## `renderflow`

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
