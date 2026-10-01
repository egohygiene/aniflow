# Integrated synthetic audio workflow

[#51](https://github.com/egohygiene/aniflow/issues/51) reconciles the delivered
audio checkpoints in [PR #63](https://github.com/egohygiene/aniflow/pull/63).
The [support matrix](audio-support-matrix.md) maps every original #13 acceptance
item to its implementation, exact profile and explicit disposition. The
[consumer guide](audio-consumers.md) explains final output selection and the
remaining renderflow/flow integrations.

This workflow exercises the canonical commands against synthetic media and
provider fixtures. It is a conformance tool, not a new product pipeline, provider
resolver or replacement for the public library. It imports no sibling source
and installs no analyzer or model.

## Generated profiles and lineage

The fixture creates an eight-second mono PCM16 mix at 48,000 Hz, then executes
a registered synthetic separation provider through Pipeline v3. That provider
generates three independent PCM outputs with explicit roles:

| Selected stem | Sample clock | Executing families |
| --- | --- | --- |
| `speech_profile` | 16,000 Hz | Technical inspection, signal, transcription and reviewed-lyrics alignment |
| `midi_profile` | 22,050 Hz | Technical inspection and MIDI candidates |
| `musical_profile` | 44,100 Hz | Technical inspection and musical estimates |

All sources are generated fixtures. The separator does not run Demucs, resample
the mix or prove that the output waveforms reconstruct it. The accepted lineage
records the original mix, separation plan/run/stage, selected output identity
and exact zero-origin duration comparison. It does not prove onset, phase or
musical correspondence. Each analyzer retains the selected stem's own sample
clock, frame count, scope and digest.

FFmpeg and ffprobe inspect the generated media locally. Transcription,
PocketSphinx alignment, the musical analyzer and Basic Pitch inference are
explicit synthetic executable/protocol fixtures with synthetic resource and
license declarations. Their outputs exercise the pinned public boundaries;
they establish no model accuracy, human review or publisher authenticity.

## Linked evidence checks

The fixture uses the exported `outputs` table from each public run outcome.
It does not discover final analysis by guessing an intermediate filename.
The retained evidence includes plans, run/status documents, accepted provider
locks and checkpoints, family reports, final normalized analysis and explicit
text/MIDI export packages.

The checks join these artifacts through their declared identities:

- Source/stem/mix digests, sizes, stream index, own-clock duration and lineage.
- Ordered plans and stage execution/reuse, provider lock and effective settings.
- Technical decode evidence and each family report's upstream references.
- Requested capability status, diagnostics, observation provenance, ordered
  timing, source bounds and available excerpt references.
- Observed transcript identity versus caller-supplied reviewed lyrics, with
  candidate alignment timing kept separate from reviewed text authority.
- Explicit timed-text conversion losses and retained normalized companions.
- MIDI candidate uncertainty, native-time/tick derivation, note pairing,
  placeholder mappings, independent read-back and immutable export completion.

Signal measurement precedes stem attachment in its current ordered pipeline.
Its original companion therefore retains an unlabelled physical source while
the final analysis and lineage add the stem role. The fixture checks exact byte
and clock equality across that overlay. Other analyzer families attach lineage
before inference and retain the stem directly in their family companions.
The consumer guide specifies this distinction.

## Refusal, partial evidence and recovery

The integrated cases distinguish runtime state from capability evidence:

| Case | Required evidence |
| --- | --- |
| Missing provider/dependency | Structured preflight refusal; no invented successful analysis or run |
| Unsupported musical input rate | Accepted technical/lineage evidence with explicit unavailable musical result; no resampling |
| Partial lyrics alignment | Untimed reviewed words/cues retained; no fabricated matching or timing |
| Empty transcription | Explicit empty observation; no placeholder transcript or review authority |
| Provider exits unsuccessfully | Failed run retains accepted earlier checkpoints; no accepted final transcript |
| Provider interrupted after its start marker | Cancelled run retains accepted earlier checkpoints; bounded process termination |
| Compatible recovery | Earlier stages reused; incomplete inference stage executes before completion |
| Exact repeated resume | Output identities unchanged and inference not relaunched |
| Model/review input changed | Resume refuses before inference; prior accepted evidence remains intact |

The provider-start marker and bounded deadline determine when to interrupt;
the test does not assume a fixed startup delay. Sources, reviewed inputs and
accepted separation evidence remain unchanged. A valid run producing partial
or unavailable optional analysis is not promoted to full capability support.

## Run and inspect

Build the native CLI, provide already installed FFmpeg/ffprobe and use the
canonical Task with explicit binary and receipt paths:

```bash
cargo build --locked
task audio:workflow:corpus \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  RECEIPT="/absolute/evidence/audio-workflow-smoke.json"
```

The script also supports direct execution:

```bash
python3 scripts/smoke-audio-workflow.py \
  --aniflow "/absolute/aniflow/target/debug/aniflow" \
  --receipt "/absolute/evidence/audio-workflow-smoke.json"
```

With a receipt path, the generated workspace remains beside the receipt so its
recorded original run/output locators can be inspected. Without a receipt,
the fixture uses a temporary workspace and cleans it up. These workspaces are
test artifacts; they grant no authority to access or mutate real media.

The independent checker uses an already installed Draft 2020-12 validator and
Mido parser to verify captured bytes, schema identities and MIDI events:

```bash
python3 scripts/check-audio-workflow.py \
  --receipt "/absolute/evidence/audio-workflow-smoke.json" \
  --output "/absolute/evidence/audio-workflow-validation.json"
```

The matching Task is `audio:workflow:check` with `RECEIPT` and `OUTPUT`.

The full repository smoke includes this integrated workflow. Set
`ANIFLOW_WORKFLOW_SMOKE_RECEIPT` to retain its receipt outside the temporary
suite workspace. The other feature smokes remain independent regressions.

## Acceptance evidence and remaining gates

The [machine-readable support matrix](validation/aniflow-51-support-matrix.json)
and [fixture index](validation/aniflow-51-fixture-index.json) retain the original
acceptance IDs and stable case names for
[#24](https://github.com/egohygiene/aniflow/issues/24). The
[checkpoint](validation/aniflow-51-checkpoint.md) records pushed savepoints and
the [final local receipt](validation/aniflow-51-local.json) records exact
implementation, environment and passing local checks. The final integrated run
completed 28 cases across 11 runs, retaining 423 documents,
44 outputs and 20 declared upstream links. Independent conformance checked
354 schema-covered documents, 64 internal integrity records by digest/semantics
and 5 private documents by digest, plus 260 nested public schema instances.
External Mido read-back matched the 2-note candidate export.

The [historical draft progress receipt](validation/aniflow-51-progress.json)
retains passing checks and the first full-smoke musical-stage failure. The
final local receipt supersedes its pending blocker after a complete qualification
run with target, generated runs and receipt workspaces outside synchronized
scratch. The external-writer explanation is a strong inference from the active
sync process and rsync temporary-name match; no syscall trace attributed the
writer. PR #63 remains draft at the user's request.

Parent #13 closes only after the reconciled closeout is reviewed and merged.
This feature evidence does not close generalized stream timing #32, layered
validation #33, cross-run reuse #34, broad corpus #24 or release #10. The next
order remains #32 → #33 → #34 → bounded #24 → #10 → flow #51.

Actual model inference/accuracy, singing quality, native macOS/other-platform
behavior, full dependency closure/OS isolation, hosted CI and release
qualification require separate evidence. The existing optional real musical
analyzer remains a separately configured feature corpus; this integrated
fixture does not execute it.
