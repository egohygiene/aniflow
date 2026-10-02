# Preparing and publishing aniflow

This is the authored #10 Rust CLI/binary release pilot. **No release, platform
qualification or passing execution is claimed by this change.** #64 retains
the deferred checks; Egolint #29 retains organization conformance. Both must
be completed before the first candidate workflow can advance.

## Authority and compatibility

`Cargo.toml` → `package.version` is the sole product version authority for the
library and CLI. `Cargo.lock`, the prepared candidate, dated changelog and exact
`vMAJOR.MINOR.PATCH` tag must agree with it. The release declaration points at
Cargo; it deliberately contains no second version number. Architecture-document
versions and independently versioned JSON contracts are different authorities.

The convention PR keeps Cargo at 0.3.0. Existing dated changelog sections are
preserved historical source records, not proof of GitHub publication. On
2026-10-02 the repository's GitHub release and tag-ref collections were empty.
The declaration therefore remains `unreleased` until a real release is observed.

Before 1.0, patch releases preserve the supported public Rust, CLI and machine
contract promises within the current minor line. Breaking changes require an
explicit minor bump and a **Breaking changes** changelog callout with migration
guidance. Compatible additions may use a reviewed minor release. At 1.0 and
later, incompatible supported-interface changes require a major bump. Deprecate
with a replacement and removal horizon where known; never hide a removal in a
patch. See [public contracts](contracts/README.md), including the 0.3.x aliases.
Pipeline v2/v3 and contract `/v1` or `/v2` identities remain explicit; the product
version does not silently upgrade old checkpoints or payloads.

Commit messages can suggest a bump. They cannot select it, rewrite history,
mark a candidate qualified, or publish. A maintainer reviews the chosen version,
the complete Unreleased content, compatibility and any outstanding migrations.
The accumulated post-0.3 work needs this review before preparing its successor.

## Local handoffs

Python 3.11+ and the already installed `jsonschema==4.23.0` package support
declaration checks. Git supports source identity. The commands do not install
dependencies. They use the pinned Aether schema in
[third_party/release](../third_party/release/README.md).

```bash
task release:plan
task release:check
# Example only: choose and review the actual successor and date first.
task release:prepare VERSION="v0.4.0" DATE="2026-10-02"
```

Preparation requires a clean tree and an explicit successor, checks all inputs,
then updates only Cargo.toml, the root Cargo.lock entry, CHANGELOG.md and
`.egohygiene/release-candidate.json`. It preserves `Unreleased` and all dated
history. It never runs Cargo resolution, commits, creates tags, or dispatches
anything. If writing is interrupted, inspect the dirty diff; verification
refuses it. Commit the reviewed preparation through a release PR.

```bash
task release:verify VERSION="v0.4.0" SOURCE_REVISION="$release_revision"
task release:verify VERSION="v0.4.0" SOURCE_REVISION="$release_revision" BUNDLE="$release_bundle"
task release:publish VERSION="v0.4.0" SOURCE_REVISION="$release_revision" CANDIDATE_RUN="$candidate_run_id"
```

Verification without BUNDLE checks clean source, exact tag/version, candidate
and changelog agreement. It does not claim build or publication readiness.
With BUNDLE it also checks the exact inventory and cryptographically verifies
the retained attestation through GitHub CLI; trust-root retrieval may use the
network. `release:publish` prints the manual-dispatch argument list for review
and **does not execute it**. `release:test` owns the synthetic refusal suite.

## Candidate creation

After #64 and Egolint #29 are completed, explicitly dispatch
`release-candidate.yml` on current `main`, supplying its full SHA and the
prepared version. The workflow rejects other events, branches, stale source,
unreviewed versions and conflicting immutable tags before native work.

| Target | Native runner | Initial scope |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | Dynamic GNU/Linux executable; the runner's userspace is the initial baseline |
| `aarch64-apple-darwin` | `macos-15` | Apple Silicon executable; no Developer ID signing or notarization |

Windows, Intel macOS, Linux ARM, musl and broader OS compatibility are outside
this pilot. A successful build is not a real-model or optional-provider
compatibility claim. Neither FFmpeg/FFprobe nor processors, models or media are
redistributed. Users must supply those dependencies independently.

The workflow provisions dependencies separately, then runs the repository's
synthetic qualification, full Rust tests, package, formatting and Clippy gates.
It uses Rust 1.85.0, the declared MSRV; a dependency incompatibility must fail,
not silently select a newer compiler. The hosted runner images, native linker
and Python dependency environment are not hermetic. Native build evidence
records Rust/Cargo/Python/host observations and the lockfile hash. This matrix
must execute before it becomes a support claim.

Each host compiles twice with `--frozen`, isolated target directories, no
incremental compilation and remapped paths. Binary and archive bytes must
match. This proves repeatability within that run's installed toolchain only.
Archive order, uid/gid, modes, names and timestamps are normalized. Runtime
timestamps and signing data stay outside these archives. Each archive contains
`bin/aniflow`, license, README, changelog and versioned public JSON Schemas.

