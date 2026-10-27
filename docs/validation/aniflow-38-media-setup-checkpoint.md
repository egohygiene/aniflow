# #38 selected media setup checkpoint

## Source and authorization boundary

Branch: `codex/38-selected-media-setup`, published as
[draft PR #129](https://github.com/egohygiene/aniflow/pull/129), targeting the
`codex/36-named-frame-repair` branch from named repair draft
[PR #128](https://github.com/egohygiene/aniflow/pull/128) at
`221af01edcbe68de56431ec0f39f76b08d6fc7a2`. Main remains
`2add9a6b230aaf415288e6c128bcf276dc05681c`. Parallel chunk draft
[PR #127](https://github.com/egohygiene/aniflow/pull/127) at
`246759871a6b705fbc34615185f9066397d3a8d0` is not included. Initial published
source: `985772ba5b82beb9d82338670dba5b6a8dda094f`; initial tree:
`2d07cba40f4336f8d492011e93568ab1d4a555bd`. These are historical first-save
receipts, not self-pins for later handoff edits. Re-query the branch and draft
before continuing. No merge occurred in this checkpoint.

The maintainer authorized #38's selected repair/Upscayl setup work. All tests,
compiler/build checks, dependency resolution/installation, lint/format,
schema/naming/drift/docs validators, native/provider/CLI/media/model execution,
smoke/package and hosted qualification remain deferred under #64. No Actions
are dispatched or polled. Commit with disabled hooks and `[skip ci]`.

## Authored boundary

The public `toolchain::prepare_media_setup` operation and thin
`toolchain prepare-media --configuration "request.json"` delivery map one
explicit capability and dependency selection to an existing typed configuration.
The selected capability's `effective_settings` is the sole options source;
closed option decoding refuses unknown settings rather than ignoring them.
The request maps exact dependency IDs for either named allenk image repair or
Upscayl recovery. It does not guess an adapter from a broad capability name.

The existing `inspect_profile` owns bounded selected-file hashing and attributed
inventory observations. Typed mapping adds exact dependency-set/kind, effect,
flag/feature, version/package, backend and native/requested-scale requirements.
A ready result returns one inert `configuration.json` with canonical digest;
missing/incompatible/unverified observations retain diagnostics and no files.
The operation reads no source media, launches no tool, writes no configuration,
registers no provider, and performs no install/download/model discovery.

The existing frame facades still bind source and index, create exact provider
registrations and plans, reobserve component identity and own processing and
resume. A retained preparation result is never execution or checkpoint
acceptance authority. Audio registration/preflight and generic toolchain
inspection contracts remain unchanged. No new scheduler, runtime, global cache
or setup persistence format is introduced beyond the closed request/result.

The allenk mapping uses the pinned 0.3.2 basic-image interface with explicit
current/legacy profile and threshold. It omits denoising, force and automatic
profile fallback. Upscayl retains separate 4x model-native and 2x/3x/4x requested
scale, separate parameter/weights pins, fixed worker policy and existing bounded
settings. The source-derived model-name inference guard includes 8x/16x tokens,
preserving upstream precedence and refusing a name that would override native4.

## Evidence and limits

Preparation reports `native_qualification: false` and
`host_runtime_evidence: unobserved`. Local byte identity and attributed package,
flag/feature, model and backend observations are distinct facts. The Upscayl CLI
has no version option; no successful native version probe is invented.
Compiled allenk assets are bound through the executable under its selected
interface, while dynamic libraries and the host remain incompletely observed.
No report establishes model origin/terms, actual device suitability, visual
repair quality, bounded RSS/VRAM, cross-host determinism or release readiness.

The one-MiB request/eight-MiB result bounds and selected two-GiB hashing budget
are retained. Runtime executable admission stays at 128 MiB, with existing
import/frame/model/resource bounds applied by the typed validators. No source
or native execution occurred while authoring the mapping or its examples.
The new candidate profile and Linux/macOS examples do not qualify those targets.

Source review also corrected the allenk caller schema's canonical `$id`, so its
relative frame-import references resolve to the sibling schema. The wire schema
identifier and runtime provider-schema digest remain unchanged; no schema
validator was run.

Root and independent source review completed with no remaining concrete blocker.
Twelve new integration cases and one CLI parser regression are authored and
unrun; the existing stable machine-command-name case is extended. Pure shared
policy validation rejects unsupported options before file inspection, including
the source-derived 8x/16x model-name override cases. No passing execution result
is claimed. After the maintainer lifts #64's
execution deferral, qualify the exact published source with focused request,
CLI, schema and mapping cases; missing/version/flags/model/backend diagnostics;
path/quoting and no-write/no-process cases; and actual native host/model behavior.
Keep that native evidence separate from substitute fixture bytes.

## Continuation and preservation

Parent #38 remains open for other typed mappings, complete runtime/environment
identity, optional tool/device probes, model-locator discovery and
native/platform qualification. No actual signed/immutable release is created;
#10 retains release ownership. Core inspection remains independent of optional
repair/upscale tools.

Next proposed source work is #35's verified frame-to-video/audio assembly and a
single music-video recipe after reconciling the open repair/setup/chunk drafts.
#36 retains manifest migration, other provider/clip limits and seams, optional
detection and native qualification. #37 retains historical-output adoption and
the full failure/native matrix. #111 can proceed independently.

The known alignment cancellation/start-marker failure remains unresolved with
cause unproven. Preserve historical handoffs and receipts; they cannot qualify
current source. See the [setup guide](../toolchain-media-setup.md),
[upstream evidence](../toolchain-upstreams.md),
[roadmap](../../ROADMAP.md#current-source-and-work-order), and
[retained work ledger](https://github.com/egohygiene/aniflow/issues/117).
