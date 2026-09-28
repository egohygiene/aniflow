---
schema: aether.architecture-decision/v1
id: aniflow-adr-0007
title: Import declared stem lineage through accepted provider evidence
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
  - aniflow-adr-0003
  - aniflow-adr-0004
  - aniflow-adr-0005
  - aniflow-adr-0006
  - aniflow-architecture
  - aniflow-roadmap
---

# ADR-0007 — Import declared stem lineage through accepted provider evidence

## Status and authority

Proposed for maintainer review with
[#45](https://github.com/egohygiene/aniflow/issues/45). Implementation, validation
or a feature merge does not imply an acceptance date or accepted decision.
ADR-0006 retains its separately proposed status.

## Context

The existing separation provider emits candidate stems and provider-owned
evidence through Pipeline v3. Technical and signal analysis need to consume
those stems without losing their original-mix relationship or making every
consumer parse one separator's output. Merely finding a WAV file or a JSON
document beside it does not establish an accepted stage relationship.

The normalized audio contract already represents a selected stem, original-mix
artifact and relationship-evidence reference. Pipeline v3 owns provider locks,
output acceptance, checkpoints and run-local resume. A second execution or
checkpoint owner would duplicate those responsibilities.

## Proposed decision

Resolve stem selection through an explicit prior run, stage and declared output
artifact ID. Verify the accepted plan, lock, execution report, checkpoint and
referenced artifact observations, then independently observe the bounded mix
and stem bytes. Interpret provider-neutral artifact roles; retain provider
JSON as opaque evidence rather than importing Demucs-specific parsing.

Limit the first profile to a separation stage with no upstream dependencies
that consumes the original pipeline mix directly. Select the entire stem and
all channels. Refuse ambiguous, stale, escaping, partial or unsupported mappings.
Compare exact frame-count/sample-rate durations under a bounded explicit
tolerance. Duration agreement is not proof of waveform, onset or phase alignment.

Reuse ordinary technical/signal providers for measurement, followed by an
ordinary lineage stage that emits the final normalized analysis and relationship
evidence. Preserve raw measurement contracts and bind all applicable scopes to
the selected stem. Retain the existing runtime's cancellation, confinement,
checkpoint and resume behavior.

An imported artifact is an explicitly selected input to a new analysis run.
This does not introduce automatic cross-run reuse under #34 or turn ordinary
provider-produced evidence into #33's general completion gate. Later families
consume the same selected source, scope and evidence instead of implementing
their own separator-specific runtime logic.

## Rationale and alternatives

Accepted runtime evidence supplies a stable import boundary across providers.
Direct-mix/full-stem constraints keep time semantics reviewable without taking
on generalized synchronization from #32. Alternatives are weaker or broader:
filename inference loses declaration authority; Demucs-native parsing couples
consumers to one implementation; arbitrary upstream transforms require timing
and provenance semantics this checkpoint does not establish.

## Trade-offs and consequences

The importer requires the retained accepted run evidence, not only a copied
stem file. Structurally valid documents alone cannot authenticate their author
or qualify separation quality. The bounded profile refuses partial selections
and more complex stage mappings until a later contract can represent and verify
them. Exact imported identities participate in analysis planning and resume.

Each plan/run/resume observes the latest complete source run, then binds exact
accepted historical authority files. The final stage rechecks those bound bytes.
This snapshot does not continuously monitor newly appended source-run revisions
or guarantee current source state at publication; generalized workspace locking
is outside this checkpoint.

## Security and privacy impact

The original mix, selected stem and prior run remain read-only. Referenced
artifacts must stay within the accepted workspace and retain observed identity.
No model execution, download authority or media-mutation permission is added.
Local executables remain trusted code under the existing provider boundary;
hash agreement is not a signature or authenticity claim.

## Evidence, validation and review triggers

Synthetic accepted runs cover the first-party vocals/accompaniment declarations
and an independent provider's declared role. Focused fixtures exercise stale and
missing evidence, duration boundaries, scope refusal, path confinement, source
preservation and compatible resume without model inference. Executed results
belong to the issue's local validation receipt, separately from this proposal.

Review this decision before partial channels/ranges, transformed or multistage
mix mappings, inferred role selection, new clock conversions, relaxed evidence
authority or automatic cross-run reuse. Model quality, native-platform evidence
and release qualification remain separate.

## Related artifacts

- [Stem-lineage guide](../../../audio-stem-lineage.md)
- [Normalized audio foundation](../../../audio-analysis.md)
- [Provider contract index](../../../contracts/README.md)
- [Architecture](../../foundation/ARCHITECTURE.md)
- [Roadmap](../../../../ROADMAP.md)
