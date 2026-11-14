# Tiny CPU/manual-SRT pilot — November 13, 2026 sprint

This checkpoint belongs to [#212](https://github.com/egohygiene/aniflow/issues/212).
The maintainer authorized focused local builds and synthetic smoke checks for
this batch. Earlier dated blanket deferrals remain history; they do not grant
or remove authority for other work. The selected path uses generated 64-frame
160×96/24-fps video, 128000 samples of 48-kHz stereo AAC and two supplied Unicode
SRT cues. Video and audio share an exact 8/3-second extent. No
private song/media, model, remote media acquisition, artifact signing or
publication is used.

## Compile bridge

The reviewed main input is `e6f3cd4d272d42f63aef034aeac25c22c3567202`, source
tree `b3354cfc83ee53f9d168b8aa20f29884748d3249`. A Rust 1.85 locked/offline CLI
check reproduced 19 library integration diagnostics after the earlier borrowed
path repair. Clearing those exposed one missing CLI import. The small bridge
repairs imports, token construction, borrowed error text, existing process
helper arguments, selected metadata stream indices and exhaustive variant
handling. It does not remove a processor, capture limit or admission check.

The gain-video metadata helper now reads the original video's inventory at
input 0/video stream 0/audio stream 1, independently of the selected gain-audio
input. Recovered unchanged-clock frames use the source RGB profile already
enforced by the complete carrier's binding validation and per-PNG structural
checks. Neither change establishes runtime/native/creative quality in those
optional branches.

Actual local observations:

- `cargo +1.85.0 check --locked --offline --bin aniflow`: passed, no warnings.
- `cargo +1.85.0 build --locked --offline --bin aniflow`: passed, jobs limited
  to two; the development build completed in 1m50s.
- Cargo manifest and lock bytes remain unchanged. Missing locked registry
  archives were fetched into the task-private Cargo cache before offline work.
- A copied development executable with only debug symbols stripped met the
  existing executable-size bound for initial diagnosis, without changing Cargo
  artifacts. Its fingerprint overhead motivated a normal optimized CLI build.
- The declared `release` profile built successfully in 8m30s, producing a
  27-MiB executable. Building a release profile is not publishing or qualifying
  a product release. The producer correction below rebuilt successfully in
  8m47s with the same declared release profile and two build jobs.
- Six Python observer/refusal regressions pass. The Taskfile parses with
  distinct aliases and explicit tool/output requirements.

## Why the old compatibility path was not accepted

An initial run through the existing Pipeline v2 CLI returned success and
produced the intended video/audio/subtitle stream inventory with FFmpeg 7.1.5.
The independent observer then refused its audio: that compatibility pipeline
decodes to PCM and encodes AAC at 320 kbit/s, changing packet bytes and extending
the observed audio duration by 240 samples. Its retained local failed receipt
is not relabeled as exact-audio or typed recipe qualification. Later assertions
in that failed attempt were not reached.

The selected script now requires real FFmpeg/ffprobe 8.0.1 and uses the existing
typed source recipe. It does not fall back to Pipeline v2 or change the product's
version gates. A separate refusal run using the installed 7.1.5 tools exits
before source generation and records the missing exact interface.

The real 8.0.1 tools were built in a private prefix with `make -j2`, from the
official fixed-version source. The detached source signature verifies with
fingerprint `FCF986EA15E6E293A5644F10B4322F04D67658D8`; the archive SHA-256 is
`05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41`.
Required development packages were independently hash-checked against signed
Debian Trixie metadata and extracted privately. No global packages/profiles
were changed. The software-only FFmpeg configuration disables networking;
its receipt additionally binds 14 host shared libraries.

Fourteen direct tool capability observations pass, including H.264/AAC, PNG,
FFV1 in MP4, float WAV, soft-SRT mux, drawtext and full decode. These are tool
checks, not an aniflow profile pass. The first strict provider attempt correctly
refused missing observed color primaries/transfer in the synthetic input.
The generator now supplies explicit encoder VUI declarations, independently
verified on the stream and every decoded frame, instead of weakening admission.

Another retained attempt used displayed 160×90 H.264 whose coded picture is
160×96. With cropping explicitly disabled, native framehash correctly refused
that discrepancy. The selected fixture is now uncropped, macroblock-aligned
160×96; this does not establish broad cropped-H.264 support.

The first actual PNG producer invocation then exposed a real FFmpeg 8 color
negotiation mismatch: its explicit scale output was RGB24 but declared a BT.709
YCbCr matrix, while the PNG encoder correctly requested RGB. The narrow fix
declares numeric `AVCOL_SPC_RGB = 0` on that conversion; input conversion
coefficients remain BT.709. `-noautoscale`, exact pixel-format restrictions and
all native validators stay enforced. All 48 corrected PNGs report RGB/full
range/BT.709 and decode to exactly the old intended reference bytes: 2,211,840
bytes, SHA-256 `7552cb65129d427364c7d7c2c0e78f051c5d7e51db2c3e43bc3609ac5a6ebf05`.
The unchanged master and delivery filter strings also pass their native
negotiation probes, with the master RGB matching the same reference.

The next actual recipe passed extraction, then correctly refused the original
two-second AAC fixture: its declared 96000-sample extent decoded to 96256 PCM
samples because the final AAC frame carried padding. The selected fixture uses
64 video frames and 128000 audio samples, an exact common 24-fps/48-kHz/1024-sample
AAC grid. Explicit synthetic-source `atrim=end_sample=128000` and a 48000-tick
movie timescale preserve that extent without decimal edit-list rounding. A
separate native probe observes 128000 decoded samples and 126 packets including
the original priming packet. No source decoder or complete-extent validator was
relaxed. General padded-AAC/cropped-H.264 coverage remains outside this subset.

The following recipe reached nested native audio inspection and identified a
tool provisioning gap: its exact decoded-audio SHA-256 command needs FFmpeg's
`hash` muxer, and the fixed true-peak command needs `apad`. A second private tool
prefix supplies both capabilities; earlier
tools and failed receipts remain retained. This does not change product code
or replace a failed observation with a passing claim.

With those tool features present, actual nested inspection identified a product
projection error: the native v2 inspector wrote a v2 capability into the closed
`audio-analysis/v1` vocabulary. Its normalized projection now uses that schema's
existing v1 family version, following the signal owner's existing pattern. The
actual native provider version, technical evidence and hashes remain unchanged.

The exact FFmpeg 8.0.1 meter also appends its normal audio-only null-mux accounting
and final progress after the ten meter summary fields, even with `-nostats`.
The parser now accepts that precisely validated two-line footer while retaining
all ten fields, units, finite-number/range checks and refusal of unknown trailing
diagnostics. Seven focused tests execute the actual parser, including both
captured native meter outputs. The inspection projection's focused regression
is authored; the full library test modules were not compiled or run.

An independent lower-level assembly check on the accepted 64-frame extraction
also exposed an authoring error before native encoding: an absent automatic
audio-stream selection became JSON `null` in a generated Pipeline v3 document.
Assembly now omits only its known absent selection fields when authoring that
document. Shared temporal serialization, existing provenance hashes, selection
meaning and the global null refusal remain unchanged. Focused audio/no-audio
and explicit-null regressions are authored; the full library test modules remain
unrun.

The diagnostic recipe then reached actual master assembly. Its live provider
guard counted the private snapshots under the candidate root but allowed only
the two final files. The default guard now accounts for the already-admitted
100000-frame ceiling plus ten enumerated fixed scratch/output files, using the
existing configured working-byte ceiling. The monitor still covers the complete
candidate tree. Explicit caller limits, media/report size limits, source checks
and strict validation of the two declared final files remain enforced.

## Repeat the selected technical subset

Build the current CLI through the locked contract. Select a release or copied,
debug-stripped executable that fits the existing provider identity bound. Supply
trusted real FFmpeg and ffprobe 8.0.1 executables. Each supplied executable must
fit the selected owner's existing 128-MiB identity bound. Tools are never installed by
the pilot itself. The output must be a fresh path outside the source checkout.

Version alone is insufficient for a minimal FFmpeg build. Before generating
assets, the pilot bounds and checks six native feature tables:

| Feature | Required selected capabilities |
| --- | --- |
| Filters | `testsrc2`, `sine`, `atrim`, `format`, `scale`, `setsar`, `aformat`, `aresample`, `ametadata`, `ebur128`, `apad` |
| Muxers | `mp4`, `wav`, `image2`, `framehash`, `hash`, `null` |
| Encoders | `libx264`, `png`, `ffv1`, `rawvideo`, `pcm_f32le`, `aac`, `wrapped_avframe`, `pcm_s16le` |
| Decoders | `h264`, `aac`, `png`, `ffv1`, `pcm_f32le` |
| Demuxers / inputs | `mov`, `wav`, `image2`, `lavfi` |
| Protocols | `file` and `pipe`, for both input and output |

Missing features fail before source generation. A retained run against the
first private build proves the missing-`apad` refusal. Advertised capabilities
do not replace actual command execution or the later content checks.

```bash
CARGO_BUILD_JOBS=2 cargo +1.85.0 build --release --locked --bin aniflow

task publishing:pilot \
  ANIFLOW="/absolute/private-tools/aniflow" \
  FFMPEG="/absolute/ffmpeg-8.0.1/bin/ffmpeg" \
  FFPROBE="/absolute/ffmpeg-8.0.1/bin/ffprobe" \
  OUTPUT="/absolute/fresh-pilot"
```

Without Task, invoke `python3 -B scripts/publishing-pilot.py` with the equivalent
`--aniflow`, `--ffmpeg`, `--ffprobe` and `--output` options. The script never
builds products or downloads tools. Across managed CLI launches it monitors a
120-second deadline, bounded process captures and a 512-MiB workspace budget,
including frozen provider/tool copies. The reused watchdog controls the CLI
process group. Native providers own separate groups and retain their existing
per-operation bounds; descendant-tree shutdown after a CLI timeout is not
qualified by this pilot. Failure retains the workspace, logs and a failed
receipt; existing output roots are refused before execution.

The fixed workflow executes the source recipe with repair/upscale disabled
and original-audio observation selected. It independently checks accepted
lossless-master/H.264 outputs, exact video clocks and AAC packet bytes/timestamps,
and fully decodes both outputs. Existing timed-text conversion supplies the
unreviewed WebVTT carrier and checks its words/intervals. The manual-lyrics owner
imports the supplied SRT against actual recipe/audio bindings; separate words
and timing reviews must remain awaiting. Export without those declarations
must refuse before creating a delivery directory. No human declaration is
invented. Completed compatible resume, changed-configuration resume refusal and
the final pass rehash both accepted videos. Existing-sidecar refusal rechecks
the exact WebVTT bytes in addition to its words and intervals.

## Actual local observations

The complete ordinary-CLI sequence passed in 17.509 seconds using a separately
labeled diagnostic build (`opt-level=1`, no debug information, no LTO). That
binary is 38959016 bytes, SHA-256
`92524d204cae6893ee228955f5bd087785ee96ffaa79af262c3e63efd8d31336`.
It observed both 64-frame outputs, 126 preserved AAC packets/configuration/
priming records, two supplied cues, completion metadata, compatible resume and
the selected refusals. Both human review states remain `awaiting_review`.

After that diagnostic, independent review strengthened the observer to freeze
both accepted video identities across resume/refusal/final checks and to reject
even a semantically equivalent WebVTT byte rewrite. The six Python cases pass,
including those preservation regressions.

The final declared Cargo release profile then built successfully in 8m16s with
Rust 1.85, locked/offline dependencies, two build jobs and no profile overrides.
Its canonical receipt was sealed on November 14 at 05:27 UTC; filenames retain
the sprint's November 13 date.
Its 27478528-byte executable has SHA-256
`5a3a0df529a80bc1a2856c3fd625f05e34ebc5b0db3056649e898e1b6c15b9f3`.
The stronger observer passed against that exact executable in 20.509 seconds,
using the real FFmpeg/ffprobe 8.0.1 r2 tools. The retained payload is 51282687
bytes, excluding its small receipt.

Both accepted videos contain exactly 64 frames at 24 fps, an 8/3-second video
clock, and the original 128000-sample 48-kHz stereo AAC extent. All 126 AAC
packets, rational PTS/DTS/durations, codec configuration and priming/padding
observations match the generated original. Full audio/video decode passes for
both files. The FFV1/bgr0 master is 872020 bytes; H.264/yuv420p delivery is 107194
bytes. The 124-byte WebVTT preserves both supplied Unicode cues and exact
intervals. Completion metadata binds the accepted file hashes and retains its
unasserted approval/publication state. Resume/refusal/final identity checks all
pass; human words/timing reviews remain held.

The sanitized
[observation](CPU-MANUAL-SRT-OBSERVATION-20261113.json) records the actual
diagnostic/canonical executable and harness hashes, output hashes, tool hashes,
selected command results and retained cropped/padded-input refusals without
private paths. A successful local build and tiny technical subset do not
qualify the product release or complete supported FFmpeg/native-host profile.

## Acceptance boundary

The report always distinguishes the observed tiny technical subset from full
profile, native-host, creative, release/install and publication acceptance.
Caption rendering, player typography and actual human word/timing/audio/visual
approval remain separate. The synthetic tone is measurement input, not a
mastering target; original-audio normalization remains `none`. The known
alignment cancellation/start-marker failure
is untouched and its cause remains unproven. The full #212 ledger remains open;
Flow #88's original final pass remains unchanged and last.
