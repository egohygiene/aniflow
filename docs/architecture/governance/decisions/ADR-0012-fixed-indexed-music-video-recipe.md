---
schema: aether.architecture-decision/v1
id: aniflow-adr-0012
title: Compose the fixed indexed music-video recipe through existing child runs
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
  - aniflow-system
  - aniflow-roadmap
  - aniflow-adr-0001
  - aniflow-adr-0002
  - aniflow-adr-0004
  - aniflow-adr-0005
  - aniflow-adr-0008
---

# ADR-0012 — Compose the fixed indexed music-video recipe through existing child runs

The #107 follow-on source checkpoint adds a separate fixed publishing
continuation after actual native completion/finishing: optional selected export,
exact-final-file QC, supplied dated preflight, explicit final-file/public-field
decision, copy-only manual package and optional unsealed retention handoff.
Native children and existing QC/preflight/package/evidence owners retain actual
acceptance. Freeze only the first native plan; derive later bindings from
actual predecessors, retain rejections, and refuse partial or stale state rather
than adopting it. No graph engine, second media acceptance database, automatic
signing/proof/timestamp/posting or cross-holon adapter is added. This extension
does not ratify the proposal or establish native/platform qualification.

## Status and authority

Proposed with the selected #35 recipe checkpoint. Implementation, draft
publication or feature merge does not record maintainer acceptance of this
proposal. No acceptance date is assigned and no historical #14 acceptance is
backfilled. Tests and native qualification remain deferred under #64.

## Context

Named frame repair, Upscayl recovery and verified assembly already own exact
source-bound v3 plans, provider locks, accepted output and compatible resume.
Users need a direct way to run that selected sequence without manually copying
output paths between commands. A later operation's exact plan cannot bind
repaired or upscaled frame hashes before those outputs exist. A recipe-level
completion database would duplicate the evidence already owned by child runs.

The concrete scope is an existing original source video and complete original
indexed PNG workspace. Broader manual/review recipes belong to #107, and
cross-tool suite orchestration remains owned by flow.

## Proposed decision

Expose one closed library configuration and thin CLI for `prepare_frames`,
optional `upscale`, then required `assembly`. Named allenk repair includes
immutable import; when repair is explicitly disabled, standalone frame import
still prepares the complete frame set. Do not run two importers for the same
phase. Require an explicit reason for each disabled optional operation and
embed the existing selected child configuration when enabled.

Resolve the first exact child plan from available inputs. Report later plans
as pending until accepted predecessor artifacts and the matching updated index
exist. Execution materializes the next plan only from those accepted outputs.
Predicted geometry is an admission declaration, not observed output evidence.

Store immutable recipe intent and use fixed child-run paths for the bounded
composition. These identify requested work and owned runs; they do not mark
media accepted or replace immutable child checkpoints. Child v3 status,
validation, locks and compatible resume remain authoritative. Recipe status
aggregates their current observations and final output references without
launching native tools or promoting loose files. It still reads bounded
recorded source/component identities for compatibility; portable offline
historical proof is not promised. Partial child initialization and abandoned
writer ownership require explicit reconciliation.

Before pending native work, reuse the assembly owner's bounded source-profile
observation so an unsupported source can be refused before expensive repair or
upscale. This observation does not create a new acceptance artifact. Planning,
status and fully accepted resume do not launch that source probe. Preserve the
original source clock, separate output geometry and all child support limits.

## Alternatives and consequences

Keeping only separate commands leaves users responsible for repeated exact
artifact handoffs. A speculative single full plan would need invented future
hashes or weaker compatibility identity. Flattening every child into a new
recipe execution engine would duplicate existing public planning and recovery
owners. The fixed composition uses the already established child facades and
makes unavailable downstream plans explicit.

The recipe gains convenience while keeping visible child runs and their
existing recovery granularity. Retained child snapshots and accepted copies
still cost disk; composition is not an in-place optimization. Initial frame
extraction, arbitrary providers/stage graphs, manual review or approvals,
shared caches and suite-wide orchestration are outside this decision.
The immutable intent and locator boundary needs explicit compatibility review
before expansion, rather than becoming a general workflow format accidentally.

