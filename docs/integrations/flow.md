# Integrating aniflow with flow

`aniflow` remains an independent temporal-media holon. `flow` may orchestrate
it, but `aniflow` never imports `flow`, `optiflow`, or `renderflow` code and does
not select downstream suite policy.

## Pipeline v3 planning boundary

Use `plan_v3` for file-based library/CLI parity, or
`PipelineV3Configuration` and `resolve_pipeline_v3` with a prebuilt registry,
when `flow` needs a reviewable temporal plan before allowing expensive work.
`flow` supplies all external evidence explicitly:

- one local path for every declared input artifact ID;
- `aniflow.provider-registration/v1` locators for every candidate it makes
  available;
- observed host CPU threads, memory, storage, GPU, and network availability;
- the exact allowed side effects; and
- offline mode.

aniflow validates the ordered temporal stage graph, artifact/port bindings,
expected outputs, validation obligations, and capability compatibility. It
then applies its existing deterministic replacement, primary, and ordered
fallback resolution. `flow` must not reorder stages or candidates, substitute a
different provider after resolution, or infer missing evidence.

| aniflow v3 evidence | flow consumption |
| --- | --- |
| Normalized configuration and input content identities | Preserve as authored temporal intent and immutable input provenance |
| Ordered stages and dependencies | Embed as the aniflow-owned portion of a wider cross-holon plan |
| Capability declarations and port bindings | Map to suite capability and artifact references without changing temporal semantics |
| Ordered resolution attempts | Retain as causal selection evidence, including rejected candidates |
| Exact provider locks | Review or project into flow policy; never silently replace or weaken them |
| Expected artifacts, filesystem kinds, and validations | Treat as execution obligations, never proof of completion |
| Typed planning failure | Project the stable code, stage, and attempts without parsing human prose |

The returned `aniflow.pipeline-plan/v1` document is self-validating canonical
evidence. Local input and registration paths are not part of its identity;
observed input content, caller-declared input semantics, the full supplied host
observation, provider, capability, configuration, implementation, component,
authority, offline, artifact, and validation identities are. Logs and telemetry
are operational signals only and must not affect its digest.

Planning is read-only. It performs no discovery, process launch, network
access, workspace, or artifact write. A successful plan therefore authorizes
no claim that output exists. `flow` should store or compare the complete
versioned plan rather than reconstructing it from private aniflow types.

## Pipeline v3 execution and resume boundary

Pipeline v3 execution is a separate, bounded application boundary. `flow`
passes the validated plan, immutable input bindings, and an explicit
`ProviderRegistry` into `PipelineV3RunRequest::new`, then calls `run_v3`.
Resume uses `PipelineV3ResumeRequest::new` with the run directory, freshly
bound inputs, and a newly built explicit registry before calling `resume_v3`.
`status_v3` is read-only.

Before mutation or launch, aniflow re-resolves every selected provider and
requires complete equality with the plan's exact lock. It then invokes the
provider through `aniflow.provider-invocation/v1`, observes candidate output,
runs the required validation, and publishes an immutable checkpoint last.
`flow` must not supply a replacement/fallback provider, rewrite the plan, infer
completion from process exit, or mutate the run workspace.

The latest `aniflow.pipeline-run/v1` revision is an append-only projection of
run and stage state. An `aniflow.stage-checkpoint/v1` record proves the stage
plan, provider lock, ordered inputs and dependencies, execution report,
file-or-directory output content, and validation result that were accepted.
On resume, aniflow reuses only compatible checkpoints. A missing or changed
output invalidates the affected stage and its downstream consumers; changed
source content or provider authority fails closed rather than silently
replanning.

The first executable subset permits one artifact per output port and only
`aniflow.validation/artifact-integrity/v1`. Unsupported cardinality,
validation contracts, lifecycle-observer stages, and publish authority fail
before workspace creation or provider launch. This boundary does not provide
cross-run caching or arbitrary DAG execution.

## Preferred Rust boundary

Use the released library facade for in-process integration:

```toml
[dependencies]
aniflow = "0.3"
```

Until `0.3.0` is published, pin the exact reviewed Git revision rather than a
moving branch:

