# Exact-artifact manual input and review checkpoint

November 7, 2026. This branch is stacked on the #35 source-bound signal
checkpoint. #107 remains open; #64 owns all deferred qualification.

`music_video_recipe::manual` provides a closed source/timeline/configuration/
recipe-bound checkpoint for prepared frames and manually authored timed lyrics.
It has explicit `awaiting_input`, `awaiting_review`, `approved`, `rejected` and
`invalidated` states. Creation supplies no input or approval. One exact manual
file can be attached as a private bounded snapshot. Its original bytes and the
snapshot must remain identical; later editing invalidates an earlier decision.
Source, clock, recipe or configuration changes also invalidate the checkpoint.
Timed-text parsing and separate wording/timing/render review remain #108 work.

Approval/rejection is an explicitly supplied `aniflow.manual-review/v1`
declaration naming both current context and artifact SHA-256, a reviewer and
optional bounded notes. There is no approval default. Decisions are append-once
records for that exact checkpoint, never overwritten by a second decision.
Changed/rejected content requires a new checkpoint or recipe, preserving the
old declaration. Native validation, legal/Identity approval, provenance signing
and publishing are not granted by this local operator declaration.

The fixed source and indexed recipes accept `review_prepared_frames: true`.
After existing v3 preparation/optional upscale acceptance, they bind the exact
frame tree, ordered hashes, index/clock, original source, current configuration
and recipe intent into `review/`. Assembly remains `awaiting_review` until the
matching declaration approves it. Planning/status never create that checkpoint
or launch tools. Pending-review resume rechecks existing accepted evidence
without fresh source probing, import, repair, upscale or assembly. Once approved,
ordinary source and child compatibility/resource checks still govern execution.

The opt-in flag defaults to false and is omitted from canonical serialization
when false, preserving old intent digests. Existing completion-only outcome
contracts remain unchanged: a paused run returns a state error containing
`awaiting_review` and the existing recovery locator; status names the waiting
phase and checkpoint. No final output is reported. After a user decision, resume
uses the same root and current accepted child runs. An assembly child present
without a currently valid required review is a divergent-state refusal, not an
artifact to adopt. Known child/writer/partial state is preserved.

The public SDK and [explicit review example](../../examples/manual_review.rs)
consume the operator's own decision document. The library never chooses its
decision/reviewer. The checkpoint directory admits only its bounded context,
subject, optional private file, review and ordinary exclusive writer record.
Writes are private no-clobber synced records. It does not add another DAG,
scheduler, arbitrary command hook, media acceptance owner or cross-tool engine.
No user media or actual review declaration was produced in this source sprint.

Four focused regressions are authored, uncompiled/unrun: manual state/input
mutation, wrong subject/changed timeline refusal, retained rejection/no implicit
second decision, and default-flag serialization/intent compatibility. No tests,
compiler/build/Cargo, lint/format/schema/docs, native/provider/media/model/CLI
execution, installation or CI ran. No dependencies, lockfiles, old fixtures,
proposed ADR status, releases or issue acceptance boxes changed. #107 retains
broader optional finishing/manual recipe modes and native/platform qualification.
The known #64 alignment cancellation/start-marker failure remains unresolved.
