# Toolchain upstream evidence

Read on 2026-10-04 for [#38](https://github.com/egohygiene/aniflow/issues/38).
These are source observations for opt-in profile design. They do not identify
an installed binary, qualify a host/backend, or establish media quality.
No dependency/model was installed, downloaded as a runnable asset, built or
executed. Native qualification and repository checks remain deferred under #64.

## Immutable source references

| Upstream | Reviewed revision and primary files | Source observation |
| --- | --- | --- |
| Upscayl NCNN | [`0beb39028a0ddd83250e845b4c3333c0675e3b97`](https://github.com/upscayl/upscayl-ncnn/tree/0beb39028a0ddd83250e845b4c3333c0675e3b97): [README](https://github.com/upscayl/upscayl-ncnn/blob/0beb39028a0ddd83250e845b4c3333c0675e3b97/README.md), [CLI](https://github.com/upscayl/upscayl-ncnn/blob/0beb39028a0ddd83250e845b4c3333c0675e3b97/src/main.cpp), [license](https://github.com/upscayl/upscayl-ncnn/blob/0beb39028a0ddd83250e845b4c3333c0675e3b97/LICENSE) | C++ `upscayl-bin`; AGPL-3.0 source. Build guidance covers Linux and macOS with Vulkan, OpenMP and platform-specific dependencies. |
| GargantuaX Gemini Watermark Remover | [`dcf688f34f4d05cc1c33236d104b4624314748ed`](https://github.com/GargantuaX/gemini-watermark-remover/tree/dcf688f34f4d05cc1c33236d104b4624314748ed): [README](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/README.md), [package](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/package.json), [CLI dispatch](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/src/cli/gwrCli.js), [remove command](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/src/cli/gwrRemoveCommand.js), [license](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/LICENSE) | Node package `@pilio/gemini-watermark-remover`, declared version `1.0.46`, exposing `gwr`; MIT. Default image file decoding/encoding additionally requires `sharp`. |
| allenk GeminiWatermarkTool | [`7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e`](https://github.com/allenk/GeminiWatermarkTool/tree/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e): [README](https://github.com/allenk/GeminiWatermarkTool/blob/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e/README.md), [build version](https://github.com/allenk/GeminiWatermarkTool/blob/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e/CMakeLists.txt), [CLI](https://github.com/allenk/GeminiWatermarkTool/blob/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e/src/cli/cli_app.cpp), [license](https://github.com/allenk/GeminiWatermarkTool/blob/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e/LICENSE) | Distinct C++ program, declared version `0.3.2`; MIT. README advertises several platforms and optional FDnCNN denoising; those are upstream claims, not aniflow qualification. |

A repository version or commit is source provenance. It is not the version,
digest or build configuration of a user's executable. Keep each identity and
its observation method separate.

## Interface facts that affect profiles

**Upscayl:** `-i`/`-o` select input/output; `-m` selects the model directory and
`-n` the model name. `-z` is native model scale; `-s` requests output scale.
Do not collapse these into one setting. The pinned source also infers scale
from some model names on its non-Windows branch, making observed output
geometry necessary. Model loading uses `.param` and `.bin` companions, with
a scale suffix for `realesr-animevideov3`. Its model-directory check expects
`models` in the path. `-g` selects GPU IDs; `-t` and `-j` configure tiling and
thread stages. `-v` means verbose, not version; the inspected parser offers
`-h` but no version flag. A help banner cannot prove an exact version.

**GargantuaX:** the CLI accepts `gwr remove INPUT --output FILE` or directory
input with `--out-dir DIRECTORY`; `--json` selects its result representation.
The inspected dispatch has no version command. Existing outputs require an
explicit `--overwrite`; aniflow must still confine new outputs and verify the
full planned set. A Node launcher file alone does not identify the package,
Node runtime, `sharp` native dependencies or optional video assets.

**allenk:** `--input`/`--output` accept image files/directories;
`--version` (`-V`) is registered. Region, detection threshold, profile
selection and optional denoising have separate flags. Default detection can
retry the legacy watermark profile; `--no-legacy` disables that behavior.
A skipped single image returns exit 1, failures return 2, and a batch can exit
0 even when members were skipped. Exact artifact observations remain required.
Do not substitute this executable for `gwr`: argv, results and skip behavior
differ. Its README describes video processing in a separate Veo repository;
that does not establish video support in this selected image executable.

Both Gemini projects describe visible-overlay removal and explicitly exclude
invisible/SynthID removal. Neither source claim establishes complete restoration,
watermark absence or the suitability of an individual frame.

These native CLIs do not implement aniflow's `--aniflow-invocation` provider
ABI. A dependency profile is not an automatic provider registration or a
replacement/fallback permission. A compatible adapter and its exact effective
configuration remain part of the normal provider resolution boundary.

## FFmpeg probe grammar for checkpoint #101

The fixed query parser was compared with primary source at
[`ef52e1cc3850846987edc792c9583103e977e3b2`](https://github.com/FFmpeg/FFmpeg/tree/ef52e1cc3850846987edc792c9583103e977e3b2),
without building or running that source:

- [opt_common.c](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/fftools/opt_common.c)
  supplies the program/version/copyright banner, six-character codec flags and
  two-character filter flags. Release token normalization is deliberately
  narrower than all possible vendor or development versions.
- [ffmpeg_opt.c](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/fftools/ffmpeg_opt.c)
  lists compiled hardware acceleration methods and the FFmpeg help sections.
  These methods do not demonstrate device or driver availability.
- [cmdutils.c](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/fftools/cmdutils.c),
  [opt_common.h](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/fftools/opt_common.h)
  and [ffprobe.c](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/fftools/ffprobe.c)
  define top-level help option rows, help aliases and FFprobe's main section.
  The parser does not promote private AVOptions or prose mentions into
  top-level command flags.
- Historical three-character TSC filter rows are represented in
  [n7.1 opt_common.c at `b08d7969c550a804a59511c7b83f2dd8cc0499b8`](https://github.com/FFmpeg/FFmpeg/blob/b08d7969c550a804a59511c7b83f2dd8cc0499b8/fftools/opt_common.c).
  Both observed row forms are accepted explicitly.

Synthetic fixtures describe these source-derived shapes; they are not captured
native qualification results. Unknown shapes remain unavailable instead of
being guessed. See [the probe boundary](toolchain-probes.md).

## macOS process observation for checkpoint #102

The macOS adapter uses safe APIs already supplied by the locked nix 0.31.3
dependency, enabling its `event` feature without changing the dependency
version. The primary implementation at
[`b5933ca178802b558a667514f717a86b3a1cedcc`](https://github.com/nix-rust/nix/blob/b5933ca178802b558a667514f717a86b3a1cedcc/src/sys/event.rs)
exposes owned `Kqueue`, `KEvent`, `EvFlags` and zero-time event polling.
Aniflow adds no unsafe Rust for this adapter.

Apple's XNU source at
[`f6217f891ac0bb64f3d375211650a4c1ff8ca1ea`](https://github.com/apple-oss-distributions/xnu/tree/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea)
informed the child-exit design:

- [kern_event.c](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_event.c)
  attaches `EVFILT_PROC` using a process reference, releases that reference
  after attachment and reports `ESRCH` when a process reference cannot be found.
  The filter observes future exit events; it does not replay every prior exit.
- [kern_proc.c](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_proc.c)
  defines process reference lookup and draining.
- [kern_exit.c](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_exit.c)
  drains references before later emitting `NOTE_EXIT` with wait-status bits.

The source-derived inference is that a successful attachment is protected
through registration, while a child already in exit may refuse attachment.
The adapter handles that case separately: retain the unreaped child while
bounded capture reaches natural pipe closure or a stop condition, finish group
signaling, disarm signals, then use the actual child wait status. Pipe EOF alone
is never treated as exit. This relies on exclusive
child-wait ownership and disabled automatic child reaping; it is not an OS
sandbox or proof against deliberately escaped descendants.

These source observations support the implementation choice. They are not
runtime evidence for any macOS version, architecture or installed tool.
Synthetic and native qualification remain deferred under #64.

## Code, model and binary terms

Upscayl's source license does not establish the terms or provenance of an
arbitrary supplied model. Record each actual `.param`/`.bin` pair, model origin,
terms, native scale and digests separately. No model pair or model license is
selected by this review.

allenk's pinned README attributes embedded FDnCNN weights to KAIR under MIT,
NCNN to BSD-3-Clause and volk to MIT. These are upstream declarations, not a
complete audit of every distributed binary and dependency. The GargantuaX
[ONNX manifest](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/public/models/allenk-fdncnn/onnx-manifest.json)
separately declares allenk-derived FDnCNN model provenance, MIT terms, a fixed
shape and a digest. Its package also lists additional model assets. Selecting
an image-only profile must not imply that those optional models were acquired,
licensed, inspected or qualified. Embedded and external weights both need
their own provenance/terms record; an executable digest alone does not supply it.

FFmpeg's [license at `ef52e1cc3850846987edc792c9583103e977e3b2`](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/LICENSE.md)
and [official licensing guidance](https://ffmpeg.org/legal.html) describe
LGPL-2.1-or-later as the base, with GPL/version-3 combinations depending on
enabled components. Libraries such as x264/x265 affect the combined license;
`--enable-nonfree` produces an unredistributable build under the upstream
guidance. Record the actual build, enabled codecs/libraries and supplier terms.
FFmpeg's name or version alone does not establish that a requested encoder,
filter or hardware backend exists, or that a particular binary can be bundled.

The boundary in [THIRD_PARTY.md](../THIRD_PARTY.md) remains independent,
caller-prepared executables. Profile support does not redistribute these
programs/models or clear a future bundle's notices, corresponding source,
model terms or codec-related obligations. Preserve upstream attribution if
code is ever copied or ported; treat redistribution as its own reviewed change.
