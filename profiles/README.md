# Music-video setup profile

`music-video-v1.json` is a candidate setup policy, not a qualified distribution
or executable pipeline. Its FFmpeg version range and platform list are declared
targets; no target has been tested for this checkpoint. Source package revisions
are reviewed upstream references, not assertions about installed binaries.

The two inventory examples deliberately contain nonexistent example paths,
all-zero digest placeholders and no version/build/hardware observations. They
cannot establish readiness. Replace every selected artifact with the absolute
path and SHA-256 of an independently prepared, reviewed local file. Record
digest-bound observations with their actual source; do not turn profile
requirements into claimed observations by copying them.

The default `core-media` selection requires FFmpeg and ffprobe only. Explicit
capability selections replace the defaults; include `core-media` alongside
`upscale` when both are needed. `metadata` uses only ffprobe. Visible-repair
profiles select different tools and never substitute one for another.

For `upscale`, replace `REPLACE_WITH_REVIEWED_MODEL_NAME`, select both exact
`.param`/`.bin` paths, record their digests and revision, and independently
record their native scale. Keep the profile's native/requested scale and mode
consistent with the selected model. The profile selects no model origin or
license. Model directories bundled inside a macOS application are suggestions
for manual review, not trusted discoveries. No files are copied automatically.

For Node-based `gwr`, pinning its launcher does not identify the Node runtime,
package files or native `sharp` dependencies. Required feature observations
describe the prepared environment; complete transitive identity enforcement
and adapter qualification remain #38 follow-up. Missing upstream version flags
cannot be replaced with invented successful `--version` evidence.

See [the guide](../docs/toolchain-profiles.md) for CLI examples, evidence meanings,
candidate setup recipes, the explicit Pipeline v3 adapter boundary and pending
qualification. No binary, model or third-party package is distributed here.

`probes-core.example.json` configures the separate explicit probe operation.
Its paths and hashes are placeholders and must be replaced. `toolchain probe`
runs the selected programs; offline doctor/plan never invoke it automatically.
See [bounded probes](../docs/toolchain-probes.md). No probe or fixture has been
executed for this authored checkpoint.

## Selected frame-processing setup

`frame-processing-v1.json` is a separate candidate for the typed frame adapters;
the existing `music-video-v1.json` policy remains unchanged. Its default
`frame-decode` selection requires only FFmpeg. Optional `allenk-repair` and
`upscayl-recovery` selections include their decoder directly, so they do not
require ffprobe or an unrelated encoder, repair tool or neural model.

The two `media-setup-*.example.json` files embed this profile and a deliberately
unready inventory. Their paths are placeholders, every expected digest is zero,
and no package, version, flag, feature, model-scale or hardware observation is
supplied. They cannot establish readiness. The allenk example declares a macOS
Apple Silicon candidate; the Upscayl example declares Linux x86_64. Use the
actual local platform and independently reviewed file identities before preparing
setup. Neither platform is qualified by these examples.

```bash
aniflow toolchain prepare-media \
  --configuration "profiles/media-setup-allenk.example.json"

aniflow --output json toolchain prepare-media \
  --configuration "profiles/media-setup-upscayl.example.json"
```

These commands request an inert, reviewable preparation result; they do not
install tools, run probes, process frames or register providers. The example
requests are expected to report missing or unverified dependencies. Use the
selected capability's `effective_settings` as the sole options authority. The
request's `selection` maps only the adapter kind and dependency IDs.

For allenk, choose `current_v2` or `legacy_v1` explicitly and retain the matching
required flag (`--no-legacy` or `--legacy`). The threshold is an integer percent.
Only the pinned 0.3.2 basic image interface is selected; denoising, force and
automatic cross-profile fallback are omitted. Its executable identity binds
compiled profile assets under the selected interface, not its complete host
runtime or source-build authenticity.

For Upscayl, replace `REPLACE_WITH_REVIEWED_MODEL_NAME` and both model locators.
Record each actual file digest, the same reviewed pair revision and native 4×
scale separately. Keep `output_scale` and the capability's `scale.requested`
equal; use `native` mode at 4× or `post_resize` at 2×/3×. Tile size starts at 32,
the GPU ID is numeric, and workers are fixed at `1:1:1`. If selecting TTA, retain
`-x` in the required flags. The pinned CLI has no version query: provide an
honest digest-bound version/package observation rather than inventing a probe.
Model provenance/terms and Vulkan device evidence remain separate requirements;
copying profile requirements into observations does not establish them.

Preparation readiness is consistency of the selected declarations and current
local bytes. It is not native, GPU, visual-quality or redistribution qualification.
Ordinary processing still uses the existing exact provider configuration, locks
and runtime checks. See the [selected setup guide](../docs/toolchain-media-setup.md)
for the public library and result contract.
