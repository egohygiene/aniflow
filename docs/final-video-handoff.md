# Observe final video completion

`aniflow --output json video completion /absolute/accepted-recipe-or-assembly-run`
returns `aniflow.final-video-handoff/v1`. Library callers use
`final_video::observe` or `observe_assembly`; `final_video::recheck` compares a
saved handoff with the currently accepted result before packaging or retry.
This is read only: no writer lock, provider, subprocess, downstream selection,
upload, media mutation or independent success record. It reuses existing v3
status, plan, provider-lock, checkpoint and #33 delivery acceptance authority.
A complete source/indexed recipe also requires its parent status before/after
observing its accepted assembly child. Partial/failed/cancelled/interrupted,
stale, changed or unaccepted outputs produce refusal.

The initial supported profile has exactly a clean RGB FFV1 archival master and
an H.264 destination copy. Artifact IDs, private locators, SHA/size, exact rational
rate/interval/duration, geometry, fixed verified SDR color/SAR/orientation,
observed audio properties, immutable plan/producer/checkpoint identities,
source lineage and delivery acceptance identify actual final bytes. Video
duration is the picture extent; the separately reported audio interval can
start earlier or continue beyond it. New assembly evidence retains observed AAC
properties; older accepted audio reports without those properties are refused
rather than inferred. No extra inspection tool is launched by this observer.

The completion identity binds the immutable plan, acceptance and ordered exact
artifact identities, excluding local paths and observation time. Repeated
observation of unchanged accepted evidence yields the same identity. Flow can
combine it with an immutable downstream release/profile/budget digest for its
packaging idempotency key. Recheck immediately before use: it is a point-in-time
observation, not a reservation against later external file edits. Failed/retried
packaging must retain the accepted master and its own child lineage separately;
aniflow does not retry packaging or reprocess video on a downstream failure.

Use `--public-summary` / `handoff.public_summary()` for the allowlisted counts
and artifact descriptors without private paths, lineage, producer/private-report
references or inherited metadata. It never includes media bytes. The private
assembly metadata dictionaries are not copied; only their public-safe scope
summary is projected. Explicit operator selection still governs public media
or attribution inclusion. A digest does not grant permission to publish.

Creative approval is not recorded by this fixed automated profile; publication
authority is none and proof state is not asserted. Manual/review-required and
creator-watermarked variants are unsupported until their own exact acceptance/
approval contract is implemented and qualified. Changed metadata/codec/marks
require a new artifact and accepted byte binding; old signatures/proofs are not
claimed valid. This observer cannot bypass those future gates.

Flow #75/#78 owns capability negotiation and invocation of an immutable released
renderflow provider. Packaging disabled/unavailable/failed leaves aniflow's
accepted result intact. Thumbnail/poster/timed-text/etc support is negotiated
individually; comic/page SDK source does not establish released AMV artifacts.
No sibling imports, arbitrary hooks or deprecated v2 handoff are added.

Five focused regression cases are authored, uncompiled/unrun. Accepted native
completion/retry, interruption, full authority tamper and downstream integration
qualification remain #64/#113 and Flow's owners. No tests/build/lint/format/
schema/docs/native/provider/media/CI execution, dependency acquisition or release.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. #113 remains open for those gates and released integration (#10/#40,
Flow #75/#78, renderflow #419).
