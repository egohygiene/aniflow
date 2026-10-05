# Issue #38 — offline toolchain setup checkpoint

Authored 2026-10-04T15:43:57Z. Parent #38 remains open; checkpoints #97–#99 deliver
an initial draft for explicit dependency evidence and setup planning.

## Merge and implementation identity

At task start Aniflow had no pending PRs. PR #96 was already merged at
`0e5773062a7a3d2d07e05d47d698a38002bd670b`, tree `650a3376f687311b10556c07ce200eae6458d269`. It was
not merged again in this session. Hosted CI was not dispatched or polled.

Local implementation commit: `e4db53a357dd92f280f27d5fcf42e147f6a8a329`.
Implementation tree: `e4f26fbb40874683f051b5cd2b7a4030fce2d737`. Publication uses actual
remote main as the parent and requires equality with the final local tree.
The PR records its published head and final tree; tree equality proves only
publication fidelity, not implementation qualification.

## Delivered boundary

- Public `ToolchainProfile` / `ToolchainInventory` parsers and `inspect_profile`
  compare explicit selected dependencies with bounded local file hashes and
  attributed, digest-bound caller observations.
- Three closed v1 contracts retain requested capabilities, package revisions,
  flags/build features, model identities, backend/platform observations and
  distinct native/requested scales. Missing, incompatible and unverified
  facts remain separate from installed facts.
- `toolchain doctor` and `toolchain plan` expose human and machine reports.
  Doctor retains diagnostics in dependency errors; plan remains reviewable
  while incomplete. `ready` means inventory consistency, and
  `native_qualification` is always false.
- Core media/metadata remain independent from opt-in Upscayl and separately
  selected visible-image repair profiles. Models and example inventory digests
  are not supplied or authenticated by the templates.
- Unix no-follow/nonblocking opens, regular-file identity observations and
  bounded hashing avoid treating paths as commands. Setup never launches tools,
  writes registrations, installs packages, downloads models or selects providers.
- Existing Pipeline v3 explicit registration/lock authority is unchanged.
  Native CLIs require compatible adapters. Package/backend/settings evidence
  belongs in the adapter's closed effective configuration before processing.
- Linux x86-64/macOS arm64 candidate preparation guidance and immutable upstream
  source/license references are included. No binaries or weights are bundled.

## Authored evidence, not passing checks

Six internal type/parser tests, nine Rust integration tests, three CLI/command
contract tests and seven Python schema tests are authored. They cover literal
paths and no-launch markers, optional isolation, version/build/model/backend
failures, stale bytes/observations, scale distinctions, strict declarations,
packaged templates, report stability and machine diagnostics.

Read-only source review found and repaired pre-open FIFO/symlink replacement,
stale observation ambiguity, scale declarations without model evidence,
structural null/schema disagreement, generated provenance bounds and synthetic
fixture assumptions. This is not compiler or execution evidence.

**All execution checks remain unrun under #64:** tests, compiler/build, fmt/lint,
schema/drift, smoke/package, native tools/models/platforms and hosted CI.
No real/private media was read or transformed. Cargo remains 0.3.0; no release,
tag or platform publication occurred.

Known failure preserved unchanged:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`,
`tests/audio_alignment.rs:547` (`marker.exists()`), joining thread at line 561.
The cause remains unproven.

## Remaining #38 work

1. Bounded live probes and trustworthy observation capture for actual executable
   versions, flags, codecs and hardware; preserve unavailable vendor versions.
2. Complete adapter/environment identity, including Node/package/native
   dependencies, and explicit registration materialization for protocol adapters.
3. Bind selected profiles into processing preflight without implicit installation
   or fallback; retain existing exact-lock authority.
4. Reviewable macOS bundled-model locator discovery and actual selected model
   origin/terms evidence; never trust or copy discoveries automatically.
5. Separately qualify allenk image/video behavior, exit/skip/artifact semantics,
   target architectures, codecs and backends. Its image binary does not imply
   support for the separate video implementation.
6. Execute accumulated synthetic and native qualification only when authorized
   under #64; include `tests/toolchain_schema.py` as well as the Rust suite and
   contract checker. #10 retains immutable release packaging/publication.

After #38: #36 → #37 → #35 → #83 → #82. Optional #81/#39 remain separate.
Flow #78 captures the future AMV preset. See [the guide](../toolchain-profiles.md)
and [upstream evidence](../toolchain-upstreams.md).
