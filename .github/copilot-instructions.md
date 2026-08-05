# Copilot instructions

## Architecture

- Keep `aniflow` a thin Rust orchestrator around specialized media tools.
- Preserve the ordered macro pipeline and the ordered frame, audio, and video
  processor chains.
- Do not introduce an arbitrary DAG engine until a concrete pipeline requires
  branching or fan-in.
- Keep `aniflow` responsible for temporal processing and the master artifact.
- Keep derivative artifacts behind the optional Renderflow handoff.
- Prefer one crate until an independent release or compile boundary is proven.

## Processor contracts

- Invoke external commands directly through `std::process::Command`.
- Never interpolate a command into a shell string.
- Every processor must declare and create an exact output artifact.
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
- Record newly introduced artifacts and relevant provenance.

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
