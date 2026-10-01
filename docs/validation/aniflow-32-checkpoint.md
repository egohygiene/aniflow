# aniflow #32 implementation checkpoint

Issue: <https://github.com/egohygiene/aniflow/issues/32>.
Base: `1b1d8c778d9eac04bb1c1e2376ac577ef35537dc`, merged PR #63.
Status: implementation in progress; early draft savepoint.

## Scope

Replace implicit first-stream and average-rate assumptions with a public exact
temporal model, bounded FFprobe observations, explicit stream selection,
machine-readable supported/refused profiles and derived-artifact source-time
bindings. Keep FFmpeg a direct-argument adapter and retain the ordered pipeline.

Inspect rational rates/time bases, presentation and decode timestamps, frame
durations, source offsets and all stream classes. Accept only processing profiles
whose timing can be preserved and checked; refuse unsupported VFR, offset or
mixed-stream transformations rather than silently retiming or dropping streams.
Add generator-first synthetic success/refusal fixtures and focused regression
tests. Do not add layered delivery gates (#33), cross-run reuse (#34), sibling
dependencies, model inference or codec-completeness claims.

## Execution direction

The maintainer explicitly deferred tests and broad check execution to conserve
work credits. Tests, compilation, formatting checks, Clippy, smoke, schema checks,
docs/package qualification and hosted CI polling are **not run** for this
implementation pass. Useful tests are authored, not claimed passing. Deferred
validation and the prior cancellation CI failure remain in
[#64](https://github.com/egohygiene/aniflow/issues/64).

Synthetic recipes only; source media is read-only and no real media, model
download, paid API, merge, tag or release is authorized for this checkpoint.
Push implementation and final documentation to the same draft PR. The maintainer
reviews and merges. Next issue after this prerequisite merges:
[#33](https://github.com/egohygiene/aniflow/issues/33), layered validation and
evidence-rich delivery.
