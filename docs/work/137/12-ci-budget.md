# Flow #137 checkpoint 12 — November 13, 2026

The existing `Workflow expressions` workflow is the single path-independent
five-minute source gate for pull requests and main pushes. Its job/check name
is retained. It runs the pinned Actionlint source/expression check, synthetic
stdlib budget regressions and a bounded JSON source budget plan. It does not
compile products, qualify providers, upload artifacts or establish release proof.
`task ci:budget` and `task ci:budget:test` expose the source checks locally.
Aniflow and Renderflow also run their synthetic read-only release-admission
regressions in this same cheap source gate, without calling GitHub APIs.

Every pre-existing non-release job now requires both:

1. Manual workflow dispatch **or** `FLOW_SUITE_HOSTED_CI == enabled`.
2. A nonscheduled event **or** `FLOW_SUITE_SCHEDULED_CI == enabled`.

The exact original job condition remains an additional conjunct. A schedule
therefore needs both opt-ins. Original job/check names, needs, permissions and
event triggers remain intact. Existing concurrency is retained; missing heavy
workflow concurrency queues runs by workflow/ref without canceling in-flight
stateful work. Missing non-release timeouts become 30 minutes; all are at most
70. These are job limits, not an aggregate billing, memory or matrix-fanout quota.

The whole heavy `CI` workflow is deferred by default. It contains no new cheap
success job that could let a release consumer mistake source inspection for
product qualification. Aggregate workflow success is insufficient: a skipped
job may still produce a successful-looking run. Release admission must require
the named prerequisite jobs to have actually completed successfully at the exact
source commit, refusing skipped, unavailable or missing job evidence.

The budget-gating rewrite excludes `release.yml` and `release-candidate.yml`
only when their explicit triggers contain `workflow_dispatch` alone; automatic
or unsupported trigger shapes refuse the source plan rather than gain an exemption.
Manual triggers, permissions and publication authority remain preserved. A
separate read-only prerequisite-admission guard may be added by checkpoint 12
to enforce the actual-job rule; byte-identical release files are not the final
claim. Enabling a budget flag cannot grant a release, signing, provider or
content approval.

The explicit JSON policy and limited stdlib job-metadata reader enforce budget
prefixes, literal timeouts and a single unfiltered cheap gate. Actionlint remains
the YAML/expression syntax owner. JSON plans are capped at 32 KiB and declare
`qualification_deferred` or `unobserved`; product/domain/execution authority stays
false. Original job conditions are not evaluated by this planner. Manual plan
eligibility does not dispatch any other workflow. Reports refuse unknown fields,
weakened gates, missing/over-budget timeouts and forged product/execution success.

Existing artifact retention above 14 days is exposed for review, including
long-lived benchmark baselines. Upload steps with missing/zero retention expose
an unknown repository-default duration; dynamic or unsupported retention values
also remain visible with unknown days. The bounded closed report never treats
either observation as a known short retention. This source repair does not silently erase their
retention requirements or alter manual release retention. The new cheap gate
adds no uploaded artifact. Operational budgets, source checks and skipped runs
are not a green supported release matrix.

Optiflow's hosted docs path filters additionally cover `examples/**` and
`tests/test_site_examples.py`; the enabled docs job runs the focused example
composition regression before full static-site composition. Existing deployment
conditions and the site link verifier are retained. Identity adoption and source
are untouched; only the pre-existing Identity workflow's execution budget changes.

Focused synthetic Python budget checks may be run in this sprint and reported
separately. No hosted workflow, Rust/native/model/hardware qualification, release
or publication is dispatched by this source change. Actual exact-commit required
job evidence remains unobserved until separately authorized execution. #88's
original final-pass ledger and known failure limits remain unchanged and last.

## Integrated local observations

The final integrated source passes 12 synthetic budget regressions, the
pinned Actionlint workflow wrapper and all 31 synthetic release-contract cases.
The admission suite has 12 passing applicable cases and two intentional
Renderflow-only skips. The default pull-request plan inspects five jobs, keeps
heavy qualification deferred and reports no artifact-retention review items.
The native candidate workflow bytes, manual publication triggers and authority
are preserved. Candidate consumption adds the separate read-only actual-job
precondition described in 12-release-workload-evidence.md. No real candidate,
native job, artifact verification, release or CI execution is qualified here.
