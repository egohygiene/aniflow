# Authoring a temporal provider

This guide describes the supported local-provider boundary for Pipeline v3.
Start from the runnable [`provider-v1` conformance
bundle](../conformance/provider-v1/README.md), not from the non-runnable JSON
shape examples in [`contracts/examples`](contracts/examples/). The conformance
bundle keeps one manifest, provider-owned schema, effective configurations,
registrations, pipelines, executable, and inputs internally consistent.

Conformance here means that a provider follows aniflow's contract and process
ABI. It does not certify media quality, model safety, semantic correctness, or
fitness for a particular workflow.
The bundle is repository-local fixture and test evidence over the existing
public contracts and Pipeline v3 APIs; it does not introduce another contract
family, conformance-report schema, provider SDK surface, or repository.

## Current support boundary

| Provider family | Declared and resolved | Pipeline v3 execution evidence | Current qualification |
| --- | --- | --- | --- |
| Frame processor | Yes | `frame` profile | One directory artifact on each bound output port |
| Audio processor | Yes | `audio` profile | One file artifact on each bound output port |
| Whole-video processor | Yes | `whole-video` profile | One file artifact on each bound output port |
| Artifact validator | Yes | `artifact-validator` profile | Runs as an ordinary stage that emits immutable validation-evidence JSON |
| Stream inspector | Yes | First-party [audio inspection](audio-inspection.md), [signal provider v2](audio-signal-analysis.md), [stem lineage](audio-stem-lineage.md), [musical estimates](audio-musical-analysis.md), [observed transcription](audio-transcription.md), [reviewed-lyrics alignment](audio-lyrics-alignment.md) and [MIDI candidates](audio-midi.md) through the fixed provider ABI | Bounded PCM16 profiles; optional analyzers require exact local tool/model identities and preserve estimate/provenance limits; transcription and alignment use separate English/mono/16 kHz profiles; transcription has unavailable word timing/confidence, while alignment preserves supplied review evidence and proposes unreviewed candidate timing; MIDI uses a separate mono/22.05 kHz CPU profile with probabilistic notes, unavailable calibrated confidence and explicit SMF export; lineage retains direct-mix/full-stem limits; real model inference and native qualification require separate evidence |
| Temporal validator | Yes | Explicit inline temporal-media profile and pinned native adapter under #33 | Exact admitted CFR/zero-origin profile; authored, tests and native qualification unrun under #64 |
| Timed-text processor, assembler/encoder | Yes | No reference profile yet | Executable only when the stage fits the closed subset below; not qualified by this kit |
| Delivery provider | Yes | No reference profile | Pipeline v3 rejects any stage requesting `publish` authority |
| Lifecycle observer | Yes | No | Pipeline v3 execution rejects lifecycle-observer stages |

The executable Pipeline v3 subset currently requires:

- at least one input and at least one output for every stage;
- every output port declared with cardinality `one` and bound to exactly one
  artifact;
- an explicit `file` or `directory` kind and an output path beneath
  `artifacts/`;
- mandatory artifact integrity, with optional explicit provider-artifact or
  temporal-media gates described in [layered validation](layered-validation.md);
- no lifecycle-observer stage and no `publish` side effect.

The `artifact-validator` reference profile proves that a provider can inspect
an artifact and produce evidence. That evidence is a normal stage output. It
does not make the provider a Pipeline v3 validation gate, and it does not
replace aniflow's built-in artifact-integrity validation. The separate
[`validation-v1` bundle](../conformance/validation-v1/README.md) authors the
inline-gate protocol under #33: exact validator resolution, immutable contexts,
provider observations and aniflow-owned layered acceptance. Its tests are
unrun under #64. The native temporal adapter is also authored but unqualified;
it accepts the explicit bounded #32 CFR/zero-origin profile.

## Document and execution flow

Authoring one provider involves seven distinct boundaries:

1. Define a provider-owned JSON Schema and validate the intended settings
   against it.
2. Declare the schema's exact SHA-256 identity and the capability in an
   `aniflow.provider-manifest/v1` document.
3. Normalize defaults and settings, then create an
   `aniflow.provider-configuration/v1` envelope.
4. Bind that manifest, configuration, and executable through a confined
   `aniflow.provider-registration/v1` locator.
5. Reference the registration and capability from an `aniflow.pipeline/v3`
   stage and grant only its required effects.
6. Accept the fixed `--aniflow-invocation <absolute-request-path>` process ABI
   and write only the requested immutable outputs.
7. Let aniflow validate the output tree and publish the execution report,
   built-in validation evidence, checkpoint, and run-manifest revision.

Each document has one job. A manifest is inert declaration data, a
registration is explicit local authority, a provider lock is the exact result
of resolution, and an invocation is ephemeral operational data. None of them
is an operating-system sandbox.

