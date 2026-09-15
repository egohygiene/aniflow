# Pipeline schemas

## Pipeline v3 planning

Pipeline v3 is an explicit, read-only planning contract. Authored YAML or JSON
uses the schema identity `aniflow.pipeline/v3`; a resolved plan uses
`aniflow.pipeline-plan/v1`. The public `plan_v3` file facade and lower-level
`PipelineV3Configuration` plus `resolve_pipeline_v3` APIs are the authoritative
implementation. The CLI delegates to the same file facade as `aniflow
plan-v3`.

`plan-v3` validates intent, hashes explicit input bytes, and resolves only the
provider registrations supplied by the caller. It does not inspect `PATH`,
search plugins, access the network, launch a process, create a workspace, write
an artifact, or execute any stage. Pipeline v3 execution and resume are separate
future checkpoints.

### Authored configuration

Root and nested objects reject unknown fields. Collections whose order carries
intent retain that order.

| Root field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schema` | string | yes | Exactly `aniflow.pipeline/v3` |
| `name` | string | yes | Stable pipeline name |
| `description` | string | no | Human context; never provider authority |
| `inputs` | array | yes | Declared immutable input artifacts |
| `stages` | array | yes | Ordered temporal stages |
| `outputs` | array | yes | Pipeline results and required validations |

Each input declares `id`, `artifact_type`, `artifact_role`, and optional
`stream_role`. The declaration contains semantic identity, not a local path.
The caller binds each declared ID to bytes separately with repeated
`--input ID=PATH` arguments or the equivalent library request. Every declared
input must have exactly one binding; undeclared or duplicate bindings fail.
Directory identities sort portable relative entry names and reject symbolic
links, platform-reserved segments, and case-folded name collisions.

Each ordered stage has this shape:

| Stage field | Meaning |
| --- | --- |
| `id` | Unique stable stage identity |
| `depends_on` | Ordered IDs of earlier stages that must precede this stage |
| `capability` | Exact capability `id` plus semantic `version_requirement` |
| `provider` | Optional replacement, required primary, and ordered fallback registration references |
| `inputs` | Capability input-port bindings to one or more declared artifact IDs |
| `outputs` | Capability output-port bindings to expected artifact IDs and paths |
| `validations` | Ordered obligations with `id`, target `artifact`, and `contract` |

A provider reference is `{ registration_id: ... }`. Resolution tries the
optional `replacement`, then `primary`, then `fallbacks` in authored order. A
stage input binding has `port` and ordered `artifacts`. A stage output binding
has `port` and ordered artifacts, each containing `id` and `relative_path`.
Output paths must be portable relative paths beneath `artifacts/`; absolute
paths, traversal, control characters, Windows-reserved names or characters,
trailing spaces or periods, and case-insensitively overlapping artifact
destinations are rejected.

Artifact IDs have one producer. Dependencies and input bindings must agree:
stages may consume declared root inputs or artifacts produced by an earlier
dependency, never missing, ambiguous, or forward-produced artifacts. Port
direction, artifact type, stream role, and cardinality must agree with the
selected capability declaration. Producer and consumer artifact roles remain
separately visible because a stage boundary can intentionally change the
artifact's workflow role without changing its media type.

Each root output contains `id`, the referenced `artifact`, and ordered
`required_validations`. A required validation must exist and target that
artifact. Expected artifacts and validation obligations are plan intent; they
do not claim that output bytes already exist.

Pipeline v3 has no `renderflow` field. Unknown-field rejection makes a v2-style
renderflow handoff an actionable configuration error. Cross-holon sequencing
belongs to `flow`.

### Explicit planning inputs and authority

Provider code is not inferred from an authored pipeline. Supply one or more
`aniflow.provider-registration/v1` JSON locator documents with repeated
`--provider-registration FILE` arguments. Each closed document contains:

- `schema` and a unique `registration_id`;
- paths to an `aniflow.provider-manifest/v1` `manifest`, an
  `aniflow.provider-configuration/v1` `configuration`, and an `executable`;
- a stable `implementation_id`; and
- observed exact `components` for tools, codecs, and models.

Manifest, configuration, and executable paths are resolved relative to the
registration document and must remain within its directory after symbolic-link
resolution. Portable locator rules reject traversal, reserved names and
characters, and trailing spaces or periods. Those locator paths are operational
input only and are omitted from canonical plan identity. Their verified
contract and content identities, including the executable digest and component
identities, are retained in the selected provider lock. Component lists are
unique and sorted by ID and version.

Planning also requires explicit host observations and authority:

```bash
aniflow plan-v3 \
  --pipeline "pipeline-v3.yml" \
  --input "source-video=/media/source.mp4" \
  --provider-registration "providers/upscale.registration.json" \
  --host-cpu-threads 8 \
  --host-memory-mib 16384 \
  --host-storage-mib 65536 \
  --host-gpu-available \
  --allow-side-effect filesystem-read \
  --allow-side-effect filesystem-write \
  --allow-side-effect subprocess \
  --allow-side-effect gpu \
  --offline \
  --output json
