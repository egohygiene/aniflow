# #111 target-resolution checkpoint

## Source and authorization boundary

Branch: `codex/111-target-resolution`, based on recipe head
`31352771bcb6e6d043bf1f19bd57b87be3853e20` from open stacked
[draft PR #131](https://github.com/egohygiene/aniflow/pull/131). Its assembly
parent [PR #130](https://github.com/egohygiene/aniflow/pull/130) remains open
at `1893bfb994d79e75b08d32739ed61aa23e58cde9`; main remains
`8cb00a323b1829de1e4b958236b6aa8529eb7045`. This checkpoint is published as
[draft PR #132](https://github.com/egohygiene/aniflow/pull/132), created on
2026-10-27 at 23:07:51Z and targeting `codex/35-music-video-recipe` at the
recipe head above. PR #130/#131 remain open and unchanged; all three drafts
are unmerged.

First saved implementation commit: `586dddf0bc0b04bfe657b0ef65615f7b7da8925f`;
first tree: `fda26a9af42b7e3b0c07e5f3ab05d5a52bd1b271`. All 47 published
blobs and the exact tree matched the reviewed local source. These are dated
first-save receipts, not candidate self-pins for later documentation changes
or execution qualification. Re-query current heads before continuing; prior
handoffs remain preserved.

The maintainer authorized #111 implementation and a stacked draft checkpoint.
The issue was read live before selecting this work. All tests, builds,
dependency resolution/installation, lint/format, naming/schema/drift/docs
validators, native/provider/CLI/media/model execution, smoke/package and hosted
qualification remain deferred under #64. No Actions are dispatched or polled.
Publication retains disabled hooks and `[skip ci]`.

## Authored policy boundary

A closed target-resolution policy decides whether actual stage input needs
existing Upscayl processing. It separates explicit target dimensions,
exact-versus-minimum intent and force intent. Both compatible oriented dimensions
matter; pixel count, a filename containing “4k” or a sufficiently wide frame
cannot establish UHD geometry. Landscape 3840×2160 and portrait 2160×3840 are
explicit examples, not inferred rotations of one another.

Bypass passes accepted input identity forward unchanged and retains geometry,
reason and rule/configuration identity. It does not require the neural tool or
model files, invent a neural execution report, downscale a larger input or
claim quality improvement. Required import, source-profile preflight and
assembly validation remain active. Executing paths retain exact existing
Upscayl executable/model/backend/device/tile/worker/TTA configuration and
per-frame or bounded-chunk recovery.

Model-native scale, requested output scale and final geometry remain separate.
Unsupported aspect, rotation, non-square pixels and scale combinations are
refused rather than silently cropped, stretched, letterboxed or resized. Force
cannot create an unsupported same-size reprocessing or downscaling profile.
Below target, exact mode requires a matching 2×/3×/4× factor; minimum selects
the smallest supported factor meeting both axes and reports any overshoot.
At/above target, both modes preserve input unless forced. Forced minimum then
selects 2× subject to existing gates; forced exact refuses unsupported 1× or
downscale behavior. Native support and resource qualification are not
established merely by the geometry arithmetic.

Planning distinguishes declaration-based input facts from accepted stage
artifacts. The actual decision is rebound to the current accepted frame bytes
and index at the recipe handoff. Changed input geometry, target, configuration
or selected component identity cannot reuse a prior decision as authority.
Existing child plans, immutable checkpoints, locks and acceptance still govern
the execution path. No general conditional engine, second checkpoint owner or
new neural-provider framework is introduced.

The `upscale_target` observer reads either original or Upscayl spatial indices,
checks every current PNG structure/IHDR/digest and binds directory identity.
Its frame profile remains declared; it is not native source or full-decode
qualification. The recipe still accepts only original RIFE input indices at
entry. Its `target_resolution` wrapper distinguishes `original_input` planning
from `prepared_frames` accepted handoff, carrying decision/resource facts and
neural component IDs only on execute.

Importer `maximum_pixels` remains 8,400,000 by default; the explicit ceiling
is raised to 33,554,432 to admit compatible above-UHD bypass when requested.
All other source/frame-byte/aggregate/capture/time/resource bounds remain
unchanged. This does not qualify 8K, and frame-byte bounds can still refuse
poorly compressible large PNGs.

Optional target fields preserve the legacy serialized field sets when absent.
The explicit importer ceiling expansion nevertheless changes embedded schema
digests, provider locks and affected plan/intent hashes. Unchanged defaults are
not a promise that older retained runs resume across this source/schema update;
existing exact compatibility rules remain authoritative.

## Bounded excerpt preview

The separate `video upscale-preview` surface selects an explicit inclusive
frame range with the same Upscayl settings and mandatory shared target policy.
The excerpt is bounded to 256 frames and ten exact source seconds. Original
ordinals and source-time intervals remain intact. Per-frame mode keeps its
existing ordinal bound; bounded chunks must start on the selected configured
chunk boundary. No automatic execution-mode or chunk-size change is inferred.

Preview outputs selected PNGs and native observations in a new isolated
ordinary v3 run. It does not create a complete collected output index or final
MP4, overwrite full-run work, carry visual approval or substitute for full-run
acceptance. A target bypass is reported as a clear preview refusal before
requiring neural dependencies, because no neural preview is necessary for the
unchanged path. The [guide](../target-resolution.md) documents commands and
configuration boundaries.

## Evidence and qualification

Root and independent source reviews found no remaining concrete blocker after
finite observer bounds, exact observed directory identity, preview resolved-input
bindings, original-CFR checks and graph-authority checks were corrected.
Seventeen focused regressions are authored and unrun: six pure target-policy,
four recipe admission/identity, four preview and three CLI cases, plus stable
machine-name mapping coverage. Preview cases cover exact bounds/grid, same
settings without complete collection, generic full-output alias/source mutation
and rebased-clock refusal. Existing regressions remain retained.
Source review is not compilation, schema validation, native execution
or passing conformance. When #64's deferral is lifted, qualify exact
and minimum targets, portrait/landscape arithmetic, already/above-target
bypass, explicit force, deceptive filenames, incompatible aspect, rotated or
non-square inputs, malformed observations and changed inputs on resume.

Also qualify dependency-free neural bypass, executing-provider failures,
actual prepared-input decision binding, policy/output tampering, child writer
ownership, stable machine output, resource estimates and excerpt
range/chunk/time refusal. Native/tool/model/device qualification must separately
exercise supported media, preview/full-run settings, decoded output geometry,
exact original clocks and the required downstream assembly gates. An excerpt
view or successful geometry comparison is not that evidence.

Next is reviewing the published stack and syncing on the remaining useful
#35 pathway; no next issue has started. #111 remains open for qualification
and residual integration. The recipe
still requires original indexed source frames; prior Upscayl spatial-index
entry is not added here. This work does not add initial extraction or broader
source profiles. Parent #35 retains its
extraction, broader media, loudness/metadata and qualification scope; #107
retains broader manual/review recipes. Destination resizing/export remains
#83. #37 recovery and #38 setup/model/environment work retain their prior
ownership. ADR-0012 remains proposed; implementation does not accept it or
backfill #14.

The known audio-alignment cancellation/start-marker failure remains unresolved,
cause unproven. Historical receipts remain unchanged. Source package 0.3.0 is
unreleased; #10 owns first immutable publication. See the
[current roadmap](../../ROADMAP.md#current-source-and-work-order) and
[retained work ledger](https://github.com/egohygiene/aniflow/issues/117).
