# Selected alignment cancellation and recovery — November 15, 2026

This checkpoint observes the existing runtime and reviewed-lyrics alignment
owners with synthetic tools. It introduces no alignment model, transcription,
automatic lyric recognition, process scheduler or process-global subreaper.
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

## Execution state

The source observer is prepared. The unchanged exact Rust cancellation target
and final source-pinned CLI observation are pending; their executed results,
compiled-source/candidate identities and peer readback will be appended to this
same checkpoint. Earlier failed observer iterations remain private historical
observations, not product failures or passed evidence.

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
  --source-revision <actual-40-hex-compiled-source> \
  --work-directory /absolute/path/to/new-observation
```

The observer does not download tools/models, install dependencies, invoke Cargo,
change caller originals or execute other tests. Reports distinguish the measured
source from later observer/documentation tips.
