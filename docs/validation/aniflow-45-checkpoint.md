# Stem lineage checkpoint (#45)

Parent mini roadmap: [#13](https://github.com/egohygiene/aniflow/issues/13).
This is checkpoint 4/10, following merged normalized audio, technical inspection,
and offline separation contracts. PR #56 (the high-rate signal follow-up) merged
before this work; the fresh base is
`92d99afa624db57c3484f37bd9ad563d427da15c`.

## Checkpoint 1: bounded contract and integration

- Refreshed main and read repository and provider instructions before branching.
- Re-read #45 and the live parent and flow suite handoffs. Historical roadmap
  snapshots are not authority for current merge or release state.
- Preserve the existing Pipeline v3 runtime, source snapshots, immutable
  artifacts, cancellation, and run-local checkpoint acceptance.
- Consume declared stem identities and artifact roles through a provider-neutral
  boundary. The #8 adapter declared vocals and accompaniment, not drums or bass.
- Bind the original mix, selected stem, scope, source/provider identities and
  relationship evidence; independently verify bytes and bounded PCM timing.
- Reject absent, ambiguous, stale, mismatched, escaping or unsupported mappings.
- Use only generated synthetic fixtures. Do not download or execute Demucs
  models or mutate real media.

Implementation, schemas, refusal fixtures, documentation and validation remain
in progress at this initial pushed checkpoint. No new capability is qualified by
this document alone. Hosted CI is not a blocking or repeatedly polled gate for
this task. The maintainer reviews and merges the completed PR.

## Checkpoint 2: implementation saved for continued validation

The public inspection request now accepts an explicit retained separation run,
stage and stem artifact. Both technical and signal workflows append the same
native lineage provider through Pipeline v3. The original measurement reports
remain intact; the final normalized analysis and a versioned lineage companion
retain the mix, stem, full channel/frame scope and separation authority.

The importer verifies the latest complete run, its direct-input separation
stage, exact plan/checkpoint/lock/report/validation identities, confined regular
output files and independently parsed PCM clocks. It accepts provider-neutral
declared roles, with no model execution or provider-specific evidence parsing.
The accepted historical authority snapshot is bound into the new plan and
rehashed before publication. It is not continuous monitoring of source-run
manifest revisions.

The v1 profile accepts the whole stem and all channels, with an exact duration
tolerance of at most 20 ms. Partial selections, transformed source mappings and
unsupported clocks refuse. Duration agreement is not measured onset or phase
alignment.

At this pushed checkpoint, the library compile and strict library Clippy pass,
as do both CLI selection refusal tests, product names and whitespace checks.
Integration/refusal fixtures, independent schema validation, complete local
checks and the final receipt remain in progress. This draft is not ready for
merge or release qualification.

## Checkpoint 3: synthetic acceptance and contract review

The seven public-library integration tests pass, covering the actual typed #8
adapter with fake model dependencies, a non-Demucs separator with its own
declared roles and opaque evidence, exact duration boundaries, unsupported
scope, stale/missing/escaping evidence, cancellation and unchanged sources.
Four real-FFmpeg CLI stem inspections pass with read-only status and checkpoint
reuse. All eight emitted lineage/normalized reports independently pass their
published schemas.

The all-target run passed 213 tests. Three final contract regression tests were
then added; all five focused stem units pass. All fourteen focused stem tests
(five units, seven integrations, two CLI tests) pass on Rust 1.85.1. Strict
all-target/all-feature Clippy, formatting, product-name checks, 50 published JSON
documents, eight independent schema tests, documentation and source-package
verification pass. Review corrections aligned public-output aliases, capability
attachment, opaque evidence typing and exact authority bounds.

The complete repository smoke is the remaining local check at this checkpoint.
The final receipt records its outcome and exact implementation identities.
Native platforms, real-model inference/quality, hosted CI and release
qualification remain unverified.
