# Integrating aniflow with flow

`aniflow` remains an independent temporal-media holon. `flow` may orchestrate
it, but `aniflow` never imports `flow`, `optiflow`, or `renderflow` code and does
not select downstream suite policy.

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

A minimal adapter owns only suite translation:

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

## Process boundary

When isolation requires the CLI, invoke commands with `--output json`. Parse
`MachineEnvelope<T>` and reject unknown `schema_version` values. Successful
envelopes arrive on standard output; typed error envelopes arrive on standard
error with the documented exit code.

The generic envelope is stable at schema version `1`. Command result structures
follow the `0.3.x` SemVer line, so a process adapter should declare the supported
aniflow range and fail closed outside it.

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

The pipeline v2 `renderflow` field is a deprecated compatibility seam. New
`flow` integration must not depend on it; pipeline v3 removes that selection
from aniflow.
