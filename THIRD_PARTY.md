# Third-party integrations

`aniflow` invokes optional tools as independent executables. Their source,
models, binaries, and licenses are not vendored into this repository.

| Tool | Upstream | License | Boundary |
| --- | --- | --- | --- |
| FFmpeg | <https://ffmpeg.org/> | LGPL/GPL depending on build | Media inspection, extraction, encoding, muxing, and subtitle filters |
| Upscayl NCNN | <https://github.com/upscayl/upscayl-ncnn> | AGPL-3.0 | Optional `upscayl-bin` child process |
| Gemini Watermark Remover | <https://github.com/GargantuaX/gemini-watermark-remover> | MIT | Optional `gwr` child process |

Users install these tools independently and are responsible for complying with
their licenses, model terms, source-media rights, and applicable platform terms.

The planned Rust-native Gemini adapter may study and port the MIT-licensed
algorithm only with preserved attribution and license notices. Any future
decision to embed, link, fork, or distribute AGPL-covered Upscayl components
must receive a separate licensing and distribution review.
