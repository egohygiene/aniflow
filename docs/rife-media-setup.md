# Explicit offline RIFE media setup

Source checkpoint for #38. The existing `toolchain setup --config ...` delivery
accepts a new closed `selection.kind: rife_interpolation` with explicit
`processor_dependency_id`, `param_dependency_id`, `weights_dependency_id`.
The selected capability's `effective_settings` contains `host` (operating system,
architecture, total/available memory MiB), `preset`, `device_id`, pair timeout,
capture byte limit, frame byte limit and total artifact byte limit. These are
caller declarations, not detected memory reservations or device qualification.

The three mapped roles must exactly match the selected dependencies. Offline
inspection checks pinned bytes; the mapper requires attributed tool/model
revisions, supplied command flags, model-loading/backend features and exact
`rife-v4.6/flownet.param` plus `flownet.bin` companions. Extra/missing files,
changed bytes, symlink paths and mismatched companion revisions refuse readiness.
The resulting inert `configuration.json` uses the existing RIFE configuration
and validation owner; ordinary planning/provider binding stays authoritative.

Device `-1` selects CPU explicitly, requires CPU feature evidence, forbids a
GPU backend and keeps the AI effect. Nonnegative device selects one explicit
Vulkan device and requires ncnn-vulkan capability/hardware declarations plus the
GPU effect. Spatial `scale` is refused: the fixed midpoint 2x clock is temporal.
No host/backend fallback, PATH search, install/download, tool launch, provider
registration or source-media read is performed. Supplied version observations
do not become upstream version-probe evidence. Ready still means unqualified
preparation, with host runtime evidence unobserved.

Three synthetic mapping regressions are authored, uncompiled/unrun. No tests,
builds, validators, installs, native/device/model execution or CI ran; #38/#64
retain broader mapping, hardware and platform qualification work. Known #64
alignment cancellation failure remains unresolved with cause unproven.
