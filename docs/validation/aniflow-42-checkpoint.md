# Audio contract checkpoint

Issue: [#42](https://github.com/egohygiene/aniflow/issues/42), checkpoint 1 of
parent [#13](https://github.com/egohygiene/aniflow/issues/13).

## Review checkpoint 3

Implementation and meaningful local validation are complete in
[PR #52](https://github.com/egohygiene/aniflow/pull/52). The
[local receipt](aniflow-42-local.json) records exact checks and unverified gates.
The implementation tree was published identically to the local tested tree.
This final checkpoint changes only documentation and the validation receipt.

Stop for maintainer review and merge. After merge, refresh main and dependencies
before selecting #43 or the independent #47 lane. Parent #13 remains open.
No native-platform, hosted-CI, model-accuracy or release qualification is claimed.

## Draft checkpoint 1

Started from main `639b8ce9aac4b5c236c0d3f5b2f4a6a9e226fb6d`, after the
maintainer merged #41. Live issue and repository instruction reads confirm no
unmerged prerequisite. This initial draft preserves the bounded work plan;
implementation and validation are still in progress.

The change defines a provider-neutral `aniflow.audio-analysis/v1` document,
public Rust types, exact audio-time helpers, strict schema and semantic
validation, synthetic examples and focused refusal tests. It does not launch
analyzers or models, add audio commands, or qualify a release.

Implementation lanes:

- `src/audio_analysis.rs`: closed envelope, source identity, bounded sample
  clock, observation uncertainty, evidence references and exact timing.
- `docs/contracts/audio-analysis-v1.schema.json`, synthetic examples and
  focused contract tests: independent shape and semantic validation.
- `docs/audio-analysis.md`, proposed decision and roadmap: compatibility,
  supported boundaries and downstream provider/run checkpoint responsibilities.

## Earlier recovery instructions (checkpoint 2)

Implementation checkpoint 2 is now present in this PR: the public Rust model,
strict schema, four reproducible examples, 14 focused Rust tests, independent
Python schema tests, documentation and a proposed ADR. The focused Rust and
schema suites passed locally. Review corrected unsupported semantic-family
bindings, empty audio identities, MIDI classification, and unknown-field
refusal on tagged fieldless variants. Final repository checks and the local
validation receipt remain the next step. No hosted CI result is claimed.

1. Read the current PR description, this file, issue #42 and parent #13.
2. Fetch the PR branch and fresh main; inspect all existing changes before
   resuming. Do not recreate the branch or discard partial work.
3. Finish model/schema/example agreement and negative cases, then review exact
   timing, scope, confidence, authority and incomplete-state invariants.
4. Run the repository's meaningful local Rust, contract, documentation and
   synthetic smoke checks. Record exact results and unverified gates in
   `docs/validation/aniflow-42-local.json` before the final handoff.
5. Push implementation and final validation checkpoints to this same PR. Do not
   wait for hosted CI or merge. Stop for maintainer review before #43 or #47.

All fixtures are synthetic. Source media is never modified. Generalized stream
timing (#32), layered validation gates (#33), cross-run reuse (#34), exhaustive
corpus closeout (#24) and release qualification (#10) remain separate work.
