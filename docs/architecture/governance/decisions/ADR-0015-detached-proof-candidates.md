---
schema: aether.architecture-decision/v1
id: aniflow-adr-0015
title: Retain detached proof candidates separately from immutable subjects
kind: architecture-decision
status: proposed
owners:
  - egohygiene
scope:
  - aniflow
governed_by:
  - architecture-decisions
supersedes: []
superseded_by: []
related:
  - aniflow-architecture
  - aniflow-adr-0013
---

# ADR-0015 — Detached proof candidate retention

## Status and authority

Proposed for #82. No acceptance date or cryptographic/native qualification.
The authorized source sprint permits implementation, not ADR ratification,
actual signing, timestamp networking, review or publication.

## Context and proposed boundary

The existing exact-byte archive excludes future proofs from its fixed subject
manifest. Adding files inside that sealed inventory would invalidate it and
create self-reference risks. A separate fresh private root can retain a selected
subject and supplied proof bytes while preserving the unchanged archive.

Pin canonical manifest and copied subject identity independently from proof
identity. Retain an immediate predecessor's exact receipt for a successor
candidate; leave every prior root unchanged. Readback validates current copied
bytes and this one-hop linkage against an external receipt digest. Full ancestry
or authenticity is not inferred from a hash chain or operator declaration.

Opaque proof bytes remain unparsed and unverified, even when named `.ots`.
No source bytes are submitted and no process/network port exists here. A
successor is a different candidate, not an authenticated OTS upgrade. Parsed
subject binding, pending calendars, anchoring, Bitcoin/verifier assumptions and
timestamp bounds need a later exact tool/format contract.

## Proposed local commitment-inspection extension (November 10, 2026)

Add a separate bounded OTS v1 SHA-256 parser instead of changing existing opaque
receipt meaning. Evaluate supported operations locally; distinguish a matched
caller-supplied digest, mismatch, unsupported structure and malformed input.
Preserve pending URI privacy and unknown attestation opacity. Bitcoin height
and resulting Merkle-root values remain unverified claims, with no selected
chain, verifier, timestamp or ownership trust. No file/process/network port or
proof mutation is added. See [protocol scope](../../../ots-commitment-inspection.md).
This extension is proposed; ten synthetic cases remain uncompiled/unrun and
interoperability/security qualification is deferred to flow #88, last.

## Alternatives and consequences

Mutating the original archive conflicts with its strict inventory and immutable
subject. A universal provenance engine exceeds Aniflow's temporal boundary.
This bounded sidecar reuses current local I/O and byte-retention owners but
duplicates the selected subject so it can be restored without original paths.
The first file-subject lane is capped at 256 MiB; larger works can select the
canonical digest manifest while their original archive remains retained.

Partial roots are preserved and refused, never adopted, cleaned or overwritten.
Exact readback is repeatable; no automatic retry/repair is introduced. Tests
and all runtime/CI qualification remain deferred to flow #88, scheduled last.

## Review triggers

Review before accepting network submission, cryptographic proof parsing,
automated upgrades, stronger history/authenticity claims or broader resource
and platform guarantees. The original #82 acceptance criteria remain open.
