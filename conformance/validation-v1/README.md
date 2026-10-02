# Synthetic validation-gate bundle

This bundle exercises the authored Pipeline v3 inline validation protocol for
#33. It uses a small text file as opaque bytes and a Python fixture executable.
It does not inspect or qualify real media. **No conformance command has been
run in this implementation pass**; execution is deferred under #64.

`pipeline.yml` binds `copy.registration.json` as the producing provider and
`passed.registration.json` as its inline artifact validator. The manifest,
closed configuration schema and exact configuration wrappers are kept together.
`provider.py` accepts the fixed `--aniflow-invocation` ABI. Registrations resolve
relative to this directory and require an explicitly supplied Python runtime
through the fixture's shebang. These test registrations are not production
validator packages.

The copy profile copies the synthetic source. Validator profiles read the
host-authored context and emit observations for success, nonpass dispositions,
duplicate checks, stale identities, contradictory checks, missing/malformed
reports, nonzero exit, output mutation and a deliberately slow invocation.
The test harness copies the bundle into a temporary directory; its
`validator-launch-count` proves intended resume behavior once tests are run.

The public [layered validation guide](../../docs/layered-validation.md) describes
the provider ports, exact locks and acceptance graph. The tests are authored in
`tests/layered_validation.rs`, `tests/validation_contracts.rs` and
`tests/validation_schema.py`. For a later authorized qualification pass:

```bash
task validation:conformance
```

Schema examples under `docs/contracts/examples/` use synthetic digests and
observations. They describe transport shapes rather than an executed accepted
run. A passing fixture in the future would establish only the exercised
protocol behavior, not temporal media quality or platform qualification.
