# Try aniflow with synthetic videos

Generate your own tiny videos, open them in a player, then run the source-video
recipe on them. The demo makes its own pixels and contains no third-party
footage, music, faces or private files. It needs no neural models.

This is a development-checkout walkthrough. The demo and source-entry recipe
are currently in stacked drafts, so a checkout must contain
`codex/24-synthetic-video-demo` until those changes merge. No release is implied.
The generator, recipe and tests are authored; their execution remains deferred
under [#64](https://github.com/egohygiene/aniflow/issues/64). The commands below
are for your explicitly selected local trial, not a record of passed checks.

## Get the checkout and tools

For a fresh checkout of this draft:

```bash
git clone --branch "codex/24-synthetic-video-demo" "https://github.com/egohygiene/aniflow.git"
cd "aniflow"
```

You need Python 3.10+ and local FFmpeg **8.0.1** with `libx264`, plus ffprobe
**8.0.1**. Supply their absolute paths below. The version requirement comes
from the current source recipe's supported tool profile. The script records
the resolved executables and their actual SHA-256 values; it does not install
or download tools. Task is optional. Rust/Cargo is needed only when you build
and try aniflow or run its Rust tests.

## Generate and watch the clips

Choose a new directory outside the checkout, with an existing parent:

```bash
export ANIFLOW_DEMO="$HOME/aniflow-demo"

python3 -B scripts/demo-music-video.py \
  --output "$ANIFLOW_DEMO" \
  --ffmpeg "/absolute/path/to/ffmpeg" \
  --ffprobe "/absolute/path/to/ffprobe"
```

Or use the equivalent Task shortcut:

```bash
task demo:video OUTPUT="$ANIFLOW_DEMO" \
  FFMPEG="/absolute/path/to/ffmpeg" FFPROBE="/absolute/path/to/ffprobe"
```

| File | What to expect |
| --- | --- |
| `landscape.mp4` | A 320×180 moving test pattern, 48 frames at 24 fps: two seconds, no audio |
| `portrait.mp4` | The corresponding 180×320 portrait pattern with the same clock |
| `landscape.recipe.json`, `portrait.recipe.json` | Source-recipe configurations with actual local tool pins and repair/upscale disabled |
| `landscape.y4m`, `portrait.y4m` | Deterministic raw source frames used to encode the MP4s |
| `manifest.json`, `logs/` | Generator/tool identities, observed media properties, file digests and retained process logs |

Open either MP4 with your usual video player. The moving pattern makes it easy
to see that frames advance and that orientation and dimensions are preserved.
The raw sources are reproducible; encoded bytes can differ between FFmpeg or
encoder builds, so the manifest records the files actually produced.

The generated recipe uses small explicit media limits for this demo. Allow a
few GiB of free scratch space when running it: the pipeline retains source,
frame and output copies and uses conservative admission bounds, even though
the finished clips are small. These settings are for the two demo clips;
review the normal recipe limits before substituting a longer or larger video.

Generation only creates the demo pack. It does not run aniflow, tests, a model
or a package installer. Its media observations are separate from aniflow's
own acceptance of a recipe run. Output directories are never overwritten or
automatically cleaned. After an interrupted or refused generation, inspect
the retained logs and choose a fresh output directory for another attempt.

## Plan and run the recipe

Build the development binary once. This command can fetch Cargo dependencies
if they are not already cached:

```bash
cargo build --locked --bin aniflow
```

Inspect the plan first:

```bash
./target/debug/aniflow --output json video music-video \
  --source-video "$ANIFLOW_DEMO/landscape.mp4" \
  --configuration "$ANIFLOW_DEMO/landscape.recipe.json" \
  --plan
```

The initial plan binds extraction exactly. Downstream frame geometry and plans
remain pending until extraction is accepted. Upscale is explicitly disabled.
That pending state is expected for source entry.

Then run it, using the human-readable output for your first attempt:

```bash
./target/debug/aniflow video music-video \
  --source-video "$ANIFLOW_DEMO/landscape.mp4" \
  --configuration "$ANIFLOW_DEMO/landscape.recipe.json" \
  --output-directory "$ANIFLOW_DEMO/runs"
```

The recipe extracts frames, imports them unchanged and assembles a master and
delivery under a new run directory. Open the reported delivery MP4 to compare
it with the synthetic input. The FFV1 master is an archival output and may
need a player with FFV1 support. The generated sources stay unchanged. Repeat
with `portrait.mp4` and `portrait.recipe.json` to try the other orientation.

Keep the exact **recipe root** printed by the run. Set that path explicitly
to inspect or resume it:

```bash
export ANIFLOW_DEMO_RUN="/absolute/recipe/root/reported/by/aniflow"

./target/debug/aniflow video music-video-status "$ANIFLOW_DEMO_RUN"

./target/debug/aniflow video music-video \
  --source-video "$ANIFLOW_DEMO/landscape.mp4" \
  --configuration "$ANIFLOW_DEMO/landscape.recipe.json" \
  --resume-run-directory "$ANIFLOW_DEMO_RUN"
```

Resume uses the same source, configuration, binary/tool identities and accepted
child outputs. You can try Ctrl-C during a run and resume from its reported
root; these clips may finish quickly. Completed compatible resumes reuse
accepted work. Incomplete initialization or an abandoned writer lock is a
visible refusal requiring inspection, not permission to delete retained files.

The demo deliberately leaves neural repair/upscale off. Enabling those later
requires the selected real tools/models and reviewed settings from the
[recipe guide](music-video-recipe.md) and
[media setup guide](toolchain-media-setup.md). It does not demonstrate their
visual quality, audio passthrough, longer-video resource use or native support.

## Run automated checks yourself

The small demo contract suite uses only Python's standard library. It neither
encodes media nor builds or invokes aniflow:

```bash
python3 -B tests/demo_music_video.py
# Equivalent:
task demo:video:test
```

The focused Rust source-entry tests use synthetic inputs and substitute tools:

```bash
cargo test --locked --test source_music_video_recipe
```

For the broader generated temporal/provider corpus, follow
[the corpus guide](adversarial-corpus.md). Its qualification runner uses offline
Cargo and requires dependencies to be provisioned first. Keep the receipt from
each run: fixture generation, a completed demo recipe, the automated suites and
native/model qualification establish different facts. None has been marked
passed by adding this walkthrough.
