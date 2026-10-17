# Explicit audio toolchain preflight

Checkpoint [#103](https://github.com/egohygiene/aniflow/issues/103) connects a
reviewed toolchain profile and inventory to explicitly selected Pipeline v3
audio-inspection stages. A guarded plan requires the same configuration on
every run and resume, including when checkpoints or cache entries could satisfy
the work. The processing gate refreshes local dependency evidence before input
binding, cache access or provider launch.

This implementation, its contract examples and synthetic coverage are authored.
Compiler, tests, formatting, lint, schema checks, native-tool execution and
hosted qualification remain **unrun under
[#64](https://github.com/egohygiene/aniflow/issues/64)**. Readiness is consistency
evidence, not native or release qualification.

## Scope and authority

The supported binding is a main stage selecting
`org.egohygiene.aniflow.audio-inspection` version `2.0.0`, capability
`aniflow/audio-technical-inspection` version `2.0.0`, and the exact typed
`aniflow.audio-inspection.provider-configuration/v2` envelope. It covers only
the stages explicitly named in `bindings`; other stages and nested validation
providers do not acquire this guard. The existing exact provider-lock checks
still apply to the complete plan.

For each binding, preflight compares the following evidence:

| Relationship | Required agreement |
| --- | --- |
| Stage and registration | The named main stage selects the exact named registration in its immutable provider lock. Fresh registry resolution must reproduce the plan's locks. |
| Provider configuration | The closed native audio-inspection settings reproduce the registration's complete effective configuration and schema identity. |
| FFmpeg and ffprobe | Each mapped inventory path and digest matches its typed tool pin; digest-bound version evidence and the planned lock's tool version/digest match that pin. |
| Local bytes | Fresh bounded inspection of the explicitly named files satisfies the selected profile, including any declared version, package revision, flags or build features. The tools must be nonsymlink executable regular files. |
| Host | Inventory OS and architecture equal the current process's host constants and satisfy the selected profile's platform requirements. This does not measure hardware availability. |
| Selected profile | Its dependency union is exactly the two distinct mapped tool dependencies. Profile side effects equal both the stage's declared effects and its locked effects. |

Selected capabilities must have no backend or scale declaration and an empty
`effective_settings` object. Only filesystem read, filesystem write, environment
read and subprocess effects are admitted. The closed provider configuration owns the
actual audio settings; arbitrary profile settings are refused instead of being
silently ignored. Unsupported provider families, legacy audio-inspection v1,
model providers and inline validators cannot use this binding.

Preflight reads local files but launches no tool or provider. Supplied version,
package and feature observations remain attributed declarations, bound to
current bytes; they do not become newly observed native results. A saved report
with `ready: true` cannot substitute for the configuration or skip fresh checks.
`native_qualification` is always `false`. Tool hashes do not identify every
dynamically linked dependency or establish an operating-system sandbox.

## Prepare the registration and declaration

1. Prepare and review the local FFmpeg and ffprobe installation separately.
   Record literal absolute paths, exact supported version tokens, byte digests
   and package revisions. Follow [toolchain profiles](toolchain-profiles.md)
   for inventory evidence. [Bounded probes](toolchain-probes.md) are a separate,
   explicitly requested operation; this gate neither runs them nor imports
   their output automatically. The audio adapter still requires its own exact
   version token; diagnostic probe normalization does not change that contract.
2. Prepare a compatible audio-inspection adapter and explicit registration
   using [registration preparation](toolchain-registration.md) or
   [provider authoring](provider-authoring.md#4-register-exact-local-authority).
   Use the native [manifest](../providers/audio-inspection/manifest.json) and
   [provider-values schema](../providers/audio-inspection/provider-configuration-v2.schema.json).
   Construct `AudioInspectionProviderConfiguration` with the reviewed settings
   and exact source identity, then use `provider_configuration()` to derive its
   effective envelope. Keep registration locators confined to their bundle;
   include exact `ffmpeg` and `ffprobe` component versions and digests. A native
   FFmpeg executable does not implement the `--aniflow-invocation` adapter ABI.
3. Author a Pipeline v3 stage selecting that registration and author a
   preflight document embedding the reviewed profile, inventory and explicit
   stage-to-tool mappings. The profile capability IDs are setup labels, not
   provider capability IDs. The profile's defaults do not replace the binding's
   required, nonempty `capability_ids` selection.
4. Inspect the report, address missing or conflicting evidence, then bind the
   configuration to a new resolved plan. Supply that same configuration to
   every processing request. A changed configuration requires a newly reviewed
   plan; an existing guard cannot be replaced in place.

The complete [configuration shape example](contracts/examples/toolchain-preflight-v1.example.json)
contains **synthetic paths, versions, package revisions and placeholder pins**.
It is not a runnable setup or an installation recommendation. Its mapping is:

```json
{
  "stage_id": "inspect_audio",
  "registration_id": "local-audio-inspection",
  "capability_ids": ["audio-inspection-tools"],
  "ffmpeg_dependency_id": "ffmpeg",
  "ffprobe_dependency_id": "ffprobe"
}
```

Replace all example identities with reviewed local evidence. The
[report example](contracts/examples/toolchain-preflight-report-v1.example.json)
is also authored shape data with placeholder digests, not an execution receipt.

## Bind and process explicitly

`--toolchain-preflight PATH` is an optional argument on the existing `plan-v3`,
`run-v3` and `resume-v3` commands. The separate
[`toolchain prepare-registration`](toolchain-registration.md) operation derives
reviewable registration documents; preflight itself does not create them.
Existing input bindings, explicit registrations, host observations,
side-effect grants and offline policy are still required.

For example, after setting the path variables and supplying actual host resource
observations, the planning handoff is:

```bash
aniflow --output json plan-v3 \
  --pipeline "$PIPELINE" \
  --input "source_audio=$INPUT" \
  --provider-registration "$REGISTRATION" \
  --toolchain-preflight "$PREFLIGHT" \
  --host-cpu-threads "$HOST_CPU_THREADS" \
  --host-memory-mib "$HOST_MEMORY_MIB" \
  --host-storage-mib "$HOST_STORAGE_MIB" \
  --allow-side-effect filesystem-read \
  --allow-side-effect filesystem-write \
  --allow-side-effect environment-read \
  --allow-side-effect subprocess \
  --offline
```

Use `run-v3` with the same arguments and an explicit `--output-directory` to
plan and process. Resume the resulting guarded run with:

```bash
aniflow --output json resume-v3 "$RUN_DIRECTORY" \
  --input "source_audio=$INPUT" \
  --provider-registration "$REGISTRATION" \
  --toolchain-preflight "$PREFLIGHT"
```

These commands are operator handoffs, not executed checkpoint evidence.
Generic planning already reads and hashes source identities before the explicit
preflight binder runs; CLI `run-v3` also performs that planning step. The
processing gate precedes the runtime's subsequent input binding and cache or
provider work. Resume must first open its existing workspace, acquire the writer
lock and read the saved plan. The guard is therefore not a promise of zero
filesystem activity before refusal.

The plan payload records `toolchain_preflight_sha256`, the canonical digest of
the full validated configuration. Omitting the configuration from a guarded
run or resume, supplying a changed one, or attaching it to an unguarded runtime
request is refused. Plans without a guard retain their existing serialized
payload and behavior; opt-in does not retroactively guard older runs.

Library callers use these public surfaces:

| API | Purpose |
| --- | --- |
| `ToolchainPreflightConfiguration::load` / `from_json_slice` | Read and validate the closed declaration. |
| `preflight_toolchain(&plan, &registry, &configuration)` | Return a fresh `ToolchainPreflightReport` without reading media or launching a process. Retain the report even when `ready` is false. |
| `bind_toolchain_preflight(plan, &registry, &configuration)` | Recheck readiness and return the immutable guarded plan. |
| `PipelineV3RunRequest::with_toolchain_preflight(configuration)` | Supply the exact declaration for a guarded run. |
| `PipelineV3ResumeRequest::with_toolchain_preflight(configuration)` | Supply it again for resume. |

CLI failures retain the existing error category/message envelope. A refusal
includes bounded excerpts from failed inspection facts, not a complete report.
Use the public inspection API before binding when a caller needs to retain the
full failed inspection facts and remediation actions. The report's
`plan_sha256` identifies the plan passed to that inspection; binding changes the
plan digest. Inspect the guarded plan when retaining evidence tied to its final
identity. Neither a report nor the binder creates a registration, installs a
package, downloads assets or searches for alternatives.

## Contracts and limits

The [configuration schema](contracts/toolchain-preflight-v1.schema.json) embeds
the existing profile and inventory shapes. It permits 1–128 unique stage
bindings, each selecting 1–64 unique profile capabilities with identifiers of
at most 128 bytes. The complete declaration is limited to one MiB. Rust
validation additionally checks cross-references, exact dependency mappings and
the selected adapter semantics that JSON shape validation cannot establish.

The [report schema](contracts/toolchain-preflight-report-v1.schema.json) records
the configuration and plan digests, readiness, diagnostics and one fresh
inspection over the union of selected capabilities. That single inspection
preserves the existing two-GiB aggregate hashing budget across all bindings;
it does not multiply the budget per stage. Invalid declarations return an
error. Valid declarations with insufficient dependency evidence produce a
non-ready report. No stored report is accepted as execution authority.

This checkpoint does not qualify media decoding, model behavior, GPUs, other
native adapters or cross-platform execution. The subsequent
[registration-preparation checkpoint](toolchain-registration.md) derives inert
setup documents and still leaves materialization explicit.
[#38](https://github.com/egohygiene/aniflow/issues/38) retains remaining
integration and setup boundaries; #64 owns execution qualification.
