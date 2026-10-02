# test: add the bounded adversarial temporal-media corpus (#24)

Aniflow's existing fixture coverage was spread across feature-specific tests,
protocol examples and historical receipts. There was no unified source-bound
catalog, exact generated-byte inventory or bounded qualification entry point
for reviewing corpus drift and remaining coverage.

This change adds 86 deterministic synthetic cases across images, frame sets,
PCM audio, raw video, timed text and temporal probe documents, plus five native
encoding profiles. Cases carry stable IDs, seed/parameters, provenance,
intentional mutation descriptions, expected typed outcomes and SHA-256
inventories. Native encoded digests remain tool-bound observations, recorded
only when the explicit qualification runner executes.

Public Rust consumers assert temporal/text behavior, normalized transport,
source immutability and refusal. The native path checks decoded pixels/samples,
geometry/cardinality and same-toolchain encoding repeatability. Existing suites
remain authoritative through 182 named Rust test locators and the 148 historical
audio IDs. These are overlapping evidence classes, not one new fixture count.

Generation targets an absent directory outside the checkout. Read-only drift
and inventory checks never repair canonical data. PR, scheduled and extended
tiers bound execution/log/storage budgets and preserve non-passing failures.
The existing CI gains a drift gate; a separate pinned workflow supports manual
qualification and opt-in weekly execution. Scheduling was not enabled.

The guide includes the coverage matrix, limits and fuzz-regression promotion
flow. This completes bounded implementation checkpoints #75–#77. Keep #24 open
as the broader corpus parent with its original acceptance criteria and named
expansion areas, including image-format/orientation and damaged-container
breadth. #69 remains the separate multi-artifact-port feature.

## Qualification

**Unrun under #64 by maintainer instruction.** Only the catalog authoring
command ran. Twenty new test functions (four Rust, 16 Python), compilation,
format/lint, schemas, drift checks, native media generation/decode, smoke,
package, stable/MSRV and hosted CI are not claimed passed. No real media or
models were used. Preserve the known alignment cancellation failure.

Runner limits are monitoring safeguards, not quotas or a sandbox. Qualification
requires Unix process-group support; native profiles require explicitly selected
local FFmpeg/FFprobe with libx264. Dependency installation is separate from the
offline runner. Historical receipts retain their original evidence scope.

Receipt: `docs/validation/aniflow-24-implementation.json`.
Handoff: `docs/validation/aniflow-24-checkpoint.md`.
Guide: `docs/adversarial-corpus.md`.

Closes #75.
Closes #76.
Closes #77.

Related: #24, #64, #69, egohygiene/flow#11. Draft review only; no merge.
