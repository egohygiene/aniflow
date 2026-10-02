# ci: author the Rust CLI binary release convention (#10)

Aniflow had a Cargo version and historical changelog but no coherent release
declaration, prepared-candidate gate or immutable binary publication handoff.
This change implements the repository-owned release surface in checkpoints
#85–#87 while preserving the maintainer's deferred qualification under #64.

Cargo remains the sole authority. Read-only checks compare Cargo, Cargo.lock,
the promoted changelog, candidate and declared handoffs. Preparation requires
an explicitly chosen successor/date and creates a reviewable four-file diff;
the publish task only prints a manual handoff. No version bump occurs here.

The native pipeline targets Linux x86-64 and macOS arm64. It compares two clean
offline builds, normalizes archive metadata, inventories every file, and emits
source/toolchain provenance and Cargo-scoped SPDX evidence. A separate manual
candidate workflow performs synthetic qualification and signs the complete
payload inventory. Publication reuses the retained signed bytes, verifies the
source/signer identity, and calls full-SHA-pinned Relay's binary profile.

Only the publication handoff grants contents write. Current default-branch
identity, completed #64/Egolint #29 gates, explicit review, successful candidate
run, exact checksums and cryptographic signature verification are required.
No ordinary PR, push, schedule or local task publishes. The guide documents
pre-1.0 compatibility, immutable rollback and separate distribution channels.

## Qualification

**Unrun under #64.** Twenty-five synthetic test functions are authored, not
executed. No tests, compilation, formatting, lint, schema/drift checks, native
builds, media tools, signatures, CI dispatch or release publication ran. Source
inspection and publication identity are not qualification. Preserve the known
alignment cancellation failure and all historic evidence.

The matrix and workflows are unqualified. Hosted environments are not hermetic,
the SPDX scope excludes OS/toolchain/media dependencies, and macOS platform
signing/notarization is not provided. crates.io/Homebrew remain unavailable.
Exact candidate reuse and Relay outer-archive retry behavior require execution.

Closes #85.
Closes #86.
Closes #87.

Related: #10, #64, #24, #69, egohygiene/egolint#29, egohygiene/flow#51.
**Do not close #10:** executed qualification, reviewed successor preparation and
the first real immutable release remain outstanding. This is a draft for review.

Implementation: `docs/validation/aniflow-10-implementation.json`.
Handoff: `docs/validation/aniflow-10-checkpoint.md` and root `CONTINUITY.md`.
Guide: `docs/releases.md`. Parallel draft #84 remains a separate documentation
lane with additive README/ROADMAP changes to reconcile at merge.
