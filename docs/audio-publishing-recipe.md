# Continue an accepted audio master to a manual posting package

`music_video_recipe::publishing::audio` is a separate fixed audio entry:
optional native WAV/FLAC export → optional exact final-file QC → destination
preflight → supplied whole-file/public-metadata review → copy-only package →
optional **unsealed retention handoff**. It uses existing native export, signal,
QC, posting and release-evidence owners. It adds no uploader or new runtime.
The existing video publishing entry keeps its request and wire meanings.

Supply `aniflow.audio-publishing-request/v1` with:

- `subject`: current `accepted_pcm` (`file`, `signal_run_directory`) or current
  `accepted_audio_export` (`run_directory`). Loose media and nested QC do not
  supply producer acceptance. The original source is complete zero-origin
  native PCM, within the existing ten-minute profile.
- `export`: optional existing `aniflow.lossless-audio-configuration/v1` for one
  explicitly selected WAV/FLAC candidate. Export starts from accepted original
  PCM only. Select a prior accepted export directly without another export.
  Integer FLAC retains its original precision/rate/channels/count; float-to-FLAC
  conversion still refuses. No dithering, gain, resampling or downmix is inferred.
- `audio_qc`: optional existing native inspection `configuration` plus
  `maximum_pcm_bytes`, `maximum_working_bytes`, `minimum_free_bytes`, for the
  [exact final audio QC owner](final-lossless-audio-qc.md). QC runs after every
  selected byte-changing export. `provider_executable` names the explicit
  compiled native facade provider whenever export/QC is selected.
- `profile` and `evaluated_on`: one supplied dated manual SoundCloud/DistroKid
  PCM requirement profile. No live policy lookup or automatic refresh occurs.
  Required loudness requires selected final QC. Unsupported-rate meter values
  stay unavailable and block a profile that requires measured quantities; they
  never borrow original-master meters or silently resample the song.
- `package`: the existing explicit public release metadata, metadata-scope
  policy and package storage bounds. Original PCM can contain inherited tags
  and needs explicit scope acknowledgment unless a separately accepted native
  export removes inherited descriptive tags. Native lossless sanitation remains
  limited to its declared scope; structural/vendor and signal/signature
  exclusions stay visible. The public release sidecar does not fill a platform's
  manual upload form or promise that platform's current requirements.
- `retention`: optional existing finite bundle/free-space bounds. This prepares
  an exact selected package-media/public-sidecar retention request and plan,
  without executing a seal, proof, signature, timestamp request or release.

```bash
aniflow --output json delivery audio-publishing plan --request audio-publishing.json
aniflow --output json delivery audio-publishing run --request audio-publishing.json --run-directory /private/audio-publishing
aniflow --output json delivery audio-publishing status --run-directory /private/audio-publishing
aniflow --output json delivery audio-publishing next-action --run-directory /private/audio-publishing
aniflow --output json delivery audio-publishing record-review --run-directory /private/audio-publishing --declaration my-actual-audio-review.json
aniflow --output json delivery audio-publishing resume --run-directory /private/audio-publishing
```

Plan is read only: current source acceptance/components and at most the first
native export plan are frozen. Future final media/QC/review/package identities
are resolved only from actual accepted predecessors. Run reserves a fresh
private root with `audio-publishing-intent.json`, fixed `children/audio_export`,
`audio-qc`, exact preflight and supplied-review markers, and `package`. Ordinary
v3 owns native execution, acceptance, checkpoints, failure and compatible export
resume. The parent stores intent/locators and exact marker bindings, never a
second native acceptance database.

Execution pauses at `awaiting_review`. Review the **complete exact final file**,
public metadata, supplied requirement comparison, selected final QC and declared
scope, then supply an actual existing `ManualReview` approval or rejection with
the exact context/subject hashes returned by `next-action`. Reviewer identity
and the decision are operator declarations, not authenticated identity or
machine-generated musical judgments. Recording a decision executes no package
or successor step. After approval, a separate resume creates the local package.
A rejection is retained; changed decisions/subjects require a new intent.

Preflight needing changes or evidence returns a blocked state. Current original
source/acceptance, source rate/channels/precision, tools/settings, profile/date,
public metadata or QC changes refuse stale reuse. Partial native export uses
only its exact retained v3 plan. Partial QC/binding or package initialization is
retained and refused for explicit reconciliation; no guessed output adoption,
replacement, cleanup or overwrite occurs. A final-file metadata-only byte edit
invalidates its QC and review even when decoded PCM remains equal.

Status and advice execute nothing and create no child or review. Video and
audio entries share final review/package/retention readback. Publishing marker
presence now admits canonical regular files and directories, while exact slot
layout still checks their required kinds and refuses symlinks/special files.
Per-phase writer guards and repeated current observations preserve owner
boundaries; the QC owner takes its actual subject writer without double-locking.
Finite native/storage/capture bounds are admission limits, not an OS disk quota.

Ten SDK/CLI cases are authored for fixed order, required QC, original-only export,
closed audio routes/options, missing acceptance, exact rejection binding, marker
file/directory safety, early retention refusal and CLI boundaries. They remain
uncompiled/unrun. Tests/compiler/build/dependencies/lint/format/schema/docs/
corpus, product CLI/SDK/native/provider/media/model and CI execution remain
fully deferred to flow #88, scheduled last with its original 45-ticket ledger.
No private Akashic assets, actual review or publishing were used. Known alignment
cancellation/start-marker failure remains unresolved; cause unproven. ADR-0019
remains proposed and no release/platform/native qualification is implied.

## Explicit v2 resumable final QC

Select `aniflow.audio-publishing-request/v2` with the same closed request fields
to select the recognized resumable final-QC owner and explicit native 44.1/48 kHz
meter views. Matching intent/plan/status and parent QC binding use v2. Existing
v1 intents retain their fresh-only QC and 48 kHz meter selection. No saved root
is upgraded merely by installing newer code; use a new v2 intent/root.

The existing audio-publishing plan/run/resume/status/next-action CLI delivers
both explicit request versions. Within v2, `audio-qc/resume-intent.json`, committed
capture/decode markers and the actual `children/signal` native child determine
reusable work. The parent requires the exact selected final-file QC request and
current native evidence. Recognized incomplete QC produces `resume` advice at
`final_audio_qc`. Unknown/legacy/half-written roots still refuse. A complete QC
child whose parent binding was interrupted can be rechecked and bound on a
separate resume; no child process is repeated simply to recreate that binding.
Preflight/review/package successors are forbidden before that binding completes.

Current exact-file preflight still precedes supplied whole-file/public-metadata
review. Recording a declaration runs no package; approval needs separate resume,
and rejection remains immutable. Retention stays a selected unsealed handoff.
No uploader, sealing, release action, automatic approval or source replacement
is added. All source cases remain uncompiled/unrun and flow #88 stays last.
