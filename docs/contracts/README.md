# aniflow public contracts

## Source video to indexed frames

The [extraction guide](../video-frame-extraction.md) describes a standalone ordinary
Pipeline v3 stage that produces a complete RGB8 PNG directory, an original
`aniflow.rife-frame-index/v1` and an extraction report. Accepted artifacts can
feed the existing indexed-input recipe; extraction does not add a second
recipe coordinator or an automatically enabled processing phase.

- [Caller settings](../../providers/frame-extraction/configuration.schema.json)
  bind exact local FFmpeg/ffprobe 8.0.1 pins, stream selection and finite source,
  frame, count, capture, time and working-data ceilings. There are no arbitrary
  filter arguments, downloads or silent geometry/clock normalization.
- [Effective operation](../../providers/frame-extraction/operation.schema.json)
  binds all materialized settings and the source digest/byte count. Plans use
  the existing v3 plan contract, read and hash source/tool bytes, and leave
  native profile, geometry, count and temporal observations pending until run.
- [Extraction report](frame-extraction-report-v1.schema.json) binds the source,
  provider/configuration/tools, original index and complete PNG directory to
  contemporaneous temporal and decoded RGB24 observations. All ordered decoded
  frame hashes must agree between the explicit source conversion and the PNG
  output. Native observations precede the existing runtime acceptance gate.

The source profile remains zero-origin CFR, even square unrotated opaque8-bit
BT.709 SDR with supported selected AAC-LC or no audio. Extraction preserves the
original clock in the index and leaves original audio in the untouched source
video for later verified assembly. YUV-to-RGB conversion is explicit and can
round; lossless PNG compression does not make that conversion mathematically
lossless. Source history, visual approval and broad backend qualification are
not inferred. `native_qualified` remains `false`.

`video_extract_frames` uses the existing machine envelope, v3 plan/outcome and
run recovery locator. Explicit resume requires compatible source, tools,
configuration and accepted output evidence; loose PNGs or a report alone cannot
authorize reuse. All execution qualification remains deferred under #64.

## Fixed music-video recipe

The [recipe guide](../music-video-recipe.md) combines existing v3 child runs in a
fixed order: prepare indexed frames, optionally upscale, then assemble the
verified master and delivery. Preparation always performs immutable import;
explicit named allenk repair includes that importer in its own child run.
Repair and upscale each require either their selected configuration or an
explicit disabled reason. Assembly remains mandatory.

- [Caller settings](../../providers/music-video-recipe/configuration.schema.json)
  reuse the existing import, named repair, Upscayl and assembly settings. No
  arbitrary steps, generic repair command, ignored root importer, provider
  fallback or implicit optional-step choice is introduced.
- [Recipe plan](music-video-plan-v1.schema.json) contains the exact initial
  child v3 plan and declared final geometry. Later child plans remain `null`
  until accepted predecessor outputs exist; an explicit target bypass has no
  child plan. Future output hashes and child plans are never invented during
  initial planning.
- [Immutable intent](music-video-intent-v1.schema.json) binds the complete
  selected configuration, skip reasons, input locators and identities, selected
  component bytes, and initial child plan. It contains no mutable phase state,
  completion flags or acceptance authority.
- [Read-only status](music-video-status-v1.schema.json) derives phase state from
  the fixed child locations and existing validated v3 manifests. Its `complete`
  field is a contemporary observation, not an additional persisted state
  machine. The [outcome](music-video-outcome-v1.schema.json) exposes accepted
  final assembly outputs; the [recovery locator](music-video-recovery-v1.schema.json)
  identifies a recipe directory without claiming that any child completed.

The caller supplies the original indexed frame set and source video. Required
tools/models, source identity, settings, manual ranges and predicted downstream
geometry are admitted up front. Plans read and hash inputs without launching
native tools or creating a recipe. Each later child plan uses the actual
accepted predecessor bytes and performs fresh resource and compatibility
admission; future output sizes are not guessed or reserved. Fixed child
directories are `children/prepare_frames`,
`children/upscale` and `children/assembly`; a directory or loose output alone
cannot authorize the next child. Execution and explicit resume retain the
existing v3 checkpoint, validation, cancellation and publication owners.

