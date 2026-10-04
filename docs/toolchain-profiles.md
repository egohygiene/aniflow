# Explicit music-video toolchain profiles

Issue #38 starts with an offline inventory and setup-planning checkpoint
(#97–#99). The public `aniflow::toolchain` API and `toolchain doctor` /
`toolchain plan` commands describe selected dependencies without launching them.
Checkpoint #101 adds a separately invoked [bounded probe operation](toolchain-probes.md)
for FFmpeg/ffprobe. This implementation and its synthetic coverage are authored; compiler, tests,
schema checks and native qualification remain unrun under #64.

The profile in `profiles/music-video-v1.json` is a reviewable starting point.
Its version requirements express a requested setup policy, not a tested
compatibility range. The inventory examples contain placeholder local paths
and synthetic identities; replace them with evidence from your own explicitly
prepared installation. Copying an example does not establish tool availability.

## Readiness and evidence

Tool existence, executable identity, supplied version/package revision, build
features, model files, backend and platform support are separate facts:

| State | Meaning |
| --- | --- |
| `installed` | The particular requirement matches the available evidence. |
| `missing` | A required locator or asset is absent. |
| `incompatible` | The available evidence conflicts with the declared requirement. |
| `unverified` | The evidence is absent, insufficient, or cannot establish the requirement. |

Local file digests are observed by reading explicitly named files. Versions,
supported flags/codecs and backend observations are caller-supplied evidence,
bound to the executable digest where applicable. They are not newly measured
by these commands. Inspect the evidence source for each fact; an `installed`
version fact is not a successful native version probe.

Supply literal absolute paths to the actual regular files, rather than symlink
launchers; spaces and shell characters remain path data. The Unix reader uses
no-follow, nonblocking opens and checks file identity around bounded reads.
Profile/inventory JSON is limited to one MiB, each artifact to its declared
limit (at most one GiB), and selected artifact hashing to two GiB per report.
These limits do not constitute a filesystem sandbox or a native execution test.

`ready` means the selected inventory is consistent with its profile. It does
not establish native execution, model quality, provider protocol compatibility
or a release qualification. Native qualification remains false. Keep the
profile, inventory and resulting report together when reviewing a setup.

Every selected artifact needs an attributed package/model revision observation
bound to its bytes. Model native scale also needs its own bound observation;
declaring a desired scale in the profile is insufficient. The inventory field
`executable_sha256` binds either a tool or model observation to that artifact's
bytes; it never implies that a model file is executable.

## Select only needed capabilities

Core media selects FFmpeg and ffprobe. Optional upscaling and visible-mark
repair require explicit capability selection. Metadata inspection reuses
ffprobe; it does not pull in Python, GPU runtimes or diffusion packages.
Capabilities are profile-local setup labels, not new Pipeline v3 capability
contracts. Request core explicitly alongside an optional selection when both
are needed.

```bash
aniflow toolchain plan \
  --profile "profiles/music-video-v1.json" \
  --inventory "profiles/inventory-linux.example.json"

aniflow --output json toolchain doctor \
  --profile "profiles/music-video-v1.json" \
  --inventory "profiles/inventory-linux.example.json" \
  --capability "core-media" \
  --capability "upscale"
```

Plan returns reviewable diagnostics and remediation even when the setup is
incomplete. Doctor reports a dependency failure when required facts are not
ready, retaining its report in the machine error result. Neither command
changes files, registers providers, installs packages, prepares caches,
searches `PATH`, probes executables or accesses the network.

## Prepare a host explicitly

These recipes describe preparation steps for candidate platforms. They are
not automated installers and have not been executed or qualified here.

| Preparation | Linux x86-64 | macOS Apple Silicon |
| --- | --- | --- |
| Core tools | Independently install a reviewed FFmpeg build using your package source; record its exact package revision and both executable paths/digests. | Independently install an arm64 FFmpeg build using your reviewed package source; record the exact revision and both paths/digests. |
| Build capabilities | Record the selected build's actual codecs/encoders and flags, tied to that binary digest. | Do the same for the installed arm64 build; package names alone do not prove codec support. |
| Optional Upscayl | Prepare a reviewed NCNN executable plus its required Vulkan runtime; locate the exact `.param` and `.bin` pair and record both hashes and model revision. | Prepare the reviewed arm64 executable and its required Vulkan/MoltenVK stack. An application-bundled model directory is only a locator suggestion. Explicitly review the selected files and terms. |
| Visible repair | Prepare the selected independently reviewed tool and dependencies, preserving its source/package revision. | Prepare the chosen tool's native architecture or runtime separately; upstream target listings do not prove this host works. |
| Review | Replace example identities and explicitly supply host/backend observations, then inspect plan/doctor output. | Replace example identities and explicitly supply host/backend observations, then inspect plan/doctor output. |

The separate [core probe operation](toolchain-probes.md) has authored Linux and
macOS process adapters. Its diagnostic evidence does not qualify these host
recipes or supply actual GPU/device observations. Native execution remains
unrun under #64.

Acquire assets and prepare caches as a separately authorized setup operation.
Ordinary processing must use local pinned dependencies and refuse missing or
incompatible ones; this checkpoint does not add a package manager. No command
in this guide was run as implementation evidence.

## Provider registration and exact execution locks

The setup report is not a Pipeline v3 registration or provider lock. Native
`ffmpeg`, `upscayl-bin`, `gwr` and GeminiWatermarkTool CLIs do not implement
`--aniflow-invocation` merely because their files exist. Use an independently
prepared compatible adapter with a closed manifest/configuration and explicitly
pass its registration to `plan-v3` / `run-v3`.

Keep actual adapter locators confined to the registration bundle as required
by `ProviderRegistrationDocument`. A discovered system path is a suggestion,
not a portable registration. Existing `ProviderRegistry` resolution remains
the selection and exact-lock authority. Bind package revision, backend,
effective flags/settings and requested/native model scales through the adapter's
closed effective configuration; retain tools/models in its component inventory.
This checkpoint does not change provider-lock v1 or add implicit fallbacks.

The Upscayl profile records native model scale separately from requested output
scale. Upstream `-z` and `-s` have different meanings. Existing Pipeline v2
adapters are unchanged; planning a profile does not silently alter their argv
or grant new native support. See #37 for recovery and adapter follow-up.

## Candidate compatibility and remaining work

| Capability | Dependency boundary | Checkpoint status |
| --- | --- | --- |
| Core inspection / extraction / assembly | Explicit FFmpeg and ffprobe; declared build features | Offline inventory checks authored; native qualification unrun. |
| Metadata inspection | Explicit ffprobe | No heavyweight optional stack; format coverage remains the tool/adapter's responsibility. |
| Upscaling | Independent Upscayl executable, local model pair, declared backend | Inventory/settings planning only; no model copied or launched. |
| Visible image repair with gwr | Independent Node CLI and its prepared environment | Optional; source package evidence does not prove runtime dependencies. |
| Visible image repair with GeminiWatermarkTool | Independently prepared image executable | Separately selected; exit semantics and adapter qualification remain outstanding. |
| Visible video repair | Separate upstream video implementation | No verified executable/adapter profile delivered here. |
| Invisible-mark cleanup | Separate experimental providers | Outside this checkpoint; #39 retains evaluation. |

Issue #38 remains open for additional tool/device probes and observation capture,
qualified adapter/registration materialization, processing-preflight integration,
model-location discovery and actual platform/codec/backend evidence. #64 owns
execution of the authored checks; #10 owns immutable release assets. The
existing alignment cancellation failure remains unchanged with cause unproven.

See [upstream evidence](toolchain-upstreams.md), [third-party notices](../THIRD_PARTY.md),
[provider authoring](provider-authoring.md) and the
[checkpoint handoff](validation/aniflow-38-checkpoint.md).
