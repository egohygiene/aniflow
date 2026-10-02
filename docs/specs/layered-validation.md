# Layered validation and delivery (#33)

## Boundaries

aniflow owns acceptance. A provider process, an observed artifact, a validation
observation, an accepted stage, and a delivered output are distinct facts.
Pipeline v3 keeps its ordered stages. Validators execute inside a producing
stage's acceptance boundary; they are not ordinary downstream processing stages.
flow consumes public evidence and does not import this implementation.

## Contracts and data flow

1. Authored validation obligations retain the existing `id`, `artifact`, and
   `contract` fields. An optional explicit provider requirement selects a
   validator capability using the existing registration/selection policy.
2. Planning resolves each validator to an exact capability and provider lock.
   Synthetic validator bindings use `artifact` and `context` inputs and one
   `report` output. Nested validators are forbidden. No process runs in planning.
3. Execution independently observes every produced artifact and rechecks inputs.
   An immutable context binds the validation to the plan, producer invocation,
   artifact, lineage, policy and exact validator lock. A bounded validator runs
   with the existing direct-argv ABI. Its observation cannot accept delivery.
4. aniflow validates that observation, independently checks all identities, and
   evaluates supported temporal invariants. Failed, partial, unavailable,
   duplicate, incompatible or stale evidence is rejected. Source and artifact
   identities are reobserved after validator execution.
5. Versioned content-addressed records cover components, stages, candidate
   masters and delivery. Checkpoints and manifests reference these records.
   Resume reuses only compatible evidence; read-only status rechecks delivery
   evidence and current workspace bytes before reporting complete.

## Compatibility and support

Existing artifact-integrity declarations remain valid and do not imply media
decodability or temporal quality. All artifacts receive mandatory integrity
acceptance. Additional validation requirements are explicit and plan-bound.
The temporal profile uses #32's exact contiguous CFR, zero-origin policy and
its rational frame/audio tolerances. Provider observations include complete
source/derived inspections and full-decode disposition; aniflow recomputes
temporal acceptance. Unsupported clocks are refused. Probabilistic model output
cannot establish delivery truth.

[#69](https://github.com/egohygiene/aniflow/issues/69) extends the executable
subset to explicitly planned nonempty `one_or_more` and `many` output ports.
`one` still requires exactly one member; optional/unbound ports and zero-output
stages are refused. Every output member has a unique artifact ID, explicit
kind and disjoint exact path. There is no discovered fan-out or partial success.

Any multi-artifact declaration selects invocation/direct-argv v2 and execution
report v2. All-`one` stages retain v1. V2 reports use unique sorted
`(port, relative_path)` member identity and hash canonical `{schema, payload}`;
v1 retains its payload-only hash. A plan maps every observed member to its exact
artifact ID. Gates remain per artifact: every member receives integrity checks,
and every declared provider observation must pass before stage publication.
Cancellation/failure cannot publish a partial checkpoint. Status, resume and
cache proof compare the complete member set and selected evidence revision;
missing, swapped or altered members invalidate completion.
Pipeline v2 remains a compatibility path with its existing temporal checks;
the new provider-gated delivery protocol is a Pipeline v3 contract.

## Checkpoints and acceptance

- #66: public contracts and deterministic exact validator planning.
- #67: runtime acceptance, cancellation, resume and status gates.
- #68: schemas, synthetic coverage and consumer documentation.

Implement the original #33 criteria without weakening them. Author focused
tests for success, missing/duplicate/contradictory evidence, wrong bytes or lock,
failed/skipped observations, cancellation, tampering and reuse. Test execution,
builds, formatting checks, lint, smoke, package qualification and hosted CI are
explicitly deferred by the maintainer under #64. Authored coverage is not a pass.
