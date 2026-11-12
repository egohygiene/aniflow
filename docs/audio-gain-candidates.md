# Explicit fixed-gain audio candidates

`delivery_audio::gain` adds an optional constant-gain candidate under #212's
retained #83 publishing scope. Select a current exact-final-file QC input and
an [explicit policy](audio-delivery-policy.md). There is no implicit mastering,
limiter, compressor, resampling, downmix or source replacement.

`aniflow.audio-gain-request/v1` contains `check` (the unchanged policy-check
request), an explicit `provider_executable`, and `configuration`:

| Setting | Meaning |
| --- | --- |
| `schema` | `aniflow.audio-gain-configuration/v1` |
| `tools` | Existing audio-inspection configuration with separately pinned FFmpeg/FFprobe 8.0.1 |
| `maximum_absolute_gain_milli_db` | Explicit 1..24000 bound on absolute gain in thousandths of dB |
| `peak_guard_milli_db` | Explicit additional 0..3000 headroom in thousandths of dB |
| `maximum_output_bytes` | Whole float WAV bound, 1024..268435456 bytes |
| `maximum_working_bytes` | Whole source/tool/raw/WAV/evidence admission, 8 MiB..1 TiB |
| `minimum_free_bytes` | Explicit free-space reserve, 0..1 TiB |

Plan reobserves current final-file QC and its complete retained original PCM.
The initial supported source is 44.1/48 kHz mono/stereo, at most ten minutes.
The retained source may be native PCM16/PCM24/float32. A candidate is float32;
that representation/change is explicit and does not rewrite the archival master.
Exact original audio-origin mapping stays in the private report even though a
WAV uses a zero-origin sample clock.

Already-matching policies and missing measurements produce inspectable
`already_within_policy` or `missing_evidence` decisions with no native plan.
Otherwise the requested target determines constant gain rounded to 0.001 dB.
Gain outside the supplied bound refuses; predicted true peak must fit the chosen
ceiling minus the explicit guard. A failure reports `gain_exceeds_bound` or
`peak_ceiling_would_be_exceeded`. It does not clamp gain, substitute a limiter or
silently accept a different goal. Unsupported nonlinear mastering remains open.

```bash
aniflow --output json delivery audio-gain plan --request gain.json
aniflow --output json delivery audio-gain run --request gain.json --output-directory /private/new-candidates
aniflow --output json delivery audio-gain status --run-directory /private/new-candidates/EXACT_RUN
aniflow --output json delivery audio-gain resume --request gain.json --run-directory /private/new-candidates/EXACT_RUN
```

Run requires a `ready` decision and a separate existing canonical parent.
The existing native v3 owner handles workspace, locking, process/cancellation,
reports, acceptance and compatible resume. The fixed provider snapshots exact
PCM and tools, runs only numeric volume conversion, and checks complete output
count, unchanged rate/channels, finite samples and absence of sample overs.
The existing minimal native WAV writer avoids codec-selected metadata/header
layouts. Failed/private work remains available; no partial result is approved.

A predicted peak/LUFS shift is not a measurement. Gating, native floating-point
behavior and later AAC encoding can change actual readings. The output reports
`requires_fresh_exact_file_qc`, with `runtime_qualified: false` and no creative
approval/publication authority. Sample-peak safety is not a substitute for
measured output true peak. Goal success must be checked again on the exact
final candidate after all later byte-changing operations.

All focused cases, source and schemas are authored and uncompiled/unrun. No
tests/compiler/build/dependencies/lint/format/schema/docs/native/product CLI/SDK,
media/provider/model/browser/container/CI, acquisition, signing/timestamps or
publication ran. Whole-work estimates are not an OS quota, disk reservation or
hostile-process sandbox. Native/resource/meter/reference and musical quality
qualification remain deferred. Flow #88 stays last and unchanged; known
alignment cancellation/start-marker failure unresolved, cause unproven.
