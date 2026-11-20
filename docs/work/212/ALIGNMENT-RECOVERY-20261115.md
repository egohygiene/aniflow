# Selected alignment cancellation and recovery — November 15, 2026

This checkpoint observes the existing runtime and reviewed-lyrics alignment
owners with synthetic tools. It introduces no alignment model, transcription,
automatic lyric recognition or process scheduler, and makes no change to product
process-wide child-subreaper policy.
The original audio and supplied review declaration remain separate from a
candidate alignment; a structurally accepted fake result is not real inference
or human timing approval.

## Historical failure and current baseline

The retained historical `tests/audio_alignment.rs` failure waited for
`alignment-started` and then failed its marker assertion. Its historical cause
remains unproven. The timer includes planning, provider hashing and inspection,
so a missing marker alone does not identify which phase failed or whether a
running native aligner ignored cancellation.

A fresh diagnostic translated that exact current fixture through the retained
41,714,352-byte CLI, SHA-256
`961985e41516acc0827d0d77ac902110c746f4648749ea0dcfba906b9109dcad`, measured
from actual public source `e1ce9b37a4b820dc7e469b5a0176229058bf741d`.
A separate Git comparison to current main
`dff9c31a6be5141ad77d81c558c4467d29bad750` confirms only five observer/Task/docs
paths differ, with no Rust, Cargo or runtime input changes. This is input
equivalence, not a new build or relabeled source identity.

The aligner reached its start marker. Its configured one-second **inner** tool
deadline then produced `ToolTimeout`; the provider exited naturally with code 5,
the outer runtime recorded Failed, and the CLI returned execution exit 7 with a
recovery location after 1.564 seconds. This run does not reproduce the missing
marker, and is neither an outer timeout nor cancellation. It does not claim that
the Rust integration target ran or that the historical cause was repaired.

## Selected observer

`scripts/observe-alignment-recovery.py` prepares a nonzero 16-kHz mono PCM16 WAV,
the existing exact synthetic inspection/aligner ABI and marker model resources.
No trained weights are loaded. It checks four independent paths:

- CLI SIGINT drives existing cancellation after the provider, aligner, child and
  grandchild have all been observed running in the provider's inherited group.
- A one-second inner native timeout returns a failed provider/run.
- The inner bounded stdout capture returns `ToolOutputLimit`, also a failed run.
- A one-second outer provider deadline with a longer inner deadline records the
  distinct TimedOut provider outcome and a failed pipeline run.

Each path must retain the completed inspection checkpoint, publish no alignment
checkpoint or accepted alignment, and preserve all original fixture inputs,
tools, model and settings bytes. Compatible resume uses the same limits, reuses
inspection only and executes alignment once; a second completed resume reuses
both stages without another aligner launch. Exact report/manifest, command-stream
and file identities accompany the observations.

Linux `/proc` start ticks, parent IDs and group IDs bind each live process before
the stop. Afterwards absent, zombie and other states stay distinct. The outer
runtime waits for its direct provider; synthetic processes record actual waits
for children they own. A zombie is not running but is not called reaped. The
observer does not assert universal descendant reaping, escaped-process-group
containment, native Mac/Windows behavior or a kernel resource sandbox.

## Actual source, build and selected observations

