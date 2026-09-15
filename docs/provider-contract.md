# Temporal provider contract

The temporal provider contract is aniflow's provider-native boundary for
specialized frame, audio, timed-text, whole-video, inspection, validation,
assembly, delivery, and observation capabilities. It lets aniflow inspect and
reason about a provider before executing it while keeping provider algorithms
outside the temporal orchestration core.

This checkpoint defines data and validation only. It does not discover a
provider, select a fallback, launch a process, migrate pipeline v2 processors,
or claim that cancellation and progress enforcement are implemented.

## Contract set

| Contract | Purpose |
| --- | --- |
| `aniflow.provider-manifest/v1` | Inert provider identity, capability, artifact, requirement, behavior, lifecycle, and provenance declarations |
| `aniflow.provider-configuration/v1` | Exact provider/capability selection plus provider-schema identity, effective values, and their canonical digest |
| `aniflow.compatibility-fingerprint/v1` | Self-validating identity across inputs, pipeline and provider configuration, provider/capability versions, tools, codecs, models, and validated outputs |

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

The common envelope does not replace provider-specific schema validation. A
provider or future registry validates `values` against the exact referenced
schema before the envelope becomes resolved configuration.

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

flow's operator-controlled lock remains authoritative for cross-holon trust,
permission grants, precedence, replacement, and fallback. The aniflow manifest
cannot grant those rights. When aniflow runs standalone, later aniflow-owned
resolution and lock records provide equivalent local authority without changing
the provider declaration.

## Published schemas and examples

- [`provider-manifest-v1.schema.json`](contracts/provider-manifest-v1.schema.json)
- [`provider-configuration-v1.schema.json`](contracts/provider-configuration-v1.schema.json)
- [`compatibility-fingerprint-v1.schema.json`](contracts/compatibility-fingerprint-v1.schema.json)
- [`provider-manifest-v1.example.json`](contracts/examples/provider-manifest-v1.example.json)
- [`provider-configuration-v1.example.json`](contracts/examples/provider-configuration-v1.example.json)
- [`compatibility-fingerprint-v1.example.json`](contracts/examples/compatibility-fingerprint-v1.example.json)
- [`example-frame-configuration-v1.schema.json`](contracts/examples/example-frame-configuration-v1.schema.json)

The example is synthetic and does not register an executable provider.

## Deferred checkpoints

- `ANI-11.2` resolves exact providers, persists locks, and enforces bounded
  execution, availability, progress, cancellation, timeouts, and output limits.
- `ANI-11.3` adapts the pipeline v2 builtin processors and removes the deprecated
  renderflow handoff only in pipeline v3.
- `ANI-11.4` publishes the complete conformance corpus and extension-authoring
  guide.

Issues #8 and #13 consume the completed SDK rather than expanding this contract
checkpoint.
