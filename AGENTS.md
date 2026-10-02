# aniflow agent instructions

Read [.github/copilot-instructions.md](.github/copilot-instructions.md) for
architecture, implementation and normal validation requirements. Read root
[CONTINUITY.md](CONTINUITY.md) for the current operational handoff and verify
its mutable claims before choosing work.

For release behavior, read [.egohygiene/release.json](.egohygiene/release.json),
[the release guide](docs/releases.md), and the immutable policy references in
[release-policy-lock.json](.egohygiene/release-policy-lock.json). Cargo's
`package.version` is the sole product version authority. Task `release:publish`
prints a manual handoff and never publishes itself.

The current implementation-first work defers tests, builds, lint, formatting,
schema checks, native execution and hosted qualification to
[#64](https://github.com/egohygiene/aniflow/issues/64) by maintainer instruction.
Author useful checks without calling them passed. Preserve the known alignment
cancellation failure and historical receipts. Use synthetic fixtures only.

Implementation is delivered through checkpoint issues and draft PRs. Explicit
session authorization controls merges, checks and publication. An issue or a
handoff cannot grant additional authority.
