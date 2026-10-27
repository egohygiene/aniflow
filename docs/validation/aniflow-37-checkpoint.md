# #37 bounded Upscayl recovery checkpoint

## Source and authorization boundary

Base: selective-repair draft PR125 at
`00cb3d0b76018990ef3d28eb5546d6d746eacefd`, itself based on main
`a717935b49fccf87151124a6e20efeb4281ff11c`.
Branch: `codex/37-upscayl-frame-recovery`. This is a separate stacked source
checkpoint in [draft PR #126](https://github.com/egohygiene/aniflow/pull/126);
re-query live branches and PRs before continuing. The runtime prerequisite first
saved at `8d84e971d3f7745682bbf78d678431d8fb95b0d8`, and the coherent feature at
`35b729a9b98d654f4d637c24c77463ffc023b9c6`. Regression and handoff updates follow
those saves. Exact published feature blobs/tree match the local source snapshot;
these pins do not predict the commit containing this document. PR #125 remains
unmerged as this draft's base.

The maintainer authorized #37 implementation. Tests, compiler/build/check,
lint/format, schema/naming/drift/docs validators, native/provider/CLI/media/model
execution, dependency resolution/installation and hosted checks remain deferred
under #64. No Actions are dispatched or polled. Save source commits with
disabled Git hooks and `[skip ci]`. Authored coverage is not passing evidence,
and a source review or merge does not qualify a native backend or release.

Root and independent source review found no remaining concrete blocker in the
bounded feature, including generic component preflight, collector dependency
authority, exact geometry/decode, process-group ownership and runtime integrity
scope. Authored synthetic cases cover recovery and integrity boundaries; they
remain unrun and do not establish compilation or native support.

## Authored scope

The public `upscayl_recovery` facade and thin `video upscale-frames` command
compose at most 1,024 independent per-frame stages and one final collector in
the existing ordered Pipeline v3 runtime. Each frame uses the ordinary immutable
`StageCheckpoint`; the collector depends on all frames. There is no alternate
partial-checkpoint database, global cache or process scheduler.

The original source/index are bound globally. Each frame stage receives its own
PNG input and exact frame/source/time declaration, rather than rehashing an
entire frame-directory input as its own dependency. Per-frame stages do not
depend on previous frame stages, allowing compatible later completed frames to
survive a retry of an earlier corrupt or missing frame. Existing final delivery
acceptance still verifies the complete run.

The named interface is pinned to source revision
`upscayl/upscayl-ncnn@0beb39028a0ddd83250e845b4c3333c0675e3b97`, already recorded
in [upstream evidence](../toolchain-upstreams.md). Native model scale `-z 4`
is separate from requested output `-s 2`, `3` or `4`. Explicit tile, single GPU,
`1:1:1` workers and TTA settings are bound with exact executable and `.param`/
`.bin` digests. Private short `models` paths and the upstream special anime
model filename rule are explicit. No version banner or CPU fallback is guessed.

Every input and new output is fully decoded through the shared pinned FFmpeg
helper. Output geometry must equal the source dimensions multiplied by the
requested output scale. Structural PNG/CRC checks are additional evidence,
not a substitute for decoding. The collector verifies the full mapping and
copies accepted frame outputs into a complete separate directory.
The output pixel bound admits both the native 4× intermediate and the requested
geometry. It does not enforce a native GPU/RSS quota.

The spatial output index preserves the original `source_index` verbatim and
records new output dimensions/digests. It does not alter the original temporal
inspection or create a second timing model. It is not automatically accepted
as a RIFE input index; assembly/consumer integration remains separate.

## Recovery and limits

Run and explicit resume use existing v3 writer ownership, exact plan/provider
compatibility, immutable accepted outputs, failure retention and read-only
status. Changed source/configuration/binary/model inputs require a new run;
loose historical PNGs or reports cannot become compatible frame checkpoints.
There is one execution attempt per explicit call, with no automatic retry
loop or silent setting changes.

Nested native processes inherit the outer provider process group. Existing
provider/runtime evidence owns cancellation, timeout and termination facts;
SIGKILL does not prove OOM. The bounded frame-progress view distinguishes
completed/reused/pending/failed frames from whole-run state and only derives
ETA from newly completed-frame durations during the current attempt, excluding
reused frames. Completed includes reused; pending includes failed work. Human
CLI progress uses stderr; the machine final-result contract is unchanged.

Storage admission includes retained output/evidence and the existing runtime's
quadratically growing manifest record sets at the bounded frame count. Shared
free-space observations are rechecked for frame and collector work. The default
256 GiB working-data limit and 64 MiB reserve are admission policy, not disk
reservations, enforced GPU/RSS quotas or filesystem/process isolation.

Run-local reuse means the previously accepted, unchanged bytes remain compatible
with the exact bound inputs/settings. It does not establish cross-GPU bitwise
determinism, an observed driver identity, native model suitability, temporal
visual consistency or successful artistic upscaling. Backend qualification is
unestablished; the original source relationship remains declared and historical
processing unknown.

## Deferred qualification and remaining #37 scope

Fourteen synthetic recovery regressions and four runtime integrity-scope
regressions are authored but unrun. They include explicit generic-resume model
pin rechecks and refusal to collect loose source-backed frame/report files
without the exact accepted frame-stage dependencies. No count is passing
execution evidence.

No current-source executable result is claimed. After the maintainer lifts the
deferral, use the exact published head for focused qualification:

1. Compile and run authored synthetic plan/provider/resume/contract coverage;
   keep substitute executables separate from native Upscayl/FFmpeg evidence.
2. Verify native 4× to requested 2× dimensions, complete decode, model filename
   resolution, exact argv, input immutability and output ownership.
3. Inject interruption after accepted frames, malformed/truncated/CRC-invalid
   output, zero exit with bad output, changed source/model/settings, concurrent
   writers, timeout and storage failure. Check retained good frames and typed
   failure evidence without inferring OOM.
4. Qualify actual pinned binary/model combinations on explicit Apple Silicon
   and other selected native GPU/backend environments. Observe resource and
   tile-boundary/temporal-quality behavior independently of synthetic tests.
5. Select bounded-chunk policy, historical-output import and downstream spatial
   index consumers as separate residual implementation slices.

The legacy native directory-batch adapter remains available without gaining a
new qualification claim. Parent #37 remains open for its full acceptance scope,
including the complete failure matrix and native evidence. The known alignment
cancellation/start-marker failure remains unresolved with cause unproven;
historical receipts are preserved. No release or cross-holon consumption is
qualified by this checkpoint.

See the [guide](../upscayl-recovery.md) and
[contract registry](../contracts/README.md) for the same bounded public surface.
