# Audio capability support and parent reconciliation

[#80](https://github.com/egohygiene/aniflow/issues/80) adds native PCM24/finite
float32 technical inspection and an explicit source-amplitude signal profile.
Those changes and synthetic checks are authored; execution remains **unrun under
[#64](https://github.com/egohygiene/aniflow/issues/64)**. The historical #51
receipts and machine-readable matrix below retain their original scope; they
are not qualification evidence for this extension. See the current profile
rows and [native signal guide](audio-signal-analysis.md#native-source-amplitude-profile-80).

## Historical #51 reconciliation

The [#13 audio roadmap](https://github.com/egohygiene/aniflow/issues/13)
has delivered its bounded feature checkpoints #42–#50. This matrix reconciles
their public surfaces with every original parent acceptance item for
[#51](https://github.com/egohygiene/aniflow/issues/51). It describes implemented
profiles, explicit omissions and retained evidence. The historical final #51 local validation is recorded in the
[receipt](validation/aniflow-51-local.json). Its PR #63 draft/review disposition
was a time-bound checkpoint; consult live GitHub state for current disposition.

The [machine-readable companion](validation/aniflow-51-support-matrix.json)
uses `aniflow.audio-support-matrix/v1` for scope bookkeeping. It is not a runtime
contract, provider discovery result, installed-tool inventory, model-accuracy
certificate or release qualification. Its baseline is merged main
`e0b63d2e3650efed1f4239a286db7970d8e354a5`. Historical validation receipts identify
the exact implementation and environment tested by each checkpoint; this index
does not imply that those environments are installed now. The #51 receipt pins
validated implementation `70ef699ac681194efa768d72367e7b2a97b0068a` and tree
`0a3c7c6583d6cf7de98e25fd11ca312ec3bc8b87`.

## Original parent acceptance

Stable IDs preserve the original checklist order and wording, with product names
using their canonical lowercase spelling. “Implemented” refers to the
bounded behavior below. Final integrated evidence is supplied by #51, separately
from the already merged feature evidence.

| ID | Original acceptance item | Implementation and evidence | Current disposition |
| --- | --- | --- | --- |
| ANI13-AC01 | A versioned audio-analysis schema and normalized provider-neutral model exist. | #42: [foundation](audio-analysis.md), [schema](contracts/audio-analysis-v1.schema.json), [Rust tests](../tests/audio_analysis_contracts.rs), [receipt](validation/aniflow-42-local.json). | Implemented; validated parsing adds cross-field checks beyond JSON Schema. |
| ANI13-AC02 | Technical, musical, transcript/timed-text, MIDI, and stem-aware capability boundaries are explicit. | #42–#50: profiles and public surfaces below; [contract index](contracts/README.md). | Implemented; representation limits and advertised family IDs do not imply an installed analyzer or support for all media. |
| ANI13-AC03 | At least one deterministic offline technical analyzer is integrated. | #43: [inspection](audio-inspection.md), [synthetic inspection smoke](../scripts/smoke-audio-inspection.py), [receipt](validation/aniflow-43-local.json). | Implemented for the PCM16 WAV profile; local FFmpeg/ffprobe evidence is distinct from arbitrary codec support. |
| ANI13-AC04 | Tempo/beat and key estimates carry confidence and provider evidence. | #46: [musical guide](audio-musical-analysis.md), [companion schema](contracts/audio-musical-analysis-v1.schema.json), [Rust tests](../tests/audio_musical.rs), [receipt](validation/aniflow-46-local.json). | Implemented; raw native scores and competing observations survive. Calibrated confidence is explicitly unavailable. |
| ANI13-AC05 | Reviewed lyrics remain distinct from observed transcription. | #47–#49: [timed text](timed-text.md), [transcription](audio-transcription.md), [alignment](audio-lyrics-alignment.md), [conversion tests](../tests/timed_text.rs), [alignment tests](../tests/audio_alignment.rs). | Implemented; supplied review declarations are retained, not independently authenticated. Candidate timing is not a new review assertion. |
| ANI13-AC06 | Timed-text conversions report preserved and lost semantics. | #47: [registered subsets and losses](timed-text.md), [conversion schema](contracts/timed-text-conversion-v1.schema.json), [tests](../tests/timed_text.rs), [receipt](validation/aniflow-47-local.json); #48/#49 exports reuse that boundary. | Implemented for registered subsets; named loss permission cannot invent timing or parse unsupported syntax. |
| ANI13-AC07 | MIDI outputs remain probabilistic candidates with provenance. | #50: [MIDI guide](audio-midi.md), [candidate schema](contracts/audio-midi-v1.schema.json), [export schema](contracts/audio-midi-export-v1.schema.json), [tests](../tests/audio_midi.rs), [receipt](validation/aniflow-50-local.json). | Implemented; activation is not calibrated confidence, an inferred instrument or authored-score authority. |
| ANI13-AC08 | #8 stem artifacts can participate without coupling analysis to Demucs. | #45: [lineage guide](audio-stem-lineage.md), [schema](contracts/audio-stem-lineage-v1.schema.json), [independent separator tests](../tests/audio_stem.rs), [receipt](validation/aniflow-45-local.json); [#8 receipt](validation/aniflow-8-local.json). | Implemented; accepted stage/artifact identities and opaque relationship evidence are consumed without Demucs-native parsing. |
| ANI13-AC09 | All external execution is bounded, cancellable, and offline-policy aware. | Existing [provider contract/runtime](provider-contract.md), [Pipeline v3 schema guide](pipeline-schema.md), feature requests and tests #43–#46/#48–#50. | Implemented within declared Unix execution profiles; direct invocation and staging are not an OS sandbox or full native dependency closure. |
| ANI13-AC10 | Structured unavailability explains missing tools/models rather than silently degrading. | Feature preflight schemas in the [contract index](contracts/README.md); [inspection tests](../tests/audio_inspection.rs), [transcription tests](../tests/audio_transcription.rs), [alignment tests](../tests/audio_alignment.rs), [MIDI tests](../tests/audio_midi.rs). | Implemented; missing dependencies may refuse before a run exists. Supported inspection plus an unsupported inference profile can instead publish partial analysis with explicit unavailable capability evidence. |
| ANI13-AC11 | Outputs have ordered timing, validation, digests, and resume-compatible provenance. | #42–#50 semantic parsers, [Pipeline v3 run evidence](pipeline-schema.md), feature failure/resume fixtures and local receipts. | Implemented; #51 cross-checks combined evidence. Run-local resume reobserves mutable dependencies; generalized clock mapping, layered completion gates and cross-run reuse remain #32/#33/#34. |
| ANI13-AC12 | Synthetic fixtures require no paid API or live model download. | Feature generators, smoke helpers and receipts below; #51 integrated fixture index. | Implemented; synthetic provider conformance does not establish actual learned-model inference or quality. |
| ANI13-AC13 | Documentation explains consumption by renderflow Sonic DNA and orchestration by flow. | #51: [consumer handoff](audio-consumers.md), this matrix and [provider contract](provider-contract.md). | Documented and locally validated in #51; maintainer review remains pending. Actual downstream integrations remain [renderflow #397](https://github.com/egohygiene/renderflow/issues/397) and [flow #51](https://github.com/egohygiene/flow/issues/51). |
| ANI13-AC14 | Formatting, tests, docs, and CI pass. | Feature local receipts and fresh #51 local validation. | Required #51 local checks passed in the [receipt](validation/aniflow-51-local.json). The active parent discipline accepts this bounded local evidence; hosted CI remains explicitly unverified, with no pass inferred. |

The parent checkpoint plan explicitly assigned wider musical-family support
dispositions to #46. Its merged schema and guide retain unsupported families;
they have not disappeared from the original scope. The active parent discipline
also replaced waiting for hosted CI with recorded meaningful local checks and
an explicit unverified hosted-CI gate. Implementation does not accept the
proposed [ADR-0006](architecture/governance/decisions/ADR-0006-audio-analysis-foundation.md).

## Actual media and feature profiles

Every executing analysis uses an immutable source and existing Pipeline v3
runtime. Audio timestamps refer to one selected stream at index zero with origin
`0/1`; source frames and exact rational seconds retain their declared clock.
These profiles are narrower than the normalized envelope's representation
limits. Input extensions do not establish admissible content.

| Family | Input/profile boundary | Available evidence | Explicit boundary |
| --- | --- | --- | --- |
| Normalized foundation | `aniflow.audio-analysis/v1`; one zero-origin stream; source-clock representation of 1–768,000 Hz and 1–64 channels | Artifact/provider/tool/model/config identities, capability outcomes, observations, timelines, authority and excerpt references | Metadata validation only; an envelope cannot certify source bytes, decodability, authenticity or analyzer support. |
| Technical inspection | Classic RIFF32 PCM16/packed PCM24 or finite float32, mono/stereo, 8–192 kHz, ≤600 seconds and 256 MiB | Technical v2: independent RIFF inspection, native-width decode digest, exact frames/duration, representation-aware bitrate; frozen PCM16 v1 remains readable | #80 authored/unrun. No RF64, extensible WAV, PCM32 integer, float64, non-finite floats, compressed samples, multistream offsets, implicit normalization or previews. |
| Legacy PCM16 signal measurements | PCM16 technical profile; sample rate divisible by ten; settings v1/report v2 | Sample peak, RMS, crest factor, per-channel silence/clipping-threshold regions; integrated/short-term loudness, loudness range and true peak when available | Integrated loudness needs ≥400 ms and qualifying gated content; full short-term windows need 3 seconds; loudness range needs ≥60 seconds and ten qualifying windows. Silent/unqualified measurements retain reasons. |
| Legacy PCM16 true peak | Legacy signal profile; ≤48 kHz, or explicit 88.2/96/176.4/192 kHz paths from #55 | ≤48 kHz padded ebur128 pass; qualified high rates use fourfold SWR interpolation and floating-point peak scan | Other rates above 48 kHz report `unsupported_true_peak_rate`; other signal measurements remain available. Interpolation changes only the private measurement stream. |
| Native sample measurements | Native technical profile; explicit ratio-based settings v2/report v3 | Per-channel sample peak, RMS, crest factor and source-frame silence/clipping regions; finite float overs preserved | #80 authored/unrun. Integrated/short-term loudness, loudness range and true peak are all `unsupported_native_signal_profile`, including native-profile PCM16. Direct-source support only; stem import/model profiles remain PCM16-only. No conversion fallback. |
| Stem lineage | Accepted complete separation run; one direct original-mix input; 1–8 immutable PCM16 WAV stems and one relationship-evidence output; full selected stem/channels | Retained accepted authority/artifact identities, selected stem and original mix, exact own clocks, duration tolerance | 0–20 ms explicit tolerance, default 20 ms. No partial-channel/range views, trimming, resampling or onset/phase mapping. Evidence contents remain opaque. |
| Musical estimates | PCM16 WAV, 44,100 Hz mono/stereo, 8–600 seconds, ≤256 MiB; pinned Essentia 2.1b6.dev1389 profile | Tempo and beat candidates; separate krumhansl/temperley key/mode observations; native scores and disagreement | Stereo arithmetic mean is explicit analysis preprocessing. No source resampling; calibrated confidence and wider families below are unavailable/unsupported. |
| Observed transcription | PCM16 WAV, 16,000 Hz mono, ≤600 seconds/256 MiB; pinned whisper.cpp 1.8.7 English tiny.en CPU profile | Probabilistic observed segment text and source-relative 10 ms timing; selected English language evidence; empty result retained | No word timing, calibrated confidence, language detection, translation, VAD, GPU or broader model profiles. No implicit downmix/resampling. Actual model inference remains unverified. |
| Reviewed-lyrics alignment | PCM16 WAV, 16,000 Hz mono, ≤600 seconds/256 MiB; pinned PocketSphinx 5.1.1 English CPU profile | Candidate word/cue timing, exact reviewed input bytes/text, token mapping and supplied review/revision identity | Reviewed JSON ≤64 KiB, ≤512 tokens, ≤16 KiB phrase; bounded English token/dictionary profile. Missing/ambiguous mappings remain partial/untimed; no acoustic correctness or singing-quality claim. |
| MIDI candidates/export | PCM16 WAV, 22,050 Hz mono, ≤120 seconds/8 MiB; pinned Basic Pitch 0.4.0 ONNX CPU profile | ≤8,192 note candidates, pitch 21–108, native activation, exact source-relative candidate timing; explicit type-0 single-track SMF export and read-back | No inferred instrument, channel, authored tempo map or score truth; export uses declared 120 BPM/PPQ 960 serialization and recorded quantization/mapping losses. No pitch bends, downmix or resampling. |
| Timed-text interchange | Explicit plain/LRC/SRT/WebVTT/TTML/normalized JSON subsets; UTF-8 and declared cue/overlap policy | Exact supplied rational timing, preserved text/provenance/audio binding, lexical facts and named conversion losses | No automatic format guessing, arbitrary subtitle/XML styling, inferred cue ends or lyric timing. New output package; completion report published last. |

Musical `unsupported_families` are exactly `downbeats`, `meter`, `tempo_changes`,
`chords`, `onsets`, `rhythm_density`, `sections`, `spectral` and `timbre`. A beat
estimate is not an onset detector or verified meter grid; a key estimate is not
a chord timeline. Original broad musical possibilities remain visible with
this explicit disposition.

No single current sample rate qualifies a recording for all optional adapters.
The integrated synthetic slice uses separate generated source/profile branches
and preserves their clocks and digests. A consumer cannot infer a cross-profile
time mapping or apply hidden resampling to make a recording fit. Generalized
timing and stream selection remain [#32](https://github.com/egohygiene/aniflow/issues/32).

The [bounded fixture index](validation/aniflow-51-fixture-index.json) preserves
existing local case IDs under distinct family namespaces, with generator,
executable, test and historical receipt references. It includes the integrated
slice's 28 cases, whose final execution and captured-evidence checks are
recorded in the #51 local receipt.
This index contributes evidence to #24; it is not that broader corpus's complete
manifest or closeout.

## Public surfaces and companion evidence

All executing requests use their owning public module's `plan`, `run` and
`resume`; feature guides document preflight availability and settings. The CLI
uses those library boundaries. Task wrappers are in the shared
[Taskfile](../Taskfile.yml); task names identify commands, not an additional
provider or scheduler. `audio:resume` remains the Demucs separation task.

| Family | Public Rust boundary | Canonical CLI and Task wrappers | Companions and local receipt |
| --- | --- | --- | --- |
| Foundation | [`audio_analysis::AudioAnalysis`](../src/audio_analysis.rs): validated parse/validate/canonical bytes | `audio:contracts`, `audio:contracts:schema` verify fixtures; no inference command | [audio-analysis/v1](contracts/audio-analysis-v1.schema.json); [#42](validation/aniflow-42-local.json) |
| Technical | [`audio_inspection`](../src/audio_inspection/mod.rs) / `AudioInspectionRequest` | `audio plan`, `audio inspect`, `audio resume`; `audio:inspect:plan`, `audio:inspect`, `audio:inspect:resume` | [technical/v2](contracts/audio-technical-inspection-v2.schema.json), readable [technical/v1](contracts/audio-technical-inspection-v1.schema.json), [preflight/v1](contracts/audio-inspection-preflight-v1.schema.json), normalized analysis; [#43](validation/aniflow-43-local.json) |
| Signal | [`audio_signal`](../src/audio_signal/mod.rs) / `SignalAnalysisRequest` | `audio plan`, `audio analyze`, `audio resume` with `--analysis signal`; `audio:signal:plan`, `audio:signal`, `audio:signal:resume` | native [signal/v3](contracts/audio-signal-measurements-v3.schema.json), legacy PCM16 [signal/v2](contracts/audio-signal-measurements-v2.schema.json), frozen [signal/v1](contracts/audio-signal-measurements-v1.schema.json), normalized analysis; [#44](validation/aniflow-44-local.json), [#55](validation/aniflow-55-local.json) |
| Stem | [`audio_stem`](../src/audio_stem/mod.rs) / `StemSelection` attached to inspection requests | Executing audio commands add `--stem-run`, `--stem-stage`, `--stem-id`; six `audio:stem:*` technical/signal wrappers | [lineage/v1](contracts/audio-stem-lineage-v1.schema.json), exported final normalized analysis; [#45](validation/aniflow-45-local.json) |
| Musical | [`audio_musical`](../src/audio_musical/mod.rs) / `MusicalAnalysisRequest` | `audio plan`, `audio analyze`, `audio resume` with `--analysis musical`; `audio:musical:plan`, `audio:musical`, `audio:musical:resume` | [musical/v1](contracts/audio-musical-analysis-v1.schema.json), [probe/v1](contracts/audio-musical-probe-v1.schema.json), normalized analysis; [#46](validation/aniflow-46-local.json) |
| Transcription | [`audio_transcription`](../src/audio_transcription/mod.rs) / `TranscriptionRequest`, `export_transcript_file` | `audio transcribe`; plan/resume use `--analysis transcription`; `audio transcript-export`; `audio:transcription:plan`, `audio:transcribe`, `audio:transcription:resume`, `audio:transcript:export` | [transcription/v1](contracts/audio-transcription-v1.schema.json), [preflight/v1](contracts/audio-transcription-preflight-v1.schema.json), normalized analysis, optional timed text; [#48](validation/aniflow-48-local.json) |
| Alignment | [`audio_alignment`](../src/audio_alignment/mod.rs) / `AlignmentRequest`, `export_alignment_file` | `audio lyrics align`; plan/resume use `--analysis lyrics-alignment`; `audio lyrics export`; `audio:lyrics:plan`, `audio:lyrics:align`, `audio:lyrics:resume`, `audio:lyrics:export` | [alignment/v1](contracts/audio-alignment-v1.schema.json), [preflight/v1](contracts/audio-alignment-preflight-v1.schema.json), normalized analysis and derived timed text; [#49](validation/aniflow-49-local.json) |
| MIDI | [`audio_midi`](../src/audio_midi/mod.rs) / `MidiRequest`, `export_midi_file` | `audio midi extract`; plan/resume use `--analysis midi-candidates`; `audio midi export`; `audio:midi:plan`, `audio:midi:extract`, `audio:midi:resume`, `audio:midi:export` | [MIDI/v1](contracts/audio-midi-v1.schema.json), [preflight/v1](contracts/audio-midi-preflight-v1.schema.json), [export/v1](contracts/audio-midi-export-v1.schema.json), [untagged notes companion](contracts/audio-midi-notes-v1.schema.json); [#50](validation/aniflow-50-local.json) |
| Timed text | [`timed_text`](../src/timed_text/mod.rs): registry/decode/encode/convert/convert_file | `timed-text formats`, `timed-text convert`; `timed-text:formats`, `timed-text:convert` | [document/v1](contracts/timed-text-v1.schema.json), [context/v1](contracts/timed-text-context-v1.schema.json), [conversion/v1](contracts/timed-text-conversion-v1.schema.json), [registry/v1](contracts/timed-text-registry-v1.schema.json); [#47](validation/aniflow-47-local.json) |

The normalized audio family vocabulary uses capability version `1.0.0`.
Legacy PCM16 signal execution declares provider capability `2.0.0` and emits
report v2; explicit native execution declares capability `3.0.0` and report v3; those versions describe different contracts and must not
be collapsed into a single “audio version.” Timed-text conversion is an
independent file facade rather than an executing audio provider.

## Runtime completion and capability availability

Consumers must read both Pipeline v3 outcome and feature evidence. A run can
complete successfully because it validly produced partial/unavailable analysis.
Successful JSON envelopes go to stdout; structured command failures go to stderr
with nonzero exit. Process exit and artifact existence remain separate facts
from accepted outputs and their semantics.

| Evidence state | Consumer meaning |
| --- | --- |
| Run complete, analysis/capability complete | Declared structural work and required artifact checks completed for the requested profile; estimates remain estimates. |
| Run complete, analysis/capability partial | Useful accepted evidence exists alongside explicit missing quantities or mappings. Inspect per-family reasons; do not replace unavailable values with zero. |
| Run complete, optional capability unavailable | Supported technical evidence can survive an unsupported inference rate, silence or insufficient duration. No invented BPM, transcript, cue or note is supplied. |
| Missing/stale dependency refusal | Feature preflight carries diagnostics; no successful run or unavailable-analysis artifact is promised. Reobserve dependencies before retry/resume. |
| Failed/cancelled/interrupted run | Recovery state/checkpoints are retained when available; no incomplete current stage is accepted as complete. Resume through the feature-specific entry point rechecks mutable dependencies. |
| Read-only `status-v3` | Inspect persisted evidence; do not infer that current external tool/model pins were reprobed. |
| Incomplete export directory | Absence of `conversion.json` or MIDI `export-report.json` completion marker means the package is incomplete. Validate its companion identities before consuming it. |

Examples include explicitly unavailable signal loudness range for short input;
partial alignment retaining untimed cues; an empty transcription retaining no
placeholder cue; and an empty MIDI candidate whose normalized MIDI capability
is unavailable. An explicit empty MIDI candidate may be exported with no note
events and `empty_candidate: true`; that is not proof that the source is silent.
The export does not upgrade the analysis capability to complete.

## Consumer and downstream disposition

The [consumer guide](audio-consumers.md) describes byte/contract verification,
final export selection, uncertainty, authority and own-clock preservation.
renderflow owns Sonic DNA interpretation and publication metadata. Its concrete
mapping remains [renderflow #397](https://github.com/egohygiene/renderflow/issues/397).
flow owns cross-holon orchestration; its independently installable aniflow
adapter remains [flow #51](https://github.com/egohygiene/flow/issues/51). This
checkpoint documents those seams without shipping sibling adapters or importing
their source.

The earlier #51 sequencing through #32, #33, #34 and #24 is historical.
Current authored native-format work is #80/#93–#95; accumulated execution
qualification remains #64. [#10](https://github.com/egohygiene/aniflow/issues/10)
release qualification and the immutable-release-dependent flow #51 integration
remain separate from implementing this profile. Source-time excerpt references are implemented; rendered previews
and generalized mappings are not implied. Actual learned-model accuracy,
native macOS/other-platform qualification, full native dependency closure,
broader adversarial coverage, hosted CI and releases retain separate evidence
gates.


The [flow #78 AMV preset](https://github.com/egohygiene/flow/issues/78) is a
separate consumer workflow. This native inspection checkpoint supplies evidence
contracts; it does not implement that preset, qualify actual media, or broaden
PCM16-only musical, transcription, alignment or MIDI adapters.
