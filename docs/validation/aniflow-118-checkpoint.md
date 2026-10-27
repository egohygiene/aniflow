# aniflow #118 documentation reconciliation

Issue: [#118](https://github.com/egohygiene/aniflow/issues/118).
Source base: main `9c969161542fcd95592f7ca3f02cf9266c00479f`.
Branch: `codex/118-current-documentation`.
Reviewed on 2026-10-27. This is an editorial source checkpoint, not executable
qualification, a release or a replacement for final audit #17.

## Live state observed

| Record | Observed state |
| --- | --- |
| PR #105 / checkpoint #104 | Merged at `fc1cbc0cc02b4b416de4c088bf59fa562f05779b`; issue #104 closed |
| PR #115 / checkpoint #114 | Merged at `5376c48c532513aa8cd5ca6f10ff8a3b16ec58a2`; issue #114 closed |
| PR #122 / parent #36 | Merged at `9c969161542fcd95592f7ca3f02cf9266c00479f`; issue #36 open |
| Parents #38 / #110 | Open for their remaining typed setup and interpolation scope |
| #64 / #10 | Open; qualification and first immutable release remain separate gates |
| Cargo / release declaration | Source version 0.3.0; declaration remains `unreleased` |
| GitHub releases | Empty collection; no release observed |
| #117 | Open, complete retained work ledger; #118–#120 are included |

GitHub issue, PR, main and release reads were separate from source inspection.
No workflow status, logs or hosted checks were queried. The issue/PR bodies
retain historical draft language; actual live merge state controls the current
documentation. These observations do not rewrite the historical bodies.

## Source-to-document corrections

| Source owner | What the current documentation now says |
| --- | --- |
| `src/temporal/probe.rs`, `src/temporal/policy.rs`, `src/media.rs`, `src/run.rs` | Rational observed frame grids and input-global stream selection replace first-stream/average-rate processing. Automatic choice requires a unique candidate; other streams require discard acknowledgement. VFR, unsupported origins and contradictory clocks are typed refusals, not broad processing support |
| `src/pipeline_v3.rs`, `src/run_v3.rs`, `src/provider_runtime.rs` | Exact nonempty planned output sets use invocation/report v2 for multi-artifact stages; all-`one` stages keep v1. Optional, empty or dynamic output sets remain refused by execution. Layered acceptance and cache reuse retain their owners |
| `src/toolchain/registration.rs`, preflight/probe modules | The landed native-v2 audio preparation returns four inert documents after bounded identity reads. It does not install, launch, register, write setup or decode media; other mappings and complete environment/device qualification remain open |
| `src/video_interpolation/` | Serial same-duration 2x RGB8/SDR indexed frames are authored. Final-video assembly/audio, broader rates, finer recovery and native qualification remain #110. Resource estimates are not enforced quotas; PNG structure/CRC checks are not decompression proof |
| `src/frame_selection.rs` | #36 preview checks complete index declarations and exact range projections only. Actual immutable decoded import, repair and unselected-byte preservation remain open |
| `Cargo.toml`, `.egohygiene/release.json`, `docs/releases.md`, `.github/workflows/ci.yml` | Source version, configured workflows, executed qualification and actual publication are separate. No new support or passing result is claimed |

The canonical architecture owner was reconciled before README and navigation.
README now gives a current-source status and precise constraints. ROADMAP has
one current entry point and clearly labels dated checkpoints, its managed
snapshot and original version horizons historical. The snapshot body, manifest
and step records are retained unchanged. CONTINUITY records merged PR122 and
the separate ongoing #36 lane instead of an active draft122. Its October17
historical block remains verbatim. Historical validation receipts are untouched.

## Evidence and continuation

Read-only source-to-document review and Git diff inspection only. No tests,
compiler/build, lint/format, naming/schema/drift, documentation validators,
smoke/package, CLI/native/provider/model execution, dependency resolution or
installation, or hosted qualification ran. No Actions were dispatched or
polled. No runtime source, dependency, schema, workflow gate or ADR status changed.

The known audio-alignment cancellation/start-marker failure remains unresolved,
with unproven cause. #64 retains qualification debt; #17 retains its final audit
scope. #119 naming repair and #120 identity review remain independent. This
focused reconciliation does not qualify every historical document or accept
an unrun criterion from a source implementation issue.

Continue issue36 from its live branch/PR before creating competing work. The
bounded target remains imported observed frames with complete decode evidence,
unknown historical processing and later selective repair. Retain #38/#110's
remaining scope and #10's release gate. The full queue stays in #117.
