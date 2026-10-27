# Upscale only when the target requires it

The fixed music-video recipe can compare the current frame geometry with an
explicit target before starting Upscayl. If compatible frames already meet or
exceed the target, it passes those accepted frames unchanged to assembly. The
neural executable and model files are unnecessary for that bypass. Required
frame import, source-profile admission and assembly validation still apply.

This is the authored [#111](https://github.com/egohygiene/aniflow/issues/111)
checkpoint, stacked on open recipe PR #131. All execution and native
qualification remain deferred under #64. Dimensions describe geometry, not
perceptual quality or recovered native detail.

## Add the rule to a recipe

In the existing [music-video configuration](music-video-recipe.md), select
`upscale.mode: "target_resolution"`, retain the complete existing Upscayl
configuration in `upscale.configuration`, and add `upscale.policy` using one
of these [closed policy objects](../providers/upscayl-recovery/target-policy.schema.json).

Landscape UHD:

```json
{
  "schema": "aniflow.upscale-target-policy/v1",
  "width": 3840,
  "height": 2160,
  "mode": "exact",
  "force_reprocess": false
}
```

Portrait UHD:

```json
{
  "schema": "aniflow.upscale-target-policy/v1",
  "width": 2160,
  "height": 3840,
  "mode": "exact",
  "force_reprocess": false
}
```

These are the `policy` objects, not complete recipe files. The embedded
`configuration` remains the [existing Upscayl configuration](upscayl-recovery.md):
explicit executable, decoder, model pair, device, tile, workers, TTA and
per-frame or chunk settings. In target mode, its `output_scale` is a template
value. The decision derives the effective 2×, 3× or 4× output scale and changes
only that field. Model-native scale remains separately declared and the
selected profile still requires its supported native 4× model.

A bypass keeps the configuration declaration in recipe intent but does not
inspect neural executable/model files or require them to exist. Executing a
neural path requires valid current pins and the existing provider's complete
admission. No model installation, dependency discovery or device fallback is
implicit.

Existing `upscale.mode: "disabled"` still requires an explicit reason and
performs no neural work. Existing `upscale.mode: "upscayl"` remains unconditional
with the configured output scale. This policy does not change the standalone
`video upscale-frames` command into a conditional operation.

Optional target-report fields remain omitted when unused, preserving the
legacy serialized field sets. This does not promise that an older retained run
can resume across this source/schema update: the larger allowed importer pixel
ceiling changes embedded schema digests, provider locks and affected plan/intent
identities. Unchanged defaults do not override the existing exact compatibility
rules. Start a new run when those identities differ.

## Exact, minimum and force

Both oriented dimensions must have the same aspect ratio as the target.
Width alone, total pixel count, a filename containing “4k”, bitrate or a tag
cannot meet the rule. The selected profile is square-pixel and unrotated;
a portrait target must be declared as portrait dimensions.

`exact` means an executing operation must reach both target dimensions using
one supported integer scale. `minimum` chooses the smallest supported scale
that reaches both dimensions; any overshoot is reported explicitly. Neither
mode downscales an already larger compatible input. Destination downscale and
export policy remain #83 work.

| Current frames | Target and mode | Decision |
| --- | --- | --- |
| 1920×1080 | 3840×2160, exact | Execute at 2×; output 3840×2160 |
| 1080×1920 | 2160×3840, exact | Execute at 2×; output 2160×3840 |
| 1280×720 | 3840×2160, exact | Execute at 3×; output 3840×2160 |
| 2560×1440 | 3840×2160, exact | Refuse: 1.5× is outside the supported integer scales |
| 2560×1440 | 3840×2160, minimum | Execute at 2×; output 5120×2880 |
| 3840×2160 | 3840×2160, either mode | `already_target`; preserve input unchanged |
| 7680×4320 | 3840×2160, either mode | `above_target`; preserve input unchanged, subject to explicit importer/storage bounds |
| 3840×1080 | 3840×2160, either mode | Refuse incompatible aspect; one matching axis is insufficient |
| 1080×1920 | 3840×2160, either mode | Refuse incompatible orientation/aspect; no automatic rotation |

All execute examples remain subject to actual tool, frame, pixel, storage and
source-profile admission; the table is geometry arithmetic, not native
qualification.

`force_reprocess: true` prevents the already/above-target bypass. It does not
invent a 1× model pass or permission to resize afterward. Below target, normal
exact/minimum scale selection still applies. At or above target, forced
`minimum` selects 2× and reports that larger effective output; existing native
intermediate and resource ceilings can still refuse it. Forced `exact` at or
above target is refused because the selected adapter cannot reprocess at 1× or
downscale. Force is an explicit request for supported neural work, not a quality
improvement guarantee.

## Actual input and decision evidence

The public `upscale_target` module separates pure `decide` arithmetic from
bounded `observe_input` file observation. The observer checks complete directory
membership, every indexed PNG's structure, IHDR dimensions and digest, then
binds the current index and v3 directory identity. Its RGB8/SDR/color/aspect
profile remains an index declaration. It does not launch native probes or
prove full image decompression, source extraction history or color fidelity.
Existing importer/source/assembly owners supply their later required evidence.

Initial recipe planning records `target_resolution.input_phase: "original_input"`.
That decision is bound to actual original PNG structure and byte identities;
it is not acceptance of the future prepared output. Once frame preparation is
accepted, status/run/resume observe the actual handoff again with
`input_phase: "prepared_frames"`. Target, geometry, index, directory or selected
component changes cannot reuse an old decision as acceptance authority.

The public observer can read original and Upscayl spatial indices. The fixed
recipe still starts only from the original `aniflow.rife-frame-index/v1`
workspace. This checkpoint does not make a previously collected Upscayl spatial
index a supported recipe entry point. Its internal accepted prepared-frame
handoff is reevaluated without rewriting source geometry or clocks.

The optional `target_resolution` report in plan, intent, status and outcome
contains:

| Field | Meaning |
| --- | --- |
| `input_phase` | Original-input observation or accepted prepared-frame observation |
| `decision` | Bound input and policy identities, reason, `disabled`/`already_target`/`above_target`/`execute`, effective dimensions and optional derived output scale |
| `resources` | Frame/pixel/RGB byte arithmetic and configured executing-path ceilings, with `memory_limit_enforced: false` |
| `required_components` | `upscayl`, `upscayl-ffmpeg`, `upscayl-param`, `upscayl-weights` for execution; empty for bypass |

`unchanged_input_sha256` binds the unchanged bypass identity. A skipped neural
phase has no model execution report or new “improved” image. The assembly child
still creates and accepts its outputs normally. Resource arithmetic is not a
measured RSS/VRAM forecast, disk reservation or enforced memory limit; executing
work still goes through the existing complete Upscayl storage admission.

Use the existing `video music-video --plan`, run/resume flags and
`video music-video-status` commands shown in the [recipe guide](music-video-recipe.md).
Downstream exact child plans still wait for accepted predecessor output.

## Above-UHD inputs need explicit resource admission

The importer keeps its default `maximum_pixels: 8400000`. The allowed ceiling
is now 33,554,432 pixels, so a compatible larger input can be admitted explicitly
for unchanged bypass. For the 7680×4320 example, set
`maximum_pixels: 33554432` in the selected preparation importer: `repair.import`
when repair is disabled, or `repair.configuration.import` for named repair.
The other selected child settings and downstream bounds still apply.

This opt-in ceiling does not qualify 8K media, tools, devices or performance.
Compressed-frame, aggregate-byte, source, timeout, working-space and capture
bounds remain unchanged. For example, a poorly compressible PNG can exceed the
existing per-frame byte limit even when its pixel count fits. No native 4×
intermediate is allocated on genuine bypass; force or another executing policy
must still fit the Upscayl intermediate-pixel ceiling.

## Preview a short excerpt with the same settings

Use the separate `video upscale-preview` command before an expensive full run.
Its [configuration](../providers/upscayl-recovery/preview-configuration.schema.json)
has schema `aniflow.upscayl-preview-configuration/v1`, a `settings` object
containing the same full Upscayl configuration, and required `target_resolution`
containing the policy above. These are embedded objects, not configuration-file
paths. The same pure policy derives the effective output scale.

```bash
aniflow --output json video upscale-preview \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/source-frames" \
  --frame-index "/absolute/source-frame-index.json" \
  --configuration "/absolute/upscale-preview.json" \
  --range "1:64" \
  --plan
```

Replace `--plan` with `--output-directory "/absolute/upscale-previews"` to
start an isolated preview run, or with
`--resume-run-directory "/absolute/upscale-previews/RUN_DIRECTORY"` to resume
the exact selected excerpt. `aniflow.upscayl-preview-plan/v1` and
`aniflow.upscayl-preview-outcome/v1` retain the source range, target decision,
excerpt resource estimate, required components and child artifacts.
Use ordinary `status-v3` for its child-stage evidence.
Do not pass the preview run directory to recipe resume.

The inclusive one-based range may contain at most 256 frames and ten exact
source seconds. A range that fits one bound can still exceed the other.
Per-frame mode retains the existing source-ordinal limit below 1,024. In
bounded-chunk mode, the range must begin at an existing configured chunk
boundary; a partial final chunk is allowed. With 64-frame chunks, starts 1,
65 and 129 align, while start 2 does not. No mode, chunk size, model or quality
setting is silently changed to fit the excerpt.

The preview retains original ordinals and source-time intervals and produces
only the selected PNGs plus native observations through ordinary v3 acceptance.
It does not build a complete collected output index or MP4. Its indexed-PNG
admission does not replace full native source-profile admission in the recipe
and assembly path. A target decision that bypasses neural work is refused as an
unnecessary preview before neural files are required.

Preview identity and viewing approval cannot substitute for full-output
acceptance, authorize loose-file reuse or certify visual quality. The full
recipe still processes and accepts its own complete outputs. All preview
execution remains unrun under #64.

#111 remains open for qualification and residual integration, including broader
recipe entry forms. #35 retains initial extraction, wider media profiles and
loudness/metadata work; #107 retains broader manual/review recipes. The fixed
rule adds no arbitrary conditional engine and publishes no immutable release.
