# #36 immutable frame-import checkpoint

## Scope and source

Base: main `9c969161542fcd95592f7ca3f02cf9266c00479f`, the merge of the
read-only selection preview in PR122.
Branch: `codex/36-immutable-frame-import`, [draft PR124](https://github.com/egohygiene/aniflow/pull/124).
Initial published source head: `211a5b16b546b3f91a4ff187d097b863acbac3c8`,
with tree `0230e150724b6fb00cd8142192defd69949ed984` matching the reviewed
local snapshot. This follow-up refines source admission and records publication.
Re-query the current branch/PR before continuing; this document does not pin its
own eventual commit or claim publication before it occurs.

The maintainer authorized continued source implementation, finishing the prior
drafts and frequent draft checkpoints. Tests, builds, compiler/check, formatting,
lint, naming/schema/drift checks, native/provider/CLI/model execution, dependency
installation/resolution, smoke/package checks and CI remain deferred. Use
disabled Git hooks and `[skip ci]` for checkpoints. No Actions are dispatched or
polled. This checkpoint does not close #36, parent #35 or qualification #64.

## Authored boundary

The public `frame_import` module implements a new contemporary import through
the existing Pipeline v3 provider ABI, plan, workspace, artifact-integrity gates
and immutable checkpoints. The fixed `aniflow/frame-import` capability receives
the original source video, exact PNG directory and existing RIFE frame index;
it produces a byte-preserved frame directory plus an observed import report.
The CLI's `video import-frames` delegates to that facade. Its `--plan` form
returns the ordinary Pipeline v3 plan without launching the decoder or writing a
workspace; execution returns the ordinary run-outcome contract.

The implementation binds actual nonempty source bytes and byte count to the
temporal inspection's source identity, checks the complete ordered filename
mapping and supported exact clock/geometry profile, and rejects directory
membership outside that mapping. PNG structure/CRC inspection and full serial FFmpeg decode are separate
observations. One raw RGB24 packet of exactly `width × height × 3` bytes is
required for each private snapshot. The original compressed PNG bytes are
copied unchanged and reobserved before completion. The facade refuses an output
parent inside any input artifact before creating a workspace. Source-video
admission hashes poll cancellation between bounded reads; cancellation checks
also separate individual frame observations. The existing general planner still
owns its own input re-observation. These checks do not promise atomic directory
handle confinement against concurrent hostile ancestor replacement.

Existing RIFE frame-file/PNG helpers are mechanically extracted into the
crate-private `frame_io` module so interpolation and import share the same
bounded structural observation and copy behavior. This does not upgrade RIFE's
existing structural checks into native decode evidence; the new import provider
owns its separate full-decode operation.

The local FFmpeg executable is explicitly pinned by path, exact version and
digest. The configured tool runs through the existing bounded process adapter;
no shell or implicit discovery/download is introduced. The source/frame,
pixel, tool capture, timeout and report limits are finite. They are admission
and observation bounds, not an OS-enforced RSS limit or filesystem reservation.
The first import report is limited to 64 MiB and embeds the complete existing
index rather than creating another input manifest.

The decoder format is grounded in FFmpeg's primary
[framehash documentation](https://ffmpeg.org/ffmpeg-formats.html#framehash) and
n8.0's immutable commit `140fd653aed8cad774f991ba083e2d01e86420c7` (annotated tag
object `a4044e04486d1136022498891088a90baf5b2775`).
[`hashenc.c`](https://github.com/FFmpeg/FFmpeg/blob/140fd653aed8cad774f991ba083e2d01e86420c7/libavformat/hashenc.c)
defines `framehash_write_header`/`framehash_write_packet`, format version 2,
packet size and cryptographic hash;
[`framehash.c`](https://github.com/FFmpeg/FFmpeg/blob/140fd653aed8cad774f991ba083e2d01e86420c7/libavformat/framehash.c)
defines media/codec/geometry headers. The code chooses raw RGB24 explicitly and
requires one complete six-column packet without side-data extensions; no
output frame-limit option masks extra output. This source pin explains the
parser, not a promise that only n8.0 may be configured or that any local build
has been qualified. Native compatibility remains unexecuted work.

## Evidence and provenance

`aniflow.frame-import-report/v1` records `historical_processing: unknown`,
`source_relationship: declared_not_verified`, the source and complete-index
digests, current provider/configuration/decoder identity, and every original
PNG and decoded RGB24 identity. The current import can have a real lock and
checkpoint because it is a new operation. It does not fabricate earlier
extraction, model, repair or provider evidence.

Matching the source digest binds the caller's declared relationship; it does
not prove the PNGs came from that video. Decode proves the admitted representation
is decodable according to the selected decoder, not original colors, authentic
authorship, unedited history, perceptual quality or final-video acceptance.
Image profile declarations remain distinct from truth about scene content.

Independent source review covered the native ABI, exact decoder argument and
framehash profile, config/schema/library/CLI agreement, output ownership and
RIFE helper extraction. The helper behavior is retained; diagnostic wording is
generalized from RIFE to frames. Source review and publication-content identity
are the available verification
in this checkpoint. Thirteen synthetic Rust regressions in
`tests/frame_import.rs` are authored for later execution. They cover closed
configuration/bounds, byte preservation and honest history, complete membership,
corrupt PNG structure, decoder packet/count/digest refusals, source identity and
size, no overwrite, changed input during decode, symlink refusal, tool pins and
version, literal decoder argument boundaries, read-only planning/cancellation,
and pre-mutation refusal of input-overlapping or symlinked output parents. Substitute decoder scripts test
the process boundary; they do not establish actual PNG decompression by FFmpeg.
No test, compiler, formatter/linter, schema, native media, package or hosted result is
claimed. The known alignment cancellation/start-marker failure remains
unresolved, with its cause unproven and historical receipts unchanged.

## Continuation

1. Keep this import and its machine/library/schema wording aligned during final
   source review. Save a draft PR and record its exact published source head in
   the issue before another feature checkpoint.
2. Implement selective repair with explicit provider/profile selection, separate
   edited outputs, exact returned index/time/cardinality, and byte-for-byte
   verification of every unselected frame. A 299-to-240 result must fail; do not
   pad, duplicate, truncate or silently discard frames to hide a mismatch.
3. Add clip-only processor splitting/reconstruction only against explicit
   segment limits and checked seams. Direct image processing remains preferable
   when a qualified provider supports it. Visible-logo providers are distinct
   implementations with distinct pins and evidence, not aliases.
4. Obtain the actual pinned `video-frames/1` schema/example before authoring its
   migration adapter. It is still absent, and no guessed compatibility is added.
5. When #64 execution resumes, compile the exact checkpoint and run the focused
   synthetic import/range/provider/machine/schema coverage, then actual pinned
   FFmpeg fixtures and native-host qualification. Include malformed/partial
   PNGs, unsupported depth/alpha/color, changed inputs, count drift, extra files,
   symlinks, interrupted work and bounded resource refusals. Keep hermetic
   substitute-provider evidence separate from native FFmpeg evidence.

No source files are edited in place, and no final media assembly, audio remux,
repair inference, automatic legacy migration, release or cross-holon consumer
qualification is included. The [user guide](../frame-import.md) and
[contract registry](../contracts/README.md) describe this same boundary.
