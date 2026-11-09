# Reviewed SRT caption candidates

Accepted soft-caption output can now enter an explicit
[subtitle-preserving metadata-only remux](metadata-remux-cli.md), including the
fixed [finishing tail](recipe-finishing.md). All authored subtitle packets,
configuration, timing, words, disposition and language are checked. The output
stays a subtitle-muxed candidate; destination/platform compatibility and full
creative review remain separate. This source checkpoint has not run native media.

Source checkpoint for #108; implementation is uncompiled and unrun. Native font,
codec, timing, visual and player qualification remains in flow #88, scheduled
last. This is not a record of actual content approval or successful rendering.

The public `subtitle_delivery` facade reads one selected accepted final-video
artifact and an existing `music_video_recipe::lyrics::observe_delivery` workflow.
The lyrics reader recomputes current separate words and timing declarations,
all selected exports and the saved receipt. SRT must already have been selected;
no recognition, alignment, new review or automatic format choice is introduced.
Rendering uses the actual millisecond SRT carrier, including recorded conversion
losses, rather than richer original times absent from that carrier. The relation
between the original lyrics PCM and compressed video audio is operator-declared;
this checkpoint does not establish that relationship through signal analysis.

## Explicit sequence

1. Supply a closed `caption-request/v1` JSON: selected video run/artifact, lyrics
   workflow/export directory, explicit `soft_mux` or `burn_in` mode, one pinned
   local TTF/OTF face, bounded style and existing assembly-tool settings.
2. `video captions preview-plan --request REQUEST` reads and binds local sources,
   exact native tool/font/renderer pins, carrier and review bytes without tools.
3. `video captions preview-run --request REQUEST --output-directory PARENT`
   uses an ordinary v3 stage to render actual selected source frames with native
   drawtext. Empty cue indices select first/middle/last cues; explicit indices
   select at most eight cues. Each chosen cue contributes its middle visible
   frame. This is a selected-frame contact sheet directory, never a full review.
4. `video captions preview-status --run-directory RUN` returns current accepted
   PNGs, their exact global frame/time mapping and review context/subject hashes.
   Supply your own `aniflow.manual-review/v1` declaration for those hashes after
   viewing the images. No command authors an approval or an authenticated signer.
5. `video captions delivery-plan` and `delivery-run` require `--request`,
   `--preview-run-directory` and `--declaration`; run additionally requires
   `--output-directory`. Rejection, stale sources/carrier/font/style/settings,
   renderer changes or changed preview bytes refuse generation.
6. `delivery-status --run-directory RUN` and `video completion RUN` project only
   current ordinary v3 native checkpoint/delivery acceptance. Review the full
   candidate separately before any packaging or publishing decision.

The two `*-resume` commands require the same request and `--run-directory`;
delivery resume also requires the same preview/declaration. Existing progress,
recovery, cancellation and v3 stage reuse apply, without a separate journal.
Inspect incomplete work with `status-v3`. Failed private work is preserved;
status never repairs or launches native tools.

## Exact scope

Initial inputs are uncaptioned zero-origin CFR H.264/yuv420p BT.709 TV-range,
square, unrotated destination copies, optional original AAC-LC, at most 100,000
frames, and 1–64 plain cues of at most 512 characters/four lines. Unsupported
markup, overlaps after CFR mapping, invisible or out-of-picture cues refuse.
Timing uses exact `ceil(start * rate)..ceil(end * rate)` frame intervals, with
no float time scaling, video trim, audio edit, rate change or `-shortest`.

Drawtext receives only fixed private filenames and finite style fields. Cue
text stays in private UTF-8 files with expansion disabled, never inside a shell,
argument or filter expression. Font bytes and renderer implementation are bound;
font glyph coverage, shaping and native typography remain to be qualified.

Soft mux copies encoded video and original AAC, adds `mov_text`, verifies video
packets and original audio configuration/packet/timing digests, and round-trips
selected text and intervals. Player font/style behavior is outside the preview
scope. It emits `subtitle_muxed_candidate`; existing destination profiles refuse
that variant pending an explicit supported subtitle policy. Burn-in deliberately
changes pixels and encodes H.264 while copying original AAC. It emits a separate
destination candidate, without asserting original frame identity. Both require
native exact picture/audio clocks, full decode, fixed geometry/color/SAR and
faststart; these gates describe authored behavior, not an observed run.

Complete operation, cue text, local paths and source/review evidence are private.
The public completion summary omits them. Metadata sanitation remains a separately
selected remux; `metadata: null` makes no sanitation claim. The supplied preview
declaration authorizes candidate generation for that selected-frame context;
full-delivery creative approval stays `not_performed`, publication authority is
`none`, and native/platform qualification is not asserted. Originals are read
without replacement. No download, sibling dependency, model or posting port.
