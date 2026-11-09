# Finish an accepted music-video candidate

After finishing, the [fixed publishing continuation](publishing-recipe.md) can
select an explicit destination export, exact-final-file audio QC, dated
preflight and separate final-file/public-metadata review before a manual posting
package. Caption-preview approval does not supply that final decision. Keep all
byte-changing finishing before QC and final review.

The fixed finishing tail starts from a current accepted destination artifact
from indexed/source music-video assembly, or an existing supported native owner.
It selects captions and/or final metadata sanitation through their existing
native facades. It is a continuation after assembly, with separate immutable
intent and fixed child slots.

`music_video_recipe::finishing` exposes `plan`, `run`, `resume`, `status`,
`record_review`, `next_action::observe` and `completion`. The
[next-action guide](finishing-next-action.md) explains the read-only advice.
Thin delivery is `aniflow video finishing`:

```sh
aniflow video finishing plan --request finishing.json
aniflow video finishing run --request finishing.json --run-directory /absolute/new-finishing-run
aniflow video finishing status --run-directory /absolute/new-finishing-run
aniflow video finishing next-action --run-directory /absolute/new-finishing-run
aniflow video finishing record-review --run-directory /absolute/new-finishing-run --declaration decision.json
aniflow video finishing resume --run-directory /absolute/new-finishing-run
```

These are documented commands only; none were executed for this checkpoint.
The closed `aniflow.finishing-request/v1` selects `video_run_directory`,
`artifact_id`, an explicitly installed `provider_executable`, optional `captions`
and optional `metadata`. Use normalized absolute paths. `captions` contains the
existing reviewed lyrics run/delivery directories and `CaptionConfiguration`;
`metadata` contains the existing `AssemblyConfiguration` with an explicitly
selected `delivery_metadata` policy. At least one operation is required. Reuse
the readiness result and exact installed pins; no setup or download occurs here.

The fixed sequence is caption preview → explicit review pause → caption delivery
→ selected metadata remux. Metadata runs last so approved creator fields are
written after the caption encoder's metadata clearing. Metadata alone is also
supported. After accepted soft-mux delivery, the metadata tail explicitly binds
that native caption child and preserves its single authored mov_text track,
including encoded payload/configuration/timing, disposition, language and full
words/millisecond round trip. Its final handoff remains a subtitle-muxed
candidate. Extra tracks and missing current acceptance refuse. A standalone
native soft-caption child can select metadata-only finishing; nested finishing
roots still refuse. Initial TikTok export/preflight and final audio QC continue
to refuse soft-subtitle inputs. Burn-in produces the plain H.264 destination
lane used by the initial publishing path.

Run creates a private root with `finishing-intent.json`, fixed `children` slots,
and later one `caption-review.json`. Intent freezes accepted base handoff, first
exact plan, settings and current executable/tool/font identities. Downstream
plans remain pending until actual native output and an exact supplied decision
exist. Existing v3 plans, provider locks, validation, checkpoints and delivery
acceptance retain authority. Unknown slots, premature children, stale inputs,
partial initialization and old decisions refuse reuse. Rejection stays retained;
start a fresh recipe after reconciliation to change a prior decision.

Preview status supplies exact context/subject hashes and private PNG locators.
The operator inspects them and supplies the existing `aniflow.manual-review/v1`
declaration. Recording a decision executes nothing; resume is separate. Approval
authorizes candidate generation only. Full final-video creative review and the
original lyrics/audio relationship remain separately declared. Manual SRT
authoring remains first; there is no automatic detection or alignment step.

`final_video::observe` recognizes a complete finishing root and projects only its
final native accepted child. Existing destination preflight/posting preparation
can consume that handoff; public summaries omit private words, paths and lineage.
The intermediate caption candidate cannot become final completion while selected
metadata is still pending. Nested finishing roots are deliberately unsupported.

This source work also accepts the two exact square-pixel spellings already used
by assembly (`1/1`) and native playback observations (`1:1`) at consumer boundaries,
preserving the observed value and refusing anamorphic/unknown ratios.

Seven meaningful synthetic cases are authored, uncompiled/unrun. No tests,
builds, format/schema/docs checks, native/provider/media/model/CLI execution or CI
ran. Actual Akashic files and operator decisions were not used. Qualification
remains #64 and flow #88 (scheduled last); #107/#108/#112 remain open for broader
acceptance. Known alignment cancellation/start-marker failure remains unresolved,
cause unproven. ADR-0012/0014 remain proposed. No publication or release occurred.
