# Repair selected frames into a separate workspace

Choose explicit frame ranges and a direct-image processor, then import and
repair a complete PNG workspace in one ordered Pipeline v3 run. Original inputs
stay separate from the new outputs. Every unselected output must retain its
original bytes; selected outputs must preserve the exact frame count, filenames,
geometry and rational clock. Selecting 299 frames cannot silently produce 240.

This is an authored [#36](https://github.com/egohygiene/aniflow/issues/36)
checkpoint. Tests, builds, schema checks, native execution and qualification
remain unrun under [#64](https://github.com/egohygiene/aniflow/issues/64).

## Choose a repair profile

Supply the original source video, a complete
`aniflow.rife-frame-index/v1`, and its PNG directory. Keep the index outside the
PNG directory: directory membership must exactly match the index. The
[import guide](frame-import.md) describes the admitted RGB8, noninterlaced PNG
and source-clock profile. This command does not extract frames from a video or
guess an older `video-frames/1` schema.

Two closed configuration schemas use the same command and immutable frame
workflow:

| Profile | Configuration schema | Recovery |
| --- | --- | --- |
| Named allenk image repair | `aniflow.allenk-frame-repair-configuration/v1` | Compatible whole import/repair stages can resume |
| Generic direct-image command | `aniflow.frame-repair-configuration/v1` | Fresh runs only; arbitrary external dependencies remain unobserved |

The named profile has a distinct provider/capability. A generic `processor.profile`
label cannot enable named behavior or resume. Neither profile automatically
chooses a tool, downloads a model or decides which source frames should change.

## Configure named allenk image repair

Use an explicitly prepared allenk `GeminiWatermarkTool` executable for the
[source-pinned basic image interface](toolchain-upstreams.md#selected-allenk-image-profile-for-36).
This profile is distinct from GargantuaX's Node-based `gwr`; they are not aliases.
The configured executable version must be `0.3.2`, and execution observes its
`--version` result before processing. Planning only reads and hashes dependencies;
it does not launch a version probe or qualify an installed binary.

Replace the paths and digest placeholders with actual trusted local tools:

```json
{
  "schema": "aniflow.allenk-frame-repair-configuration/v1",
  "import": {
    "schema": "aniflow.frame-import-configuration/v1",
    "ffmpeg": {
      "executable": "/absolute/bin/ffmpeg",
      "version": "8.0.0",
      "sha256": "REPLACE_WITH_OBSERVED_FFMPEG_SHA256"
    }
  },
  "tool": {
    "executable": "/absolute/bin/GeminiWatermarkTool",
    "version": "0.3.2",
    "sha256": "REPLACE_WITH_OBSERVED_ALLENK_SHA256"
  },
  "watermark_profile": "current_v2",
  "detection_threshold_percent": 25,
  "maximum_working_bytes": 274877906944
}
```

`watermark_profile` is required. `current_v2` emits `--no-legacy`; `legacy_v1`
emits `--legacy`. There is no automatic cross-profile fallback. The integer
`detection_threshold_percent` defaults to 25 and admits 0–100; 25 becomes native
`--threshold 0.25`. This is the selected tool's detector threshold, not a promise
that confidence is calibrated or a visual repair succeeds. Manual ranges still
determine which frames reach the tool.

The adapter generates a fixed argv directly, without a shell:

```text
--input input.png --output output.png --remove --no-banner --no-legacy --threshold 0.25
```

The actual private paths are assigned by the provider. The selected basic image
path omits denoising and external models. It deliberately omits `--denoise`
entirely, because even `--denoise off` selects a different upstream processing
path. Force, region, snap, fallback-region and size overrides are outside this
profile. If the selected image is skipped (native exit 1) or fails (exit 2), the
repair stage fails and retains evidence. It does not silently switch profile,
force a weak detection or accept an absent output.

The four visible-overlay profile assets are compiled into the selected native
executable and bound by its digest. The source revision identifies the reviewed
interface; it does not prove how a supplied binary was built. Dynamic libraries,
host settings, native suitability and visual quality remain unobserved or
unqualified. Whole-stage resume reuses previously accepted bytes with the exact
bound source, decoder, executable and typed settings; it does not claim complete
host identity or deterministic results across hosts.

## Configure a generic direct-image command

The [caller configuration](../providers/frame-repair/configuration.schema.json)
has three required fields: `schema`, `import`, and `processor`.
`import` is the existing complete frame-import configuration, including its
pinned FFmpeg decoder and byte/pixel limits. For example, the following is a
configuration template; replace both executable paths, versions and digest
placeholders with the actual trusted local tools before using it:

```json
{
  "schema": "aniflow.frame-repair-configuration/v1",
  "import": {
    "schema": "aniflow.frame-import-configuration/v1",
    "ffmpeg": {
      "executable": "/absolute/bin/ffmpeg",
      "version": "8.0.0",
      "sha256": "REPLACE_WITH_OBSERVED_FFMPEG_SHA256"
    }
  },
  "processor": {
    "profile": "local-direct-image-v1",
    "tool": {
      "executable": "/absolute/bin/my-image-repair-wrapper",
      "version": "1.0.0",
      "sha256": "REPLACE_WITH_OBSERVED_PROCESSOR_SHA256"
    },
    "arguments": ["--input", "{input}", "--output", "{output}"]
  },
  "maximum_working_bytes": 274877906944
}
```

The example argv describes a wrapper interface you supply; it is not a claim
that a particular package accepts those flags. Choose a trusted, self-contained
direct-image executable or wrapper that consumes one PNG and produces one PNG.
Aniflow copies the executable into private work before launching it; adjacent
files are not relocated with it. The tool must work from that copy. Any external
paths it requires must be explicit absolute paths, and those dependencies still
have unobserved identity in this profile.
This generic configuration does not become the named allenk adapter through a
profile label. It does not infer aliases, download dependencies, discover models
or provide clip-only compatibility. A profile is a caller label of 1–128 ASCII letters, digits, dots,
underscores or hyphens, beginning with a letter or digit.

`{input}` and `{output}` must each occupy exactly one whole argv element, once.
Aniflow replaces them with private per-frame paths. All other arguments remain
literal; there is no shell interpolation. Embedded placeholders, other braces,
empty strings and controls are refused. There are at most 256 arguments, each
at most 4 KiB of UTF-8, and at most 32 KiB combined. An argv entry such as
`--output={output}` is invalid; use two elements instead.

The configured processor version and profile are declarations. Its executable
bytes are observed and pinned, but that does not identify an interpreter,
external model, environment-dependent configuration or service it might use.
`model_environment_evidence` remains `unobserved`; use only a tool whose
behavior and dependencies you understand. The processor has no generic version
probe or native qualification claim. The existing decoder performs its own
strict version observation. Neither the profile nor the manifest is an OS
sandbox. This first manifest declares no GPU or network use; choose a local
wrapper that fits those limits. The repair provider is environment-dependent
and not cacheable.

## Plan and run

Ranges are one-based inclusive positions in the complete index, independent of
filename numbering. Repeat `--range` for separate ranges; the planner sorts
them into ascending order and refuses overlaps or repeats. This example selects
positions 1–299 and 600–898:

```bash
aniflow --output json video repair-frames \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/frames" \
  --frame-index "/absolute/frame-index.json" \
  --configuration "/absolute/frame-repair.json" \
  --range "1:299" \
  --range "600:898" \
  --plan
```

The effective operation stores the sorted ranges, so reordering the same range
list does not change its configuration identity.

The plan uses the existing `aniflow.pipeline-plan/v1` shape. It binds original
source/frame/index bytes, exact effective settings and both executable pins,
checks selection and resource admission, and resolves the two ordered stages.
It launches neither tool and creates no output workspace. Plan success does
not establish successful image decoding or repairs.

To execute, replace `--plan` with
`--output-directory "/absolute/repair-runs"`. Keep that parent outside the
input artifacts. The command creates a new isolated run; successful execution
returns the existing `aniflow.pipeline-run-outcome/v1` with durable output
references. Both forms use machine command `video_repair_frames`. Machine
success is written to stdout and machine failure to stderr.

The public `frame_repair` module owns generic plan/run behavior.
`frame_repair::allenk` exposes `AllenkRepairConfiguration`, `AllenkRepairRequest`,
`plan`, `run` and `resume` for the named profile. Requests carry explicit paths,
settings, ranges and the provider executable; the CLI supplies the current
aniflow executable. Low-level native invocation functions only produce candidates;
they do not themselves establish accepted checkpoints or own cancellation.

## Resume the named profile

For named allenk configuration, keep the source video, PNG workspace, index,
ranges, executable pins and settings unchanged. Replace the final option in the
command with:

```text
--resume-run-directory "/absolute/repair-runs/RUN_DIRECTORY"
```

`--resume-run-directory` is mutually exclusive with `--plan` and
`--output-directory`. The facade rebuilds the current exact plan and compares it
with the retained one before requesting ordinary v3 resume. Changed source,
selection, profile, threshold or executable/decoder identity requires a new run.
Generic v3 run/resume also freshly checks the named component files and exact
same-run importer topology, including importer configuration and source bindings.
A loose import report or a differently configured producer cannot substitute.

The checkpoint unit is the entire import or repair stage. A compatible accepted
import can reuse after a later repair failure. An interrupted repair retries
all selected frames once, even when some private outputs already exist. A fully
accepted unchanged repair stage can reuse as a whole. There is no per-frame
repair checkpoint or automatic retry loop, and no mixing of partial results
from different settings. Existing exclusive writer ownership, invalidation,
failure retention and final whole-run acceptance remain authoritative.

Generic repair remains fresh-run only. Generic `resume_v3` still refuses plans
containing `aniflow/frame-repair` version `1.0.0`, including forced reruns,
before progress, durable resume transitions or provider launch. Arbitrary argv
can reference unobserved models or settings; the named capability does not relax
that guard. Read-only `status_v3` remains available for either profile.

## What is preserved and observed

The fixed [pipeline](../providers/frame-repair/pipeline.yml) first runs the
existing immutable importer. The repair stage receives that same run's imported
frames and report as declared dependencies, plus the original index. A loose
`import.json` supplied by itself is not adopted as accepted historical evidence.

Selected frames are privately snapshotted and processed serially. Each output
must be the expected single PNG, pass the shared strict PNG checks and fully
decode through the pinned FFmpeg helper to exactly one RGB24 packet of
`width × height × 3` bytes. Every unselected PNG is copied and its byte identity
rechecked; its decoded identity is supported by the same-run import and that
byte equality. The full output index retains filenames, ordinals, cuts,
source identity and rational times. Only selected frame digests may change.

| Output artifact | Relative path | Meaning |
| --- | --- | --- |
| `imported_frames` | `artifacts/frame-import/frames` | Byte-preserved imported PNG directory |
| `import_evidence` | `artifacts/frame-import/import.json` | Current import observations |
| `repaired_frames` | `artifacts/frame-repair/frames` | Complete separate output PNG directory |
| `repaired_index` | `artifacts/frame-repair/index.json` | Existing RIFE index with selected output digests |
| `repair_evidence` | `artifacts/frame-repair/repair.json` | Selection and per-frame repair observations |

The generic [repair report](contracts/frame-repair-report-v1.schema.json) binds
the import report, effective operation, provider lock and executable identities.
The named `aniflow.allenk-frame-repair-report/v1` wraps those complete observations
under `repair` and adds the pinned upstream revision, observed processor version,
selected profile/threshold and explicit dependency facts:
`profile_assets_evidence: "embedded_in_pinned_executable"`,
`denoising: "disabled"`, `host_runtime_evidence: "unobserved"` and
`backend_qualification: "unqualified"`. Its evidence remains separate from
historical provenance and creative approval.
Each frame records its input/output SHA-256, output size, decoded RGB24 hash,
exact source-time range, selection and observed byte change. A selected frame
may remain byte-identical; that is a valid observation, not proof that a visual
problem was repaired. `changed_frame_count` counts byte changes, not successful
repairs. Decode and byte integrity do not establish perceptual quality.

The source-to-frame relationship remains `declared_not_verified` and earlier
processing remains `unknown`. New run evidence does not prove historical
extraction, authentic authorship or unchanged original colors. Deserializing a
report or validating its JSON structure does not grant runtime acceptance.

## Resource and execution boundary

`maximum_working_bytes` defaults to 256 GiB and allows 1 MiB–16 TiB. Admission
accounts for source/input bytes, candidate and accepted import/repair copies,
selected-output upper bounds, private scratch frames, tools and bounded index/
report evidence. It is a conservative admission bound, not a disk reservation,
an enforced memory limit or process sandbox. The complete import and repair
copies mean this operation needs additional storage.

Existing import bounds still apply, including at most 100,000 frames, 64 MiB
per PNG, a 16 MiB original index, 64 MiB per report and 128 MiB per executable.
The nested import timeout also bounds each processor/decoder invocation. The
outer runtime defaults remain independent: six hours per provider invocation,
1 MiB stdout/stderr, 100,003 artifact files and 64 GiB plus 128 MiB artifact
bytes. Raising a configuration limit does not silently raise those limits;
library callers can supply explicit existing `ProviderExecutionLimits`.

Child processes use the existing bounded process runner and inherit the outer
native provider's process group so cancellation has one owner. No new process
runtime, checkpoint or acceptance format is introduced. Failed or interrupted
candidates remain subject to the existing v3 workspace/evidence rules. These
checks do not promise filesystem confinement against a malicious processor or
hostile concurrent ancestor replacement.

Automatic damage detection, native provider/model qualification, GWR integration,
clip splitting and reconstruction, legacy manifest migration, video assembly,
audio remux and visual approval remain separate work. The known alignment cancellation/start-
marker failure also remains unresolved with its cause unproven. See the
[named checkpoint handoff](validation/aniflow-36-allenk-checkpoint.md) for the
deferred execution plan; the [original generic handoff](validation/aniflow-36-repair-checkpoint.md)
retains its dated evidence.