```

`--host-gpu-available` and `--host-network-available` are optional observed
availability flags. Repeat `--allow-side-effect` for explicit grants from
`filesystem-read`, `filesystem-write`, `environment-read`, `subprocess`,
`network`, `ai`, `gpu`, and `publish`. `--offline` rejects candidates whose
declared behavior permits or requires network access. These inputs are checked
against provider declarations; they are not OS sandbox or resource-quota
guarantees.

### Resolved plan and canonical identity

A successful plan contains the normalized ordered stages, dependencies, typed
bindings, expected artifacts, validations, capability declarations,
replacement/primary/fallback policy, every deterministic resolution attempt,
and the selected exact `aniflow.provider-lock/v1` for each stage. Locks retain
the exact provider, capability, configuration-schema, effective-configuration,
manifest, implementation/executable, tool, codec, model, authorized-effect,
selection-source, and offline identities already defined by the provider
contract.

The resolved document has `schema: aniflow.pipeline-plan/v1`,
`algorithm: sha256`, `canonicalization: aniflow.canonical-json/v1`,
`plan_sha256`, and `payload`. `plan_sha256` covers only the payload encoded with
canonical JSON v1:

- object keys are recursively sorted;
- arrays retain their declared order;
- no insignificant whitespace is emitted; and
- scalars use the JSON representation emitted by `serde_json`.

Mapping-key order and registration insertion order therefore cannot affect
identity. Changing input bytes, ordered stages, dependencies, bindings,
capabilities, provider/configuration/implementation/components, expected
artifacts, validations, grants, offline mode, or any supplied host observation
changes the plan. The optional human `description`, local locator paths,
working directories, temporary roots, timestamps, run IDs, human diagnostic
prose, logging, and telemetry are not identity material.

Planning failures use `aniflow.pipeline-planning-failure/v1`. Diagnostics retain
a stable code, the affected stage when applicable, and the existing ordered
provider-resolution attempts and typed availability reasons. Missing,
incompatible, denied, offline-conflicting, ambiguous, or contradictory intent
fails before execution. Human output summarizes the same evidence; machine
consumers do not parse prose.

### Migrating from Pipeline v2

Pipeline v2 is never silently upgraded. `plan`, `run`, and `resume` keep their
v2 behavior, while `plan-v3` requires `aniflow.pipeline/v3`. Supplying a v2
document to `plan-v3` returns a typed migration diagnostic. V3 execution and
resume are not available yet.

| Pipeline v2 | Pipeline v3 planning |
| --- | --- |
| `version: 2` | `schema: aniflow.pipeline/v3` |
| One CLI `--input PATH` | Declare root input identity and bind it with `--input ID=PATH` |
| Ordered `frame_processors`, `audio_processors`, and `video_processors` | Ordered `stages` with explicit dependencies, capability requests, ports, and artifacts |
| Processor `command`, adapter options, and implicit `PATH` compatibility | Provider manifest/configuration plus an explicit registration locator and registration policy |
| `{input}` / `{output}` placeholders | Typed input/output port bindings |
| Root `validation` and processor-specific checks | Ordered stage `validations` and root `required_validations` |
| `output.file` | Stage artifact `relative_path` beneath `artifacts/` plus a root output reference |
| `enabled: false` documentation entries | Omit the stage from executable intent |
| Optional `renderflow` handoff | No mapping; let `flow` sequence renderflow separately |

Do not translate legacy configuration by changing only its version marker.
Provider capabilities, artifact roles, dependencies, registrations, host
observations, and granted effects must be made explicit and reviewed.

## Pipeline v2 compatibility

Pipeline files are YAML documents. Root objects reject unknown fields so
misspellings cannot silently alter a run. The new processor `limits` object is
also closed to unknown fields.

### Root

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

### `frame_processors`

Every enabled processor requires a unique lowercase `id`. Each stage receives
the previous stage's PNG directory.

#### `kind: gemini_watermark_remover`

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

#### `kind: upscayl_ncnn`

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

#### `kind: external`

| Field | Default | Description |
| --- | --- | --- |
| `command` | required | Executable name or path |
| `arguments` | `[]` | Direct arguments containing `{input}` and `{output}` |
| `concurrency` | `1` | Simultaneous frames |
| `limits` | bounded defaults | Common provider-execution limits described below |

Supported placeholders are `{input}`, `{output}`, `{frame}`, and `{run_dir}`.

### `validation`

| Field | Default | Description |
| --- | --- | --- |
| `require_uniform_dimensions` | `true` | Reject dimension changes between adjacent output frames |
| `minimum_frame_bytes` | `64` | Reject empty or suspiciously small PNG outputs |

Frame names and total count must always match the extracted source sequence.

### `audio_processors` and `video_processors`

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

### Processor resolution and limits

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

### `subtitles`

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Whether subtitles are applied |
| `source` | required | SRT or ASS path relative to the pipeline |
| `mode` | `burn` | `burn` or `mux` |

Enabled subtitle sources are copied into the run before processing.

### `renderflow`

> **Deprecated:** this pipeline v2 compatibility field remains executable in
> `0.3.x`, but it is no longer part of aniflow's target architecture. Pipeline
> v3 configuration rejects cross-tool selection; flow orchestrates renderflow
> or other downstream tools from aniflow's validated master and versioned
> result.

The renderflow compatibility handoff is intentionally not adapted as a
processor provider. It remains available only to Pipeline v2 callers.

| Field | Default | Description |
| --- | --- | --- |
| `enabled` | `false` | Whether the handoff executes |
| `command` | `renderflow` | Future CLI executable |
| `arguments` | built-in stub | Direct args containing `{input}` and `{output}` |

`{input}` is the completed master and `{output}` is the run's `renderflow/`
directory.

### `output`

| Field | Default |
| --- | --- |
| `file` | `output/master.mp4` |
| `video_codec` | `libx264` |
| `crf` | `18` |
| `preset` | `slow` |
| `pixel_format` | `yuv420p` |

The configured master path must remain beneath the run's `output/` directory.
