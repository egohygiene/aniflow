# Select delivery metadata policy

The [redacted audit](metadata-audit.md) SDK summarizes scoped changes without
exposing arbitrary private tag names or values.

The [metadata remux CLI](metadata-remux-cli.md) delivers explicit
`video metadata-remux plan|run|resume|status` commands over the existing SDK.
Its accepted status command returns the public summary only.

Set `assembly.delivery_metadata` in a recipe or `delivery_metadata` in standalone
assembly configuration. Omission preserves historical behavior/intent fields.
The versioned object requires `schema: aniflow.delivery-metadata-policy/v1`,
`mode` and `approved`. Modes are `preserve`, `minimal`, and
`clear_inherited_then_apply_allowlist`. The first two require an empty approved
object. The allowlist accepts operator-selected title, creator, copyright,
credits, project_id, release_id, version, public_url and description. No identity,
legal authorship, date or public URL is inferred. Unicode is supported; public
URLs require HTTPS without user information. Text and total evidence are bounded.

Policy applies only to the new delivery candidate during the existing final
encode. It does not rewrite the source/master or add a lossy encoding pass.
Explicit format and selected-stream mappings and separate chapter treatment
prevent relying on a global metadata flag alone. Minimal/allowlist remove chapters
and unselected stream kinds stay subject to explicit existing source selection.
Programs are refused by the MP4 policy. Stream language is retained explicitly;
codec, timing, SAR, orientation and color remain governed by the existing strict
native media profile. Unsupported rotation/HDR etc remain refusals.

`delivery_metadata::remux_arguments` exposes an inert copy-only argv primitive
for a separately orchestrated metadata-only candidate. It executes nothing and
cannot confer acceptance. A caller must bind tools/inputs/resources, observe the
actual result and independently verify elementary payloads/timing before any
remux-preservation claim. The later [accepted remux checkpoint](validation/aniflow-112-accepted-remux.md)
adds `delivery_metadata::remux::{plan,run,resume,public_summary}` and an SDK/CLI
example over the ordinary v3 runtime. Its initial H.264/AAC subset compares
complete packet/timing/configuration evidence and bounded global MP4 mdta.

Delivery reinspection includes FFprobe format, stream, chapter and program
fields. An incorrect approved value, changed language, unexpected stream/scope
or residual inspected field prevents stage acceptance. Regenerated brand/muxer
fields are counted separately. The report intentionally says `unsupported` even
when inspected tag checks agree: opaque/nested atoms, attachments, signed source
credentials and format-aware sanitation have not been qualified. An empty tag
list never means universal metadata removal. Full decode and final-byte identity
remain independently checked. No signal/pixel mark is removed by this policy.

Private before/after dictionaries, policy and exact source/candidate hashes live
in the private assembly report and stage scratch receipt (files mode 0600 on
supported Unix execution). Do not bundle the private report. Its `public_summary`
contains counts and declared scope only. Routine errors omit tag values. Sources
and original credentials remain intact; no credential for changed bytes is
asserted. Perform sanitation before future final credentials/proofs, then compute
hashes and #82 proof subjects after the last byte-changing step. #113 projects
only public-safe completion information; publication and creative approval stay separate.

Five original policy cases and five additional remux cases are authored, uncompiled/unrun. All native/tool/media,
compiler/test/lint/format/schema/docs/hosted checks remain deferred under #64.
The known alignment cancellation/start-marker failure remains unresolved, cause
unproven. #112 remains open for full format-aware/native/platform qualification
and broader chapter/stream/container profiles. The remux lane's scoped clean
result preserves all declared opaque/signature/signal exclusions. No keys,
releases, uploads or posting.
