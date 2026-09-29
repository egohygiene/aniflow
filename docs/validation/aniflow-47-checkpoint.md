# aniflow #47 checkpoint

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

## Implementation checkpoint — 2026-09-29 UTC

The public conversion API, six-profile registry, file facade, machine CLI and
Task shortcuts are implemented. Five bounded text codecs share exact timing,
Unicode, provenance and per-kind loss rules with normalized JSON transport.
Four closed schemas and synthetic examples are published. The file facade
exclusively reserves the destination, creates new artifacts, rechecks the source,
and atomically publishes the complete report last without replacing an existing
report. No source replacement or model execution was added.

Initial local evidence: 10 public API/CLI tests, 18 synthetic CLI cases, 11 schema
tests and the 62-document contract catalog pass. The smoke run retained 42
normalized/report documents plus registry/context; all 44 independently validate.
Published reviewed SRT-to-JSON fixture bytes and digests match the real CLI.
Ten focused markup tests pass after correcting XML declaration interoperability.
Initial strict Clippy, formatting and naming checks pass; the final complete
Rust suite, strict checks, source-package/docs checks and repository smoke remain
to be run against the final implementation checkpoint.

Independent review corrected TTML unknown-language emission, authored leading
BOM ambiguity, duplicate metadata keys and invented JSON loss claims. Review
found no remaining material blocker within the documented caller-controlled
filesystem boundary. This is still a draft, not release qualification.

Scope pause: #47 remains checkpoint 6/10 only. #48/#49 have not started. Resume
from this branch, complete local validation, record exact implementation/evidence
identities, push the validation handoff, and stop for maintainer review/merge.

## Validation and review handoff — 2026-09-29 UTC

Implementation checkpoint `0a70acd83273b7ae2bd415a7b879219c484f7385`
matches remote `db81893463eda8dd531fdb8ea2f67e4a6540b6c1`, tree
`6fc0f1cdae1d5633270b1ab22b4d35cddc8c963b`. Final handoff changes only
evidence/documentation and fixture temporary-path normalization; production
Rust is unchanged. See [the local receipt](aniflow-47-local.json).

- 276 Rust tests passed, with strict Clippy/formatting, product-name checks,
  11 independent schema checks, 62 published contract documents, Rust docs and
  verified source-package compilation.
- Rust 1.85.1 passed 32 focused unit and 10 public integration tests. The full
  MSRV suite was not run.
- Complete repository smoke passed: synthetic video/provider/recovery, two
  technical cases, eleven signal cases, 28 high-rate cases, four stem cases and
  eighteen timed-text cases. The unchanged optional musical analyzer was skipped
  because its opt-in environment variable was unset.
- All 44 final timed-text documents independently pass their schemas. Actual
  published SRT-to-JSON example bytes/digests agree with the CLI. Sources remain
  unchanged and refusals never silently become successful lossy conversions.
- A final fixture-only correction resolves temporary-directory symlinks before
  invoking the canonical-path converter. Ten affected Rust tests, eighteen CLI
  cases and eleven schema checks passed with a synthetic symlinked temporary
  root. This does not substitute for native macOS qualification.

Native macOS/other filesystem qualification, full MSRV, actual Task binary
invocation, power-loss/hostile-filesystem behavior, broader editor
interoperability, hosted CI and release qualification remain unverified. No real
media, model downloads, paid APIs, hosted CI polling, merge, tag or release.

Stop for maintainer review/merge of [PR #59](https://github.com/egohygiene/aniflow/pull/59).
#47 closes only on merge; #13 remains open. Refresh live GitHub, instructions and
main before #48 or #49. Alignment #49 does not require transcription #48; #50 is
independent. Later suite release/audit gates remain unchanged.
