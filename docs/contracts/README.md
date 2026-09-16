# aniflow public contracts

This directory defines the machine boundary intended for scripts, `flow`, and
other independent consumers. Human console text is presentation and is not a
compatibility contract.

## Temporal provider contracts

The provider-native v1 contract set covers declarations, compatibility,
standalone resolution authority, lifecycle observations, and execution evidence:

- [`provider-manifest-v1.schema.json`](provider-manifest-v1.schema.json) defines
  identity, temporal capabilities, typed immutable ports, requirements,
  behavior, lifecycle support, side effects, and provenance promises.
- [`provider-configuration-v1.schema.json`](provider-configuration-v1.schema.json)
  binds effective values to exact provider, capability, and provider-owned
  configuration-schema identities.
- [`compatibility-fingerprint-v1.schema.json`](compatibility-fingerprint-v1.schema.json)
  binds input, configuration, provider, tool, codec, model, and validated-output
  evidence through a self-validating canonical SHA-256 digest.
- [`provider-lock-v1.schema.json`](provider-lock-v1.schema.json) binds the exact
  local selection, implementation digest, components, authorized effects, and
  offline state.
- [`provider-event-v1.schema.json`](provider-event-v1.schema.json) defines
  ordered lifecycle observations for one lock.
- [`provider-execution-report-v1.schema.json`](provider-execution-report-v1.schema.json)
  retains applied bounds, termination, redacted captures, validated outputs,
  events, and a self-validating report digest.
- [`pipeline-v2-processor-configuration-v1.schema.json`](pipeline-v2-processor-configuration-v1.schema.json)
  defines the provider-owned normalized configuration used by pipeline v2's
  frame, batch, audio, and whole-video compatibility adapters.

Canonical synthetic examples live in [`examples/`](examples/). The public Rust
types expose constructors and parsers for the same shapes. See the
[temporal provider contract](../provider-contract.md) for invariants, canonical
hashing, deterministic resolution, bounded runtime behavior, output acceptance,
flow mapping, and explicit isolation limits.

## Pipeline v3 planning contracts

Pipeline v3 has four independently versioned public planning documents:

- [`pipeline-v3-configuration-v1.schema.json`](pipeline-v3-configuration-v1.schema.json)
  defines strict authored `aniflow.pipeline/v3` intent.
- [`provider-registration-v1.schema.json`](provider-registration-v1.schema.json)
  defines an explicit `aniflow.provider-registration/v1` local locator for a
  manifest, effective configuration, executable, implementation, and observed
  components.
- [`pipeline-v3-plan-v1.schema.json`](pipeline-v3-plan-v1.schema.json) defines
  the self-validating `aniflow.pipeline-plan/v1` result.
- [`pipeline-v3-planning-failure-v1.schema.json`](pipeline-v3-planning-failure-v1.schema.json)
  defines typed `aniflow.pipeline-planning-failure/v1` diagnostics.

Matching canonical examples are published in [`examples/`](examples/). The
public Rust file boundary is `plan_v3`; callers with an already constructed
`PipelineV3Configuration` and `ProviderRegistry` can use the lower-level
`resolve_pipeline_v3`. The thin process boundary is `plan-v3`, which delegates
to the file facade.

The plan contains ordered stages, dependencies, port/artifact bindings,
expected artifacts, validations, requested capability declarations,
replacement/primary/fallback policy, deterministic resolution attempts, and
exact provider locks. It carries `algorithm: sha256` and
`canonicalization: aniflow.canonical-json/v1`; `plan_sha256` covers only the
`payload` serialized under that encoding. Object keys are recursively sorted,
arrays retain their declared order, insignificant whitespace is omitted, and
scalar encoding follows `serde_json`. Parsing recomputes the digest and rejects
tampering.

Local input and registration-locator paths are never canonical identity.
Observed input content identity, caller-declared input type and roles, the full
supplied host observation, and provider, capability, configuration,
implementation/executable, tool, codec, model, authorization, offline,
artifact, and validation identity are covered. Timestamps, run IDs, temporary
roots, the authored pipeline description, human diagnostic prose, logs, and
telemetry are excluded.

Planning itself is read-only. It uses only explicitly supplied `ID=PATH`
inputs, registration documents, host-resource observations, side-effect
grants, and offline state. It performs no implicit discovery, process launch,
network access, workspace creation, or output write. Pipeline v3 execution and
resume are separate APIs and commands that consume the exact resulting plan.
Pipeline v2 `plan`, `run`, and `resume` retain their existing contracts. The
closed v3 configuration rejects the deprecated v2 renderflow handoff.

## Pipeline v3 execution and recovery contracts

The bounded Pipeline v3 executor adds three independently versioned documents:

- [`provider-invocation-v1.schema.json`](provider-invocation-v1.schema.json)
  defines the closed `aniflow.provider-invocation/v1` request passed as
  `<executable> --aniflow-invocation <request-path>`.
- [`pipeline-run-v1.schema.json`](pipeline-run-v1.schema.json) defines one
  self-validating `aniflow.pipeline-run/v1` revision in an append-only run
  history.
- [`stage-checkpoint-v1.schema.json`](stage-checkpoint-v1.schema.json) defines
  immutable `aniflow.stage-checkpoint/v1` evidence for one accepted stage
  attempt.

Matching canonical examples live in [`examples/`](examples/). The invocation
document is ephemeral operational data: it carries effective configuration and
typed file-or-directory input/output paths, while runtime paths are excluded
from durable checkpoint compatibility identity. Providers receive only the
fixed direct-argument invocation shape; aniflow never synthesizes a shell
command.

