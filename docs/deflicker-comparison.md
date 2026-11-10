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
Also supply the exact `provider_executable`. The selected excerpt is at most
ten seconds, uses the original CFR grid and has no arbitrary filter strings.

Initial selected source scope is accepted unrotated square-pixel H.264
`yuv420p`, BT.709 SDR limited range, at up to 8,294,400 pixels. Unsupported
color/geometry/source profiles refuse instead of converting a creative look.
The preview performs an explicit limited-range BT.709-to-full RGB display
conversion on both branches; no LUT, grading, sharpening, crop or stabilization
is added. It preserves original frame ordinal/source-time mapping. Deflicker
runs within the selected excerpt; edge-window effects need visual review.

```bash
aniflow --output json video deflicker plan --request comparison.json
aniflow --output json video deflicker run --request comparison.json --run-directory /private/compare
aniflow --output json video deflicker status --run-directory /private/compare
aniflow --output json video deflicker resume --run-directory /private/compare
aniflow --output json video deflicker record-review --run-directory /private/compare --declaration supplied-review.json
```

Plan observes current source/acceptance/components and finite whole-work
admission only. A selected run uses one fixed ordinary native v3 child. The
worker snapshots exact source/tools, runs closed before/deflicker PNG arguments,
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

Six SDK cases and one CLI case are authored, uncompiled/unrun. All tests/builds/
checks/native/provider/media/browser/CI execution remains deferred. No private
Akashic assets or actual visual approval were used. Flow #88 stays last with
its original ledger unchanged; known alignment failure remains unresolved with
unproven cause. ADR-0021 remains proposed.
