# #37 bounded native chunks checkpoint

## Source and authorization boundary

Base main: `2add9a6b230aaf415288e6c128bcf276dc05681c`.
Branch: `codex/37-upscayl-chunks`; [draft PR #127](https://github.com/egohygiene/aniflow/pull/127).
The initial saved head is `987f97f6fcb226f3d2f2163889a58159a046d7a8`, with tree
`905b4c94d0e74ef5e5d1790d91757668241965e0` exactly matching the reviewed local
snapshot. The explicit native-capture follow-up comes after that save; re-query
the draft and published head before continuing. The maintainer authorized merging repair PR #125 and per-frame
recovery PR #126, then implementing bounded chunks. PR #125 merged at
`7b3aa0fc671d853518e382d74f717945940be337`; PR #126 merged at the base above.
Their original handoffs remain dated source evidence, including their then-draft
state. This document does not predict its own containing commit.

All tests/compiler/build/check, lint/format, schema/naming/drift/docs validators,
native/provider/CLI/media/model execution, dependency resolution/installation,
packaging/smoke and hosted qualification remain deferred under #64. No Actions
are dispatched or polled. Commit with disabled hooks and `[skip ci]`. Source
review and authored synthetic coverage do not establish passing execution,
native support, release readiness or safe processing of real/private media.

## Authored boundary

The public `upscayl_recovery` facade and `video upscale-frames` retain their
existing plan/fresh-run/resume surface. Configuration defaults to `per_frame`
with its 1,024-frame bound. Explicit `bounded_chunk` uses 2–256 frames per
native directory process, at most 1,024 chunks and 100,000 total frames. The
complete source index, 64 MiB plan limit and storage policy remain independent
admission gates. At 64 frames per chunk, 5,602 frames create 88 processing stages
plus one collector, with a final 34-frame chunk.

Each chunk has a directory output and a report under one ordinary immutable v3
stage checkpoint. Input PNGs retain separate source bindings; complete reports
bind every ordinal, name, rational source time and digest. Native output must
have exact membership, expected dimensions, valid PNG structure/CRC and actual
full FFmpeg decode. The final collector requires the exact same-run chunk
producers and complete source mapping. Loose reports or PNGs cannot substitute
for accepted history. Original source timing is retained in the shared spatial
output index with derived geometry recorded separately.

Only complete compatible chunks reuse. Interrupted, failed or invalid chunks
remain pending as a whole and receive one fresh attempt on explicit resume.
Partial files stay retained failure evidence; no per-frame checkpoint inside
an unaccepted chunk is inferred. Independent chunks preserve later accepted
work when an earlier chunk is retried. Source, model, binary, settings, mode,
chunk size, timeout or native capture-bound changes require a new exact plan/run.

The native timeout is explicit: 30 minutes by default, bounded at 6 hours.
Native stdout and stderr capture are each bounded explicitly, default 16 MiB
and range 1 KiB–64 MiB, to accommodate the pinned interface's per-tile progress.
Exceeding the configured cap fails the chunk. These settings share the existing
isolated process runner and outer provider group, while FFmpeg/audio capture
and inspection/decoder timeout admission stay unchanged at 120 seconds or less. The
outer provider invocation timeout continues to bound processing plus decode
and copying. No new process implementation, scheduler or checkpoint store is
introduced. Timeout/cancellation evidence does not diagnose OOM from SIGKILL.

Frame progress advances by a completed/reused chunk's cardinality. Failed
chunk frames remain pending; no native stdout-derived partial progress is
accepted. ETA uses durations and frame counts of newly accepted units in the
current call, excluding reused chunks. Zero pending processing frames may
still precede collector acceptance.

## Resource admission

Append-only manifests repeat stage records, with growth tied to the square
of processing-unit count. The source plan, operations and reports separately
retain frame-level mappings. The conservative metadata budget accounts for
both counts, roughly `(units + 1) × (frames + units + 1) × 16 KiB`; it does not
claim every manifest stores the source inventory. Chunk directory/report outputs
reduce stage-output growth. Complete-set artifact counts accommodate the selected
frame total; byte and caller-supplied provider limits remain explicit.

Working-data admission includes retained output copies, source/input data,
reports, metadata, component snapshots and private work. At 5,602 frames, three
copies under the default 64 MiB maximum PNG size alone need about 1.03 TiB. The
guide uses an explicit 8 MiB maximum to reduce that component to about 131.3 GiB,
within the 256 GiB default policy only if the remaining data and overhead fit.
Outputs exceeding the selected maximum are refused. Storage observations are
not reservations, process/GPU memory quotas or a promise of no extra storage.
Resume checks new metadata and immediate operation needs rather than charging
all retained output copies a second fresh-run allocation.

## Source evidence and deferred work

The pinned upstream interface remains
`upscayl/upscayl-ncnn@0beb39028a0ddd83250e845b4c3333c0675e3b97`. Source inspection
shows directory mode requires both directories to exist and emits filename
stems with the requested extension. The wrapper supplies bounded exact `.png`
sets in private relative directories and reuses existing binary/model/decoder
pins. Source-derived interface facts do not qualify installed tools, drivers,
models or the native backend.

Ten new chunk regression cases and two contract cases are authored but unrun.
Root and independent source review, including the explicit native capture
follow-up, found no remaining concrete blocker. No executable or native
qualification is established. After the maintainer
lifts #64's execution deferral, qualify the exact published candidate:

1. Compile and exercise policy bounds, complete/tail chunk mapping, plan scale,
   provider/configuration contracts, progress weights and resume compatibility.
2. Verify native directory argv, membership, complete decode, input immutability,
   pinned model/binary snapshots and native 4× to requested 2× dimensions.
3. Inject a partial chunk, nonzero/zero-exit invalid output, CRC/truncation,
   changed pins/settings, missing accepted output, cancellation, timeout,
   concurrent writers and storage failure. Confirm complete-chunk reuse and
   whole-chunk retry without loose-output adoption.
4. Qualify selected actual Apple Silicon/NCNN and other GPU/backend environments
   separately from hermetic substitute executables. Review resource use and
   temporal/tile consistency without inferring an enforced memory quota.

Parent #37 remains open for its full failure matrix, explicit historical-output
adoption and native qualification. The legacy native directory-batch adapter
is unchanged. Downstream video/audio assembly remains #35/#110. The known
alignment cancellation/start-marker failure remains unresolved with cause
unproven, and historical receipts remain preserved.

See the [guide](../upscayl-recovery.md),
[original per-frame handoff](aniflow-37-checkpoint.md) and
[retained roadmap](https://github.com/egohygiene/aniflow/issues/117).
