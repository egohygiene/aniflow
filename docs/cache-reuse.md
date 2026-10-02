# Pipeline v3 cache reuse and operations

The #34 implementation is **unqualified**: its synthetic tests, build, format,
lint, contract checks, packaging and platform checks have not run. Qualification
is deferred under #64. Multiple artifacts on one output port remain #69.

## Opt in

`run-v3` and `resume-v3` accept `--cache-policy <policy.json>`. Without it, the
executor never reads or initializes a cross-run cache. Existing run-local resume
remains available. Planning and `status-v3` never initialize, repair or prune a
cache. Run/resume now acquire an exclusive workspace writer lock.

Start with the [policy example](contracts/examples/cache-policy-v1.example.json).
Choose a cache root whose parent already exists. Relative roots in CLI policy
files resolve beside the policy file; library roots resolve against the caller's
working directory. The cache root must be absent or already marked for the exact
owner. An arbitrary existing directory, even an empty directory, is not adopted.
Keep policy files outside the managed root. Reject links and dot/traversal
components, and keep sources, run parents and the cache root disjoint.

The public library exposes `cache_v3::CachePolicy`, `inspect_cache`,
`invalidate_cache`, `prune_cache`, and typed reports/diagnostics. Requests use
`with_cache(policy)` and `with_rerun_stages(vec!["stage-id".to_owned()])`.

## What a hit means

Only a producer whose capability declares `cacheable: true` is eligible. Existing
provider validation disallows cacheable environment-dependent/nondeterministic
capabilities. An ineligible producer executes normally with a `not_cacheable`
receipt. Exact provider authority must still resolve on the current host.

The key binds stage semantics, producer/tool/model identities and configuration,
inline validator requirements/locks, ordered input digests and types, transitive
dependency invocation/output identity, execution bounds and planning policy.
Unrelated plan stages and the pipeline display name are outside a stage key.
Changing an identity-bearing value prevents that stage and its dependents from
matching compatible entries. There is no distributed cache or hidden discovery.

Each immutable entry seals its origin plan, checkpoint/dependency metadata,
producer/validator execution and acceptance documents, and output inventory.
Lookup checks the inventory bytes, origin acceptance, current input identity,
policy and invocation. An entry's timestamps or filename alone cannot authorize
reuse. Corrupt, incompatible, future, linked or incomplete entries are refused;
expired entries are misses. A refusal stops the attempt with recoverable run
state instead of silently trusting it.

A hit copies outputs into fresh candidate inodes and runs the normal acceptance
path, including **current validator executions**. Original provider execution
reports retain their original identity. The new checkpoint and delivery evidence
bind the current plan. Run-local resume can subsequently reuse that current
checkpoint without relaunching validators. Fresh and cached results normalize by
artifact and validation semantics; timestamps, run paths and execution report
identities are expected to differ.

A `cache-origin` document records the sealed origin entry/checkpoint and the
current plan. `cache-decision` documents record miss/hit/expiry, producer
ineligibility, explicit rerun, publication and run-checkpoint reuse. Successful
outcomes include these decisions, and stage status identifies cached producer
reuse. These receipts describe provenance; current acceptance remains the
completion authority. Retained origin proof lives in the owned cache entry and
may be explicitly pruned; current run evidence remains independently usable.

## Targeted reruns

Pass repeatable `--rerun-stage <stage-id>` on run or resume. Aniflow computes the
selected stages and transitive dependent frontier in plan order. On resume it
records the whole frontier before execution. The manifest retains the pending
rerun and excluded old checkpoint digests through interruption/cancellation.
A later resume cannot resurrect those excluded checkpoints or satisfy the
pending rerun from the cache. Independent compatible checkpoints remain usable.

The fresh producer result goes through normal acceptance and cache publication.
If a sealed entry already exists for the same key, matching normalized results
keep the original entry. Different accepted results yield `conflicting_entry`;
there is no silent replacement. Explicitly invalidate the entry before attempting
to replace it. Expired entries are not refreshed by a fresh execution with the
same result; prune or invalidate them to create a new retention interval.

## Inspect, invalidate and prune

```bash
aniflow --output json cache inspect --policy cache-policy.json
aniflow --output json cache invalidate --policy cache-policy.json --key "<sha256>"
aniflow --output json cache prune --policy cache-policy.json
aniflow --output json cache prune --policy cache-policy.json --apply
```

Inspection is a bounded read-only metadata/inventory snapshot. `available` means
structurally eligible for full lookup verification, not already accepted media.
It reports expiry, invalid entries, temporary entries, bytes and writer presence.
It does not hash every payload or execute validators. If a writer changes an
entry during inspection, the snapshot may refuse or report invalid; retry after
the writer finishes. Inspection never creates a lock.

Prune defaults to preview. Applied prune acquires exclusive ownership, selects
expired entries first, then oldest entries to meet total byte/count policy,
breaks ties by key, and removes at most `maximum_prune_entries`. Lower the byte
or entry retention limit to request capacity reclamation. If the selected policy
is already within limits and nothing expired, no entries are selected. Files
outside the namespace, active work, unknown entries and unsealed temporary work
are never automatic cleanup targets. Corrupt data that fails even its sealed
metadata/inventory checks needs explicit operator reconciliation; neither prune
nor invalidation guesses ownership from filenames.

## Bounds and concurrency

The default policy permits 4 GiB total, 1,024 entries, 256 MiB per entry,
100,000 file/directory inventory nodes per entry, 30-day age, 64 MiB minimum free
space, and 128 removals per prune. Scans have hard limits of 4,096 directory
children and 250,000 namespace nodes. These are bounded local caches; scale-out
and automatic abandoned-temporary recovery are not implemented.

With caching enabled, effective provider output byte/file limits are narrowed
to at most one quarter of the policy entry limits, leaving room for evidence and
copies. Both producers and validators receive those bounds; execution reports
and keys expose the effective values. Run/resume should receive the same cache
policy and requested execution limits to preserve compatibility.

Before provider work, reserve a new entry envelope against namespace limits and
check actual free storage for bounded run copies. Publication rechecks the budget,
syncs the staged snapshot and atomically renames it into the owned entries
directory. Physical storage preflight uses `nix::sys::statvfs` on Unix; other
platforms refuse cache writes until a qualified implementation exists. These
checks cannot reserve blocks against unrelated processes and do not impose a
filesystem quota or sandbox. I/O failures remain failures, never completion.

A namespace lock is held across lookup, execution, validation and publication.
Other writers, invalidation and applied prune fail immediately with `busy`.
Separate caches can run concurrently. Exclusive lock creation also protects
run/resume workspace state. Normal exit removes only the lock token it owns.
A crash leaves a lock; it is not stolen based on a PID or age. Confirm the
owning operation has stopped before reconciling the lock manually, or select a
fresh namespace. Do not remove a live writer's lock.

## Deferred qualification

The focused future entry point is `task cache:conformance`. It includes synthetic
copy/validator providers, cache and CLI tests, and closed schema fixtures. No
network, paid APIs, model downloads or real media are needed. The accumulated
#64 audit must also run existing Pipeline v3, layered validation, stable/MSRV,
format/lint, package and platform gates. See the [receipt](validation/aniflow-34-implementation.json).
