# Bounded audio signal analysis

`audio analyze --analysis signal` measures an immutable PCM WAV source through
two existing Pipeline v3 stages: `inspect_audio`, then `measure_audio`. The first
stage establishes the bounded technical/decode evidence from
[audio inspection](audio-inspection.md); the second consumes the source and both
inspection artifacts before producing signal evidence and a final normalized
analysis document. It does not rewrite or normalize the source.

This is [#44](https://github.com/egohygiene/aniflow/issues/44), the third
checkpoint under [#13](https://github.com/egohygiene/aniflow/issues/13).
The existing `aniflow.audio-analysis/v1` schema remains unchanged. Its final
analysis includes signal capability/evidence references; the companion signal
contract owns measurement fields rather than hiding required values in opaque
extensions. The companion is
[`aniflow.audio-signal-measurements/v1`](contracts/audio-signal-measurements-v1.schema.json).

## Support matrix

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
| True peak | Separate padded FFmpeg pass; supported only at source rates up to 48 kHz; higher rates explicitly unavailable |
| Platforms | Unix process-group execution; local Linux evidence does not qualify native macOS or other FFmpeg builds |

Inputs outside the signal clock subset are refused rather than resampled.
At supported signal rates above 48 kHz, other measurements remain available
while true peak reports `unsupported_true_peak_rate`. No new resampler,
multistream selection, general container timing, model inference, network
fallback or preview rendering is introduced.

## Measurement definitions and availability

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

Sample peak and true peak are separate measurements. The true-peak pass adds
100 ms of zero padding within its private processing path to flush the
resampler; it does not modify or lengthen the source artifact. Loudness is not
taken from this padded pass. The report records the separate filter/method and
padding so consumers can distinguish both operations. The padding is appended
after the last source sample only in that measurement pass.

Command evidence records the fixed unpadded filter
`ebur128=metadata=1:peak=true:framelog=verbose,ametadata=print:key=lavfi.r128.S:file=-`.
The separate true-peak filter is
`apad=pad_len=N,ebur128=peak=true:framelog=verbose`, where `N` is exactly
`sample_rate_hz / 10` source frames. Above 48 kHz, this second command is omitted
and the true-peak result retains its unsupported-rate reason.

The reviewed [FFmpeg 6.1 `ebur128` source](https://ffmpeg.org/doxygen/6.1/f__ebur128_8c_source.html)
uses a 192 kHz true-peak resampling target. This profile therefore limits true
peak to source rates at or below 48 kHz, providing at least a fourfold target
rate. Rates above 48 kHz are not silently treated as equally qualified.
FFmpeg summary values have 0.1 dB/LU precision, while short-term metadata has
0.001 LUFS precision. These recorded output precisions are not accuracy
guarantees or cross-platform equivalence claims.

## Signal settings

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
signal selection. Existing inspection commands and tasks keep their #43
meaning.

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

The signal workflow exports three artifact IDs: `technical`, `signal` and
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

`aniflow::audio_signal::SignalAnalysisRequest` combines an existing
`AudioInspectionRequest` with validated `SignalAnalysisConfiguration` settings.
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
artifact-integrity gate or implement #33's future layered validation system.

## Synthetic fixtures and qualification

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
| `signal_high_rate` | 96 kHz measurements retain explicit unavailable true peak |
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

These are fixture acceptance thresholds, not general measurement-error bounds.
Fixtures are generated synthetic inputs only. The smoke receipt records actual
observations and exact local tool versions; the summary's decimal precision
alone does not substitute for that evidence. Native macOS, alternate FFmpeg
builds, hosted CI and release qualification remain
separate gates. The maintainer owns review and merge; #13 remains open through
its other feature checkpoints and integrated closeout.
