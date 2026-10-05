# Issue #103 — audio toolchain preflight checkpoint

Authored 2026-10-05. Extends draft PR #100 after #97–#99/#101/#102. Parent #38
remains open. No merge occurred and no execution checks were authorized.

## Implementation and publication identity

Main remained `0e5773062a7a3d2d07e05d47d698a38002bd670b`, tree
`650a3376f687311b10556c07ce200eae6458d269`. Starting PR #100 remote head:
`208bc4a28d63f04f4889c082d58faab5397c8888`, tree
`afa3500c9a5c15208d2b3d137aedea3d351ea970`; identical to starting local
`6e323ec19f17f1e005bcfac0b59f31b5d02c4510`.

Local implementation commit: `7b8554e77b7dfdf041723227a8e9772aef289dcc`.
Implementation tree: `f4ebda1a557f5297cccca16c9caeca935896d7b5`.
Publication extends the actual remote PR head with the complete final handoff
tree. The PR records the resulting remote commit/tree and matching local head.
Tree equality establishes publication fidelity, not execution qualification.

## Delivered boundary

- A closed, bounded preflight document embeds profile/inventory evidence and
  explicit selected main-stage, registration and FFmpeg/ffprobe dependency
  mappings. Only the existing native-v2 audio-inspection adapter is supported.
- Fresh union inspection shares the existing two-GiB aggregate file-hashing
  limit. Exact literal paths, pins and attributed version evidence must agree
  with the closed adapter settings and actual resolved lock components.
  Current OS/architecture and selected side effects must also agree. Unsupported
  backend, scale and arbitrary settings are refused.
- Public reports retain fresh facts, actions and readiness diagnostics. Saved
  reports are never accepted as runtime authority. CLI errors retain the
  existing category/message contract and bounded excerpts; callers needing
  the complete failed report use `preflight_toolchain` before binding.
- The immutable plan binds `toolchain_preflight_sha256`. Guarded run/resume
  require the exact document and fresh readiness before input rebinding, cache
  access or provider launch. Missing or changed configuration is refused even
  when cache/checkpoints might satisfy the work. Exact locks remain authoritative.
- Historical unguarded plans omit the optional field and preserve their bytes
  and hashes. Null/malformed digests are invalid. Strict older consumers reject
  guarded plans. Provider-lock v1 is unchanged; the Rust public payload/request
  structs gain optional fields with constructors/builders for ordinary callers.
- `--toolchain-preflight` is available on plan-v3, run-v3 and resume-v3. Ordinary
  planning still hashes source identities before explicit binding; CLI run-v3
  performs that planning step. Resume first opens the workspace and acquires
  its existing writer lock. It validates manifest authority before source
  rebinding, including when a persisted plan has had its guard removed.

Bindings cover named stages only. This does not create registrations, install
or download dependencies, automatically import probe reports, or qualify native
execution. Inventory observations remain caller-attributed, package provenance
is not authenticated, and original-path hashes are not atomic executed-file
attestation. Existing adapter execution checks remain in place. The
[guide](../toolchain-preflight.md) describes preparation and supported limits;
[ADR-0010](../architecture/governance/decisions/ADR-0010-plan-bound-toolchain-preflight.md)
is proposed, not accepted.

## Authored evidence and source review

Eight synthetic Rust integration cases and one CLI parser case are authored.
They cover frozen-plan compatibility, canonical read-only binding, exact
registration/configuration/lock identities, inventory drift, unsupported profile
semantics, omitted/changed run and resume guards, and strict bounded JSON. A
guard-stripping case recomputes a valid plan digest and retains the original
pending manifest to exercise immutable authority without launching a provider.
Three Python schema cases cover nested closure, unqualified report evidence and
optional-but-nonnull plan guard shape. The contract catalog includes the two
new schemas and examples. None of these checks was run.

Source review repaired a side-effect mismatch with the shipped audio manifest,
standalone reports overlooking an existing plan binding, generic readiness
errors lacking actionable fact excerpts, and resume validating manifest
authority after input hashing. Independent re-review found no remaining
concrete source defect. This is not compiler, schema or runtime evidence.

**All execution checks remain unrun under #64:** tests, compiler/build,
format/lint, schema/drift, smoke/package, native tools/models/platforms and
hosted CI dispatch/polling. No real/private media, assets, release or tag was
processed or published. Cargo remains 0.3.0.

Known failure unchanged:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`,
`tests/audio_alignment.rs:547` (`marker.exists()`), joining thread561;
cause unproven.

## Next boundary

Continue #38 with reviewable registration-bundle preparation and complete
adapter/environment identity, then additional typed toolchain mappings,
optional tool/device probes and model-locator discovery. Qualification remains
#64 work. The implementation lane after #38 remains #36 → #37 → #35 → #83 → #82.
Flow #78 captures the AMV preset; its general workflow CLI and Aniflow adapter
integration are not delivered by this checkpoint. Actual immutable release #10
still precedes Flow #51; later music-video release needs #40 and Flow #75.
