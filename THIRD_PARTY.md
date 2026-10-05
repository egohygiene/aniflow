# Third-party integrations

`aniflow` invokes optional media tools as independently prepared executables.
Its toolchain profiles do not install or redistribute those executables or
model weights. This is separate from the release-policy data snapshots and
their retained licenses under `third_party/release/`.

| Tool | Upstream | License | Boundary |
| --- | --- | --- | --- |
| FFmpeg | [Source/license reference](https://github.com/FFmpeg/FFmpeg/blob/ef52e1cc3850846987edc792c9583103e977e3b2/LICENSE.md), [official guidance](https://ffmpeg.org/legal.html) | LGPL/GPL depending on the actual build and enabled components; nonfree builds require separate treatment | Independent FFmpeg/ffprobe for media inspection, extraction, encoding, muxing, and subtitle filters; no binary bundled |
| Upscayl NCNN | [Pinned source](https://github.com/upscayl/upscayl-ncnn/tree/0beb39028a0ddd83250e845b4c3333c0675e3b97) | [AGPL-3.0](https://github.com/upscayl/upscayl-ncnn/blob/0beb39028a0ddd83250e845b4c3333c0675e3b97/LICENSE); supplied model terms recorded separately | Optional `upscayl-bin` child process; no executable/model pair bundled |
| GargantuaX Gemini Watermark Remover | [Pinned source](https://github.com/GargantuaX/gemini-watermark-remover/tree/dcf688f34f4d05cc1c33236d104b4624314748ed) | [MIT](https://github.com/GargantuaX/gemini-watermark-remover/blob/dcf688f34f4d05cc1c33236d104b4624314748ed/LICENSE), retaining Jad and AllenK attribution; native dependencies and optional models retain separate terms | Optional `gwr` child process; Node/package/native dependency identities remain distinct |
| allenk GeminiWatermarkTool | [Pinned source](https://github.com/allenk/GeminiWatermarkTool/tree/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e) | [MIT](https://github.com/allenk/GeminiWatermarkTool/blob/7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e/LICENSE); upstream separately attributes FDnCNN weights and runtime dependencies | Distinct optional image-tool profile; not a drop-in `gwr` command or proof of an installed provider |
| PocketSphinx 5.1.1 | [Pinned source](https://github.com/cmusphinx/pocketsphinx/tree/511126b492dcb267cf30d49d631946d7b61a9530) | [BSD-2-Clause and bundled-component notices](https://github.com/cmusphinx/pocketsphinx/blob/511126b492dcb267cf30d49d631946d7b61a9530/LICENSE) | Optional local English forced-alignment process; explicitly pinned acoustic files and dictionary, no downloads |
| Basic Pitch 0.4.0 | [Pinned source](https://github.com/spotify/basic-pitch/tree/9991303bba609a3b93089d13ec80d1d495083596) | [Apache-2.0](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/LICENSE) and [NOTICE](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/NOTICE) | Optional local note estimation with caller-pinned ONNX model; no weights or runtime vendored |
| ONNX Runtime 1.20.1 | [Pinned source](https://github.com/microsoft/onnxruntime/tree/v1.20.1) | [MIT](https://github.com/microsoft/onnxruntime/blob/v1.20.1/LICENSE) | Optional independently prepared CPU runtime for the Basic Pitch profile |

Users install these tools independently and are responsible for complying with
their licenses, model terms, source-media rights, and applicable platform terms.

The [toolchain upstream evidence](docs/toolchain-upstreams.md) records the
2026-10-04 source review, exact CLI differences and model/build limitations.
Source pins and upstream platform claims are not native compatibility or
redistribution qualification. #38 adds opt-in identity/readiness handling;
tests, builds and native qualification remain deferred under #64.

The planned Rust-native Gemini adapter may study and port the MIT-licensed
algorithm only with preserved attribution and license notices. Any future
decision to embed, link, fork, or distribute AGPL-covered Upscayl components
must receive a separate licensing and distribution review.

The [reviewed-lyrics alignment profile](docs/audio-lyrics-alignment.md) records
the [English acoustic model notice](https://github.com/cmusphinx/pocketsphinx/blob/511126b492dcb267cf30d49d631946d7b61a9530/model/en-us/en-us/README)
from Alpha Cephei and the supplied CMU dictionary identity separately from the
executable license. These are upstream declarations and identity evidence, not
independent authentication of supplied files or a legal conclusion. PocketSphinx
and its model/dictionary assets are not included in the aniflow package.

The [MIDI candidate profile](docs/audio-midi.md) records Basic Pitch's upstream
repository license and notice, including the bundled `icassp_2022` model asset.
No separate model license was found at the pinned revision. Supplied model
bytes are bound by digest; aniflow does not authenticate their publisher or
license status. The explicit local Python environment and its transitive
dependencies retain their respective licenses. Model inference, quality and
redistribution qualification remain separate from synthetic adapter tests.
