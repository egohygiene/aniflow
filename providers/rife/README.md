# Serial RIFE frame provider

This optional first profile wraps explicitly pinned `rife-ncnn-vulkan` bytes and
the `rife-v4.6` model for serial midpoint-only 2x frame interpolation. It consumes
an immutable PNG directory and an explicit temporal/frame index, then emits a
new PNG directory and output lineage index. Direct MP4 input/output, audio remux,
general target rates, automatic cut detection and native qualification remain
outside this checkpoint.

- `configuration.schema.json` describes the caller configuration, disabled by
  default. Host observations and resource admission are explicit.
- `provider-configuration.schema.json` wraps an enabled caller configuration for
  the existing provider ABI. Every effective settings field is required; caller
  defaults must be normalized before constructing the hashed provider envelope.
- `configuration.example.json` is a synthetic shape fixture. Its paths, digest
  markers, byte sizes, version and host values do not describe a real installation.

`RifeProviderConfiguration::manifest()` generates an inert manifest with the exact
raw provider-schema digest and CPU/Vulkan effect declarations for the selected
configuration. No static placeholder manifest is presented as executable setup.
Registration, invocation and Pipeline v3 acceptance remain separate operations.

One active pair, `1:1:1` threads, CPU device `-1`, disabled TTA/UHD flags and
conservative automatic admission are the first-profile defaults. The memory
estimate is not an enforced RSS/VRAM quota and does not guarantee crash prevention.
No tool, model, device or host is native-qualified by these contracts.

See [the user guide](../../docs/video-interpolation.md) and
[the provider authoring contract](../../docs/provider-authoring.md). Independent
schema coverage is authored in `tests/rife_schema.py`; checks, builds, tests and
native execution remain unrun under #64.
