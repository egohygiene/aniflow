# aniflow public contracts

## Layered validation and acceptance

The [layered validation guide](../layered-validation.md) defines explicit
provider-backed gates and component/stage/candidate-master/delivery acceptance
for Pipeline v3. These contracts and synthetic examples are authored under #33;
compilation, schema checks, tests and platform qualification remain unrun under
#64. JSON Schema describes shape; Rust and runtime checks establish cross-record
relationships and supported temporal semantics.

- [`validation-context-v1.schema.json`](validation-context-v1.schema.json):
  immutable plan, producer, validator, artifact and input authority.
- [`validator-observation-v1.schema.json`](validator-observation-v1.schema.json):
  provider observations, including explicit nonpass dispositions.
- [`validation-report-v1.schema.json`](validation-report-v1.schema.json):
  retained context, raw/normalized observation and exact execution reference.
- [`acceptance-record-v1.schema.json`](acceptance-record-v1.schema.json): all
  four acceptance layers with `aniflow.layered-acceptance/v1` semantics.
- [`validation-diagnostic-v1.schema.json`](validation-diagnostic-v1.schema.json):
  typed optional `error.validation` refusal.
- [`native-validation-configuration-v1.schema.json`](native-validation-configuration-v1.schema.json):
  explicit pinned FFmpeg/FFprobe and resource limits.

The existing v1 checkpoint, run-manifest and outcome schemas gain optional
`acceptance`/`delivery` references to preserve historical readability. Current
execution and status require the new graph for accepted completion; optional
transport fields are not permission to omit runtime evidence. Pipeline v2
retains its compatibility boundary.

## Exact temporal observations and source bindings

The [temporal guide](../temporal-correctness.md) describes explicit stream
selection, exact rational clocks, typed support/refusal and compatibility.
`temporal_inspect` is a machine-envelope command whose successful result can
still report `processing.supported=false`. Temporal errors retain the optional
`error.temporal` diagnostic. These schemas are authored for #32; execution and
qualification are deferred to #64.

- [`temporal-inspection-v1.schema.json`](temporal-inspection-v1.schema.json)
- [`temporal-artifact-index-v1.schema.json`](temporal-artifact-index-v1.schema.json)
- [`segment-plan-v2.schema.json`](segment-plan-v2.schema.json)
- [`segment-manifest-v2.schema.json`](segment-manifest-v2.schema.json)
- [`reconstruction-report-v2.schema.json`](reconstruction-report-v2.schema.json)

New short-segment records use v2 and bind exact source stream intervals. V1
schemas below remain historical; old segment state and Pipeline v2 state without
exact temporal evidence are refused for resume rather than silently upgraded.
JSON Schema describes structural shape. Reduced rational values and cross-field
clock, selection, identity, ordering and tolerance invariants require Rust.

This directory defines the machine boundary intended for scripts, `flow`, and
other independent consumers. Human console text is presentation and is not a
compatibility contract.

## Audio-analysis foundation

[`audio-analysis-v1.schema.json`](audio-analysis-v1.schema.json) defines the
closed `aniflow.audio-analysis/v1` source, observation, timing and evidence
boundary. The public Rust parser performs additional cross-field semantic
validation; JSON Schema validation alone does not establish a valid audio
analysis. See the [audio-analysis guide](../audio-analysis.md) for the exact
supported audio-only subset, compatibility policy and existing Pipeline v3
artifact/resume relationship.

