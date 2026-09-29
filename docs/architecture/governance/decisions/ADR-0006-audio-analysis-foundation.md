---
schema: aether.architecture-decision/v1
id: aniflow-adr-0006
title: Bound normalized audio analysis to explicit source-relative evidence
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
  - aniflow-architecture
  - aniflow-roadmap
---

# ADR-0006 — Bound normalized audio analysis to explicit source-relative evidence

## Status and authority

Proposed for maintainer review with
[#42](https://github.com/egohygiene/aniflow/issues/42). No acceptance date or
accepted decision is inferred from implementation, tests or a feature merge.

## Context

The [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13)
needs independently reviewable technical, musical, timed-text, transcription,
lyric-alignment, MIDI and stem capabilities. Consumers need shared source and
evidence identity without parsing each analyzer's native output. The existing
Pipeline v3 runtime already owns execution, checkpoints and run-local resume.

Generalized container timing, stream selection and synchronization remain #32.
Provider-backed layered validation gates remain #33. Deferring the entire
analysis contract to those issues would block useful audio-only work; pretending
that an evidence document solves those larger boundaries would overstate it.

## Proposed decision

Publish the independently versioned `aniflow.audio-analysis/v1` foundation as a
strict schema and public Rust model. Keep the first checkpoint limited to the
provider-neutral envelope, source identity, exact source-relative audio clock,
observation/event primitives, provenance, diagnostics and artifact references.
Family-specific payloads and executable adapters belong to their owning child
issues rather than a speculative universal analyzer model.

Support only the declared audio-only source profile. Sample frames, rational
conversion and range endpoints have explicit semantics; unsupported origins
and stream mappings are refused. Preserve deterministic, heuristic and
probabilistic classification independently of contract validity. Confidence is
bounded evidence or explicitly unavailable/uncalibrated; validation never
manufactures confidence or confers reviewed lyric authority.

Keep core contract fields closed. Namespaced provider extensions are optional
supplemental evidence; they cannot change required semantics or replace missing
core facts. An incompatible core field, value or behavior requires deliberate
contract evolution rather than silently accepting a field a consumer ignores.

Treat an analysis document as an immutable artifact of the existing provider
boundary. Existing Pipeline v3 plans, provider locks, observed input/output
identities and checkpoints remain the owners of execution and resume identity.
Parsing an analysis document establishes its artifact-specific invariants,
not process success, media authenticity, model quality, source decodability or
a general completion gate.

## Rationale

A small common contract lets each capability ship as a bounded feature while
consumers retain source-time and provenance meaning. Exact source-relative
coordinates avoid adding floating-point timing ambiguity to the new boundary.
Closed fields make unsupported meaning visible. Explicit estimate and authority
labels keep useful model output distinct from verified or user-reviewed facts.

## Evidence and assumptions

Observed: #8 / PR #41 already specializes the provider runtime for bounded
stem separation. Its source-preserving outputs and provider-owned evidence
remain distinct from Pipeline v3 built-in artifact-integrity validation.
Observed: #13 has ten checkpoint issues with #42 as their foundation.
Assumed: a bounded audio-only clock and generic observation primitives are
sufficient until each family supplies concrete payload and fixture evidence.
The first checkpoint does not prove any analyzer's accuracy or broad input
support.

## Timed-text companion implementation note

[#47](https://github.com/egohygiene/aniflow/issues/47) applies this boundary to
the separate `aniflow.timed-text/v1` document and its context, registry and
conversion companions. A typed library utility converts explicitly registered
text subsets behind the thin CLI. It reads no audio, invokes no analyzer and
does not require a provider stage or introduce another checkpoint store.
The existing normalized audio contract is unchanged.

Untimed text, start-only cues and explicit intervals remain distinct. Exact
millisecond rational times are preserved unless the caller authorizes a named,
reported target-format precision loss; missing timestamps and cue ends are
never inferred. Known semantic loss requires an explicit allowlist, while
unsupported rich syntax refuses. Source labels remain distinct from unique
internal cue identity.

Observed transcripts and supplied reviewed-lyrics declarations remain separate.
An optional validated audio-source declaration bounds cue timing without
proving inspected bytes or synchronization. Text-only carriers may omit evidence
that their syntax cannot represent; normalized companions retain it and the
conversion report names those omissions. Conversion does not authenticate
review authority or create a general runtime completion gate.

The file facade preserves its input and exclusively creates a new package with
create-new payload/companion files. It publishes the conversion report last as
an atomic completion marker; the complete directory is not an atomic rename.
Incomplete publication is never a successful conversion. This implementation
note records the bounded companion without accepting this proposed ADR or
claiming release qualification.

## Alternatives considered

- Expose only provider-native JSON: requires each consumer to reconstruct
  source-time, confidence and provenance semantics.
- Design every future family payload immediately: enlarges the first review
  and freezes assumptions before their feature evidence exists.
- Rewrite the runtime or generalized timeline now: duplicates established
  execution ownership and absorbs #32–#34 into a contract checkpoint.
- Use floating-point seconds as canonical timing: loses exact sample identity
  and leaves rounding policy implicit.

## Trade-offs and expected consequences

The supported input and observation subset is deliberately limited. Later
families may require new versioned semantics, and strict consumers will refuse
unknown core meanings until updated. Producers must retain evidence identity
and callers must perform semantic validation as well as JSON Schema validation.
Those costs keep compatibility and uncertainty reviewable across independent
consumers.

## Security, privacy and accessibility impact

The contract grants no process, network, filesystem or mutation authority.
Artifact identities use digests and logical references rather than granting
paths or URLs execution meaning. Producers remain responsible for excluding
secrets and unnecessary private content from persisted evidence and extensions.
Typed diagnostics make refusals accessible to downstream automation.

## Review triggers

Review before expanding supported origins, container stream mappings, timing
conversion policy, observation semantics, extension requirements or artifact
validation authority. Also review if real family fixtures require consumers to
interpret provider-native extension data for a required core outcome.

## Related artifacts

- [Audio analysis contract guide](../../../audio-analysis.md)
- [Lyrics and timed-text guide](../../../timed-text.md)
- [Published contract index](../../../contracts/README.md)
- [Temporal provider contract](../../../provider-contract.md)
- [Repository roadmap](../../../../ROADMAP.md)

## Validation

Synthetic round-trip and negative fixtures compare Rust serialization and the
published schema, exercise exact timing/scope/ordering, and reject invalid
confidence, units, identity and missing evidence. Future capability checkpoints
add their own provider and media evidence. This record's proposed status is
separate from those implementation checks and from release qualification.
