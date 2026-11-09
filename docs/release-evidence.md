# Retain and verify exact release subjects locally

`release_evidence` owns a small local retention boundary for #82. Select exact
originals, milestones, final deliveries, sidecars and production-evidence files;
the library copies their bytes and writes a fixed private subject manifest.
Complete any transformations, metadata changes or optional credential embedding
before selecting final delivery bytes. Changed bytes require a fresh subject
and a new bundle. This operation never alters the originals or embeds metadata.

The closed [request](contracts/release-evidence-request-v1.schema.json) declares
release ID/version, 1–64 unique subject IDs, role, explicit public/private
classification, parents and source input, plus maximum bundle/minimum free bytes.
Each subject may reference up to eight distinct earlier IDs. These are retained
lineage declarations, not executable transforms or an arbitrary processing DAG.
Inputs are independently selected:

- `accepted_media`: an accepted video artifact or source-bound PCM input from
  [destination preparation](destination-preflight.md). Existing native owners
  reobserve current acceptance before and after copying; the private observation
  retains its exact evidence references, stream facts and byte identity.
- `posting_member`: one fixed `media` or `public_metadata` member of a currently
  verified [posting package](posting-package.md). The private origin retains the
  package receipt and its exact raw digest. Private members cannot be selected
  through this lane.
- `declared_file`: one canonical regular file with explicit expected SHA-256,
  finite size bound and bounded declared origin. Retain selected raw native
  plans/reports, an original or an SRT this way. History remains operator declared;
  unknown settings stay unknown. No provider acceptance or ownership is created.

Native observation references do not back up every referenced workspace file.
Explicitly select any raw tool/model/settings plans, review declarations and
reports that must survive. Role and parent labels alone cannot prove a historical
transform. The manifest, request, origin snapshots and machine results are private.
`public_summary` explicitly projects only declared public IDs, roles, hashes,
sizes and parent edges whose two ends are public. It excludes private paths,
reports, private parents and request/manifest digests. No publication is performed.

```bash
aniflow --output json delivery evidence-plan --request subjects-request.json
aniflow --output json delivery evidence-seal --request subjects-request.json \
  --output-directory "/absolute/new evidence bundle"
```

The initial writer requires Unix, an absent normalized output root under an
existing canonical parent, disjoint source/run/package paths and available
storage. Directories are 0700, documents/copies 0600. Requests/manifests are
bounded to 2 MiB each. The plan includes 8 MiB of separate evidence headroom;
all selected bytes plus request, manifest and headroom must fit the supplied
bundle limit (at most 1 TiB). Selected native lanes may have smaller bounds.

```text
request.json                     # private explicit selections and policy
subjects/0000-original.bin        # exact retained bytes, fixed ordinal + ID
subjects/0001-delivery.bin        # another explicitly selected subject
subjects.json                    # immutable canonical manifest, no self-digest
seal.json                        # separate completion-last retention receipt
observations/00000001.json       # optional later local byte-check observations
```

Only the exact canonical file bytes of `subjects.json` form the possible future
timestamp subject. It records selected copies' digest/size, classification,
lineage and private origin snapshots. It never includes itself, `seal.json`,
later proof/status files or verification observations. Keep the returned raw
`subject_manifest_sha256` separately. Serialization changes, member changes or
new subjects require another manifest/bundle; do not regenerate an old subject.
No timestamp proof exists merely because a seal receipt exists.

```bash
aniflow --output json delivery evidence-verify \
  --directory "/absolute/new evidence bundle" --manifest-sha256 KNOWN_DIGEST
aniflow --output json delivery evidence-record-verification \
  --directory "/absolute/new evidence bundle" --manifest-sha256 KNOWN_DIGEST
```

Read-only `verify` requires the externally supplied known digest; it never trusts
a local receipt's digest as its own trust anchor. It checks exact canonical
documents, private request bindings, fixed membership, all retained bytes and
the separate observation chain. It works after originals disappear and expressly
does not reobserve producer acceptance. A matching digest is not an authenticated
identity or historical-execution verdict. Verification is a point-in-time read,
not an operating-system guarantee that an external writer cannot edit files.

`record_verification` holds the root writer and appends a successful byte check
under a fresh contiguous sequence name, without rewriting any immutable file or
prior event. Each event is at most 16 KiB, with a maximum of 256. The previous
event's raw canonical digest and local wall clock are recorded separately.
This unsigned local chain is not externally anchored; deleted tail events need
an outside record to detect. The clock is not a trusted timestamp. Corrupt,
unknown, partial or stale writer evidence requires explicit reconciliation;
there is no automatic adoption, overwrite, repair, pruning or cleanup. Cancelled
or failed writes retain partial evidence and never append a false success.

No keys, signatures, OTS calendar calls, Bitcoin verifier, uploads, posting or
ownership assertions are introduced. The stamp → pending → upgrade → verify
lifecycle, detached proofs, bounded verifier assumptions, actual subject/proof
restore exercise and Flow/Catalog consumer handoff remain in #82. The local
verification state is `exact_bytes_match_supplied_subject_digest`, with timestamp
`not_verified` and producer acceptance `not_reobserved_by_archival_byte_check`.

Seven synthetic local-byte/refusal/privacy/history cases and one CLI parser
case are authored, uncompiled and unrun. Tests, builds, schema validation, native
media and CI execution remain deferred to [flow #88](https://github.com/egohygiene/flow/issues/88).

## Detached proof candidates

The [offline detached retention SDK](detached-proof-retention.md) copies one
explicitly selected retained subject and opaque candidate proof into a separate
private root. It leaves this archive unchanged and always reports timestamp
verification and cryptographic subject binding as unverified. No signing,
stamping, upgrade, network operation or actual proof check runs implicitly.
