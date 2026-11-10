# Restore a retained subject/proof candidate

This #82 source checkpoint restores a previously retained exact subject/proof
pair to a fresh private directory. An independently retained receipt SHA-256 is
required. Original production workspaces, source masters, the original bundle
and the original supplied proof file may be unavailable.

The selected bytes remain an **opaque proof candidate**. Byte integrity is
checked; OTS parsing, cryptographic subject binding, anchoring, trusted-time
verification, full ancestry, calendar networking and ownership are not checked.
The output keeps `timestamp_state: not_verified` and the existing explicit
unverified subject-binding declaration. A successful copy is not a verified
timestamp or a native-producer/review decision.

```json
{
  "schema": "aniflow.detached-proof-restore-request/v1",
  "source_directory": "/private/retained candidate",
  "receipt_sha256": "<known exact receipt SHA-256>",
  "maximum_total_bytes": 16777216,
  "minimum_free_bytes": 0
}
```

The locators/digest above are placeholders, not actual evidence. Read a bounded
plan, then explicitly create a new local copy:

```bash
aniflow --output json delivery proof-restore-plan --request "/private/restore.json"
aniflow --output json delivery proof-restore \
  --request "/private/restore.json" --output-directory "/private/new restored pair"
aniflow --output json delivery proof-inspect \
  --directory "/private/new restored pair" --receipt-sha256 "<known receipt SHA-256>"
```

`proof-retain --request FILE --output-directory NEW_ROOT` exposes the existing
explicit opaque-candidate retention operation. It is not a stamp/upgrade tool.
`proof-inspect` uses the existing external-pin byte/one-hop receipt checker.
Neither command fetches a proof or asserts a time bound.

Restoration copies the fixed request, subject, proof and optional immediate
predecessor receipt, then copies the unchanged source receipt last. The exact
original inspect contract works at the new root. Each member is confined and
bounded; sources and copied bytes are reobserved. Existing, partial, symbolic,
unknown-member and overlapping roots refuse. Failed/cancelled copies preserve
partial evidence; inspect it and choose a fresh destination. No automatic
adoption, cleanup or overwrite occurs. The total-copy policy is at most 265 MiB
under the existing 256 MiB subject / 1 MiB proof / bounded metadata limits.

Six library/refusal/restore cases use synthetic opaque data that is not valid
OTS, plus one CLI parser case. All are authored, uncompiled/unrun. No tests,
builds, dependencies, native/media/model/provider/CLI/SDK execution, network
timestamp operation or CI occurred. #82 retains its OTS lifecycle, independent
verification and qualification scope. ADR-0013 stays proposed and Flow #88 stays
last, with its original ledger untouched and known alignment failure unchanged.
