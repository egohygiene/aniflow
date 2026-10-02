# feat: layered validation and evidence-rich delivery (#33)

Pipeline v3 previously accepted its bounded outputs through built-in artifact
integrity. A provider's successful exit or ordinary evidence artifact could not
establish media correctness or a complete, inspectable delivery boundary.

This change resolves explicit inline validators to exact locks and executes
them within the producing stage's acceptance boundary. aniflow verifies the
context, original report bytes, source/artifact identities and supported exact
temporal policy before publishing component, stage, candidate-master and
delivery records. Resume and read-only status verify the retained graph and
current workspace artifacts before claiming compatible reuse or completion.

The public library includes provider-artifact and temporal-media profiles plus
a native adapter with explicit FFmpeg/FFprobe version/hash pins and bounds.
Built-in integrity remains mandatory. Pipeline v2 keeps its existing behavior;
historical v3 documents stay readable but cannot silently acquire current
acceptance authority. Flow consumes public evidence and typed errors.

The change also adds six schemas, synthetic examples, a coherent failure-mode
fixture bundle, 24 authored Rust tests and four authored Python schema tests,
consumer/provider guides and proposed ADR-0008. Multiple single-artifact ports
remain supported; multi-artifact ports are explicitly deferred to #69.

## Validation

**Unrun by maintainer instruction under #64.** No test, build, formatting,
lint, schema, smoke, package, MSRV/stable or hosted-CI qualification is claimed.
The focused future entry point is `task validation:conformance`. Synthetic
coverage does not establish real-media correctness, native-platform support
or release readiness. Original requirements and assertions are preserved.

Implementation receipt: `docs/validation/aniflow-33-implementation.json`.
Recovery and review handoff: `docs/validation/aniflow-33-checkpoint.md`.

Closes #33.
Closes #66.
Closes #67.
Closes #68.

Related: #64, #69, #24. No #34 implementation or merge is included.
