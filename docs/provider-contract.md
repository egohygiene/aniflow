# Temporal provider contract

The temporal provider contract is aniflow's provider-native boundary for
specialized frame, audio, timed-text, whole-video, inspection, validation,
assembly, delivery, and observation capabilities. It lets aniflow inspect and
reason about a provider before executing it while keeping provider algorithms
outside the temporal orchestration core.

The contract separates inert declarations from explicit local registration,
deterministic resolution, authority-bearing locks, bounded execution, and
validated results. Pipeline v2 processors use adapters over this boundary; a
provider process is still configured local code, not untrusted sandboxed code.

## Contract set

| Contract | Purpose |
| --- | --- |
| `aniflow.provider-manifest/v1` | Inert provider identity, capability, artifact, requirement, behavior, lifecycle, and provenance declarations |
| `aniflow.provider-configuration/v1` | Exact provider/capability selection plus provider-schema identity, effective values, and their canonical digest |
| `aniflow.compatibility-fingerprint/v1` | Self-validating identity across inputs, pipeline and provider configuration, provider/capability versions, tools, codecs, models, and validated outputs |
| `aniflow.provider-lock/v1` | Exact standalone selection, implementation and component identities, granted effects, and offline state |
| `aniflow.provider-event/v1` | Ordered lifecycle observations scoped to one provider lock |
| `aniflow.provider-execution-report/v1` | Applied bounds, process termination, redacted diagnostics, validated outputs, events, and terminal outcome |
| `aniflow.pipeline-v2-processor.configuration/v1` | Normalized legacy processor kind, identity, arguments, options, and execution limits used by the pipeline v2 adapters |
| `aniflow.provider-registration/v1` | Explicit local locators that bind a registration ID to manifest, configuration, executable, implementation, and observed component inputs |
| `aniflow.pipeline/v3` | Strict authored Pipeline v3 stages, dependencies, artifact bindings, provider policy, and validation obligations |
| `aniflow.pipeline-plan/v1` | Canonical read-only resolution result with exact locks and ordered resolution evidence |
| `aniflow.pipeline-planning-failure/v1` | Typed planning diagnostic with affected stage and provider attempts when applicable |

The public Rust models and their parsers use the same field names and reject
unknown fields and unknown contract identifiers. Provider and capability
versions use semantic versioning. A contract major and a provider/capability
semantic version are intentionally separate: the contract major governs the
document shape, while semantic versions identify implementations.

The pipeline v2 configuration entry is a provider-owned JSON Schema rather than
a second common envelope. Its exact bytes are identified from each generated
manifest and effective configuration by semantic version and SHA-256 digest.

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

For the portable `plan-v3` boundary, an
`aniflow.provider-registration/v1` JSON document is an explicit locator, not a
provider declaration or grant. It contains a unique registration ID; relative
paths to the manifest, effective configuration, and executable; a stable
implementation ID; and observed exact tool, codec, and model components. Paths
are resolved relative to the locator document, must remain confined beneath its
directory after symbolic-link resolution, and follow portable path rules. They
are excluded from plan identity, while the verified documents, executable
digest, implementation, and unique sorted component identities populate the
existing registry and exact provider lock.

## Pipeline v3 planning reuse

`PipelineV3Configuration` and `resolve_pipeline_v3` build on this provider
contract instead of creating a parallel resolver. Each authored stage requests
a capability/version and names an optional replacement, a primary, and ordered
fallback registration IDs. Resolution uses `ProviderRegistry` with the caller's
explicit host-resource observations, side-effect grants, and offline mode.

The resolved `aniflow.pipeline-plan/v1` document retains the complete capability
declaration, authored selection policy, ordered resolution attempts, and exact
`aniflow.provider-lock/v1` selected for each stage. A rejected plan uses
`aniflow.pipeline-planning-failure/v1`, preserving the affected stage and typed
candidate availability reasons. An unavailable candidate is never rewritten as
a generic planning success, and no fallback begins after execution because this
planner never executes providers.

The plan names `aniflow.canonical-json/v1`; its `plan_sha256` covers the payload
under that encoding. Object keys are recursively sorted, arrays retain
meaningful order, insignificant whitespace is omitted, and JSON scalars use
`serde_json` encoding. Exact
manifest, configuration-schema, effective-configuration, provider,
capability, implementation/executable, tool, codec, model, observed input
content, caller-declared input semantics, artifact, validation, authority,
offline, and the full host-resource observation supplied for planning are
retained.
Locator paths, temporary roots, timestamps, run IDs, diagnostics, logging, and
telemetry are excluded from canonical identity.

