# Reviewed manual lyrics in recipe delivery

The [manual lyrics CLI](manual-lyrics-cli.md) exposes this request/attachment and
the separate initialize/import/status/review/export workflow. Decisions remain
explicit supplied JSON for each exact scope.

Source checkpoint for #108. The SDK's fixed recipe now offers an explicit
`music_video_recipe::recipe_lyrics` attachment operation. Lyrics remain optional;
the native video is complete independently. Enable the recipe's original-audio
observation to establish an exact complete PCM binding first. No lyric detection,
alignment model, download, muxing, burn-in or video rewrite is introduced.

1. Call `recipe_lyrics::request(recipe_root, RecipeLyricsOptions { ... })` with
   an explicitly supplied interval format (SRT first, or WebVTT/TTML), language,
   overlap/conversion policy and desired sidecars.
2. Pass that request to the existing `lyrics::initialize/import` workflow in a
   separate private directory, supplying the manually authored SRT. Review its
   exact words and timing separately using the existing manual checkpoints.
3. Once final assembly is accepted, call `recipe_lyrics::attach(recipe_root,
   &request, workflow_root)`. It exports only the selected existing codecs to
   the fixed `lyrics-delivery` slot and commits a hash-bound attachment last.

The request derives the accepted complete PCM subject, original timeline digest,
recipe intent, original audio start offset and the union of original picture/song
extents. It does not guess offsets or clip cues. The initial millisecond-based
manual contract explicitly refuses nonrepresentable offsets or unsupported
negative delivery origins. User edits, wrong PCM, changed offsets, lost review
or mismatched recipe/assembly identity invalidate attachment. Loss-bearing codec
exports still require the existing explicit conversion policy.

Both recipe status/outcome forms expose optional `lyrics_delivery`; read-only
status rechecks current native assembly/audio, separate workflow reviews,
delivery receipt, exact sidecar hashes and complete file membership. External
review inputs must remain available. Partial export without a final attachment
marker is preserved and blocks automatic adoption/replacement. Attach acquires
the recipe and accepted assembly writers. This is sidecar delivery evidence;
neither word/timing declarations nor source merge approve final subtitle styling,
burn-in appearance, release or publication.

Two sidecar integrity/membership regressions are authored, uncompiled/unrun,
in addition to existing manual review/loss cases. All #64 execution and native
qualification remains deferred; its alignment cancellation/start-marker failure
is unresolved with cause unproven. No tests/builds, validators, native tools or CI ran.
