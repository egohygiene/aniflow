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
