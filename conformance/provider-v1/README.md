# Provider v1 conformance fixtures

This bundle is a tiny, redistribution-safe reference implementation of the
Pipeline v3 provider boundary. It is deliberately synthetic: the frame, audio,
and video inputs are text bytes with media-shaped contract identities, not
production media. No FFmpeg, model, GPU, network access, sibling repository, or
paid service is required.

The executable accepts only the provider-invocation v1 ABI:

```text
reference-provider.py --aniflow-invocation <absolute-request-path>
```

It checks the closed request fields it consumes, reads one bound immutable
input, and writes one bound output. Processor outputs retain the source bytes
behind a deterministic reference header. The validator emits compact,
deterministic JSON evidence describing its input.

## Bundle layout

| Path | Purpose |
| --- | --- |
| `manifest.json` | One inert manifest declaring all four capabilities |
| `schemas/reference-configuration-v1.schema.json` | Closed provider-owned schema for `values` |
| `configs/*.json` | Effective happy-path and deterministic-failure configurations |
| `reference-*.registration.json` | Confined registration locators; none contains `..` |
| `pipelines/*.yml` | Standalone Pipeline v3 happy-path and failure examples |
| `inputs/` | Tiny synthetic input files and frame directory |
| `reference-provider.py` | Python standard-library reference process |

The exact SHA-256 of the checked-in provider-owned schema is
`e1afbc8a40457b98255976814a36297752914c3787ebf3aac534f152bca3a317`.
That identity appears in the manifest, every capability, every effective
configuration, and the executable's closed request checks. General validation
of provider-specific JSON Schema remains the embedding caller's responsibility.

## Happy-path profiles

| Profile | Capability | Input binding | Provider ports | Registration | Pipeline | Mode |
| --- | --- | --- | --- | --- | --- | --- |
| Frame | `aniflow/frame.process` | `source_frames=inputs/frame` (directory) | `frames` → `processed_frames` | `reference-frame.registration.json` | `pipelines/frame.yml` | `copy` |
| Audio | `aniflow/audio.process` | `source_audio=inputs/audio.txt` (file) | `audio` → `processed_audio` | `reference-audio.registration.json` | `pipelines/audio.yml` | `copy` |
| Whole video | `aniflow/whole-video.process` | `source_video=inputs/whole-video.txt` (file) | `video` → `processed_video` | `reference-whole-video.registration.json` | `pipelines/whole-video.yml` | `copy` |
| Artifact validator | `aniflow/artifact.validate` | `candidate=inputs/candidate.txt` (file) | `artifact` → `evidence` | `reference-artifact-validator.registration.json` | `pipelines/artifact-validator.yml` | `validation_evidence` |

All profiles request only `filesystem_read` and `filesystem_write`, declare
network and GPU use forbidden, and have no external component inventory.
Resolve them offline with one observed CPU thread and zero declared memory or
storage minimums.

From the repository root, run the frame profile with:

```sh
cargo run -- run-v3 \
  --pipeline conformance/provider-v1/pipelines/frame.yml \
  --input source_frames=conformance/provider-v1/inputs/frame \
  --provider-registration conformance/provider-v1/reference-frame.registration.json \
  --host-cpu-threads 1 \
  --host-memory-mib 0 \
  --host-storage-mib 0 \
  --allow-side-effect filesystem-read \
  --allow-side-effect filesystem-write \
  --offline \
  --output-directory target/provider-conformance-runs
```

Use `plan-v3` instead of `run-v3` and omit `--output-directory` for read-only
resolution. For another happy-path profile, substitute the input binding,
registration, and pipeline from the table; those table paths are relative to
this bundle, so prefix them with `conformance/provider-v1/` when invoking the
command from the repository root. The successful stage output is independently checked by
`aniflow.validation/artifact-integrity/v1`, producing an execution report and a
stage checkpoint. Provider-emitted validator evidence is an ordinary artifact;
it does not replace aniflow's built-in validation authority for another stage.

## Deterministic failures

The same closed configuration schema also exposes two test-only modes:

| Mode | Registration | Pipeline | Expected result |
| --- | --- | --- | --- |
| `missing_output` | `reference-missing-output.registration.json` | `pipelines/missing-output.yml` | Process exits zero, runtime rejects the absent declared output |
| `nonzero` | `reference-nonzero.registration.json` | `pipelines/nonzero.yml` | Process exits 23, runtime records typed exit-failure evidence |

Both failure pipelines reuse `source_frames=inputs/frame`. Run them with the
same command shape and side-effect grants as the frame profile, prefixing the
bundle-relative input, registration, and pipeline paths as described above.
Omitting either side-effect grant exercises authority denial before launch. Invoking the
executable with any argument shape other than the fixed ABI, an invalid JSON
request, unknown fields, mismatched identities, unsafe paths, or unexpected
ports is rejected and cannot establish a checkpoint. An error discovered while
traversing a bound directory may leave an incomplete candidate output for the
runtime to reject.

## Runtime support

The source fixtures and contract planning are platform-neutral. Live reference
execution requires Python 3.10 or newer and an operating system that can launch
the executable script through its `python3` shebang. The checked-in executable
bit is significant on Unix. Treat only platforms exercised by the repository's
test matrix as certified; this bundle does not claim native Windows script
launch support.
