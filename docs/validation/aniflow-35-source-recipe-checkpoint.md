# #35 source-entry music-video recipe checkpoint

## Source and authorization boundary

The maintainer authorized continuing the next implementation checkpoint on
October 28, 2026. This slice starts from standalone extraction draft
[PR #133](https://github.com/egohygiene/aniflow/pull/133), saved head
`9e05b05574ad03cb94137f90e3525cbf51a5ebed` and tree
`ab6c2d0b4cfb0cee7d81bc2ccc30e20a055385a7`.
Branch `codex/35-source-music-video-recipe` is saved as
[draft PR #134](https://github.com/egohygiene/aniflow/pull/134), created on
2026-10-28 at 05:53:28Z (UTC) and targeting `codex/35-source-frame-extraction` at
the saved PR #133 base above. PR #133 and PR #134 remain open drafts; neither
is merged by this continuation.

First saved implementation head: `55a0ba322ec914b486dd233e00c4166b1d40f2bb`;
tree: `e84c2383a7b4dc7d0ea54d88901e037c1573281a`. All 25 published blobs and
the exact tree matched reviewed local committed source. These are dated
first-save receipts, not a self-pin of subsequent documentation changes. The
continuity candidate revision remains null; re-query the PR live head before
continuing.

Main remains `e1634df58fa32feda23021565b032a83c5a30d66`, tree
`f70ffe58fc2b225c498feffc566baaff70203f36`, with merged assembly PR #130,
indexed recipe PR #131 and target-resolution PR #132. Their dated first-save,
review and merge receipts remain in the existing handoffs. No qualification,
ADR acceptance or release is implied by source inclusion or publication.

## Selected checkpoint

The new source configuration is `aniflow.source-music-video-recipe/v1` with
`extraction` and the original `recipe` object. `video music-video` accepts it
with source/configuration and plan/output/resume arguments, without frame
flags. Indexed callers still supply both frame arguments and their original
v1 configuration. The thin CLI routes to separate source public functions;
indexed request and result contracts stay unchanged. A common status command
routes by the recorded immutable intent schema.

One root owns a source intent and fixed `extract_frames`, `prepare_frames`,
`upscale` and `assembly` child slots. Extraction is the same ordinary v3 stage
as the standalone command. After its complete output is accepted, the
coordinator derives the existing indexed tail in memory from those exact
frames and original index. It persists no second recipe intent or nested root.
Extraction writer ownership is held while the tail consumes those outputs.

Initial planning binds the source, configuration and mandatory selected tool
identities and resolves only extraction exactly. Source support, geometry,
frame count, output dimensions and target decisions remain unavailable until
accepted extraction. The target-resolution option does not require neural
executable/model files before actual geometry selects execution; unchanged
bypass preserves existing evidence limits. Unconditional Upscayl continues to
select those dependencies up front. Downstream plans never invent future
frame hashes or replace actual accepted inputs with predicted geometry.

The source intent, plan, status and outcome use separate versioned schemas.
Existing progress and `aniflow.music-video-recovery/v1` remain shared, and the
machine command names remain `video_music_video` / `video_music_video_status`.
Status observes authoritative child state and reports pending extraction
geometry without accepting partial PNGs. It still needs recorded source/tool
identities; it is not a portable offline proof after those inputs disappear.

Existing v3 plans, provider locks, immutable snapshots, acceptance, writer
ownership and recovery remain authoritative. Accepted compatible extraction
reuses its checkpoint; interruption retries the complete extraction stage.
Incomplete/incompatible children and abandoned locks require explicit
reconciliation. A completed compatible recipe resumes without another native
source probe or extraction. Child recovery granularity remains unchanged.

The same strict FFmpeg/ffprobe 8.0.1 source and assembly limits apply: original
zero-origin CFR, supported explicit streams and clocks, opaque BT.709 SDR,
selected AAC-LC or no audio. RGB conversion evidence is not a mathematically
lossless YUV round trip, prior authenticity, native qualification or visual
approval. Extraction detects no shot cuts and does not qualify RIFE across
cuts. Source and generated outputs stay separate; retained snapshots and
accepted copies can consume substantial disk. No automatic cleanup, dependency
download, private-media execution or release is added.

## Review and deferred qualification

Root and independent source review found no remaining concrete blocker on
2026-10-28 at 05:50:33Z (UTC). Review resolved the exact extraction output-port
handoff, re-reading root intent before extraction, and reusing the shared
accepted tail status. These source observations do not establish compilation
or execution. Draft publication retains that reviewed implementation; the
first-save receipt above does not qualify execution.

Seventeen new regressions are authored and **unrun**: ten public facade cases,
five CLI cases and two exact-child cases. Existing CLI mode/recovery tests are
extended. The extraction handoff retains its separate dated 13-case
authored/unrun receipt. No passed-check or native-qualification result is claimed.

All tests, compiler/build, dependency resolution/installation, lint/format,
schema/naming/drift/docs validators, smoke/package, CLI/provider/native/media/model
execution and hosted qualification remain deferred under #64. No Actions are
dispatched or polled. Publication retains disabled hooks and `[skip ci]`.
The known alignment cancellation/start-marker failure remains unresolved,
cause unproven. Source package 0.3.0 remains unreleased. ADR-0012's source-entry
extension is explicitly proposed, preserving the historical indexed proposal
without assigning an acceptance date.

Later qualification needs synthetic source-to-extraction-to-recipe handoff,
full accepted output identities, target execute/bypass decisions, conditional
neural admission, changed-input refusals, interruption at every child boundary,
completed resume without native extraction/probing, status dispatch and native
host/tool qualification. This checkpoint records authored behavior, not those
executed results or permission to process private media.

## Remaining work

#35 remains open for broader media/audio-tail/derived-clock profiles,
loudness/metadata and complete workflow qualification. #111 retains prior
Upscayl spatial-index recipe entry and qualification. #107 retains broader
manual/review composition. #36/#37/#38/#110 retain their own remaining scope;
#83 destination copies and #82 optional exact-byte proofs remain separate.
Review this saved checkpoint and sync before selecting the next issue scope.
No parent issue closeout, immutable release or native qualification is claimed.

See the [recipe guide](../music-video-recipe.md),
[extraction guide](../video-frame-extraction.md),
[proposed ADR extension](../architecture/governance/decisions/ADR-0012-fixed-indexed-music-video-recipe.md#proposed-source-entry-extension--2026-10-28)
and [current roadmap](../../ROADMAP.md#current-source-and-work-order).
