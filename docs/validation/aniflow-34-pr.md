# feat: add content-addressed reuse and operational controls (#34)

Pipeline v3 could resume accepted stages inside one run, but separate runs had
no explicit way to reuse accepted producer outputs or manage retained content.
Operators also lacked targeted rerun, cache ownership and bounded cleanup controls.

This change adds opt-in local cache reuse through the library and
`run-v3` / `resume-v3 --cache-policy`. Versioned keys bind stage semantics,
producer/validator locks and configuration, inputs, dependency identity,
execution bounds and planning policy. Entries seal the origin plan, accepted
checkpoint, retained evidence and output inventory. A hit verifies that proof,
copies outputs to fresh run-owned inodes, and executes current validators before
publishing current-plan acceptance. Ineligible providers execute normally.

Repeatable `--rerun-stage` selects a stage and its transitive dependents. Pending
rerun obligations and excluded checkpoint hashes persist across interruption.
Same-key conflicting accepted results are refused rather than overwritten.

`cache inspect`, `cache invalidate` and preview/apply `cache prune` expose
bounded, owner-scoped operations with typed diagnostics. Exclusive namespace
and workspace locks protect writers. Storage preflight, entry/count/age limits,
sealed staging and atomic publication bound the cache lifecycle. Planning,
status and inspection do not initialize or clean a cache.

Six closed schemas, examples, 20 authored Rust tests, four authored Python
schema tests, consumer documentation, Flow guidance and proposed ADR-0009
accompany checkpoints #71–#73. Multiple artifacts per output port remain #69.

## Qualification

**Unrun by maintainer instruction under #64.** No build, tests, formatting,
lint, schema, smoke, package, stable/MSRV or hosted-CI success is claimed.
Future focused entry point: `task cache:conformance`. Original #34 acceptance
criteria remain unqualified, and the earlier alignment cancellation failure
under #64 remains unresolved. Synthetic coverage is not real-media evidence.

Cache write preflight currently uses Unix `statvfs`; other platforms refuse
cache writes. Namespace locks serialize writers and crash locks require manual
reconciliation. No automatic abandoned-temporary cleanup is implemented.
Storage preflight is not a filesystem quota or sandbox. ADR-0009 is proposed.

Implementation receipt: `docs/validation/aniflow-34-implementation.json`.
Recovery and review handoff: `docs/validation/aniflow-34-checkpoint.md`.

Closes #34.
Closes #71.
Closes #72.
Closes #73.

Related: #64, #69, #24. Draft review only; no merge is authorized by this PR.
