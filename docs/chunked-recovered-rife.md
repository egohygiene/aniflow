# Explicit recovered RIFE chunk checkpoints

This source checkpoint adds `video rife recovered chunks plan|run|resume|status|bind-assembly` and matching `video_interpolation::chunks` SDK functions. It is uncompiled and unqualified. Select a closed `aniflow.chunked-recovered-rife-request/v1` with `base` containing the existing pinned recovered RIFE request and `output_frames_per_chunk` an even 2..256. There are at most128 chunks; smaller bounds may refuse a long clock. The original whole-run mode and recovered recipe v2 remain unchanged; using chunked output for assembly is explicit through its accepted carrier.

`plan --request chunks.json` freezes the complete current carrier, original clock, selected components and the full ordinary native v3 graph. `run --request chunks.json --run-directory /private/new-root` creates a fresh parent. `resume --run-directory /private/root` reads only its immutable saved intent. `status --run-directory /private/root` is read-only. `bind-assembly --run-directory /private/root --output-file /private/new-assembly.json` requires actual complete native acceptance and current PNGs/origins. Native child location is fixed at `children/interpolate`.

Migration: use `--output-file` for the binding destination. The former local
`--output PATH` collided with global `--output human|json` and caused a Clap
type-downcast panic. No ambiguous destination alias is retained; the global
option still selects presentation before or after the subcommands.

Each bounded global output interval owns a native producer checkpoint and exact PNG/report set. The collector depends on every selected producer and admits only the full ordered, unique current set. Partial reports retain the full plan but use a distinct schema and cannot become full assembly evidence. Original pixels, cut/endpoint holds, rational full-duration 2x mapping, recovered geometry and unknown historical processing stay intact. Generic native entry points also require the exact full graph; arbitrary collector-only graphs cannot adopt loose files.

On explicit resume, native v3 reuses compatible accepted chunks and retries an unaccepted failed/cancelled chunk as a whole. There is no adoption inside a partially rendered chunk. Earlier exact accepted chunks remain independently reusable when later work fails. Changing grouping, source, origins, tool/model/provider, settings or selected bounds refuses reuse. Status counts are checkpoint observations and never imply visual approval. Unknown roots are retained and refused.

Admission conservatively includes retained chunk frames plus full collection, source snapshots, pinned components and evidence. Source hashing can still traverse the whole carrier per chunk; no speed or peak resource guarantee is made. Failed private work is retained under existing native recovery. No automatic model download or fallback is introduced. Full native cancellation/resume, model output, resource, PNG decoder and visual qualification is deferred to flow #88, last. Authored cases are uncompiled/unrun.
