# Immutable release policy inputs

These unmodified, MIT-licensed data snapshots support offline declaration and
profile inspection. Each owner directory contains its original license.
`.egohygiene/release-policy-lock.json` records the owning repository, full
revision, canonical source path, local path and SHA-256. The local checker
rejects snapshot drift; updates require review of both data and the lock.

- Aether owns the normative `egohygiene.repository-release/v1` schema. Its
  revision matches the contract pin inside Hygiene's policy.
- Hygiene owns applicability; its `1.0.0-alpha.1` policy remains **proposed**.
- Relay owns the binary profile and immutable publication. Its source pin is
  `04bd32c8ef492418f47d6df6faee425d6888f341`, including product release naming.
  This is a reviewed immutable source reference, not a claim of a newer Relay
  release. Relay's latest observed release was v1.5.0 on 2026-10-02.

The Aniflow checker is a repository-specific consistency gate. It does not
replace Egolint or claim that Egolint #29's unfinished conformance rollout is
complete. The shared publication workflow is called at the pinned revision;
its implementation is not copied into this repository.
