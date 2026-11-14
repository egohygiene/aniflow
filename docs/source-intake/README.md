# Archived media trial scripts

The seven original trials remain on the archive branch at immutable revision
[`16aa7cec698a6de2c6e7945a751400d65e797c89`](https://github.com/egohygiene/aniflow/tree/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13).
They are reference sources, outside the installed product. Keep that branch;
this index does not copy, execute, install or silently adopt those scripts.

The dispositions below compare the complete sources with Aniflow main at
`2c6d6947d68608725df62380820c7218fb47df70`. “Covered” means an existing source
owner describes/implements that responsibility, not that its native behavior
has been qualified. Broader compilation, model, media, platform and release
acceptance remains under [#212](https://github.com/egohygiene/aniflow/issues/212)
and [Flow #88](https://github.com/egohygiene/flow/issues/88).

| Intake | Sources | Disposition |
| --- | --- | --- |
| [#228](https://github.com/egohygiene/aniflow/issues/228) | `resume-upscale.py`, `finish-video.py` | [Reuse recovery, assembly, metadata and final-file QC owners](upscaling-and-finishing.md). Preserve trial algorithms; do not add another finisher or loose-file checkpoint authority. |
| [#229](https://github.com/egohygiene/aniflow/issues/229) | `normalize-video`, `to-vr`, `check_deovr.sh` | [Retain optional playback references](quest-deovr-playback.md). No current Quest/DeoVR target or device qualification is claimed. |
| [#230](https://github.com/egohygiene/aniflow/issues/230) | `hls-download` | [Keep acquisition as external preparation](hls-acquisition.md). Feed only a completed local source into the existing source-entry workflow. |
| [#231](https://github.com/egohygiene/aniflow/issues/231) | `watermark-lab.py` | [Retain a separate optional experiment](watermark-lab.md). Keep structural, quality and detector evidence distinct. |

## Exact source identities

Every size and SHA-256 below was recomputed from `git show` bytes and compared
with the archived [manifest](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/manifest.json).
Git blob identities are also retained there. These hashes identify the supplied
source; they do not establish a test result, dependency right or adoption decision.

| Original source | Bytes | SHA-256 |
| --- | ---: | --- |
| [resume-upscale.py](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/resume-upscale.py) | 12002 | `061c21b31776a2fdd199417044d3724e71bb77cd83617f666f8c2af49733952c` |
| [finish-video.py](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/finish-video.py) | 19514 | `9ca70a2e58e4402667145dad5818a9259a137834d4a959bbe1809ae33f6982ea` |
| [normalize-video](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/normalize-video) | 12175 | `0cf38c0e902812b06e02b42d8839ee92a4c2c31166a70afcca13fb262f9ce1e2` |
| [to-vr](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/to-vr) | 2227 | `3e4145a294acb31655f13c0f0f66cba2cb736458950182f5063a8949c33c3f48` |
| [check_deovr.sh](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/check_deovr.sh) | 2984 | `27fe0482205bbe39b21fe58d89422943cccfcfb6e6d1384b9e1a2a964ae386b8` |
| [hls-download](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/hls-download) | 28219 | `34dfc031d45127ffbd571d6d3065a2ea9a280a049bce8a55ddca5287477ed1b6` |
| [watermark-lab.py](https://github.com/egohygiene/aniflow/blob/16aa7cec698a6de2c6e7945a751400d65e797c89/docs/source-intake/2026-11-13/watermark-lab.py) | 28038 | `c87543367c12af94b0f4436d12cf6cbee37430c8de254044e909a4afd7e659e2` |

## What this review establishes

All seven scripts were read end to end. Existing owner paths and local document
links were checked; original bytes and archive identities remain unchanged.
No archived entrypoint, `--help`, setup, model import, download, media process,
probe, playback device, remote oracle or provider was executed. No product
dependency, schema, source profile or release version changes in this intake.

Each intake issue explicitly allows a clear documented disposition instead of
a second implementation. The guides preserve concrete useful behavior, explain
why it is reused or kept external, and name a bounded follow-up when adopting it
would require a new supported profile. The follow-ups stay within #212; closing
an intake must not be interpreted as completing that parent's feature gates.
