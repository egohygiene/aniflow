---
schema: aether.architecture-document/v1
id: aniflow-system
title: aniflow System
kind: architecture-document
version: 0.1.5
status: draft
owners:
  - egohygiene
created: 2026-08-13
updated: 2026-10-27
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
| Optional processors | Specialized transformations explicitly registered, resolved, and invoked through typed capabilities |
| Local operating system | Filesystem, process, clock, signal, and resource boundary |
| flow | Optional external consumer and cross-holon orchestrator |
| renderflow and optiflow | Sibling holons with no direct aniflow dependency or selection relationship |

aniflow emits a validated master and domain evidence. flow may pass those
artifacts to other tools, but aniflow does not choose or invoke a sibling as part
of its target architecture.

## Current-state evidence and gaps

The source package remains 0.3.0, with no GitHub release observed on 2026-10-27.
Base main `a717935b49fccf87151124a6e20efeb4281ff11c` exposes a library facade and
a provider-native registry/runtime.
Pipeline v2 frame, batch, audio, and whole-video processors are explicitly
registered and use that bounded runtime, including locks, reports, cancellation,
and output observations. Exact rational clocks, explicit stream selection and
discard acknowledgement replace the older first-stream/average-rate behavior.
Pipeline v3 owns planned nonempty output sets and layered acceptance; the
toolchain owns bounded diagnostics, selected audio preflight and inert
registration preparation. RIFE remains a serial 2x frame/index provider without
final-video assembly. #36 includes selection preview and immutable decoded import.
The current selective-repair checkpoint composes import and explicit direct-image
processing under the existing runtime, preserving unselected bytes and the full
source mapping. Its generic profile does not qualify named tools or visual quality.
Merged #37 source provides bounded per-frame Upscayl recovery. The current
checkpoint adds explicit bounded native chunks for longer videos, with one
existing immutable stage checkpoint per chunk and a complete-set collector.
Compatible completed chunks reuse; interrupted chunks retry in full. Exact
executable/model/settings and chunk-boundary bindings retain observed decoded
geometry for every frame. This adds no second runtime, global cache or
native-platform qualification.

These are authored source boundaries. #64 retains deferred qualification and
the unresolved alignment cancellation failure; merge does not prove native
safety or a released consumer contract. See the
[current roadmap](../../../ROADMAP.md#current-source-and-work-order) and
[architecture gaps](ARCHITECTURE.md#current-implementation-gaps).
`run.rs` still combines compatibility-pipeline
planning, coordination, validation, state transition, and delivery
responsibilities; wider system separation remains roadmap work.

## Assumptions and open questions

The public boundary may initially remain one Cargo package containing a library
and binary. Separate adapter or processor SDK packages require consumer and
release evidence rather than architectural enthusiasm alone.

## Validation

System tests exercise capabilities at their boundaries, while architecture
reviews reject overlapping ownership or cross-holon leakage.