## Evidence and review triggers

Synthetic configuration, phase planning, handoff, status, interruption and
recovery coverage is authored and unrun. Source inspection is not compilation,
execution, native suitability or release qualification. Preserve the known
alignment cancellation failure and its unproven cause under #64.

Review before adding new recipe phases, providers, concurrent children,
mutable recipe edits, cross-recipe reuse, extraction, derived clocks or
review/approval authority. Revisit if child evidence cannot express a real
supported handoff without a second acceptance owner. Parent #35 and #107
retain their remaining product scope.

## Lineage

No accepted decision is superseded. This proposal composes the existing
library-first, polyrepo, versioned-contract and temporal-truth boundaries.
It does not accept another proposed ADR by reference.


## Proposed source-entry extension — 2026-10-28

The original indexed proposal above remains historical scope and retains its
proposed status. The standalone extraction checkpoint in draft PR #133 now
provides the concrete missing predecessor. This extension proposes composing
that existing child before the same preparation/upscale/assembly sequence.
Implementation and publication do not accept this extension or the original
proposal; no acceptance date is assigned.

Expose a distinct `aniflow.source-music-video-recipe/v1` configuration containing
`extraction` and the existing `recipe` configuration. Preserve the original
indexed v1 request, configuration and result shapes. The source facade has
separate intent/plan/status/outcome schemas and uses the same CLI command
without frame flags. Status discovers the immutable intent schema and selects
the matching public facade; it does not infer a mode from loose artifacts.

Keep one immutable source intent and one root with fixed child slots. Bind an
exact extraction plan first. Only after ordinary child acceptance may the
coordinator derive an indexed request in memory from the accepted frame bytes
and original index. Reuse the existing tail planning and execution owners;
never persist a second indexed recipe intent or nested recipe workspace.
Retain extraction writer ownership through the downstream handoff.

Initial source plans cannot know geometry, frame count or the target-resolution
decision. Keep them unavailable instead of inventing a full plan. Observe
mandatory selected source/preparation/assembly identities up front. In target
mode, defer neural executable/model admission until accepted geometry selects
execution; unchanged bypass retains the existing evidence boundary. A caller
selecting unconditional Upscayl still selects its dependencies up front.

Child v3 plans, exact locks, validation, acceptance and compatible checkpoints
remain authoritative. Reuse complete compatible extraction, retry interrupted
extraction as a whole stage, and retain incompatible/incomplete children for
explicit reconciliation. Fully accepted compatible resume launches neither
extraction nor a fresh native source probe. Source entry does not expand media
profiles, add arbitrary stage graphs, RIFE-derived clocks, manual approval or
cross-tool orchestration. It does not convert RGB observations into historical
authenticity, model quality or native qualification.

Useful source-entry regression coverage is authored and unrun under #64.
Broader media/audio-tail profiles, loudness/metadata and workflow qualification
remain #35 work. Prior Upscayl spatial-index entry and qualification remain
#111 work; broader manual/review composition remains #107 work.

## Proposed source continuation — November 6, 2026

The selected #111/#35/#112/#113 source checkpoints extend the same fixed recipe:
accepted prior spatial ports bypass preparation/neural work; an explicitly
selected exact audio-extent policy retains the song's independent origin/end;
selected delivery metadata handling stays within the existing final encoder and
private evidence; a read-only completion view projects existing #33 authority.
No second runtime, generic DAG, analyzer or downstream invocation engine is added.
Flow remains the consumer/orchestrator. Source implementation/merge does not
ratify this proposed ADR or qualify native behavior; #64 deferral remains.

## Proposed accepted-video finishing continuation (November 9, 2026)

After accepted assembly, compose caption preview/review/delivery and selected
metadata-remux owners as one fixed continuation. Preserve the original recipe
intent and ordinary native child authority. Store a bounded intent, fixed child
locators and one supplied generic preview declaration. Metadata runs after
burn-in; soft-mux plus metadata refuses unsupported subtitle preservation. Final
completion waits for all selected children. See [scope](../../../recipe-finishing.md).
This remains proposed; seven synthetic cases are authored uncompiled/unrun. No
native qualification or full creative/publication approval is recorded.
