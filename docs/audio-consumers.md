# Consuming normalized audio evidence

aniflow exports versioned audio evidence through its public library and canonical
CLI. A consumer uses the final normalized `aniflow.audio-analysis/v1` envelope
and the family companions it references. It does not parse FFmpeg, Essentia,
whisper.cpp, aeneas or Basic Pitch output to reconstruct that evidence.

This is the consumer handoff for [#51](https://github.com/egohygiene/aniflow/issues/51)
and the [audio support matrix](audio-support-matrix.md). It documents delivered
aniflow artifacts and the remaining integration work in the owning repositories.
It adds no dependency on flow, renderflow or optiflow, no downstream adapter and
no new completion or reuse gate.

## Public artifact intake

Successful audio execution and resume return an
[`aniflow.pipeline-run-outcome/v1`](contracts/pipeline-run-outcome-v1.schema.json)
through the library or machine envelope. Its `outputs` entries bind the exported
ID, artifact ID, absolute local `path` and `sha256`. Use those output locators;
the guides' run-relative filenames explain the layout but are not discovery
rules. Select the final exported `analysis`, particularly after stem lineage has
been attached. An upstream inspection analysis is retained evidence, not a
substitute for the final family result.

The normalized envelope supplies source identity, sample clock, scope, provider
identity, family outcome, observations and companion references. Family
companions carry required details that do not fit the foundation's four global
observation kinds. Reading only global observations loses signal measurements,
alignment mappings and MIDI derivation evidence.

| Evidence to retain | Exact accepted document | Consumer meaning |
| --- | --- | --- |
| Final normalized analysis | `aniflow.audio-analysis/v1` | Source, scope, time, requested family outcomes and evidence references |
| Technical inspection | `aniflow.audio-technical-inspection/v1` | Supported source metadata and decode observations |
| Signal companion | `aniflow.audio-signal-measurements/v2` | Units, windows, method-specific availability, regions and true-peak algorithm; stem role comes from final analysis plus lineage because this companion precedes the overlay; frozen `/v1` requires its own support policy |
| Stem relationship | `aniflow.audio-stem-lineage/v1` | Original-mix identity, selected whole-stem role, retained authority and duration-only relationship |
| Musical companion | `aniflow.audio-musical-analysis/v1` | Competing tempo/key estimates, beat observations, family availability and uncalibrated scores |
| Text document and conversion | `aniflow.timed-text/v1`, `aniflow.timed-text-conversion/v1` | Text authority, exact cue semantics and explicit carrier losses |
| Transcription companion | `aniflow.audio-transcription/v1` | Observed text and candidate segment timing, including empty/unavailable outcomes |
| Alignment companion | `aniflow.audio-alignment/v1` | Reviewed input retained independently of candidate, ambiguous or unmatched timing |
| MIDI companion and export | `aniflow.audio-midi/v1`, `aniflow.audio-midi-export/v1` | Probabilistic notes and explicit serialization mappings; `notes.json` follows the untagged [notes schema](contracts/audio-midi-notes-v1.schema.json) |

The [contract index](contracts/README.md) links each closed JSON Schema and
public semantic parser. Native probe/observation contracts describe an adapter
transport. They do not replace the accepted final analysis and family reports.
Some report fields identify captured native bytes by hash without promising a
persisted raw artifact; do not invent a path for such a reference.

Apply intake checks in this order:

1. Pin the supported aniflow interface and document versions. For CLI delivery,
   validate machine-envelope `schema_version`, command, status and actual exit
   together. Success is on stdout; typed failure is on stderr. Retain any
   `aniflow.pipeline-run-recovery/v1` locator after durable startup instead of
   interpreting a failed invocation as a completed analysis.
2. Resolve the exported artifact through caller-controlled local bindings.
   Reobserve its SHA-256 and size before intake, and compare with the output and
   referenced artifact identities. Artifact IDs/digests in an analysis are not
   filesystem paths or fetch authority. A parser alone does not verify bytes.
3. Validate the exact JSON Schema and additional cross-field semantics using
   the public parser, or equivalent checks in the consuming implementation.
   `AudioAnalysis::from_json_slice` validates semantics; bare Serde
   deserialization requires an explicit `validate`. Reject unsupported core
   versions, unknown core fields and inconsistent references.
4. Retain the final source, stream, sample clock, channel/stem scope and family
   companions. Keep original-mix and stem identities distinct. For the signal
   workflow, join its earlier companion to the later stem overlay by verified
   artifact and clock identity as described below. Check requested capability
   outcomes and diagnostics before selecting usable observations. Pipeline
   completion does not make every optional family available.
5. Preserve units, rational timing, provider/tool/model/configuration digests,
   confidence tags and text/MIDI authority. Any derivative mapping records its
   source report identity, mapping version and explicit losses. Never fill
   missing evidence from a filename, a model description or successful exit.

These checks are the downstream intake responsibility. aniflow's existing
Pipeline v3 checkpoint and artifact-integrity gate remain its execution
boundary; the procedure does not implement [#33](https://github.com/egohygiene/aniflow/issues/33).

## Stem overlay and companion ordering

With a selected whole stem, the signal workflow orders `inspect_audio`,
`measure_audio`, then `attach_stem_lineage`. The technical and signal reports
are immutable evidence produced before the lineage overlay. The signal
companion's `source.stem` and measurement scopes' `stem_id` remain `null`; that
absence does not declare that the measured bytes were the original mix. The
final exported `analysis` applies the verified stem identity, and the exported
`stem_lineage` binds its selected stem to the original mix and retained
separation authority. Use those two final exports to establish the signal's
stem role without rewriting its earlier companion.

Accept this documented pre-overlay relationship only after verifying that the
signal companion's source artifact ID, SHA-256 and byte size and its stream
index, sample rate, channel count, frame count and origin equal the final
analysis source with only `stem` removed. The lineage's `selected_stem` must
agree with that final source, and its `upstream_analysis_artifact` binds the
pre-overlay signal analysis. Reobserve referenced bytes through the accepted
output/history bindings and compare their identities. A matching filename,
path, duration or stem label cannot establish this join.

Musical analysis, transcription, reviewed-lyrics alignment and MIDI extraction
instead attach lineage before their analyzer stage. Their companions receive
the stem-aware source/scope directly. Do not apply the signal exception as a
general permission to ignore a missing or contradictory stem identity in
those reports. Neither ordering proves onset/phase alignment with the mix.

## Timing, authority and availability

Ranges are half-open source sample-frame intervals; rational times are exact
reduced seconds. Preserve this clock when deriving descriptions or packaging
companions. A beat marker occupies one sample frame, not a beat duration.
Whole-stem lineage uses `zero_origin_duration_only`; it does not prove onset or
phase alignment to the mix. General container offsets, multi-stream mappings
and A/V synchronization remain [#32](https://github.com/egohygiene/aniflow/issues/32).

| Native distinction | Required downstream treatment |
| --- | --- |
| `complete`, `partial`, `unavailable`, `unsupported`, `cancelled`, `failed` | Keep the requested outcome and diagnostics; missing evidence is not a zero-valued observation |
| Deterministic, heuristic, probabilistic provenance | Retain the classification and producer identity; successful validation does not establish factual accuracy |
| Calibrated, uncalibrated, unavailable, not-applicable confidence | Keep the tagged meaning; an activation or uncalibrated score is not a calibrated probability |
| Observed transcript versus supplied-reviewed lyrics | Keep separate identities and authority; transcription does not approve lyrics |
| Reviewed lyrics with candidate alignment | Preserve reviewed text independently of timing; candidate timing is not reviewed synchronization |
| Candidate MIDI versus authored MIDI | Keep separate artifacts; extracted note candidates do not replace an authored score |
| MIDI tempo, channel/program and velocity mappings | Fixed 120 BPM is a serialization clock, channel/program are placeholders, and velocity derives from activation rather than measured loudness |
| Excerpt reference | Retain existing artifact identity, source range and scope; it is not an excerpt-rendering operation or permission grant |

An empty observation and an unavailable family are distinct. A completed empty
MIDI export does not prove acoustic silence. Consumers retain explicit metric
unavailability, competing estimates and unmatched words instead of silently
converting them into certainty or discarding them.

## Explicit carrier exports

These commands consume already produced reports and write new output packages.
Replace the illustrated report paths with the corresponding accepted output
locators. They require no sibling executable or new analyzer execution:

```bash
aniflow --output json audio midi export \
  --candidate "/absolute/generated-runs/RUN_ID/artifacts/audio-midi/midi.json" \
  --output-directory "/absolute/generated-evidence/midi-candidate"

aniflow --output json audio lyrics export \
  --alignment "/absolute/generated-runs/RUN_ID/artifacts/audio-alignment/alignment.json" \
  --to webvtt \
  --allow-loss language,metadata \
  --output-directory "/absolute/generated-evidence/alignment-webvtt"
```

MIDI export retains `candidate-report.json`, `notes.json` and
`export-report.json` beside `candidate.mid`. Its export report is published last
as the completion marker and records independent read-back and quantization.
A directory without that marker is incomplete. Keep the companion reports;
a standalone `.mid` file does not carry model confidence or source authority.

The alignment export preserves candidate-timing metadata in its normalized
companion; WebVTT requires explicit language/metadata loss permission. The
conversion report is the completion marker. Additional authored fields may
require additional declared losses, and incomplete cue timing is refused for
an interval carrier. Loss permission cannot approve dropping unmatched text or
inventing timing. See the [alignment guide](audio-lyrics-alignment.md) and
[timed-text guide](timed-text.md) for each admitted carrier subset.

## renderflow Sonic DNA handoff

Consumer audit on 2026-10-01 pinned renderflow main at
[`8e866f91aa8b20255f25a553031f25fa98aa3856`](https://github.com/egohygiene/renderflow/tree/8e866f91aa8b20255f25a553031f25fa98aa3856).
The [Artifact DNA guide](https://github.com/egohygiene/renderflow/blob/8e866f91aa8b20255f25a553031f25fa98aa3856/docs/user-guide/artifact-dna.md)
and [built-in extractor](https://github.com/egohygiene/renderflow/blob/8e866f91aa8b20255f25a553031f25fa98aa3856/crates/renderflow-core/src/dna.rs)
provide format, media-type and byte-size baseline observations for audio.
The Sonic DNA fixture demonstrates the extensible shape; it is not an aniflow
ingestion implementation or evidence that tempo was analyzed.

Concrete normalization, semantic audio description, prompt guidance and
approval remain
[renderflow #397](https://github.com/egohygiene/renderflow/issues/397).
That consumer should take immutable audio plus validated normalized analysis
and family companions. aniflow retains ownership of temporal extraction;
renderflow owns DNA normalization, hygiene, optional semantic interpretation,
candidate/review state and publication packaging. Passing structured reports
to a text model does not establish that the model heard audio.

The current
[`renderflow.artifact-dna/v1` schema](https://github.com/egohygiene/renderflow/blob/8e866f91aa8b20255f25a553031f25fa98aa3856/schemas/renderflow-artifact-dna-v1.schema.json)
requires numeric observation confidence in `[0, 1]`. That shape does not directly
represent aniflow's unavailable or not-applicable confidence and does not retain
the calibrated/uncalibrated distinction by itself. This is an explicit mapping
gap. Retain the unchanged aniflow companions as authoritative evidence; defer a
lossless mapping policy to #397. Do not cast unknown confidence to zero or one,
normalize analyzer strengths into probabilities, or present a lossy core DNA
observation as equivalent evidence. The generic namespaced extension mechanism
is not an implemented or agreed confidence policy.

## flow orchestration handoff

Consumer audit on 2026-10-01 pinned flow main at
[`2102c3d48998d6fa00da3751e2ae409dfa8b6d6c`](https://github.com/egohygiene/flow/tree/2102c3d48998d6fa00da3751e2ae409dfa8b6d6c).
Its [suite boundaries](https://github.com/egohygiene/flow/blob/2102c3d48998d6fa00da3751e2ae409dfa8b6d6c/docs/integrations/suite-boundaries.md)
and [extension contract](https://github.com/egohygiene/flow/blob/2102c3d48998d6fa00da3751e2ae409dfa8b6d6c/docs/integrations/extension-contract.md)
own suite selection, immutable artifact bindings, process authority, durable
run state and artifact acceptance. Provider-envelope consistency and observed
bytes are distinct from native audio-domain validity.

The [capability matrix](https://github.com/egohygiene/flow/blob/2102c3d48998d6fa00da3751e2ae409dfa8b6d6c/docs/integrations/capability-matrix.md)
still marks the concrete aniflow adapter pending. Generic process and artifact
gates and the existing optiflow read-only adapter do not establish aniflow
compatibility. [flow #51](https://github.com/egohygiene/flow/issues/51) owns that
adapter after [aniflow #10](https://github.com/egohygiene/aniflow/issues/10)
produces an immutable independently installable release. The separately
qualified music-video profile remains
[aniflow #40](https://github.com/egohygiene/aniflow/issues/40) and
[flow #75](https://github.com/egohygiene/flow/issues/75).

A flow-owned adapter pins that release and selects its published library or
direct-argv CLI interface. It retains the aniflow-native plan, status, recovery,
output and audio evidence described above, translating only suite policy and
envelopes. It does not copy sibling source, add path dependencies, parse native
analyzer output or substitute a provider after aniflow locks its plan. A local
development checkout or contract example is not release qualification.

See [the aniflow integration guide](integrations/flow.md) for existing public
Pipeline v3 planning/run/resume interfaces. Compatible resume reobserves source,
external dependencies and accepted artifacts within the original run. It grants
no cross-run cache reuse; [#34](https://github.com/egohygiene/aniflow/issues/34)
owns that separate boundary.
