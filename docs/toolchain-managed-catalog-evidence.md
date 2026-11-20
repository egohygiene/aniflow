# Managed Upscayl and RIFE catalog evidence

The [bundled catalog](../providers/managed/catalog.v1.json) contains nine
versioned package selections: anime and general Upscayl profiles and one RIFE
profile, each for Linux x86_64, macOS x86_64 and macOS aarch64. The underlying
Mac assets are universal binaries; the architecture rows select their native
slices. These are verified download and file identities, not a green native
qualification matrix.

On 2026-11-20, official GitHub release metadata was read through the connected
GitHub source, then the exact published assets were acquired with GitHub CLI.
The immutable Real-ESRGAN license was acquired over HTTPS. Full downloads and
synthetic fixtures stay outside the repository. No user application, model
directory, video, image, private key or private asset was accessed.

## Published assets and measured identities

| Official asset | Bytes | Observed SHA-256 |
| --- | ---: | --- |
| [Upscayl Linux 20251207-174704](https://github.com/upscayl/upscayl-ncnn/releases/download/20251207-174704/upscayl-bin-20251207-174704-linux.zip) | 3,952,825 | `a9fab3c770b62f2b7a35d8d6d61eb4e8b3aef79128b665c919d080b85a2292f2` |
| [Upscayl macOS 20251207-174704](https://github.com/upscayl/upscayl-ncnn/releases/download/20251207-174704/upscayl-bin-20251207-174704-macos.zip) | 9,338,240 | `277419791281a56eae0c739c70120b974d7267cf7c2de8e86dc09798d4b314db` |
| [Real-ESRGAN model distribution 20220424](https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesrgan-ncnn-vulkan-20220424-ubuntu.zip) | 46,931,474 | `e5aa6eb131234b87c0c51f82b89390f5e3e642b7b70f2b9bbe95b6a285a40c96` |
| [RIFE Linux 20221029](https://github.com/nihui/rife-ncnn-vulkan/releases/download/20221029/rife-ncnn-vulkan-20221029-ubuntu.zip) | 431,302,796 | `1e2c7ee7fa7daa326542d50622f0afedc80cf6f1858bda411d16385ffa5cdf68` |
| [RIFE macOS 20221029](https://github.com/nihui/rife-ncnn-vulkan/releases/download/20221029/rife-ncnn-vulkan-20221029-macos.zip) | 436,537,917 | `4a63a1f3c9c715773c57d2ee51df1b315ed20cd6c63103e45c483ecc4400b595` |
| [Real-ESRGAN BSD-3-Clause license](https://raw.githubusercontent.com/xinntao/Real-ESRGAN/685d429c81888252bdb10f56c7754baededc3823/LICENSE) | 1,519 | `4a699ec4863d96a91fc265948a0c90033f7e8735d515524dcf3444736406e0c2` |

GitHub supplies publisher SHA-256 digests for the two Upscayl ZIP assets;
both matched the acquired bytes. The older RIFE and model assets have no
publisher digest in their release metadata. Their catalog hashes are measured
full-download identities, not signatures or publisher attestations.
[The release records](../providers/managed/evidence/upstream-release-records.json)
retain asset IDs, exact URLs, publisher digests or their absence, release dates
and source revisions. [The complete inventories](../providers/managed/evidence/complete-asset-inventories.json)
retain every regular archive member's identity, including excluded files.
No acquired ZIP contains a symlink.

## Curated models and interface mapping

| Profile | CLI model name / directory | Installed parameter + weight pair | Scope |
| --- | --- | --- | --- |
| Upscayl anime | `-n realesr-animevideov3 -m models` | `models/realesr-animevideov3-x4.param`, `models/realesr-animevideov3-x4.bin` | Original video-oriented anime model; native `-z 4`; independent output `-s 2`, `3` or `4` |
| Upscayl general | `-n realesrgan-x4plus -m models` | `models/realesrgan-x4plus.param`, `models/realesrgan-x4plus.bin` | Original generic model; native `-z 4`; independent output `-s 2`, `3` or `4` |
| RIFE | `-m rife-v4.6` | `rife-v4.6/flownet.param`, `rife-v4.6/flownet.bin` | Serial midpoint `-s 0.5`; selected 2x CFR profile; no TTA or UHD claim |

The Upscayl executable archive contains no models. The curated models come
from the original authors' official Real-ESRGAN v0.2.5.0 model distribution,
not a local GUI installation. The catalog extracts only the selected pair;
the model archive's executable, demonstration media and other model pairs
are excluded. The separately named `upscayl-standard-4x` and `digital-art-4x`
models are not silently substituted for these identities.

The RIFE archive supplies the executable and models together. Only the
`rife-v4.6` pair is installed, plus the executable, license and README. Older
RIFE families with context/fusion/flownet companions are excluded. Both
platform archives contain identical selected model bytes. Their Git blob
identities also match the [pinned source model directory](https://github.com/nihui/rife-ncnn-vulkan/tree/a7532fc3f9f8f008cd6eecd6f2ffe2a9698e0cf7/models/rife-v4.6):
`3cb69d4ae09b085a2b5b51eb3c2e93745f805644` for the parameter and
`70de38ecc5b6f1b63afa56108bf76911f04ce630` for the weights.

Executable release source identities remain separate from interface
declarations. Upscayl 20251207-174704 resolves to
`d02fa88e078e7109fd689932453362430144014d`; it is not labeled as the earlier
`0beb39028a0ddd83250e845b4c3333c0675e3b97` interface source. The reviewed
`main.cpp` differs only in a progress message's percent escape; argument
parsing and model lookup are unchanged. The [source difference](../providers/managed/evidence/upscayl-interface-source.diff)
is retained. Release names such as `20251207-174704` are not semantic product
version observations. Neither tool offers a reliable version switch; `-v`
means verbose.

## Runtime layout and platform bounds

The exact acquired Mac executables were inspected as universal Mach-O files,
including all dylib-loading command families, RPATH commands and minimum OS
commands in both slices. [Upscayl](../providers/managed/evidence/upscayl-macho-load-commands.json)
and [RIFE](../providers/managed/evidence/rife-macho-load-commands.json) load
only Apple system frameworks and `/usr/lib` system libraries. Neither has
an `@rpath`, `@executable_path`, external MoltenVK or external OpenMP dylib
dependency. Thus this selected distribution needs no companion dylib copied
beside its staged executable. This observation does not extend to arbitrary
GUI-bundled or user-supplied binaries.

| Selected package | Observed platform floor | Remaining host requirement |
| --- | --- | --- |
| Upscayl macOS x86_64 | macOS 13.0 | Compatible Apple graphics stack and native runtime qualification |
| Upscayl macOS aarch64 | macOS 14.0 | Compatible Apple graphics stack and native runtime qualification |
| RIFE macOS, both architectures | macOS 11.0 | Compatible Apple graphics stack or explicit CPU selection and native runtime qualification |
| Upscayl Linux x86_64 | ELF symbol requirements include `GLIBC_2.35`, `GOMP_4.0` | System Vulkan loader, OpenMP/system runtimes and a usable Vulkan device/driver |
| RIFE Linux x86_64 | ELF symbol requirements include `GLIBC_2.15`, `GLIBCXX_3.4.21`, `GOMP_4.0` | System Vulkan loader, C/C++/OpenMP runtimes; compatible Vulkan device/driver for the selected GPU backend |

[Linux ELF dependency captures](../providers/managed/evidence/upscayl-elf-dynamic.txt)
and the [RIFE capture](../providers/managed/evidence/rife-elf-dynamic.txt)
retain complete `DT_NEEDED`, symbol versions and publisher build-directory
RUNPATH values. The RUNPATH values are not installed runtime libraries.
Managed assets do not install system libraries or GPU drivers. Musl Linux,
Linux aarch64, Windows and older macOS rows have no selected, verified package
in this catalog revision; they are not claimed available or qualified.

Upscayl rejects negative GPU IDs and has no CPU mode in the selected source.
RIFE documents and implements explicit `-g -1` CPU selection; it is a separate
selection, not an automatic fallback from the catalog's Vulkan backend.

## License and execution evidence

The catalog retains the Upscayl archive's AGPL-3.0 text, the immutable original
Real-ESRGAN BSD-3-Clause license alongside each curated Upscayl pair, and the
RIFE archive's MIT license. Source origins are included in every installed
package declaration. The RIFE distribution supplies no independent
weight-specific license notice; its packaged models and repository license
are recorded as published. No training-data, output ownership or other
rights claim is inferred from a model filename or repository license.

Both acquired Linux executables were invoked with only `-h`, a 10-second
timeout and captured stdout/stderr, executable SHA-256 and status. They loaded
and emitted the required options. Both return 255 for help, reflecting their
upstream `return -1` behavior, so a help diagnostic must not require status
zero. [The captures](../providers/managed/evidence/linux-help-diagnostics.json)
are diagnostic evidence only. No model inference, native Mac execution,
Vulkan-device qualification, perceptual-quality comparison or media
publication is claimed by this catalog evidence.
