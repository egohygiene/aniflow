# Metadata-only remux — source checkpoint

`delivery_metadata::remux` now authors one typed fixed v3 stage around the
existing copy-only argv owner. `plan` binds source/selection, explicit existing
metadata policy, settings/schema digest, tool bytes, provider/configuration and
ordinary v3 ports without executing tools. `run` and `resume` use the existing
runtime, single writer, private candidate, cancellation, immutable checkpoints,
artifact observation and independent native temporal/decode validator. Changed
source, policy or tool/schema pins require a new run. There is no new runtime.

The bounded initial lane is Unix, pinned FFmpeg/ffprobe 8.0.1, nonfragmented
MP4/MOV input with only selected H.264 and optional AAC-LC, at most 100,000
contiguous zero-origin CFR video frames, supported unrotated square-pixel BT.709
SDR and the existing exact selected audio extent. Chapters/programs, extra or
discarded streams, unknown packet side data and unsupported preservation
requirements refuse. AAC packets, extradata/configuration and priming/timing
retain existing exact checks. Video packets independently compare complete
ordered payload hashes, sizes, flags and rational PTS/DTS/durations; codec
extradata, geometry, SAR and color are compared separately. Output fully decodes
and retains the required MP4 faststart layout. `-c copy` introduces no lossy
encode, resize, rate conversion, normalization, signal-mark removal or audio edit.

An independent host MP4 reader inspects at most 8,192 boxes, depth 12 and 1 MiB
of descriptive payload/padding. It bounds box sizes, rejects open-ended boxes,
parses the FFmpeg global UTF-8 `mdta` handler/keys/items and compares exact key
values against final FFprobe dictionaries. Duplicate/unknown containers, UUID
boxes, nonzero padding, incompatible metadata types/namespaces and inconsistent
fields refuse the scoped clean result. The reader follows known container
headers and excludes structural leaf payloads, sample entries, coded media/SEI,
signatures and signal marks. It cannot establish universal private-data removal
or credential validity. Source credentials and private originals are retained.

Only this new combined lane can emit `clean_within_scope`. Existing assembly's
FFprobe-only policy receipt remains `unsupported` when visible fields agree;
its previous contract is not silently upgraded. No inspector is called qualified
because source was written. The mdta wire mapping was checked against primary
FFmpeg n8.0.1 `libavformat/movenc.c` (`mov_write_mdta_*` and
`mov_write_meta_tag`); no native fixture was executed.

The accepted report contains private before/after fields, selected replacements,
source/candidate hashes, packet digests and exact tool/configuration authority.
Report files use 0600; tool/source snapshots live in private scratch. Failure
retains scratch/candidate with non-acceptance evidence. Runtime output is not a
release or destination approval. `public_summary` rechecks current complete
acceptance and fixed producer/operation/artifact bindings and returns only
counts/scope/status; never share the private report as the public summary.

See [the SDK example](../../examples/metadata_remux.rs). It takes source,
assembly settings with explicit `delivery_metadata`, stream-selection JSON,
the actual Aniflow provider executable, a separate run parent/root and
`plan|run|resume`. The SDK does not auto-select a source, author, date or URL.
This adds no cross-repository orchestration or automatic recipe/final-handoff
rewrite; Flow and #83/#82 retain their composition/proof ownership.

Five synthetic regression cases are authored and remain uncompiled/unrun.
All compiler/build, dependencies, tests, lint/format/schema/docs checks,
native/provider/media/model/CLI/SDK and CI execution remain deferred under #64.
The alignment cancellation/start-marker failure is unresolved; cause unproven.
Full format/platform/adversarial/native qualification, broader chapter/stream
and metadata profiles remain on #112; its historical acceptance stays open.
