# Normalized audio-analysis foundation

`aniflow.audio-analysis/v1` defines provider-neutral source, timing and evidence
identity for the [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13).
The [#42 checkpoint](https://github.com/egohygiene/aniflow/issues/42) establishes
the public Rust model, strict JSON Schema and shared semantic validators. It
contains no analyzer, model execution, new audio CLI family or release claim.

The [published schema](contracts/audio-analysis-v1.schema.json) is the wire
shape. The Rust parser additionally checks relationships that JSON Schema
cannot express, including source scope, rational timing and event ordering.
These checks validate declared metadata consistency. They do not read media,
rehash artifact bytes, authenticate supplied review authority, verify a
calibration claim or establish source decodability, factual correctness, media
authenticity or model quality.

## Capability boundaries

| Capability | Owning checkpoint | Foundation responsibility |
| --- | --- | --- |
| Technical inspection | [#43](https://github.com/egohygiene/aniflow/issues/43) | Source and deterministic observation identity |
| Loudness, peaks, silence and clipping | [#44](https://github.com/egohygiene/aniflow/issues/44) | Measured values, units and source-time scope |
| Stem lineage | [#45](https://github.com/egohygiene/aniflow/issues/45) | Explicit stem scope; no inferred mix relationship |
| [Tempo, beat and key estimates](audio-musical-analysis.md) | [#46](https://github.com/egohygiene/aniflow/issues/46) | Estimate classification, confidence and provenance |
| [Lyrics and timed-text conversion](timed-text.md) | [#47](https://github.com/egohygiene/aniflow/issues/47) | Distinct artifact and supplied-authority identities |
| [Timestamped transcription](audio-transcription.md) | [#48](https://github.com/egohygiene/aniflow/issues/48) | Observed transcript identity |
| Reviewed-lyrics alignment | [#49](https://github.com/egohygiene/aniflow/issues/49) | Retained review authority and source-time alignment |
| MIDI candidates | [#50](https://github.com/egohygiene/aniflow/issues/50) | Candidate identity with probabilistic provenance |
| Integrated workflow | [#51](https://github.com/egohygiene/aniflow/issues/51) | Reconcile feature evidence and consumer documentation |

Capability identifiers declare a family of requested evidence. Their presence
is not a declaration that an executable provider is installed or that a later
checkpoint is implemented. Family-specific payloads and adapters ship with the
owning feature. Generalized container offsets, multi-stream selection and A/V
synchronization remain #32; broad layered validation remains #33.

## Envelope and public Rust boundary

Types are public through `aniflow::audio_analysis`. The validated parse boundary
is `AudioAnalysis::from_json_slice`; callers constructing or directly
deserializing values must call `AudioAnalysis::validate` before consumption.
Serde deserialization alone accepts the wire shape without applying every
cross-field invariant. `AudioAnalysis::canonical_json_bytes` validates first
and then applies the existing `aniflow.canonical-json/v1` encoding. It does not
wrap the analysis in a self-signed or self-authenticating document.

The JSON parser bounds input to 8 MiB; core collections are limited to 10,000
entries. These are contract resource limits, not a provider execution budget.

| Envelope field | Meaning |
| --- | --- |
| `schema`, `status` | Exact contract identifier and overall outcome |
| `source` | Artifact digest/size, selected audio stream and exact sample clock |
| `artifacts` | Logical artifact IDs with SHA-256 and byte-size claims |
| `providers` | Provider, implementation, configuration, tool/model and license evidence |
| `capabilities` | Versioned requested family outcomes and their evidence/diagnostic references |
| `observations`, `timelines` | Global values and ordered source-time events |
| `semantic_artifacts` | Explicit observed-transcript, reviewed-lyrics or candidate-MIDI identities |
| `excerpts` | Existing artifact references scoped to source frames; no rendering request |
| `diagnostics` | Stable IDs/codes, severity and human explanation |
| `extensions` | Optional namespaced supplemental data |

Artifact references are identities, not filesystem paths or fetch authority.
The caller resolves and verifies bytes through its normal artifact boundary.
Provider evidence retains `implementation_sha256`, `configuration_sha256`,
provider version, and exact tool/model version, revision and digest. Model
requirements distinguish `none_required`, `available` and `unavailable`.
License evidence is a recorded statement with an artifact reference or an
explicit unavailable reason; it is not a legal conclusion.

The capability identifiers in `AUDIO_CAPABILITY_IDS_V1` use capability version
`1.0.0`. They reserve the explicit technical-inspection, signal-measurements,
stem-lineage, musical-structure, timed-text, transcription, lyrics-alignment and
MIDI-extraction families. Outcomes distinguish `complete`, `partial`,
`unavailable`, `unsupported`, `cancelled` and `failed`. Incomplete capability
outcomes retain diagnostic references instead of silently omitting the request.
An overall `complete` outcome cannot conceal an incomplete requested capability.

## Source, scope and exact time

The first version supports an explicitly selected, single audio stream at index
zero with source-relative origin `0/1`. It does not reinterpret a nonzero or
negative container origin, infer a stream selection or map between source clocks.
The source binds the sample rate, channel count and sample-frame count to its
artifact identity. A sample frame contains one sample for each channel; it is
not a single interleaved channel value. The declared profile allows 1–768,000 Hz,
1–64 channels and 1–9,007,199,254,740,991 sample frames. These representation
limits do not qualify an analyzer for every value in that range.

`AudioRationalTime` represents exact seconds as a reduced numerator and positive
denominator. `AudioFrameRange` is a nonempty half-open range `[start, end)` on
the declared source sample clock. The start is included; the end is excluded and
may equal `source.frame_count`. Range endpoints do not use floating-point
seconds. The last source sample frame is `frame_count - 1`.

`AudioSource::time_for_frame(frame)` returns the reduced exact fraction
`frame / sample_rate_hz`. `AudioSource::frame_for_time(time)` performs the
inverse only when the time is already an exact sample-frame boundary. It
refuses negative time, unreduced fractions, fractional sample frames and times
beyond the source duration; it never floors, rounds or clamps. Both methods
allow the final exclusive boundary at `frame_count`. For a 48,000 Hz source,
frame 24,000 is exactly `1/2` second, while `1/96_000` second is refused because
it is half a sample frame.

`AudioScope.channels` explicitly lists zero-based channels in ascending order;
it never uses an empty list to mean all channels. A stem scope identifies the
same stem declared by the source. `AudioStemIdentity` retains an original-mix
artifact reference and relationship-evidence ID. This establishes a coherent
declaration, not proof of alignment with the original mix clock; #45 owns the
working [stem-lineage import](audio-stem-lineage.md). That bounded importer
retains the same source and scope semantics for later analysis families.

An excerpt refers to an artifact, a range and a scope using the same source
clock. The contract neither generates a preview nor infers the excerpt's own
local timeline from its filename.

## Observations, confidence and overlap

Only four normalized global observation kinds are introduced here:

| Kind | Value shape | Unit |
| --- | --- | --- |
| `sample_rate` | `quantity` | `hertz` |
| `channel_count` | `count` | `channels` |
| `tempo` | `quantity` | `beats_per_minute` |
| `key` | `label` | No numeric unit |

The technical observations must be deterministic and agree with their declared
source; `channel_count` describes the full source, even when an observation's
scope lists selected channels. Tempo is finite and in the v1 representational
range `(0, 1000]` BPM. Tempo and key remain heuristic or probabilistic estimates;
their confidence and producing provider are explicit.
These primitives are representation fixtures, not implemented technical or
musical analyzers. Later families supply their own richer payload contracts.

Every observation/event has a deterministic, heuristic or probabilistic
provenance class, a provider-evidence reference and evidence-artifact references.
Confidence distinguishes `calibrated`, `uncalibrated`, `unavailable` and
`not_applicable`; absence of confidence is never converted into a perfect score.
A producer's calibration or model-provenance statement remains supplied evidence
that this metadata validator cannot independently certify.

Timeline policy belongs to each `AudioTimeline.kind`:

| Kind | Range and overlap rule |
| --- | --- |
| `markers` | Each marker occupies exactly one source sample frame; markers cannot overlap |
| `disjoint_regions` | Ordered regions cannot overlap; adjacent endpoints are allowed |
| `overlapping_regions` | Ordered regions may overlap, including simultaneous/polyphonic candidate events |

Events sort strictly by `(start, end, id)` for all timeline kinds. IDs break
ties between simultaneous regions, allowing valid coincident events in an
`overlapping_regions` timeline. Valid source bounds remain required. Permitting
region overlap does not define a MIDI note payload, timed-text conversion or
alignment implementation. Those semantics stay with their feature checkpoints.

`observed_transcript`, `reviewed_lyrics` and `candidate_midi` are distinct
semantic-artifact kinds. Reviewed lyrics require explicitly supplied
`AudioReviewedAuthority` provenance; successful validation never upgrades an
observed transcript into reviewed lyrics or confirms who reviewed the content.
MIDI candidates remain candidates and do not replace authored MIDI sources.

The [timed-text utility](timed-text.md) uses a separate normalized document for
untimed, start-only and interval cues. It retains supplied review declarations
and optional audio-source binding without changing this audio envelope or
claiming that text conversion verifies media or review authority.

## Existing provider and Pipeline v3 boundary

A future audio provider emits its normalized analysis as an ordinary immutable
file artifact on a declared output port. It uses the existing
[provider contract](provider-contract.md), exact configuration schema and
provider lock, fixed direct-argument invocation, isolated output workspace and
bounded runtime. The current executor requires one artifact per output port;
multiple declared ports can carry an analysis document and separate artifacts.
No special audio runner or second checkpoint store is introduced here.

The analysis envelope retains source content and scope, provider/tool/model
identity, effective configuration identity and referenced artifact checksums.
Those fields describe the evidence itself. Existing Pipeline v3 plans and
checkpoints separately bind source inputs, declared stage/provider/configuration
semantics, output bytes, and the validation results actually obtained. A provider
or embedding application must keep those identities coherent; parsing a
standalone analysis JSON does not compare it with a live plan, provider lock or
filesystem.

Resume uses the existing exact-lock and accepted-output checks. Changing the
analysis artifact changes its observed output identity; a stale or missing
accepted output cannot be reused as a compatible checkpoint. A matching
analysis document alone grants no cross-run reuse, which remains #34. Providers
that depend on external models or tools must still supply and recheck those
identities through their normal provider integration.

The built-in `aniflow.validation/artifact-integrity/v1` gate remains the current
Pipeline v3 completion gate. Audio contract validation is artifact-specific
semantic validation callable by the producing adapter or consumer. An ordinary
stage that emits analysis or validation evidence does not become #33's future
provider-backed completion gate.

## Compatibility and extension policy

The contract identifier versions the supported core meaning. Parsers refuse
unknown core fields, enum variants and incompatible versions instead of
silently dropping them. A producer must not add new required meaning under an
unchanged contract and expect old consumers to accept it. Later family
checkpoints must explicitly decide compatible use of existing primitives or a
new versioned payload/contract.

Namespaced extensions hold optional provider-native detail. Consumers may
ignore them without losing required core meaning; extensions cannot override
source identity, time bounds, units, classification, confidence, authority or
outcome. A feature needing extension data to understand a required observation
must first define that meaning in its public contract. Persisted extensions and
diagnostics must exclude secrets and unnecessary private content.

The compatibility proposal is recorded in
[ADR-0006](architecture/governance/decisions/ADR-0006-audio-analysis-foundation.md).
It is proposed for explicit maintainer review, with no acceptance inferred from
implementation or tests.

## Synthetic evidence and downstream consumption

The checked-in examples cover
[technical observations](contracts/examples/audio-analysis-technical-v1.example.json),
[estimates](contracts/examples/audio-analysis-estimated-v1.example.json),
[unavailable capability evidence](contracts/examples/audio-analysis-unavailable-v1.example.json)
and [timeline/authority references](contracts/examples/audio-analysis-timeline-v1.example.json).
They contain synthetic identities and observations generated from in-memory
synthetic bytes. They need no media executables, model weights, network
downloads or real user media. They prove shape and semantic validation only;
later feature fixtures must establish actual provider execution and supported
media behavior.

Verify fixture reproducibility without rewriting them:

```bash
python3 scripts/generate-audio-contract-fixtures.py --check
```

Running the same generator without `--check` deliberately regenerates the
checked-in examples. With the `jsonschema` Python package already available,
run the independent published-schema checks:

```bash
python3 tests/audio_contract_schema.py
```

The schema tests and Rust semantic tests are complementary. Cross-reference,
source-clock and status consistency require semantic validation even after a
document passes JSON Schema.

renderflow can consume the versioned analysis and its referenced artifacts for
Sonic DNA and publication work without parsing individual analyzer formats.
flow can select and compose public capabilities without importing aniflow source
or moving analysis logic into suite orchestration. These are consumer boundaries,
not implementation of a renderflow or flow adapter in this checkpoint.