## 1. Own and validate the configuration schema

Keep the provider-specific settings in a closed JSON Schema. Give the schema a
versioned ID of the form `aniflow.<name>/v<major>` and a semantic version. Hash
the exact schema file bytes with SHA-256; whitespace and line-ending changes
therefore produce a new digest.

The embedding application is responsible for applying defaults, normalizing
values, and validating the resulting `values` object against those exact
schema bytes. aniflow verifies that the schema ID, semantic version, and digest
agree across the manifest, capability, and effective-configuration envelope.
It does not interpret arbitrary provider-owned JSON Schema keywords and a
registration document does not locate or validate the schema on the caller's
behalf.

The reference bundle verifies the published schema digest, and its executable
independently enforces the small closed values contract it consumes. That
fixture check is not a general JSON Schema evaluator; a real embedding
application must perform provider-owned schema validation before registration.

## 2. Declare the provider and capability

Create an `aniflow.provider-manifest/v1` document using
[`provider-manifest-v1.schema.json`](contracts/provider-manifest-v1.schema.json)
as the transport shape. The Rust `ProviderManifest::from_json_slice` parser
also enforces semantic invariants that JSON Schema alone cannot express.

For each capability:

- use a lowercase dotted provider ID and semantic provider/capability
  versions;
- reference one of the manifest's exact configuration-schema identities;
- declare every immutable input and output port with its artifact type, role,
  optional stream role, cardinality, and batching behavior;
- list every required tool, codec, model, and minimum host observation;
- keep `network` and `gpu` requirements consistent with their side-effect
  declarations;
- describe determinism, fidelity, cacheability, content changes, progress,
  and cancellation honestly; and
- declare the complete v1 provenance promise.

The provider lock's `manifest_sha256` covers the validated manifest value under
aniflow canonical JSON, so formatting-only manifest changes do not change that
identity. This differs intentionally from a configuration schema's digest,
which identifies its exact published file bytes.

Side effects are requests, not permissions. aniflow considers them authorized
only when the caller explicitly grants them during planning. That policy gate
does not technically prevent a process from using the user's network, and
declaring only filesystem read/write does not confine the process to invocation
paths. The executable still runs with the user's operating-system authority.
Use normal operating-system isolation when the provider is not fully trusted.

## 3. Publish an effective configuration

Create one `aniflow.provider-configuration/v1` envelope for the exact provider
and capability selection. The `configuration_schema` reference must match the
manifest and capability byte for byte. `effective_configuration_sha256` is the
SHA-256 of only the normalized `values` object under
`aniflow.canonical-json/v1`:

- object keys are recursively sorted;
- array order is preserved;
- insignificant whitespace is omitted; and
- JSON scalars use `serde_json` encoding.

Rust authors should prefer `ProviderConfiguration::new`, which derives and
validates this digest. Other implementations must reproduce the same canonical
encoding and should verify the resulting document with
`ProviderConfiguration::from_json_slice` through a small Rust integration
test.

Changing a setting, a default that becomes effective, the provider/capability
selection, or the provider-owned schema identity requires a new coherent
configuration envelope. Do not edit a digest field by hand to make a stale
document parse.

## 4. Register exact local authority

Create an `aniflow.provider-registration/v1` document for each capability and
effective configuration that should be selectable. Its manifest,
configuration, and executable locators are portable relative paths resolved
from the registration document's directory.

`ProviderRegistrationDocument::load` binds the parsed document to its
canonical parent directory. `into_registration` then resolves every locator
and requires each target to remain beneath that directory after symbolic-link
resolution. An absolute locator, `..` traversal, escaping symlink, missing
target, or unusable executable fails before planning can select the provider.
The registry never searches `PATH` and never runs discovery commands.

Give every registration a stable unique `registration_id` and
`implementation_id`. List the exact observed tools, codecs, and models needed
to satisfy the manifest. Resolution checks versions and optional component
digests, hashes the executable, and records all selected identities in the
provider lock.

The manifest and registration still do not grant execution authority. The
caller supplies a replacement/primary/fallback order, host observations,
allowed side effects, and offline policy. Denied effects, missing resources,
component mismatch, offline incompatibility, or an unavailable executable make
the candidate unavailable before launch. Fallback is allowed only at that
pre-launch boundary.

## 5. Implement the direct invocation ABI

Pipeline v3 launches the exact registered executable without a shell:

```text
<executable> --aniflow-invocation <absolute-request-path>
```

The request is a closed `aniflow.provider-invocation/v1` document containing:

- `execution_semantics: aniflow.provider-invocation/direct-argv/v1`;
- the stage ID and exact provider-lock digest;
- the complete validated effective configuration; and
- ordered input and output bindings with port, artifact identity, type, role,
  optional stream role, filesystem kind, and absolute local path.

