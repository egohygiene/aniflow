# Third-party integrations

`aniflow` invokes optional tools as independent executables. Their source,
models, binaries, and licenses are not vendored into this repository.

| Tool | Upstream | License | Boundary |
| --- | --- | --- | --- |
| FFmpeg | <https://ffmpeg.org/> | LGPL/GPL depending on build | Media inspection, extraction, encoding, muxing, and subtitle filters |
| Upscayl NCNN | <https://github.com/upscayl/upscayl-ncnn> | AGPL-3.0 | Optional `upscayl-bin` child process |
| Gemini Watermark Remover | <https://github.com/GargantuaX/gemini-watermark-remover> | MIT | Optional `gwr` child process |
| PocketSphinx 5.1.1 | [Pinned source](https://github.com/cmusphinx/pocketsphinx/tree/511126b492dcb267cf30d49d631946d7b61a9530) | [BSD-2-Clause and bundled-component notices](https://github.com/cmusphinx/pocketsphinx/blob/511126b492dcb267cf30d49d631946d7b61a9530/LICENSE) | Optional local English forced-alignment process; explicitly pinned acoustic files and dictionary, no downloads |
| Basic Pitch 0.4.0 | [Pinned source](https://github.com/spotify/basic-pitch/tree/9991303bba609a3b93089d13ec80d1d495083596) | [Apache-2.0](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/LICENSE) and [NOTICE](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/NOTICE) | Optional local note estimation with caller-pinned ONNX model; no weights or runtime vendored |
| ONNX Runtime 1.20.1 | [Pinned source](https://github.com/microsoft/onnxruntime/tree/v1.20.1) | [MIT](https://github.com/microsoft/onnxruntime/blob/v1.20.1/LICENSE) | Optional independently prepared CPU runtime for the Basic Pitch profile |

Users install these tools independently and are responsible for complying with
their licenses, model terms, source-media rights, and applicable platform terms.

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
