---
schema: aether.architecture-document/v1
id: aniflow-ontology
title: Aniflow Ontology
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-ontology
depends_on:
  - aniflow-purpose
  - aniflow-vision
  - aniflow-principles
  - aniflow-foundations
  - aniflow-epistemology
related:
  - aniflow-system
  - aniflow-architecture
supersedes: []
---

# Aniflow Ontology

## Domain scope and boundaries

The domain is ordered video media and the transformations required to produce a
validated temporal master. Collection membership, duplicate policy, general
document rendering, downstream publication packages, cross-tool orchestration,
and origin or authorship judgments remain outside the domain.

## Canonical concepts

| ID | Canonical term | Definition |
| --- | --- | --- |
| temporal-source | Temporal source | Immutable source artifact plus observed stream and timing evidence |
| stream | Stream | Ordered encoded or decoded media component with identity, type, time base, and disposition |
| timeline | Timeline | Model relating stream timestamps, durations, ordering, and synchronization |
| temporal-component | Temporal component | Frames, audio samples, subtitles, or other ordered material derived from a source |
| decomposition | Decomposition | Declared extraction of temporal components while retaining their relationship to the timeline |
| processor | Processor | Typed capability that consumes declared artifacts and produces declared artifacts |
| processor-chain | Processor chain | Ordered processors in which each accepted output becomes the next input |
| stage | Stage | One planned, isolated unit of work with inputs, outputs, state, and validation |
| execution-plan | Execution plan | Resolved, serializable intent for a run before work begins |
| run | Run | One execution attempt bound to a source identity and execution plan |
| artifact | Artifact | Immutable content identity plus typed observations and one or more locations |
| checkpoint | Temporal checkpoint | Reuse evidence binding stage identity, inputs, plan, implementation, and validated outputs |
| validation-observation | Validation observation | Recorded result of applying a named validation method to a scoped subject |
| master | Master | Reconstructed video artifact that passed its declared temporal and delivery validation |
| run-evidence | Run evidence | Structured observations, decisions, events, logs, and relationships produced by a run |

## Relationship model

A temporal source contains streams whose timestamps form a timeline.
Decomposition derives temporal components while preserving their timeline
relation. An execution plan orders stages and processor chains. A run executes
that plan, producing immutable artifacts and validation observations. A valid
checkpoint may allow compatible work to be reused. Reconstruction joins chosen
components, and only successful master validation yields a master.

## Canonical invariants

- Every stream and artifact has identity distinct from its path.
- Component order and timing relation are explicit.
- A processor cannot redefine the source or timeline silently.
- A stage state and its artifact validation are related but distinct facts.
- A checkpoint is evidence of compatibility, not merely a completion marker.
- A master is a validated reconstruction, not any final filename.
- Run evidence describes Aniflow's observations without claiming unobserved
  authorship, intent, or authenticity.

## Aliases and deprecated terms

| Term | Treatment |
| --- | --- |
| Frames | Use only for an ordered image component set, never as shorthand for the complete video |
| Completed | Use only after declared validation; otherwise prefer process-exited or artifact-present |
| Cache marker | Deprecated as a synonym for checkpoint because a marker alone proves insufficient compatibility |
| Provenance | Qualify as run evidence, source evidence, or an external provenance claim rather than using the term ambiguously |
| Final video | Prefer master only when master validation has passed; otherwise use output artifact |

## Concept lifecycle

New concepts require a distinct meaning supported by domain evidence. Renames
preserve aliases and migration impact. Implementation types, database rows,
commands, and directory names do not automatically become domain concepts.

## Assumptions and open questions

Segment and scene concepts are intentionally absent until targeted invalidation
or continuity analysis establishes a stable meaning. Timeline normalization may
be both a planning decision and a stage; fixture work must settle that boundary.

## Validation

Schemas, public Rust types, CLI documentation, and architecture use these terms
without silently redefining them.
