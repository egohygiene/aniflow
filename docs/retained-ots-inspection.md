# Inspect and restore a retained OTS subject binding

`release_evidence::detached::inspection` connects the pure
[OTS commitment inspector](ots-commitment-inspection.md) to actual retained
subject/proof bytes. Supply the external known `receipt_sha256` for an existing
[retained candidate](detached-proof-retention.md). It reuses the strict fixed
inventory, canonical receipt/request and byte-hash checks, rehashes the actual
copied subject, parses the exact retained proof against that digest and then
reobserves the pair. Original bundle/producer/proof paths need not survive.

```text
aniflow delivery proof-inspect-ots --directory /private/pair --receipt-sha256 KNOWN_RECEIPT_SHA256
aniflow delivery proof-restore-bound --request restore.json --output-directory /private/fresh-pair
```

The existing [restore request](contracts/detached-proof-restore-request-v1.schema.json)
selects the source directory, external receipt digest and finite copy/free-space
bounds. `restore_bound` requires a supported parsed OTS proof whose header
matches the observed subject **before creating the destination**. It calls the
ordinary exact-byte restoration owner, which copies to a new private root and
copies the unchanged original receipt last, then rechecks destination and
source against the pre-copy interpretation. Failure retains partial work and
does not adopt, overwrite, retry, prune or clean it. Existing/overlapping roots
and changed bytes refuse. Opaque `proof-restore` remains separately available
when a proof is malformed or outside this inspector's closed profile.

Read-only `inspect` reports mismatch or unsupported explicitly in the nested
proof result; malformed input refuses. `PairInspection::subject_matches` means
only a supported locally computed subject/header/commitment relationship. It
does not mean that any calendar, Bitcoin block, claimed time or ownership was
verified. Unknown attestations remain opaque, and an unsupported hash profile
does not establish that the original proof is universally invalid.

The [private report](contracts/retained-ots-inspection-v1.schema.json) contains
the original retained receipt and the separate new interpretation. The original
receipt still says `unparsed` and `not_verified`; it is historical immutable
data, not silently reclassified. New interpretation always retains
`timestamp_state: not_verified`, `ownership: not_established` and
`producer_acceptance: not_reobserved; archival_bytes_only`. No native media
acceptance is recreated from archived claims. The report is a point-in-time
observation; reread the actual pair for current facts rather than trusting a
saved loose JSON report. No trusted wall-clock time is added.

This source checkpoint does not run commands or process private proof/media
data. Six synthetic library and one CLI cases are authored **uncompiled/unrun**.
Actual interoperability, restore/fault/cancellation/platform/security/native
qualification remains deferred to flow #88, last. Stamp/pending/upgrade/Bitcoin
verification, signature/identity trust and broader #82 acceptance remain open.
No networking, key/certificate setup, signing, posting, publication or ADR
ratification occurs here.
