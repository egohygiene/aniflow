# aniflow

> Define the pipeline once. Transform every frame. Rebuild the experience.

`aniflow` is a reproducible, resumable Rust orchestrator for frame-based video
processing. Version 0.3.0 exposes the working music-video path through a typed
Rust library and versioned machine-readable CLI contract:

```text
inspect → extract → ordered frame processors → validate → assemble
        → audio processors → subtitles → whole-video processors
        → master → delivery manifest
```

The engine owns temporal orchestration, checkpoints, validation, and run
evidence. FFmpeg, Upscayl, Gemini Watermark Remover, and future processors
retain ownership of their specialized media operations.

The package exposes a reusable Rust library behind a thin standalone CLI. See
the [architecture graph](docs/architecture/README.md) and
[v1 roadmap](ROADMAP.md) for the accepted boundaries and staged evolution.

## Current capabilities

- Inspect source streams and timing with `ffprobe`.
- Extract predictably named lossless PNG frames and 24-bit PCM audio.
- Chain any number of ordered per-frame processors.
- Use first-class `upscayl_ncnn` and `gemini_watermark_remover` adapters.
- Add generic restoration, denoise, color, stylization, or custom commands.
- Use native directory batching for Upscayl and Gemini Watermark Remover.
- Resume generic per-frame processors by skipping valid frame outputs.
- Validate frame count, ordering, file integrity, and uniform dimensions.
- Run ordered audio and whole-video external processor chains.
- Restore processed or original audio as 320 kbps AAC.
- Burn ASS/SRT subtitles or mux a subtitle track.
- Snapshot configuration and subtitle inputs into each run.
- Record stage state, logs, media metadata, checksums, and delivery metadata.
- Preserve compact per-frame Gemini removal decisions as JSON Lines metadata.
- Embed diagnostics, inspection, planning, execution, resume, and status through
  a deliberately small crate-root Rust API.
- Emit a versioned JSON envelope and typed error category from every CLI
  operation.
- Publish provider-native v1 manifest, effective-configuration, and
  compatibility-fingerprint contracts for temporal extension authors and
  independent consumers.
- Resolve explicitly registered local providers through deterministic
  replacement, primary, and fallback policy with exact provider locks.
- Parse strict Pipeline v3 intent and resolve it through the read-only
  `plan_v3` file facade, lower-level `resolve_pipeline_v3` library API, or
  `plan-v3` CLI into a canonical, self-validating plan with exact provider
  locks and resolution attempts.
- Execute resolved providers with cancellation, wall-clock and capture bounds,
  process-tree termination, redacted diagnostics, strict artifact limits, and
  output validation independent from exit status.
- Adapt every pipeline v2 frame, batch, audio, and whole-video processor to the
  same provider runtime, with one lock per stage and one execution report per
  invocation.
- Split videos into explicit sub-30-second segments with either keyframe-aligned
  stream copy or frame-accurate H.264/AAC transcoding.
- Resume a verified segment prefix and reconstruct it through an immutable,
  checksummed manifest with duration validation.
- Preserve the disabled-by-default renderflow handoff only on the pipeline v2
  compatibility path; pipeline v3 configuration rejects it.

## Requirements

Base runtime:

```bash
brew install ffmpeg rust
```

Rust `1.85` is the minimum supported compiler; current stable Rust is the
recommended toolchain.

Optional processors:

- [`upscayl-bin`](https://github.com/upscayl/upscayl-ncnn) for the
  `upscayl_ncnn` adapter.
- [`gwr`](https://github.com/GargantuaX/gemini-watermark-remover) for the
  `gemini_watermark_remover` adapter.

Install Gemini Watermark Remover and its image codec:

```bash
pnpm add --global @pilio/gemini-watermark-remover sharp
```

Upscayl NCNN currently requires a source build or a compatible binary plus its
model files. Its upstream README includes Apple Silicon CMake and MoltenVK
instructions. The executable is named `upscayl-bin`.

If either tool is already cloned instead of installed on `PATH`, set `command`
to its absolute executable path. For Gemini Watermark Remover, that may be the
executable `bin/gwr.mjs`; for Upscayl, it is the built `upscayl-bin`. Pipeline
v2 retains bare command names for compatibility: aniflow resolves such a name
once from the caller's `PATH`, converts it to an absolute path, and then
registers that exact executable. The provider registry itself never performs
implicit discovery.

## Quick start

```bash
cargo build --release

./target/release/aniflow doctor

./target/release/aniflow inspect "/path/to/video.mp4"

./target/release/aniflow plan \
  --input "/path/to/video.mp4" \
  --pipeline "pipelines/passthrough.yml"

./target/release/aniflow run \
  --input "/path/to/video.mp4" \
  --pipeline "pipelines/passthrough.yml"
```

These `plan` and `run` commands use the Pipeline v2 compatibility path. To
resolve Pipeline v3 intent without launching providers or creating a run
workspace, supply every input, provider-registration locator, host observation,
and authority grant explicitly:

```bash
./target/release/aniflow plan-v3 \
  --pipeline "pipeline-v3.yml" \
  --input "source-video=/path/to/video.mp4" \
  --provider-registration "providers/upscale.registration.json" \
  --host-cpu-threads 8 \
  --host-memory-mib 16384 \
  --host-storage-mib 65536 \
  --host-gpu-available \
  --allow-side-effect filesystem-read \
  --allow-side-effect filesystem-write \
  --allow-side-effect subprocess \
  --allow-side-effect gpu \
  --offline
```

Each `--input` is an authored artifact ID and local path in `ID=PATH` form.
Each registration document uses `aniflow.provider-registration/v1` and names a
manifest, effective configuration, executable, implementation ID, and observed
components. Locator paths are resolved relative to that document and never
enter plan identity. Repeat both flags when the pipeline needs more inputs or
provider candidates.

Human output is the default. Every command also exposes the same application
result through the machine contract:

```bash
./target/release/aniflow plan \
  --input "/path/to/video.mp4" \
  --pipeline "pipelines/passthrough.yml" \
  --output json
```

`plan-v3` accepts the same `--output json` mode. On success, its envelope result
is an `aniflow.pipeline-plan/v1` document. On failure, the result retains an
`aniflow.pipeline-planning-failure/v1` diagnostic, including the affected stage
and deterministic provider-resolution attempts when applicable.

Successful envelopes are written to standard output, failure envelopes to
standard error, and the exit code identifies the typed failure category. See
the [public contract and compatibility policy](docs/contracts/README.md).

The final output and delivery manifest appear beneath one timestamped run:

```text
.aniflow/runs/<timestamp>-<pipeline>/
├── providers/<stage>/provider-lock.json
├── providers/<stage>/reports/<invocation>.json
├── output/master.mp4
└── delivery/manifest.json
```

Resume or inspect a run:

```bash
./target/release/aniflow resume \
  ".aniflow/runs/20260726T220000Z-gemini-clean-upscale"

./target/release/aniflow status \
  ".aniflow/runs/20260726T220000Z-gemini-clean-upscale"
```

## Rust library

Add aniflow to another Rust project's dependencies while the public preview is
developed from `main`:

```toml
[dependencies]
aniflow = { git = "https://github.com/egohygiene/aniflow", branch = "main" }
```

The crate-root facade returns application data without parsing CLI arguments or
printing human output:

```rust,no_run
use aniflow::{Result, RunRequest};

fn process_video() -> Result<()> {
    let plan = aniflow::plan("source.mp4", "pipelines/passthrough.yml")?;
    println!("{} stages", plan.stages.len());

    let outcome = aniflow::run(RunRequest::new(
        "source.mp4",
        "pipelines/passthrough.yml",
    ))?;
    let status = aniflow::status(&outcome.run_directory)?;
    println!("{} completed stages", status.stages.len());
    Ok(())
}
```

Use `run_with_progress` or `resume_with_progress` when an embedding application
needs lifecycle observations. Use `run_with_progress_and_cancellation` or
`resume_with_progress_and_cancellation` to supply a shared `CancellationToken`
that is forwarded to processor invocations. The public API is intentionally
small and pre-1.0; `ErrorCategory`, `MachineEnvelope`, and command result types
provide the `0.3.x` integration boundary.

Temporal provider authors and registries can use `ProviderManifest`,
`ProviderConfiguration`, `CompatibilityFingerprint`, `ProviderRegistry`, and
the versioned provider lock/event/report types through the crate root. Local
executables must be registered explicitly and are launched only after exact
resolution and authority checks. See the [temporal provider
contract](docs/provider-contract.md).

Pipeline v3 integrators use `plan_v3` for the same file-based boundary as the
CLI, or `PipelineV3Configuration` and `resolve_pipeline_v3` with a prebuilt
registry. The planner is deliberately read-only: it hashes explicit inputs,
validates the authored stage graph and artifact bindings, and resolves only
explicitly supplied provider registrations. It does not discover providers,
launch processes, create output directories, or execute/resume a v3 pipeline.
Operational logs and telemetry may observe a caller in the future, but they are
never canonical plan, provider-lock, fingerprint, or artifact identity.

`flow` should consume the library facade when running in-process and the v1
machine envelope when a process boundary is required. See the dedicated
[`flow` integration guide](docs/integrations/flow.md).

The independent consumer example exercises every supported application
operation without importing the CLI parser:

```bash
cargo run --example library -- inspect "/path/to/video.mp4"
cargo run --example library -- plan \
  "/path/to/video.mp4" \
  "pipelines/passthrough.yml"
```

## Short-segment workflows

Plan before writing media:

```bash
aniflow segment plan \
  --input "source.mp4" \
  --output-directory ".aniflow/segments/demo" \
  --segment-duration-ms 10000 \
  --mode stream-copy
```

Run and later resume the same isolated workspace:

```bash
aniflow segment run \
  --input "source.mp4" \
  --output-directory ".aniflow/segments/demo" \
  --segment-duration-ms 10000 \
  --mode transcode-h264-aac

aniflow segment resume \
  --run-directory ".aniflow/segments/demo"
```

Reconstruct only after the complete manifest and all segment checksums verify:

```bash
aniflow segment reconstruct \
  --run-directory ".aniflow/segments/demo" \
  --output-file "reconstructed.mp4"
```

See [Short-segment workflows](docs/short-segments.md) for timing semantics,
recovery behavior, public Rust APIs, and flow/renderflow ownership boundaries.

## First real Gemini music-video pass

The ready-made pipeline removes the small visible Gemini mark before upscaling.
That order avoids enlarging the watermark before removal.

First, edit the pipeline if your Upscayl models are not resolved from the
current working directory:

```yaml
model_path: /absolute/path/to/upscayl-ncnn/models
```

Then verify every enabled dependency:

```bash
./target/release/aniflow doctor \
  --pipeline "pipelines/gemini-clean-upscale.example.yml"
```

Plan and run:

```bash
./target/release/aniflow plan \
  --input "/path/to/music-video.mp4" \
  --pipeline "pipelines/gemini-clean-upscale.example.yml"

./target/release/aniflow run \
  --input "/path/to/music-video.mp4" \
  --pipeline "pipelines/gemini-clean-upscale.example.yml"
```

Upscayl and Gemini Watermark Remover run in their native directory modes, so
each tool loads once for the complete stage. Upscayl controls its internal GPU
and worker behavior through its own options.

Use watermark removal only for media you created or are authorized to modify.
The adapter targets Gemini's visible overlay; it does not remove invisible
provenance systems such as SynthID.

## Processor model

### Frame processors

Frame processors are ordered. Every processor reads the complete output
directory of the previous processor and writes a new immutable stage directory:

```text
frames/source
  → frames/stages/01-remove-gemini-watermark
  → frames/stages/02-upscale
```

Built-in adapter kinds:

| Kind | Command | Purpose |
| --- | --- | --- |
| `gemini_watermark_remover` | `gwr` | Native directory batch; reverse-alpha removal of supported Gemini visible marks |
| `upscayl_ncnn` | `upscayl-bin` | Native directory batch; NCNN/Real-ESRGAN frame upscaling |
| `external` | configured | Concurrent per-frame restoration, denoise, grading, stylization, or custom logic |

### Audio and video processors

Audio and whole-video processors use one safe external-command contract. The
command runs directly, without a shell:

| Placeholder | Meaning |
| --- | --- |
| `{input}` | Absolute input artifact |
| `{output}` | Absolute output artifact the command must create |
| `{run_dir}` | Run workspace |

Example whole-video interpolation stub:

```yaml
video_processors:
  - id: interpolate
    enabled: false
    command: rife-ncnn-vulkan
    arguments:
      - --input
      - "{input}"
      - --output
      - "{output}"
    output_extension: mp4
```

Disabled entries document future intent without creating runtime dependencies.

Every enabled pipeline v2 processor is normalized into a typed local provider,
resolved before expensive media work, and executed with direct arguments. The
runtime confines output to a fresh invocation directory, enforces configured
bounds, and validates the declared artifact before aniflow performs its
processor-specific checks and promotes it into the stage. Exit code zero alone
never completes a processor stage.

All frame, audio, and video processor entries accept the same optional `limits`
object:

| Field | Default |
| --- | ---: |
| `timeout_seconds` | `21600` |
| `termination_grace_milliseconds` | `2000` |
| `maximum_stdout_bytes` | `67108864` |
| `maximum_stderr_bytes` | `67108864` |
| `maximum_artifact_files` | `1000000` |
| `maximum_artifact_bytes` | `1099511627776` |

The provider lock binds the adapter configuration and executable digest. When
`upscayl_ncnn.model_path` is explicit, it also binds the selected `.param` and
`.bin` model files. Execution reports retain termination, bounded redacted
captures, lifecycle events, artifact observations, and the terminal outcome.

## Pipeline packs

- `pipelines/passthrough.yml`: FFmpeg-only timing and reconstruction proof.
- `pipelines/anime-upscale.example.yml`: first-class Upscayl NCNN adapter.
- `pipelines/gemini-clean-upscale.example.yml`: watermark removal followed by
  Upscayl.
- `pipelines/lyrics.example.yml`: prepared ASS subtitle burn.

See [Pipeline schemas](docs/pipeline-schema.md) for Pipeline v3 planning and
every Pipeline v2 compatibility field.

## Validation

The full local gate is:

```bash
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --doc
python3 scripts/check-contracts.py
./scripts/check-product-names.sh
./scripts/test-product-names.sh
cargo package --locked
./scripts/smoke-test.sh
```

CI repeats the locked check, all-target tests, and doc tests on the minimum
supported Rust 1.85 toolchain.

The synthetic smoke test generates a two-second video with audio, processes it
through both the complete FFmpeg path and a hermetic external-frame provider,
inspects the master, and verifies retained provider lock/report evidence.

## Suite boundary

`aniflow` owns time-based processing and produces a release-ready master.
`renderflow` owns its independent transform and derivative domain. `flow` owns
cross-tool selection, sequencing, compatibility, and suite-level provenance.

aniflow does not directly depend on renderflow, optiflow, or flow in the target
architecture. They may consume its released library or CLI externally.

Pipeline v2 still contains an optional, disabled-by-default renderflow handoff:

```yaml
renderflow:
  enabled: false
  command: renderflow
  arguments:
    - run
    - --input
    - "{input}"
    - --output-directory
    - "{output}"
```

This is a deprecated compatibility seam rather than the future integration
contract. Pipeline v3 configuration and planning reject cross-holon renderflow
selection. When v3 execution is implemented, flow will receive the validated
master and aniflow run evidence through a versioned public boundary.

## Known constraints

- Frame reconstruction uses source average frame rate and targets
  constant-frame-rate inputs.
- Only the first video and first audio stream are processed.
- The frame interchange format is PNG.
- Completion caching is scoped to one run.
- Audio is decoded to PCM and encoded to AAC in the MP4 master.
- Continuity validation checks sequence, file integrity, and dimensions; visual
  flicker and motion-consistency analysis are future stages.
- Pipeline v2 processor executables are content-hashed and locked per stage;
  FFmpeg, FFprobe, and the deprecated renderflow handoff remain on their legacy
  dependency paths.
- Cross-run content-addressed caching and targeted stage invalidation are not
  implemented.
- Completion markers do not yet prove processor, configuration, input, and
  validated-output compatibility.
- Pipeline v3 currently stops at deterministic, read-only planning; v3
  execution, state, recovery, checkpoint reuse, and resume are not implemented.
- Public run progress remains stage-level and provisional; provider execution
  reports retain versioned invocation lifecycle events, and stable command
  results and error categories remain available in `0.3.x`.

## License

MIT