For an explicit local native build after dependencies have been prepared:

```bash
task release:build VERSION="v0.4.0" SOURCE_REVISION="$release_revision" \
  TARGET="aarch64-apple-darwin" WORK="$new_build_directory" OUTPUT="$new_target_directory"
```

Both output paths must be absent and outside the checkout with existing parents.
Build work/logs remain on failure; no automatic cleanup adopts other paths.
The two builds have a shared 40-minute deadline and 4 GiB monitored work budget.
The bundle cap is 512 MiB/1,024 files. These checks are safeguards, not process
sandboxing or filesystem quotas. Hosted jobs add outer time limits.

## Evidence and signing

Target artifacts contain their archive, `build.json`, target SPDX 2.3 SBOM and
exact SHA256SUMS. Assembly refuses a partial or extra target, source/version
disagreement, altered members, symlinks, traversal, oversized data or missing
required evidence. It never extracts an untrusted tar onto disk.

The aggregate SBOM describes Cargo's target-filtered resolved graph, including
build/dev dependencies and available Cargo.lock checksums. Its scope is
explicit: it is not a complete SBOM for the OS, toolchain, FFmpeg or models.
Cargo-reported licenses are observations, with `NOASSERTION` where unknown.

`provenance.json` binds source, version, target archives, external channels and
rollback. `payload-SHA256SUMS` binds every unsigned payload file. A job with
scoped OIDC/attestation permission signs that inventory through GitHub's
provenance action. GitHub CLI then verifies the retained Sigstore bundle against
this repository, `.github/workflows/release-candidate.yml`, the exact source and
signer SHA, `refs/heads/main`, and a hosted runner. Only successful verification
creates `signature.json`; a final SHA256SUMS covers payload and signature files.
This is supply-chain provenance, not macOS platform code signing.

The successful run retains `aniflow-release-candidate-<source SHA>` for 30 days.
Review/download it before publishing and preserve its exact bytes for recovery.
Logs and partial failure evidence are separate from the accepted bundle.
No signature, SBOM completeness or passing qualification is inferred from an
authored file or from a successful command exit alone.

## Reviewed publication and retry

Explicitly dispatch `release.yml` on current `main` with version, source SHA,
successful candidate run ID, and the reviewed checkbox. It verifies the
candidate run's workflow, event, conclusion and source; downloads the retained
signed bundle; rechecks its checksums and actual signature; and gives those
identical bytes to Relay's pinned `semantic-release.yml` using `profile: binary`
and `release-name: aniflow`. Write permission exists only in the publication
handoff. No normal push, PR, schedule or local task publishes.

Relay repeats its declaration/profile/default-branch verification and creates
or resumes the immutable tag and GitHub Release. The three outer assets are
`aniflow-vX.Y.Z.tar.gz`, `release-asset.sha256` and `release-evidence.json`.
The outer archive contains both platform archives and their signed evidence.
The archive's source/tag is pinned; no moving major alias is created.

Retry publication with **the same candidate run**. It must not rebuild or
resign: fresh attestation timestamps would produce different immutable bytes.
If the retained candidate expires or main advances, stop and reconcile the
original evidence before retrying. Never bypass a mismatch, overwrite an asset,
or move a tag. A corrected source requires a reviewed successor version.

crates.io, Homebrew, installers and other registries remain `unavailable` until
a separate repository-owned adapter is implemented and authorized. A GitHub
release proves none of those deliveries. Flow #51 can consume only a real,
verified immutable release, with checksums and compatible public contracts.

## Rollback

Pin consumers back to the last known verified version and digests, retain the
bad release as history, revert the defect and prepare a corrective successor.
Revoke an affected external channel through its owner if one exists. For the
first release there may be no prior binary: stop adoption until a successor is
verified. Never delete/recreate a release to conceal a failed candidate.

## Qualification still outstanding

#85–#87 deliver the implementation and authored coverage. Parent #10 stays open
for #64 execution/fixes, Egolint #29 conformance, a reviewed successor/changelog,
native matrix and signature verification, and the first immutable publication.
Preserve the known audio-alignment cancellation assertion at
`tests/audio_alignment.rs:547` (`marker.exists()`); its cause is not established.
The known stack includes #32, #33, #34 and bounded #24 work that remains unqualified.

Reference contracts: [Aether](https://github.com/egohygiene/aether/blob/8a2a3d08f3aa9da3847bd5277843506ab855192e/library/organization/specs/release/repository-release.spec.md),
[Hygiene](https://github.com/egohygiene/hygiene/blob/63d313b1ddf8669808e897853b74928505494da0/catalog/repository-release-policy.json),
[Relay lifecycle](https://github.com/egohygiene/relay/blob/04bd32c8ef492418f47d6df6faee425d6888f341/SEMANTIC_RELEASE.md),
[Relay binary profile](https://github.com/egohygiene/relay/blob/04bd32c8ef492418f47d6df6faee425d6888f341/RELEASE_PROFILES.md),
[GitHub signature verification](https://cli.github.com/manual/gh_attestation_verify).
