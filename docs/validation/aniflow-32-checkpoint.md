# aniflow #32 implementation checkpoint

Issue: <https://github.com/egohygiene/aniflow/issues/32>.
Base: `1b1d8c778d9eac04bb1c1e2376ac577ef35537dc`, merged PR #63.
Status: implementation and regression coverage authored in draft PR #65;
qualification intentionally deferred. The maintainer owns review and merge.

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

## Delivered implementation

- `src/temporal/`: reduced signed rational arithmetic, inventory/frame/packet
  observations, explicit stream selection and typed processing refusals.
- Public library and CLI `temporal inspect`, plus selection flags for inspect,
  plan, run and short-segment planning/execution.
- Exact CFR frame-grid reconstruction, selected A/V clock checks, source identity
  rechecks, frame/audio/video/master bindings and selection-aware resume.
- V2 segment plans/manifests/reconstruction reports with exact source windows,
  observed output clocks and refusal of unsupported legacy resume state.
- Five closed temporal/segment schemas and the typed machine-envelope extension.
- Thirty generator-first protocol fixtures, twelve focused contract tests and
  two native synthetic FFmpeg integration tests authored for the later audit.
- Guides, README, existing temporal ADR outcome and roadmap handoff updated.

Early scope checkpoint: `6844651`. Core implementation checkpoint:
`c5c343b8e46abe0bf8ff3c3a500feb20074137e8`. Final source/document identities are
recorded in [the implementation receipt](aniflow-32-implementation.json) and PR
#65; they do not establish test results.

Static inspection during completion added rechecks for frozen selection intent
and duplicate stream/timeline identities. Video-only and authored-subtitle
projections retain matching selection intent. Native regression recipes cover
fractional segmentation/resume/reconstruction and explicit multi-video choice.
No test, build, compiler/check, schema check, full smoke or package qualification
was run. No real-media operation, model download, paid API, merge or release.

`#33` remains next after this draft merges; `#64` retains accumulated validation
and the earlier hosted cancellation failure. #32's check-related acceptance
remains explicitly unqualified instead of being marked green.
