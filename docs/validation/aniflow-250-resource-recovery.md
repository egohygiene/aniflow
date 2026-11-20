# #250 resource and recovery qualification checkpoint

This checkpoint adds measured admission and stronger recovery controls; it does
not complete native full-video Upscayl qualification. The intended Mac/GPU and
full-video quality gates remain open under
[#250](https://github.com/egohygiene/aniflow/issues/250).

The implementation uses existing source/import, per-frame/chunk providers,
Pipeline v3 acceptance and explicit resume. No second scheduler, automatic retry
loop or alternative checkpoint authority is introduced. See
[resource measurements](../upscayl-resources.md) for the new read-only command.

## Evidence classes

| Class | What it can establish | What it cannot establish |
| --- | --- | --- |
| Synthetic protocol regressions | Direct argv, finite accounting, exact identity/policy reuse, output membership/PNG checks, lock/margin refusal and retained partial attempts | Actual Upscayl inference, real model quality, device suitability or full native media/audio acceptance |
| Host observation | Observed OS/architecture, free space, Linux DRM nodes and installed ICD document identities | Successful device initialization, NCNN inference, measured working set or native Mac compatibility |
| Official binary/model availability | Exact downloaded upstream asset/file identities and help/interface observations when separately recorded | GPU inference or a full-video pass |
| Intended native run | Only the exact hardware/tool/model/settings/media behavior actually observed | Other hardware, drivers, models, future throughput or human quality approval |

The scoped synthetic additions exercise measured source PNG and component
bytes; actual index storage; native4× geometry despite requested2×; finite
partition bounds; an over-budget explanation without launch; a large reserve
that denies launch on the observed small volume; changed geometry/resource/
GPU/tile/TTA/tool/catalog/package policies; changed original runtime bytes;
competing writer refusal; and timeout with a retained unaccepted partial chunk.
The existing zero-exit malformed-output matrix now also includes truncation.
Existing suites cover source/model/binary drift, missing/CRC-invalid/wrong-size/
extra outputs, whole-chunk retries, corrupt accepted members and boundary
cancellation/resume. The two Upscayl fixture suites register small pinned
provider dispatch scripts to avoid repeatedly hashing the large compiled CLI
for hundreds of merely planned stages. Each actual invocation verifies that
compiled binary's expected SHA-256 and then execs its production CLI with
unchanged arguments. Their registry implementation identity is the fixture
script; these are protocol/acceptance regressions, not a native executable
identity or throughput qualification. Native pilot commands must use the
actual binary directly. No production hash/acceptance rule is weakened. Substitute FFmpeg framehash data remain fabricated fixture
observations. A reserve comparison is not a real ENOSPC/write-failure trial.

Focused commands for the composed source are:

```sh
cargo test --test upscayl_chunks
cargo test --test upscayl_recovery
cargo test --bin aniflow cli::recovery::tests
```

Results must be attached to the exact composed source and executable when run;
this authored command list alone is not a passing receipt. The resource JSON
schema and selected formatting are separate checks from model execution.

## Native qualification kit

1. Keep originals, old accepted outputs and historical receipts read-only. Use
   synthetic or explicitly selected disposable media on a separate output
   volume. Record the exact aniflow build, OS/architecture, intended device,
   driver/runtime, upstream package/catalog receipt, executable, models and any
   staged runtime libraries. `-v` is Upscayl verbose mode, not a version probe.
2. Run `scripts/observe-upscayl-resource-environment.py --storage-parent
   /absolute/existing-volume` and retain its JSON. It runs no provider and never
   sets `native_qualified` true. On the November20 Linux session, no `/dev/dri`
   device nodes or Vulkan ICD documents were observed; a Vulkan loader alone
   does not establish usable inference. This session has no native Mac access.
3. Obtain a complete existing source index/PNG workspace through the selected
   source recipe's accepted extraction. Measure a bounded representative
   excerpt with the intended native model. Record real input/output PNG sizes,
   native4× geometry, elapsed native durations, peak host RSS and available
   device-memory observations with their actual measurement scope. Use the
   selected host's native process monitor or `/usr/bin/time -l` on macOS;
   process RSS is still distinct from total GPU/driver memory.
4. Run the resource observer before changing limits. Keep actual measured PNG
   distributions separate from maximum caps. Select a finite output cap and
   adequate working/free-space policy that fit the measured profile; do not
   lower a cap to bypass a denial. Measure retained scratch separately after
   every failed attempt. No lifetime retry allocation is promised.
5. Use the existing `video upscale-frames` command with the exact source,
   directory/index and selected settings, first with `--plan`, then with
   `--output-directory /absolute/disposable-runs`. Run the chosen multi-minute
   fixture at intended geometry and record progress/elapsed observations,
   including conditional estimates and their exclusions. A long tiny-resolution
   fixture measures orchestration at that geometry, not intended full-size GPU
   memory or visual quality.
6. Interrupt an active native attempt in a disposable run; separately observe
   configured timeout and selected process termination. Keep the private
   candidates. Explicit `--resume-run-directory` must reuse only exact accepted
   compatible units and retry an unaccepted chunk as a whole, once per request.
   Verify unchanged source bytes and stopped direct child identities. Record
   which descendant processes could be signalled/waited and any adopted zombie
   identities. Do not infer universal containment, host reaping or a resolved
   historical alignment cause from process exit or marker files.
7. Retain a native failure/control matrix and the separate bounded synthetic
   fault results. Source/model/settings/runtime and chunk/resource policy drift
   must refuse incompatible reuse before mutation. Reconcile any stale writer
   explicitly; do not delete an unknown lock. Real full-volume/write failures
   require their own disposable-volume observations; the margin fixture does
   not stand in for them.
8. Verify accepted complete decode and ordering, unchanged rational source
   frame times and the selected audio-preservation policy through final
   assembly. Review representative tile seams, motion, fine linework and mixed
   historical/new frames. Record an explicit creative/technical review before
   calling the output approved. Resource reports, elapsed time, progress,
   filesystem existence, zero exit and model availability grant none of these.

Keep actual raw logs and exact source/build/device identities with each native
observation. This kit does not authorize private-media upload, keys/signing,
hosted CI, a tag/release or disposal of any original. Earlier immutable
alignment observations and their unproven cause remain unchanged.
