# Contributing

Keep changes small, reproducible, and grounded in a real media workflow.

Before opening a pull request:

```bash
task validate
```

Pull requests should:

- explain the user-visible pipeline behavior being changed;
- preserve source safety and path handling;
- include unit coverage for configuration or planning changes;
- extend the synthetic smoke test when execution behavior changes;
- follow the canonical [architecture graph](docs/architecture/README.md);
- follow the canonical
  [product-name rule](docs/architecture/identity/PRINCIPLES.md#canonical-product-names);
- update the pipeline schema, architecture documents, and decision log when
  contracts, ownership, or invariants change;
- use Conventional Commits.

Avoid adding a general abstraction until at least two concrete processors or
stages require it.

## Provider changes

Read [Authoring a temporal provider](docs/provider-authoring.md) before adding
or changing a manifest, capability, configuration, registration, or provider
process. Start from the coherent
[`provider-v1`](conformance/provider-v1/README.md) bundle; files beneath
`docs/contracts/examples/` are synthetic schema fixtures, not an executable
distribution.

A provider-facing change should:

- validate effective values against the exact provider-owned schema and update
  its raw-byte SHA-256 reference;
- derive the effective-configuration digest using aniflow canonical JSON;
- keep registration locators relative, confined, explicit, and independent of
  `PATH` discovery;
- request only the side effects and host resources the implementation uses;
- exercise the fixed `--aniflow-invocation <absolute-request-path>` ABI through
  Pipeline v3;
- prove output acceptance, the execution report, checkpoint, read-only status,
  and compatible resume without relaunch;
- retain focused denied-effect, nonzero-exit, and malformed or missing-output
  coverage; and
- describe media quality and platform support separately from contract
  conformance.

Run the focused suite while iterating:

```bash
cargo test --test provider_conformance --locked
```

Do not treat a manifest or side-effect declaration as sandbox enforcement, an
artifact-validator stage as a provider-backed completion gate, or a
compatibility fingerprint as an automatically accepted Pipeline v3 checkpoint.
Inline provider-backed gates must use the explicit obligations and evidence
contracts in [layered validation](docs/layered-validation.md). The authored
`task validation:conformance` suite covers that boundary; execution for #33 is
deferred under #64 at the maintainer's request. Issue #24 owns the exhaustive
adversarial provider corpus.

Cache changes should extend `task cache:conformance` and preserve ownership,
origin proof, current acceptance, source isolation and rerun obligations.
The maintainer deferred execution of these #34 checks under #64; authored
coverage must never be reported as passing evidence.
