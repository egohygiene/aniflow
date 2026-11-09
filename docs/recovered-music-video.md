# Continue from accepted recovered frames

`music_video_recipe::recovered` is a fixed entry over existing native v3 owners:
selected missing Upscayl frames → complete recovered carrier → verified native
master/delivery assembly. Reuse accepted historical/current frame work without
rerunning those ordinals or labeling historical images as native Upscayl output.
The source/indexed entries keep their existing contracts and meaning.

Supply `aniflow.recovered-music-video-request/v1`: `spatial` is the explicit
[carrier selection](complete-spatial-carrier.md), `assembly` is existing native
assembly configuration, and `provider_executable` is the selected compiled
provider. Set finite `maximum_working_bytes`/`minimum_free_bytes` and
`frames_per_run` (1..128). If selection inspection has gaps, also supply paired
original `frames_directory` and explicit per-frame `upscale` configuration.
Complete reuse needs neither a fresh model selection nor original PNGs for
already accepted ordinals. Existing native sources still retain their exact
current component/acceptance requirements.

```bash
aniflow --output json video recovered plan --request recovered.json
aniflow --output json video recovered run --request recovered.json --run-directory /private/recovered-run
aniflow --output json video recovered status --run-directory /private/recovered-run
aniflow --output json video recovered resume --run-directory /private/recovered-run
aniflow --output json video completion --run-directory /private/recovered-run
```

Plan is read only. It freezes original source/index, current selected origins,
all missing original PNG identities, fixed serial batches and component pins.
Only the first native child plan is initially resolved; subsequent known-input
native plans become current when predecessors complete. Missing ordinals retain
their original positions, including values above 1023; the sparse v2 operation
admits original ordinals below 100000 with at most 128 frames per run. Existing
v1 operation/report/schema bytes and their 1024-frame bound remain unchanged.
Sparse v2 cannot be used as an implicit v1 collector input.

Run reserves a fresh private root with `recovered-intent.json` and fixed
`children/missing_NNNN` and `children/assembly` slots. Existing native source
admission runs before expensive missing-frame work. Each missing child uses the
same existing native per-frame provider, private snapshots, process boundaries,
checkpoint/recovery and validation gates. Resume freshly checks compatibility
and reuses accepted frame checkpoints instead of replaying successful frames.
No missing original file is synthesized or fetched.

After all missing children are accepted, `carrier/` composes old selections and
the new accepted frame units. Complete native assembly consumes that exact
carrier through an explicit v2 assembly operation. It retains the original
temporal inspection and performs the existing full PNG/pixel, frame count,
rational clock, AAC packet/extent, container and decode gates. Final completion
is a read-only projection of the actual accepted assembly child, with no parent
completion database or synthetic future acceptance record.

Status rechecks intent and current owners without tools or mutation. Parent
and current native writers protect execution. Rerun/excluded/changed source
units, original input changes, mismatched geometry or settings refuse. Partial
native initialization or partial carrier publication is preserved for explicit
reconciliation; it is never guessed complete or overwritten. The conservative
working estimate and each native owner's finite storage admission are not an
OS reservation, disk quota or native RSS guarantee. Preserved failures can need
separate operator reconciliation; no source or evidence is disposed of.

Historical raw index bytes and native canonical index fingerprints are bound
separately, so pretty-printing is not mistaken for a native processing identity.
Per-frame source history remains in the complete carrier. Import declarations,
native measurements and assembly completion do not grant full creative approval.
Use [finishing](recipe-finishing.md) and [publishing](publishing-recipe.md) only
after actual completion, with their separate exact-final-file review boundary.

Seven SDK sparse-version/order/coverage/slot/refusal cases and one CLI parser
case are authored, uncompiled/unrun. All tests/compiler/build/dependencies/lint/
format/schema/docs/corpus/CLI/SDK/native/provider/media/model/CI execution remains
deferred to [flow #88](https://github.com/egohygiene/flow/issues/88), scheduled
last. No private Akashic processing or actual review occurred. Known alignment
cancellation/start-marker failure remains unresolved; cause unproven. ADR-0018
remains proposed; no native/platform/release qualification is implied.
