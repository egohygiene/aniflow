# Bounded musical estimates

`audio analyze --analysis musical` adds tempo, beat-position and key/mode
estimates to an immutable audio source. It uses the existing technical inspector
and Pipeline v3 runtime, with an explicitly installed local analyzer. Musical
estimates retain their method identities and uncertainty; completion does not
make them authoritative musical facts.

This is [#46](https://github.com/egohygiene/aniflow/issues/46), checkpoint five of
the open [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13).
The normalized `aniflow.audio-analysis/v1` foundation remains unchanged.
The source file is never overwritten, normalized or resampled.

## Selected analyzer and profile

The optional adapter selects Essentia `2.1b6.dev1389` with NumPy `2.3.5`,
PyYAML `6.0.3` and six `1.17.0` behind an explicitly pinned Python interpreter
and adapter script. This narrow profile
provides rhythm and tonal estimates through one local implementation without
loading a learned model. The public normalized artifact boundary keeps later
consumers separate from Essentia's native output.

The selected
[RhythmExtractor2013](https://essentia.upf.edu/reference/std_RhythmExtractor2013.html)
supplies BPM, tick positions and competing BPM estimates, and requires 44.1 kHz
input. [KeyExtractor](https://essentia.upf.edu/reference/std_KeyExtractor.html)
provides key/scale estimates with selectable profiles. aniflow uses the fixed
`krumhansl` and `temperley` profiles independently so disagreement remains
visible. Rhythm uses `multifeature` with `minTempo=40` and `maxTempo=208`;
key extraction uses the selected package's other default parameters. These
documented outputs and explicit local execution fit this bounded
checkpoint; selecting the adapter is not a comparative musical-accuracy claim.

| Property | Supported profile |
| --- | --- |
| Media | Nonempty PCM16 RIFF WAV within the existing technical-inspection profile |
| Sample clock | Exactly 44,100 Hz, zero source-relative origin, one audio stream |
| Channels | Mono or stereo; stereo is converted to an arithmetic mean only inside the analysis input |
| Duration | 8–600 seconds, at most 256 MiB |
| Preprocessing | PCM scale conversion and explicit channel average; no hidden resampling, ReplayGain or source mutation |
| Algorithm dependencies | Exact Python and adapter pins, plus the installed Essentia/NumPy/PyYAML/six package inventory |
| Model dependencies | None for this profile; no downloads or model cache |
| Execution | Bounded local provider subprocesses through the existing Pipeline v3 ABI |
| Scope | Whole supported mix or a full stem selected through the existing lineage importer |

When an otherwise supported technical source has another sample rate, the Rust
workflow retains technical inspection and emits explicitly unavailable musical
estimates; it does not silently resample. The direct raw Python adapter refuses
that unsupported input. Arithmetic stereo averaging can cancel equal
opposite-polarity channels; their original
per-channel energy does not justify manufacturing a musical result for a silent
analysis input. A beat estimate is not an onset detector or a verified metrical
grid.

| Musical family | Current availability |
| --- | --- |
| Global tempo/BPM | Heuristic estimate with retained competing observations |
| Beat positions | Ordered source-relative estimates, with unavailable calibrated confidence |
| Key/mode | Separate `krumhansl` and `temperley` observations with raw strengths |
| Downbeats and meter | Unsupported |
| Tempo changes | Unsupported; competing BPM observations are not a time-varying tempo map |
| Chords | Unsupported; a key estimate does not identify a chord sequence |
| Onsets and rhythm density | Unsupported |
| Sections | Unsupported |
| Spectral/timbre descriptors | Unsupported |

These wider families remain explicitly unsupported in the companion evidence.
They are not silently omitted as though the adapter had measured them.

## Install dependencies separately

Prepare a trusted, dedicated Python environment outside analysis runs, following
the [upstream installation guidance](https://essentia.upf.edu/installing.html)
and the exact [selected release](https://pypi.org/project/essentia/2.1b6.dev1389/).
The supported package versions are fixed; installing a newer package does not
implicitly expand the profile. Installation is an explicit operator action.
The interpreter must be a nonsymlink regular executable within that environment;
a virtual environment created with `python3 -m venv --copies` supplies that form.
Simply resolving a virtual-environment symlink to the system Python can lose the
environment's installed packages. Use the actual prepared executable and record
its exact version and hash.
Ordinary planning, analysis, resume and the synthetic corpus never invoke an
installer, fetch weights or fall back to a network service.

The selected PyPI release records `AGPL-3.0-only`; retain that package metadata
alongside the [upstream licensing information](https://essentia.upf.edu/licensing_information.html)
and the actual installed distribution's license files. NumPy publishes its own
[license terms](https://numpy.org/doc/2.3/license.html). Runtime license evidence
records observations; it does not establish a redistribution or application
compatibility conclusion. No pretrained-model license is implied because this
profile uses no learned model assets.

Probe the explicitly selected interpreter and this checkout's adapter to obtain
local dependency observations:

```bash
/absolute/musical-env/bin/python -I -B \
  /absolute/aniflow/scripts/audio-musical-adapter.py --probe

task audio:musical:probe PYTHON="/absolute/musical-env/bin/python"
```

The probe observes a local installation; it does not choose or install one.
Planning and execution must compare its observations with authored exact pins.
The adapter hash identifies the reviewed script, while package fingerprints
identify installed package content. Version text alone is insufficient for
resume compatibility. The retained probe also distinguishes the package version
from Essentia's runtime build version and Git revision marker.

The runtime fingerprint hashes the installed distribution inventories, package
directories and package-named bundled-library directories, excluding generated Python bytecode
caches. It binds relative paths, byte sizes and SHA-256 values beneath the
interpreter prefix, with at most 25,000 files and 1 GiB of inventoried bytes.
Symlink or escaping package paths are refused. This is a bounded package-content
identity, not a signature, an authenticity guarantee or a fingerprint of every
operating-system/shared-library dependency.

Author the separate musical settings against
[`providers/audio-musical/configuration.schema.json`](../providers/audio-musical/configuration.schema.json):

```json
{
  "schema": "aniflow.audio-musical.configuration/v1",
  "python": {
    "executable": "/absolute/musical-env/bin/python",
    "version": "3.12.0",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "adapter": {
    "path": "/absolute/aniflow/scripts/audio-musical-adapter.py",
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "runtime": {
    "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "file_count": 1,
    "byte_count": 1
  },
  "tool_timeout_milliseconds": 120000,
  "maximum_tool_output_bytes": 1048576
}
```

This illustrates the shape; its hashes/counts and Python version are placeholders.
Use observations from the exact trusted installation:

| Configuration field | Observed value |
| --- | --- |
| `python.version` | Probe `python_version`; an exact SemVer token |
| `python.sha256` | SHA-256 of the selected regular interpreter file |
| `adapter.sha256` | SHA-256 of the selected adapter script bytes |
| `runtime.sha256` | Probe `runtime_sha256` |
| `runtime.file_count` | Probe `runtime_file_count` |
| `runtime.byte_count` | Probe `runtime_byte_count` |

The settings document is bounded to 64 KiB and rejects unknown fields. Its
tool deadline is 1–120,000 ms; stdout and stderr each have a configured cap of
1,024–1,048,576 bytes. The facade constructs the separate
[provider configuration](../providers/audio-musical/provider-configuration.schema.json)
that binds these settings to the technical tool pins, observed source identity
and upstream normalized analysis. Operators do not author that internal wrapper.

## Plan, analyze and resume

The existing
[technical tool configuration](audio-inspection.md#configure-exact-local-tools)
pins FFmpeg and ffprobe. Musical execution takes a separate musical dependency
configuration. Both are required for the selected workflow:

```bash
aniflow --output json audio plan --analysis musical \
  --input "/absolute/audio/synthetic-clicks.wav" \
  --configuration "/absolute/config/tools.json" \
  --musical-configuration "/absolute/config/musical.json"

aniflow --output json audio analyze --analysis musical \
  --input "/absolute/audio/synthetic-clicks.wav" \
  --configuration "/absolute/config/tools.json" \
  --musical-configuration "/absolute/config/musical.json" \
  --output-directory "/absolute/analysis-runs"

aniflow --output json status-v3 "/absolute/analysis-runs/RUN_ID"

aniflow --output json audio resume "/absolute/analysis-runs/RUN_ID" \
  --analysis musical \
  --input "/absolute/audio/synthetic-clicks.wav" \
  --configuration "/absolute/config/tools.json" \
  --musical-configuration "/absolute/config/musical.json"
```

Always select `--analysis musical` explicitly: `audio analyze` retains its
existing signal default, and `audio plan`/`audio resume` retain their technical
default. Supply only the configuration for the selected analysis family;
musical analysis does not accept signal settings as a substitute.

The `audio:musical:preflight` task is an alias for the musical plan operation.
It requires the same source and configuration arguments, verifies dependencies
and emits the existing plan shape without creating a run workspace. A ready
dependency check does not establish successful analysis or musical accuracy.

```bash
task audio:musical:preflight \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-clicks.wav" \
  CONFIGURATION="/absolute/config/tools.json" \
  MUSICAL_CONFIGURATION="/absolute/config/musical.json"

task audio:musical \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-clicks.wav" \
  CONFIGURATION="/absolute/config/tools.json" \
  MUSICAL_CONFIGURATION="/absolute/config/musical.json" \
  OUTPUT="/absolute/analysis-runs"
```

`audio:musical:plan` uses the same variables as preflight. The resume task
`audio:musical:resume` requires `RUN` instead of `OUTPUT`. All task wrappers
preserve the public CLI behavior; `ANIFLOW_BIN` optionally selects the binary.

## Estimates, confidence and disagreement

Tempo and key are heuristic observations, not deterministic media facts.
Analyzer-native confidence/strength values remain raw uncalibrated scores in
their companion evidence. They are not converted into probabilities, percentages
or a normalized certainty score. The normalized document records confidence as
unavailable when this profile has no calibration evidence.

Competing BPM observations and both key profiles retain their identities. A
disagreement is preserved rather than resolved by inventing a single consensus.
Even matching profiles are related algorithm observations, not independent proof
of a correct key. Half/double-tempo ambiguities remain distinguishable from an
exact tempo match in synthetic evaluation.

The companion reports `tempo_status`, `beats_status` and `key_status`
independently. A missing tempo does not discard available key observations, and
an absent key does not discard beats. Missing family estimates use the explicit
`no_estimate` reason. All raw profile results remain retained, including a
nonpositive key strength that does not produce a normalized key candidate.

Raw beat times in seconds remain in the companion. Normalized markers use the
nearest source sample frame, with halfway values rounded upward, and retain
strictly ordered in-range source coordinates. Each marker occupies
`[source_frame, source_frame + 1)` in the normalized timeline. This declared quantization does not turn
the estimator's timing precision into an accuracy guarantee.

Silence, insufficient duration, unsupported input, malformed results and failed
dependencies cannot become a plausible default BPM or key. Non-finite numbers,
unordered/out-of-range beats and contradictory source identities are refused.
Failure, cancellation and retained recovery evidence remain distinct from an
accepted musical result.

For valid technical input with ready dependencies, the public report uses
`unsupported_sample_rate`, `insufficient_duration` or `silent_downmix` when no
musical inference is supported. The musical capability is then unavailable,
while the final normalized document is partial and preserves technical evidence.
When the analyzer supplies some families but omits others, the available tempo,
beat or key candidates survive and the capability is partial. All three
families being estimated permits a complete musical capability; that status
still carries heuristic provenance and unavailable calibrated confidence.

## Public artifacts and validation

The final workflow exports these artifact IDs:

| Export | Run-relative path and meaning |
| --- | --- |
| `technical` | `artifacts/audio-inspection/technical.json`; unchanged technical/decode evidence |
| `musical` | `artifacts/audio-musical/musical.json`; the musical companion |
| `analysis` | `artifacts/audio-musical/analysis.json`; final normalized analysis, artifact ID `musical_analysis` |
| `stem_lineage`, when selected | `artifacts/audio-stem/lineage.json`; unchanged imported relationship evidence |

The companion schema is
[`aniflow.audio-musical-analysis/v1`](contracts/audio-musical-analysis-v1.schema.json).
The raw adapter observation and dependency probe have separate public schemas:
[`aniflow.audio-musical-observation/v1`](contracts/audio-musical-observation-v1.schema.json)
and [`aniflow.audio-musical-probe/v1`](contracts/audio-musical-probe-v1.schema.json).
All three have validated public Rust parsers named `from_json_slice` on
`AudioMusicalAnalysis`, `AudioMusicalObservation` and `AudioMusicalProbe`.
JSON Schema checks shape; Rust additionally enforces exact clocks, method,
candidate derivation, ordering and dependency relationships. Neither parser
authenticates a producer's claims or independently listens to the source.

Native provider `org.egohygiene.aniflow.audio-musical`, execution capability
`aniflow/audio-musical-structure` and its normalized family declaration all use
version `1.0.0`; the implementation ID is `aniflow-audio-musical-v1`. The existing
normalized schema remains unchanged. Companion command/settings evidence uses
adapter/snapshot placeholders and digests rather than persisting local paths.

The companion is bounded to 8 MiB, raw observations to 1 MiB and probes to
64 KiB. Beat/tempo arrays are bounded to 8,192 observations; excessive output
is refused rather than silently truncated. The existing outer provider limits
apply independently to every stage, while musical subprocess deadlines and
capture limits come from its settings. These bounds are separate from musical
accuracy or measured hardware-resource claims.

## Stem lineage and runtime ownership

The public `aniflow::audio_musical` module exposes
`MusicalAnalysisRequest::new(inspection, configuration)` using an existing
`AudioInspectionRequest` and validated `MusicalAnalysisConfiguration`.
`plan`, `run` and `resume` use the existing cancellation token, progress callbacks,
Pipeline v3 results and `AudioInspectionFailure`. The library's `preflight`
accepts the musical configuration, inspection-tool configuration and cancellation
token to reobserve dependencies without analyzing media. CLI argument handling
does not own an alternate implementation.

The existing [stem-selection flags](audio-stem-lineage.md) also apply to musical
analysis. Keep `--input` bound to the original mix and supply the accepted stem
run, stage and artifact ID. The ordered pipeline is `inspect_audio`,
`attach_stem_lineage` when selected, then `analyze_musical`. The musical stage consumes that
normalized source/scope and preserves its original-mix and relationship evidence.

All channels and the complete selected stem remain the supported scope. Beat
times belong to that stem's clock; duration tolerance does not establish an
onset/phase mapping to the original mix. No separator or model is rerun. Existing
Pipeline v3 owns immutable outputs, checkpoints, cancellation, status and
run-local resume; no additional scheduler or cross-run cache is introduced.

The exact source, adapter/interpreter/package identities, effective settings and
upstream analysis participate in plan and checkpoint identity. Resume rechecks
those dependencies before accepting run-local reuse. A completed process alone
does not establish valid output, and provider-owned musical evidence does not
become the general layered completion gate from #33.

## Synthetic conformance and musical observations

Deterministic adapter fixtures exercise contract semantics and refusal behavior
without installing Essentia. A separate optional corpus executes the selected
real analyzer on generated clicks, tones/chords and deliberate ambiguity cases.
These answer different questions: contract conformance establishes that the
adapter preserves declarations; an observed synthetic estimate covers only that
fixture and exact dependency environment.

| Synthetic fixture IDs | Declared check |
| --- | --- |
| `click_90`, `click_120`, `click_150` | 24-second click tracks; BPM within 3 of the intended rate or an explicitly recorded half/double-tempo alternative |
| `c_major`, `a_minor` | Generated 12-second triads; retain both profile observations and record which match the constructed key |
| `alternating_triads` | Deliberately ambiguous tonal material; retain observed agreement/disagreement without assigning a single ground-truth key |
| `silence_8s`, `antiphase_8s` | Silent arithmetic mean; raw adapter refusal and public explicit musical unavailability |
| `short_4s` | Below the minimum inference duration |
| `unsupported_rate_48000` | Preserve the unsupported musical-rate boundary without implicit resampling |

The click tolerance is a fixture acceptance rule, not a general tempo error
bound. A half/double match is reported as ambiguity, not an exact match. Beat
counts/order and source preservation are checked separately; raw score magnitude
is never an accuracy assertion. Generated triads are not a benchmark for arbitrary
music, changing keys or tuning systems.

The corpus command requires an already prepared interpreter and writes only
generated temporary inputs and the requested result file:

```bash
task audio:musical:corpus \
  PYTHON="/absolute/musical-env/bin/python" \
  ADAPTER="/absolute/aniflow/scripts/audio-musical-adapter.py" \
  OUTPUT="/absolute/generated-evidence/musical-corpus.json"
```

That task checks the raw adapter. Use `audio:musical:corpus:native` with the same
variables plus `ANIFLOW_BIN` to also exercise public native planning, analysis,
read-only status and compatible resume. The underlying script's `--aniflow`
option selects that additional boundary. Corpus output is a new evidence file;
choose a fresh `OUTPUT` path for each execution.

Executed fixture results, tolerances, exact tool identities and remaining gates
belong to the local validation receipt. This guide does not infer broad musical
accuracy, real-media quality, native-platform support or release qualification
from synthetic conformance. The maintainer reviews and merges the checkpoint;
the parent audio roadmap remains open.
