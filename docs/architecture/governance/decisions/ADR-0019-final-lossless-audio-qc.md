---
schema: aether.architecture-decision/v1
id: aniflow-adr-0019
title: Bind final lossless audio measurements to exact current native subjects
kind: architecture-decision
status: proposed
owners: [egohygiene]
scope: [aniflow]
governed_by: [architecture-decisions]
supersedes: []
superseded_by: []
related: [aniflow-architecture, aniflow-adr-0013, aniflow-adr-0017]
---

# ADR-0019 — Exact final lossless audio QC

Proposed for #83; source inclusion grants no ratification or qualification.

A source-preserving export proves a PCM relationship and native producer
acceptance. Its original-master measurements are not a new final-file meter
pass. Compose the current exact accepted file with a fresh private complete
native decode, the existing native v3 signal pipeline and a separately versioned
view over the existing two-pass meter. Bind final file identity independently
from original PCM identity, so even a container-only change invalidates QC.

Preserve PCM16/PCM24/float32 precision, complete count, source rate and channels.
WAV is copied exactly; accepted integer FLAC is decoded to raw native samples
and wrapped by the existing minimal WAV writer. The independent native parser
checks the complete PCM digest and clock before signal analysis. No PCM16
fallback, gain, resampling, downmix, trim, padding or source replacement occurs.
The existing 48 kHz meter view can consume native integer/float samples under a
new v2 receipt. Unsupported rates keep typed unavailable quantities. Supported
observations remain unqualified; native-v3 signal report meanings stay intact.

The facade retains immutable intent and a final marker written last, with
ordinary v3 signal authority in the child. Fresh failures preserve partial work
and cannot be adopted as completion. Readback requires an external receipt pin
and current native producer/child/bytes/components. Selected final QC drives
preflight; no original-master meter can replace unavailable final quantities.
Manual review, requirement declarations, packaging and actual publication remain
separate. No new general runtime, acceptance database, uploader or network
provider is introduced. All executed qualification remains in flow #88, last.


The subsequent #107 checkpoint proposes a separate fixed audio publishing entry
using the same native export and exact final QC owners. Shared final review,
manual package and optional unsealed retention checks preserve their existing
bindings. The entry selects only dated supplied SoundCloud/DistroKid manual
profiles; it does not infer live policy, upload, release or seal evidence.
Recording supplied review executes no successor step. Native export resumes
only its unchanged ordinary v3 plan. Distinct audio request/intent/plan/status
schemas preserve the existing video entry and its serialized meanings.
