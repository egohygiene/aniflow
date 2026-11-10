# Read recorded recovery progress

The source checkpoint for #37 adds read-only `video upscale-recovery inventory`
and `progress`. Supply the same original source, PNG directory/index, effective
settings and existing native run as `video upscale-frames`:

```bash
aniflow --output json video upscale-recovery progress \
  --source-video "/private/original.mp4" \
  --frames-directory "/private/original frames" \
  --frame-index "/private/original-index.json" \
  --configuration "/private/upscayl.json" \
  --run-directory "/private/existing native run"
```

Use `inventory` with the same arguments when no timing projection is needed.
Both reuse the existing exact native request/plan and preserve separate pending,
active/uncertain, failed, cancelled and invalidated counts. Complete counts are
retained state, not fresh output PNG acceptance. Explicit native resume remains
the only reuse/retry owner; neither command executes or repairs anything.

`aniflow.upscayl-recovery-progress/v1` wraps the unchanged inventory contract.
Every complete unit contributes its frame weight and the duration from its
exact checkpoint-bound successful provider report. Self-validating plan,
manifest, stage/provider lock and report identities must agree. Evidence reads
are confined and bounded to 8 MiB per document / 64 MiB total and are reobserved.
Changed, missing or unrelated timing evidence refuses; it is never estimated
from filesystem modification times, an assumed start time or a different run.

Remaining serial-work time uses the frame-weighted mean provider duration,
rounded up to milliseconds. No samples or zero total measured duration yields
`null`; no remaining frame work yields zero. The estimate excludes collection,
validation, retries, contention, active-unit elapsed time and future host changes.
Provider durations can include their own setup/decoding costs. Different final
chunk sizes and runtime/host conditions can change throughput. This is neither
a completion deadline nor a memory/performance/native qualification.

Four library cases and one CLI parser case are authored, uncompiled/unrun.
Tests/compiler/build/dependency/lint/format/schema/docs/corpus/native/provider/
media/model/CLI/SDK/CI execution remains deferred. No actual review, source
replacement/disposal or release occurred. #37 remains open; Flow #88 stays last
with its original ledger untouched. The alignment cancellation/start-marker
failure remains unresolved; its cause is unproven.
