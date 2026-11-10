# Prepare a selected frame-processing configuration

Use `toolchain prepare-media` to turn an explicitly selected repair or Upscayl
profile and local dependency inventory into a configuration for the existing
video commands. Preparation checks bounded local file identities and returns
reviewable JSON. It does not process source media, launch a tool, write setup
files, register providers, install packages or download models.

This #38 checkpoint is authored source. Tests, builds, lint, schema checks,
native/platform execution and hosted qualification remain unrun under
[#64](https://github.com/egohygiene/aniflow/issues/64). A ready preparation means
that its supplied declarations and current local bytes agree with this mapping;
it does not mean the native tool, model or hardware has been qualified.

## Choose one typed mapping

Start with the matching synthetic request example:

| Selection | Request example | Prepared configuration | Consumer |
| --- | --- | --- | --- |
| `allenk_image_repair` | [allenk setup](../profiles/media-setup-allenk.example.json) | `aniflow.allenk-frame-repair-configuration/v1` | `video repair-frames` |
| `upscayl_recovery` | [Upscayl setup](../profiles/media-setup-upscayl.example.json) | `aniflow.upscayl-recovery-configuration/v1` | `video upscale-frames` |

The examples contain placeholder paths, pins and observations. Replace them
with reviewed evidence for the exact local files; the examples are not ready
installations. The [frame-processing profile](../profiles/frame-processing-v1.json)
keeps optional repair and upscale choices separate from its default decoder
inspection. The broader [music-video profile](../profiles/music-video-v1.json)
retains its existing inventory semantics; this checkpoint does not silently
reinterpret its older settings as a typed adapter configuration.

A request contains `schema`, `profile`, `inventory`, `capability_id` and
`selection`. The selection names one adapter kind and the dependency IDs to
use. Repair requires a distinct FFmpeg decoder and allenk executable. Upscayl
requires a distinct decoder, Upscayl executable, model parameter file and model
weights file. The selected capability must declare exactly those dependencies
with the correct tool/model kinds and matching effects, backend and scale.
There is no capability-name guessing or ambient executable lookup.

Keep operation options in the selected capability's `effective_settings`.
They are decoded into closed typed options; unknown or unsupported settings are
refused. Every setup option is explicit; this request supplies no hidden defaults.
Do not repeat those options in `selection` or manually insert executable
pins into them. Preparation derives pins from the explicitly mapped inventory.

| Mapping | Effective settings |
| --- | --- |
| Both | Import timeout and source/frame/total-frame/pixel admission limits. |
| allenk | Explicit `current_v2` or `legacy_v1`, integer detection threshold, and maximum working bytes. |
| Upscayl | Model name, native and requested scales, tile size, numeric GPU ID, fixed worker string, TTA choice, and output/working/free-space bounds. |

The selected allenk interface is version `0.3.2` at
`7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e`. It emits the fixed basic-image
arguments with the selected profile flag and no denoise option, forced edit or
automatic cross-profile fallback. Upscayl uses the interface at
`0beb39028a0ddd83250e845b4c3333c0675e3b97`. That CLI has no version query;
its exact version is separate caller-supplied package evidence bound to the
executable digest, never an invented probe result.

Upscayl's model-native scale is 4. Requested output scale is independently 2,
3 or 4; the profile uses `post_resize` for 2/3 and `native` for 4. Model names
that cause the pinned native interface to infer a different scale are refused.
The parameter and weights files remain independently pinned and must carry
matching supplied revisions and native-scale observations. Their origin,
revision and terms are supplied evidence, not authenticated by hashing.

## Prepare and review

After reviewing the request and preparing its dependencies independently:

```bash
aniflow --output json toolchain prepare-media \
  --configuration "media-setup.json"
```

Omit `--output json` for human-readable facts and diagnostics. The machine
command is `toolchain_prepare_media`. Invalid request structure or an ambiguous
mapping fails as a configuration error. A valid request with missing,
incompatible or unverified dependencies produces a preparation result with
`ready: false`, diagnostics and no files; the machine envelope reports a
dependency failure while retaining that result.

The result includes the canonical request digest and the existing toolchain
inspection report. That report separates installed/missing/incompatible/
unverified facts and supplies remediation actions and declared locators.
Preparation also checks adapter-specific requirements that a permissive custom
profile cannot waive. Exact local executable files, digest-bound version and
package observations, model scale, selected flags/features and declared backend
readiness must agree. Unselected dependencies are not read or made prerequisites.

A ready result contains one `files` entry named `configuration.json`, with
`content` and its canonical JSON `sha256`. Review that content and save it to a
path you choose. Save the content value itself, not the preparation envelope.
Preparation never writes the file or overwrites an existing configuration. The
canonical digest identifies JSON content; pretty-printing changes raw file bytes
without changing that canonical identity.

Use the saved file with the existing planner. For selected repair:

```bash
aniflow --output json video repair-frames \
  --source-video "/absolute/source video.mp4" \
  --frames-directory "/absolute/frames" \
  --frame-index "/absolute/frame-index.json" \
  --configuration "/absolute/prepared-repair.json" \
  --range "1:299" \
  --plan
```

For upscaling, use `video upscale-frames` with the corresponding saved
configuration and the same three source arguments, without `--range`. Follow
[repair planning and resume](frame-repair.md) or
[Upscayl planning and resume](upscayl-recovery.md) for explicit execution.
Setup preparation neither reads these source paths nor creates their provider
registrations. The existing facade plans the source-bound stages and exact locks.

A retained ready result is a preparation-time observation. Reprepare after
changing dependencies, settings or host evidence. The subsequent plan and
run/resume paths retain their own fresh executable/model checks, source
admission, exact locks, immutable publication and checkpoint acceptance. Do not
substitute a saved setup report for those authorities.

## Evidence, bounds and support

`native_qualification` is always `false` and `host_runtime_evidence` remains
`unobserved`. Executable identity binds the selected bytes and, for allenk,
compiled profile assets under the chosen interface. It does not establish a
complete shared-library, operating-system or device identity. Supplied Vulkan
availability is not an observed successful model run or a memory-safety promise.

Requests are bounded to one MiB and preparation results to eight MiB. The
shared selected-dependency inspection has a two-GiB aggregate hashing budget;
individual files retain their inventory bounds. Selected roles use distinct
canonical paths without symlink ancestors. Executables must also satisfy
the existing frame adapters' 128-MiB tool bound. Existing typed configuration
validators retain the processing and resource limits; setup adds no resource
runtime, disk reservation or memory quota.

The examples describe candidate Linux x86_64 and macOS Apple Silicon setups.
The declared inventory host must match the current preparation host. Actual
platform, package, model, codec/backend and visual qualification remains open. This operation adds no redistribution: independently prepared executable
code and model/weight terms remain separate under [THIRD_PARTY.md](../THIRD_PARTY.md).

The public library exposes `MediaSetupRequest`, `MediaSetupSelection`, the
closed option types, `MediaSetupPreparation` and `prepare_media_setup` through
`aniflow::toolchain`. Request loading rejects duplicate and unknown fields;
canonical serialization binds the complete request. See the
[caller schema](contracts/toolchain-media-setup-v1.schema.json) and
[result schema](contracts/toolchain-media-setup-preparation-v1.schema.json).

Further #38 work includes other typed adapters, optional live tool/device
probes, reviewable model-locator discovery, complete runtime identity and
native/platform qualification. The existing audio registration/preflight path,
core inspection and ordinary offline processing remain independently usable.

## Caption finishing readiness (#38)

Select `caption_rendering` with distinct `ffmpeg_dependency_id` and
`ffprobe_dependency_id` tool roles. The selected capability's closed
`CaptionSetupOptions` contains `media` (all existing assembly bounds/CRF/preset),
explicit `mode`, pinned local `font`, `style`, and `preview_cue_indices`. The
result is the existing `caption-configuration/v1`, suitable for explicit caption
requests; preparation does not select source media or review declarations.

Fresh offline dependency inspection checks actual local pins. Typed mapping
requires attributed exact 8.0.1 versions, interface flags and features for native
observation/preview and selected delivery. Both modes need drawtext/select/PNG
for actual-frame review; soft mux additionally needs SRT/mov_text, while burn-in
needs libx264/color options. The font stays an explicit local asset, rather than
an executable or neural-model dependency. Existing bounded caption-font admission
checks canonical nonsymlink bytes/digest and one TTF/OTF header. Missing/changed
font, unavailable filter/encoder declarations, incompatible tools or settings
produce unready diagnostics and no configuration file.

Build/version/feature declarations are caller-attributed, not executed probes.
Font headers/digests do not prove glyph coverage, shaping, license or readability;
host/link closure and native qualification remain separate. No media read,
process launch, PATH/model/font discovery, writes, registration or downloads.
The preparation schema also includes the previously implemented RIFE result
variant. Four new synthetic cases are authored, uncompiled/unrun; all checks,
native execution and CI remain deferred to flow #88, scheduled last.

## Destination video readiness (#38)

Select `destination_video` with explicit `ffmpeg_dependency_id` and
`ffprobe_dependency_id`. `DestinationVideoSetupOptions` is the selected
capability's closed effective-settings type: existing `media` bounds/CRF/preset,
`geometry`, `allow_upscale`, dated destination `profile`/`evaluated_on`, and the
explicit descriptive `metadata` policy. The existing public preparation API and
machine CLI return one inert `aniflow.destination-video-configuration/v1`
configuration only when inventory, feature declarations and typed options agree.

Exact FFmpeg/ffprobe 8.0.1 pins are derived from the two selected roles. The CPU
profile needs H.264/FFV1/AAC decoding, libx264 encoding, MP4/null output, exact
probe/frame/packet/hash interfaces, scale/setsar/format and opaque yuv420p SDR.
Explicit padded geometry additionally needs `filter:pad`. Audio packets remain
copied, so AAC encoding is not a required capability. No font or neural model is
selected. Unsupported versions/options/backends/scales, missing features or
changed tool files produce no configuration. No PATH lookup, tool launch,
source-media read, setup write, registration, installation or download occurs.

Supplied build features and platform inventory are declarations, not actual
native/hardware qualification. Source/cases are authored, uncompiled/unrun;
all tests/builds/checks/native/CLI/SDK/CI remain deferred to flow #88, last.
Known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Broader #38 platform/distribution/qualification scope stays open.

## Publishing prerequisites (#38)

The same inert `prepare_media_setup` API accepts three additional selections:

- `final_video_audio_qc`: exact 8.0.1 FFmpeg/ffprobe, complete native float32 decode
  and the existing two-pass ebur128 meter, with `ebur128`, `ametadata`, `apad`,
  WAV and null-output features. Closed options are `media` (the existing complete
  `FinishingMediaOptions`) and `maximum_pcm_bytes` (1024..268435456). The inert
  `aniflow.final-video-audio-qc-setup/v1` document contains `configuration` and
  `maximum_pcm_bytes`; add the actual current source and provider only when
  making a final-QC request. No source or QC result is invented here.
- `metadata_remux`: `media`, explicit delivery-metadata `policy`, and `subtitles`
  (`none` or `preserve_accepted_soft_subtitle`). The latter requires mov_text
  decoding, SRT text extraction and the exact subtitle disposition flag. Output
  is the existing assembly settings with `delivery_metadata`. The actual native
  remux separately requires accepted caption evidence and exact track binding.
- `lossless_audio_export`: `format` (`wav`/`flac`),
  `tool_timeout_milliseconds`, `maximum_tool_output_bytes`,
  `maximum_output_bytes`, `maximum_working_bytes`, `minimum_free_bytes`, and
  `metadata_policy: remove_inherited_descriptive_metadata`. Output is the
  existing lossless configuration. FLAC stays integer-only; readiness does not
  convert a float master, choose quantization or accept source media.

Each selection names exact `ffmpeg_dependency_id` and `ffprobe_dependency_id`
roles. Profile dependencies must equal those roles, with the existing precise
filesystem read/write, environment-read and subprocess side effects. There is
no AI/GPU/model backend for these three mappings. Required build observations
are supplied, digest-bound declarations; file identity is independently current.
Missing features or files produce diagnostics and no configuration for that
selection. No flags are probed and no executable is launched by preparation.

`toolchain::PublishingSetupRequest` (`aniflow.toolchain.publishing-setup/v1`)
composes one shared `profile`/`inventory` and 1..8 explicit `selections`, each
containing a distinct existing `capability_id` and typed `selection`. It accepts
publishing mappings only: destination video, optional captions, final video
QC, metadata remux and lossless export. It neither infers omitted steps nor
runs a recipe. Source/frame recovery retains its separate existing preparation.

```bash
aniflow --output json toolchain prepare-publishing --configuration publishing-setup.json
```

The SDK `prepare_publishing_setup` and CLI return per-selection preparations,
a combined `ready` flag and attributed diagnostics. A successful report command
can return `ready: false`; inspect the data before using returned configuration.
Every explicitly selected prerequisite must be ready. Each returned
`configuration.json` is scoped to its step; the command writes none of them.
The request and report have finite one/eight MiB bounds. Native, provider,
source-media, platform and musical-review qualification remains unobserved;
there is no publication authority or installation/download action.

Six SDK refusal/requirement cases and one CLI parser case are authored,
uncompiled/unrun. All execution and qualification remain deferred to flow #88,
scheduled last. Earlier media-setup serialized meanings remain intact.

[Core extraction and clean master/delivery assembly](core-media-setup.md) have
separate explicit mappings and a bounded `prepare-core` composition. Existing
publishing preparation retains its own scope; all native qualification is unrun.

## One selected workflow readiness report

`toolchain prepare-workflow --configuration workflow.json` and public
`prepare_workflow_setup` compose 1..11 distinct existing domain mappings using
`aniflow.toolchain.workflow-setup/v1` (`profile`, `inventory`, `selections`). Each
selection has an explicit capability ID and existing typed selection. Frame
extraction, repair, upscale, interpolation, clean assembly, captions, metadata,
destination video, lossless audio and both final-QC kinds retain their individual
configuration and diagnostic owners. No omitted role is implicitly enabled.

The bounded report lists each selected preparation and every omitted role. Whole
readiness requires all selected roles ready and a second observation with the
same exact per-role preparation. Unselected models are not loaded or inspected.
Selection order is presentation, not a temporal execution plan. Request/report
bounds remain 1/8 MiB; there are no source-media, process, installation,
configuration-write or registration ports. Core and publishing setup contracts
retain their prior scopes and semantics. Four SDK cases and one CLI case are
authored, uncompiled/unrun. No native/platform qualification is inferred; all
standing execution/CI deferrals remain and flow #88 stays last.
