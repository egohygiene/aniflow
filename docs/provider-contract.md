# Temporal provider contract

The temporal provider contract is aniflow's provider-native boundary for
specialized frame, audio, timed-text, whole-video, inspection, validation,
assembly, delivery, and observation capabilities. It lets aniflow inspect and
reason about a provider before executing it while keeping provider algorithms
outside the temporal orchestration core.

The contract separates inert declarations from explicit local registration,
deterministic resolution, authority-bearing locks, bounded execution, and
validated results. It does not migrate the pipeline v2 processors or treat a
provider process as untrusted sandboxed code.

## Contract set

| Contract | Purpose |
| --- | --- |
| `aniflow.provider-manifest/v1` | Inert provider identity, capability, artifact, requirement, behavior, lifecycle, and provenance declarations |
| `aniflow.provider-configuration/v1` | Exact provider/capability selection plus provider-schema identity, effective values, and their canonical digest |
| `aniflow.compatibility-fingerprint/v1` | Self-validating identity across inputs, pipeline and provider configuration, provider/capability versions, tools, codecs, models, and validated outputs |
| `aniflow.provider-lock/v1` | Exact standalone selection, implementation and component identities, granted effects, and offline state |
| `aniflow.provider-event/v1` | Ordered lifecycle observations scoped to one provider lock |
| `aniflow.provider-execution-report/v1` | Applied bounds, process termination, redacted diagnostics, validated outputs, events, and terminal outcome |

The public Rust models and their parsers use the same field names and reject
unknown fields and unknown contract identifiers. Provider and capability
versions use semantic versioning. A contract major and a provider/capability
semantic version are intentionally separate: the contract major governs the
document shape, while semantic versions identify implementations.

The JSON Schemas validate transport shape, required fields, identifier
patterns, and closed enums. Cross-field semantics such as observer mutability,
cache coherence, resource/effect agreement, provenance completeness, and
self-validating digests are enforced by the Rust parsers and must be mirrored
by non-Rust implementations.

## Provider manifest

A manifest is inert declaration data. Loading it never executes a provider or
grants authority. Each manifest contains:

- a lowercase dotted provider ID, semantic version, and display name;
- one or more exact provider-owned configuration-schema references;
- one or more `aniflow/*` capability declarations; and
- the complete v1 provenance promise.

Every capability declares one temporal extension family, its semantic version,
typed immutable inputs and outputs, batching mode, maximum concurrency,
requirements, behavior, side effects, progress granularity, and cancellation
support.

Artifact ports distinguish their artifact role from an optional temporal stream
role. Direction comes from the `inputs` or `outputs` collection. All ports are
immutable: a provider receives source or prior-stage identity and produces a new
artifact rather than rewriting accepted source or completed stage bytes.

Lifecycle observers are structurally read-only. They have inputs, no outputs,
no batching, no content-changing declaration, and cannot request filesystem
writes or publication. A content-changing hook is therefore represented as an
ordinary processor capability with a new immutable output, not as an observer.

Side effects describe requested behavior; they are not permission grants. Only
delivery providers may declare publication, and the caller still owns
authorization.

## Effective configuration

Provider-specific settings remain typed by a separate JSON Schema. The common
configuration envelope binds:

- exact provider and capability IDs and semantic versions;
- the provider-owned schema ID, semantic version, and SHA-256 digest;
- the effective JSON object after defaults and normalization; and
- `effective_configuration_sha256`.

The schema digest identifies the exact schema bytes. The effective digest uses
aniflow's canonical JSON encoding: object keys are recursively sorted, arrays
retain their declared order, no insignificant whitespace is emitted, and JSON
scalars use `serde_json` encoding. `ProviderConfiguration::new` derives this
digest; parsing an existing envelope recomputes it and rejects mismatch.

The common envelope does not replace provider-specific schema validation. The
embedding application validates `values` against the provider-owned schema
before registration. The local registry then verifies that the exact schema
identity agrees across the manifest, capability, and effective configuration;
it does not interpret arbitrary provider-owned JSON Schema keywords.

## Compatibility fingerprint

`CompatibilityFingerprint::new` hashes a canonical payload containing:

- ordered immutable input identities;
- the normalized pipeline-configuration digest;
- the effective provider-configuration digest and exact configuration-schema
  identity;
- exact provider and capability IDs and semantic versions;
- sorted exact tool, codec, and model identities, with digests where available;
- ordered validated output identities bound to validation-contract evidence.

The document stores both the payload and its SHA-256 fingerprint. Parsing
recomputes the fingerprint, so editing any compatibility field without deriving
a new identity fails closed. Tool, codec, and model arrays must be unique and
sorted by ID and version; temporal input and output arrays retain meaningful
order.

This is the compatibility material required by later checkpoint and resume
decisions. It does not itself authorize reuse. The runtime must still inspect
the referenced state and explain why it is accepted or invalidated.

## Explicit registration and resolution

`ProviderRegistry` contains only registrations supplied by the embedding
application. It never searches `PATH`, executes discovery commands, or treats a
manifest as executable authority. Every registration binds a validated
manifest and effective configuration to an absolute executable path, a stable
implementation ID, and observed exact tool, codec, and model identities.

A resolution request names one primary registration, an optional replacement,
and ordered fallbacks. Resolution considers replacement, primary, then
fallbacks in declared order. Each unavailable candidate retains typed reasons
covering capability and version mismatch, executable availability, missing or
incompatible components, host CPU/memory/storage observations, required GPU or
network availability, offline incompatibility, and denied side effects. The
first available candidate becomes resolved; fallback never occurs after its
process starts.

