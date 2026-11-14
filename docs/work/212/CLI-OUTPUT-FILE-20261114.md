# Separate destination files from global presentation

This necessary owner repair belongs to [flow #21](https://github.com/egohygiene/flow/issues/21).
An actual support-conformance run exposed a panic: global `--output human|json`
and three local `PathBuf` destinations shared Clap's `output` ID. The selected
command attempted to downcast the presentation value to a path.

Only those local JSON-file destinations now use `--output-file`: diagnostics
bundle, recovered RIFE bind-assembly, and chunked recovered RIFE bind-assembly.
Global presentation remains unchanged. The three guides and diagnostics Task
command use the new spelling. This is an explicit unreleased CLI repair;
`--output` is reserved for presentation. A same-long-name alternative or legacy
alias remains invalid under Clap's debug assertions and is deliberately absent.

Three focused parser regressions cover presentation before/after the command,
default human presentation, exact destination paths and missing/old destinations.
The actual enum definitions and those cases passed in an isolated cached-Clap
Rust 1.85 wrapper. This is distinct from compiling the full Rust test modules,
which remain unrun.

The full CLI built with Rust 1.85, locked/offline dependencies and two build jobs
in 4m37s. The separately labeled diagnostic profile uses opt-level 1, no debug
information, no LTO and 16 codegen units; Cargo manifests/locks are unchanged.
The executable is 38963280 bytes, SHA-256
`570f8a183ffd6b02a0528d01a4ce29bd8b4e64c65e0de86a6183b7fbd2283544`.

Actual full-CLI bundle and inspect succeed with JSON presentation both before
and after the subcommands. Missing destinations and the conflicting old spelling
return parser exit 2 without creating an output. A panic is not counted as a
refusal. The existing canonical pilot executable and native evidence remain
unchanged. Rust formatting and whitespace checks pass for the scoped source.

The [sanitized observation](CLI-OUTPUT-FILE-OBSERVATION-20261114.json) binds source,
build, executable and actual command results without private request/output paths.
It establishes no RIFE/model execution, platform, release or publication claim.
