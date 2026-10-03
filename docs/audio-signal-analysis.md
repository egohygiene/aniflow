# Bounded audio signal analysis

`audio analyze --analysis signal` measures an immutable supported WAV source through
two existing Pipeline v3 stages: `inspect_audio`, then `measure_audio`. The first
stage establishes the bounded technical/decode evidence from
[audio inspection](audio-inspection.md); the second consumes the source and both
inspection artifacts before producing signal evidence and a final normalized
analysis document. It does not rewrite or normalize the source.

The foundation shipped in [#44](https://github.com/egohygiene/aniflow/issues/44),
the third checkpoint under [#13](https://github.com/egohygiene/aniflow/issues/13).
The bounded high-rate true-peak follow-up
[#55](https://github.com/egohygiene/aniflow/issues/55) merged through
[PR #56](https://github.com/egohygiene/aniflow/pull/56).
The existing `aniflow.audio-analysis/v1` schema remains unchanged. Its final
analysis includes signal capability/evidence references; the companion signal
contract owns measurement fields rather than hiding required values in opaque
extensions. Legacy PCM16 settings emit
[`aniflow.audio-signal-measurements/v2`](contracts/audio-signal-measurements-v2.schema.json);
the [v1 companion](contracts/audio-signal-measurements-v1.schema.json) remains a
frozen readable contract with its original support semantics.

[Stem selection](audio-stem-lineage.md) adds an optional lineage stage after
`measure_audio`. Stem import retains its existing PCM16-only source/stem
contract; #80 native PCM24/float32 support is for direct sources. The technical
and legacy signal stages measure the entire selected stem. The final normalized
analysis retains the declared stem scope and original-mix relationship.

## Native source-amplitude profile (#80)

[#80](https://github.com/egohygiene/aniflow/issues/80), through checkpoints
#93–#95, adds an explicit native profile for classic RIFF PCM16, packed PCM24
and finite IEEE float32. It keeps the [inspection bounds](audio-inspection.md):
mono/stereo, 8–192 kHz, at most 600 seconds and 256 MiB. No integer conversion,
resampling, level correction or source rewriting is used for its measurements.
Unlike the legacy loudness path, native sample statistics do not require a
sample rate divisible by ten.

Select the native profile with a settings file using
[`aniflow.audio-signal.configuration/v2`](../providers/audio-signal/configuration-native.schema.json):

```json
{
  "schema": "aniflow.audio-signal.configuration/v2",
  "silence_threshold_ratio": 0.0009765625,
  "minimum_silence_milliseconds": 100,
  "clipping_threshold_ratio": 1.0
}
```

The shown defaults apply when numeric fields are omitted. Silence is a finite
full-scale amplitude ratio in `[0, 1]`; clipping is in `(0, 1]`; minimum silence
is an integer in `[1, 600000]` milliseconds. The settings document is bounded to
64 KiB. Pass this file through the same `--signal-configuration` option shown
below; its schema selects native execution. Existing `/v1` settings retain the
PCM16-only profile and its report v2. Unsupported schemas are refused rather
than falling back or converting the source.

| Native quantity | Meaning |
| --- | --- |
| Source representation | `source_format` is `pcm16`, `pcm24` or `float32` |
| Full-scale reference | PCM16: 32,768; packed PCM24: 8,388,608; float32: 1.0 |
| Sample peak / RMS | Source amplitude divided by the format's reference; squared values use compensated `f64` accumulation |
| Logarithmic level / crest | `20 × log10(ratio)` dBFS; peak-to-RMS ratio in dB; zero energy has `silent_input` unavailability |
| Silence / clipping regions | Inclusive magnitude comparisons against the configured ratios; exact half-open source-frame ranges, per channel |
| Float32 above full scale | Finite amplitudes above 1.0 and positive dBFS survive unchanged; they do not prove audible clipping |
| Loudness / true peak | Integrated loudness, short-term loudness, loudness range and true peak are all explicitly unavailable with `unsupported_native_signal_profile` |

The native default clipping ratio is exactly 1.0. With the signed full-scale
reference, PCM24 `-8388608` meets it but `+8388607` is just below it; PCM16
`-32768` and `+32767` have the same asymmetry. An explicit lower ratio can mark
both endpoints. This threshold is not a silent reinterpretation of legacy
`clipping_threshold_pcm: 32767`, and threshold regions do not prove a source's
clipping history.

All native formats, including PCM16 selected through settings v2, use that
same unavailable-meter rule until their meter execution is separately qualified.
The native stage issues no loudness or true-peak tool commands. It still binds
the pinned upstream FFmpeg/ffprobe inspection evidence. NaN and infinity are
refused; they never become silence, zero, a clipped finite value or a successful
partial source scan.

The new companion is
[`aniflow.audio-signal-measurements/v3`](contracts/audio-signal-measurements-v3.schema.json),
read through `NativeAudioSignalMeasurements::from_json_slice`. It retains
`aniflow.native-sample-statistics/v1`, the full-scale reference, source format,
settings, upstream identities and explicit meter unavailability. Provider and
runtime capability use `3.0.0`; the source-bound wrapper uses
`aniflow.audio-signal.provider-configuration/v2`. The normalized analysis
remains `aniflow.audio-analysis/v1`. Synthetic
[PCM24](contracts/examples/audio-signal-measurements-v3-pcm24.example.json) and
[float32](contracts/examples/audio-signal-measurements-v3-float32.example.json)
examples describe the authored contract; they are not measured-media receipts.

This implementation, examples and synthetic coverage are **authored and unrun**.
Tests, builds, formatting, lint, schema, native-tool and hosted checks remain
deferred under [#64](https://github.com/egohygiene/aniflow/issues/64). Historical
PCM16 loudness/true-peak receipts do not qualify the native profile, a host,
actual media, or an installed toolchain.

## Legacy PCM16 support matrix

| Measurement or input | Supported profile and meaning |
| --- | --- |
| Source | #43 PCM16 RIFF WAV: one zero-origin audio stream, mono/stereo, 8–192 kHz, nonempty, at most 600 seconds and 256 MiB |
| Signal sample clock | Sample rate must be divisible by ten so 100 ms steps have exact sample-frame boundaries |
| Sample peak | Per-channel maximum sample magnitude from the independently read PCM values |
| RMS level | Per-channel whole-source root-mean-square sample amplitude |
| Crest factor | Per-channel peak-to-RMS ratio expressed in dB; a separate quantity from loudness range |
| Silence regions | Per-channel sample-threshold runs meeting the configured minimum duration |
| Clipping-threshold regions | Per-channel magnitude-threshold crossings; these do not prove audible distortion or clipping history |
| Integrated loudness | FFmpeg `ebur128` evidence over the unpadded source; at least 400 ms and sufficient gated content |
| Short-term loudness | Full 3-second source windows sampled every 100 ms; the adapter reports values at or above its −70 LUFS measurement floor |
| Loudness range | At least 60 seconds and at least ten qualifying full short-term windows, as defined below |
| True peak, source rates ≤48 kHz | Existing separate padded `ebur128` pass and 0.1 dB summary precision |
| True peak, 88.2/96/176.4/192 kHz | Explicit fourfold SWR interpolation followed by a floating-point peak scan; six-decimal output |
| Other source rates >48 kHz | Other supported signal metrics remain available; true peak reports `unsupported_true_peak_rate` |
| Platforms | Unix process-group execution; local Linux evidence does not qualify native macOS or other FFmpeg builds |

Inputs outside the signal clock subset are refused rather than normalized.
The high-rate true-peak path applies interpolation only to its private
measurement stream, at exactly the four listed input rates. It does not rewrite,
upsample or replace the source artifact. Other rates above 48 kHz are not
silently added to that qualification set. No multistream selection, general
container timing, model inference, network fallback or preview rendering is
introduced.

## Legacy PCM16 measurement definitions and availability

Channel scope is explicit. PCM-derived levels use the source's sample values:
`sample_peak_ratio = max(abs(sample)) / 32768` and
`rms_ratio = sqrt(sum(sample²) / frame_count) / 32768`. Peak and squared sums are
accumulated in the integer domain; square roots and logarithmic conversions are
floating-point results. Nonzero peak/RMS levels are `20 × log10(ratio)` dBFS;
crest factor is `20 × log10(sample_peak_ratio / rms_ratio)` dB. A zero-energy
channel retains zero linear ratios and explicit `silent_input` unavailability
for its logarithmic levels and crest factor.

Loudness describes the declared mono/stereo program through the pinned FFmpeg
filter. Loudness range, crest factor and RMS level retain distinct names and
units rather than an ambiguous combined “dynamic range” score.

Silence and clipping settings are expressed as integer PCM sample magnitudes.
The default silence threshold is 32 with a minimum run of 100 ms; the default
clipping threshold is 32,767. Silence includes samples with magnitude less than
or equal to its threshold; clipping regions include magnitude greater than or
equal to their threshold, including the negative PCM16 endpoint where applicable.
The minimum silence length is `ceil(milliseconds × sample_rate / 1000)` frames;
a clipping region needs at least one frame. Regions retain exact half-open
source sample-frame ranges, sorted by channel then source range, with adjacent
runs coalesced and no overlap within each channel. They do not use rounded
decimal timestamps. An empty region list means no region met the configured
criteria, not that the analyzer failed to report regions.

A finite measured value and an unavailable result have different representations.
Silence, insufficient duration, below-gate integrated output, below-floor
short-term output and an unsupported true-peak rate retain explicit reasons.
Non-finite values, tool sentinels or unavailable measurements cannot become
numeric zero. A measured zero loudness range can be
valid for a sufficiently long constant signal; it differs from unavailable LRA.
A completed analysis may retain an unavailable metric with its exact reason;
that is different from a provider failure or incomplete run.

Short-term loudness itself is ungated. This adapter deliberately represents
short-term readings below −70 LUFS as `below_measurement_floor`, separately
from integrated loudness's `below_absolute_gate` (I at or below −70 LUFS).
Exactly −70 LUFS remains a measured short-term value; LRA's population guard
requires observations above that floor. The floor is an explicit adapter
support policy, not a claim that EBU short-term loudness applies an
absolute gate.

### Loudness range guard

The adapter keeps integrated/short-term loudness and LRA on the unpadded source.
It reports LRA only after at least 60 seconds and at least ten complete
3-second windows whose reported short-term loudness is above −70 LUFS and at
least the reported LRA relative gate plus 0.1 LU. At the published 0.001 LUFS
short-term precision this is `value >= max(-69.999, lra_threshold_lufs + 0.1)`.
The report retains the gate, qualifying-window count and method thresholds. Otherwise LRA is unavailable
with `insufficient_duration` or `insufficient_gated_windows`.

The 0.1 LU addition conservatively accounts for rounded tool summaries; the
ten-window rule is an aniflow availability guard, not an EBU requirement.
[EBU Tech 3341, section 2.4](https://tech.ebu.ch/docs/tech/tech3341.pdf) requires
an indication that LRA is not yet stable in the first 60 seconds.
[EBU Tech 3342](https://tech.ebu.ch/docs/tech/tech3342.pdf) supplies the LRA
algorithm context. These references explain the distinction and guard policy;
this bounded adapter does not claim EBU compliance or certification.

### True peak and precision

Sample peak and true peak are separate measurements. Both supported true-peak
paths append exactly 100 ms of zeros to their private processing stream to
flush interpolation at the last source sample. They do not modify or lengthen
the source artifact, and loudness is never taken from the padded stream.

Command evidence retains the unchanged unpadded loudness filter:

```text
ebur128=metadata=1:peak=true:framelog=verbose,ametadata=print:key=lavfi.r128.S:file=-
```

For source rates at or below 48 kHz, the separate peak filter remains
`apad=pad_len=N,ebur128=peak=true:framelog=verbose`, with
`N = sample_rate_hz / 10` source frames. The reviewed
[FFmpeg 6.1 `ebur128` source](https://ffmpeg.org/doxygen/6.1/f__ebur128_8c_source.html)
uses a 192 kHz interpolation target; the <=48 kHz profile retains at least a
fourfold target rate. Its 0.1 dB summary precision and prior support boundary
remain unchanged.

For source rates **88,200, 96,000, 176,400 and 192,000 Hz**, the v2 path uses
explicit fourfold SWR interpolation and `astats` to observe the maximum across
the declared channels. This pass also fixes `-filter_threads 1`. The filter
template is shown below; actual command evidence substitutes
`N = sample_rate_hz / 10` and `T = sample_rate_hz * 4`:

```text
apad=pad_len=N,aresample=T:resampler=swr:osf=dblp:tsf=dblp:filter_size=64:phase_shift=10:linear_interp=0:exact_rational=1:cutoff=1:filter_type=kaiser:kaiser_beta=9:dither_method=0:async=0,astats=metadata=0:reset=0:measure_perchannel=none:measure_overall=Peak_level+Number_of_samples+Number_of_NaNs+Number_of_Infs
```

Double-precision planar processing keeps intersample values above full scale
representable; an observed positive dBTP value is not clamped to zero. The
provider requires exactly `4 * (source.frame_count + N)` processed frames and
zero NaN/infinite sample counts. A malformed, missing or non-finite report
cannot become an accepted numeric peak. All-silent source PCM retains explicit
`silent_input` unavailability instead of encoding a logarithmic infinity.

The four high-rate cases use six-decimal peak output. Existing `ebur128`
loudness/peak summaries retain 0.1 dB/LU precision, and short-term metadata
retains 0.001 LUFS precision. Those output precisions are not accuracy bounds,
EBU compliance evidence or cross-platform equivalence claims. The exact filter,
tool digest/version and method identity are part of the retained evidence.

## Legacy PCM16 signal settings

Author the separate settings file against
[`providers/audio-signal/configuration.schema.json`](../providers/audio-signal/configuration.schema.json):

```json
{
  "schema": "aniflow.audio-signal.configuration/v1",
  "silence_threshold_pcm": 32,
  "minimum_silence_milliseconds": 100,
  "clipping_threshold_pcm": 32767
}
```

The three numeric settings default to the shown values when omitted. Silence
threshold is in `[0, 32768]`, clipping threshold in `[1, 32768]`, and minimum
silence duration in `[1, 600000]` milliseconds. Unknown fields and unsupported
versions are refused; the settings document is bounded to 64 KiB. Settings do not select different loudness filters,
window sizes or true-peak algorithms; those remain the declared fixed profile.

The effective
[`provider-configuration.schema.json`](../providers/audio-signal/provider-configuration.schema.json)
wrapper binds these settings, the #43 pinned-tool configuration and the source
artifact identity. Operators author the settings and tool files; the public
facade constructs the source-bound provider configuration and its digest.

## Plan, analyze and resume

Use the [exact pinned-tool configuration](audio-inspection.md#configure-exact-local-tools)
from #43 and a separate signal-settings file. The tool executable paths, exact
SemVer tokens and SHA-256 pins remain explicit; normal operations never discover
or install dependencies. For the full 600-second signal profile, set
`maximum_tool_output_bytes` to `1048576` in that tool configuration. The shorter
#43 example's 65,536-byte cap can intentionally refuse a longer short-term
metadata stream. The adapter never silently increases a configured capture
limit.

```bash
aniflow --output json audio plan --analysis signal \
  --input "/absolute/audio/synthetic-tone.wav" \
  --configuration "/absolute/config/audio-tools.json" \
  --signal-configuration "/absolute/config/signal-settings.json"

aniflow --output json audio analyze --analysis signal \
  --input "/absolute/audio/synthetic-tone.wav" \
  --configuration "/absolute/config/audio-tools.json" \
  --signal-configuration "/absolute/config/signal-settings.json" \
  --output-directory "/absolute/generated-runs"

aniflow --output json status-v3 "/absolute/generated-runs/RUN_ID"

aniflow --output json audio resume "/absolute/generated-runs/RUN_ID" --analysis signal \
  --input "/absolute/audio/synthetic-tone.wav" \
  --configuration "/absolute/config/audio-tools.json" \
  --signal-configuration "/absolute/config/signal-settings.json"
```

`audio plan` and `audio resume` retain the technical-only default. Selecting
`--analysis signal` requires `--signal-configuration`; a signal-settings file
with the technical selection is rejected. `audio analyze` defaults to the
signal selection. Technical inspection now uses its native v2 profile; the signal-settings
schema independently selects native v3 or legacy PCM16 v2 measurement behavior.

Planning rechecks tool pins and source identity without creating a run; it does
not prove successful source inspection or measurement. Machine envelopes retain
`schema_version: 1`; commands are `audio_plan`, `audio_analyze` and
`audio_resume`. Success envelopes go to stdout; failure envelopes go to stderr
with a nonzero exit. Read-only status remains `status-v3`.

Equivalent Taskfile entry points are `audio:signal:plan`, `audio:signal` and
`audio:signal:resume`. All take `INPUT`, `CONFIGURATION` and
`SIGNAL_CONFIGURATION`; execution also takes `OUTPUT`, and resume takes `RUN`.
`ANIFLOW_BIN` selects the executable, defaulting to `aniflow`:

```bash
task audio:signal \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-tone.wav" \
  CONFIGURATION="/absolute/config/audio-tools.json" \
  SIGNAL_CONFIGURATION="/absolute/config/signal-settings.json" \
  OUTPUT="/absolute/generated-runs"
```

## Output contracts

The following table describes the preserved legacy PCM16 path. The native
configuration/report/provider boundaries are listed above; their versions must
not be inferred from this legacy table.

| Boundary | Current version and compatibility |
| --- | --- |
| Normalized analysis document | `aniflow.audio-analysis/v1`; unchanged |
| Normalized signal-family declaration | `aniflow/audio-signal-measurements` at `1.0.0`; unchanged semantic evidence family |
| Signal companion report | `aniflow.audio-signal-measurements/v2`; requires an explicit true-peak algorithm |
| Executable signal provider | `org.egohygiene.aniflow.audio-signal` at `2.0.0` |
| Runtime invocation capability | `aniflow/audio-signal-measurements` at `2.0.0` |
| Implementation identity | `aniflow-audio-signal-v2` |
| Authored settings and source-bound wrapper | Their existing `/v1` schemas remain unchanged |
| Historical companion report | Frozen `/v1`, provider `1.0.0`, original method and unavailable high-rate policy |

The normalized family declaration describes the meaning of signal evidence;
the runtime capability selects a concrete provider execution contract. The
shared capability name therefore does not imply that those two version numbers
must match. Provider evidence and the referenced v2 companion identify the exact
method used for legacy PCM16 execution.

`AudioSignalMeasurements::from_json_slice` reads both frozen v1 reports and
current v2 reports, applying the correct provider, method and command invariants
for each schema; v3 uses the separate `NativeAudioSignalMeasurements` parser.
V1 does not gain the new algorithm field or high-rate support.
V2 requires `method.true_peak_algorithm`, with kind `legacy_ebur128`,
`swr_4x_astats` or `unsupported_rate` for the declared source rate. The SWR
variant records every fixed interpolation setting shown above.
`AudioSignalMethod::for_sample_rate` retains historical v1 behavior;
`for_sample_rate_v2` constructs the new method, and
`true_peak_supported_sample_rate` exposes the closed rate-selection rule.

Readable historical evidence does not authorize checkpoint reuse under the new
provider. Legacy PCM16 execution uses provider/capability `2.0.0` and implementation
`aniflow-audio-signal-v2`, so an old v1 plan/lock cannot silently resume as v2.
The CLI commands, authored settings and artifact export names remain stable.

The [v2 canonical example](contracts/examples/audio-signal-measurements-v2.example.json)
uses generated 192 kHz silence and synthetic tool identities to illustrate the
contract. It does not claim that an actual provider or FFmpeg tool ran.

Without stem selection, the signal workflow exports three artifact IDs: `technical`, `signal` and
`analysis`. The first is the technical inspection evidence, the second is the
companion measurement report, and `analysis` is the final normalized analysis.
The first stage's normalized inspection analysis remains a separate immutable
input to the measurement stage, not the final exported `analysis`.

| Exported ID | Path within the run |
| --- | --- |
| `technical` | `artifacts/audio-inspection/technical.json` |
| `signal` | `artifacts/audio-signal/signal.json` |
| `analysis` | `artifacts/audio-signal/analysis.json` |

The upstream inspection analysis remains at
`artifacts/audio-inspection/analysis.json`.

With [stem selection](audio-stem-lineage.md), the existing raw artifacts remain
in place and a final lineage stage supplies the `analysis` export plus a
`stem_lineage` companion. Use that final exported analysis for the original-mix
relationship and stem scopes.

`AudioSignalMeasurements` retains source and upstream artifact checksums,
provider/implementation/configuration/lock identity, exact tool observations,
settings, method windows/thresholds, command evidence and scoped measurements.
`AudioSignalMeasurement` uses an explicit unit and a tagged value:
`measured` with a finite number or `unavailable` with a reason. Per-channel
linear ratios remain available for mathematical interpretation of zero energy.
Integrated/short-term loudness uses LUFS, LRA uses LU, sample peak/RMS uses dBFS,
crest factor uses dB, and true peak uses dBTP. The signal report is bounded to
8 MiB; each region collection and the short-term series is bounded to 10,000
entries. Excessively fragmented regions are refused rather than truncated.

The normalized analysis preserves upstream evidence and adds the signal
capability and measurement-artifact reference. Contract validation checks
consistency, scope and identity declarations; merely parsing a JSON report does
not independently remeasure audio or authenticate the producer's claims.

## Library, identity and recovery

`aniflow::audio_signal::SignalAnalysisRequest::new` combines an existing
`AudioInspectionRequest` with validated legacy `SignalAnalysisConfiguration`
settings. `SignalAnalysisRequest::new_native` instead takes that inspection
request and `NativeSignalAnalysisConfiguration`; it retains native samples and
emits `NativeAudioSignalMeasurements` v3.
The sibling module's `plan`, `run` and `resume` operations reuse the existing
cancellation token, progress callbacks, Pipeline v3 results and
`AudioInspectionFailure` evidence. CLI parsing remains a delivery adapter.

The source, inspection evidence, effective signal settings, native provider
binary and tool identities participate in the existing plan and checkpoint
identity. The measuring stage cannot substitute unrelated inspection artifacts
or silently reuse evidence from changed settings. Resume rechecks exact
external dependencies and reuses only compatible run-local checkpoints;
missing/changed outputs follow existing Pipeline v3 invalidation rules.
No separate run store or cross-run cache is introduced.

Each provider stage has the #43 default outer bounds: 600 seconds, a 2-second
termination grace period, 1 MiB each for captured stdout/stderr, eight temporary
artifact files and 275 MiB of artifact bytes. The two stages are independently
bounded, so total pipeline wall time may exceed one stage's 600-second limit.
Configured child-tool deadlines and capture bounds remain separate. Existing
CLI provider-limit flags and library request limits allow explicit bounds.

The source stays immutable; private temporary processing and final outputs
remain in the run workspace. Missing tools, changed identities, cancellation,
timeout, excessive output and failed measurement retain structured failure and
recovery state. Provider-owned signal validation does not replace the runtime's
artifact-integrity and layered acceptance gates.

## Synthetic fixtures and qualification

The tables and historical smoke tolerances below describe the legacy PCM16
profile. New #80 native contract, RIFF, amplitude, threshold and refusal cases
are authored in the audio inspection/signal tests; their execution is deferred
under #64. The earlier smoke receipts do not establish their results.


| Stable fixture ID | Intended evidence |
| --- | --- |
| `signal_silence` | Exact zero PCM, silence bounds and explicit unavailable logarithmic/gated measurements |
| `signal_tone_1khz` | Independently known tone amplitude and loudness/level relationships |
| `signal_impulse_tail` | Source-end impulse and true-peak flush behavior |
| `signal_intersample_peak` | Sample peak and true peak remain distinct |
| `signal_channel_asymmetry` | Explicit channel scope and unequal channel levels |
| `signal_clipping_thresholds` | Exact threshold crossings without an audible-distortion claim |
| `signal_sparse_lra` | Insufficient gated populations keep LRA unavailable |
| `signal_constant_lra` | Long constant signal can yield measured zero LRA |
| `signal_short_input` | Incomplete loudness windows report unavailability |
| `signal_high_rate` | Original 96 kHz fixture; v2 applies the explicit high-rate peak method |
| `signal_tool_refusal` | Missing/stale/failing bounded dependencies cannot complete |

The [local smoke helper](../scripts/smoke-audio-signal.py) defines these numeric
acceptance tolerances against its generated inputs:

| Quantity checked by the synthetic smoke | Absolute tolerance |
| --- | --- |
| Native linear peak/RMS ratios | `1e-12` |
| Native logarithmic levels/crest factor | `1e-9` dB |
| Constant 1 kHz tone integrated/short-term loudness | `0.2` LU |
| Supported tone/intersample true peak | `0.2` dB |
| Source-end impulse true peak | `0.1` dB |
| Long constant-signal loudness range | `0.1` LU |
| Sample-frame regions, source identity and checkpoint reuse | Exact equality |

The [high-rate smoke helper](../scripts/smoke-audio-true-peak.py) separately uses
fixture IDs `true_peak_<RATE>_<CASE>`, with rates `88200`, `96000`, `176400` and
`192000` and the seven cases below: 28 generated probes in total.

| Case suffix | Independent input or required result |
| --- | --- |
| `sine` | Tapered quarter-rate sine at 45° phase; reconstructed amplitude `0.5`, distinct from sample peak |
| `tail` | Impulse in the final source frame; interpolation must include the source tail |
| `silence` | Exact zero input with `silent_input` true-peak unavailability |
| `short_one_frame` | One nonzero PCM frame; peak remains measurable despite unavailable loudness |
| `short_partial_hop` | Final impulse before a complete 100 ms hop; no discarded tail peak |
| `asymmetric` | Stereo with a silent channel and a sine channel; explicit all-channel peak scope |
| `over_full_scale` | Reconstructed amplitude `1.4`; positive dBTP must survive floating-point interpolation |

For example, `true_peak_88200_sine` names one fixture. Non-silent high-rate cases
use a `0.02` dB absolute tolerance against their independently generated
amplitude; tone cases also require true peak to exceed sample peak by more than
`2.9` dB. Source bytes, artifact digests and the method/command declarations are
checked exactly. A sine case at each rate additionally checks two-stage resume
without re-execution. This matrix does not qualify unlisted rates or another
FFmpeg build.

These are fixture acceptance thresholds, not general measurement-error bounds.
The [#44 local receipt](validation/aniflow-44-local.json) retains the baseline
signal profile's historical evidence. The
[#55 local validation receipt](validation/aniflow-55-local.json) records
current high-rate checks, exact local tool versions and remaining qualification
limits. Each helper's optional `--receipt` output retains fixture observations
and source/output identities; output decimal precision alone is not validation
evidence. Native macOS, alternate FFmpeg builds, hosted CI, EBU compliance and
release qualification remain separate gates. The maintainer owns review and
merge; #13 remains open through its other feature checkpoints and closeout.
