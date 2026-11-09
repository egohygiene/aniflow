---
schema: aether.architecture-decision/v1
id: aniflow-adr-0013
title: Prepare local destination candidates from accepted temporal-media evidence
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
  - aniflow-adr-0001
  - aniflow-adr-0002
  - aniflow-adr-0004
  - aniflow-adr-0005
  - aniflow-adr-0008
---

# ADR-0013 — Prepare local destination candidates from accepted evidence

## Status and authority

Proposed for #83's selected source checkpoint. Source, draft or merge does not
ratify this record. No acceptance date, actual human decision, native/platform
qualification or publication is asserted. Execution remains deferred to flow #88.

## Context

Existing final-video completion and accepted PCM signal evidence already own
technical observations. Destination requirements depend on route, account and
date; a static codec list cannot establish current service acceptance. Copying
those owners into another media engine or treating a report as an approval would
lose the original source, temporal and authority distinctions.

## Proposed decision

Expose one domain-owned library operation and thin delivery command. Resolve
actual current accepted video or original PCM bytes through existing owners,
then compare them with closed, finite, explicitly supplied requirement evidence.
Record its source URLs/checker/date/validity and an explicit evaluation date.
No implicit current-policy lookup, service, model, transform or posting port.

Use matches-supplied-profile, needs-changes and missing-evidence outcomes.
Preserve complete picture/song extents, exact rates/channels/PCM precision and
unknown loudness. Recheck saved reports through current accepted owners before
downstream use. Keep complete observation/requirement/provenance locators private.

Later local package or exact-byte evidence operations must remain independently
selected and separately versioned. They may reuse this evidence, pinned copies
and explicit declarations; they may not generate approval, change the accepted
subject, mutate historical proof or claim posting/signing/timestamp authority.

## Rationale and trade-offs

This preserves independent libraries/CLIs and keeps cross-holon orchestration in
flow. Explicit requirement evidence needs maintenance by its supplier and is not
independent platform qualification. Initial fixed MP4/H.264 and accepted PCM WAV
lanes refuse broader inputs; no speculative conversion framework is introduced.
Read-only freshness is a point-in-time check, not a reservation against file edits.

## Consequences and review triggers

The subsequent #83 local-package checkpoint implements a separate selected
copy-only operation: a current matching preflight, explicit public fields and
metadata-scope policy define a private review context; a supplied approving
declaration is required before a fresh private root is reserved. Fixed public
media/release JSON stay separate from private request/review evidence. Pinned
copies and native rechecks precede the completion-last receipt. Inherited or
uninspected metadata requires explicit acknowledgment; failed/residual checks
cannot be bypassed. Package verification rechecks current producer authority.
This source extension does not ratify the proposal or grant publication rights.

The subsequent #82 local retention checkpoint separates immutable canonical
subject bytes/copies from a completion receipt and bounded append-only local
byte-check observations. Current native/package evidence is captured at selection;
unknown file history remains explicitly declared. Archival verification requires
a caller-supplied known manifest digest and does not reassert live producer,
ownership or timestamp authority. The unsigned local clock/history cannot become
a trusted existence proof. Signing, OTS networking and proof upgrade/restore remain
separate unimplemented feature scope. This extension also remains proposed.

Public profile/report contracts need compatibility and tamper/refusal coverage.
Review before adding encoders, API/account discovery, new formats, credential
transport, automatic policy refresh/approval, public private-evidence projection,
signing or timestamp networking. Preserve all original #83/#82 requirements;
source-only preflight does not complete their export or proof lifecycle.
