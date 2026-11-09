# Selected destination-video export [skip ci]

`destination::video::intent` and `delivery video-export intent --request <FILE>`
read one current accepted native video and plan explicit geometry/encoding.
There is no tool/provider execution in this intent checkpoint. The machine
command is `delivery_video_export_intent`, with data schema
`aniflow.destination-video-intent/v1`. Request/configuration schemas are
`aniflow.destination-video-request/v1` and
`aniflow.destination-video-configuration/v1`.

Select a current accepted FFV1 RGB8 clean master or H.264/yuv420p destination
copy. The first lane is square, unrotated BT.709 SDR with the complete original
CFR clock, at most 100000 frames, and optional unchanged 48 kHz mono/stereo
AAC-LC. Subtitle-bearing sources refuse until an explicit preserving profile
exists; selecting an uncaptioned clean master does not erase another derivative.
Keep the clean master separately. This plans a lossy video candidate, not an
improvement, full creative approval or platform qualification.

Choose one closed geometry rule:

- `exact_aspect`: exact even dimensions with the source aspect ratio.
- `fit_within`: largest representable exact source aspect inside even bounds.
- `fit_and_pad`: exact canvas with explicit opaque black padding around fitted
  content. Chroma-aligned offsets can differ by two pixels across opposite edges.

Upscaling requires explicit `allow_upscale`. No crop, stretch, duration trim,
frame-rate conversion, audio gain, resampling or alternate model is inferred.
Exact integer aspect can use slightly fewer pixels than conventional rounding:
1920×1080 within 1080×1920 resolves to 1056×594. An already-portrait 2160×3840
source reduces exactly to 1080×1920. Source and output geometry stay distinct.

Configuration reuses closed `AssemblyConfiguration` tool/CRF/preset/resource
settings and requires an explicit descriptive metadata policy. Supply dated
manual TikTok requirement evidence and an evaluation date; stale/contradictory
requirements or unsupported full source clock/audio extents refuse. These are
operator declarations, not a live platform lookup. Actual file-size checks,
output validation and human review occur after a separate native export.
`recheck` requires the exact current source/configuration/geometry intent.
The machine result contains private source/tool paths and must not be telemetry.

Six geometry/bounds regression cases and one parser case authored, uncompiled/
unrun. All tests/builds/checks/native/media/provider/model/CLI/SDK/CI remain
deferred to flow #88, scheduled last. ADR-0017 remains proposed; broader #83
export/native/platform acceptance remains open. Known alignment cancellation/
start-marker failure remains unresolved, cause unproven. No actual media,
creative decision, export or post occurred.
