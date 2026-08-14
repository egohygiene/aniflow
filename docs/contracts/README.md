# aniflow public contracts

This directory defines the machine boundary intended for scripts, `flow`, and
other independent consumers. Human console text is presentation and is not a
compatibility contract.

## Machine envelope v1

Every command accepts `--output json` and emits one
`aniflow.machine-envelope/v1`-equivalent JSON document:

- successful documents are written to standard output;
- failed documents are written to standard error;
- `schema_version` is currently `1`;
- `command` identifies the invoked operation;
- `status` is either `success` or `error`;
- `result` contains the command-specific public result on success;
- `error.category` and `error.message` describe failure without requiring prose
  parsing.

Consumers must reject unsupported `schema_version` values. The Rust
`MachineEnvelope::from_json_slice` helper performs that check.

The generic envelope shape is described by
[`machine-envelope-v1.schema.json`](machine-envelope-v1.schema.json). Result
schemas remain tied to the `0.3.x` public Rust types until independently
versioned command-result schemas are justified by real `flow` integration.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | success |
| `2` | CLI usage or argument parsing failure |
| `3` | invalid or missing input |
| `4` | invalid or unsupported configuration |
| `5` | missing or unusable dependency |
| `6` | media inspection or interpretation failure |
| `7` | pipeline execution failure |
| `8` | invalid or incompatible run state |
| `9` | filesystem or other I/O failure |
| `70` | internal serialization or invariant failure |

The public `ErrorCategory::exit_code` mapping and CLI contract tests enforce
these values.

## Naming and compatibility

- Product names are lowercase: `aniflow`, `flow`, `optiflow`, and `renderflow`.
- New CLI options use complete long-form names.
- The former `run --output-dir` spelling remains a visible compatibility alias
  for `--output-directory` throughout `0.3.x`.
- The former `inspect --json` spelling retains its raw v0.2 inspection object
  throughout `0.3.x`; new integrations use the versioned `--output json`
  envelope.
- Additive result fields are permitted in `0.3.x`; removing or changing the
  meaning of fields requires a schema or SemVer change.
- Paths are serialized using Rust path serialization for the host platform.
- Unknown envelope versions are rejected rather than interpreted loosely.

Breaking compatibility requires an ADR update, migration notes, and contract
fixtures demonstrating both rejection and the supported replacement.
