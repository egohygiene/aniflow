# Bounded offline audio inspection

The `audio inspect` command produces normalized technical and decode evidence
from an immutable PCM WAV source. It uses the existing Pipeline v3 provider
runtime, checkpoints and run-local resume. The source is never replaced or
normalized; accepted JSON evidence lives beneath a new isolated run workspace.

This is [#43](https://github.com/egohygiene/aniflow/issues/43), the second
checkpoint under the [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13).
It specializes the [audio-analysis foundation](audio-analysis.md) without
changing `aniflow.audio-analysis/v1`. [Signal analysis](audio-signal-analysis.md)
adds loudness measurements, and [musical analysis](audio-musical-analysis.md)
adds an optional bounded tempo, beat and key adapter. Preview rendering and
generalized container timing remain separate checkpoints.

For an accepted separation output, [stem selection](audio-stem-lineage.md)
keeps `--input` bound to the original mix and selects the stem by run, stage and
artifact ID. The ordinary inspection stage measures the selected stem, then a
final lineage stage adds the relationship to the final normalized analysis.
The raw technical-report contract and this guide's media limits stay unchanged.

## Supported input and execution profile

| Property | Supported behavior |
| --- | --- |
| Input | Nonempty RIFF32 WAV with PCM format code 1 and 16-bit little-endian samples |
| Audio | One stream at index 0, mono or stereo, 8–192 kHz, zero source-relative origin |
| Bounds | At most 600 seconds and 256 MiB; at most 4,096 ancillary RIFF chunks |
| WAV structure | One `fmt ` chunk before one `data` chunk; format chunk length 16 or 18 with an empty extension; exact block alignment and PCM byte rate |
| Dependencies | Explicit absolute FFmpeg and ffprobe executable paths, exact version tokens and executable SHA-256 pins |
| Provider | First-party Rust stream inspector, invoked through the existing fixed provider ABI |
| Inspection outputs | `artifacts/audio-inspection/technical.json` and `artifacts/audio-inspection/analysis.json` within the run; optional stem selection adds the final lineage outputs |
| Source safety | Private snapshot for subprocess input; source identity rechecked; no in-place update |
| Policy | Local execution only; no tool/model downloads, models, network fallback or preview generation |

RF64, WAVE_FORMAT_EXTENSIBLE, float samples, compressed codecs, multiple audio
streams, nonzero/container clock mappings, unsupported sample layouts and files
outside the profile are refused. Final symlinks for source and tool executable
paths are refused. A filename ending in `.wav` does not establish supported
content. The profile is intentionally smaller than the normalized contract's
representation limits.

Tool execution currently requires Unix process-group cancellation; non-Unix
platforms are refused. Native macOS qualification must be established separately
from Linux synthetic and local FFmpeg evidence. The guide does not claim a
public release.

## Configure exact local tools

Create a private JSON configuration matching
[`providers/audio-inspection/configuration.schema.json`](../providers/audio-inspection/configuration.schema.json).
Its contract is `aniflow.audio-inspection.configuration/v1`:

```json
{
  "schema": "aniflow.audio-inspection.configuration/v1",
  "ffmpeg": {
    "executable": "/absolute/bin/ffmpeg",
    "version": "6.1.1",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "ffprobe": {
    "executable": "/absolute/bin/ffprobe",
    "version": "6.1.1",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "tool_timeout_milliseconds": 30000,
  "maximum_tool_output_bytes": 65536
}
```

This is a configuration shape example, not a usable tool pin. Replace both
paths, both exact version tokens and both digests with observations of your
trusted installed binaries. `6.1.1` is illustrative, not an implicit version
selection. The adapter runs each explicit binary with `-version` and compares
the exact token from its first stdout line. That token must itself be valid
SemVer for the existing provider inventory: for example, `7.1` and non-SemVer
development/vendor labels are refused rather than normalized into invented
versions. It does not search `PATH`, accept a version range or silently adopt a
changed executable.

Paths must be normalized absolute UTF-8 paths without parent traversal or
control characters. The configuration file must be a nonsymlink regular file
of at most 64 KiB. Per-tool timeouts are
1–120,000 milliseconds; stdout and stderr each have a configured cap of
1,024–1,048,576 bytes. Keep the configuration private because it contains local
dependency paths. Executable digests detect drift; they do not authenticate a
download source or cover all dynamically linked libraries.

The facade wraps these settings with the observed `source_audio` artifact ID,
digest and byte size using
[`provider-configuration.schema.json`](../providers/audio-inspection/provider-configuration.schema.json).
The provider manifest pins the exact bytes of that effective-values schema;
its configuration digest binds both authored settings and source identity.
Operators author the settings file above, not the internal wrapper or a second
registration bundle.

## Plan, inspect and resume

Build a compatible local binary with `cargo build --locked --bin aniflow`.
The CLI uses that running binary as the first-party provider executable; library
callers supply an explicit compatible executable path. Supply only the source
and your explicit pinned-tool configuration:

```bash
aniflow --output json audio plan \
  --input "/absolute/audio/synthetic-tone.wav" \
  --configuration "/absolute/config/audio-inspection.json"

aniflow --output json audio inspect \
  --input "/absolute/audio/synthetic-tone.wav" \
  --configuration "/absolute/config/audio-inspection.json" \
  --output-directory "/absolute/generated-runs"
```

`audio plan` performs bounded `-version` checks and hashes the explicit local
tool executables. It reads source identity and checks that the input is a
regular file within the basic size bounds, then resolves a plan without
creating a run workspace or output artifacts. It does **not** certify RIFF
structure or decodability; those checks happen in the inspection stage. This
preflight behavior differs from generic `plan-v3`, which never launches a
provider or discovery process.

Successful machine output uses the existing machine envelope with
`schema_version: 1` and command `audio_plan`, `audio_inspect` or `audio_resume`. The result is the
existing Pipeline v3 plan or run outcome, so consumers can use the established
run directory and artifact locators. Human console text is presentation rather
than the machine contract. Successful JSON envelopes go to stdout; failure
envelopes go to stderr with a nonzero exit. Read-only inspection uses the
existing command:

```bash
aniflow --output json status-v3 "/absolute/generated-runs/RUN_ID"

aniflow --output json audio resume "/absolute/generated-runs/RUN_ID" \
  --input "/absolute/audio/synthetic-tone.wav" \
  --configuration "/absolute/config/audio-inspection.json"
```

Use the audio resume entry point to perform the dependency preflight as well as
existing run-local checkpoint checks. Do not substitute a generic resume call
and assume it supplies the feature-specific external dependency checks.

The Taskfile exposes equivalent machine-readable shortcuts:

```bash
task audio:inspect:plan \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-tone.wav" \
  CONFIGURATION="/absolute/config/audio-inspection.json"

task audio:inspect \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-tone.wav" \
  CONFIGURATION="/absolute/config/audio-inspection.json" \
  OUTPUT="/absolute/generated-runs"

task audio:inspect:resume \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-tone.wav" \
  CONFIGURATION="/absolute/config/audio-inspection.json" \
  RUN="/absolute/generated-runs/RUN_ID"
```

The existing `audio:resume` task remains the Demucs separation task; inspection
uses `audio:inspect:resume`.

## Public Rust boundary and execution bounds

`aniflow::audio_inspection` exposes the same behavior independently of CLI
argument parsing. Construct an `AudioInspectionRequest` from an input path,
validated `AudioInspectionConfiguration` and an explicit compatible native
provider executable. The public operations are:

| Operation | Result and effects |
| --- | --- |
| `preflight` | Structured local tool observations and diagnostics; no media/workspace mutation |
| `plan` | Existing `PipelineV3Plan` after dependency/source identification |
| `run` | Existing `PipelineV3RunOutcome`, isolated workspace and normal stage checkpoints |
| `resume` | Existing run outcome after dependency and checkpoint compatibility checks |

All operations accept the existing `CancellationToken`; `run` and `resume`
also accept Pipeline v3 progress callbacks. `AudioInspectionFailure` retains
available preflight or planning evidence as well as the typed public error.
`AudioTechnicalInspection::from_json_slice` performs the companion report's
semantic checks, just as the normalized analysis has its own validated parser.

The outer provider bounds are separate from the per-tool limits in the JSON
configuration. The audio request defaults are 600 seconds total provider time,
a 2,000-millisecond termination grace period, 1 MiB each for captured provider
stdout/stderr, eight artifact files and 275 MiB (288,358,400 bytes) across provider artifacts.
`AudioInspectionRequest::with_execution_limits` supplies explicit outer bounds
for library callers. `audio inspect` and `audio resume` expose the corresponding
`--provider-timeout-seconds`, `--provider-termination-grace-milliseconds`,
`--maximum-stdout-bytes`, `--maximum-stderr-bytes`, `--maximum-artifact-files`
and `--maximum-artifact-bytes` options. The Taskfile shortcuts use the defaults.
These are admission and runtime bounds, not claims of measured peak resource use
or operating-system memory quotas.

`AudioInspectionRequest::with_stem_selection` attaches the shared bounded
lineage import described in the [stem guide](audio-stem-lineage.md). With that
selection, the full stem is the measurement source and the original mix remains
relationship evidence; neither file is changed.

## Evidence and acceptance

The Rust adapter independently parses the supported WAV structure, derives
sample-frame count and exact rational duration, and hashes the PCM sample bytes.
FFprobe metadata must agree with that independently observed profile, including
stream selection and timestamp-derived sample counts. FFmpeg then decodes the
private snapshot to the same PCM representation; the decoded sample digest must
match the independently read WAV sample digest. A zero subprocess exit alone
cannot establish completion.

The companion
[`aniflow.audio-technical-inspection/v1` schema](contracts/audio-technical-inspection-v1.schema.json)
records container, codec, sample format, rate, channels, exact frames/duration,
PCM bitrate, source and PCM digests, decoded PCM digest, provider/configuration/
lock identities, tool observations and ordered command evidence. PCM bitrate is
`sample_rate_hz × channels × 16`, not the complete WAV file size divided by its
duration. Persisted command arguments replace the private snapshot path with a
placeholder.

The unchanged normalized `aniflow.audio-analysis/v1` companion carries the
technical capability outcome, deterministic source-rate/channel observations,
artifact references, provider evidence and a source-time excerpt reference.
An excerpt reference describes source frames; it does not create a media clip.
Parsing either report checks contract consistency. A copied or fabricated JSON
document does not independently prove that the recorded bytes were processed.

Pipeline v3 separately records invocation and process outcome, immutable output
observations, its built-in artifact-integrity checks and the accepted stage
checkpoint. Provider-owned technical evidence remains an ordinary stage output;
it is not #33's future general provider-backed validation gate.

## Failures and compatibility

The [preflight contract](contracts/audio-inspection-preflight-v1.schema.json)
retains structured dependency diagnostics for missing or invalid
tools, digest/version mismatch, tool timeout, excessive tool output, nonzero
exit, cancellation and unsupported platform. Failure after run creation retains
the existing Pipeline v3 recovery locator and checkpoint state. Incomplete or
failed inspection cannot yield a complete accepted report.

Each run binds the exact source, configuration, provider implementation and
observed tool identities. Resume must recheck those dependencies before reusing
an accepted stage. Source drift or changed tool/configuration authority is a
refusal, not an invitation to edit old evidence. Missing or changed accepted
output is handled by the existing Pipeline v3 compatibility rules. This is
run-local resume; #34 owns cross-run caching.

The provider and subprocesses run as trusted local code with the user's
operating-system authority. Direct arguments, output confinement and process
bounds are not an operating-system sandbox.

## Qualification and roadmap handoff

Validation uses generated silence, tones, sample-rate/channel variants and
bounded malformed/failing fixtures. The normal `scripts/smoke-test.sh` also runs
[`scripts/smoke-audio-inspection.py`](../scripts/smoke-audio-inspection.py) with
the explicitly built binary. The helper uses Python's standard library to
generate mono silence and stereo tones, discovers and pins local tools only
for that smoke fixture, and checks plan, inspect, status, unchanged source bytes,
exact technical evidence and checkpoint reuse on resume. It needs no
`jsonschema` package. Real local FFmpeg/ffprobe checks use those
synthetic sources; no real user media is required. Local passing evidence does
not establish native macOS support, hosted CI completion or release readiness.

After the maintainer merges #43, the recommended next checkpoint is
[#44](https://github.com/egohygiene/aniflow/issues/44) for loudness, peaks, silence
and clipping. [#47](https://github.com/egohygiene/aniflow/issues/47) remains an
independent ready timed-text lane after merged #42. The parent #13 remains open.
