# Flow #137 checkpoint 2 — aniflow, November 13, 2026

The observed E0277 errors compare borrowed output PathBufs to an owned tool
PathBuf. Both comparisons now use borrowed `Path` views. This keeps executable
alias refusal and the order of source/model/output safety checks intact; it does
not move, clone, canonicalize or bypass the pinned tool. The patch changes no
provider arguments, artifacts, approvals or public schema.

## Source evidence and acceptance

Repairs are grounded in the failed jobs linked from
[Flow #137](https://github.com/egohygiene/flow/issues/137). The rustix 1.1.4 signal
constant and rusqlite 0.40.2 error-conversion variants were additionally read
from their exact upstream source tags; no dependency was installed or resolved.
No Cargo manifest/lock, MSRV, feature support, journal schema or native authority
gate changes here. The separate SQLite security-advisory/toolchain conflict is
checkpoint 6; these type repairs do not resolve it.

This branch is stacked after its owning checkpoint 1 workflow-lint draft.
No tests/compiler/build, formatting/lint, schema/drift, native/provider/media or
hosted check ran under the maintainer's existing execution deferral. Authored
checks and source review are not evidence of successful compilation.

Acceptance still requires the declared all-target/MSRV/host/feature checks to
run, the affected refusal/range/argv cases to pass, and the relevant CI workloads
to get past library compilation. Do not mark checkpoint 2 or the whole CI issue
complete from this source repair. Preserve the unresolved alignment/start-marker
failure and the separate native/APFS qualification requirements with their
current owners; this patch makes no claim about those causes or results.
