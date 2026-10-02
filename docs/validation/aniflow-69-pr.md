# feat: support exact multi-artifact provider output sets (#69)

Pipeline v3 plans could name multiple artifacts on an output port, but execution
and reports still assumed one observation per port. This change implements the
bounded exact-set profile in checkpoints #89–#91.

All-`one` stages keep their v1 invocation/report behavior. Explicit nonempty
`one_or_more` and `many` ports use v2 contracts. Invocation artifact IDs and
report port/path pairs bind every member to the immutable plan; v2 report hashes
also bind the schema. Optional/unbound/empty output sets remain unsupported.

Execution observes the full planned set, including kind, size, digest and
aggregate bounds. Each artifact keeps its own lineage and component evidence.
Only complete stage acceptance can establish a checkpoint, delivery or cache
entry. Resume/status/cache refuse missing or altered siblings and report-profile
downgrades. Validator and stem-import consumers use the same version boundary.

This preserves the existing logical checkpoint commit boundary. Interrupted
copies can leave unaccepted workspace bytes; file presence cannot establish
partial success. No global filesystem transaction is claimed.

## Qualification

**Unrun under #64.** Synthetic contract, runtime, cancellation/recovery, tampering
and cache tests are authored. No test, build, compiler, formatting, lint, schema,
drift, native, smoke/package or hosted qualification ran. No CI was dispatched
or polled. Source review and exact publication-tree comparison are not passing
qualification. The known alignment cancellation failure remains unchanged.

Updated closed schemas/examples, provider and Flow guidance, proposed ADR-0008,
corpus source locators, roadmap and root continuity. Cargo remains 0.3.0.
No release, tag, real media or model execution occurred.

Closes #69.
Closes #89.
Closes #90.
Closes #91.

Qualification remains tracked by #64. Related: #24, #80, #10, #79.
Handoff: `docs/validation/aniflow-69-checkpoint.md`.
Receipt: `docs/validation/aniflow-69-implementation.json`.

Base: `be2833185c0737e5e955e5a34cabd773bc6d3f5f`, tree `e394fdb5dc6f063d31fe5459a091f0ca196af731`.
Implementation checkpoint: `03eadfb4f0fbd30aa37f4ed736ea369ac4447218`.
Final remote head/tree will be recorded after publication.