The actual clean published-source candidate is
[`f851f9cc0f68291329ebc34acc36b16c07b9f82e`](https://github.com/egohygiene/aniflow/commit/f851f9cc0f68291329ebc34acc36b16c07b9f82e),
tree `5a4570ee90fe8bcd50ef9557aff327da75e7d3f1`. That source adds only this guide
and the initial observer to main; no product Rust, schema or dependency changed.
Later observer and receipt/documentation changes remain distinct from the
compiled source. Their separate hashes are retained in the binding receipt.

The **unchanged exact current Rust cancellation integration test passes**:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`, one
passed and seven filtered out, with 2.87 seconds of test execution. Its scoped
Rust 1.85.0 locked/offline Cargo compile and test took 227.893 seconds using the
existing optimized-dev cache, two jobs, no debug information and no incremental
compilation. A subsequent actual public-source build took 0.075565 seconds with
all emitted artifacts fresh. There is no independent rebuild claim. The source
was clean before and after compilation/capture. This bounded test does not
reproduce the historical marker failure or establish its earlier cause.

| Captured subject | Bytes | SHA-256 |
| --- | ---: | --- |
| New CLI | 41,715,264 | `d776fd120ad0a77b21271c162815546062434f10e4df93e2f9686c664d1a666c` |
| Immutable build-time record | 10,608 | `5128d54de7b5e5d0a69a4269ee9196408c91fb0697bee78e6beb5788bbee988c` |
| Actual CLI release-contract envelope | 13,587 | `b93ed24401364b66ba091a1616e8aacc7a20ccd3a09b58a03782b4aeb476007c` |
| Separately normalized descriptor document | 12,538 | `939cfc732fed34775a4bb1b8618b73009f42dc29e92e83fcc649123f2aaa3027` |

The original build record and native envelope remain unchanged. A separate
projection extracts the actual JSON envelope's `result`, serializes sorted keys
with two-space indentation and a newline, and verifies parsed equality. Its
canonical descriptor equals the earlier descriptor bytes; that does not relabel
the new executable or raw capture. The build record's first extracted unsorted
document has its own `a31b52fd…` physical hash. These serialization identities
are explicitly separate rather than represented as a changed product contract.

The final selected pair uses **one final observer and this same CLI**:

- [Unsupervised host observation](evidence/20261115/alignment-recovery-unsupervised.json):
  four cases, 16 CLI calls and 276 retained files. Product stop is observed,
  while the container's PID1 can leave an adopted aligner or child as a zombie.
- [Isolated fixture-supervisor observation](evidence/20261115/alignment-recovery-supervised.json):
  the same four cases, 16 CLI calls and 276 retained files, with separate actual
  waits for the already stopped adopted process.
- [Source/build/check bindings](evidence/20261115/alignment-recovery-bindings.json):
  the exact unchanged Rust test and eleven source blobs, raw/build/descriptor
  projections, final observer and 17 retained log identities.

Across the selected pair, 24 calls succeed and eight intended stopping calls
return execution exit7. Every stopped run retains the same inspection checkpoint
through compatible resume, executes alignment once, and then reuses both stages
on completed resume. Original fixtures stay byte-identical. No stopped run
publishes an accepted alignment or alignment checkpoint.

| Case in each observation | Provider outcome | Durable pipeline state |
| --- | --- | --- |
| Cancellation after all process identities are observed | Cancelled | Cancelled |
| Inner one-second tool deadline | Failed, native `ToolTimeout` | Failed |
| Inner stdout bound | Failed, native `ToolOutputLimit` | Failed |
| Outer one-second provider deadline, longer inner deadline | TimedOut | Failed |

### Stopping and reaping have different owners

In the original host observation, the outer provider is absent after the product
returns. Cancellation and outer timeout leave the aligner as an adopted zombie,
while its fixture-owned child and grandchild have been waited for. Inner timeout
and output limits wait for the direct aligner, but its child can become a zombie;
the grandchild has been waited for by that child. All four selected process
identities are not running. **A zombie is not counted as reaped.**

The optional `--fixture-reaper` runs the observer inside a separate temporary
Linux child with child-subreaper enabled only for that fixture process. It never
changes product library/CLI policy or the invoking parent's process policy.
The original PID/start tick and adoptive PPID must agree; all selected processes
must already be absent or zombie **after product return and before any external
wait**. Only then does the fixture supervisor call `waitpid` on those exact
adoptees, retaining each actual wait status and a separate all-absent observation.
The supervised waits returned status0 in these selected cases. Product-driven
SIGTERM/SIGKILL are also explicitly representable termination outcomes, not
automatically evidence of a fixture failure.

Neither the supervisor nor the synthetic tool handlers send a signal to a
descendant on a passing path; handlers only wait for children they own after the
product signals the whole inherited group. Product stopping, fixture-owned waits
and normal-init-like adoptive waits remain separate facts. This does **not**
establish universal product reaping, escaped-group containment, live
SIGKILL-fallback behavior, native-platform qualification or the historical
failure's cause. Those remaining limits stay open under #212.

Earlier diagnostic and observer iterations are retained separately. A missing
diagnostic field, changed resume deadline and an overly strict zero-only wait
assertion were observer setup mistakes, not evidence of a product defect. The
selected final fixture removes redundant child signals and pins actual waits.

Only scoped local execution is authorized for this checkpoint. Full product
tests/strict Clippy, platform/native-model qualification, private music/media,
human review, signing, immutable release, hosted CI and publication remain
outside this observation. Parent #212 and final Flow #88 remain open.

## Reproduce the final selected observation

Supply an explicitly captured CLI and its actual public source revision. The
output must be a new absolute directory whose parent exists:

```sh
python3 scripts/observe-alignment-recovery.py \
  --binary /absolute/path/to/captured/aniflow \
  --source-revision f851f9cc0f68291329ebc34acc36b16c07b9f82e \
  --work-directory /absolute/path/to/new-observation
```

Repeat with a different new directory and `--fixture-reaper` for the separately
scoped supervisor case. The original unsupervised observation must also be kept;
external waiting is not a substitute for reporting the host's zombie limit.

The observer does not download tools/models, install dependencies, invoke Cargo,
change caller originals or execute other tests. Reports distinguish the measured
source from later observer/documentation tips.
