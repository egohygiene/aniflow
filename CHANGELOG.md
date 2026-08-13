# Changelog

All notable changes to `aniflow` are documented here.

## [Unreleased]

### Added

- Repository-local Aether architecture graph covering identity, foundations,
  epistemology, ontology, systems, structure, methodology, and decisions.
- Accepted v1 capability roadmap and five initial architecture decision records.

### Changed

- Clarified that Aniflow targets a public Rust library plus thin CLI.
- Assigned cross-tool orchestration to Flow and deprecated the pipeline v2
  Renderflow handoff as a compatibility seam for removal in pipeline v3.
- Distinguished current v0.2.0 constraints from enduring temporal invariants.

## [0.2.0] - 2026-07-26

### Added

- Ordered per-frame processor chains with isolated stage directories.
- First-class Upscayl NCNN and Gemini Watermark Remover adapters.
- Generic audio and whole-video processor chains.
- Pipeline-aware dependency diagnostics.
- PNG integrity, minimum-size, and uniform-dimension validation.
- Optional disabled-by-default Renderflow handoff.
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
