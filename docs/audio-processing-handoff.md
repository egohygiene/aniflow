# Structured audio-processing handoff

Relates to #212. This source boundary composes existing owners without a second
runner. It is not an execution, release or reference-meter qualification claim.

`delivery audio-processing plan --request selected.json` accepts one existing
closed request: fixed gain, resumable final-lossless QC, resumable final-video
audio QC or gain-video. No wrapper silently supplies settings. The versioned
plan pins the full request, exact selected source files and native owner intent.

`run --request selected.json --output-directory fresh-parent` delegates to the
selected owner. Gain/video owners choose their ordinary native child runs; QC
uses a fresh `final-lossless-qc` or `final-video-audio-qc` child. `status` and
explicit `resume` require both `--request` and the exact `--run-directory`.
Partial native work is retained; incomplete owners cannot emit a handoff.
Ordinary native commands retain their detailed partial-phase status views.

The handoff contains current accepted artifact IDs, purposes, full byte pins,
native intent, owner report and canonical plan/handoff identities. Gain WAVs
require fresh exact-file QC; gain-video copies require fresh encoded-file QC.
QC outputs are supporting reports, not new media or a selected-policy pass.
Policy matching, qualified meters, musical/visual review, platform admission and
actual publication remain separate. Clean originals are never replaced.

Two focused parser/refusal cases are authored, uncompiled/unrun. All compiler,
tests/builds/lint/schema/docs/native/media/provider/CLI/CI execution remains
deferred. The known alignment cancellation/start-marker failure is unresolved,
cause unproven. Flow #88 remains last and unchanged; no ADR is ratified.
