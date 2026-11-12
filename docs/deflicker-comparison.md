# Compare one explicit deflicker excerpt

`deflicker_preview` is a separate bounded SDK/CLI comparison owner for #106.
It produces a **silent visual excerpt**, leaving the original video/audio and
full finishing/export pipeline unchanged. No restoration is selected implicitly.

Supply `aniflow.deflicker-comparison-request/v1`, one current accepted
`video_run_directory`/`artifact_id`, `first_frame`, and `frame_count` (1..240).
Omitted/null `configuration` means a saved bypass: no renderer/tool/model file
is loaded or launched. A selected configuration contains existing native
`tools` (assembly settings, exact FFmpeg/probe 8.0.1 pins, no metadata policy),
an odd `window` of 3..31 fitting the excerpt, and `mode` `mean` or `median`.
It may also contain an explicitly selected `spatial_polish` profile below.
Also supply the exact `provider_executable`. The selected excerpt is at most
ten seconds, uses the original CFR grid and has no arbitrary filter strings.

Initial selected source scope is accepted unrotated square-pixel H.264
`yuv420p`, BT.709 SDR limited range, at up to 8,294,400 pixels. Unsupported
color/geometry/source profiles refuse instead of converting a creative look.
The preview performs an explicit limited-range BT.709-to-full RGB display
conversion on both branches. Without a spatial profile there is no denoise,
deband or sharpen. No LUT, grading, crop or stabilization is added. It preserves
original frame ordinal/source-time mapping. Deflicker
runs within the selected excerpt; edge-window effects need visual review.

## Optional spatial cleanup

For retained #106 scope under [#212](https://github.com/egohygiene/aniflow/issues/212),
the configuration can add the following nested object. It is an example of
requested settings, not a recommended look or a qualified native preset:

```json
{
  "spatial_polish": {
    "schema": "aniflow.spatial-polish/v1",
    "denoise": {"strength_hundredths": 120, "patch_odd": 3, "search_odd": 7},
    "deband": {"threshold_thousandths": 20, "radius": 16},
    "sharpen": {"amount_hundredths": 50, "kernel_odd": 5}
  }
}
```

Select at least one stage; omit the others. An omitted/null `spatial_polish`
preserves the original deflicker-only filter and serialized configuration.
The SDK's `spatial_polish::Profile` validates the same closed numeric settings
and compiles filters without reading media or launching tools. New Rust
`deflicker_preview::Configuration` struct literals supply `spatial_polish: None`
to retain the old behavior; existing JSON requests need no new field.

| Choice | Accepted values | Candidate processing |
| --- | --- | --- |
| `denoise` | strength 100..1000 hundredths, patch 3/5/7, odd search 7..15 | Spatial NLMeans, matching explicit luma/chroma patch and search sizes; no temporal denoise stage |
| `deband` | threshold 1..250 thousandths, radius 1..32 | Same per-plane threshold, fixed sampling distance and direction, blurred comparisons, uncoupled planes |
| `sharpen` | amount 1..200 hundredths, kernel 3/5/7 | Positive luma unsharp amount; chroma amount zero |

Selected order is always **deflicker -> denoise -> deband -> sharpen -> the
existing shared RGB display conversion**; omitted stages disappear. The control
gets no deflicker or spatial stage. Only compiler-generated numeric FFmpeg
filters enter the existing direct argv invocation. No caller expressions, LUT
paths, shell strings or automatic balance/look choices are accepted.

Profile settings are part of the existing operation/provider/context identity.
Changed settings require a new root; an old declaration cannot approve or reject
a different context. The current renderer binary pin is still required. This
does not change the original clock/excerpt limits, native acceptance owner,
silent output or production finishing chain. NLMeans cost can be substantial;
existing time/output bounds do not establish RAM/performance suitability.
Native FFmpeg compatibility, internal pixel conversions, texture retention,
halos, banding, animation intent and display quality still need real comparison
and human review. Fixed deband sampling is not a cross-platform determinism claim.

## Existing comparison lifecycle

```bash
aniflow --output json video deflicker plan --request comparison.json
aniflow --output json video deflicker run --request comparison.json --run-directory /private/compare
aniflow --output json video deflicker status --run-directory /private/compare
aniflow --output json video deflicker resume --run-directory /private/compare
aniflow --output json video deflicker record-review --run-directory /private/compare --declaration supplied-review.json
```

Plan observes current source/acceptance/components and finite whole-work
admission only. A selected run uses one fixed ordinary native v3 child. The
worker snapshots exact source/tools, runs closed control/candidate PNG arguments,
checks exact membership/PNG geometry/CRC and completely decodes each sequence
to full RGB frame hashes on the exact cadence. Native resource/capture/
cancellation/checkpoint owners remain unchanged. Accepted compatible child
work can resume; unaccepted partial native initialization is retained/refused.
Private failures are retained. Admission estimates are not hard RAM/RSS quotas,
a hostile-process sandbox or a disk reservation. No GPU/model is required.

The accepted comparison directory has `before/`, `after/` and `preview.html`.
The local viewer plays both sequences at the original rational cadence and
supports frame selection. Browser scheduling/display fidelity is unqualified;
normal-speed inspection must consider intentional pulses, cuts, animation holds,
textures and edge-window artifacts. Source times/ordinals remain in the native
report; no audio or full-video completeness claim is made for this excerpt.

A supplied ordinary manual-review declaration binds the current
`context_sha256` and actual report `comparison_sha256`. Recording it creates no
export, rewrites no media and grants no full-video approval. Rejection remains
immutable; a changed decision/source/settings requires a new intent/root.
Status rechecks current native evidence and exact frames/viewer without tools.
This first preview does not automatically enter the production finishing chain.

SDK and CLI cases are authored, uncompiled/unrun. All tests/builds/
checks/native/provider/media/browser/CI execution remains deferred. No private
Akashic assets or actual visual approval were used. Flow #88 stays last with
its original ledger unchanged; known alignment failure remains unresolved with
unproven cause. ADR-0021 remains proposed.

Broader automatic balance, color/LUT/OCIO/look recipes, standalone/full-master
application and demonstrated quality remain unfinished in #212. This source
checkpoint does not close its restoration ledger row. See the
[source handoff](validation/aniflow-212-spatial-polish-checkpoint.md).
