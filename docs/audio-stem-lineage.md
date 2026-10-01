# Audio analysis with stem lineage

Audio inspection and analysis can select a declared stem from an
accepted Pipeline v3 separation run while retaining its original-mix identity.
The original mix, selected stem and prior run remain read-only. Analysis uses
the existing audio providers and appends a lineage stage; it does not separate
audio again, run a model or download dependencies.

This is [#45](https://github.com/egohygiene/aniflow/issues/45), checkpoint four
under the open [#13 mini roadmap](https://github.com/egohygiene/aniflow/issues/13),
merged in [PR #57](https://github.com/egohygiene/aniflow/pull/57).
The [technical-inspection](audio-inspection.md) and
[signal-analysis](audio-signal-analysis.md) input and measurement limits still
apply. Lineage identifies the declared relationship; it does not certify
separation quality or alignment of musical events.

## Supported import boundary

| Selection or evidence | Current behavior |
| --- | --- |
| Original mix | `--input` names the unchanged original pipeline input of the separation run |
| Prior run | Complete Pipeline v3 run with accepted plan, provider lock, execution report, checkpoint, integrity validations and artifact observations |
| Separation stage | Explicit stage ID with capability `aniflow/audio.separate` version `1.0.0`; one direct original-mix input and no upstream dependencies |
| Stem | Explicit output artifact ID declared as immutable audio with the temporal-component role |
| Output set | One to eight declared WAV stems and exactly one validation-evidence file, all retained as accepted public run artifacts |
| Relationship evidence | Declared validation-evidence output from the same accepted stage; provider-owned contents remain opaque |
| Stem media | The existing nonempty PCM16 RIFF WAV profile: mono/stereo, 8–192 kHz, at most 600 seconds and 256 MiB |
| Channels | All declared stem channels, in ascending zero-based order; omitted selection means all |
| Frame range | Entire stem, `[0, frame_count)`; omitted selection means the full range |
| Duration tolerance | Integer 0–20 ms, default 20 ms; compare exact source/stem frame counts and rates |
| Ownership | Read-only import into a new analysis run; existing Pipeline v3 owns execution, checkpoints and resume |

Explicit selection of all channels and the entire frame range is accepted.
Partial channels or ranges are refused in this profile; no trimming, channel
remapping, resampling or implicit padding makes an unsupported selection fit.
Stem sample rate and frame count need not equal the mix's sample clock, provided
their independently observed durations satisfy the explicit tolerance. Analysis
timestamps and ranges remain on the selected stem's clock.

The importer resolves declarations and accepted runtime evidence rather than
guessing a role from a filename. The first-party
[offline Demucs profile](offline-demucs.md) declares `vocals` and
`accompaniment` from stage `separate_vocals`; it does not declare drums or bass.
A different provider may declare another valid output artifact ID. Synthetic
fixtures exercise that case without importing a Demucs-specific evidence parser
into analysis consumers.

The selected run must be internally consistent and its referenced bytes must
still match accepted identities. Missing, stale, ambiguous, mismatched,
outside-workspace and unsupported mappings fail closed. A transformed upstream
source or multistage separation mapping is outside this direct-mix profile.
Provider-native JSON remains opaque even when the provider is Demucs; its
artifact identity and accepted stage relationship are what the importer binds.
The evidence input is transported as opaque bytes (`application/octet-stream`);
JSON syntax is not required or interpreted for that provider-owned content.
Every declared stem in the accepted output set must still satisfy its byte
identity and bounded media/duration checks, including unselected stems. Each
output needs exactly one accepted built-in artifact-integrity validation.
The prior run need not retain an executable separator or model installation;
the importer uses its retained provider identities and does not relaunch it.

Run and media paths must be canonical and nonsymlink. Referenced output and
authority paths must stay within the retained workspace. Import bounds include
8 MiB per evidence/authority document and 128 files per retained authority
directory. Copying one stem away from its accepted run is insufficient.

Each plan, run or resume requires the latest observed separation state to be
complete, then retains the exact accepted authority files as a historical
snapshot. The lineage stage rehashes those bound files and media before it
publishes evidence. It does not continuously monitor or lock the source run:
appending a new manifest without changing the bound historical files does not
invalidate that retained relationship during execution. Completion therefore
does not claim that the source workspace still has the latest complete state
at publication.

## Duration is not onset alignment

Let the mix and stem have sample-frame counts `M` and `S`, and sample rates
`Rm` and `Rs`. Their durations are the exact fractions `M/Rm` and `S/Rs`.
Acceptance compares their absolute difference to the selected millisecond
tolerance without rounded decimal timestamps. The endpoint within tolerance is
accepted; a larger difference is refused.

Matching zero-origin clocks and near-equal durations do not prove phase
alignment, latency compensation or synchronized onsets. No cross-correlation,
auditory review, source-separation quality evaluation or waveform equivalence
is inferred. The normalized contract's original-mix relationship remains an
identity claim supported by retained evidence, not a conversion from stem frames
to mix frames. General stream synchronization remains
[#32](https://github.com/egohygiene/aniflow/issues/32).

## Plan, inspect and analyze

Use the same explicitly pinned FFmpeg/ffprobe configuration described in the
[inspection guide](audio-inspection.md#configure-exact-local-tools). For signal
analysis, also provide its separate signal settings and tool-output bounds.
The following paths stand for synthetic media and an already accepted
separation run:

```bash
aniflow --output json audio plan \
  --input "/absolute/audio/synthetic-mix.wav" \
  --configuration "/absolute/config/tools.json" \
  --stem-run "/absolute/separation-runs/RUN_ID" \
  --stem-stage separate_vocals --stem-id vocals

aniflow --output json audio inspect \
  --input "/absolute/audio/synthetic-mix.wav" \
  --configuration "/absolute/config/tools.json" \
  --stem-run "/absolute/separation-runs/RUN_ID" \
  --stem-stage separate_vocals --stem-id vocals \
  --output-directory "/absolute/analysis-runs"

aniflow --output json audio analyze --analysis signal \
  --input "/absolute/audio/synthetic-mix.wav" \
  --configuration "/absolute/config/tools.json" \
  --signal-configuration "/absolute/config/signal.json" \
  --stem-run "/absolute/separation-runs/RUN_ID" \
  --stem-stage separate_vocals --stem-id accompaniment \
  --output-directory "/absolute/analysis-runs"
```

`--stem-run`, `--stem-stage` and `--stem-id` are a complete selection. Optional
`--stem-channels 0,1` lists zero-based channels. Use
`--stem-start-frame 0 --stem-end-frame FRAME_COUNT` together to make the full
range explicit; `FRAME_COUNT` is the selected stem's observed count, not the
mix's count. `--stem-duration-tolerance-milliseconds 20` makes the default
tolerance explicit. These options declare a supported selection rather than
requesting a media transformation.

`audio plan` resolves source and import evidence, rechecks pinned tools and
creates no analysis workspace. It does not execute the analysis providers.
`audio plan` and `audio resume` default to technical analysis; use
`--analysis signal --signal-configuration ...` for signal work. The existing
machine envelopes and commands remain unchanged. Successful JSON goes to
stdout; failure JSON goes to stderr with a nonzero exit.

```bash
aniflow --output json status-v3 "/absolute/analysis-runs/ANALYSIS_RUN_ID"

aniflow --output json audio resume "/absolute/analysis-runs/ANALYSIS_RUN_ID" \
  --input "/absolute/audio/synthetic-mix.wav" \
  --configuration "/absolute/config/tools.json" \
  --stem-run "/absolute/separation-runs/RUN_ID" \
  --stem-stage separate_vocals --stem-id vocals
```

Resume supplies the same selection and rechecks the imported relationship,
source, tools and run-local checkpoints. Read-only status retains its existing
meaning; it does not substitute for feature-specific resume checks.

Equivalent tasks use full-channel/full-range selection by default:

```bash
task audio:stem:inspect \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/audio/synthetic-mix.wav" \
  CONFIGURATION="/absolute/config/tools.json" \
  STEM_RUN="/absolute/separation-runs/RUN_ID" \
  STEM_STAGE=separate_vocals STEM_ID=vocals \
  OUTPUT="/absolute/analysis-runs"
```

| Task | Additional required variables |
| --- | --- |
| `audio:stem:plan` | None |
| `audio:stem:inspect` | `OUTPUT` |
| `audio:stem:resume` | `RUN`, the analysis run |
| `audio:stem:signal:plan` | `SIGNAL_CONFIGURATION` |
| `audio:stem:signal` | `SIGNAL_CONFIGURATION`, `OUTPUT` |
| `audio:stem:signal:resume` | `SIGNAL_CONFIGURATION`, `RUN` |

All six require `INPUT`, `CONFIGURATION`, `STEM_RUN`, `STEM_STAGE` and `STEM_ID`.
`ANIFLOW_BIN` optionally selects the binary, and
`STEM_DURATION_TOLERANCE_MILLISECONDS` defaults to 20. Use the direct CLI for
explicit full-channel/range declarations and provider execution limits.

## Library and retained evidence

Library callers attach a stem selection with
`AudioInspectionRequest::with_stem_selection`, using
`aniflow::audio_stem::AudioStemSelection::new(run_directory, stage_id, stem_id)`
(`StemSelection` is an alias). Optional `channels` and `range` fields can make
the full selection explicit; `duration_tolerance_milliseconds` defaults to 20.
Signal requests wrap that same inspection request, so technical and signal
execution share one import boundary.
The CLI maps arguments to the public request; it does not parse provider-native
separation evidence or own a separate lineage implementation.

The native `org.egohygiene.aniflow.audio-stem` provider exposes
`aniflow/audio-stem-lineage` version `1.0.0` through the existing fixed provider
ABI. Its source-bound
[configuration schema](../providers/audio-stem/configuration.schema.json) is
`aniflow.audio-stem.configuration/v1`; callers select inputs rather than author
that internal wrapper. The companion
[`aniflow.audio-stem-lineage/v1`](contracts/audio-stem-lineage-v1.schema.json)
retains the following facts:

| Evidence | Meaning |
| --- | --- |
| `original_mix`, `selected_stem`, `scope`, `range` | Exact observed audio identities, separate source clocks and full selected scope |
| Stage, artifact and output-port IDs | Declared relationship from the accepted separation stage |
| `relationship_evidence` | Identity of the opaque separation evidence, distinct from the new lineage companion |
| `authority_artifacts` | Exact raw-byte identities of retained plan, manifest, checkpoint, lock, report and validation files |
| Source plan/checkpoint/provider/implementation/configuration digests | Existing semantic and provider identities; canonical digests remain distinct from raw file hashes |
| `timing_basis` | `zero_origin_duration_only`; explicit duration tolerance without an onset/phase mapping |
| Upstream analysis and lineage-provider identity | Which ordinary analysis artifact was enriched and which provider produced the companion |

`AudioStemLineageReport::from_json_slice` validates the companion's closed shape
and cross-field invariants. Parsing alone does not reobserve files, authenticate
their author or establish an accepted run.

Analysis stages consume the selected immutable stem. Their raw technical and
signal reports retain their existing contracts and measurement definitions. A
final lineage stage emits relationship evidence and the final normalized
`aniflow.audio-analysis/v1` document. That document sets `source.stem` to the
declared stem ID, original-mix artifact reference and relationship-evidence ID;
every applicable scope retains the same stem ID. Raw measured values are not
rewritten to suggest an original-mix measurement.

| Workflow | Ordered stages | Exported final analysis |
| --- | --- | --- |
| Technical stem inspection | `inspect_audio` → `attach_stem_lineage` | `artifacts/audio-stem/analysis.json` |
| Signal stem analysis | `inspect_audio` → `measure_audio` → `attach_stem_lineage` | `artifacts/audio-stem/analysis.json` |

The final `analysis` export binds the `stem_analysis` artifact. The additional
`stem_lineage` export is `artifacts/audio-stem/lineage.json`. Existing `technical`
and, for signal work, `signal` exports retain their original paths. Intermediate
normalized analysis artifacts remain available within the run; consumers should
use the exported final analysis for stem scope and original-mix lineage.

The accepted plan, provider identities, selected artifact bytes, relationship
evidence, exact clocks, selection and duration policy participate in the new
analysis identity. Existing cancellation, output confinement, bounded provider
execution and run-local recovery apply. No new scheduler or checkpoint store
is added. Importing previously accepted separation artifacts is not automatic
cross-run reuse under [#34](https://github.com/egohygiene/aniflow/issues/34), and
the lineage provider's output is not
[#33](https://github.com/egohygiene/aniflow/issues/33)'s general completion gate.

The [musical adapter](audio-musical-analysis.md) consumes the lineage-enriched
analysis before estimating tempo, beats and keys. Its ordered path is
`inspect_audio` → `attach_stem_lineage` → musical analysis, and its final
normalized export retains the selected stem scope.

The delivered [transcription](audio-transcription.md),
[reviewed-lyrics alignment](audio-lyrics-alignment.md) and
[MIDI](audio-midi.md) adapters consume the selected `AudioSource` and
`AudioScope`, preserve original-mix and relationship evidence, and express
events on the selected stem's clock. Their own admitted sample-rate/channel
profiles still apply; selection never resamples a stem. The
[integrated workflow](audio-workflow.md) exercises these relationships across
explicit synthetic profiles. No adapter infers a clock mapping from a filename,
erases stem scope or reruns separation during ordinary analysis. New mapping
semantics require a separately bounded contract change.

## Synthetic validation and limits

The checkpoint uses generated PCM and accepted synthetic provider runs to
exercise both the #8-compatible vocals/accompaniment declarations and an
independent declared stem role:

| Fixture provider | Selected artifact IDs | Declared output ports |
| --- | --- | --- |
| #8-compatible synthetic Demucs dependencies | `vocals`, `accompaniment` | `vocals`, `accompaniment` |
| Independent synthetic separator | `drums`, `room_tone` | `percussion`, `ambience` |

The second fixture demonstrates that artifact IDs and output-port names need
not match. It supplies opaque non-JSON relationship bytes; its `drums` label is
an explicit synthetic declaration, not a claim about #8 or perceptual separation.
Fixtures cover exact and tolerance-boundary
durations, explicit full selection, unsupported partial selection, missing or
mismatched evidence, stale digests, confined paths, unchanged source/stem bytes
and compatible resume. Their execution results belong in the local validation
receipt; this guide does not turn an intended fixture into a passed check.

```bash
cargo test --locked --test audio_stem --test audio_stem_cli
python3 tests/audio_stem_schema.py
python3 scripts/smoke-audio-stem.py --aniflow /absolute/aniflow/target/debug/aniflow
```

The schema check requires an already installed `jsonschema` package and installs
nothing. Its [canonical example](contracts/examples/audio-stem-lineage-v1.example.json)
uses generated PCM clocks and synthetic authority markers; it is a shape fixture,
not evidence of a completed separation run. The smoke helper generates its own
separation fixtures and observes installed local FFmpeg/ffprobe identities for
real bounded inspection. Its optional `--receipt PATH` records the observations
and retained report copies.

No Demucs models or real media are needed for lineage checks. Synthetic
contract evidence does not qualify actual model inference, separation quality,
onset alignment, native macOS, alternate tool builds or a public release. The
maintainer owns review and merge before the next dependent checkpoint.
