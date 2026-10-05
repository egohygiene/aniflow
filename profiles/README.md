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