Changed intent, source or selected dependencies cannot silently adopt an old
recipe. Partial child initialization, conflicting child plans and absent
acceptance evidence remain visible refusals. This recipe does not add arbitrary
workflow scheduling, original-video frame extraction, interpolation, visual
approval, frame-history verification or a broader media profile. The assembly
and upstream provider qualification limits still apply. Synthetic regressions
are authored only; all execution remains deferred under #64.

## Explicit target resolution and excerpt preview

The recipe's opt-in `target_resolution` upscale mode uses the
[closed target policy](../../providers/upscayl-recovery/target-policy.schema.json)
over both dimensions and the current frame identities. `exact` selects a
supported integer 2x, 3x or 4x result at the target; `minimum` selects the smallest
supported factor reaching both dimensions and reports any overshoot. The aspect
ratio must match. There is no implicit crop, stretch, letterbox or fractional
resize. Landscape 3840×2160 and portrait 2160×3840 are distinct targets;
3840×1080 satisfies neither.

- [Target decisions](upscale-target-decision-v1.schema.json) bind input and
  policy digests, effective geometry and `disabled`, `already_target`,
  `above_target` or `execute`. A bypass preserves the exact current input
  identity and creates no Upscayl child, model report or quality claim. Required
  neural components are empty; import and assembly retain their dependencies.
- [Resource observations](upscale-target-resources-v1.schema.json) distinguish
  geometric RGB byte arithmetic from configured PNG/working-data ceilings.
  They are not measured usage, a reservation or an enforced memory limit.
  Neural-specific fields are `null` on bypass. When executing, the resolved
  target factor replaces the settings template's `output_scale`; model-native
  scale, model pins and all other settings retain their existing owners.
- [Preview settings](../../providers/upscayl-recovery/preview-configuration.schema.json)
  require a target rule and the same neural settings. The
  [preview plan](upscayl-preview-plan-v1.schema.json) and
  [preview outcome](upscayl-preview-outcome-v1.schema.json) limit scope to
  `selected_indexed_pngs_only`, at most 256 original indexed frames and ten
  exact source seconds. A no-change target refuses a needless neural preview.
  Preview acceptance or human approval cannot authorize the unprocessed
  remainder, a complete spatial output index or final audiovisual output.

Same-aspect input already at or above the target is retained without downscale.
Explicit `force_reprocess` forbids bypass: minimum can choose a larger supported
integer result; exact refuses when reaching the target would require 1x or
downscale. Destination downscaling remains separate work.

Initial recipe evidence is labelled `original_input`; after preparation, the
decision is recomputed over `prepared_frames` and bound to accepted identities.
PNG structure observations do not invent native SAR, rotation or historical
processing evidence. The recipe retains separate native source/profile
admission, while PNG-only preview makes no original-video/audio admission claim.
Optional `target_resolution` fields are omitted from legacy recipe serialization.
The explicit importer pixel ceiling is 33,554,432 with its 8,400,000 default
unchanged; this does not qualify native high-resolution processing. Updated
schema digests remain part of exact provider-lock and resume compatibility.
Synthetic checks remain authored and unrun under #64.

## Observed frame assembly and delivery

The [assembly guide](../video-assembly.md) describes #35's two ordered Pipeline
v3 stages: `assemble_master` writes a full RGB FFV1 MP4, then `encode_delivery`
writes a lossy H264/yuv420p MP4 from that accepted master. Both use faststart and
preserve the selected AAC-LC audio packets. This is a closed FFmpeg/ffprobe
8.0.1 source interface, with native qualification still deferred under #64.

- [Caller settings](../../providers/video-assembly/configuration.schema.json)
  require exact executable paths, versions and SHA-256 pins, with finite source,
  frame, output, capture, working-data and timeout bounds. Delivery CRF/preset
  are explicit; arbitrary argv, audio reencoding, timestamp normalization and
  container fallback are not part of this profile.
- [Effective operation](../../providers/video-assembly/operation.schema.json)
  binds all materialized defaults, stage kind, source bytes and stream
  selection, exact input-index digest, frame count and actual output geometry.
  Its settings definitions are local so the schema digest covers their shape.
