# Continue recovered frames through optional RIFE and native assembly

`music_video_recipe::recovered::with_rife` is a separate versioned fixed entry.
It reuses existing sparse missing-frame work and complete carrier composition,
optionally runs recovered-input RIFE, binds actual accepted interpolation output,
then performs existing native master/delivery assembly. The old recovered v1
entry and all of its serialized meanings stay unchanged.

Supply `aniflow.recovered-music-video-request/v2` with `base` containing the
existing complete v1 recovered request and optional `interpolation` containing
an explicitly enabled existing RIFE configuration. Null/omitted interpolation
is the bypass: no RIFE executable/model identities or readiness are inspected.
Selected interpolation must fit the existing geometry/device/resource profile
and 50000-source/100000-derived-frame bound before expensive missing work starts.
The base whole-recipe storage bound must also cover conservative native RIFE
snapshots/output/evidence admission. These are estimates, not OS/RSS/VRAM quotas.

```bash
aniflow --output json video recovered with-rife plan --request recovered-v2.json
aniflow --output json video recovered with-rife run --request recovered-v2.json --run-directory /private/recovered-v2
aniflow --output json video recovered with-rife status --run-directory /private/recovered-v2
aniflow --output json video recovered with-rife resume --run-directory /private/recovered-v2
aniflow --output json video completion --run-directory /private/recovered-v2
```

Plan is read only. It freezes existing original/index/selected-origin and missing
input evidence, exact optional component identities and finite whole-run bounds.
Only known-input child plans are resolved; no future carrier/RIFE/output identity
is invented. Run reserves a fresh private root with `recovered-rife-intent.json`,
fixed `children/missing_NNNN`, optional `children/interpolate`, and
`children/assembly`. `carrier/` and optional `rife-assembly.json` are created only
from their actual current accepted predecessors.

Every old selected frame keeps its original ordinal and history. Fresh missing
work remains bounded native v3 per-frame recovery. Interpolation consumes the
complete recovered pixels, preserves original inspection and derives its actual
full-duration 2x clock. Its accepted output supplies a v2 assembly carrier and
v3 native assembly operation; source song offsets/tails and original temporal
truth remain in the existing assembly/validation owners. Import declarations,
interpolated frames and structural measurements do not approve creative quality.

Compatible resume rechecks original input, current origins, components, native
plans and actual checkpoints. Completed children are reused; RIFE keeps its
existing whole native stage recovery granularity. Partial native initialization
or carrier publication is retained/refused, never guessed complete or overwritten.
Parent guards protect original/missing native evidence; assembly remains the
owner of its accepted RIFE writer. Already complete resume returns readback
without fresh source-admission/native work. Final completion projects the actual
accepted assembly and is consumable by existing finishing/publishing owners.

Five SDK order/bypass/ordinal/refusal cases and one CLI parser case are authored,
uncompiled/unrun. All tests/compiler/build/dependencies/lint/format/schema/docs/
corpus/product/native/provider/media/model/CI execution remains deferred to flow
#88, scheduled last with its original 45-ticket ledger. No private Akashic assets,
actual review, model processing or publication occurred. Known alignment
cancellation/start-marker failure remains unresolved, cause unproven. ADR-0020
remains proposed. Historical workflow #35 stays closed; this extension belongs
to #107/#110 rather than reopening older work.
