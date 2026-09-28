# Offline vocal separation

The `audio:separate` task produces **candidate** vocals, accompaniment, and
machine-readable evidence through the existing Pipeline v3 provider runtime.
It preserves the source and keeps completed outputs in a new run workspace.
Separation quality still needs listening/review; a valid WAV is not proof that
the model isolated the desired sounds.

## Supported first profile

| Property | Supported behavior |
| --- | --- |
| Platform | Unix profile; Linux synthetic checks executed; native macOS unverified; Windows unsupported |
| Input | RIFF PCM16 WAV, mono/stereo, 8–192 kHz, nonempty, at most 600 seconds and 256 MiB |
| Dependency | Demucs **4.0.1**, a compatible Python/PyTorch/Torchaudio environment, FFmpeg and FFprobe in the same binary directory |
| Model | `htdemucs_6s`, local `htdemucs_6s.yaml` and `5c90dfd2-34c22ccb.th` |
| Execution | CPU; `--two-stems vocals --shifts 0 --jobs 0`; explicit local `--repo` |
| Outputs | `vocals.wav`, `accompaniment.wav`, and `evidence.json` |
| Stem validation | Complete PCM16 stereo WAV at 44.1 kHz; equal stem sample counts; each duration within 20 ms of source |
| Policy | Offline only; no downloads; no overwrite; source and model cache outside generated output |
| Reproducibility | Exact configuration/local file digests; environment-dependent inference; no cross-run cache claim |

Other input formats, GPU, model choices, RF64/extensible/float WAV, arbitrary
duration, and automated perceptual acceptance are outside this profile. Export
a separate supported WAV first when needed; this task never converts or
overwrites the original for you.

## Prepare dependencies separately

