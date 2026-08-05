# aniflow

> Define the pipeline once. Transform every frame. Rebuild the experience.

`aniflow` is a reproducible, resumable Rust orchestrator for frame-based video
processing. Version 0.2.0 implements a real music-video path while preserving
extension seams for the larger system:

```text
inspect → extract → ordered frame processors → validate → assemble
        → audio processors → subtitles → whole-video processors
        → master → optional renderflow handoff → delivery manifest
```

The engine owns orchestration, checkpoints, validation, and provenance. FFmpeg,
Upscayl, Gemini Watermark Remover, and future processors retain ownership of
their specialized media operations.

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
- Optionally invoke a disabled-by-default Renderflow CLI handoff.

## Requirements

Base runtime:

```bash
brew install ffmpeg rust
```

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
executable `bin/gwr.mjs`; for Upscayl, it is the built `upscayl-bin`.

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

The final output and delivery manifest appear beneath one timestamped run:

```text
.aniflow/runs/<timestamp>-<pipeline>/
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

## Pipeline packs

- `pipelines/passthrough.yml`: FFmpeg-only timing and reconstruction proof.
- `pipelines/anime-upscale.example.yml`: first-class Upscayl NCNN adapter.
- `pipelines/gemini-clean-upscale.example.yml`: watermark removal followed by
  Upscayl.
- `pipelines/lyrics.example.yml`: prepared ASS subtitle burn.

See [Pipeline Schema](docs/pipeline-schema.md) for every pipeline v2 field.

## Validation

```bash
task validate
```

Equivalent commands:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
./scripts/smoke-test.sh
```

The synthetic smoke test generates a two-second video with audio, processes it
through the complete FFmpeg path, and inspects the master.

## aniflow and renderflow

`aniflow` owns time-based processing and produces a release-ready master.
`renderflow` will own alternate formats, thumbnails, previews, transcripts,
posters, metadata expansion, and derivative packages.

The v0.2.0 handoff is deliberately optional:

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

This reserves a stable boundary without pretending Renderflow's unfinished
contract is final.

## Known v0.2.0 constraints

- Frame reconstruction uses source average frame rate and targets
  constant-frame-rate inputs.
- Only the first video and first audio stream are processed.
- The frame interchange format is PNG.
- Completion caching is scoped to one run.
- Audio is decoded to PCM and encoded to AAC in the MP4 master.
- Continuity validation checks sequence, file integrity, and dimensions; visual
  flicker and motion-consistency analysis are future stages.
- External tool versions are diagnosed but not yet locked in a toolchain file.
- Cross-run content-addressed caching and targeted stage invalidation are not
  implemented.

## License

MIT
