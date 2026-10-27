---
schema: aether.architecture-document/v1
id: aniflow-system
title: aniflow System
kind: architecture-document
version: 0.1.7
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
Main `8cb00a323b1829de1e4b958236b6aa8529eb7045` exposes a library facade and
a provider-native registry/runtime, including merged named repair PR #128,
selected media setup PR #129 and bounded native chunks PR #127. Open draft
PR #130 adds the selected assembly profile. The current #35 recipe checkpoint
stacks on that branch at `1893bfb994d79e75b08d32739ed61aa23e58cde9`;
no assembly or recipe merge is claimed.
Pipeline v2 frame, batch, audio, and whole-video processors are explicitly
registered and use that bounded runtime, including locks, reports, cancellation,
and output observations. Exact rational clocks, explicit stream selection and
discard acknowledgement replace the older first-stream/average-rate behavior.
Pipeline v3 owns planned nonempty output sets and layered acceptance; the
toolchain owns bounded diagnostics, selected audio preflight, inert audio
registration preparation and selected allenk/Upscayl configuration preparation.
The media setup mapping reads only explicitly selected dependency files, returns
one reviewable typed configuration, and retains diagnostics when unready. It
reads no media, launches no tools and writes/registers/installs nothing; source
planning and processing remain in the existing media facades. RIFE remains a
serial 2x frame/index provider without final-video assembly. #36 includes selection preview and immutable decoded import.
Generic selective repair composes import and explicit direct-image processing,
preserving unselected bytes and the full source mapping. Its fresh-only policy
remains unchanged. The distinct typed allenk image profile shares that loop and
adds whole-stage resume after current component checks and exact importer
provenance admission. Interrupted repair retries all selected frames; partial
files do not become checkpoints. Compiled profile bindings do not qualify the
host's shared libraries or visual quality.
Merged #37 per-frame Upscayl recovery uses existing immutable stage checkpoints
and a complete-set collector. Merged PR #127 adds bounded native
chunks for longer videos: complete chunks reuse and interrupted chunks retry
in full. Exact source, executable/model/settings and chunk-boundary bindings
retain observed decoded geometry for every frame. No second runtime, global
cache or native-platform qualification is introduced.

The selected #35 `video_assembly` facade adds an audiovisual FFV1/bgr0 MP4
master and an H.264/yuv420p/faststart MP4 delivery through two existing ordered
v3 stages. When audio is selected, both copy original admitted AAC packets and
compare their payloads, integer timestamps, extradata and priming. Original
indexed PNGs or Upscayl
spatial output indices bind unchanged source clocks and separate output
geometry. Master validation compares decoded pixels and frame clocks; imported
PNG/source lineage stays declared and prior processing stays unknown. Exact
FFmpeg/ffprobe 8.0.1 identities are required. Unsupported VFR, offsets, long
A/V tails, HDR, alpha, rotation and derived RIFE clocks are refused. This
profile creates no new runtime or mux owner. See the
[assembly guide](../../video-assembly.md).

The fixed music-video recipe coordinates indexed inputs, frame preparation,
optional Upscayl and assembly through existing child v3 runs. Named repair
includes import; disabled repair still selects standalone immutable import.
Immutable intent and fixed child paths record requested work; they are not
new checkpoint or acceptance authority. Later exact child plans remain pending
until accepted predecessor outputs supply their actual bytes and updated index.
Disabled optional steps retain a reason. The recipe does not extract frames,
automate manual review or expand any child's media/resource support. Broader
manual/review recipes remain #107 work; see the
[recipe guide](../../music-video-recipe.md).

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
