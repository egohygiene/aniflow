# Package selected local toolchain configurations

This #38 source checkpoint packages the existing `prepare-workflow` result for
explicitly selected roles. It writes configuration documents only, preserving
current missing/incompatible dependency diagnostics instead of installing or
searching for tools/models. No source media or provider process is opened.

```bash
aniflow --output json toolchain package-workflow \
  --configuration "/private/selected workflow.json" \
  --output-directory "/private/new local package"
```

The request is the existing `aniflow.toolchain.workflow-setup/v1` profile,
inventory and explicit role selection. Every selected preparation must be ready
and stable. Omitted optional roles remain omitted; their models are not read.
The package is private and contains the exact request, preparation, and each
selected role's existing `configuration.json`. It copies no executable, model or
asset, performs no provider registration, and contains machine-specific private
locators. It is not a public or portable installation bundle.

Creation requires a new output directory, separate from inventory locators.
Files use private no-clobber writes; the completion manifest
`workflow-bundle.json` is written last. A failed or cancelled creation can leave
an incomplete directory: preserve and inspect it, then choose a new path.
There is no partial-folder promotion or automatic repair.

Keep the returned `manifest_sha256` separately. Read retained bytes without
reobserving external dependencies:

```bash
aniflow --output json toolchain inspect-workflow-bundle \
  --directory "/private/local package" \
  --manifest-sha256 "<known exact manifest SHA-256>"
```

Readback checks exact role membership, no symbolic/special/extra members,
request/preparation/configuration bindings, raw hashes and byte counts. Add
`--reobserve-dependencies` to repeat the existing inert selected preparation.
`current_preparation_matches`, `changed_or_unavailable` and `not_reobserved`
remain separate from retained integrity. Neither an external digest nor a
consistent supplied inventory authenticates a tool's origin or qualifies native
behavior. Ordinary domain planning still validates the exact tools, providers,
settings and media before any explicitly requested execution.

The package admits at most 11 selected roles, 1 MiB request/configuration files,
2 MiB preparation, a 256 KiB manifest and 16 MiB retained documents. These can be
tighter than the standalone preparation's report bounds; oversized data refuses
without truncation. Supplied observations remain declarations, not probe runs.

Six library package/readback/refusal cases use synthetic inert tool bytes and
supplied observations; one parser case is also authored. All are uncompiled and
unrun. Tests/builds/dependencies/schema/native/provider/model/media/CLI/SDK and
CI execution remain deferred. No installation, actual review, source mutation,
release or publication occurred. Full #38 acceptance remains open and Flow #88
stays last with its original ledger untouched.
