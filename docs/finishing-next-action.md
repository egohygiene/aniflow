# Finishing next action [skip ci]

`aniflow video finishing next-action --run-directory <DIRECTORY>` and
`music_video_recipe::finishing::next_action::observe` project advice from the
existing fixed finishing owner. The machine envelope command is
`video_finishing_next_action`; its data schema is
`aniflow.finishing-next-action/v1`. Existing status v1 is unchanged.

The typed action distinguishes a planned native start, an existing child whose
native recovery state needs inspection, an exact accepted preview awaiting a
supplied decision, a retained rejection, and a final candidate needing separate
creative review. It rechecks state before returning and has no write/process
port. No retry, approval, fallback, source mutation or posting occurs.

Advice never becomes an execution token. Explicit run/resume/review commands
retain all source/tool/lyrics bindings and native interruption/acceptance guards.
A complete native workflow does not establish full-delivery creative approval.
Rejected previews remain rejected; changing them requires a fresh intent.

Four SDK regression cases and one parser case are authored, uncompiled/unrun.
Tests/builds/checks/native/CLI/SDK/CI execution remain deferred to flow #88,
scheduled last. #107 retains broader modes, composition and qualification.
Known alignment cancellation/start-marker failure remains unresolved, cause
unproven. No release, publication or proposed ADR acceptance is asserted.
