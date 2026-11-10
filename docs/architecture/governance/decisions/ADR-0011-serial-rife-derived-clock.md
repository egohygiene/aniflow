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

The November 7 #110 source extension carries current accepted RIFE output into
ordinary master/delivery assembly with a separate exact 2x clock and a distinct
derived-temporal validation contract. It preserves original inspection/audio
and normal v3 acceptance. See [the checkpoint](../../../validation/aniflow-110-derived-assembly.md).
This additive implementation note changes neither proposed status nor the
unrun native/resource/visual qualification requirements.

No accepted decision is superseded. This proposal extends typed tool ports,
public contracts and exact temporal truth within their existing ownership.

## Proposed short-preview extension (#110, November 8, 2026)

Share the existing serial pair renderer and component inventory with a fixed
ordinary v3 preview stage. Keep the complete original index/inspection and
source hashes unchanged. Select one explicit window at most 120 source frames
and ten seconds; compute only its at most 240 globally mapped derived PNGs.
Preserve full-run midpoint/cut/endpoint rules, including one right-context frame
when required. Record original rational presentation intervals, not a guessed
zero-origin excerpt or fabricated whole-run clock. Separate preview media/report
types cannot be consumed as full RIFE output or assembly acceptance.

Keep full-input admission I/O and finite tool/model/pair/output storage visible.
Resume uses current ordinary v3 acceptance only. Caller memory observations and
estimates are not measured quotas. No perceptual approval, full-run promotion,
video/audio assembly, native/device qualification or publication is inferred.
Revisit before preview encoding, montage selection, multiple windows, adaptive
sampling, automatic review or per-pair checkpointing. This remains proposed;
source or merge does not ratify this extension. Cases are authored uncompiled/
unrun; deep qualification remains in flow #88, scheduled last.

## Proposed accepted-spatial input extension (November 9, 2026)

Admit current accepted Upscayl frames before fresh RIFE in the fixed recipe.
Reuse the serial pair engine and v3 execution owners with an explicit spatial
binding and distinct output-index type. Preserve original inspection, spatial
dimensions and derived temporal clock separately. Assembly owns the accepted
RIFE writer and current lineage checks. Existing resource limits still refuse
unsupported geometry; no arbitrary ordering or RIFE-before-upscale is implied.
See [scope](../../../recipe-spatial-rife.md). Six synthetic cases are authored
uncompiled/unrun. This extension is proposed and grants no native qualification.

## Proposed watchable-preview extension (November 10, 2026)

Add one separate ordinary v3 encoder capability over an exact currently accepted
PNG preview, its pinned report and complete original index. Reuse the existing
FFmpeg/FFprobe 8.0.1 process and media-observation owners. Keep original input
evidence immutable and validate global source mapping before and after work.
Create a silent H.264 intermediate viewing copy with exact rational frame count
and period. Container timing is explicitly local zero; retain the original
global frame/time/source mapping in a distinct report. No full-run or assembly
carrier can be inferred from this intermediate artifact.

Make RGB8-to-BT.709 limited viewing interpretation explicit without authenticating
source color. Refuse odd geometry, unsupported clocks and exceeded finite
resource bounds. Reuse native lifecycle and exact resume identity; retain failed
work instead of silently adopting or clearing it. Grant no visual, final-video,
publication or runtime qualification authority. See [usage](../../../rife-preview-movies.md).
Ten synthetic cases are authored uncompiled/unrun. This extension remains
proposed; neither source delivery nor merge ratifies it. Flow #88 remains last.