- [Assembly report](video-assembly-report-v1.schema.json) binds the exact
  provider/configuration/source/index/tool/output identities to contemporary
  observations. The master records the digest of the complete ordered decoded
  RGB24 frame hashes. The delivery's decoded-pixel digest is `null`, preserving
  its lossy status. Both retain full temporal inspections and matching selected
  AAC packet, timing and priming observations.

Admission accepts the original `aniflow.rife-frame-index/v1` or the spatial
`aniflow.upscayl-output-index/v1` view. Upscaled geometry comes from the actual
output declaration; the embedded original source geometry and clock remain
unchanged. The profile requires zero-origin CFR, square unrotated SDR BT.709
frames, even delivery dimensions, and either no selected audio or one supported
AAC-LC track. Unsupported clocks, codecs, color/geometry, incomplete frame
membership or unrepresentable timing are refused; process success alone cannot
accept an output. Full output decoding, rational timeline comparison, complete
master pixel comparison, AAC packet/priming comparison and MP4 atom inspection
remain separate observations.

Planning reads and hashes immutable inputs without launching native tools or
creating a run. Execution and explicit resume use the existing v3 checkpoint,
validation and publication owners. A compatible accepted master can be reused
when delivery is retried; loose media or reports do not grant reuse authority.
Resource admission is not a filesystem reservation or a process memory quota.

`source_relationship: declared_not_verified` and `historical_processing: unknown`
do not claim the supplied frames were extracted from the declared source.
`backend_qualification: invocation_observed_not_backend_qualified` and
`native_qualified: false` distinguish a particular invocation's observations
from general backend support. Deserialization, a matching schema or this
authored implementation is not native qualification, visual approval or release
evidence. Synthetic regressions are authored only; all execution remains
deferred under #64.

## Bounded Upscayl recovery

The [recovery guide](../upscayl-recovery.md) describes #37's explicit serial
per-frame and bounded native-chunk modes over existing Pipeline v3 stage
checkpoints. Machine command `video_upscale_frames` keeps the existing
plan/run-outcome contracts for planning, fresh runs and explicit resume.
Per-frame mode admits at most 1,024 independent frame stages. Chunk mode admits
at most 100,000 frames and 1,024 independent chunks of 2–256 frames, followed
by the exact complete-set collector. The final chunk may be smaller. There is
no alternate partial-checkpoint runtime or global cache prerequisite.

- [Caller settings](../../providers/upscayl-recovery/configuration.schema.json)
  bind Upscayl/FFmpeg pins, model files, separate native/requested scales,
  tile/device/thread/TTA settings and finite resource bounds. Optional
  `execution` explicitly selects `per_frame` or `bounded_chunk`; chunk size,
  native timeout and finite output capture participate in identity. Omission
  preserves per-frame behavior. Chunk capture defaults to 16 MiB per stream,
  bounded from 1 KiB through 64 MiB, without widening decoder capture.
- [Frame operation](../../providers/upscayl-recovery/frame-operation.schema.json)
  and [frame collector](../../providers/upscayl-recovery/collect-operation.schema.json)
  retain their existing effective serialized shape and schema digests.
- [Chunk operation](../../providers/upscayl-recovery/chunk-operation.schema.json)
  and [chunk collector](../../providers/upscayl-recovery/chunk-collect-operation.schema.json)
  bind the complete materialized settings and source-index/frame declarations.
  Their nested definitions are local so schema digests bind their full shape.
- [Frame report](upscayl-frame-report-v1.schema.json) and
  [chunk report](upscayl-chunk-report-v1.schema.json) record exact input/output
  mappings and byte/decode observations. A chunk's elapsed time describes its
  single native invocation; it is not invented per-frame timing.
- [Spatial output index](upscayl-output-index-v1.schema.json) retains the original
  source index and clock unchanged, with scaled output geometry and PNG digests.
  It admits up to 100,000 frames; per-mode limits remain authoritative. It is
  not automatically interchangeable with the RIFE input contract.
- [Frame recovery report](upscayl-recovery-report-v1.schema.json) and
  [chunk recovery report](upscayl-chunk-recovery-report-v1.schema.json) collect
  the complete same-run set through declared accepted dependencies.

