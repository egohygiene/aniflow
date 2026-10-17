# Prepare an explicit audio provider registration

Checkpoint [#104](https://github.com/egohygiene/aniflow/issues/104) adds a
reviewable setup handoff for the native-v2 audio-inspection adapter. A caller
supplies its reviewed toolchain profile, inventory, source identity and local
adapter identity. Preparation derives matching registration documents instead
of requiring the caller to assemble their tool pins and provider settings by
hand.

The implementation, schemas, examples and synthetic coverage are authored.
Tests, builds, formatting, lint, schema checks, native-tool execution and hosted
qualification remain **unrun under
[#64](https://github.com/egohygiene/aniflow/issues/64)**. Preparation does not
establish native or release qualification.

## Setup and execution remain explicit

Preparation covers provider `org.egohygiene.aniflow.audio-inspection` version
`2.0.0` and capability `aniflow/audio-technical-inspection` version `2.0.0`.
It does not select an adapter family from a broad capability description. The
adapter must implement the existing
`--aniflow-invocation <absolute-request-path>` ABI; an FFmpeg executable is not
that adapter. A compatible aniflow binary supplies this native dispatch.

Keep these decisions separate:

1. Prepare the local adapter, FFmpeg and ffprobe through the operator's normal
   installation process. Review their exact identities and supported host.
2. Describe the source and the selected profile capability explicitly. Tool
   version observations remain attributed declarations tied to byte digests.
3. Prepare and review the generated documents, then place them within the
   caller-managed registration bundle as described below.
4. Pass the registration and matching preflight configuration explicitly to
   Pipeline v3 planning and execution, with the required side-effect grants.

Preparation launches no process, downloads no asset, installs no package and
does not silently add anything to a registry. It does not decode source media
or infer its identity. The supplied source digest and size must describe the
intended source; subsequent planning and execution retain their own source
checks. A stored preparation result cannot replace fresh plan-bound preflight.

## Reviewable documents and local paths

The [request example](contracts/examples/toolchain-audio-inspection-registration-v1.example.json)
is authored shape data with synthetic paths, versions, source identity and
placeholder digests. It cannot be used as a ready installation. Replace those
declarations with reviewed local evidence before preparing a real bundle.

The request names an existing `registration_directory`, the relative adapter
locator within it, an expected adapter digest and caller-chosen implementation
ID. That directory must have a canonical absolute path, with no symlink
ancestors. The adapter and its parents must be nonsymlink targets. Preparation
observes executable regular-file bytes within the supplied bound and reports
digest mismatches. This operation has authored Unix support; unsupported hosts
are refused. Linux and macOS execution remain unqualified.

The embedded `preflight` has exactly one explicit stage binding. It selects
profile capabilities whose dependency union is exactly the distinct FFmpeg and
ffprobe mappings. Selected effects must exactly match the native manifest:
filesystem read, filesystem write, environment read and subprocess. Unsupported
backend, scale or arbitrary profile settings are refused. Inventory paths and
digest-bound exact semantic version observations supply the typed tool pins;
preparation does not normalize a vendor version token or infer missing data.

After preparing the request, invoke:

```bash
aniflow --output json toolchain prepare-registration \
  --configuration "request.json"
```

The machine command is `toolchain_prepare_registration`. A structurally invalid
request fails before producing a preparation result. A valid request with
unready tools, a missing or mismatched adapter, or a host mismatch retains its
diagnostic result with `ready: false` and no files. The JSON envelope reports a
dependency failure for this non-ready result; inspect its `result` for facts,
actions and diagnostics. The separate
[incomplete result example](contracts/examples/toolchain-registration-preparation-v1.example.json)
illustrates that shape without claiming an observation occurred.

A ready result contains exactly these four documents in `files`, in the order
shown. Each entry contains `relative_path`, a canonical JSON `content` value and
its `sha256`.

| Relative path | Purpose |
| --- | --- |
| `configuration.json` | Source-bound native-v2 configuration with exact tool pins and caller-selected limits. |
| `manifest.json` | Native-v2 declaration with exact FFmpeg/ffprobe version and digest requirements. |
| `preflight.json` | The reviewed embedded preflight configuration, preserved for later planning, run and resume. |
| `registration.json` | Existing registration format locating the manifest, configuration and caller-prepared adapter. |

Preparation returns these values without writing them. Review each document,
then save its `content` under the named path within the same registration
directory. Keep the existing compatible adapter at its requested relative
locator. Do not overwrite unrelated files or change the content to bypass a
refusal. The file digest identifies canonical JSON content; pretty-printing the
saved JSON does not change that canonical identity, but raw on-disk byte hashes
will differ if its formatting differs.

The resulting registration uses the existing confined relative-locator
contract. Manifest, effective configuration and compatible adapter must resolve
beneath the registration document's parent directory. Absolute locators, parent
traversal and escaping symbolic links remain refused by the registration
loader. The tool pins within the typed configuration are explicit absolute
FFmpeg and ffprobe paths and keep their separate meaning. Adapter locators
overlapping any of the four reserved document names are refused, including
case variants.

Do not substitute the result's JSON envelope for a
`aniflow.provider-registration/v1` document. The generated registration and
preflight are separate inputs to the existing workflow. See
[provider authoring](provider-authoring.md#4-register-exact-local-authority)
for the loader boundary and [plan-bound preflight](toolchain-preflight.md) for
runtime freshness and refusal semantics.

**Check the adapter again in the first plan.** The existing registration format
has no expected executable-digest field. `result.adapter.executable_sha256`
records the adapter bytes observed during preparation; a later first planning
operation locks the bytes present at that time. Compare the selected provider
lock's `payload.implementation.executable_sha256` with the prepared adapter
digest, and reprepare and review the setup if they differ. Preparation alone
does not bind those two moments. Existing runtime checks protect the resolved
plan's adapter identity after planning.

For library callers, these APIs are in `aniflow::toolchain`:

| API | Purpose |
| --- | --- |
| `AudioInspectionRegistrationRequest::load` / `from_json_slice` | Parse the bounded closed request with duplicate-key rejection. |
| `AudioInspectionRegistrationRequest::validate` / `sha256` | Check declaration semantics and derive the complete canonical request identity. |
| `prepare_audio_inspection_registration(&request)` | Inspect the selected dependencies and adapter; return a ready or diagnostic preparation result. |
| `ToolchainRegistrationPreparation::canonical_json_bytes` | Serialize the result as canonical JSON; this does not reobserve files or authorize registration. |

The [request schema](contracts/toolchain-audio-inspection-registration-v1.schema.json)
and [result schema](contracts/toolchain-registration-preparation-v1.schema.json)
describe closed transport shapes. Rust enforces cross-file identity and local
filesystem rules. Parsing or deserializing a retained result does not establish
that its readiness, hashes or observations are current.

Requests are limited to one MiB and canonical results to eight MiB. Adapter
paths are limited to 4,096 UTF-8 bytes,
implementation IDs to 128 bytes and adapter observation to a caller-selected
limit between one byte and one GiB. The selected tool inspection retains its
existing two-GiB aggregate hash budget; adapter observation has its own separate
bound. Source size is between 44 bytes and 256 MiB. Native-tool settings admit
timeouts from 1 to 120,000 milliseconds and output limits from 1,024 bytes to
one MiB; preparation records those execution limits without launching tools.

Neither file hashing nor matching version declarations authenticates every
dynamically linked library, shell environment or operating-system dependency.
The native adapter still requires explicit qualification. Other adapter
families, backend/model discovery and complete environment identity remain
under [#38](https://github.com/egohygiene/aniflow/issues/38).
