# Continue an accepted video toward a manual publishing package

`music_video_recipe::publishing` connects the existing local owners in a fixed
order. Start after native assembly or [caption/metadata finishing](recipe-finishing.md).
The original master stays separate. No cross-repository adapter, generic graph,
automatic lyric alignment or new media acceptance database is introduced.

1. Optionally create one explicitly configured destination video with the
   existing [native export](destination-video.md).
2. Optionally measure audio from those exact final bytes through
   [final-file QC](final-video-audio-qc.md). This is required when the selected
   profile requests loudness evidence. No gain or normalization occurs.
3. Save [preflight](destination-preflight.md) against the frozen supplied dated
   requirements. Missing/unavailable evidence or format changes block the tail.
4. Pause for a supplied decision after reviewing the whole exact final file,
   public release metadata and declared scope. Preview review is insufficient.
5. On a separate resume after approval, create the existing exact copy-only
   [manual posting package](posting-package.md).
6. If selected, retain a read-only request/plan handoff for existing exact-byte
   [release evidence](release-evidence.md). This step does not seal, sign,
   timestamp, upload or post anything.

The initial lane is a plain native H.264 destination copy, or an explicitly
selected destination export from an accepted master. Burned captions are plain
video here. Soft-subtitle candidates still need a compatible future destination
lane and are refused; preservation through metadata does not establish TikTok
compatibility. Selected QC also refuses missing/unsupported audio explicitly.

Supply `aniflow.publishing-request/v1` with `video_run_directory`, `artifact_id`,
`provider_executable`, `profile`, `evaluated_on` and `package`. Optional `export`
is the existing destination-video configuration and must use exactly the same
profile/date as preflight. Optional `audio_qc` contains existing assembly/tool
`configuration` plus finite `maximum_pcm_bytes`. Optional `retention` contains
finite `maximum_bundle_bytes` and `minimum_free_bytes`. Package fields are
existing explicit `public_metadata`, `metadata_scope_policy`,
`maximum_package_bytes` and `minimum_free_bytes`. Null/absent optional operations
are skipped explicitly; a requested loudness check cannot skip QC. No tools,
fonts or models are installed by this recipe.

```bash
aniflow --output json delivery publishing plan --request publishing.json
aniflow --output json delivery publishing run --request publishing.json --run-directory /private/publishing
aniflow --output json delivery publishing status --run-directory /private/publishing
aniflow --output json delivery publishing next-action --run-directory /private/publishing
```

Plan freezes the current original native handoff, request, selected executable
identities and optional first export plan. Later file hashes, QC and review
bindings remain pending until their actual accepted predecessors exist. Run and
resume advance the selected fixed owners until a review pause, rejection,
preflight block or local completion. Status/next-action only recheck current
files/owners; they execute no probes or providers and create no decisions.
The evaluation date is frozen, not refreshed to current live platform policy.

At `record_review`, retain the supplied `aniflow.manual-review/v1` declaration
using the exact `review_context_sha256` and `review_subject_sha256` from current
status. The context binds file, public metadata, profile, QC and declared scope.
`approve` and `reject` are both retained decisions; they are unverified operator
declarations, not authenticated identity or independent creative approval.

```bash
aniflow --output json delivery publishing record-review --run-directory /private/publishing --declaration my-decision.json
aniflow --output json delivery publishing resume --run-directory /private/publishing
```

Recording a decision does not run packaging. Rejection blocks it. A decision
cannot be overwritten or reused after context/file changes; preserve the old
root and start a new intent. All byte-changing operations precede QC and final
review. Earlier source PCM or QC before a metadata/mux change cannot substitute.

The private root has `publishing-intent.json`, optional native
`children/destination_export`, optional `audio-qc` with last
`audio-qc-binding.json`, then `preflight.json`, `posting-review.json`, `package`
and optional `retention-handoff.json`. Existing owners retain actual native
acceptance, QC, package membership and exact-byte evidence. Parent/current child
writers protect mutations, and every readback recomputes current authority.
Only exact known native export work resumes. Partial QC/package/marker
initialization is preserved for explicit reconciliation or a new root; it is
never adopted or overwritten as success. Unknown/unselected descendants refuse.

After completion, the operator can separately post the selected files from
`package/public`. Keep `package/private`, QC, native receipts and the retention
handoff private. The handoff's `request` may be supplied to the existing explicit
`delivery evidence-seal` operation with a separate fresh root. Any subsequent
proof/signature/timestamp operation remains separate. No source disposal,
publication, platform acceptance or ownership proof is inferred here.

Eight SDK request/order/review/advice/refusal cases and two CLI parser cases are
authored, uncompiled/unrun. All tests/build/compiler/dependency resolution/lint/
format/schema/docs/corpus/CLI/SDK/native/provider/media/model/browser/container
and hosted CI execution remain deferred to
[flow #88](https://github.com/egohygiene/flow/issues/88), scheduled last with its
original 45-ticket ledger. The alignment cancellation/start-marker failure
remains unresolved; cause unproven. No private Akashic files or actual review
were used. ADR-0012/0013/0017 remain proposed. This draft supplies source for
#107/#35/#83, not release or operational qualification.
