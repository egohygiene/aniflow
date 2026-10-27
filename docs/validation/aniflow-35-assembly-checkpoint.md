# #35 verified assembly checkpoint

## Source and authorization boundary

Branch: `codex/35-verified-frame-assembly`, based on merged main
`8cb00a323b1829de1e4b958236b6aa8529eb7045`. Named repair PR #128 merged at
`52d84f237baafae7cdde36bff40ad2ccf745d9df`; selected media setup PR #129 merged
at `e71a59ca35340c3beda0efab7838cfbc8718e4e1`; reconciled chunk PR #127 merged
at the current base. Draft PR publication metadata is pending. Do not infer a
candidate self-pin or a merge of this checkpoint from its base receipt.

The maintainer authorized these merges and continuation into #35. All tests,
compiler/build checks, dependency resolution/installation, lint/format,
schema/naming/drift/docs validators, native/provider/CLI/media/model execution,
smoke/package and hosted qualification remain deferred under #64. No Actions
are dispatched or polled. Publication uses disabled hooks and `[skip ci]`.

## Authored boundary

`video_assembly::{AssemblyConfiguration, AssemblyRequest, AssemblyReport}` and
its `plan`, `run`, `resume` operations expose the behavior through the public
library. `video assemble-frames` is the thin CLI adapter; `status-v3` remains
the existing read-only inspection surface. The fixed plan materializes
`assemble_master` then `encode_delivery`, with output IDs `lossless_master`,
`master_evidence`, `h264_delivery`, `delivery_evidence`. Existing provider locks,
bounded process execution, immutable stage checkpoints, cancellation and
acceptance remain authoritative. No new scheduler, cache, checkpoint database
or standalone mux owner is introduced.

The first stage produces a full audiovisual FFV1 version-3/bgr0/faststart MP4
master. The second reads that accepted master for H.264/libx264/yuv420p video
and copies selected AAC afresh from the original source into faststart MP4.
No-audio sources remain without audio. Both output stages compare original
AAC packet payload hashes, sizes, rational PTS/DTS/durations, flags, extradata,
channel/sample properties and skip/discard metadata. No AAC reencode,
`-shortest`, silent tail trim, padding, stretching or offset normalization is
requested. Master acceptance also compares fully decoded RGB24 frame hashes
with the processed PNGs and exact frame intervals with the source clock.
Delivery explicitly changes video pixels through BT.709 limited-range 4:2:0
conversion and lossy H.264 compression.

Original `aniflow.rife-frame-index/v1` and Upscayl spatial output indices retain
original source geometry and clocks separately from output dimensions. No
RIFE-derived clock is accepted. The index remains a declaration of the PNGs'
relationship to the source. Reports retain
`source_relationship: declared_not_verified`, `historical_processing: unknown`,
`backend_qualification: invocation_observed_not_backend_qualified` and
`media.native_qualified: false`. Current output observations do not establish
earlier extraction, repair or upscale provenance or creative quality.

## Admission and recovery

The exact native interface is FFmpeg/ffprobe 8.0.1 with actual executable pins
and fresh version observations. Source evidence is anchored to FFmpeg commit
`894da5ca7d742e4429ffb2af534fcda0103ef593`; it does not identify or qualify the
installed build. The supported source is the observed MP4/MOV family, strict
progressive zero-origin CFR, explicit BT.709 SDR, square pixels, no rotation,
closed opaque 8-bit pixel formats, and no unsupported color/transform side
data. Every decoded frame must retain the declared geometry, pixel/aspect/color
profile with zero crop. Audio is absent or observed AAC-LC with complete
packet/configuration evidence. A/V-end tolerance stays one observed video
frame plus one observed audio decode block. Long tails, offsets, VFR, HDR,
alpha, unsupported audio and derived clocks are refused before encoding.

Inputs are complete named PNG sets of 1–100,000 frames with even output width
and height, bounded file sizes, total pixels, index/plan/capture size and
working-space policy. Planning reads identities but launches no native tools
and creates no run; it cannot establish native decode or profile suitability.
Execution freshly observes source and component identity before accepting
media. Bound source/index/frame/tool/settings mutations cannot justify reuse.
Delivery binds the exact same-run master dependency; a loose MP4/report is not
accepted-stage authority. An interrupted stage retries as a whole, while a
compatible accepted master can be reused through ordinary v3 resume.

Default limits include a six-hour assembly native-call bound, 64 MiB capture,
64 GiB source, 128 GiB per output, 256 MiB per PNG, 256 GiB total frames and
2 TiB working-data policy. The conservative estimate includes private and
accepted copies, source frames/tools, reports and captures. Default output
ceilings can require considerable free storage; explicitly chosen tighter
bounds may be appropriate for a workload. This is admission, not disk
reservation, enforced RAM/VRAM limits or a native safety guarantee. It is not
in-place replacement. Shared temporal validators retain their own bounds and
snapshot media on the system temporary volume. A fresh run and each actual
provider attempt require bounded free space there as well as in the run
workspace. Fully accepted resume avoids a new attempt's allocation; these
observations remain non-reserving.

## Qualification and continuation

Synthetic coverage is authored alongside the implementation and remains
unrun. Source review is not compilation, execution or passing conformance.
When the maintainer lifts #64's execution deferral, qualify exact-source
planning/configuration/CLI/schema behavior, source/index/tool mutation,
refusals, output-set/authority tampering, pixel and audio drift, interruption,
resume and artifact corruption. Native qualification must separately cover
real FFmpeg 8.0.1 builds, FFV1 MP4 support, color conversion, libx264,
non-integer frame rates, AAC priming/sample-grid behavior, playback,
faststart atom structure and realistic storage/capture limits. Substitute
fixture output does not establish any of those native properties.

The [assembly guide](../video-assembly.md) gives a useful manual handoff from
selected repair/Upscayl setup and accepted output frames into assembly. There
is no automated full repair/upscale/assembly recipe in this checkpoint.
Parent #35 remains open for that complete workflow, broader supported media
and tail profiles, remaining acceptance and qualification. #110 retains RIFE
clock/audio handling; #83 destination copies and #82 optional proofs remain
separate. Parents #36/#37/#38 retain their prior open scope.

The known audio-alignment cancellation/start-marker failure remains unresolved,
cause unproven; no qualification claim is promoted. Retain historical handoffs,
proposed ADR statuses and their exact receipts. Source package 0.3.0 remains
unreleased; #10 owns the first qualified immutable publication. See the
[current roadmap](../../ROADMAP.md#current-source-and-work-order) and
[retained work ledger](https://github.com/egohygiene/aniflow/issues/117).