```toml
[dependencies]
aniflow = { git = "https://github.com/egohygiene/aniflow", rev = "<commit>" }
```

The existing Pipeline v2 adapter remains a supported compatibility boundary.
A minimal v2 adapter owns only suite translation:

```rust,no_run
use aniflow::{ErrorCategory, Result, RunOutcome, RunRequest};

pub fn execute_aniflow(request: RunRequest) -> Result<RunOutcome> {
    aniflow::run(request).map_err(|error| {
        match error.category() {
            ErrorCategory::Input | ErrorCategory::Configuration => {
                // flow may translate this into its own invalid-request category.
            }
            _ => {
                // flow retains the aniflow category and message as causal evidence.
            }
        }
        error
    })
}
```

`flow` should treat `RunOutcome` as the handoff locator and query `RunStatus`
for aniflow-owned operational evidence. It should not deserialize private run
manifests as a substitute for the public facade or mutate an aniflow workspace.

New integrations should prefer the Pipeline v3 request types described above.
They expose versioned run manifests and checkpoint decisions without requiring
`flow` to interpret Pipeline v2 marker files.

## Process boundary

When isolation requires the CLI, invoke commands with `--output json`. Parse
`MachineEnvelope<T>` and reject unknown `schema_version` values. Successful
envelopes arrive on standard output; typed error envelopes arrive on standard
error with the documented exit code.

The generic envelope is stable at schema version `1`. Command result structures
follow the `0.3.x` SemVer line, so a process adapter should declare the supported
aniflow range and fail closed outside it.

For Pipeline v3 planning, invoke `plan-v3` with repeated `--input ID=PATH` and
`--provider-registration FILE` values, explicit host observations and allowed
side effects, optional availability flags, `--offline` when required, and
`--output json`. A successful `plan_v3` envelope contains an independently
versioned `aniflow.pipeline-plan/v1`. A failed envelope may contain an
`aniflow.pipeline-planning-failure/v1` result with the same typed stage and
provider-attempt evidence exposed by the library.

For execution, invoke `run-v3` with the same planning arguments and optional
`--output-directory`, plus any explicitly chosen provider bounds, and
`--output json`. To resume, invoke `resume-v3 <run-directory>` with every
original `--input ID=PATH` binding and the explicit
`--provider-registration FILE` documents needed to re-establish the exact
locks. Host observations and effect policy come from the immutable plan rather
than caller defaults. Invoke `status-v3 <run-directory> --output json` for a
strictly read-only projection. The corresponding envelope command names are
`run_v3`, `resume_v3`, and `status_v3`.

If `run_v3` or `resume_v3` fails after its durable `Started` boundary, the
error envelope contains an `aniflow.pipeline-run-recovery/v1` result. `flow`
should retain its `run_directory` for status inspection or an explicitly
authorized resume. `run_manifest` is included only when aniflow can validate
the newest manifest and its referenced authority at error-render time. A
preflight failure has no result and therefore exposes no newly created run.

## Ownership boundary

`flow` owns:

- suite-level selection and sequencing;
- compatibility among released holon versions;
- passing aniflow's validated master to later capabilities;
- suite-wide provenance and policy;
- presenting cross-tool progress and failures.

`aniflow` owns:

- source inspection and temporal interpretation;
- its pipeline plan and stage order;
- isolated run workspaces and checkpoints;
- reconstruction and master validation;
- aniflow run status, artifacts, and causal errors.

For short-segment orchestration, flow selects and sequences the stable
`media.video.segment/v1` and `media.video.reconstruct/v1` capabilities. aniflow
alone decides source-time boundaries, writes the segment manifest, validates
the reusable prefix during resume, and validates reconstruction. flow passes
manifest and output locators; it must not rewrite segment ordering or infer
successful reconstruction from child exit status.

renderflow may transcode a complete video artifact through a bounded adapter.
It does not own temporal decomposition, segment ordering, or reconstruction.
Likewise, aniflow does not choose publication derivatives or import renderflow.

The pipeline v2 `renderflow` field is a deprecated compatibility seam. New
`flow` integration must not depend on it. Pipeline v3 configuration and
planning reject that selection, and Pipeline v3 execution cannot reintroduce
it; `flow` sequences renderflow as a separate holon when policy calls for it.
