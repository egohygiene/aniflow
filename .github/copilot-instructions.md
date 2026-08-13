# Copilot instructions

## Architecture

- Keep `aniflow` a thin Rust orchestrator around specialized media tools.
- Preserve the ordered macro pipeline and the ordered frame, audio, and video
  processor chains.
- Do not introduce an arbitrary DAG engine until a concrete pipeline requires
  branching or fan-in.
- Keep `aniflow` responsible for temporal processing, validation, the master,
  and its own run evidence.
- Keep cross-tool selection and orchestration in Flow. Do not add dependencies
  on Renderflow, Optiflow, or Flow.
- Treat the pipeline v2 Renderflow handoff as deprecated compatibility behavior;
  do not extend it or reproduce it in pipeline v3.
- Put meaningful behavior in the public library and keep the CLI a delivery
  adapter.
- Begin with one package containing library and binary targets; split packages
  only when an independent release, compile, feature, or consumer boundary is
  proven.
- Follow the canonical graph under `docs/architecture/` and record significant
  boundary changes as ADRs.

## Processor contracts

- Invoke external commands directly through `std::process::Command`.
- Never interpolate a command into a shell string.
- Every processor must declare and create an exact output artifact.
- Keep process exit, artifact observation, and validation as separate facts.
- Give high-value integrations typed adapters while retaining the generic
  external-command adapter.
- Keep the `gemini_watermark_remover` pipeline kind stable when migrating from
  the `gwr` CLI to Rust-native code.

## Data safety and reproducibility

- Read source media without modifying it.
- Keep generated artifacts beneath the isolated run workspace.
- Reject absolute output paths and parent traversal.
- Snapshot mutable pipeline inputs before execution.
- Preserve checkpoints on failure.
- Refuse resume when the source SHA-256 changes.
- Reuse work only when source, timeline, plan, processor, configuration, tool,
  inputs, outputs, and validation are compatible.
- Record newly introduced artifacts and relevant run evidence without implying
  unobserved authenticity or authorship.
- Preserve stream identity, rational timing, timestamps, and synchronization;
  reject unsupported temporal behavior rather than normalizing it silently.

## Rust standards

- Forbid unsafe Rust unless an accepted architecture decision changes policy.
- Prefer explicit names and small modules over clever abstractions.
- Propagate errors with `anyhow::Context` at I/O and process boundaries.
- Reject unknown configuration fields where Serde supports it safely.
- Add focused unit tests for argument generation, validation, and path safety.

## Validation

Run before completing a change:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
./scripts/smoke-test.sh
```

When a real external processor cannot run in CI, unit-test its exact generated
arguments and keep the FFmpeg-only end-to-end smoke test deterministic.
