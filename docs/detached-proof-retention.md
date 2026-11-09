# Offline detached proof candidate retention [skip ci]

`release_evidence::detached::{retain,inspect,public_summary}` keep one selected
exact retained subject and explicitly supplied opaque proof candidate together.
They have no process or network port. A `.ots` suffix does not establish OTS
validity, subject binding, calendar submission, upgrade, anchoring or trusted
time. Every result says `not_verified`; no verified/anchored state is accepted.

Supply `ProofRetentionRequest` with schema
`aniflow.detached-proof-retention-request/v1`, an existing sealed bundle and
external `manifest_sha256`, selected `manifest` or `subject` ID, explicit proof
path and SHA-256, optional predecessor directory/receipt SHA-256, finite subject
and free-space limits. Select the canonical fixed digest manifest for a large
work. The separate original archive still retains its actual subjects.

Retention requires a fresh disjoint Unix-private root. It copies `subject.bin`
and `proof.ots` with pinned bytes, retains a private `request.json`, optionally
copies `previous.json`, and writes canonical `receipt.json` last. One subject
is limited to 256 MiB and one proof to 1 MiB. Explicit storage headroom, native
local writer and no-clobber file primitives are reused. Partial/unknown roots
are preserved and refused; there is no adoption, cleanup or automatic retry.

`inspect(root, known_receipt_sha256, cancellation)` checks exact copied bytes,
strict membership, private selection and immediate predecessor receipt. It
can work without original input paths. A successor must retain a different
candidate for the same exact subject/manifest and name the exact immediate
receipt and prior proof identity. Earlier roots are never rewritten. Retained
one-hop linkage does not authenticate the whole ancestry or prove an OTS
upgrade. The external receipt pin remains necessary to detect rewritten history.

The public summary has only explicit subject/manifest/proof identities,
sequence and unverified-state limits; it omits private paths, subject IDs,
selection history and proof bytes. Sharing that summary still requires the
operator's decision. No private media, actual proof or content approval was used
to author this checkpoint.

Six synthetic regression cases are authored, uncompiled/unrun. Tests/builds/
checks/native/SDK/CLI/network/CI execution remain deferred to flow #88, scheduled
last. Full #82 stamp/pending/upgrade/verify lifecycle, cryptographic subject
binding, network bounds, trusted-verifier assumptions and real restore checks
remain open. [ADR-0015](architecture/governance/decisions/ADR-0015-detached-proof-candidates.md)
is proposed, not accepted. Known alignment cancellation/start-marker failure
remains unresolved; cause unproven.
