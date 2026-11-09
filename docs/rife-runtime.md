# Fixed RIFE runtime facade

The [RIFE CLI](rife-cli.md) delivers plan/run/resume/status over this same facade
without another runtime or implicit component selection.

The #110 source checkpoint adds `video_interpolation::runtime::{plan,run,resume,
status}` with `RifeRunRequest`. The existing pure `video_interpolation::plan`
API and provider ABI remain unchanged. One checked-in fixed v3 pipeline binds
the original frame directory/index to the existing serial RIFE capability,
producing `interpolated_frames` and `interpolated_index` with ordinary runtime
integrity, checkpoint and delivery acceptance. No second execution/recovery
engine or per-pair checkpoint database is added.

Planning checks exact canonical regular inputs, directory membership, indexed
PNG structure/hash/geometry, pinned tool and model companions, provider binary
and config/manifest identities. The existing native provider's storage admission
and frame-byte policy are reused, including predicted output-index bounds.
Initial scope is at most 50,000 original / 100,000 derived frames. Planning
creates no workspace and launches no tool. CPU/device and available-RAM values
remain explicit supplied declarations, not fresh GPU/RSS/host qualification or
an OS-enforced quota. Disabled/unsupported profiles refuse explicitly.

`run` selects a fresh output root; the recipe-only `run_at` seam requires its
exact preplanned child. `resume` rebuilds current original frame/index/tool/model/
provider/configuration bindings and compares the exact retained plan before
entering ordinary v3 recovery. Loose or incompletely initialized directories
cannot authorize adoption. A complete accepted stage can be reused; an explicit
resume may retry an unaccepted whole stage under existing runtime attempt rules.
Individual partial pairs are not adopted. `status` is read-only and checks only
the named fixed facade profile plus native current state, without rerunning RIFE.

The provider retains its existing serial midpoint policy, cut/endpoint holds,
original-versus-derived clocks and explicit device selection. Indexed temporal
source identity remains supplied input evidence, not independent proof that
these PNGs came from a compressed video. Audio is not processed. Existing
accepted-RIFE assembly binding remains responsible for actual native acceptance
and exact clock/source joins; later assembly performs full image/media checks.
Integrity acceptance alone does not qualify motion quality, native fidelity,
decoder behavior, GPU/memory safety or final delivery/publication.

Four focused facade regressions are authored, uncompiled/unrun, reusing the
existing synthetic provider fixture. No tests/builds, dependency resolution,
validators, provider/model/media/CLI/native execution or CI ran. #110/#64 retain
native/platform/resource/visual qualification; the known #64 alignment
cancellation/start-marker failure remains unresolved, cause unproven.
