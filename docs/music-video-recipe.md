# Run the fixed music-video recipe

Use `video music-video` to prepare existing indexed PNGs, optionally repair
selected frames, optionally upscale them, then produce the verified master and
delivery. One command coordinates the existing child runs and hands accepted
outputs to the next operation. Original source files remain unchanged.

This is authored #35 source on a draft stacked above the assembly checkpoint.
Tests, builds, lint/schema checks, native execution and qualification remain
unrun under [#64](https://github.com/egohygiene/aniflow/issues/64). The recipe
retains every child's supported media and tool limits.

## Start with an indexed frame workspace

Supply the original source video, a complete PNG directory and its original
`aniflow.rife-frame-index/v1` input index. Keep the index outside the PNG
directory; directory membership, frame digests and exact source-clock mapping
must match. This command does not extract frames, infer timing or import an
older `video-frames/1` manifest. It does not accept an Upscayl spatial index or
RIFE-derived output index as its starting input. Upscaled indices are produced
and handed to assembly internally when upscaling is selected.

The order is fixed:

| Phase | Behavior |
| --- | --- |
| `prepare_frames` | Named allenk repair already includes immutable import. With repair disabled, the existing standalone importer still copies and fully decodes the complete frame set. |
| `upscale` | Run the existing Upscayl recovery profile, or record the explicit disabled reason. |
| `assembly` | Run the existing verified assembly profile over the accepted prepared or upscaled frames. |

No second importer is added when named repair is selected. The recipe uses
existing child Pipeline v3 plans, locks, checkpoints and validation. It does
not introduce arbitrary stage lists, automatic provider selection or a new
media-acceptance authority.

## Choose the optional work explicitly

The [configuration schema](../providers/music-video-recipe/configuration.schema.json)
requires `schema`, `repair`, `upscale` and `assembly`. Child configurations are
embedded JSON objects, not paths to configuration files. Unknown fields are
refused. Disabling repair or upscaling requires a readable, nonblank reason;
the reason participates in recipe intent identity.

This complete template imports and assembles without repair or upscaling.
Replace the paths and digest placeholders with your trusted local executable
identities before using it:

```json
{
  "schema": "aniflow.music-video-recipe/v1",
  "repair": {
    "mode": "disabled",
    "reason": "No frame repair is selected for this source.",
    "import": {
      "schema": "aniflow.frame-import-configuration/v1",
      "ffmpeg": {
        "executable": "/absolute/bin/ffmpeg",
        "version": "8.0.1",
        "sha256": "REPLACE_WITH_OBSERVED_FFMPEG_SHA256"
      }
    }
  },
  "upscale": {
    "mode": "disabled",
    "reason": "Keep the current frame dimensions."
  },
  "assembly": {
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
}
```

Enable a step by replacing its whole object with the corresponding selected
mode and existing configuration:

| Step | Required enabled fields |
| --- | --- |
| Named repair | `repair.mode: "allenk"`, `repair.configuration` containing the complete [named allenk configuration](frame-repair.md#configure-named-allenk-image-repair), and `repair.ranges`, such as `[{"first":1,"last":299}]` |
| Upscayl | `upscale.mode: "upscayl"` and `upscale.configuration` containing the complete [Upscayl recovery configuration](upscayl-recovery.md#pin-the-executable-and-model) |

Repair ranges are explicit inclusive, one-based positions. The generic
arbitrary-command repair profile is not a recipe option. Enabled repair uses
its own embedded import settings; do not add a separate root import setting or
leave the disabled-mode `reason`/`import` fields in an enabled repair object.

The [selected media setup](toolchain-media-setup.md) operation can prepare the
existing allenk or Upscayl configuration to embed. It does not create the recipe
or choose optional steps. Its Upscayl mapping retains per-frame policy; for a
longer input, choose bounded chunks explicitly in the embedded configuration.
Keep decoder and assembly tool pins consistent; the assembly interface requires
FFmpeg/ffprobe 8.0.1. Recipe planning checks selected settings and predicted
output geometry without claiming native execution or output acceptance.

## Optional target-resolution rule

The opt-in [target-resolution policy](target-resolution.md) adds an explicit
execute-or-bypass choice to the existing upscale phase. It compares actual
stage geometry against a landscape or portrait target, with exact/minimum
intent and a separate force choice. A genuine bypass forwards the accepted
frames unchanged without needing neural tool/model files. Required importer,
source-profile and assembly checks still apply. No silent crop, stretch,
letterbox or destination downscale is added.

The policy guide contains the current configuration shape, geometry examples,
reporting limits and separate same-settings excerpt preview. Omitting the
policy retains the existing disabled or unconditional Upscayl behavior.

## Plan, run and continue

Start with a read-only plan:

```bash
aniflow --output json video music-video \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/source-frames" \
  --frame-index "/absolute/source-frame-index.json" \
  --configuration "/absolute/music-video.json" \
  --plan
```

The plan reads source/frame/tool identities without launching tools or creating
a recipe workspace. It returns `aniflow.music-video-plan/v1` with immutable
intent, selected output dimensions and three phase descriptions. The first
`prepare_frames` plan can be exact because its inputs already exist. Enabled
upscale and assembly plans are `pending` with no child plan until accepted
predecessor output exists. Disabled upscaling retains its reason. Pending does
not mean unsupported or failed: it means the exact future frame bytes and index
are not available to bind yet.

Replace `--plan` with `--output-directory "/absolute/music-video-runs"`
to start. The directory is a parent for a new isolated recipe workspace,
not an output MP4 filename or an existing child run. Execution records recipe
intent, freshly observes the original source's assembly support before the
first pending native child, and proceeds in the fixed order. Unsupported source
profiles can therefore stop the recipe before expensive repair/upscale work.
The source observation is a preflight fact, not a new accepted artifact.

Each enabled child must complete with accepted output before the next exact
plan is materialized. The outcome returns `aniflow.music-video-outcome/v1`,
the recipe root, phase/child references and the final assembly outputs. Keep
the recipe root and its child runs together.

After interruption, keep the same source, frames, index, configuration and
tools. Replace the final option with
`--resume-run-directory "/absolute/music-video-runs/RECIPE_DIRECTORY"`.
The directory is the **recipe root**, not one of its child v3 directories.
Machine failures retain `aniflow.music-video-recovery/v1` when that root is
known. Changing selected settings, disabled reasons or bound inputs requires
a new intent/run; the recipe does not silently combine incompatible results.

Read recipe status without supplying source/tool configuration or launching
media tools:

```bash
aniflow --output json video music-video-status "/absolute/music-video-runs/RECIPE_DIRECTORY"
```

The result uses `aniflow.music-video-status/v1` and includes phase states,
recorded child locations, exact plan identities when available, verified child
manifest evidence and final outputs when complete. Use ordinary `status-v3`
on a reported child directory for its detailed stage evidence. Neither status
command grants acceptance to loose files. Although the status CLI needs no
explicit source/configuration arguments, it uses the recorded intent to read
bounded source and component identities for compatibility. It is not an offline,
portable proof after the original inputs or tools have disappeared.

A completed recipe resume reuses compatible accepted child evidence without
launching a fresh native source probe. If work remains, the fresh source
preflight precedes the first pending native child. Each child retains its own
recovery unit: whole named-repair stages, Upscayl frames or chunks, and whole
master/delivery stages. The recipe does not add per-frame checkpoints to a
whole-stage provider or adopt private partial output.

If interruption leaves a child directory before its required plan/manifest
initialization is complete, the recipe retains that directory and refuses to
overwrite it automatically. An abandoned exclusive-writer lock also requires
explicit reconciliation under the existing writer policy; a dead PID alone
does not authorize automatic lock deletion. Inspect the retained evidence
before deciding how to recover.

## Evidence, outputs and limits

Immutable recipe intent binds the selected configuration, initial identities
and provider/component information. Fixed child paths identify the owned runs;
there is no separate mutable locator-file registry. The recipe stores no
independent completion decisions: child v3 plans, validation and accepted
checkpoints remain the authority. Do not edit intent or replace a fixed child
path with another run to attempt recovery.

The final artifacts are the assembly child's `lossless_master`,
`master_evidence`, `h264_delivery` and `delivery_evidence`. Use their reported
paths. The master is FFV1/bgr0 MP4; the delivery is H.264/yuv420p MP4. Original
AAC is copied and verified when selected. See [assembly](video-assembly.md) for
decoded-pixel, exact-clock, packet, priming and faststart evidence, and the
strict progressive zero-origin CFR/opaque BT.709 SDR support boundary.

The recipe does not broaden those limits or turn declared PNG/source lineage
into proof of extraction or past processing. It does not add visual approval,
metadata-authenticity claims, RIFE-derived clocks, long-tail handling or native
qualification. Original and upscaled geometry stay distinct.

Selected child resource limits still apply. Child snapshots, retained accepted
outputs and failure evidence can use substantial space, and assembly also
checks the system temporary volume before new native work. There is no in-place
replacement or automatic source/prior-run deletion. A successful plan does not
reserve disk or certify memory, device availability or tool/model suitability.

The public `music_video_recipe` module provides `MusicVideoConfiguration`,
`MusicVideoRequest`, `plan`, `run`, `resume` and `status`, with typed plan,
intent, progress, outcome, status and recovery contracts. Progress preserves
the child phase and its existing v3 events. The CLI machine names are
`video_music_video` and `video_music_video_status`.

Parent #35 retains remaining assembly acceptance, broader media profiles and
qualification. [#107](https://github.com/egohygiene/aniflow/issues/107) owns
broader manual/review recipes. The fixed recipe does not extract initial frames,
automate approval, perform destination copies (#83) or add optional provenance
proofs (#82). No immutable release is published by this checkpoint.
