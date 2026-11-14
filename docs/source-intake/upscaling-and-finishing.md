# Upscaling recovery and finishing prototypes (#228)

Disposition: reuse the existing recovery and delivery owners. Keep both original
scripts as references through the [source index](README.md), rather than add a
second checkpoint runner or finishing engine. This completes the lightweight
[intake](https://github.com/egohygiene/aniflow/issues/228); it does not establish
native acceptance of the larger [#212](https://github.com/egohygiene/aniflow/issues/212)
workflow.

## Useful behavior and its current owner

| Observed trial behavior | Existing owner and disposition |
| --- | --- |
| `resume-upscale.py` requires a complete `video-frames/1` manifest and an exact eight-digit, one-based PNG sequence. It checks signature, chunk CRCs, RGB8 geometry and a complete ending before skipping files. | [Frame import](../frame-import.md) and [recovery](../upscayl-recovery.md), implemented in [`frame_import`](../../src/frame_import/mod.rs), [`frame_io`](../../src/frame_io.rs) and [`upscayl_recovery`](../../src/upscayl_recovery/mod.rs), already own exact inventories, source/index bindings, structure checks and full decode. Keep the trial's corruption checklist; do not weaken full decode to CRC-only acceptance. |
| Resume invokes one Upscayl process per pending frame, explicit native/final scales, a tile size, and `1:1:1` workers. It checks model companion files and free space. | Recovery has per-frame and bounded-chunk execution, binary/model pins, explicit tile/device/worker settings and finite admission. Trial defaults (model, 128-pixel tile, scale and application path) are examples, not project requirements. Model/configuration identity must remain in checkpoint compatibility. |
| A nonblocking directory lock, private temporary render, recheck, retained invalid predecessor and no-overwrite hard-link publication protect work. | Ordinary [v3 runtime](../../src/provider_runtime.rs) and recovery provider own writer authority, cancellation, private candidates, acceptance and retained failures. Reuse those mechanisms; do not add the trial's `.upscale-resume` state as a competing authority. File existence alone cannot authorize reuse. |
| Existing output pixels cannot reveal which model or settings produced them; CRC validation does not detect black images or temporal artifacts. | [Historical import preparation](../historical-import-preparation.md) and [selected historical import](../historical-spatial-import.md) explicitly preserve unknown historical processing, current decoder evidence and manual selection. Do not invent a native processing receipt for a loose PNG. |
| `finish-video.py` binds the extraction source SHA, complete image set, output geometry and CFR clock; it checks frame size/mtime during encoding. | [Assembly](../video-assembly.md), [`video_assembly::media`](../../src/video_assembly/media.rs) and source-entry recipe own exact source/frame identities, clocks, accepted inputs and native observations. Retain the trial's independent source/frame recheck principle; its header/end-marker and stat checks do not replace current digest/decode evidence. |
| Delivery is lossy H.264/yuv420p, with optional separate FFV1 RGB master. Copy-only metadata cleanup compares packet hashes and time-base-scaled PTS/DTS/durations, then original audio payloads. | Assembly already has distinct master/delivery stages; [metadata remux](../metadata-remux-cli.md), [`delivery_metadata::remux`](../../src/delivery_metadata/remux.rs) and the shared native media observer own scoped payload/timing verification. Do not concatenate an extra encoder/remux path outside accepted v3 lineage. |
| Optional title/artist, chapter/inherited-tag suppression, read-only `ebur128` loudness, checksums and a private summary accompany output. | [Metadata policy](../delivery-metadata.md), [final-file audio QC](../final-video-audio-qc.md), [`audio_signal::loudness`](../../src/audio_signal/loudness.rs) and [final completion](../final-video-handoff.md) own these responsibilities. Meter observations remain separate from normalization, musical approval and publication. |

## Differences that must remain explicit

The scripts' `video-frames/1` manifest and one-based names are not Aniflow's
versioned source/frame-index contracts. Do not rename files or fabricate an
index to make them look compatible. Select existing import/recovery preparation
and supply its required source/index evidence; an unsupported legacy extraction
needs a reviewed converter or a fresh supported extraction.

The trial finisher can copy all source audio tracks; the current selected
assembly profile admits its explicit selected AAC-LC stream or no audio. Its
source color/clock/geometry restrictions also remain authoritative. No silent
drop, downmix, SDR conversion, VFR-to-CFR substitution or claim that every
prototype input is supported follows from this mapping.

The trial's FFV1 master is MKV, whereas the existing selected Aniflow master is
its explicitly described RGB FFV1 archival profile. Do not change the container
or imply interchange without checking the existing profile. Neither means the
upscaled pixels recover lost source detail. The trial's packet comparisons and
new report do not supply provenance for unknown earlier frame generation.

Minimal metadata is scoped tag/chapter handling. It is not complete forensic
sanitation, C2PA removal, invisible signal-watermark removal or detector evidence.
Private logs/probes can contain source metadata and local paths; carry only
the explicit [public-safe completion](../final-video-handoff.md) downstream.

## Bounded next work under #212

No new product code is selected by this intake: the reusable safety and finishing
responsibilities already have owners, while adding a second script engine would
lose current authority and resource binding. Two concrete follow-ups remain:

1. If a real `video-frames/1` workspace must be consumed, add an explicit offline
   converter through historical import preparation. Require complete ordered
   membership, source digest, exact rational timing/geometry and current decode;
   keep historical model identity unknown unless independent evidence proves it.
2. Extend that owner's synthetic failure matrix for corrupt/truncated PNGs,
   model/configuration drift, concurrent writers, interrupted candidates and
   changed source/frame bytes. Qualify a real intended tool/device separately;
   inspect tile boundaries and motion before accepting mixed historical/new frames.

The source/hash/link review performed for this intake is recorded in the index.
No native tool, model, media or archived script was run; pre-existing broader
compilation and qualification limits are unchanged.
