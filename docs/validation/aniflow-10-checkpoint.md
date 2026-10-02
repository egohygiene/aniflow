# #10 implementation handoff

## Delivered source

Base: merged #24 PR #78 at `b0a346c8705927bb5db07333609c55ea0551bf7b`, tree
`4b0abcbb6f7998495d496f4920935c9a88ee310d`. Checkpoints #75–#77 are closed.
The local prior checkpoint had exactly that tree; Git transport was unavailable,
so publication uses the GitHub connector with the real merged main as parent.

Implementation checkpoint: `55696ae1aacefcaa9f931b58946bd1678a5d812b`, tree
`359992268a2c6f5bbad960bca668852d9cc5f7e2`. The final candidate adds this handoff,
root continuity and the implementation receipt. The PR records the final tree.

- #85: pinned Aether/Hygiene/Relay data, Cargo authority, exact version drift,
  explicit preparation and the four standard Taskfile handoffs.
- #86: bounded offline native builds, two-build binary/archive comparison,
  normalized archives, exact inventory, source/toolchain records and Cargo SPDX
  dependency evidence. The pilot matrix is Linux x86-64 and macOS arm64.
- #87: manual candidate and publication workflows, retained Sigstore signature
  verification, immutable Relay binary-profile handoff and rollback guidance.

Twenty-five synthetic test functions are authored in `tests/release/test_release.py`.
They are not passing evidence. Product Rust code, Cargo version and lockfile
are unchanged. No candidate version, tag, release, registry or distribution was
created. The guide's v0.4.0 commands are examples requiring a version decision.

## Validation disposition

**Not run by maintainer instruction under #64:** release contract and synthetic
tests, compilation, format/lint, schemas, corpus drift, full Rust/native suites,
package/smoke checks, workflow qualification, builds, signature verification
and release publication. No remote CI was dispatched or polled. Publication of
the draft may trigger existing repository events; their outcomes are unobserved.

Source inspection, dependency retrieval, authoring, immutable-source pin/digest
recording, Git metadata and final publication-tree comparison are implementation
evidence only. No checker was run to turn authored expectations into green status.
The known alignment cancellation failure at `tests/audio_alignment.rs:547`
(`marker.exists()`, thread join at 561) remains; no cause or fix is claimed.

## Remaining acceptance and next action

Parent #10 remains open for executed release acceptance. Aether #61, Hygiene #27
and Relay #47 were closed when inspected. Egolint #29 remains open. Both Egolint
#29 and Aniflow #64 are explicit first-release gates; closing them does not skip
the candidate workflow's actual same-source qualification.

Review this draft, then explicitly authorize #64 qualification/fixes and finish
Egolint #29. Review the accumulated changelog and choose a compatible successor;
prepare/merge that release PR. Create, inspect and preserve the signed native
candidate before manual immutable publication. Qualify publication retries,
including Relay's outer archive byte stability, against the same retained
candidate. Flow #51 remains blocked until that immutable release exists.

Limitations: shared hosted images and OS/toolchain dependencies are not hermetic;
the SPDX graph is Cargo-scoped; macOS notarization is not implemented; Windows,
Intel macOS, Linux ARM, musl, crates.io and Homebrew are not delivery claims.
The workflow artifacts expire after 30 days unless preserved. No real media,
external model or optional-provider qualification occurred.

Parallel PR #84 (`2c3272e769aaf4fb5de26a40a111b0c93e416d58`) is separate manual
audio/video finishing guidance. It was still draft/open at inspection and was
not a prerequisite for #10. Reconcile its additive README/ROADMAP changes when
merging; do not erase either lane's current facts. Broader corpus #24 and
multi-artifact output ports #69 remain separate work.
