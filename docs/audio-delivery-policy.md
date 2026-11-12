# Explicit audio delivery goals

`delivery_audio::policy::Policy` describes an operator-selected loudness goal
and true-peak ceiling. There is no default policy and no claim that one number
is an industry-wide mastering or upload requirement. Choose a policy for the
approved musical intent and specific delivery route. Platform playback
normalization, an upload requirement and a creative master are different facts.

```json
{
  "schema": "aniflow.audio-delivery-policy/v1",
  "id": "operator-preview",
  "revision": "1",
  "purpose": "Example operator choice; verify suitability before actual delivery",
  "target_milli_lufs": -14000,
  "tolerance_milli_lu": 500,
  "maximum_true_peak_milli_dbtp": -1000
}
```

These example values are not recommendations or verified platform rules.
`target_milli_lufs` is thousandths of integrated LUFS, `tolerance_milli_lu`
is a symmetric tolerance in thousandths of a loudness unit, and
`maximum_true_peak_milli_dbtp` is thousandths of dBTP. This example therefore
describes -14.5 through -13.5 LUFS, with true peak at or below -1 dBTP.

Goals range from -70 to 0 LUFS, tolerance from 0 to 3 LU with the entire range
inside those bounds, and peak ceilings from -20 to 0 dBTP. Unknown fields,
missing goals, unbounded text and incompatible versions refuse. These are
supported configuration bounds, not artistic or standards recommendations.

The canonical settings digest includes the purpose, ID and revision as well
as numeric goals. Changing a policy changes its identity. Parsing/identifying
a policy performs no I/O, metering, gain adjustment, limiter, upload, source
rewrite or musical review. Final-file observations and human review remain
separate. The existing plain QC/preflight contracts keep their meanings.

## Compare current final-file QC

`delivery_audio::policy_check::compare` is a pure numeric comparison, with
explicit units and the full original mono/stereo channel scope. Its result
does not establish a current media binding. `assess` instead requires a current
`accepted_video_with_qc` or `accepted_audio_with_qc` input and reuses the existing
exact-source/acceptance/QC readback. An earlier master meter is refused. The
assessment contains private source/QC references; do not publish it wholesale.

```bash
aniflow --output json delivery audio-policy describe --policy selected-goals.json
aniflow --output json delivery audio-policy check --request final-file-goals.json
```

`final-file-goals.json` uses `aniflow.audio-policy-check-request/v1`, with
`input` in the same exact-final-QC shape as destination preflight and `policy`
containing the selected object above. Each quantity reports `matches`,
`outside_target` or `missing_evidence`. Both quantities must match for
`matches_selected_policy`. Silent/gated/unsupported observations keep their
typed reason and never become zero. Wrong units, a partial/stem channel scope
and nonfinite values refuse. Bounds are inclusive and no rounding is used to
turn an outside value into a match.

`policy_check::recheck` regenerates the assessment against current owner state.
Changed final bytes (including a mux or metadata edit), stale QC, changed policy
or modified report contents cannot inherit a saved passing flag. The digest
binds exact private evidence; it is not a signature or creative approval.

Checkpoint two adds three focused library cases and one CLI parser case,
authored and uncompiled/unrun. No meter is executed by comparison; its original
qualification remains false. This adds an explicit policy-check surface while
legacy destination preflight retains its existing evidence-availability meaning.

This is checkpoint one of the authorized audio-readiness batch under #212,
retaining predecessor #83. Source contracts and two useful cases are authored,
uncompiled and unrun. No tests/build/compiler/lint/format/schema/docs/native/
CLI/SDK/media/provider/CI or publication ran. Flow #88 stays last and unchanged;
the known alignment cancellation/start-marker failure remains unresolved with
unproven cause. ADR-0024 is proposed only.
