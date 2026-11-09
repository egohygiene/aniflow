# Private cue review worksheet [skip ci]

After manually importing timed SRT, call
`music_video_recipe::lyrics::worksheet::observe` with the same request and
work directory. The result binds the exact input, audio, timeline, offset,
settings and current independent words/timing review state. It contains authored
cue text/labels/speakers and exact original and delivery-relative intervals.
It is private working material, not a portable diagnostic or public manifest.

For each selected cue, supply `CueNote` with its ID and canonical SHA-256 of
the complete original cue, independent words/timing check declarations, and
any correction reason. `assess_notes` reobserves the current worksheet before
summarizing. Missing notes remain unchecked. Unknown, duplicate, stale or
oversized notes refuse. Totals are supplied declarations, never authenticated
review or permission to export, mux or post.

A correction does not mutate snapshots or existing decisions. Edit the SRT
externally, import into a fresh workflow and supply new artifact-bound words
and timing decisions through the existing owner. Rendering review is still
separate. No automatic transcription/alignment/model path is added.

Five regression cases authored, uncompiled/unrun. Execution and CI remain
deferred to flow #88, scheduled last. #108 retains its remaining criteria;
known alignment cancellation/start-marker failure remains unresolved, cause
unproven. No private lyrics or actual content approval were produced.
