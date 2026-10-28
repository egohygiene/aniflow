# #35 source-frame extraction checkpoint

## Source and authorization boundary

The maintainer authorized merging the pending Aniflow stack and continuing
#35 on October 28, 2026. The three source checkpoints merged in dependency
order, retaining their reviewed heads:

| PR | Saved head | Merge commit |
| --- | --- | --- |
| [#130](https://github.com/egohygiene/aniflow/pull/130), verified assembly | `1893bfb994d79e75b08d32739ed61aa23e58cde9` | `41d1706f5484218df4ac8f8e0a94548976ef8869` |
| [#131](https://github.com/egohygiene/aniflow/pull/131), fixed recipe | `31352771bcb6e6d043bf1f19bd57b87be3853e20` | `822c6582a8bdcf410bff0d1f1b755618249ba978` |
| [#132](https://github.com/egohygiene/aniflow/pull/132), target resolution | `94887d45ecd18f9fc760fa5119e13cfb627b9f3e` | `e1634df58fa32feda23021565b032a83c5a30d66` |

Main is `e1634df58fa32feda23021565b032a83c5a30d66`; its tree
`f70ffe58fc2b225c498feffc566baaff70203f36` matches the saved #132 tree.
Branch `codex/35-source-frame-extraction` is published as
[draft PR #133](https://github.com/egohygiene/aniflow/pull/133), created on
2026-10-28 at 05:21:00Z and targeting unchanged merged main.

First saved head: `0bb6d72b134b03c81f92aedaf93384d8f0c1eb70`; tree:
`5ec59b2667c1d7ce936414ce7d94579f791c3876`. All 27 published blobs and the
exact tree matched the reviewed local source. Candidate revision remains null
rather than a circular self-pin; these dated first-save receipts do not pin
later documentation changes. The assembly, recipe and target-resolution handoffs
retain their dated publication, source-review and unrun-test receipts.
Merge establishes source inclusion, not execution qualification or ADR approval.

## Selected checkpoint

The public `frame_extraction` facade and thin `video extract-frames` command
add one ordinary v3 stage from an original source video to a complete PNG
directory, original `aniflow.rife-frame-index/v1` index and extraction report.
The accepted `extracted_frames` and `extracted_index` artifacts are explicit
inputs to the unchanged indexed music-video recipe. Actual paths come from
accepted v3 outputs rather than guessed filesystem locations.

Plan binds current source and executable identities and closed settings
without launching tools or inventing geometry, timing or frame count. Native
run observes the source before extraction. It reuses the existing strict
FFmpeg/ffprobe 8.0.1 original-clock assembly profile and its rational temporal
owner. Unsupported profiles are refused; extraction does not expand media,
audio-tail, color, rotation or derived-clock acceptance.

The selected source conversion is explicit full-range RGB24/BT.709. Complete
PNG decode observations are compared with source-frame RGB observations at
the original rational clocks, then recorded in the existing original index.
The report's observed source relationship is narrower than historical
authenticity or past processing. Lossless PNG storage does not make a
YUV-to-RGB round trip mathematically lossless. No native or visual-quality
qualification is implied.

`cut_before_ordinals` is empty because extraction performs no shot-cut
detection. It is not a no-cuts finding or authorization to interpolate across
cuts. The supported downstream handoff here is original-clock repair,
upscale and assembly.

Existing Pipeline v3 owns source snapshots, exact locks, output validation,
acceptance, writer control, retained failure evidence and compatible whole-stage
resume. There is no second execution engine, generic workflow, checkpoint
database or per-frame partial-output adoption. A completed compatible stage
can be reused; interrupted extraction retries the stage. Original source,
extraction outputs and downstream recipe outputs remain separate.

Initial storage admission covers source/tool copies and bounded observations.
After native geometry/count observation, the conservative per-frame PNG
allowance is `2 * (3 * width * height + height) + 65,536` bytes. Both configured
frame/aggregate ceilings and working/free-space admission must cover it,
including private and final frame copies. This can refuse highly compressible
inputs; it is not a predicted compression ratio, disk reservation or RSS quota.

The recipe's existing input/configuration/intent contracts and fixed child
order are unchanged. Source-only one-command recipe composition remains a
later #35 integration. ADR-0012 stays proposed; standalone extraction does not
silently extend its recipe-phase decision.

## Review and deferred qualification

Root source review and independent API/runtime and native/schema reviews found
no remaining concrete blocker. Thirteen new regressions are authored and
**unrun**: seven public facade cases, three CLI parser cases, two native
argument/conversion cases and one PNG cICP/gamma case, plus stable machine-name
mapping coverage. Source review does not establish compilation or execution;
no passed-check result is claimed. Publication records the reviewed source;
it does not qualify execution.

Review corrected runtime artifact file/byte limits to account for simultaneous
private and final frame copies. Storage admission now uses bounded initial
source/tool observations, followed by a conservative envelope derived from
actual geometry/count, instead of reserving the configured aggregate ceiling
before probing. The shared PNG scanner admits only the pinned FFmpeg 8.0.1
BT.709 cICP tuple `[1,1,0,1]` with matching `gAMA` 50,994 when present, and
refuses conflicting tags or sRGB combinations. The native interface and gamma
mapping were checked against the pinned upstream
[`pngenc.c`](https://github.com/FFmpeg/FFmpeg/blob/n8.0.1/libavcodec/pngenc.c) and
[`csp.c`](https://github.com/FFmpeg/FFmpeg/blob/n8.0.1/libavutil/csp.c) source;
that source comparison is not native qualification.

All tests, compiler/build, dependency resolution/installation, lint/format,
schema/naming/drift/docs validators, smoke/package, CLI/provider/native/media/model
execution and hosted qualification remain deferred under #64. No Actions are
dispatched or polled. Publication retains disabled hooks and `[skip ci]`.
The known alignment cancellation/start-marker failure remains unresolved;
its cause is unproven. Source package 0.3.0 remains unreleased.

Later qualification needs redistribution-safe generated video fixtures,
complete source/PNG decoded identity and timing observations, exact settings
and tool pins, unsupported-profile refusals, interruption and compatible
whole-stage reuse, and the extraction-to-recipe/assembly handoff. This source
checkpoint does not supply those executed results or authorize private media.

## Remaining work

#35 stays open. Next is reviewing saved PR #133 and syncing before one-command
source-entry recipe composition; that next slice has not started.
Broader media/audio-tail profiles, loudness/metadata and complete
workflow qualification also remain open. #111 retains
qualification and broader recipe entry forms. #107 retains manual-artifact and
review composition; #36/#37/#38/#110 retain their own remaining contracts.
Destination copies #83 and optional exact-byte proofs #82 remain separate.
No new release or parent issue closeout is claimed.

See the [extraction guide](../video-frame-extraction.md),
[fixed recipe](../music-video-recipe.md) and
[current roadmap](../../ROADMAP.md#current-source-and-work-order).
