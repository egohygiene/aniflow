# Changelog

All notable changes to `aniflow` are documented here.

## [Unreleased]

## [0.3.0] - 2026-08-14

### Added

- Repository-local Aether architecture graph covering identity, foundations,
  epistemology, ontology, systems, structure, methodology, and decisions.
- Accepted v1 capability roadmap and five initial architecture decision records.
- Deliberately small crate-root Rust facade for diagnostics, inspection,
  planning, execution, resume, and status.
- Independent library consumer example and public API integration tests.
- Versioned machine envelopes for every CLI operation, a published JSON Schema,
  golden fixtures, and library/CLI parity coverage.
- Typed public error categories with documented stable exit codes.
- `flow` integration guidance and an enforced lowercase product naming policy.

### Changed

- Clarified that aniflow targets a public Rust library plus thin CLI.
- Assigned cross-tool orchestration to flow and deprecated the pipeline v2
  renderflow handoff as a compatibility seam for removal in pipeline v3.
- Distinguished current v0.2.0 constraints from enduring temporal invariants.
- Reduced the binary to CLI parsing and human presentation while preserving the
  existing application path and console behavior.
- Made library execution silent by default with opt-in provisional progress
  observations for delivery adapters.
- Standardized machine output on `--output json` and the run destination on
  `--output-directory` while retaining the v0.2 compatibility spellings.
- Established Rust `1.85` as the minimum supported compiler and added stable
  Linux, stable macOS, MSRV, and release-package CI evidence.

### Fixed

- Kept the smoke-test temporary directory alive until its `EXIT` cleanup trap,
  avoiding an unbound local variable after a successful pipeline run.

## [0.2.0] - 2026-07-26

### Added

- Ordered per-frame processor chains with isolated stage directories.
- First-class Upscayl NCNN and Gemini Watermark Remover adapters.
- Generic audio and whole-video processor chains.
- Pipeline-aware dependency diagnostics.
- PNG integrity, minimum-size, and uniform-dimension validation.
- Optional disabled-by-default renderflow handoff.
- Versioned delivery manifest.
- Tool summaries captured in run metadata.
- Copilot repository instructions and v0.2.0 implementation spec.

### Changed

- Pipeline schema upgraded from version 1 to version 2.
- Final master path changed from `output/final.mp4` to `output/master.mp4`.
- Run manifest schema upgraded to version 2.

## [0.1.0] - 2026-07-26

### Added

- Rust CLI with inspect, plan, run, resume, status, and doctor commands.
- FFmpeg extraction, assembly, audio restoration, and subtitle support.
- Single generic frame processor and resumable run workspaces.
- Source and output SHA-256 provenance.
- Synthetic end-to-end smoke test and GitHub Actions validation.
