# Manual timed-lyrics workflow checkpoint

November 7, 2026. Stacked on #107's manual review checkpoint. #108 remains open.
The maintainer selected manual SRT as the current lyrics workflow. Existing
recognition/alignment capabilities remain separate; no new recognition issue,
automatic-detection stub, model download or generated lyric proposal is added.

`music_video_recipe::lyrics` exposes initialize/import/status/export. Initialize
without an input while writing an SRT externally, or supply an explicit SRT,
WebVTT or supported TTML with complete interval timing. Plain text and point-only
LRC are not imported by inventing end times. The existing #47 parser owns UTF-8,
cue syntax/order/duration/overlap/language and loss-aware codecs. Authored text,
repeats and intentional omissions remain untouched. Syntax is not word accuracy.

The closed `aniflow.manual-lyrics-request/v1` names an exact original PCM audio
file and AudioSource, external timeline/recipe identity, signed exact-millisecond
offset, nonnegative delivery extent, language, overlap policy, explicit targets
and explicit permitted conversion losses. Audio bytes and native WAV facts are
rechecked without a provider process. The external timeline/recipe hashes are
caller bindings, not an independently observed compressed-video relationship;
use #35's source-bound PCM receipt when composing with video. This is a manually
selected local workflow, not a claim of automatic recipe/destination integration.

Import snapshots the raw manual carrier separately for words and timing, with
different #107 context fingerprints. It saves the unreviewed audio-relative
normalized document and performs a candidate delivery-coordinate projection.
Cues outside original audio or selected delivery extent, unsupported fractional
millisecond offsets, incomplete timing and unpermitted overlaps are refused.
The signed offset shifts presentation only; no audio edits or implicit trimming
occur. Source text, exact audio, timing/offset/configuration changes cannot reuse
old review. Reject or edit by starting a new workflow, preserving old receipts.

Words and timing each need their own explicit operator-supplied declaration.
One decision cannot substitute for the other. Reviewers check sung words,
repeats/omissions, synchronization, instrumental gaps and readability themselves.
No declaration is synthesized for user content. Approved words become an
explicitly supplied #47 reviewed-lyrics provenance record with its retained
review digest; that provenance remains a declaration, not authenticated identity.

Export prepares only explicitly selected existing formats (SRT/WebVTT/TTML,
normalized JSON, or loss-permitted LRC/plain text). Every codec loss still needs
the request's explicit permission. Existing Renderflow publication conversion
can consume the SRT; no sibling dependency or duplicate format engine is added.
Shifted delivery coordinates do not pretend to be audio v1 zero-origin samples:
their exact audio/timeline/offset relationship stays in the versioned delivery
receipt alongside output hashes, losses and carrier omissions. The original
audio-relative document and private review evidence remain in the work area.

Output is a fresh private no-overwrite directory. After sources/reviews/current
request are rechecked, payloads are synced and `lyrics-delivery.json` is last.
Changed/missing review, normalized-document corruption or source drift refuses
publication. Interrupted writes may leave unaccepted partial files. Sidecar
export does not approve rendering: mux/burn-in/visual preview and final sealing
remain explicitly unperformed and owned by existing #108/#33/#82 work.

The [manual workflow example](../../examples/manual_lyrics.rs) and
[#107 review example](../../examples/manual_review.rs) are authored entry points.
Four synthetic cases cover separate review/Unicode text/exact offset export,
stale audio/offset/manual edits, out-of-range timing, and cross-scope approval/
unpermitted end-time loss. All are uncompiled/unrun. No tests/compiler/build/
Cargo, lint/format/schema/docs/native/provider/media/model/CLI/install/CI
execution occurred. No private audio/lyrics, real approval, passing qualification,
new issue, release or publishing occurred. Broader recipe/preview/mux/burn-in and
platform qualification remain in #108/#64. Known alignment cancellation failure
is retained unresolved.
