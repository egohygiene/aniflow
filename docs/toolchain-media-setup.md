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
