# Fixed release-stage journey (#107)

`music_video_recipe::stages::observe` projects one explicitly selected current
accepted video into a closed, ordered release journey. It is a library read-only
view over existing owners, not another executor, graph engine or acceptance
database. The first version begins **after accepted source assembly**. Source
inspection, frame selection/repair/upscale/RIFE and prepared-frame review remain
in their current recipe owners. This view does not adopt loose manual video.

The request is `aniflow.release-stages-request/v1`: `source` is an existing
accepted-video input, `source_sha256` is its exact expected native file hash, and
`stages` contains every row below in this order. Each row has `stage`, `mode`,
optional typed `selection`, and `disabled_reason`. Modes are `automated`,
`manual_artifact`, `automated_with_review`, `disabled`. Modes are intent, never a
promise of support: missing or incompatible owners remain `unavailable`, with
no implicit manual/model/provider substitution. There are no arbitrary edges or
command strings. Required master/publishing rows cannot be disabled. Optional
disabled rows require a printable reason and cannot carry hidden selections.

| Ordered stage | Current mode/owner | Completion evidence |
| --- | --- | --- |
| `accepted_master` | `automated`, current final-video observer | Current original native acceptance, artifact hash and rational clock |
| `timed_lyrics` | `manual_artifact`, existing lyrics import/review/export | Separate exact words/timing declarations and current reviewed SRT delivery |
| `finishing` | Captions: `automated_with_review`; metadata-only: `automated` | Existing accepted caption preview/review/delivery and optional metadata remux |
| `creator_identity` | Explicit disabled or unavailable future hook | No creator/ownership authority inferred |
| `provenance_inspection` | Explicit disabled or unavailable future hook | No authenticity or origin claim inferred |
| `creator_watermark` | Explicit disabled or unavailable composition hook | No creator mark or upstream-mark removal inferred |
| `destination_publishing` | `automated_with_review`, existing publishing v1/v2 | Exact export/QC/preflight, final file/metadata review and local manual package |
| `c2pa` | Explicit disabled or unavailable future hook | No credentials inferred |
| `exact_byte_evidence` | `automated` read-only current retention verification | Exact selected manifest and retained current native final-delivery subject |
| `signature` | Explicit disabled or unavailable composition hook | No signature created or accepted by this view |
| `timestamp` | Explicit disabled or unavailable composition hook | No timestamp/network request inferred |
| `post_upload` | Explicit disabled or unavailable composition hook | Local package completion does not mean posted or platform accepted |

All selected enabled stages must complete in order. An unavailable/incomplete/
awaiting/rejected/invalidated stage blocks later enabled rows without reading
their selected filesystem state. Disabled optional rows are explicitly skipped;
they create no output and cannot meet an enabled caption dependency. Enabling a
hook stops the journey; a stub never counts as success. Hook unavailability here
does not imply the separate capability owner has no implementation elsewhere.

Selections embed existing typed requests and private run references. Lyrics
select their request/root and optional current delivery. Finishing selects its
request, optional root and exact output artifact ID. Publishing selects its
request and optional root. Missing roots for finishing/publishing mean pending
explicit planning. Retention selects an existing directory and externally known
manifest hash. No owner request is silently rewritten. Source/configuration
changes invalidate the view or refuse readback. All reached owner observations
are repeated before returning, without native processing or model calls.

The handoff contains the exact predecessor file digest, final-video interval,
rational frame rate and timeline hash, fixed artifact contract/action and
current review context/subject hashes. Lyrics' `timeline_sha256` must equal the
handoff hash of `(frame_rate, video_interval)`, and their declared delivery extent
must equal the selected video interval. Actual PCM origin/caption compatibility
remains the caption adapter's check, not a new inference from a declaration.
If captions are selected, their lyrics roots must match the explicitly enabled
lyrics stage. Downstream finishing/publishing must consume the exact current
accepted predecessor; complete finishing advances that subject, then publishing
advances it to the exact final destination media. Retention cannot substitute a
loose declared file for that native final subject.

Completion means only that the selected implemented owners report their current
evidence. Technical acceptance, declared human review, descriptive metadata,
visible overlays, signed credentials and invisible marks remain distinct. No
source is changed, tool launched, approval fabricated, account touched, key
created, source disposed or item posted. The local package is a manual handoff.
All contracts and cases are authored/uncompiled/unrun; flow #88 remains last.
Feature #107 and proposed ADR-0023 retain broader raw-source/manual-video,
additional capability composition and native/workflow qualification acceptance.

