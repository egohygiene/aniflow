# Layered validation and delivery evidence

Pipeline v3 owns acceptance of artifacts, stages, candidate masters and final
delivery. A successful provider process supplies observations; aniflow decides
whether those observations satisfy the exact planned obligations. This is the
implementation authored for [#33](https://github.com/egohygiene/aniflow/issues/33).
Tests, builds, schema checks and platform qualification are **not run** in this
pass, as requested by the maintainer under #64. See the
[checkpoint](validation/aniflow-33-checkpoint.md) for the implementation receipt.

## Declaring a gate

Every output receives mandatory byte, kind, size and cardinality observation.
Existing `aniflow.validation/artifact-integrity/v1` declarations keep their
meaning and existing report shape. Integrity alone does not establish media
decodability or temporal correctness. Additional requirements are explicit:

```yaml
validations:
  - id: "candidate-integrity"
    artifact: "candidate"
    contract: "aniflow.validation/artifact-integrity/v1"
  - id: "candidate-observation"
    artifact: "candidate"
    contract: "aniflow.validation/provider-artifact/v1"
    validator:
      capability:
        id: "aniflow/artifact.validate"
        version_requirement: "=1.0.0"
      provider:
        primary:
          registration_id: "fixture-validator"
```

Place these obligations on the producing stage. A root output's
`required_validations` names the obligations required for its candidate master.
All authored stage obligations must pass before the stage can complete,
including ones that a root output does not repeat. The coherent synthetic
[validation-v1 bundle](../conformance/validation-v1/README.md) supplies the
registrations and pipeline for this example.

For a port with multiple artifacts, obligations still name each exact artifact
ID. Each member receives mandatory integrity observation, and a provider gate
for one member does not validate its siblings. All planned members and their
individual obligations must pass before a stage checkpoint can be published.

`validator` uses the existing exact capability requirement and explicit
replacement/primary/fallback policy. Planning resolves it into the producing
stage's `validation_providers` with a complete provider lock. Registration
ordering does not substitute authority. Run and resume re-resolve every
producer and validator lock before workspace mutation or launch.

The reserved `aniflow-gate-` namespace belongs to resolved inline validators.
Authored stage/artifact IDs and output paths cannot collide with it. Validators
are executed within their producing stage's acceptance boundary and do not
become independent scheduler stages. An ordinary artifact-validator stage still
produces ordinary artifacts; its output alone is not a gate.

## Exact provider observation boundary

| Profile | Required capability kind | Inputs | Report checks |
| --- | --- | --- | --- |
| `aniflow.validation/provider-artifact/v1` | `artifact_validator` | `artifact`, `context` | Artifact identity, source lineage, provenance |
| `aniflow.validation/temporal-media/v1` | `temporal_validator` | `artifact`, `context`, `source` | The same checks plus decodability and complete source/derived temporal inspections |

Both profiles emit one file through the `report` port with artifact type
`aniflow.validator-observation/v1`. The context port has type
`aniflow.validation-context/v1`. Artifact/source types and optional stream roles
must match the bound producing artifacts. Validators must be observational,
non-content-changing providers without model, AI, network or publish authority.
Nested validation obligations are refused. Existing direct-argument execution,
process limits, cancellation and output confinement apply.

The immutable context binds the plan, validation ID/contract, producer
invocation, producer and validator locks, observed output, and ordered input
lineage. A provider returns the context digest, artifact digest, one disposition
and one check for each required criterion. `passed` is the only admitted
disposition. `failed`, `partial`, `skipped` and `unavailable` are retained as
representable observations but cannot accept an artifact. Missing, duplicated,
stale, incompatible or contradictory checks fail closed. There is no
provider-owned `accepted` shortcut.

aniflow reobserves source/input/output bytes after validation and retains the
original observation JSON alongside its normalized interpretation and exact
provider execution report. Reuse binds that raw JSON's digest and size to the
provider's recorded report artifact. The host independently checks identities,
lineage and provenance references; media observations remain attributable to
the selected executable. A hash is content identity, not a signature or proof
that arbitrary local provider code is trustworthy.

## Temporal profile and native adapter

The temporal profile additionally declares:

```yaml
temporal:
  source_artifact: "source-video"
  source_selection:
    video_stream: 0
    audio_stream: 1
    no_audio: false
    discard_streams: []
  artifact_selection:
    video_stream: 0
    audio_stream: 1
    no_audio: false
    discard_streams: []
  allow_authored_subtitles: false
```

Selections use input-global stream indexes and explicitly classify every
stream. The named source must be a direct input of the producing stage. The
host recomputes the support assessment from each full inspection and applies
[#32's exact reconstruction policy](temporal-correctness.md): contiguous CFR
video, supported zero origins, exact frame/audio duration tolerances and
compatible selected streams. Variable cadence, unsupported timestamp domains,
ambiguous selection and contradictory support assessments are refused.

`validation::native::registration` builds an explicit registration for
`org.egohygiene.aniflow.validation` / `aniflow/temporal.validate` version `1.0.0`.
The caller supplies the aniflow executable, artifact/source types, absolute
FFmpeg and FFprobe paths, exact version-banner tokens, SHA-256 pins and explicit
media-size/decode-time limits. Numeric release tokens are normalized only for
semantic component-version locks; the original banner stays exact in the
configuration. No PATH discovery, downloads or tool execution occur in the
registration builder.

At invocation, the adapter checks the pinned tool bytes and banners, inspects
private media snapshots with the pinned FFprobe, fully decodes the selected
video/audio streams of a private candidate snapshot with pinned FFmpeg, and
rechecks bound media/tool bytes. Decode failures and bounded-capture failures
cannot pass. The native profile currently requires Unix process-group support
and video stream roles; it is not an audio-only, arbitrary-media or qualified
cross-platform validator. Real media and installed FFmpeg builds have not been
qualified in this implementation pass.

## Evidence graph

| Record | Meaning | Durable link |
| --- | --- | --- |
| Component | One observed artifact and its required validation reports | Child of the stage acceptance record |
| Stage | Exact producer invocation/lock, inputs and all accepted components | `stage-checkpoint.payload.acceptance` |
| Candidate master | One declared pipeline output and its required validations | References its producing stage |
| Delivery | Every declared candidate output, under one plan and semantics version | `pipeline-run.payload.delivery` and `pipeline-run-outcome.delivery` |

All four records use `aniflow.acceptance-record/v1` and
`aniflow.layered-acceptance/v1`. Their content-addressed references use canonical
JSON v1 and SHA-256. Delivery remains within the isolated run workspace; this
does not publish media to an external service or choose suite delivery policy.

A stage checkpoint is committed only after provider success, independent
artifact observation, all required reports and the component/stage evidence
exist. Final acceptance rechecks every stage checkpoint and bound source
before appending a complete manifest with delivery evidence. Pending, skipped,
failed, cancelled and incomplete work cannot become completed delivery.

## Status, resume and compatibility

`status_v3` is read-only. It verifies plan/provider authority, checkpoint and
acceptance references, retained observations and current workspace artifact
bytes. Complete status additionally requires the candidate/delivery graph. It
does not launch a validator or repair evidence. Original local source paths
are not persisted; status can verify their plan-bound identities and retained
lineage, but cannot re-read an external original source. Run/resume require
fresh source bindings and verify those bytes.

Resume reuses a checkpoint only when its exact plan fragment, input/dependency
identities, provider locks, execution bounds, observations and acceptance graph
remain compatible. Missing evidence or changed output invalidates that boundary
and downstream consumers. Changed source or provider authority refuses resume.
A missing content-addressed record may be recreated from compatible evidence;
an existing conflicting record is not overwritten.

The new checkpoint `acceptance` and manifest/outcome `delivery` fields are
optional in the v1 transport schemas so historical documents remain readable.
Current execution semantics require them for reuse/completion. A new
`acceptance_semantics` invocation identity prevents silent promotion of legacy
checkpoints; archives without acceptance cannot report current complete status
and need an explicit compatible resume/reexecution. Pipeline v2 retains its
existing #32 checks and does not adopt this new v3 evidence protocol.

## Scope and deferred qualification

[#69](https://github.com/egohygiene/aniflow/issues/69) adds explicitly planned
multiple artifacts to output ports declared `one_or_more` or `many`; every
port must remain nonempty. `one` still requires exactly one member. Optional,
unbound and zero-output stages remain refused. Each member has a unique
artifact ID and disjoint exact file/directory path. Runtime observation,
publication, cancellation cleanup, status and reuse operate on the complete
set; there is no dynamic discovery or partial-stage acceptance.

Stages using those declarations select invocation/report v2, even when a
multi-artifact port has only one planned member. All-`one` stages keep v1.
The selected ABI and report revision are part of reuse compatibility; a v1
report cannot stand in for v2 evidence. See the
[provider contract](provider-contract.md#output-acceptance) for exact member
identity and report hashing. #34's optional cache requires the same full-set
proof; matching bytes from one member do not establish reusable completion.

The #33 and #69 Rust contract/runtime tests and Python schema tests are authored, not
executed. `task validation:conformance` is the focused future entry point. #64
owns formatting, compilation, checks, synthetic execution and broader audit
work; #24 owns the exhaustive adversarial corpus. No release, native-platform,
model-quality or real-media qualification is implied.