The resulting provider lock includes the canonical manifest and effective-
configuration digests, exact executable digest, relevant component identities,
authorized effects, selection source, and offline mode. The lock digest covers
its canonical payload. Parsing recomputes it, and `write_new` publishes it
atomically without replacing an existing lock.

Host resource values are observations supplied by the embedding application.
They make declared minimum-resource preflight deterministic and testable; they
are not kernel quotas.

## Bounded local execution

`ResolvedProvider::execute` re-hashes the executable immediately before launch
and rejects an implementation that changed after resolution. It then uses a
direct argument array with no shell, closed standard input, explicit absolute
working and output directories, and an initially empty output directory.

The caller supplies a cancellation token and explicit wall-clock, termination-
grace, stdout, stderr, artifact-file-count, and artifact-byte limits. stdout and
stderr are drained independently. Crossing a capture or artifact bound stops
the process instead of merely truncating evidence after it exits. On Unix the
runtime places the provider in a new process group, sends that group `SIGTERM`,
waits the configured grace period, and then uses `SIGKILL` if necessary. This
also covers descendants; other platforms terminate the direct child. Timeout
and cancellation use the same cleanup path.

Lifecycle events are emitted as work changes state and copied into the final
report with contiguous sequence numbers. The report retains the applied bounds,
process exit or signal, bounded diagnostics, and failure code. It never contains
the provider argv or environment. Caller-supplied sensitive values are replaced
before captured diagnostics enter the report.

## Output acceptance

Exit code zero is necessary but never sufficient for success. Every manifest
output port must have one caller-owned relative path binding. The runtime
rejects absolute paths, traversal, overlapping bindings, unexpected entries,
symlinks, wrong filesystem kinds, missing required outputs, and empty artifacts.
It recursively enforces total file and byte bounds and derives deterministic
file or directory digests. File digests cover file bytes. Directory digests
cover canonical JSON entries sorted by portable UTF-8 relative path, including
entry kind, byte count, and each file digest. Only then does the report become
`succeeded`.

Failure, cancellation, timeout, capture overflow, artifact overflow, and
validation failure remove the contents of the initially empty output directory.
Accepted outputs remain immutable for later compatibility and checkpoint
evidence.

Configured local executables still run with the user's authority. Side-effect
authorization is fail-closed preflight policy, not an OS filesystem or network
sandbox. CPU and memory declarations are availability checks, not cgroup or
container quotas.

## Relationship to flow

aniflow owns these temporal-domain contracts and imports no flow source or
schema. A flow adapter translates the released aniflow boundary into flow's
suite contracts:

| aniflow evidence | flow projection |
| --- | --- |
| Provider and capability declarations | `flow.extension-manifest/v1` capability declaration |
| Artifact types and roles | Immutable `flow.artifact/v1` references |
| Effective configuration | `flow.extension-invocation/v1` configuration identity |
| Side effects and requirements | Extension permission requests, never grants |
| Progress and cancellation support | Invocation bounds and `flow.extension-event/v1` behavior |
| Compatibility fingerprint | Provider result, provenance, and checkpoint compatibility evidence |
| Standalone provider lock | Input to flow's independently authoritative extension resolution and lock policy |
| Execution events and report | `flow.extension-event/v1` and result/evidence projections |

flow's operator-controlled lock remains authoritative for cross-holon trust,
permission grants, precedence, replacement, and fallback. The aniflow manifest
cannot grant those rights. The aniflow provider lock is authoritative only for
standalone temporal execution and can be projected into flow without importing
flow source or schemas.

## Published schemas and examples

- [`provider-manifest-v1.schema.json`](contracts/provider-manifest-v1.schema.json)
- [`provider-configuration-v1.schema.json`](contracts/provider-configuration-v1.schema.json)
- [`compatibility-fingerprint-v1.schema.json`](contracts/compatibility-fingerprint-v1.schema.json)
- [`provider-lock-v1.schema.json`](contracts/provider-lock-v1.schema.json)
- [`provider-event-v1.schema.json`](contracts/provider-event-v1.schema.json)
- [`provider-execution-report-v1.schema.json`](contracts/provider-execution-report-v1.schema.json)
- [`provider-manifest-v1.example.json`](contracts/examples/provider-manifest-v1.example.json)
- [`provider-configuration-v1.example.json`](contracts/examples/provider-configuration-v1.example.json)
- [`compatibility-fingerprint-v1.example.json`](contracts/examples/compatibility-fingerprint-v1.example.json)
- [`provider-lock-v1.example.json`](contracts/examples/provider-lock-v1.example.json)
- [`provider-event-v1.example.json`](contracts/examples/provider-event-v1.example.json)
- [`provider-execution-report-v1.example.json`](contracts/examples/provider-execution-report-v1.example.json)
- [`example-frame-configuration-v1.schema.json`](contracts/examples/example-frame-configuration-v1.schema.json)

The examples are synthetic and contain placeholder implementation and artifact
digests. Their enclosing lock and report digests are nevertheless canonical and
self-validating.

## Deferred checkpoints

- `ANI-11.3` adapts the pipeline v2 builtin processors and removes the deprecated
  renderflow handoff only in pipeline v3.
- `ANI-11.4` publishes the complete conformance corpus and extension-authoring
  guide.

Provider-native item-count or fractional progress ingestion, remote providers,
kernel/container isolation, and Pipeline v3 checkpoint reuse remain deferred.

Issues #8 and #13 consume the completed SDK rather than expanding this contract
checkpoint.
