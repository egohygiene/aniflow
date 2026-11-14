# HLS acquisition prototype (#230)

Disposition: keep acquisition as explicit external source preparation. The
[archived `hls-download`](README.md) is a useful design reference, not a new
Aniflow command, provider or installation prerequisite. This resolves the small
[intake decision](https://github.com/egohygiene/aniflow/issues/230) without
adding a downloader framework or weakening the current local-input contract.

The existing [`SourceMusicVideoRequest`](../../src/music_video_recipe/source_types.rs)
selects a local `source_video`. [`plan_source`](../../src/music_video_recipe/source.rs)
composes the existing [extraction](../video-frame-extraction.md) and
[indexed recipe](../music-video-recipe.md) owners. No HLS acquisition adapter is
present at the inspected baseline. A URL, playlist, download log or partial
recording cannot stand in for an accepted local temporal source.

## Useful mechanisms to retain

| Observed trial behavior | Disposition and integration boundary |
| --- | --- |
| HTTP(S) or local playlists; MKV default with explicit MP4/TS choices; optional start/duration, stream selectors, subtitles and audio-only admission. | Retain as external acquisition requirements. A local playlist can reference remote segments; “local path” must not be treated as a network-free capture. Selection/transcode/trim changes need provenance separate from later Aniflow processing. |
| Stream copy by default, explicit H.264/AAC transcode, faststart for MP4 and temporary output beside final output. | Preserve the distinction between copied and re-encoded media. Stream copy is not an end-to-end lossless/source-equivalence guarantee. Aniflow independently observes its selected completed source and refuses unsupported clocks/streams. |
| Segment retries use HLS `seg_max_retry` when supported. Optional protocol reconnection probes available HTTP options and bounds delays, retries and total delay. | Preserve the demuxer-vs-HTTP retry distinction: retrying ordinary finite-response EOF can loop. Capability probing and all effective limits must be recorded if this becomes an adapter; unsupported controls cannot silently imply an enforced limit. |
| Explicit headers/header files, referer, user agent and cookies; newline rejection for headers; query redaction in selected messages/dry-run. | Keep authentication external. Never store supplied secrets, full signed URLs or raw tool output as public evidence. The current script's redaction is partial, not a reusable privacy guarantee. |
| Optional curl preflight recognizes playlist markers, master/live/VOD hints and selected encrypted/DRM key markers. | Retain only as diagnostic hints. Missing curl, failed fetch or oversized response can skip checks; a successful preflight is not source completeness, codec admission, segment availability or authorization evidence. |
| Directory lock/PID, stale-lock removal, `.part` output, optional retention and final FFprobe stream observation precede `mv`. Live interruption can finalize a nonempty probed recording. | Preserve lock/partial/evidence lessons; do not copy this as current checkpoint authority. A stream observation is not complete decode/timing/integrity validation; a finalized interrupted recording is explicitly partial. Current v3 acceptance remains independent. |

## Limits that matter before reuse

`--dry-run` is not an inert plan: the script resolves tools, creates output
directories and a lock, and invokes FFmpeg capability help before printing the
plan. `--list-streams` probes media/network. Neither is used during this intake.

The dry-run masks header/cookie arguments and input query text, but not every
sensitive field: URL user information, fragments, referer/user-agent values and
raw FFmpeg diagnostics need separate handling. Secrets also reach external
process arguments. A future adapter must supply a narrowly scoped credential
boundary and sanitized errors rather than treating this printed plan as safe
for an issue or workflow artifact.

Its cooperative lock does not protect against unrelated writers or arbitrary
path replacement, and final `mv` can replace a destination that appeared after
the initial existence check. Failure cleanup deletes partial media unless
retention was explicitly requested. These behaviors are not adopted as
Aniflow's immutable-source, no-clobber or retained-failure contract. Explicit
overwrite in a standalone trial does not authorize replacement of an original
source or accepted run artifact.

The output check establishes a nonempty file with a stream, not complete
segment coverage, exact rational duration/PTS, audio layout, full decode or
successful cancellation semantics. A live capture has no inherent complete
duration; keep the requested boundary, observed end and stop reason distinct.

## How it fits today and a bounded future adapter

Today an operator separately acquires an authorized source, keeps acquisition
credentials/private diagnostics outside the run, and explicitly selects the
completed local media for normal source-entry inspection. Aniflow does not
fetch, retry or authenticate the playlist on that operator's behalf. This
intake does not recommend executing the unqualified archived script unchanged.

If native acquisition is later selected under
[#212](https://github.com/egohygiene/aniflow/issues/212), scope one optional
external adapter to a finite VOD capture first:

1. Bind source/profile identity without exposing signed URLs; select exact
   streams, copy/transcode mode, requested interval and finite retry/deadline
   policy. Declare network/authentication access explicitly.
2. Use a new isolated destination, no-clobber publication, descendant-aware
   cancellation and retained partial/error evidence. Do not add another generic
   process runtime or treat an interrupted file as a complete original.
3. Record tool versions, effective supported retry options, final bytes/hash,
   observed stream clocks, completeness and failures. Hand only completed local
   media to existing extraction/temporal admission, which remains authoritative.
4. Start with synthetic local playlists and injected finite-response EOF,
   missing segment, auth/rate-limit, unsupported-option, timeout/interruption,
   output-race and corrupt-output cases. Keep real network/native qualification
   separate; no credentials or source URLs belong in committed fixtures.

No download, probe, archived script, subprocess capability check or media
operation was run here. The recorded evidence is source/hash/link inspection.
