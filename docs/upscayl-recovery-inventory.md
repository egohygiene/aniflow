# Recorded recovery inventory [skip ci]

`upscayl_recovery::inventory::observe(request, run_directory, cancellation)`
rehashes the selected source/components through the existing planning owner,
compares the exact native plan and summarizes its retained manifest. Frame and
chunk policies use their actual planned weights. Required skipped units,
duplicate/missing stages, malformed native records and changed reads refuse.

Complete, pending, active/uncertain, failed, cancelled and invalidated frame
counts are separate. A completed collector is also reported separately. A
running/validating record after interruption does not establish a live process.
No ETA is synthesized from absent historical timing measurements.

This read-only SDK surface does not launch a tool or write run state. Recorded
completion is not fresh output validation or permission to adopt historical
files. Explicit native resume retains those checks and interruption decisions;
configuration/input changes require the existing compatible/new-plan path.
The projection includes no source paths, model paths, raw provider messages or
new completion database. The read-only `video upscale-recovery inventory` CLI
exposes this projection; `progress` adds checkpoint-bound timing observations.
See [recovery progress](recovery-progress.md) for their exact shared arguments.
Both commands observe retained evidence; neither validates arbitrary saved PNGs,
repairs files, launches native work or authorizes output reuse.

The [trial-script reconciliation](source-intake/2026-11-13/228-reconciliation.md)
maps the original resume/finishing prototypes to these current owners. Their
legacy extraction manifest, paths, model defaults and file-replacement behavior
are reference inputs, not another admitted recovery format.

Five regression cases are authored, uncompiled/unrun. Tests/builds/checks/native
and CI execution remain deferred to flow #88, scheduled last. #37 remains open
for historical import, remaining recovery and native/platform qualification.
Known alignment cancellation/start-marker failure remains unresolved; cause
unproven. No media processing, review, release or proposed ADR acceptance.