This contract is the foundation checkpoint under
[#13](https://github.com/egohygiene/aniflow/issues/13), not an executable analyzer
or general validation gate. Family payloads and adapters belong to the
subsequent child issues.

## Audio technical-inspection evidence

[`audio-technical-inspection-v1.schema.json`](audio-technical-inspection-v1.schema.json)
defines the companion `aniflow.audio-technical-inspection/v1` evidence produced
by the [bounded offline audio inspector](../audio-inspection.md). It carries
exact supported WAV metadata and source/decode evidence alongside an unchanged
`aniflow.audio-analysis/v1` document. The public parsers enforce additional
cross-field invariants; a well-shaped JSON document alone does not prove that
its media bytes were inspected or its evidence is authentic.

[`audio-inspection-preflight-v1.schema.json`](audio-inspection-preflight-v1.schema.json)
defines tool-readiness observations and structured dependency refusals. A ready
preflight does not establish source-profile acceptance or successful decoding.

The inspector uses the existing provider/Pipeline v3 artifact and checkpoint
contracts. Its source and decode checks do not promote provider-owned evidence
into a general Pipeline v3 completion gate.

## Audio signal-measurement evidence

[`audio-signal-measurements-v2.schema.json`](audio-signal-measurements-v2.schema.json)
is the current companion for [bounded signal analysis](../audio-signal-analysis.md).
Its explicit true-peak algorithm distinguishes the existing ≤48 kHz method,
four qualified high-rate SWR profiles and unsupported rates. Measurement units,
channel scope, availability, source-time regions and upstream/provider
identities remain explicit.
The [v2 canonical example](examples/audio-signal-measurements-v2.example.json)
uses generated silence and synthetic tool identities; it illustrates the
contract without claiming an executed provider measurement.

The [v1 schema](audio-signal-measurements-v1.schema.json) and its
[canonical example](examples/audio-signal-measurements-v1.example.json) remain
frozen and readable with their original ≤48 kHz support policy.
The public `AudioSignalMeasurements` parser validates v1 and v2 according to
their own method, command and provider-version rules. New execution uses
provider/runtime capability `2.0.0` and implementation `aniflow-audio-signal-v2`;
old checkpoints are not silently upgraded.

The normalized `aniflow.audio-analysis/v1` document and its `1.0.0` signal-family
declaration remain unchanged. That semantic family version is distinct from
the provider's execution capability version; the referenced companion and
provider evidence identify the concrete measurement method. Authored settings
and source-bound configuration-wrapper schemas also remain `/v1`.

Signal execution composes `inspect_audio` and `measure_audio` through existing
Pipeline v3 checkpoints. Sample peak, true peak, RMS, crest factor and loudness
range remain distinct quantities; tool precision and scoped local checks do not
establish EBU compliance or release qualification.

## Audio stem-lineage evidence

[`audio-stem-lineage-v1.schema.json`](audio-stem-lineage-v1.schema.json) defines
the provider-neutral relationship companion for
[whole-stem analysis](../audio-stem-lineage.md). It binds original-mix and stem
identities, declared stage/output roles, full channel/range selection, exact
duration tolerance, retained authority-file hashes and source/provider evidence.
Its timing basis is `zero_origin_duration_only`, not proof of onset or phase
alignment. Provider-authored separation JSON remains opaque to the importer.
The [canonical example](examples/audio-stem-lineage-v1.example.json) uses
generated PCM clocks and synthetic authority markers to illustrate the shape;
it does not claim an actual separation run or model execution.

The public `AudioStemLineageReport` parser adds cross-field semantic checks;
parsing a report alone does not verify media bytes or authenticate a retained
run. The source-bound
[provider configuration](../../providers/audio-stem/configuration.schema.json)
is `aniflow.audio-stem.configuration/v1`. Native provider
`org.egohygiene.aniflow.audio-stem` and capability `aniflow/audio-stem-lineage`
are both version `1.0.0`.

An ordinary final lineage stage produces this companion and an unchanged
`aniflow.audio-analysis/v1` envelope with explicit `source.stem` and matching
scopes. Raw technical/signal reports keep their existing contracts. The import
retains an accepted-history snapshot after checking the latest complete source
run; it is not continuous source-workspace locking, a general validation gate
or automatic cross-run reuse.

## Audio musical-estimate evidence

[`audio-musical-analysis-v1.schema.json`](audio-musical-analysis-v1.schema.json)
defines the companion for [bounded musical analysis](../audio-musical-analysis.md).
It retains source/stem scope, exact provider/runtime identities, raw observations,
per-family availability, competing tempo/key candidates, ordered beat positions,
uncalibrated scores and the explicitly unsupported wider families. Estimates
have heuristic provenance and unavailable calibrated confidence.
The [canonical example](examples/audio-musical-analysis-v1.example.json) uses
generated PCM and synthetic runtime/provider markers to illustrate competing
observations; it does not claim that an analyzer measured them.

[`audio-musical-observation-v1.schema.json`](audio-musical-observation-v1.schema.json)
defines the raw adapter's 44.1 kHz observation shape.
[`audio-musical-probe-v1.schema.json`](audio-musical-probe-v1.schema.json) defines
the local dependency inventory, package/build versions and retained license
metadata identities. These transports are distinct from the normalized final
`aniflow.audio-analysis/v1` document and do not establish musical accuracy.

The public `AudioMusicalAnalysis`, `AudioMusicalObservation` and
`AudioMusicalProbe` parsers add semantic validation to their strict schemas.
`MusicalAnalysisConfiguration` uses the authored
[settings schema](../../providers/audio-musical/configuration.schema.json);
the facade binds it to source/tools/upstream analysis through the separate
[provider schema](../../providers/audio-musical/provider-configuration.schema.json).
Native provider `org.egohygiene.aniflow.audio-musical` and runtime/normalized
capability `aniflow/audio-musical-structure` use version `1.0.0`.

The ordinary `analyze_musical` stage follows inspection and optional stem
lineage. It preserves upstream evidence and produces the final normalized
analysis. Missing family estimates stay unavailable or partial; malformed
observations, dependency failures and cancelled execution do not become facts.
Provider-owned evidence remains separate from general Pipeline v3 completion
gates and release qualification.

## Observed audio-transcription evidence

[`audio-transcription-v1.schema.json`](audio-transcription-v1.schema.json)
defines the companion for [bounded offline transcription](../audio-transcription.md).
It retains original source/stem scope, exact tool/model/configuration identities,
probabilistic provenance, segment observations and explicitly unavailable word
timing/confidence. Results distinguish observed, empty and unavailable outcomes.
Only nonempty observations carry an embedded `aniflow.timed-text/v1` document;
it must preserve the observed producer, text, timing and source binding exactly.
No observation or export gains reviewed-lyrics authority.

The raw captured JSON has a digest and byte size, not a persisted artifact-path
promise. The [canonical example](examples/audio-transcription-v1.example.json)
uses synthetic protocol evidence and is not an executed real-model transcript.
[`audio-transcription-preflight-v1.schema.json`](audio-transcription-preflight-v1.schema.json)
and its [example](examples/audio-transcription-preflight-v1.example.json) define
dependency readiness/refusal independently from source acceptance or successful
inference.

Public `AudioTranscriptionReport` and `AudioTranscriptionPreflight` parsers add
semantic checks beyond schema shape. Supplied asset pins and licensing URLs
remain identity/declaration evidence rather than authenticated origin or a
verified native dependency closure. The authored
[settings schema](../../providers/audio-transcription/configuration.schema.json)
is separate from the source/tool-bound
[provider wrapper](../../providers/audio-transcription/provider-configuration.schema.json).
Provider `org.egohygiene.aniflow.audio-transcription` and capability
`aniflow/audio-transcription` use version `1.0.0`; the normalized audio foundation
is unchanged.

The ordinary `analyze_transcription` stage follows inspection and optional
lineage under existing Pipeline v3 execution and run-local checkpoints. Export
reuses the timed-text conversion boundary, retaining separate original-report
and extracted-document identities. Synthetic provider conformance does not
qualify real model accuracy, native platforms or the general completion gate.

## Reviewed-lyrics alignment evidence

[`audio-alignment-v1.schema.json`](audio-alignment-v1.schema.json) defines the
companion for [reviewed-lyrics alignment](../audio-lyrics-alignment.md). It retains
the exact reviewed JSON identity and parsed document, supplied review evidence,
source/stem binding, provider/configuration identities and conservative token
mapping. The original reviewed text remains unchanged; proposed timing has
probabilistic provenance and unavailable calibrated confidence. A supplied
review assertion is not independently authenticated, and forced alignment does
not establish that the supplied words occur in the audio.

Word outcomes distinguish candidate, unmatched and ambiguous timing; cue
outcomes additionally distinguish partial timing. Derived timed text retains
all authored cues, leaving incomplete cues untimed. Loss-aware export can retain
those cues in normalized JSON or plain text; an interval carrier refuses a cue
without complete timing instead of dropping text or inventing boundaries. The
export outcome keeps its original alignment-report identity and explicitly
identifies timing as candidate.

[`audio-alignment-preflight-v1.schema.json`](audio-alignment-preflight-v1.schema.json)
records dependency readiness separately from source-profile acceptance and
successful alignment. The configured PocketSphinx `5.1.1` version is a supplied
profile declaration, not an observed version probe. Local tool, acoustic-model
and dictionary hashes bind supplied byte identities without authenticating
origin, model quality or licensing. The authored
[settings schema](../../providers/audio-alignment/configuration.schema.json)
and source/text/tool-bound
[provider wrapper](../../providers/audio-alignment/provider-configuration.schema.json)
remain separate contracts.

Public `AudioAlignmentReport`, `AudioAlignmentPreflight` and `ReviewedLyrics`
parsers add semantic checks beyond schema shape, including exact text/timing
relationships. Provider `org.egohygiene.aniflow.audio-alignment` and capability
`aniflow/audio-lyrics-alignment` use version `1.0.0`. The ordinary
`analyze_alignment` stage follows inspection and optional stem lineage under
existing Pipeline v3 execution and run-local checkpoints. The normalized
`aniflow.audio-analysis/v1` foundation is unchanged; this evidence does not become
a general completion gate or qualify real-model accuracy.

## Probabilistic MIDI candidate evidence

[`audio-midi-v1.schema.json`](audio-midi-v1.schema.json) defines the companion
for [local MIDI candidates](../audio-midi.md). It retains the source/stem clock,
exact tool/model/configuration identities, note timing, pitch and native mean
activation. Candidate provenance remains probabilistic and calibrated confidence
is explicitly unavailable. Activation strength and derived MIDI velocity do
not establish score truth, measured loudness or instrument identity.

[`audio-midi-preflight-v1.schema.json`](audio-midi-preflight-v1.schema.json)
records dependency readiness separately from source acceptance and successful
inference. The authored [settings](../../providers/audio-midi/configuration.schema.json)
and source/tool-bound
[provider wrapper](../../providers/audio-midi/provider-configuration.schema.json)
remain separate. Caller-supplied pins and installed package observations bind
identity without authenticating publisher origin or every native dependency.

The [native observation](audio-midi-observation-v1.schema.json) and
[dependency probe](audio-midi-probe-v1.schema.json) are strict adapter contracts,
separate from the accepted normalized report. Provider
`org.egohygiene.aniflow.audio-midi` and capability `aniflow/audio-midi-extraction`
use version `1.0.0`.

Public report parsers add source bounds, ordering and cross-field validation
beyond schema shape. Technical inspection and optional stem lineage precede the
ordinary `analyze_midi` stage under existing Pipeline v3 execution and run-local
checkpoints. Extraction produces candidate evidence and normalized analysis;
Standard MIDI File creation requires a separate explicit export.

[`audio-midi-export-v1.schema.json`](audio-midi-export-v1.schema.json) describes
the immutable export package evidence. The bounded type-0, single-track subset
uses a constant 120 BPM serialization clock and PPQ 960; this is not inferred
musical tempo. Evidence records native-time and tick quantization, velocity
mapping and placeholder channel/program assignment. Independent MIDI read-back
checks pairing, order and agreement with the candidate before completion.
Unsupported features and collapsed note intervals are refused rather than
silently rewritten. Export never replaces an authored MIDI file.

[`audio-midi-notes-v1.schema.json`](audio-midi-notes-v1.schema.json) defines the
untagged normalized `notes.json` companion. Its version is bound by the export
report and the declared export profile; the candidate report remains the source
of note provenance and uncertainty.

Synthetic examples and adapter/refusal tests do not qualify actual model
inference, musical accuracy, native platforms or release readiness. The
normalized audio-analysis foundation is unchanged; candidate evidence does not
become a general completion gate or a cross-run cache.

## Lyrics and timed-text interchange

The [timed-text guide](../timed-text.md) defines the explicit plain lyrics, LRC,
SRT, WebVTT and TTML read/write subsets plus normalized JSON transport. Conversion
preserves source bytes, does not read audio or execute a model, and never invents
timing or review authority.

| Contract | Purpose | Synthetic example |
| --- | --- | --- |
| [`timed-text-v1.schema.json`](timed-text-v1.schema.json) | Normalized Unicode cues, exact timing, source identity, supplied provenance and optional audio binding | [Document](examples/timed-text-v1.example.json) |
| [`timed-text-context-v1.schema.json`](timed-text-context-v1.schema.json) | Explicit import provenance, language, overlap policy and optional audio-source declaration | [Context](examples/timed-text-context-v1.example.json) |
| [`timed-text-conversion-v1.schema.json`](timed-text-conversion-v1.schema.json) | Input/output identities, normalized-document hashes, allowed/reported losses, lexical facts and carrier omissions | [Conversion](examples/timed-text-conversion-v1.example.json) |
| [`timed-text-registry-v1.schema.json`](timed-text-registry-v1.schema.json) | Versioned list of registered format subsets and normalized transport | [Registry](examples/timed-text-registry-v1.example.json) |

The public Rust validators additionally enforce millisecond rational time,
cue ordering/overlap, source bounds and provenance relationships. Schema shape
alone does not verify artifact bytes, media synchronization or a reviewer's
identity. Reviewed-lyrics authority remains supplied evidence; a successful
conversion never promotes observed transcription to reviewed lyrics.

The new output package retains both normalized documents and a conversion
report beside the payload; the report is an atomic completion marker published
last, not an atomic rename of the whole directory. Known semantic loss requires
the specific explicit allowlist; unsupported syntax refuses. Carrier omissions separately identify
evidence that a standalone text file cannot represent. These utility contracts
do not change `aniflow.audio-analysis/v1` or introduce a provider execution,
Pipeline v3 checkpoint or general validation gate.

## Temporal provider contracts

The first-party [offline Demucs profile](../offline-demucs.md) specializes
these provider/Pipeline v3 contracts. Its closed
[`demucs-separation-v1.schema.json`](demucs-separation-v1.schema.json) describes
accepted PCM integrity/duration evidence, separate from the host checkpoint.
Its provider-owned configuration schema lives with the executable under
[`providers/demucs`](../../providers/demucs/configuration.schema.json).

The provider-native v1 contract set covers declarations, compatibility,
standalone resolution authority, lifecycle observations, and execution evidence:

- [`provider-manifest-v1.schema.json`](provider-manifest-v1.schema.json) defines
  identity, temporal capabilities, typed immutable ports, requirements,
  behavior, lifecycle support, side effects, and provenance promises.
- [`provider-configuration-v1.schema.json`](provider-configuration-v1.schema.json)
  binds effective values to exact provider, capability, and provider-owned
  configuration-schema identities.
- [`compatibility-fingerprint-v1.schema.json`](compatibility-fingerprint-v1.schema.json)
  binds input, configuration, provider, tool, codec, model, and validated-output
  evidence through a self-validating canonical SHA-256 digest.
- [`provider-lock-v1.schema.json`](provider-lock-v1.schema.json) binds the exact
  local selection, implementation digest, components, authorized effects, and
  offline state.
- [`provider-event-v1.schema.json`](provider-event-v1.schema.json) defines
  ordered lifecycle observations for one lock.
- [`provider-execution-report-v1.schema.json`](provider-execution-report-v1.schema.json)
  retains applied bounds, termination, redacted captures, validated outputs,
  events, and a self-validating report digest.
- [`pipeline-v2-processor-configuration-v1.schema.json`](pipeline-v2-processor-configuration-v1.schema.json)
  defines the provider-owned normalized configuration used by pipeline v2's
  frame, batch, audio, and whole-video compatibility adapters.

Canonical synthetic examples live in [`examples/`](examples/). The public Rust
types expose constructors and parsers for the same shapes. See the
[temporal provider contract](../provider-contract.md) for invariants, canonical
hashing, deterministic resolution, bounded runtime behavior, output acceptance,
flow mapping, and explicit isolation limits. Provider implementers should use
the [authoring guide](../provider-authoring.md) and coherent runnable
[`provider-v1` conformance bundle](../../conformance/provider-v1/README.md).

Files in [`examples/`](examples/) are synthetic contract-shape fixtures, not a
directory-relative executable provider distribution. The provider manifest,
configuration, and invocation examples align where they share identity and port
claims, but placeholder digests, paths, and executables still make the
collection non-runnable. The conformance bundle is the complete cross-document
integration example.

## Pipeline v3 planning contracts

Pipeline v3 has four independently versioned public planning documents:

- [`pipeline-v3-configuration-v1.schema.json`](pipeline-v3-configuration-v1.schema.json)
  defines strict authored `aniflow.pipeline/v3` intent.
- [`provider-registration-v1.schema.json`](provider-registration-v1.schema.json)
  defines an explicit `aniflow.provider-registration/v1` local locator for a
  manifest, effective configuration, executable, implementation, and observed
  components.
- [`pipeline-v3-plan-v1.schema.json`](pipeline-v3-plan-v1.schema.json) defines
  the self-validating `aniflow.pipeline-plan/v1` result.
- [`pipeline-v3-planning-failure-v1.schema.json`](pipeline-v3-planning-failure-v1.schema.json)
  defines typed `aniflow.pipeline-planning-failure/v1` diagnostics.

Matching canonical examples are published in [`examples/`](examples/). The
public Rust file boundary is `plan_v3`; callers with an already constructed
`PipelineV3Configuration` and `ProviderRegistry` can use the lower-level
`resolve_pipeline_v3`. The thin process boundary is `plan-v3`, which delegates
to the file facade.

The plan contains ordered stages, dependencies, port/artifact bindings,
expected artifacts, validations, requested capability declarations,
replacement/primary/fallback policy, deterministic resolution attempts, and
exact provider locks. It carries `algorithm: sha256` and
`canonicalization: aniflow.canonical-json/v1`; `plan_sha256` covers only the
`payload` serialized under that encoding. Object keys are recursively sorted,
arrays retain their declared order, insignificant whitespace is omitted, and
scalar encoding follows `serde_json`. Parsing recomputes the digest and rejects
tampering.

Local input and registration-locator paths are never canonical identity.
Observed input content identity, caller-declared input type and roles, the full
supplied host observation, and provider, capability, configuration,
implementation/executable, tool, codec, model, authorization, offline,
artifact, and validation identity are covered. Timestamps, run IDs, temporary
roots, the authored pipeline description, human diagnostic prose, logs, and
telemetry are excluded.

Planning itself is read-only. It uses only explicitly supplied `ID=PATH`
inputs, registration documents, host-resource observations, side-effect
grants, and offline state. It performs no implicit discovery, process launch,
network access, workspace creation, or output write. Pipeline v3 execution and
resume are separate APIs and commands that consume the exact resulting plan.
Pipeline v2 `plan`, `run`, and `resume` retain their existing contracts. The
closed v3 configuration rejects the deprecated v2 renderflow handoff.

## Pipeline v3 execution and recovery contracts

The bounded Pipeline v3 executor adds five independently versioned documents:

- [`provider-invocation-v1.schema.json`](provider-invocation-v1.schema.json)
  defines the closed `aniflow.provider-invocation/v1` request passed as
  `<executable> --aniflow-invocation <request-path>`.
- [`pipeline-run-v1.schema.json`](pipeline-run-v1.schema.json) defines one
  self-validating `aniflow.pipeline-run/v1` revision in an append-only run
  history.
- [`stage-checkpoint-v1.schema.json`](stage-checkpoint-v1.schema.json) defines
  immutable `aniflow.stage-checkpoint/v1` evidence for one accepted stage
  attempt.
- [`pipeline-run-outcome-v1.schema.json`](pipeline-run-outcome-v1.schema.json)
  defines the successful `aniflow.pipeline-run-outcome/v1` command result.
- [`pipeline-run-recovery-v1.schema.json`](pipeline-run-recovery-v1.schema.json)
  defines the durable `aniflow.pipeline-run-recovery/v1` locator returned after
  execution starts but cannot complete.

Matching canonical examples live in [`examples/`](examples/). The invocation
document is ephemeral operational data: it carries effective configuration and
typed file-or-directory input/output paths, while runtime paths are excluded
from durable checkpoint compatibility identity. Providers receive only the
fixed direct-argument invocation shape; aniflow never synthesizes a shell
command.

The run manifest references the immutable plan, retains ordered stage states
and compatibility decisions, and chains each revision to its predecessor.
Every checkpoint binds the relevant stage-plan identity, invocation and
execution semantics, exact provider lock, ordered input/dependency evidence,
provider execution report, observed outputs, and required validation results.
Parsing recomputes the canonical SHA-256 digest and rejects tampering.

Before an initial run or resume mutates a workspace, aniflow rebuilds the
registry only from explicit registration documents, re-resolves each selected
provider, and requires its full lock to equal the plan. Resume re-observes the
explicit inputs and verifies accepted output content. Compatible checkpoints
are reused; changed or missing accepted output invalidates the affected stage
and its downstream consumers. Changed source identity, changed provider
authority, unsupported or tampered state, and a different plan fail closed.
There is no fallback after execution starts and no cross-run checkpoint import.

The initial executable subset is intentionally closed: one artifact per output
port, explicit file-or-directory output kind, ordered stages, and only
`aniflow.validation/artifact-integrity/v1`. Unsupported cardinality,
validation contracts, lifecycle-observer stages, and publish authority fail
before workspace creation or provider launch. `status_v3` is read-only.
Pipeline v2 state, resume, and completion markers do not consume or emulate
these contracts.

An artifact-validator capability can run within this subset as an ordinary
stage that emits immutable validation evidence. Its output is not a
provider-backed Pipeline v3 completion gate; only the built-in
artifact-integrity contract currently decides validation acceptance. The
reference conformance bundle demonstrates that distinction as well as frame,
audio, and whole-video process profiles.

## Machine envelope v1

Every command accepts `--output json` and emits one
`aniflow.machine-envelope/v1`-equivalent JSON document:

- successful documents are written to standard output;
- failed documents are written to standard error;
- `schema_version` is currently `1`;
- `command` identifies the invoked operation;
- `status` is either `success` or `error`;
- `result` contains the command-specific public result on success;
- `error.category` and `error.message` describe failure without requiring prose
  parsing.

Consumers must reject unsupported `schema_version` values. The Rust
`MachineEnvelope::from_json_slice` helper performs that check.

Short-segment operations use the same envelope with the command names
`segment_plan`, `segment_run`, `segment_resume`, and `segment_reconstruct`.
Their durable result documents are independently versioned as
`aniflow.segment-plan/v1`, `aniflow.segment-manifest/v1`, and
`aniflow.reconstruction-report/v1`.

- [`segment-plan-v1.schema.json`](segment-plan-v1.schema.json)
- [`segment-manifest-v1.schema.json`](segment-manifest-v1.schema.json)
- [`reconstruction-report-v1.schema.json`](reconstruction-report-v1.schema.json)

Pipeline v3 planning uses command name `plan_v3`. On success, `result` is the
resolved `aniflow.pipeline-plan/v1` document. A planning failure may include an
`aniflow.pipeline-planning-failure/v1` result alongside the normal envelope
error, retaining the stage and ordered provider-resolution attempts without
requiring message parsing.

Pipeline v3 execution uses command names `run_v3`, `resume_v3`, and
`status_v3`. Successful run and resume results use
[`aniflow.pipeline-run-outcome/v1`](pipeline-run-outcome-v1.schema.json) and
retain the run directory, plan digest,
latest manifest locator, outputs, and executed/reused stage IDs. A successful
status result is the newest validated `aniflow.pipeline-run/v1` manifest.
Failures keep the normal typed envelope category. Once durable execution has
started, a run or resume failure also carries an
[`aniflow.pipeline-run-recovery/v1`](pipeline-run-recovery-v1.schema.json)
result containing the run directory and an optional latest validated manifest
locator. The manifest field is omitted if current state cannot be validated;
failures before workspace startup retain no result. `resume_v3` requires fresh
explicit input and registration authority. `status_v3` requires only the run
directory and never repairs or creates state.

The generic envelope shape is described by
[`machine-envelope-v1.schema.json`](machine-envelope-v1.schema.json). Result
schemas remain tied to the `0.3.x` public Rust types until independently
versioned command-result schemas are justified by real `flow` integration.
The Pipeline v3 plan, planning-failure, run-outcome, run-recovery,
run-manifest, and stage-checkpoint values above are already versioned
exceptions.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | success |
| `2` | CLI usage or argument parsing failure |
| `3` | invalid or missing input |
| `4` | invalid or unsupported configuration |
| `5` | missing or unusable dependency |
| `6` | media inspection or interpretation failure |
| `7` | pipeline execution failure |
| `8` | invalid or incompatible run state |
| `9` | filesystem or other I/O failure |
| `70` | internal serialization or invariant failure |

The public `ErrorCategory::exit_code` mapping and CLI contract tests enforce
these values.

## Naming and compatibility

- Product names are lowercase: `aniflow`, `flow`, `optiflow`, and `renderflow`.
- New CLI options use complete long-form names.
- The former `run --output-dir` spelling remains a visible compatibility alias
  for `--output-directory` throughout `0.3.x`.
- The former `inspect --json` spelling retains its raw v0.2 inspection object
  throughout `0.3.x`; new integrations use the versioned `--output json`
  envelope.
- Additive result fields are permitted in `0.3.x`; removing or changing the
  meaning of fields requires a schema or SemVer change.
- Paths are serialized using Rust path serialization for the host platform.
- Unknown envelope versions are rejected rather than interpreted loosely.
- Canonical JSON v1 sorts object keys recursively and preserves array order;
  changing these rules requires a new contract revision.

Breaking compatibility requires an ADR update, migration notes, and contract
fixtures demonstrating both rejection and the supported replacement.