Compatible validated frame or whole-chunk checkpoints can be reused. A chunk
is accepted only when every required output passes validation; an interrupted
unaccepted chunk retries in full, once per explicit call. Loose preexisting
PNGs and reports cannot grant reuse. Changed source/configuration/binary/model
inputs require a new run. Runtime failure evidence, writer ownership,
immutable publication and read-only status keep their existing owners. Frame
progress and measured-throughput estimates are presentation, not checkpoint
authority; chunk progress advances by its frame count after acceptance.

The source relationship stays `declared_not_verified`, historical processing
stays `unknown`, and `backend_qualification` stays `unqualified`. Exact pins do
not prove cross-GPU bitwise determinism, installed native support or visual
quality. Tests, builds, schema checks and native execution remain unrun under
#64. Historical-output adoption, the full fault matrix, native qualification
and downstream assembly remain separate work; see the
[handoff](../validation/aniflow-37-checkpoint.md).

## Observed selective frame repair

The [repair guide](../frame-repair.md) describes the next #36 checkpoint:
fresh immutable import and explicit manual-range repair in one ordered
Pipeline v3 run. Machine command `video_repair_frames` retains the existing
plan and run-outcome contracts. The native repair provider emits a complete
separate PNG directory, the existing RIFE frame-index shape with selected
digests updated, and
[`frame-repair-report-v1.schema.json`](frame-repair-report-v1.schema.json).

[Caller settings](../../providers/frame-repair/configuration.schema.json) bind
an explicit direct-image processor, literal argv and import/resource policy.
The [effective operation](../../providers/frame-repair/provider-configuration.schema.json)
also binds sorted manual ranges and all materialized defaults. Reordering the
same caller range list preserves its operation configuration identity. The report binds the
same-run import dependency and exact provider/configuration/tool identities,
preserves count/order/times/cuts, and distinguishes selection from observed byte
changes. Selected outputs fully decode; unselected files retain the same-run
import's decoded identity through observed byte equality.

Processor version/profile remain declared, external model/environment evidence
is `unobserved`, historical processing is `unknown`, and the source relationship
is `declared_not_verified`. Neither byte changes nor process success establish
visual repair success. A loose import report is not accepted-stage authority;
JSON Schema/deserialization does not establish these runtime relationships.
The generic profile introduces no named-provider alias, hidden model identity,
new checkpoint owner or native qualification. Each generic repair command starts
a fresh run.
`resume_v3` refuses exact capability `aniflow/frame-repair` version `1.0.0`
before progress, a durable resume transition or provider launch, including
forced reruns; unobserved model/environment identity cannot support checkpoint
reuse. Read-only `status_v3` remains available. Tests, schemas and native execution remain unrun
under #64; see the [handoff](../validation/aniflow-36-repair-checkpoint.md).

## Named allenk repair and resume

The separate [allenk caller configuration](../../providers/frame-repair-allenk/configuration.schema.json)
selects the image-only `allenk/GeminiWatermarkTool` interface pinned at
`7c6a99f2c3df97eb3c430ef87a0c962aea5cb80e`. Its distinct
`aniflow/frame-repair-allenk` capability uses
[closed effective settings](../../providers/frame-repair-allenk/provider-configuration.schema.json),
with an explicit `current_v2` or `legacy_v1` visible-overlay profile. An arbitrary
generic processor profile string cannot enable this authority. GargantuaX/gwr
remains a different interface and is not substituted or aliased.

`detection_threshold_percent` defaults to 25 and admits whole percentages from
0 through 100. The wrapper generates fixed decimal threshold arguments, disables
implicit legacy fallback for `current_v2`, and omits denoise options so the pinned
basic image path keeps denoising disabled. No caller argv, force mode, region,
snap, fallback region or external model selection is admitted. A skipped
single-image invocation is a failure, not an accepted successful repair.

The provider emits
[`allenk-frame-repair-report-v1.schema.json`](allenk-frame-repair-report-v1.schema.json).
It wraps the complete existing repair observations and adds the source interface
revision, observed installed processor version, explicit profile/threshold,
`embedded_in_pinned_executable` profile-asset evidence, disabled denoising,
`unobserved` host-runtime evidence and `unqualified` backend status. The source
revision identifies the inspected interface, not verified provenance of the
installed build. Neither executable identity nor an observed version establishes
all dynamic-library dependencies, native suitability or perceptual quality.
Historical processing remains unknown and the source relationship remains declared.

