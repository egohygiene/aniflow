# Metadata-only remux from the CLI

To preserve a current native soft-caption candidate, pair
`--preserve-subtitles-from-run /private/native-caption-child` with
`--subtitle-artifact-id caption_video` on plan/run/resume. The source path and
explicit selection must match that accepted child: video 0, optional audio 1,
and its subtitle index (2 with audio, 1 without) in `discard_streams` for the
picture/audio observer. That observer selection does **not** discard the track
from output: v2 explicitly maps it with `-c copy` and independently verifies
subtitle configuration, all packet payloads and exact rational times,
disposition, language and complete words/millisecond SRT round trip. No extra
tracks or serialized reports without current acceptance can be adopted.

The SDK uses `RemuxRequest::subtitle_preservation` with
`SubtitlePreservation::observe(native_caption_run, artifact_id, token)`. Leave
the field absent for old v1 semantics. The v1 operation schema/serialized shape
is unchanged; explicit v2 has its own schema identity. A later remux or finishing
root is not a substitute for the original native caption owner. The fixed
finishing tail builds this binding after accepted soft-caption delivery.

Current completion remains `subtitle_muxed_candidate`, not a plain destination
copy. Initial TikTok export/preflight and final audio QC still refuse it. This
change supplies preservation source, not player/platform/native qualification,
full creative approval, proof, signing or posting authority. Six new SDK
relationship/argv/tag cases, one parser case and an updated fixed-order case
are authored, uncompiled/unrun; all execution remains deferred to flow #88,
scheduled last. The known alignment cancellation/start-marker failure remains
unresolved; cause unproven.

`video metadata-remux` delivers the existing
[fixed native remux](validation/aniflow-112-accepted-remux.md) through four
explicit commands. It creates a separate stream-copy candidate. Original source
and master bytes remain untouched; no encode, resize, rate change, normalization
or automatic metadata choice is added.

Supply an existing `AssemblyConfiguration` JSON with exact local tool pins,
finite bounds and an explicit `delivery_metadata` policy: `preserve`, `minimal`
or `clear_inherited_then_apply_allowlist`. See [policy selection](delivery-metadata.md).
The existing delivery CRF/preset fields retain their wire shape but never select
an encode in this remux profile. No author, title, date, URL, identity or legal
authority is inferred.

Supply a separate closed `StreamSelection` JSON (at most 64 KiB), for example:

```json
{
  "video_stream": 0,
  "audio_stream": 1,
  "no_audio": false,
  "discard_streams": []
}
```

Indices are input-global and must name the actual source. The SDK requires an
explicit video selection, exact supported audio behavior and no discarded
streams. For unequal picture/song extents, explicitly select
`"audio_extent": "preserve_exact"`; the existing temporal observer retains each
original rational origin/end. This lane refuses unsupported chapters/programs, extra
streams, source temporal behavior and container/payload profiles; an explicit
selection does not waive preservation checks. It does not trim or guess a song's
duration. Settings/selection files are bounded; source/tool bytes and the closed
metadata operation are freshly observed by the SDK.

```bash
aniflow --output json video metadata-remux plan \
  --source-video "/absolute/source delivery.mp4" \
  --configuration remux-settings.json --selection selected-streams.json
aniflow --output json video metadata-remux run \
  --source-video "/absolute/source delivery.mp4" \
  --configuration remux-settings.json --selection selected-streams.json \
  --output-directory "/absolute/separate existing run parent"
aniflow --output json video metadata-remux resume \
  --source-video "/absolute/source delivery.mp4" \
  --configuration remux-settings.json --selection selected-streams.json \
  --run-directory "/absolute/existing remux run"
aniflow --output json video metadata-remux status \
  --run-directory "/absolute/accepted remux run"
```

Path flags resolve against the current working directory; embedded executable
pins retain their canonical exact bindings. Planning hashes and resolves without
launching tools or reserving a workspace. Run requires a separate canonical
existing parent and creates an ordinary isolated v3 run. Resume requires the
same exact source, policy, selected streams and tool/schema pins; changed
bindings require a new run. The SDK's finite execution defaults are retained.
Actual operator invocation of run/resume would launch the selected native
providers and independent validators; none were invoked for this checkpoint.

Run/resume share ordinary v3 cancellation, progress, writer/checkpoint authority
and machine failure recovery. New-run recovery begins at its Started event;
resume retains the explicitly named run even if admission refuses before a new
event. Partial candidates never become accepted from their presence alone.
Private `status-v3` remains available for incomplete runtime inspection.

Plans, runtime outcomes and failure recovery contain private bindings/locators
and belong in the operator's private work area. Remux `status` instead calls
the existing accepted-report owner and returns only `MetadataPublicSummary`:
policy/status, counts, inspected/unsupported scope, source credential treatment
and signal-mark treatment. It has no private-report or execution mode, and
refuses incomplete, stale or different-profile runs. It neither probes nor
remuxes media. The public summary excludes private tag names/values, source
locators, hashes and private report paths. Do not package `private.json` as the
summary or publish complete plan/outcome JSON. The public-safe summary contract
does not turn a local CLI stream into a publication action.

The first native lane retains all its existing restrictions: Unix, pinned
FFmpeg/ffprobe 8.0.1, bounded nonfragmented MP4/MOV, H.264 with optional AAC-LC,
supported zero-origin CFR and SDR playback fields. Complete packet/configuration/
timing preservation, bounded MP4 mdta agreement, decode and current runtime
acceptance remain separate observations. `clean_within_scope` preserves its
opaque/signed/signal exclusions; it is not universal metadata removal, native
qualification or credential validity. Aniflow's final-video projection can read
an accepted remux independently; no recipe, original delivery or Flow handoff
is automatically rewritten.

Commands use v1 envelopes named `video_metadata_remux_plan`,
`video_metadata_remux_run`, `video_metadata_remux_resume` and
`video_metadata_remux_status`. Two parser regressions plus empty-workspace summary
refusal and early resume recovery are authored, uncompiled/unrun. Tests/builds/dependency resolution,
lint/format/schema/docs/native/provider/media/CLI/SDK/CI execution stays deferred
under #64. The alignment cancellation/start-marker failure remains unresolved,
cause unproven. #112 remains open for broader scopes and format/platform/native
qualification. No actual sanitation, private-media trial, release or publishing
occurred.
