# Public evidence handoff for Flow/Catalog consumers

`release_evidence::handoff` prepares one explicit portable field projection from
an exact private [release archive](release-evidence.md). Consumers can correlate
public release/version, selected file identities and selected public lineage
without copying the private manifest or producer history. It invokes no sibling,
registry, calendar, signer, network, media tool or posting service.

## Select disclosure explicitly

The closed [request](contracts/release-evidence-handoff-request-v1.schema.json)
requires an external archive manifest digest and 1–64 unique selected IDs.
Every ID must already be classified public in that archive. Missing/private
subjects refuse; there is no include-private switch or automatic latest-file
selection. The output preserves archive order and includes parent edges only
when both ends are selected. An unselected public subject is omitted too.

```json
{
  "schema": "aniflow.release-evidence-handoff-request/v1",
  "bundle_directory": "/private/release-archive",
  "manifest_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
  "subjects": [{"id": "delivery", "proof": null}],
  "maximum_archive_subject_bytes": 1073741824,
  "maximum_proof_bytes": 0,
  "minimum_free_bytes": 67108864
}
```

Replace paths/IDs and the zero digest with exact observed values. A selected
subject can explicitly supply `proof: {"directory": "/private/file-proof",
"receipt_sha256": "KNOWN_SHA256"}`. Its [retained OTS inspection](retained-ots-inspection.md)
must match the exact file ID, SHA-256, size and private source manifest. Aggregate
manifest proofs are excluded because they bind a private aggregate rather than
the disclosed file. Mismatched/unsupported/malformed or differently bound proofs
refuse the handoff; do not silently omit a requested proof.

Only the proof digest/size/profile, local subject relationship and counts of
pending/Bitcoin/unknown claims are disclosed. No raw proof, subject media,
calendar URI, claimed block height/root, unknown payload, private path, original
producer/request/receipt/manifest digest or history is included. Proof bytes
need a **separate disclosure** before independent verification; a digest is an
external lookup reference, not an embedded proof or a new automatic fetch route.
No inferred creator/identity/ownership claim is added.

## Local packet and two kinds of readback

The public library exposes `plan`, `create`, current private-owner `observe`,
and `PublicHandoff::from_pinned_json` for portable byte/contract readback.
The CLI is delivery of those same operations:

```text
aniflow delivery evidence-handoff-plan --request disclosure.json
aniflow delivery evidence-handoff --request disclosure.json --output-directory /private/fresh-packet
aniflow delivery evidence-handoff-status --directory /private/fresh-packet --receipt-sha256 KNOWN_PRIVATE_RECEIPT_SHA256
aniflow delivery evidence-handoff-inspect --file /private/fresh-packet/public/release-evidence.json --handoff-sha256 KNOWN_PUBLIC_HANDOFF_SHA256
```

Creation uses a fresh private root disjoint from the archive and selected proofs:

```text
request.json                     private source/disclosure/budget selections
public/release-evidence.json      selected portable fields only
receipt.json                     private completion-last source/output binding
```

Only `public/release-evidence.json` is the portable projection. Do not share the
whole directory: the request and receipt retain private locators and aggregate
source identity. All files remain private by default on Unix; classification as
public is not filesystem/world-read permission or authority to post. Retain the
public file's exact digest independently from the private receipt digest.

Planning verifies the private archive and each explicitly selected proof before
and after projection. The whole archive subject-byte bound is checked before
hashing its contents; selected proofs have a cumulative admission bound, with
each proof individually limited to 1 MiB by its existing owner. Upstream hashes
may be read more than once; these bounds are not an exact I/O budget or an OS
reservation. Requests are at most 128 KiB, canonical documents at most 2 MiB,
and creation admits 8 MiB of metadata work plus the selected free reserve.
There are no source-media or proof copies in the packet. Failed partial,
existing, extra, symbolic or stale inventory is refused and preserved rather
than adopted, overwritten or cleared. Cancellation does not manufacture a receipt.

Private-owner `observe` requires current surviving archive/proof bytes and the
external private receipt pin. Portable `from_pinned_json` needs only exact
canonical public bytes and the external public digest; it works after private
sources disappear. That second operation verifies byte/contract integrity only.
It does not reobserve producers, authenticate the supplied hash, establish a
signature or prove that a selection was approved by a particular person.

The public contract always retains `archival_bytes_only`, `not_signed`,
`not_verified`, `not_established` and no publication authority. A matched OTS
subject is not independently verified Bitcoin time. Human content approval,
creator-ID/key claims, timestamps, current native producer acceptance, original
media, lookup storage and actual publishing remain separate owners/operations.

Nine library and one CLI cases are authored **uncompiled/unrun**. This source
checkpoint starts no actual proof/media operation or public disclosure. Full
OTS lifecycle, independent verification, consumer/native/security/platform
qualification and broader #82 acceptance remain open. Flow #88 remains last;
the original ledger and known alignment failure are unchanged. ADR-0015 stays
proposed, and no Identity adoption or ratification is inferred.