The same `video_repair_frames` machine command retains existing plan/run-outcome
contracts and accepts explicit resume only for the named configuration. Resume
rebuilds the exact plan, rechecks actual executable/decoder pins, and requires the
exact same-run importer and its matching configuration and artifact bindings.
The existing runtime remains the checkpoint owner. A completed compatible import
or repair stage can be reused; interruption inside repair retries that entire
stage. No per-frame checkpoint or loose-report adoption is inferred. The generic
capability's resume refusal remains unchanged. These are authored source
contracts; executable/schema/native qualification remains deferred under #64.

## Observed immutable frame import

The [frame-import guide](../frame-import.md) describes the next #36 checkpoint.
Machine command `video_import_frames` uses the existing Pipeline v3 plan when
`--plan` is selected and the existing run-outcome contract on successful
execution. Its native provider emits
[`frame-import-report-v1.schema.json`](frame-import-report-v1.schema.json)
alongside a byte-preserved complete frame directory.

The report binds source bytes, the complete existing RIFE frame index,
current provider/configuration/decoder identities, original PNG digests and
full decoded RGB24 frame hashes/counts. `historical_processing: unknown` and
`source_relationship: declared_not_verified` keep contemporary observations
separate from unproven extraction or previous processing. The
[closed configuration](../../providers/frame-import/configuration.schema.json)
materializes caller defaults before provider identity is hashed. This adds no
second input frame manifest or alternate checkpoint owner. Tests, schemas and
native execution remain unrun under #64; deserializing this report is not
runtime acceptance, visual approval or release qualification.

## Explicit frame-selection preview

The [selection guide](../frame-selection.md) describes the first read-only #36
checkpoint. `video_frame_selection_plan` adapts the existing complete
`aniflow.rife-frame-index/v1` declaration into
[`frame-selection-plan-v1.schema.json`](frame-selection-plan-v1.schema.json).
One-based inclusive user ranges become ordered half-open ordinals and exact
source-time spans. Selected/unselected counts describe intent, not observed
preservation. Index-only evidence and unknown historical processing remain
explicit; this is not accepted import, image decoding or repair. No RIFE tool
configuration is required. Tests and schemas remain unrun under #64.

## Bounded RIFE frame interpolation

