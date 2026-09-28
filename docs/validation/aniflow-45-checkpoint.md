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
