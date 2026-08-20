---
schema: aether.architecture-document/v1
id: aniflow-system
title: aniflow System
kind: architecture-document
version: 0.1.0
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-08-13
governed_by:
  - architecture-system
depends_on:
  - aniflow-purpose
  - aniflow-vision
  - aniflow-foundations
  - aniflow-ontology
related:
  - aniflow-architecture
  - aniflow-roadmap
supersedes: []
---

# aniflow System

## Purpose and scope

aniflow is one temporal-media system composed of a small set of capability
systems. This inventory assigns one primary owner to each domain capability
without treating the current source modules as permanent system boundaries.

## System inventory

| System | Purpose | Primary capabilities |
| --- | --- | --- |
| Source intelligence | Establish what the input is and whether aniflow can treat it safely | identity, probing, stream inventory, timing observations, support checks |
| Planning | Turn configuration and discovered capabilities into resolved intent | configuration validation, capability resolution, execution plan, plan digest, preflight |
| Temporal transformation | Preserve temporal relationships through decomposition, processing, and reconstruction | component extraction, processor-chain coordination, stream selection, assembly, encoding intent |
| Processor execution | Run specialized capabilities behind bounded contracts | typed processor discovery, direct process execution, resource bounds, progress, cancellation, output collection |
| Validation | Determine whether components, stages, and reconstructed artifacts satisfy declared contracts | structural, temporal, stream, synchronization, decodability, and delivery validation |
| Run control and evidence | Own execution lifecycle and recoverable state | workspaces, events, atomic manifests, checkpoints, invalidation, status, diagnostics, retention metadata |
| Delivery | Expose the same aniflow semantics to humans and software | public Rust API, standalone CLI, human presentation, versioned machine output |

## Capability ownership boundaries

aniflow owns temporal-media planning, stage ordering, media handoff, continuity,
reconstruction, validation, and run evidence. A processor owns its specialized
algorithm. It does not own whether its output qualifies as a completed aniflow
stage.

The Delivery system does not implement a second execution path. Source
intelligence does not decide processor policy. Run control does not redefine
temporal validity. Validation does not infer creative quality beyond a declared
method.

## Major interactions

Source intelligence produces observations for Planning. Planning produces an
execution plan for Run control. Run control coordinates Temporal transformation
and Processor execution. Validation evaluates each required boundary before Run
control advances state. Delivery invokes these capabilities and presents their
events and results.

## External system relationships

| External system | Relationship |
| --- | --- |
| FFmpeg and FFprobe | Foundational media adapter for probing, decoding, encoding, filtering, and muxing |
| Optional processors | Specialized transformations discovered and invoked through typed capabilities |
| Local operating system | Filesystem, process, clock, signal, and resource boundary |
| flow | Optional external consumer and cross-holon orchestrator |
| renderflow and optiflow | Sibling holons with no direct aniflow dependency or selection relationship |

aniflow emits a validated master and domain evidence. flow may pass those
artifacts to other tools, but aniflow does not choose or invoke a sibling as part
of its target architecture.

## Current-state evidence and gaps

Version 0.2.0 implements most capabilities in one binary crate. `run.rs`
currently combines planning, execution, validation, state transition, and
delivery responsibilities. The system inventory is therefore accepted target
ownership, not a claim that these boundaries already exist in code.

## Assumptions and open questions

The public boundary may initially remain one Cargo package containing a library
and binary. Separate adapter or processor SDK packages require consumer and
release evidence rather than architectural enthusiasm alone.

## Validation

System tests exercise capabilities at their boundaries, while architecture
reviews reject overlapping ownership or cross-holon leakage.
