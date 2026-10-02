# Content-addressed reuse and operational controls (#34)

Status: implementation contract, qualification deferred under #64.
Checkpoints: #71 contracts → #72 execution → #73 operations and handoff.

## Boundary and data flow

The public `cache_v3` module owns policy, owned local storage, bounded inventory,
locks, sealed entries, inspection, invalidation and pruning. The existing
Pipeline v3 executor owns semantic keys, retained proof verification and current
acceptance. CLI commands delegate to the same library. Flow consumes versioned
results and diagnostic codes. Pipeline v2 and provider-local model caches are
outside this boundary.

Run/resume receives an optional explicit cache policy and rerun stage IDs.
Absent policy means no cross-run cache lookup or mutation. Planning/status
remain read-only. Inputs, run directories and the selected cache namespace must
not overlap. A cache hit copies bytes to fresh run-owned inodes, retains and
verifies origin proof, then executes the current plan's validation gates. The
producer is skipped; validator invocations are never fabricated or rebound.

## Identity and evidence

Use a versioned key over the existing stage invocation identity and normalized
planning policy. The invocation already binds the full resolved stage (including
validator contracts/locks/configuration), ordered inputs, execution bounds and
transitive dependency invocation/output identities. A sealed cache entry binds
that key to the origin plan, accepted checkpoint and its dependency metadata,
retained execution/validation/acceptance documents, and exact output bytes.
Absolute run/source paths and timestamps are not semantic cache-key inputs.

Validate the origin plan, checkpoint, producer/validator reports, layered
acceptance and payload inventory before using bytes. Unrelated stage changes
can reuse matching producers; current validation is still mandatory. Cache
entries do not replace current run evidence. A public decision receipt explains
miss, hit, refusal, explicit rerun and publication. Different accepted output
semantics for one key are a conflict, never an overwrite.

## Operations and lifecycle

Namespace ownership is explicit and marked. Never adopt arbitrary populated
directories. An exclusive namespace lock spans an active run/cache mutation,
including lookup, execution, publication and deletion. A separate workspace
lock prevents concurrent resumes. Locks use exclusive creation and fail closed;
crashes leave a visible lock requiring operator reconciliation, never automatic
PID-based lock stealing. Inspection does not acquire or create a lock.

Publish a fully synced temporary entry by atomic rename on the same filesystem.
Incomplete temporary entries have no acceptance authority. Bound entry bytes,
file counts, namespace bytes/count, age and deletion batch size. Preflight actual
free storage and policy budget before expensive work/copying, and recheck before
publication. Provider output limits remain independently enforced. These checks
are not a filesystem quota or an OS sandbox against unrelated writers.

Explicit invalidation removes a selected entry under the namespace lock.
Pruning deterministically selects expired entries and then oldest entries to
meet retention targets. Only sealed entries of the selected owner may be
deleted; unknown, corrupt, linked and temporary content is retained and reported.
No automatic cleanup in planning, status, or ordinary run preflight.

Targeted reruns validate stage IDs before mutation, persist invalidation of the
selected stage/dependent frontier, bypass run-local and cross-run reuse for that
frontier, and retain other compatible checkpoints. Previously invalidated stages
remain excluded from historical checkpoint discovery across interrupted resumes.

## Qualification

Author hermetic identity, corruption, expiry, conflict, contention, cancellation,
retention, path isolation and rerun tests. Do not execute them during this pass.
Record exact source changes and not-run status under #64. Multiple artifacts on
one output port remain #69. No real-media or native-platform qualification is
implied by source implementation or merge.
