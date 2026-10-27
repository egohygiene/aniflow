# #36 selective frame-repair checkpoint

## Source and execution boundary

Base: main `a717935b49fccf87151124a6e20efeb4281ff11c`, after merging the
documentation refresh in PR123 and immutable import in PR124.
Branch: `codex/36-selective-frame-repair`. Re-query the current branch/PR for its
published source head; this document does not predict its own commit or claim
publication before it occurs. The earlier
[import checkpoint](aniflow-36-import-checkpoint.md) remains a historical record.

The maintainer authorized continued implementation and frequent draft saves.
Tests, compiler/build/check, lint/format, schema/naming/drift checks, native
provider/CLI/media/model execution, dependency installation/resolution and
hosted checks remain deferred under #64. No Actions are dispatched or polled.
Use disabled Git hooks and `[skip ci]` when saving this source checkpoint.
Source review is not a passing execution result or native qualification.
This slice does not close #36 or parent #35.

## Authored behavior

`frame_repair` adds a fixed two-stage Pipeline v3 facade: fresh immutable
`import_frames`, then `repair_frames`. It uses the existing provider registry,
native ABI, plan, artifact-integrity validations, workspace, process cancellation
and checkpoint owners. The repair stage consumes the actual same-run import
outputs through declared dependencies; it does not adopt a standalone report
as accepted historical state. The original source index remains a declared
input, and no second frame manifest is introduced.

The caller supplies manual one-based inclusive ranges and an explicit
direct-image profile, executable pin and literal argv. Exact `{input}` and
`{output}` whole-argument placeholders occur once each. Settings and sorted
manual ranges are materialized into a closed effective provider operation.
Reordering the same caller range list preserves its operation configuration
identity. No shell expansion, provider alias, guessed GWR
interface, model discovery or hidden backend fallback is introduced.

Each selected frame is privately snapshotted, processed into a separate
per-frame output and admitted only after exact membership, PNG structure and
full RGB24 decode checks. Every unselected PNG is copied and observed for byte
equality. The complete output preserves cardinality, filename/ordinal mapping,
cuts, source identity, geometry and exact rational timing; only selected PNG
digests may change. A 299-to-240 result must fail. Selected but byte-identical
results are allowed and distinguished from observed byte changes.

Repair shares the crate-private frame I/O and importer decoder helpers. Nested
processor and decoder children use `run_tool_isolated` with `GroupPolicy::Inherit`
under the outer native provider's process group. Reusing a nested
`ResolvedProvider::execute` would start another group and break that cancellation
ownership, so it is not the execution surface for these children.

The new `video repair-frames` command delegates to the library. `--plan` returns
the existing Pipeline v3 plan after byte/pin/selection/resource admission without
launching tools or creating a run; execution returns the existing v3 run-outcome.
Machine command spelling is `video_repair_frames`. The new report is
`aniflow.frame-repair-report/v1`; caller settings and the private provider
operation have separate closed schemas. No custom repair checkpoint or resume
wrapper is added. Each command execution starts a fresh run. Generic
`resume_v3` refuses any plan containing exact capability `aniflow/frame-repair`
version `1.0.0` while external model/environment identity remains unobserved.
Both checkpoint reuse and forced rerun are refused before progress, a durable
resume transition or provider launch. Read-only `status_v3` remains available.

## Identity, limits and claims

`processor_profile` and `processor_declared_version` remain caller declarations.
The executable digest and exact argv/settings are bound, but an executable pin
does not establish arbitrary external models, interpreters or environment
contents. `model_environment_evidence` stays `unobserved`. The first profile is
for an explicitly trusted self-contained direct-image tool/wrapper, is
environment-dependent and is not cacheable. It neither certifies named visible-
logo implementations nor treats different upstream projects as aliases.
The executable must run from its private copy; adjacent dependencies are not
relocated. Explicit absolute external paths do not gain dependency identity
evidence from the executable pin.
The first manifest declares GPU and network use forbidden; these are caller-
trusted profile requirements, not OS-enforced isolation.

`source_relationship: declared_not_verified` and
`historical_processing: unknown` remain unchanged from import. Each report
observation separates selection, byte change and decoded identity. Neither a
successful process nor a changed digest proves visual repair quality. Report
deserialization/schema validity does not grant accepted-stage authority.

Finite argument, file, pixel, decoder/processor capture and timeout bounds remain
explicit. The additional `maximum_working_bytes` budget accounts conservatively
for source/input, candidate and accepted import/repair copies, scratch frames,
tool copies and index/report bounds. It defaults to 256 GiB, with an allowed
range of 1 MiB–16 TiB. Independent outer runtime artifact limits still apply.
This is admission accounting, not a disk reservation, OS resource quota or
sandbox; no hostile filesystem/process confinement is claimed.

## Deferred execution and continuation

Review the final source and authored regressions against the exact published
head before resuming execution. No result from the following work is claimed:

1. Compile and run focused synthetic library/provider/CLI/machine coverage and
   closed configuration, operation/report schema coverage, including the
   authored, unrun `tests/frame_repair_resume.rs` regressions for refusal before
   resume mutation/progress/provider launch and retained read-only status.
   Keep substitute process fixtures distinct from actual FFmpeg and processor
   evidence.
2. Exercise exact range boundaries, deterministic ordering, overlap refusal, whole argv
   placeholders, control/UTF-8 byte bounds, materialized configuration identity,
   tool pins, read-only planning and aggregate resource admission.
3. Verify changed and unchanged selected results, every unselected byte,
   full output count/index/times/cuts, malformed PNGs, wrong geometry, partial
   decode, extra files, changed inputs, symlink refusal, output ownership,
   process failure and cancellation with retained evidence.
4. Qualify actual pinned direct-image wrappers and FFmpeg on explicit native
   hosts. External model/runtime identity requires its own bounded explicit
   dependency profile before claiming model reproducibility or named-provider
   qualification. No such evidence exists in this source checkpoint.
5. Continue #36 clip-only splitting/reconstruction only against documented
   provider limits and checked seams. Obtain the actual pinned `video-frames/1`
   schema/example before implementing migration. Automatic detection, video
   assembly/remux and visual approval remain separate capabilities.

The known alignment cancellation/start-marker failure remains unresolved and
its cause unproven; historical receipts remain unchanged. The
[user guide](../frame-repair.md), [pipeline](../../providers/frame-repair/pipeline.yml)
and [contract registry](../contracts/README.md) describe this same authored
boundary. No native media, release or cross-holon consumer qualification is
implied by merging this slice.
