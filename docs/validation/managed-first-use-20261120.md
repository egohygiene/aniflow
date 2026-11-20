# Managed first-use checkpoint — 2026-11-20

This records scoped Linux x86_64 evidence for #248–250. Review the stacked
drafts [#264](https://github.com/egohygiene/aniflow/pull/264),
[#265](https://github.com/egohygiene/aniflow/pull/265) and
[#266](https://github.com/egohygiene/aniflow/pull/266) in that order.
These are source checkpoints; intended Mac/GPU and full-video qualification
criteria remain open in the issues.

| Check | Observed result | Scope |
| --- | --- | --- |
| Official acquisition | Default HTTPS transport fetched the selected Linux Upscayl executable, model and license; verified installation and offline readback passed | Tool/model byte identities and acquisition, without Upscayl inference |
| Managed RIFE | Official 20221029 / v4.6 model ran on CPU; two generated 32×32 frames at declared 24 fps produced four frames at 48 fps with the same 1/12-second duration | Real native interpolation; synthetic frame import, without video/audio or visual qualification |
| Offline RIFE resume | Retained exact configuration reused the accepted interpolation checkpoint and identical outputs, without selecting current defaults | Both initial cached-archive installation and a later installed-package run were captured |
| Normal source command | A run without `--configuration` completed extraction, frame import, one upscale chunk plus collection, FFV1 master and H.264 delivery | Real FFmpeg 8.0.1; explicitly supplied synthetic Upscayl executable/model fixtures, without neural upscale inference |

The normal source was a newly generated, explicitly tagged BT.709, 32×32,
four-frame, one-second silent H.264 clip. All resource, chunk and spatial-scale
defaults were retained; only explicit tool/model paths and silent source
selection were supplied. Both accepted output files contain four 128×128
frames at 4 fps for one second, with no audio. The master reports full-range
RGB/BT.709; the delivery reports limited-range BT.709. Its native complete-decode
evidence matches the accepted PNG RGB samples exactly for the lossless master.
Original source/tool/model bytes were unchanged and the unused managed cache
was never created for this fully manual dependency selection.

## Master color signalling correction

Actual encoding exposed a pre-existing mismatch between the declared RGB
profile and FFmpeg frame properties. Untagged admitted PNGs caused the encoder
to overwrite output color options with unknown properties. Forcing `colr`
alone emitted unspecified primaries/transfer and did not satisfy acceptance.

The [master encoder](../../src/video_assembly/media.rs) now binds the already
admitted BT.709 input/output primaries and transfer on its existing full-range
`scale` filter. A real encode then exposes `pc` / `gbr` / BT.709 in the stream
and every decoded frame, with `nclx` 1/1/0 and the full-range bit. Delivery
inherits the accepted master and exposes `nclx` 1/1/1 with limited range.
Source-profile gates, exact sample/hash checks and output acceptance remain
mandatory. The earlier failed attempts and their observations were preserved.

## Checks and evidence boundaries

The final source passed 85 integration cases across managed store, resolution,
runtime files, per-frame/chunk recovery, assembly and interpolation targets,
plus seven focused CLI/parser cases. The separately authorized real HTTPS
case passed explicitly and remains ignored in the default test invocation.
Timeout regressions now inspect actual timeout receipts and retained partial
work rather than treating planning time as a native process deadline.

Synthetic recovery tests use a small, registry-pinned dispatcher that verifies
the exact compiled executable SHA-256 before forwarding unchanged arguments.
Both live pilots invoke the actual executable directly. The final pilot binary
SHA-256 is
`01649895331b508ac81c71ca020655b10f46457286efa6ac8fa1553286514898`.
The local QA build used Rust 1.85.0 with a private development-profile override
optimizing standard SHA/digest dependencies; repository Cargo files were not
changed for this override. These timings are not production or model-throughput
benchmarks. Earlier source-pinned captures and their binary were retained
separately rather than relabelled as the corrected build.

Full-repository formatting and product-name checks match the observed baseline:
the same three formatting paths and 218 naming occurrences, with no additions
from this sprint. No hosted CI or release-matrix qualification is established
by these local checks.

Actual native Upscayl on the intended Mac/GPU, multi-minute storage/device/RSS
and failure qualification, audio-bearing publishing validation, and human
seam/motion/quality review remain separate acceptance gates. See the
[resource/recovery qualification kit](aniflow-250-resource-recovery.md).
