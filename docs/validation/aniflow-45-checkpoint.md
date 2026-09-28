# Stem lineage checkpoint (#45)

Parent mini roadmap: [#13](https://github.com/egohygiene/aniflow/issues/13).
This is checkpoint 4/10, following merged normalized audio, technical inspection,
and offline separation contracts. PR #56 (the high-rate signal follow-up) merged
before this work; the fresh base is
`92d99afa624db57c3484f37bd9ad563d427da15c`.

## Checkpoint 1: bounded contract and integration

- Refreshed main and read repository and provider instructions before branching.
- Re-read #45 and the live parent and Flow suite handoffs. Historical roadmap
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