Planning only reads and hashes explicit inputs and provider evidence. It does
not discover providers or plugins, inspect `PATH`, use the network, launch a
provider, create a run workspace, write artifacts, or imply that expected
outputs were produced. Pipeline v3 execution, content-aware state, checkpoint
reuse, recovery, and resume remain deferred. Pipeline v2 planning, execution,
and resume retain their existing compatibility behavior.

Pipeline v3 configuration has no renderflow selection. The closed
`aniflow.pipeline/v3` shape rejects that v2 compatibility field; `flow` owns any
cross-holon sequencing.

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

## Pipeline v2 adapters

The existing `external`, `upscayl_ncnn`, and
`gemini_watermark_remover` frame processors, plus generic audio and whole-video
processors, are normalized into explicitly registered local providers. They
retain their pipeline v2 identifiers, arguments, ordering, concurrency, batch
behavior, output extensions, and compact Gemini decision metadata.

Pipeline v2 accepts bare command names for compatibility. The adapter resolves
one candidate from the caller's `PATH`, canonicalizes it to an absolute path,
and passes only that candidate to `ProviderRegistry`; the registry does not
discover alternatives. Relative command paths are resolved from the process
working directory. An explicit upscayl model path binds the selected `.param`
and `.bin` files into component evidence.

Each enabled processor stage persists one provider lock and a report for every
invocation beneath the run's `providers/` directory. Reports are retained for
nonzero exit, cancellation, timeout, missing output, and other runtime failures.
After runtime validation, aniflow applies processor-specific rules: frame files
must parse as PNG, and directory batches must preserve exact frame count and
names. Only a fully validated temporary result is promoted into the stage.

The deprecated pipeline v2 renderflow handoff remains on its compatibility
path. It is cross-holon orchestration rather than a temporal processor, and its
configuration is absent from and rejected by Pipeline v3.

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
| Pipeline v3 configuration | Temporal stage intent retained by aniflow, not rewritten by flow |
| Resolved Pipeline v3 plan | Reviewable ordered stages, exact locks, attempts, artifacts, and validations for suite orchestration |
| Pipeline v3 planning failure | Typed causal evidence that flow may project without parsing prose |

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
- [`pipeline-v2-processor-configuration-v1.schema.json`](contracts/pipeline-v2-processor-configuration-v1.schema.json)
- [`provider-registration-v1.schema.json`](contracts/provider-registration-v1.schema.json)
- [`pipeline-v3-configuration-v1.schema.json`](contracts/pipeline-v3-configuration-v1.schema.json)
- [`pipeline-v3-plan-v1.schema.json`](contracts/pipeline-v3-plan-v1.schema.json)
- [`pipeline-v3-planning-failure-v1.schema.json`](contracts/pipeline-v3-planning-failure-v1.schema.json)
- [`provider-manifest-v1.example.json`](contracts/examples/provider-manifest-v1.example.json)
- [`provider-configuration-v1.example.json`](contracts/examples/provider-configuration-v1.example.json)
- [`compatibility-fingerprint-v1.example.json`](contracts/examples/compatibility-fingerprint-v1.example.json)
- [`provider-lock-v1.example.json`](contracts/examples/provider-lock-v1.example.json)
- [`provider-event-v1.example.json`](contracts/examples/provider-event-v1.example.json)
- [`provider-execution-report-v1.example.json`](contracts/examples/provider-execution-report-v1.example.json)
- [`provider-registration-v1.example.json`](contracts/examples/provider-registration-v1.example.json)
- [`pipeline-v3-configuration-v1.example.json`](contracts/examples/pipeline-v3-configuration-v1.example.json)
- [`pipeline-v3-plan-v1.example.json`](contracts/examples/pipeline-v3-plan-v1.example.json)
- [`pipeline-v3-planning-failure-v1.example.json`](contracts/examples/pipeline-v3-planning-failure-v1.example.json)
- [`example-frame-configuration-v1.schema.json`](contracts/examples/example-frame-configuration-v1.schema.json)

The examples are synthetic and contain placeholder implementation and artifact
digests. Their enclosing lock and report digests are nevertheless canonical and
self-validating.

## Deferred checkpoints

- The adversarial provider-conformance corpus remains tracked by issue #24; a
  complete extension-authoring guide requires that corpus or another separately
  authorized checkpoint.
- Pipeline v3 execution and content-aware state replace blind completion
  markers with compatibility-aware checkpoint, recovery, and resume evidence.

Provider-native item-count or fractional progress ingestion, remote providers,
kernel/container isolation, and Pipeline v3 checkpoint reuse remain deferred.

Issues #8 and #13 consume the completed SDK rather than expanding this contract
checkpoint.