A provider should reject any other argument shape, unsupported schema or
execution semantics, unknown field, mismatched configuration, unexpected port,
artifact type/role, or filesystem kind. Treat all input paths as immutable.
Create exactly the requested outputs at their bound paths and do not create
undeclared siblings. A successful exit is only a candidate result; aniflow
independently rejects missing, empty, wrong-kind, symlinked, overlapping,
unexpected, or over-limit output trees.

The request file is permission-private temporary operational data and can
contain sensitive local paths and configuration values. aniflow removes it
after the process returns, but abnormal host termination can leave it for the
operating system's temporary-storage cleanup. Do not copy it into output,
logs, telemetry, or durable provider state.

There is no alternate positional-argument, standard-input, environment, or
shell-command provider ABI in v1. stdout and stderr are bounded diagnostics,
not result channels.

## 6. Prove the complete boundary

Run the repository's coherent reference suite:

```bash
cargo test --test provider_conformance --locked
```

The suite covers all four reference profiles from manifest and schema identity
through configuration, confined registration, offline deterministic
resolution, exact lock, invocation, report, checkpoint, status, and resume.
It also proves that a compatible checkpoint is reused without relaunching the
provider, denied effects stop before launch, and missing-output and nonzero-exit
providers cannot establish completion.

Use [`conformance/provider-v1/README.md`](../conformance/provider-v1/README.md)
for the exact fixture layout, profiles, and validation command. When adapting
it:

1. keep one profile coherent before adding another;
2. recompute the raw schema digest and canonical configuration digest;
3. load the registration through `ProviderRegistrationDocument`, rather than
   constructing authority from unverified paths;
4. resolve and run through Pipeline v3 so the direct invocation ABI is tested;
5. assert the lock, execution report, built-in validation, checkpoint, final
   status, and resume reuse; and
6. include at least one pre-launch policy rejection and one post-launch output
   or process failure.

Issue #24 owns the exhaustive malicious/adversarial corpus. A provider change
does not need to duplicate that entire matrix, but it must retain a focused
proof for every boundary it changes.

## Evidence, compatibility, and observability

aniflow owns the execution report and checkpoint acceptance decision. Exit
code zero, provider-authored evidence, or an
`aniflow.compatibility-fingerprint/v1` document cannot mark a stage complete by
itself.

The compatibility-fingerprint contract is available to applications that need
an explicit portable summary across inputs, configuration, provider,
components, outputs, and validation evidence. The current Pipeline v3 executor
does not automatically construct or persist that document. Its run-local
checkpoint identity instead binds the resolved stage, exact provider lock,
execution semantics and bounds, observed inputs and dependency outputs,
execution report, observed outputs, required validation evidence and the
layered acceptance-semantics identity. Gate providers cannot bypass host
acceptance by emitting their own completion field.

Logs, human diagnostics, timestamps, temporary paths, and telemetry may help
operators observe a run, but they are deliberately excluded from canonical
plan, provider-lock, compatibility-fingerprint, checkpoint, and artifact
identity. Do not make provider correctness or cache identity depend on a
logging backend or telemetry exporter.

## Platform evidence

| Surface | Evidence in this repository |
| --- | --- |
| Published JSON-contract validation | Linux CI |
| Rust crate and contract models | Compiled on Linux and macOS; all-target tests run on Linux stable/MSRV CI |
| Reference provider process | Python 3.10+ standard library with a Unix executable/shebang boundary |
| Full provider-conformance execution | Linux CI and local Unix-compatible hosts |
| macOS provider-conformance execution | Not currently a CI claim |
| Windows provider-conformance execution | Not currently supported by the reference executable or proven by CI |

The JSON documents are portable data, but a successful live run also depends
on executable permissions, path behavior, process-tree handling, and the host
runtime. Do not infer platform support from schema validation or compilation
alone.

## Versioning checklist

Before publishing a provider revision:

- change the provider or capability semantic version when implementation or
  declared behavior changes require it;
- change the configuration-schema ID major only for a breaking document-shape
  revision, and update its semantic version and raw-byte digest as appropriate;
- regenerate every effective configuration affected by new defaults or
  normalization;
- keep registration component observations and implementation identity exact;
- test offline and least-authority resolution;
- test missing/malformed output and a nonzero provider exit;
- prove run status and resume behavior from accepted immutable evidence; and
- document any quality, model, codec, hardware, or platform limits that the
  contract cannot prove.

See the [temporal provider contract](provider-contract.md) for the normative
invariants and the [public contract index](contracts/README.md) for schemas,
machine envelopes, exit codes, and compatibility policy.
