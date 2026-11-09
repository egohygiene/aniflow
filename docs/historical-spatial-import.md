# Import a selected historical spatial frame set [skip ci]

The SDK `upscayl_recovery::historical` and thin
`video historical-import review-plan|plan|run|resume|status` commands import
explicitly selected interrupted-run PNGs through current native validation.
Supply an original video/index, exact selected file identities/ordinals, uniform
2x/3x/4x processing geometry and a pinned decoder. A request selects 1..128
files per native stage; unselected files remain untouched.

`review-plan` produces the context and selected-subject hashes without executing
a decoder. Supply your own `aniflow.manual-review/v1` approving declaration for
those exact hashes to plan/run. This is an import decision, not creative quality
approval or authenticated ownership. No decision is generated automatically.
Changed source/index/selected bytes, clock, geometry, decoder or decision refuse
stale resume. Plan/status do not execute tools.

Native execution uses the existing v3 process/lock/cancellation/acceptance owner,
private pinned decoder, PNG structure/CRC and complete RGB24 decode, immutable
copies and finite byte/pixel/file limits. Its output is a distinct
`aniflow.imported-spatial-selection/v1` subset plus report. Original global
ordinals and source times remain intact. Prior tool/model/settings history stays
unknown. A subset is not a complete assembly index, native Upscayl checkpoint,
whole-job recovery or automatic mixed native/imported collection. Those broader
#37 handoffs remain explicit future work rather than fabricated prior execution.

The native report/status exposes current exact acceptance and decoded hashes;
original inputs are never overwritten or disposed of. Partial failed candidates
are retained by the existing native runtime and never inferred complete.

Six SDK regression cases plus one parser case are authored, uncompiled/unrun.
All tests/builds/checks/native/provider/media/model/CLI/SDK/CI execution remains
deferred to flow #88, scheduled last. ADR-0016 is proposed. Known alignment
cancellation/start-marker failure remains unresolved; cause unproven. No actual
media, import decision, creative approval, release or publication was produced.