## CLI and explicit manual actions

`video stages plan --request ABS` and `video stages status --request ABS` expose
this same read-only view through the ordinary machine envelope. Neither launches
native work. Begin with optional rows explicitly disabled to inspect the accepted
master clock/context, then author any selected lyrics/finishing/publishing
requests. The example intentionally leaves publishing unavailable until its
concrete request is supplied. Examples are inert placeholders, not a qualified
Akashic preset or implicit defaults.

`video stages manual --request ABS --change ABS` accepts a separate closed
`aniflow.release-stage-manual-change/v1` declaration. It must name the latest
`intent_sha256` AND `context_sha256`. Input/settings/review changes invalidate
old actions. Import requires an explicit external interval-lyrics path, expected
SHA-256, bounded operator and declared origin. The existing lyrics owner parses,
snapshots and retains original-input/normalized-text evidence; no ASR/alignment
runs. `export_lyrics` requires both existing words/timing decisions and an
explicit output directory. `record_review` requires a fully supplied existing
`ManualReview` and current `words`, `timing`, `caption` or `final_video` scope.
Approve/reject are both supplied data; no default approval exists.

Manual actions delegate to the existing owner writer and exact review gate.
They cannot run/resume native children, replace old decisions, sign, post or
turn a hook into success. After any change, observe again and use a fresh context
for the next action. Existing native owners must be explicitly invoked for
pending automated work. Cancellation or a later changed source may leave the
owner's retained partial state or declaration; preserve and inspect it. The view
adds no global reservation or rollback over independent owner directories.

The returned `aniflow.release-stage-manual-receipt/v1` includes the supplied
change/declared origin, its hash, prior context and fresh status. It is a private
artifact that the caller must retain explicitly; it is not an additional review
or media acceptance database. Provider execution remains false for these manual
actions. The original path/origin/reviewer fields must not enter portable
telemetry by default. No real operator decision or private lyric import was
performed while authoring this checkpoint.

Four additional manual SDK cases and two parser cases are authored, uncompiled
and unrun. Original qualification limits remain unchanged.

## Source/indexed entry before assembly — November 10, 2026

`stages::entry::observe` adds a separate closed entry request/status over an
**existing initialized** source or indexed-frame recipe. The accepted-master
request/status v1 above retain their meanings. This entry selects the exact
existing `recipe_run_directory`, expected immutable recipe intent SHA and final
`artifact_id`. Before assembly completes, no final bytes, clock, approval or
release acceptance are invented. Source/initial input hashes and configuration,
current phase/plan/native-checkpoint identities come from the existing owners.

`video stages entry-status --request ABS` reports the exact existing phase and
what the operator must do next. Pending/ready work points to the existing recipe
resume operation; running/failed/cancelled/incomplete native children require
inspection. This view never starts them. Current prepared frames are exposed
only when the actual review owner can bind accepted preparation/interpolation
outputs. A ready assembly awaiting an unbound interpolation carrier cannot
manufacture a review subject. Rejected/invalidated decisions remain distinct and
require a new context. Complete child phases alone do not establish parent
completion; missing parent bindings remain incomplete.

`video stages entry-review --request ABS --decision ABS` takes a fully supplied
`aniflow.release-stages-entry-decision/v1` with latest entry `context_sha256` and
an existing exact `ManualReview`. It delegates only to prepared-frame record;
source/frame/index/timing/configuration subject checks and existing writer locks
remain authoritative. There is no default approve/reject, override of a prior
record, native processing or automatic assembly resume. The returned state may
be ready for an explicitly separate recipe resume operation.

An optional `release_request` is structurally checked but no selected release
owner is observed until the recipe actually completes native assembly and the
current final-video observer accepts the exact chosen output. Its source root,
artifact and expected byte hash must match that actual master before the
unchanged release-stage view can be reached. An omitted tail reports master
accepted/configure tail; it does not imply posting readiness. The entry context
binds request, source/configuration, native phases and review state and is
reobserved before return. It writes no new acceptance record or workflow state.

The example is an inert existing-recipe locator with placeholder hashes. Five
SDK/projection/refusal cases and one parser case are authored/uncompiled/unrun.
No actual source media, review, native/provider/model/CLI/SDK operation, test,
compiler, dependency or CI was run. Actual creative/native/workflow acceptance
and broader manual-video composition remain feature #107/Flow #88 work. The
known alignment cancellation/start-marker failure remains unresolved, cause
unproven. Proposed ADR-0023 remains proposed.
