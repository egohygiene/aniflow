# Resume and finishing trial reconciliation

November 13, 2026 (America/New_York). Relates to [#228](https://github.com/egohygiene/aniflow/issues/228)
under [#212](https://github.com/egohygiene/aniflow/issues/212). This is a source
coverage review of the two original trials against product tree
`b3354cfc83ee53f9d168b8aa20f29884748d3249`, observed at main
`e6f3cd4d272d42f63aef034aeac25c22c3567202`. The containing PR identifies this
review's candidate revision. Nothing here declares a native run accepted.

The useful mechanisms already have product owners within their documented
admitted profiles. The small accepted change is to correct the inventory guide's
stale claim that no CLI exists. `video upscale-recovery inventory` and `progress`
already dispatch to the read-only library owners; no additional command,
processor, model default or finishing engine is needed.

## Original sources retained as reference

These immutable links preserve the original bytes and context. The scripts were
read as data, not executed, installed, imported or copied into a runtime owner.
The accompanying [review record](228-reconciliation.json) binds both script
observations to the original archive manifest and named product symbols.

| Trial at archive commit `16aa7cec698a6de2c6e7945a751400d65e797c89` | Bytes | SHA-256 |
| --- | ---: | --- |
| [resume-upscale.py](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/resume-upscale.py) | 12002 | `061c21b31776a2fdd199417044d3724e71bb77cd83617f666f8c2af49733952c` |
| [finish-video.py](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/finish-video.py) | 19514 | `9ca70a2e58e4402667145dad5818a9259a137834d4a959bbe1809ae33f6982ea` |

Both fetched byte counts, SHA-256 values and Git blob identities were checked
against the [original seven-file manifest](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/manifest.json).
Only these two trials are reviewed by #228; the other five remain with their
existing intake owners. The archive need not exist in the current main tree to
identify its exact original input.

## Coverage and decisions

“Covered” below means that the named source owner expresses the useful contract;
it does not mean the current binary, tool, model or media was qualified.

| Trial idea | Current owner | Reconciliation decision |
| --- | --- | --- |
| Original source hash and complete ordered PNG inventory | [frame import](../../frame-import.md), [frame-file observations](../../../src/frame_io.rs), [recovered recipe](../../recovered-music-video.md) | Covered. Exact membership, content identities, original ordinals and temporal inspection remain required. A filename or mtime cannot establish identity. |
| Resume missing frames while retaining successful work | [Upscayl recovery](../../upscayl-recovery.md), [recovered recipe](../../recovered-music-video.md) | Covered by compatible run-local checkpoints and explicit bounded resume. Reuse rechecks exact source, tools, model, configuration and accepted outputs. No implicit retry or settings change. |
| Detect incomplete/corrupt saved output | [frame observations](../../../src/frame_io.rs), [per-frame provider](../../../src/upscayl_recovery/provider.rs), [chunk provider](../../../src/upscayl_recovery/chunk_provider.rs) | Covered within admitted PNG profiles by structural checks plus bounded native full decode and acceptance. Trial CRC/header checks alone cannot replace pixel decoding or checkpoint validation. |
| Show saved/pending work and remaining-time advice | [inventory](../../upscayl-recovery-inventory.md), [progress CLI](../../recovery-progress.md) | Covered; the inventory guide is corrected here. Counts describe retained state. Conditional timing uses exact successful provider reports and excludes collection/retries; it is not a deadline or fresh acceptance. |
| Retain useful historical images without rerendering them | [historical import](../../../src/upscayl_recovery/historical.rs), [complete carrier](../../complete-spatial-carrier.md) | Covered through explicit selected-file hashes, review and declared history. A trial's `video-frames/1` manifest or existing PNG must not be adopted as an aniflow native checkpoint. |
| Bound work, free-space admission and private candidates | [recovery configuration](../../../src/upscayl_recovery/mod.rs), [recovered recipe](../../../src/music_video_recipe/recovered.rs), [assembly](../../video-assembly.md) | Covered through explicit finite limits and existing writers/private workspaces. This is not an OS disk reservation or RSS/VRAM guarantee. The trial's rename/link replacement of invalid old output is not adopted. |
| Stitch complete frames at their original clock and preserve audio | [video assembly](../../video-assembly.md), [assembly media evidence](../../../src/video_assembly/media.rs) | Covered for selected supported streams and exact rational timing/packet/configuration/priming observations. The trial's tolerance and arbitrary multiple-audio-stream behavior are not new product promises. |
| Keep a separate lossless master and lossy delivery | [video assembly](../../video-assembly.md) | Covered by the existing admitted master/delivery profiles. Lossless means relative to admitted PNG pixels, not proof of the original creative history. The trial's optional FFV1/MKV choice stays reference-only. |
| Explicit title/creator metadata and copy-only sanitation | [metadata policy](../../delivery-metadata.md), [accepted remux](../../metadata-remux-cli.md), [finishing](../../recipe-finishing.md) | Covered by scoped, explicit policies and packet/timing verification. No universal privacy cleanup, watermark removal, authorship or rights claim is inferred. |
| Loudness/true-peak observation after all byte changes | [final-video audio QC](../../final-video-audio-qc.md), [publishing tail](../../publishing-recipe.md) | Covered within the documented audio profile by exact-final-file measurement and bound receipts. Typed unavailability, meter qualification and output targets stay explicit; no automatic gain/normalization or universal loudness verdict is adopted. |
| Checksums, reports and posting preparation | [final handoff](../../final-video-handoff.md), [publishing tail](../../publishing-recipe.md) | Covered by exact artifact evidence and separate final-file review/manual packaging. Trial `status: verified` or process success cannot replace runtime acceptance, creative approval or publication authority. |

## Trial choices retained only as reference

- Project names, default `.frames`/`.frames-4k`/`release` locations, model names,
  the macOS application model path, tile/thread settings, scale, CRF and preset
  are trial configuration. Current tools/models must be explicitly selected
  and pinned. Nothing installs or downloads a processor/model.
- The trial's legacy extraction schema is not an aniflow frame-index format.
  Select the existing source/import entry, or review and import historical
  frames through its explicit owner. Do not rename frames to manufacture lineage.
- Arbitrary installed FFmpeg versions, arbitrary audio-stream layouts and
  FFV1/MKV output are outside the current closed profiles. Assembly and final
  QC retain their documented stream/rate/channel/container restrictions.
- Moving aside an invalid destination and linking a replacement does not become
  permission to mutate an existing media tree. Existing private candidates,
  preserved failures, no-clobber output and explicit recovery owners remain.
- Manual SRT remains supplied/reviewed input. Neither trial introduces an
  automatic lyric detector or an inferred content/release decision.

## Acceptance and evidence boundary

The issue's three intake decisions are recorded: useful mechanisms are mapped
to existing owners, the small documentation gap is repaired, and source/profile/
qualification limits are retained. No additional implementation gap was
established from these trials, so no new follow-up issue or duplicate finisher
is created. Broader product/media qualification remains with #212 and the
original final flow #88 pass; those criteria are not closed by this record.

Executed for this review: original byte-count/SHA-256/Git-blob comparisons,
JSON parsing of the review record, existence/symbol checks for its named source
owners, relative documentation-link checks, the existing product-name check
and Git whitespace inspection. These are source/document observations only.
Both archive sources, 13 named owners and 28 relative links verify. The existing
product-name check exits 1 on 182 baseline mixed-case references; comparison
against the reviewed base finds the same 182 and no newly introduced violation.
This checkpoint does not relabel that policy gate green or broaden into a
repository-wide naming rewrite. Git whitespace inspection passes.
No trial, native Upscayl/FFmpeg, model, original media, Rust product compilation,
Rust test, hosted workflow, release, signing or publication ran for #228.
The independent synthetic pilot has its own exact source/result receipt;
this coverage review does not substitute for it. The known alignment
cancellation/start-marker failure remains unresolved with cause unproven.
