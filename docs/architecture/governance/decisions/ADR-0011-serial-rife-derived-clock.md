---
schema: aether.architecture-decision/v1
id: aniflow-adr-0011
title: Bound RIFE interpolation with serial pairs and a derived clock
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
  - aniflow-roadmap
  - aniflow-adr-0003
  - aniflow-adr-0004
  - aniflow-adr-0005
---

# ADR-0011 — Bound RIFE interpolation with serial pairs and a derived clock

## Status and authority

Proposed with #114 under #110. Implementation delivery is not recorded
maintainer acceptance or qualification. Tests and native execution are deferred
under #64.

## Context

Interpolation changes frame cardinality and requires source-pair mapping.
Ordinary reconstruction evidence preserving one output per source frame cannot
validate that transform. The operator reports a memory-heavy RIFE invocation
crashed a MacBook; the exact invocation and failure mechanism are unobserved.
Upstream directory queues and optional augmentation can amplify memory use.

## Proposed decision

Expose one explicit RGB8/SDR indexed-frame profile through the existing provider
runtime. Initially use midpoint-only 2x same-duration interpolation with a
derived rational rate, exactly 2N outputs, declared cut holds and a final
endpoint hold. Keep source temporal inspection, frame digests and derived
mapping distinct. A reusable output-index validator recomputes the exact plan
and source mapping; byte/decode acceptance remains separately scoped.

Run one pinned RIFE pair process at a time, with `1:1:1` workers and no spatial
or temporal augmentation/UHD flag. Snapshot exact tool/model bytes privately,
retain the `rife-v4.6` model directory name required by upstream selection and
use the existing inherited process runner. New timeouts, shell execution,
provider-selection engines or cross-tool dependencies are unnecessary.

Default to disabled and explicit CPU device selection. Resolve the automatic
Mac policy conservatively; larger explicit or admitted Linux presets change
padded-pixel/memory admission, never worker concurrency. Memory estimates and
operator host facts remain candidly uncalibrated and unenforced. No preset
establishes native safety or a hard RAM/VRAM limit.

## Alternatives and consequences

A generic MP4 external-command stub does not match RIFE's frame-pair CLI.
Treating interpolation as an unchanged-cardinality frame processor would
misrepresent temporal provenance. Directory-batch mode retains more concurrent
buffers and long-lived allocator state; the first profile chooses process
teardown at each midpoint at the cost of repeated model loads.

All-core/device discovery and live resource probing remain separate authority
and qualification work. The provider does not extract media, resize inputs,
infer cuts, mux audio or encode a final video. Frame-index/color declarations
are import evidence rather than independently authenticated native inspection.

## Evidence and review triggers

Synthetic planning, wire shape, paths, pin drift, timeout, missing output,
cut/endpoint and semantic-evidence cases are authored; none are reported passed.
Review before expanding rates, input profiles, host probes, augmentation,
directory concurrency, finer checkpoints or final-video derived-clock gates.
Parent #110 retains final assembly and real Apple Silicon resource measurements.

## Lineage

No accepted decision is superseded. This proposal extends typed tool ports,
public contracts and exact temporal truth within their existing ownership.
