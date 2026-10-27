# #36 named allenk image repair checkpoint

## Source and authorization boundary

Base main: `2add9a6b230aaf415288e6c128bcf276dc05681c`.
Branch: `codex/36-named-frame-repair`, published as
[draft PR #128](https://github.com/egohygiene/aniflow/pull/128) against main.
Initial published source: `4ccdca50ed9c11b495fa9e39b82d9206c5d769c3`;
initial tree: `3b59e3f75c241f640b2faf798191cb0ed049016a`. These are historical
publication receipts, not self-pins for later handoff edits. Re-query the draft
and exact published head before continuing. PR #125's generic repair and PR #126's per-frame Upscayl
recovery are merged. Parallel chunk draft #127 remains separate at
`246759871a6b705fbc34615185f9066397d3a8d0`; this branch does not include it.
The maintainer authorized the next #36 named-provider/resume checkpoint.

Tests/compiler/build/check, lint/format, schema/naming/drift/docs validators,
native/provider/CLI/media/model execution, dependency resolution/installation,
packaging/smoke and hosted qualification remain deferred under #64. No Actions
are dispatched or polled. Commit with disabled hooks and `[skip ci]`. Authored
source and synthetic coverage are not compilation, native-support or release
qualification. No real/private media processing is authorized by this handoff.

## Authored boundary

The public `frame_repair::allenk` facade adds a closed named configuration,
request, plan, run and resume. The existing `video repair-frames` command selects
it through `aniflow.allenk-frame-repair-configuration/v1` and accepts explicit
`--resume-run-directory` only for this named profile. Generic repair's public
configuration/wire surface and exact fresh-run-only guard remain unchanged.
An arbitrary generic profile label cannot grant named execution authority.

Both paths share immutable import and the existing repair copy/decode loop.
The named capability uses two ordinary v3 stages: import, then complete selected
repair. Exact source bytes/index/time, normalized inclusive ranges, binary and
decoder identities, visible-overlay profile and threshold govern compatibility.
Every unselected PNG remains byte-identical; selected outputs fully decode and
retain geometry, complete cardinality, names, ordinals, clocks and cuts.

The selected upstream is allenk/GeminiWatermarkTool at
`7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e`, declared version `0.3.2`.
The typed `current_v2`/`legacy_v1` choice emits `--no-legacy`/`--legacy`,
respectively. The integer threshold defaults to 25 percent and maps to native
`0.25`; bounds are 0–100 percent. Fixed argv uses the basic single-image path,
without force, region, snap, fallback-region, size overrides or denoising.
Omit `--denoise` entirely: even `off` would select a different upstream pipeline.
The native version is observed before repair execution and must match the pin.
A skipped single image exits 1 and fails the stage; process exit 2 also fails.
Neither triggers automatic profile fallback, forced edits or output promotion.

The basic path uses four compiled visible-overlay assets and no external model
file. Installed executable identity binds those compiled assets. It does not
prove a binary's source build or capture all dynamic OpenCV/fmt/spdlog libraries,
host settings or native suitability. Reports distinguish embedded-asset binding
from `host_runtime_evidence: unobserved` and `backend_qualification: unqualified`.
No visual-success or complete environment-identity claim follows from decode,
byte changes or a valid report.

## Acceptance and recovery

Generic run and resume freshly recheck named processor and decoder files before
durable transitions/provider execution. The named stage must depend on the
exact same-run importer with matching import configuration, source bindings and
expected frame/evidence outputs. Loose import reports or a different producer
cannot establish predecessor acceptance. The public resume facade rebuilds the
exact plan and compares it with the retained plan; changed inputs, ranges,
profile, threshold or pins require a new run.

The checkpoint unit is the whole stage. A compatible accepted import can reuse
after repair fails. Interrupted repair retries every selected frame once; its
private partial outputs remain failure evidence. A complete compatible repair
can reuse as one unit. No partial per-frame repair checkpoints, automatic retry
loop, new scheduler or provider-owned acceptance database is introduced.
Whole-stage reuse retains observed accepted bytes; it does not promise bitwise
reproduction across different shared-library or host environments.

The existing writer lock, invalidation, process-group cancellation, immutable
publication, final whole-run integrity and failure-retention owners remain
unchanged. Generic arbitrary-argv repair still refuses resume, including forced
reruns. Current named authority does not relax unobserved generic dependencies.

## Evidence and resource limits

The named `aniflow.allenk-frame-repair-report/v1` wraps complete generic repair
observations under `repair`. Its additional fields record pinned upstream
revision, observed processor version, profile, threshold, compiled-asset binding,
disabled denoising and the explicit host/native qualification limits. Historical
processing remains `unknown`; source-to-frame relationship remains
`declared_not_verified`.

Existing import geometry/index/frame limits and the 256 GiB default working-data
admission remain in force. Complete import and repair copies need additional
disk space. This accounting is not a disk reservation or an enforced memory
quota. Existing bounded native capture/timeouts and outer provider limits remain
separate; the named profile adds no resource runtime or dependency installation.

Source-only implementation and independent resume/provenance review completed
without a concrete blocker. Eight synthetic integration cases and one contract
regression are authored and unrun. No passing execution result is claimed.
After the maintainer lifts #64's deferral, qualify the exact published source:

1. Compile and exercise the named configuration, CLI plan/run/resume, schemas
   and typed report contracts while retaining generic fresh-only regression.
2. Observe native version, exact argv, profile selection, skipped/failed images,
   actual decode/geometry and unchanged source/unselected bytes on synthetic data.
3. Inject changed processor/decoder pins, changed settings/source/ranges, corrupt
   accepted artifacts, interrupted repair, concurrent writers and storage failure.
   Confirm whole-stage reuse and refusal of loose importer evidence.
4. Qualify selected native binary/platform/shared-library combinations separately
   from substitute fixture executables. Review visual results independently of
   byte changes and structural validity.

Parent #36 remains open for pinned external `video-frames/1` migration, GWR
integration, clip-only cardinality/splitting/seams, optional detection review and
native qualification. No actual external manifest schema/example is invented.
Next proposed work is #38 selected setup, then #35 frame assembly/audio handling
and a single workflow recipe. The known alignment cancellation/start-marker
failure remains unresolved with cause unproven; historical receipts are retained.

See the [guide](../frame-repair.md),
[original generic handoff](aniflow-36-repair-checkpoint.md),
[upstream source evidence](../toolchain-upstreams.md#selected-allenk-image-profile-for-36)
and [retained roadmap](https://github.com/egohygiene/aniflow/issues/117).