Install Demucs 4.0.1 and compatible PyTorch/Torchaudio into a dedicated virtual
environment following the [upstream v4.0.1 instructions](https://github.com/facebookresearch/demucs/tree/v4.0.1).
Provision the model assets while network access is deliberately available, then
place both files in a persistent model directory outside your project/run
artifacts. The upstream
[model bag](https://github.com/facebookresearch/demucs/blob/v4.0.1/demucs/remote/htdemucs_6s.yaml)
selects `5c90dfd2`; the
[model file list](https://github.com/facebookresearch/demucs/blob/v4.0.1/demucs/remote/files.txt)
identifies `5c90dfd2-34c22ccb.th`. Use trusted model files: loading PyTorch
checkpoints is an executable-code trust decision.

Ordinary processing **never** installs a Python package, warms a missing model,
or falls back to another model. A generic Torch Hub cache alone is insufficient:
the [local-repository implementation](https://github.com/facebookresearch/demucs/blob/v4.0.1/demucs/pretrained.py)
must receive the explicit directory with the bag and checkpoint. The narrow
adapter verifies that bag selects only the expected signature and rejects
ambiguous checkpoint filenames.

Create a new provider bundle after installing those dependencies:

```bash
task audio:prepare \
  PYTHON="/absolute/venv/bin/python" \
  FFMPEG="/absolute/bin/ffmpeg" \
  FFPROBE="/absolute/bin/ffprobe" \
  MODEL_REPOSITORY="/absolute/models/demucs-4.0.1" \
  BUNDLE="/absolute/providers/demucs-local"
```

The bundle parent must already exist. Paths must be absolute with canonical
parent directories; the Python executable may remain a venv symlink so its
environment is preserved. Preparation checks installed Demucs metadata and
records observed SHA-256 digests of Python, FFmpeg, FFprobe, the bag, and checkpoint.
Those digests detect local drift; they do not authenticate the download origin
or attest every installed Python package. Preparation refuses an existing bundle
and downloads nothing. Keep the bundle private: effective configuration contains
local dependency paths.

## Run and inspect

Build the current source with `cargo build --locked --bin aniflow` if you do not
already have a compatible binary. Supply the resources you have actually made
available to this job (the declared admission floor is one CPU thread, 4096 MiB
memory and 2048 MiB storage; it is not a measured inference peak):

```bash
task audio:separate \
  ANIFLOW_BIN="$(pwd)/target/debug/aniflow" \
  INPUT="/absolute/audio/Synthetic source.wav" \
  BUNDLE="/absolute/providers/demucs-local" \
  OUTPUT="/absolute/generated-runs" \
  CPU_THREADS="4" MEMORY_MIB="8192" STORAGE_MIB="16384"
```

The task rechecks dependencies, then delegates to `aniflow run-v3`. It grants
only filesystem read/write, environment read, subprocess and local AI effects;
it does not grant network, GPU, or publication. The machine envelope reports the
run directory and output paths. Lifecycle progress and raw tool diagnostics
stay separate from structured stdout; fractional model progress is not parsed.

```bash
aniflow --output json status-v3 "/absolute/generated-runs/RUN_ID"
```

The provider validates the complete PCM payload independently of Demucs and
checks both input and model/tool identities again after separation. Its
`aniflow.demucs-separation/v1` evidence records input/output digests, bytes,
sample rate, channels, exact rational duration, model/tool observations, offline
settings and explicit untested perceptual quality. Pipeline v3 separately
records process outcome, immutable output observations, integrity validations,
checkpoint and run-manifest revisions. The evidence JSON alone is not a
completed Pipeline v3 checkpoint.

## Resume, failures, and limits

```bash
task audio:resume \
  ANIFLOW_BIN="$(pwd)/target/debug/aniflow" \
  INPUT="/absolute/audio/Synthetic source.wav" \
  BUNDLE="/absolute/providers/demucs-local" \
  RUN="/absolute/generated-runs/RUN_ID"
```

Use this task or the equivalent `workflow.py resume` path for this provider.
It reobserves model/tool digests **before** cached checkpoint reuse. Calling
generic `resume-v3` directly with a stale registration cannot discover changes
to files referenced only inside provider configuration. Library embedders must
perform the same dependency verification before `resume_v3`.

An unchanged completed run reuses its checkpoint without separation. Changed
source, provider bundle, executable or model material refuses reuse. Prepare a
new bundle and start a new run after an intentional dependency upgrade. Missing,
empty, truncated, corrupt, wrong-duration, unexpected, symlinked or failed
outputs never produce an accepted checkpoint. Failed attempt evidence stays
available for inspection. Interruptions can leave unaccepted private attempt
staging; never promote those files manually or treat their existence as success.

The task uses existing runtime process-tree cancellation and a six-hour default
deadline, 1 MiB stdout, 4 MiB stderr, and eight transient files / 1 GiB. That
transient budget includes the input snapshot, Demucs decode temporary file,
stems and publication links; successful output validation still requires exactly
the three declared artifacts. Python bytecode writes are disabled and Demucs'
temporary directory is confined beneath the private attempt staging directory.
Use `python3 providers/demucs/workflow.py run --help` for the direct wrapper;
its `--timeout-seconds` also applies to resume. CPU/RAM and intermediate disk
use are not hard OS quotas. Demucs' own random shifts are disabled, but bitwise
output reproducibility across PyTorch/hardware environments is not promised.

`TORCH_HUB_OFFLINE=1`, `HF_HUB_OFFLINE=1`, isolated Python and the explicit local
repository remove the ordinary download path. They are **not an OS network
sandbox**. Executables and Python/model installations are trusted local code;
hash checks do not eliminate concurrent same-user path swaps. For network-denial
qualification, run the real-model compatibility test on a host with networking
disabled and record that host evidence separately.

## Pipeline composition and verification

`providers/demucs/pipeline.yml` names three independent output ports. A later
audio processor may bind either WAV as a temporal component; the evidence port
remains validation evidence. No music-video orchestration or cross-holon
selection is added here. The provider performs audio checks internally;
Pipeline v3 still uses its existing built-in artifact-integrity gate, not a new
provider-backed validation framework.

```bash
task audio:test
```

The tests generate their own PCM WAV and fake dependency executables. They run
the real adapter through public Pipeline v3 APIs, exercise refusal/cancellation,
and prove compatible resume. They do not require network, weights, GPU, or real
media, and do not prove Demucs inference compatibility. See the [local validation
receipt](validation/aniflow-8-local.json) for executed versus unavailable gates. The real-model compatibility
test is: prepare trusted Demucs 4.0.1 / `htdemucs_6s`, disconnect networking,
generate a short synthetic PCM16 fixture, run `audio:separate`, verify both stems
and evidence, then run `audio:resume` and verify no separation relaunch. Record
OS, dependency environment, source/model/tool digests and the complete run.
