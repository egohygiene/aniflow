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

This is checkpoint one of the authorized audio-readiness batch under #212,
retaining predecessor #83. Source contracts and two useful cases are authored,
uncompiled and unrun. No tests/build/compiler/lint/format/schema/docs/native/
CLI/SDK/media/provider/CI or publication ran. Flow #88 stays last and unchanged;
the known alignment cancellation/start-marker failure remains unresolved with
unproven cause. ADR-0024 is proposed only.