The run manifest references the immutable plan, retains ordered stage states
and compatibility decisions, and chains each revision to its predecessor.
Every checkpoint binds the relevant stage-plan identity, invocation and
execution semantics, exact provider lock, ordered input/dependency evidence,
provider execution report, observed outputs, and required validation results.
Parsing recomputes the canonical SHA-256 digest and rejects tampering.

Before an initial run or resume mutates a workspace, aniflow rebuilds the
registry only from explicit registration documents, re-resolves each selected
provider, and requires its full lock to equal the plan. Resume re-observes the
explicit inputs and verifies accepted output content. Compatible checkpoints
are reused; changed or missing accepted output invalidates the affected stage
and its downstream consumers. Changed source identity, changed provider
authority, unsupported or tampered state, and a different plan fail closed.
There is no fallback after execution starts and no cross-run checkpoint import.

The initial executable subset is intentionally closed: one artifact per output
port, explicit file-or-directory output kind, ordered stages, and only
`aniflow.validation/artifact-integrity/v1`. Unsupported cardinality,
validation contracts, lifecycle-observer stages, and publish authority fail
before workspace creation or provider launch. `status_v3` is read-only.
Pipeline v2 state, resume, and completion markers do not consume or emulate
these contracts.

## Machine envelope v1

Every command accepts `--output json` and emits one
`aniflow.machine-envelope/v1`-equivalent JSON document:

- successful documents are written to standard output;
- failed documents are written to standard error;
- `schema_version` is currently `1`;
- `command` identifies the invoked operation;
- `status` is either `success` or `error`;
- `result` contains the command-specific public result on success;
- `error.category` and `error.message` describe failure without requiring prose
  parsing.

Consumers must reject unsupported `schema_version` values. The Rust
`MachineEnvelope::from_json_slice` helper performs that check.

Short-segment operations use the same envelope with the command names
`segment_plan`, `segment_run`, `segment_resume`, and `segment_reconstruct`.
Their durable result documents are independently versioned as
`aniflow.segment-plan/v1`, `aniflow.segment-manifest/v1`, and
`aniflow.reconstruction-report/v1`.

- [`segment-plan-v1.schema.json`](segment-plan-v1.schema.json)
- [`segment-manifest-v1.schema.json`](segment-manifest-v1.schema.json)
- [`reconstruction-report-v1.schema.json`](reconstruction-report-v1.schema.json)

Pipeline v3 planning uses command name `plan_v3`. On success, `result` is the
resolved `aniflow.pipeline-plan/v1` document. A planning failure may include an
`aniflow.pipeline-planning-failure/v1` result alongside the normal envelope
error, retaining the stage and ordered provider-resolution attempts without
requiring message parsing.

Pipeline v3 execution uses command names `run_v3`, `resume_v3`, and
`status_v3`. Successful run and resume results use
[`aniflow.pipeline-run-outcome/v1`](pipeline-run-outcome-v1.schema.json) and
retain the run directory, plan digest,
latest manifest locator, outputs, and executed/reused stage IDs. A successful
status result is the newest validated `aniflow.pipeline-run/v1` manifest.
Failures keep the normal typed envelope category. Once durable execution has
started, a run or resume failure also carries an
[`aniflow.pipeline-run-recovery/v1`](pipeline-run-recovery-v1.schema.json)
result containing the run directory and an optional latest validated manifest
locator. The manifest field is omitted if current state cannot be validated;
failures before workspace startup retain no result. `resume_v3` requires fresh
explicit input and registration authority. `status_v3` requires only the run
directory and never repairs or creates state.

The generic envelope shape is described by
[`machine-envelope-v1.schema.json`](machine-envelope-v1.schema.json). Result
schemas remain tied to the `0.3.x` public Rust types until independently
versioned command-result schemas are justified by real `flow` integration.
The Pipeline v3 plan, planning-failure, run-outcome, run-recovery,
run-manifest, and stage-checkpoint values above are already versioned
exceptions.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | success |
| `2` | CLI usage or argument parsing failure |
| `3` | invalid or missing input |
| `4` | invalid or unsupported configuration |
| `5` | missing or unusable dependency |
| `6` | media inspection or interpretation failure |
| `7` | pipeline execution failure |
| `8` | invalid or incompatible run state |
| `9` | filesystem or other I/O failure |
| `70` | internal serialization or invariant failure |

The public `ErrorCategory::exit_code` mapping and CLI contract tests enforce
these values.

## Naming and compatibility

- Product names are lowercase: `aniflow`, `flow`, `optiflow`, and `renderflow`.
- New CLI options use complete long-form names.
- The former `run --output-dir` spelling remains a visible compatibility alias
  for `--output-directory` throughout `0.3.x`.
- The former `inspect --json` spelling retains its raw v0.2 inspection object
  throughout `0.3.x`; new integrations use the versioned `--output json`
  envelope.
- Additive result fields are permitted in `0.3.x`; removing or changing the
  meaning of fields requires a schema or SemVer change.
- Paths are serialized using Rust path serialization for the host platform.
- Unknown envelope versions are rejected rather than interpreted loosely.
- Canonical JSON v1 sorts object keys recursively and preserves array order;
  changing these rules requires a new contract revision.

Breaking compatibility requires an ADR update, migration notes, and contract
fixtures demonstrating both rejection and the supported replacement.