The [interpolation guide](../video-interpolation.md) describes the first #110
checkpoint (#114): an optional, serial midpoint-only 2x PNG-frame provider and
read-only machine command `video_interpolation_plan`. This slice does not accept
MP4 directly or assemble/remux a final video. Broader rates, cut detection and
native host/device qualification remain parent #110 work.

- [`rife-frame-index-v1.schema.json`](rife-frame-index-v1.schema.json): imported
  temporal observations, source geometry, ordered PNG identities and explicit
  cut boundaries.
- [`rife-plan-v1.schema.json`](rife-plan-v1.schema.json): exact rational clocks,
  source/index/configuration bindings and resource-admission evidence.
- [`rife-output-index-v1.schema.json`](rife-output-index-v1.schema.json): exact
  output-frame digests and source/midpoint/cut-hold/endpoint-hold lineage.
- [Caller settings](../../providers/rife/configuration.schema.json) and
  [provider wrapper](../../providers/rife/provider-configuration.schema.json):
  closed optional CPU/explicit-Vulkan profile with pinned tool/model bytes.

JSON Schema defines structure. Rust additionally recomputes temporal support,
ordered cuts and filenames, canonical digests, resource arithmetic and exact
output mapping. `RifeOutputIndex::from_json_slice` reads the bounded closed
document and `validate_against` binds it to the recomputed source/configuration
plan. Effective provider settings require all materialized defaults before
their raw values are hashed. Imported color/temporal declarations are not independent native
inspection of the original video. Tool version remains declared, not proven by
a native version probe. Resource estimates are uncalibrated admission heuristics;
`memory_limit_enforced` and `native_qualified` are false. Synthetic schema tests
are authored in `tests/rife_schema.py`; all checks remain unrun under #64.

## Selected image-repair and Upscayl setup

The closed
[`toolchain-media-setup-v1.schema.json`](toolchain-media-setup-v1.schema.json)
request selects one profile capability and explicitly maps its decoder,
processor and, for Upscayl, two model dependencies. The selected capability's
`effective_settings` is the sole source of operation options; its closed
allenk or Upscayl option shape is documented in the request schema's definitions.
Rust resolves the capability ID and checks that shape, exact dependency kinds
and sets, side effects, backend, scales and mandatory adapter requirements.
Other profile capabilities retain their independent option shapes.

Machine command `toolchain_prepare_media` returns
[`toolchain-media-setup-preparation-v1.schema.json`](toolchain-media-setup-preparation-v1.schema.json).
A ready result contains one `configuration.json` document using the existing
[allenk caller contract](../../providers/frame-repair-allenk/configuration.schema.json)
or [Upscayl recovery caller contract](../../providers/upscayl-recovery/configuration.schema.json),
plus its canonical JSON digest. Missing, incompatible or unverified selected
dependencies retain diagnostic evidence and no files. The CLI preserves that
result inside a dependency-error envelope.

Preparation only observes explicitly selected local dependency files. It does
not read source media, write the returned configuration, launch tools, register
providers, download models or discover replacements. `ready` means fresh local
byte and supplied-observation consistency; `native_qualification` remains false
and `host_runtime_evidence` remains `unobserved`. A retained report cannot replace
the existing source-bound planning, exact provider locks and runtime checks.
The request is bounded to one MiB and canonical results to eight MiB. Schemas
and synthetic coverage are authored and unrun under #64.

## Explicit audio registration preparation

The [registration guide](../toolchain-registration.md) describes checkpoint
[#104](https://github.com/egohygiene/aniflow/issues/104): a closed
[`toolchain-audio-inspection-registration-v1.schema.json`](toolchain-audio-inspection-registration-v1.schema.json)
request and an inert
[`toolchain-registration-preparation-v1.schema.json`](toolchain-registration-preparation-v1.schema.json)
result. Machine command `toolchain_prepare_registration` derives four existing
registration/configuration/manifest/preflight documents from one explicit native-v2
audio binding and fresh bounded local file observations. An unready request
retains diagnostic evidence and no generated files. No file is written, provider
registered or process launched by preparation.

The adapter's observed digest is preparation-time evidence. The unchanged
registration format does not pin that digest: compare it with the first plan's
implementation digest and reprepare on drift. Source identity remains a caller
declaration. Result deserialization grants neither readiness nor execution
authority. These schemas, examples and tests are authored and unrun under #64.

## Bound audio toolchain preflight

The [preflight guide](../toolchain-preflight.md) introduces the closed
[`toolchain-preflight-v1.schema.json`](toolchain-preflight-v1.schema.json) and
[`toolchain-preflight-report-v1.schema.json`](toolchain-preflight-report-v1.schema.json)
contracts. They explicitly connect selected profile/inventory dependencies to
native-v2 audio-inspection stages and registrations. No registration is created
and no provider is selected by the preflight document.

Resolved plan v1 gains optional `payload.toolchain_preflight_sha256`. Absent
fields preserve historical serialized bytes and hashes; explicit null is
invalid. New guarded plans fail closed in older strict consumers. Current
run/resume require the exact document and fresh readiness before source
rebinding, cache access or provider launch. Provider-lock v1 is unchanged.
Bindings cover only the named stages. Reports do not grant execution authority
or native qualification. These #103 contracts and coverage remain unrun under #64.

## Offline toolchain setup evidence

The separately invoked [probe operation](../toolchain-probes.md) adds
[`toolchain-probe-configuration-v1.schema.json`](toolchain-probe-configuration-v1.schema.json)
and [`toolchain-probe-report-v1.schema.json`](toolchain-probe-report-v1.schema.json)
for explicit pinned FFmpeg/ffprobe diagnostic queries. Machine command
`toolchain_probe` retains process/capture/parser evidence, including incomplete
outcomes. It does not change the offline doctor/plan contracts below. Compiled
hardware backends do not establish usable devices; native qualification stays
false. These #101 contracts and tests are authored and unrun under #64.

The [toolchain guide](../toolchain-profiles.md) describes the explicit setup
boundary introduced by the first #38 checkpoint:

- [`toolchain-profile-v1.schema.json`](toolchain-profile-v1.schema.json) maps
  selected capabilities to dependency, platform, backend and settings policy.
- [`toolchain-inventory-v1.schema.json`](toolchain-inventory-v1.schema.json)
  supplies explicit local paths/digest pins and attributed observations.
- [`toolchain-report-v1.schema.json`](toolchain-report-v1.schema.json) retains
  independent facts, setup actions and pending native qualification.

Machine commands `toolchain_doctor` and `toolchain_plan` return the same report
shape. Doctor retains the report on dependency failure. `ready` describes
inventory consistency, not executable/adapter/platform qualification. Examples
are synthetic shape fixtures. Schema and Rust tests are authored but unrun
under #64. Runtime semantic validation additionally checks cross-references,
selection isolation and identity/scale relationships. These records never
grant registration, installation, processing or network authority.

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

[`audio-technical-inspection-v2.schema.json`](audio-technical-inspection-v2.schema.json)
defines the native companion emitted by the
[bounded offline audio inspector](../audio-inspection.md): classic RIFF PCM16,
packed PCM24 or finite float32. It retains exact source and native-width decode
digests, codec/sample format, frame clock and representation-aware bitrate.
Provider and runtime capability `2.0.0` use source-bound
[`provider-configuration/v2`](../../providers/audio-inspection/provider-configuration-v2.schema.json);
the authored pinned-tool settings remain `/v1`.

The frozen [technical v1 schema](audio-technical-inspection-v1.schema.json)
remains PCM16/provider `1.0.0`. `AudioTechnicalInspection::from_json_slice`
validates both versions using their own format, provider and exact-command
invariants; readable history is not an upgrade of old run locks. The normalized
`aniflow.audio-analysis/v1` envelope is unchanged. Synthetic
[PCM24](examples/audio-technical-inspection-v2-pcm24.example.json) and
[float32](examples/audio-technical-inspection-v2-float32.example.json) examples
illustrate the new contract without claiming a real tool execution.

[`audio-inspection-preflight-v1.schema.json`](audio-inspection-preflight-v1.schema.json)
retains tool-readiness observations and structured dependency refusals. A ready
preflight does not establish source-profile acceptance or successful decoding.
The inspector uses existing Pipeline v3 artifact/checkpoint/acceptance contracts;
a valid companion cannot independently prove the identity or processing of bytes.

## Audio signal-measurement evidence

Native [signal v3](audio-signal-measurements-v3.schema.json) uses explicit
[source-amplitude settings v2](../../providers/audio-signal/configuration-native.schema.json)
and [source-bound provider configuration v2](../../providers/audio-signal/provider-configuration-native.schema.json).
`NativeAudioSignalMeasurements::from_json_slice` retains `source_format`
(`pcm16`, `pcm24`, `float32`), the full-scale reference and
`aniflow.native-sample-statistics/v1` method, per-channel peak/RMS/crest,
source-frame regions and upstream identities. Finite float32 overs remain
representable. Provider/runtime capability `3.0.0` is selected explicitly;
settings v1 do not silently opt into it.

Every native integrated/short-term loudness, loudness-range and true-peak
quantity is unavailable with `unsupported_native_signal_profile`. The sample
statistics stage emits no external meter command evidence. Its synthetic
[PCM24](examples/audio-signal-measurements-v3-pcm24.example.json) and
[float32](examples/audio-signal-measurements-v3-float32.example.json) examples
are authored contract fixtures, not qualification receipts.

The legacy PCM16 [signal v2 schema](audio-signal-measurements-v2.schema.json)
retains its ≤48 kHz `ebur128` true-peak policy, four explicit high-rate SWR
profiles and unsupported rates. The [v2 example](examples/audio-signal-measurements-v2.example.json)
uses synthetic identities. [Signal v1](audio-signal-measurements-v1.schema.json)
and its [example](examples/audio-signal-measurements-v1.example.json) remain
frozen with their original ≤48 kHz policy. `AudioSignalMeasurements` reads
v1/v2 according to each method/command/provider invariant. Legacy settings v1
select provider/runtime capability `2.0.0` and report v2; historical v1
checkpoints are not silently upgraded.

The normalized `aniflow.audio-analysis/v1` envelope and its `1.0.0` signal-family
vocabulary are unchanged. That semantic family version is distinct from the
runtime capability and report versions. Signal execution composes inspection
and measurement through existing Pipeline v3 checkpoints. See the
[signal guide](../audio-signal-analysis.md) for each exact support profile.

The #80 schemas, examples and synthetic tests are authored. Tests, builds,
formatting, lint, schema, native-tool and hosted qualification remain **unrun
under #64**. Earlier PCM16 receipts do not establish native-format meter
support, actual-media results, native-host qualification or EBU compliance.

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

The provider-native contract set covers declarations, compatibility,
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
- [`provider-execution-report-v2.schema.json`](provider-execution-report-v2.schema.json)
  retains that closed shape with explicit repeated-port member identity,
  all-or-none output evidence and a digest over canonical `{schema, payload}`.
  V1 keeps its original payload-only digest.
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

The v2 [invocation](examples/provider-invocation-v2.example.json) and
[report](examples/provider-execution-report-v2.example.json) illustrate two
members on one output port. They require a matching `one_or_more` or `many`
manifest instead of the original v1 example's `one` declaration. These new
examples and schemas are authored; validation execution is deferred under #64.

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

The bounded Pipeline v3 executor adds independently versioned documents:

- [`provider-invocation-v1.schema.json`](provider-invocation-v1.schema.json)
  defines the closed `aniflow.provider-invocation/v1` request passed as
  `<executable> --aniflow-invocation <request-path>`.
- [`provider-invocation-v2.schema.json`](provider-invocation-v2.schema.json)
  defines the same direct-argument shape with direct-argv/v2 semantics and
  explicitly planned multiple members per output port.
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
The generic selective frame-repair v1 capability remains excluded from resume,
including forced reruns, because its external model/environment identity is
unobserved; use a fresh generic repair run and read-only status instead. The
distinct named allenk capability follows the exact dependency/topology checks
described above and reuses only complete compatible stage checkpoints.

The executable subset is intentionally closed: every output port is explicitly
nonempty, with exactly one artifact for `one` and one or more for `one_or_more`
or `many`. Every member has a unique artifact ID, explicit file/directory kind
and disjoint exact path. Optional/unbound output ports and zero-output stages
remain refused. Stages declaring `one_or_more` or `many` select invocation and
report v2; all-`one` stages keep v1. ABI and report revision are part of reuse
identity. Runtime reports preserve all members, ordered by `(port,
relative_path)` in v2, and the plan maps each member to its exact artifact ID.
Full-set integrity and all authored gates must pass before checkpoint
publication; there is no discovered or partially accepted member set.

Ordered stages retain mandatory `aniflow.validation/artifact-integrity/v1`
and explicit [provider-backed gates](../layered-validation.md). Unsupported
validation contracts, lifecycle-observer stages, and publish authority fail
before workspace creation or provider launch. `status_v3` is read-only.
Pipeline v2 state, resume, and completion markers do not consume or emulate
these contracts.

An artifact-validator capability can run within this subset as an ordinary
stage that emits immutable validation evidence. Its output is not a
provider-backed Pipeline v3 completion gate; an explicit resolved validation
obligation is required to participate in layered acceptance. The
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

## Cache contracts (#34)

Closed v1 schemas and shape examples cover `cache-policy`, `cache-entry`,
`cache-inspection`, `cache-operation`, `cache-decision` and `cache-diagnostic`.
Entry examples are synthetic shapes; placeholder digests are not accepted cache
proof. Runtime lookup verifies the sealed inventory and origin acceptance.
Run outcomes add optional nonempty `cache_decisions`; manifest stage records add
optional pending-rerun/excluded-checkpoint fields. Machine errors add optional
`cache` diagnostics. Older documents omit these fields; explicit false/empty
rerun fields are not the normalized representation. Changes require current
consumers of these closed contracts. See [cache operations](../cache-reuse.md).
No schema or runtime checks were executed during the #34 implementation pass.
