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

An explicitly selected v2 final-QC request/intent/report adds original-rate
44.1 kHz native PCM16/PCM24/float32 observations via the same existing two-pass
meter/parser. A v3 meter receipt pins its actual rate, argv, rate-dependent
windows and native sample-peak cross-check. No final sample conversion occurs.
V1 remains 48 kHz only; existing 44.1 kHz unavailable observations are not
reinterpreted or invalidated by this extension. Numeric readings remain
unqualified; independent platform/algorithm execution remains deferred.

The #107/#83 continuation proposes a distinct resumable-QC wrapper over final
QC v2. Capture and decoded-subject boundaries are committed last with immutable
markers. A fixed ordinary v3 signal child owns its exact plan, checkpoints,
resume and acceptance. A retained meter observation can finish final receipt
publication without repeating decode or accepted signal work. Identity-only plan
reconstruction makes status process-free; execution still preflights exact native
tools. Unmarked partial capture/decode/publication and unknown native roots are
preserved and refused, with no overwrite, directory guessing or stale-lock reclaim.

Audio publishing v2 explicitly selects that owner and its original-rate meter
support. A missing parent binding is resumable only after exact recognized-child
readback. Legacy publishing/QC v1 roots and their incomplete-initialization
refusals retain their meaning. Final preflight, supplied whole-file and public
metadata review, separate post-approval resume, local package and unsealed
retention remain separate. These transitions add no native acceptance store,
algorithm qualification, musical approval or remote publication authority.
This ADR remains proposed; source delivery is not ratification.

The #83 video continuation applies the same recognized-boundary pattern to
current accepted final video AAC, through a distinct explicit wrapper. The
existing source-signal decoder, rational extent/parser and exact retained
receipt checks remain shared owners. Complete decoded PCM has one ordinary
native signal child; retained meter evidence can finish immutable signal/final
QC receipt publication after interruption. Status reconstructs exact plans
without probes. Legacy fresh-only video QC and unknown partial roots are not
upgraded; musical review, posting and qualification remain separate. This is
proposed source composition, not ADR ratification.
