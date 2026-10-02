# Deterministic adversarial corpus

This is the bounded #24 corpus surface. `catalog.json` is generated from
repository-owned recipes and coverage locators. Its qualification state is
always `authored_not_run`: runtime evidence belongs in separate receipts.
The #24 implementation pass authored coverage without executing it under #64.

Start with [the operator guide](../../docs/adversarial-corpus.md). The
[closed catalog schema](catalog.schema.json) describes fixture identities,
provenance, parameters, exact byte inventories, expected outcomes, source
bindings, coverage locators and qualification budgets. Recipe parameters and
oracle property maps are intentionally recipe-specific JSON objects; their
values are bound by read-only regeneration, not treated as arbitrary commands.

All media is synthetic and repository-authored under the root MIT license.
No copyrighted sample collection, user media, model weights, paid services or
secrets are needed. No generated media binary is committed. FFmpeg encodings
are tool-bound: qualification records actual executable digests and version
banners, compares two generations on that toolchain and retains observed output
digests. Those digests are not invented or claimed portable across toolchains.

## Stable identity and updates

- `ani.corpus.*` names deterministic bytes and expected behavior. Fix a broken
  implementation without changing expectations to conceal a regression.
- `ani.native.*` names an explicit FFmpeg encoding profile, qualified only when
  its dependencies are available and its assertions execute.
- `ani.suite.*` links existing Rust test targets and named tests. These are
  source locators, not counts of new fixtures or passing scenarios.
- `ani.index.audio.*` preserves the 148 historical audio IDs. Historical
  receipts retain their original commit/environment; they do not qualify main.

Changing a recipe's meaning requires a new ID or an explicitly documented
catalog version migration. The recipe generator uses fixed seed `240034`,
integer PCM/Y4M bytes and PNG stored-DEFLATE blocks to avoid library compressor
variation. The catalog pins the complete source files on which its claims rely.

`task corpus:catalog` is an explicit authoring action. `task corpus:check` is
read-only and fails if recipes, expected bytes, referenced source files or test
locators drift. CI never runs the authoring command. Review the catalog diff
alongside intentional source changes, including changes to referenced tests.

The parent #24 remains the coverage roadmap. Original acceptance and platform
claims require executed evidence; see #64 for deferred qualification and #69
for multi-artifact output ports. The [coverage matrix](../../docs/adversarial-corpus.md#coverage-and-limits)
records the boundary of this implementation.
