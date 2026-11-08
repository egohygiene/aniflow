# Accepted metadata-only destination completion

Source checkpoint for #113. `final_video::observe`/`recheck` now dispatches only
the two named native profiles: fixed assembly (including complete recipe child)
or `metadata-only-remux`. `observe_remux` is also available explicitly. A remux
handoff contains one `metadata_delivery` destination copy, with no invented
archival master or extra encode.

The existing remux owner rechecks private report, exact operation/provider lock,
source lineage, current accepted candidate bytes, bounded independent MP4 mdta
receipt and clean-within-scope metadata result. Completion additionally joins the
accepted producer checkpoint, native temporal validation and delivery acceptance.
Its playback projection records only fresh native report facts: exact geometry,
CFR count/rate/interval/duration, codec/pixel/color/range/SAR/orientation and AAC
packet/configuration/timing properties when present. Original elementary packet
preservation and full decode stay native evidence, not claims of qualification.

New remux execution reports include optional observed `playback` facts. Historical
reports remain readable by the existing metadata-summary owner, but completion
refuses older reports lacking those facts; it never backfills them or runs tools
during observation. Candidate changes or stale authority invalidate recheck.

The existing explicit public allowlist remains in use. Private locators, report
fields, removed metadata values, source lineage and transcript content are not
projected. `clean_within_scope` still excludes structural leaf payloads, sample
entries, coded signal/signatures and other declared unsupported metadata scopes.
Assembly's older FFprobe-only Unsupported receipts remain unchanged. Automated
completion records no creative approval, publication authority or proof state.

Three synthetic projection regressions are authored, uncompiled/unrun; they do
not fabricate native acceptance. No tests/builds, validators, native/remux/media
execution or CI ran. #113/#64 retain observed native/platform, consumer/release
qualification; the known alignment cancellation failure remains unresolved with
cause unproven. Flow remains the cross-repository orchestration owner.

The final source pass also resolves automatic original stream selection into
explicit IDs for the optional recipe meter and joins lyric audio by original
source/stream/extent facts. Meter and recipe inspections retain their own honest
digests when their selection declarations differ. One additional focused stream
selection regression is authored, uncompiled/unrun; native behavior is unobserved.
