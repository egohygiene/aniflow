# #35 fixed music-video recipe checkpoint

## Source and authorization boundary

Branch: `codex/35-music-video-recipe`, based on
`1893bfb994d79e75b08d32739ed61aa23e58cde9` from the open assembly
[draft PR #130](https://github.com/egohygiene/aniflow/pull/130), branch
`codex/35-verified-frame-assembly`. The recipe draft will target that branch;
publication metadata is pending. Main remains
`8cb00a323b1829de1e4b958236b6aa8529eb7045`. Neither assembly nor recipe is
merged. PR #130's first-save receipts and source-review observations remain in
its [dated handoff](aniflow-35-assembly-checkpoint.md); they are not a self-pin
or an execution result for this checkpoint.

The maintainer authorized continuation into the selected fixed recipe. All
tests, compiler/build checks, dependency resolution/installation, lint/format,
schema/naming/drift/docs validators, native/provider/CLI/media/model execution,
smoke/package and hosted qualification remain deferred under #64. No Actions
are dispatched or polled. Publication uses disabled hooks and `[skip ci]`.

## Authored boundary

The fixed sequence starts with an original source video and complete original
indexed PNG inputs. `prepare_frames` uses named allenk repair's existing
import/repair child, or a standalone immutable import when repair is disabled.
It then optionally invokes Upscayl recovery and invokes verified assembly. Optional steps are explicitly
enabled with an existing child configuration or disabled with a reason. No
missing setting implies a tool choice or fallback. This is the selected
music-video path, not an arbitrary stage graph or a general recipe interpreter.

Each enabled operation owns an ordinary child Pipeline v3 run through its
existing facade. Immutable recipe intent and fixed child-run paths bind the
requested composition and identify owned runs. They cannot mark media accepted,
replace child checkpoints or establish provider authority. Existing exact
plans, source/tool/configuration compatibility, output validation, immutable
checkpoints, writer ownership, cancellation and run-local resume remain owned
by the child runtime.

An exact downstream plan needs actual accepted predecessor frame bytes and
the corresponding updated index. Initial planning exposes pending downstream
work rather than inventing output hashes, claiming all steps planned or using
original frames in place of unproduced repaired/upscaled frames. Execution
resolves each next child after acceptance. Before the first pending native
child, it reuses the assembly owner's bounded actual-source profile observation;
that preflight is not a new accepted artifact. Planning, status and fully
accepted resume launch no native source probe. Reported aggregate progress and
status remain views over this fixed sequence and its child evidence. Status
still reads bounded recorded source/component identities for compatibility;
absence of explicit CLI configuration does not make it portable offline proof.
A child interrupted before required plan/manifest initialization is retained
and refused rather than overwritten. Abandoned writer ownership requires
explicit reconciliation, without automatic lock deletion based only on PID.

The [recipe guide](../music-video-recipe.md) connects explicit configuration,
planning, execution, status and resume. The individual repair/upscale/assembly
commands remain available. This checkpoint adds no extraction, arbitrary
provider dispatch, review/approval automation, nested process scheduler,
global cache or second media-checkpoint database. The proposed
[ADR-0012](../architecture/governance/decisions/ADR-0012-fixed-indexed-music-video-recipe.md)
records this narrow composition boundary; no ADR acceptance or historical #14
acceptance is inferred from implementation.

## Inherited support and evidence limits

The recipe does not widen any child's supported source, index, tool, model,
clock, pixel, audio or resource profile. Named repair retains its explicit
current/legacy image profile and whole-stage recovery. Upscayl retains explicit
per-frame or bounded-chunk policy and complete-set collection. Assembly retains
its FFmpeg/ffprobe 8.0.1 interface, original-clock progressive zero-origin CFR,
opaque 8-bit BT.709 SDR and optional original AAC-LC passthrough. It still
refuses unsupported offsets, long A/V tails, HDR, VFR, side data and derived
RIFE clocks. Source geometry remains separate from upscaled output geometry.

Assembly produces the FFV1/bgr0 MP4 master and H.264/yuv420p MP4 delivery with
its own decoded-pixel, exact-clock, AAC-packet and container observations.
Coordinating those operations does not upgrade declared PNG/source lineage,
unobserved historical processing, native backend qualification or creative
quality. Source assets remain immutable; retained child runs and private work
still require the existing disk, capture and temporary-volume admission.
No in-place replacement or automatic deletion of prior runs is introduced.

## Qualification and continuation

Synthetic regression coverage is authored with the implementation and remains
unrun. Source reading does not establish compilation, executable behavior or
passing conformance. When the maintainer lifts #64's execution deferral,
qualify configuration and explicit disabled reasons, honest pending-plan
reporting, exact child handoff, source/configuration/locator mutation,
interruption around child creation and completion, cancellation, resume,
output corruption and read-only status. Native qualification must separately
exercise the exact selected tool/model/build combinations and media subset;
synthetic provider artifacts cannot establish those properties.

Parent #35 remains open for remaining assembly acceptance, broader media
profiles and qualification. #107 retains broader manual/review recipes,
reviewed handoffs and other workflow scope. #110 retains RIFE clock/audio
handling; #83 destination copies, #82 optional proofs and #36/#37/#38's prior
remaining work retain separate ownership. No issue is closed merely because
this source checkpoint is authored or published.

The known audio-alignment cancellation/start-marker failure remains unresolved,
cause unproven. Preserve historical receipts and proposed ADR statuses. Source
package 0.3.0 remains unreleased; #10 retains first immutable publication.
See the [current roadmap](../../ROADMAP.md#current-source-and-work-order) and
[retained work ledger](https://github.com/egohygiene/aniflow/issues/117).
