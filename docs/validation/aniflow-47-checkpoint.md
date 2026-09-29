# Aniflow #47 checkpoint

## Scope checkpoint — 2026-09-29 UTC

PR #58 is merged. This branch starts from fresh main
`8ec3488a6a4cea6085af0bfc47610c0f0ad2c72a` and implements only
[audio checkpoint 6/10 (#47)](https://github.com/egohygiene/aniflow/issues/47)
under the [#13 mini-roadmap](https://github.com/egohygiene/aniflow/issues/13).

The bounded deliverable is a loss-aware typed text interchange and conversion
API/CLI for plain lyrics, centisecond LRC, basic SRT, WebVTT, flat TTML, and a
normalized JSON transport. It preserves Unicode, authored cue order, exact
supported timing, and explicit unreviewed/observed/reviewed provenance. Conversion
must refuse invented timing and unsupported syntax, and require explicit approval
for each known semantic loss. Caller-supplied review attestations are not
independently verified human review.

File conversion reads a bounded source without changing it and exclusively creates
a new output directory containing the payload, normalized input/output documents,
and a digest-bearing completion report written last. All fixtures are synthetic;
this checkpoint does not inspect or modify real media, invoke models, download
weights, or qualify a release.

Implementation is split into independently owned core, legacy codec, XML/WebVTT,
schema, documentation, and fixture lanes. The root owns the CLI and integration.
This scope checkpoint is intentionally a draft: implementation and local checks
are pending. Follow-up commits will record implementation and validation evidence.

## Gates and next boundary

Run meaningful Rust, schema, documentation/catalog, and synthetic CLI checks.
Do not wait for or poll hosted CI. Report native macOS and other unverified gates
without release qualification claims. Stop for the user's review/merge after this
issue; transcription #48 and alignment #49 remain separate follow-up checkpoints.
