# #114 — Serial RIFE interpolation checkpoint

Parent #110; immutable starting main:
`08cb2a83dbe288f6d893a278cbeb34905cc19a49`.
Branch: `feat/aniflow-110-rife`. [Draft PR #115](https://github.com/egohygiene/aniflow/pull/115)
is open. Code publication: `27458ad121a468169a81c4447c1c273ec50b665f`; tree
`70f2dbf7ec37bc740b1661441c87f63fe9513e0c` exactly matches the local implementation
tree. Later metadata-only bookkeeping may advance the live PR head. Cargo
remains `0.3.0`.

## Delivered authored behavior

The new public `video_interpolation` module provides closed caller/provider
configuration, indexed source frames, read-only clock/resource planning, exact
output-frame mapping and imported output-evidence validation. The CLI exposes
`video interpolation-plan` with human and machine presentation. The existing
raw provider ABI dispatches the RIFE capability through Pipeline v3 authority.

The adapter snapshots exact pinned executable and `rife-v4.6` model files,
validates ordered RGB8 PNG inventory/digests/CRC/geometry, reserves a declared
bounded storage estimate and processes one pair at a time with `1:1:1` workers.
Copied originals and cut/endpoint holds preserve their exact digests; each
midpoint has explicit source ordinals. Exactly 2N frames at the doubled rational
rate retain source duration. Output evidence is recomputed against the original
configuration/index before publication.

Disabled/CPU defaults, conservative Apple Silicon automatic policy and explicit
larger presets respond to the operator's reported memory-heavy laptop crash.
The cause of that prior crash was not observed. Padded-pixel/working-set
admission is an uncalibrated heuristic over operator memory declarations,
never a RAM/VRAM reservation or enforced quota. This cannot establish that a
particular laptop, backend or model invocation will be safe.

## Source review

Independent source review identified and repaired provider-values conversion
to `BTreeMap`, shadowed fixture helper calls, effective-schema/default
normalization, missing imported-output semantic validation and repeated
full cut scans in per-frame loops. Internal mapping iteration now validates
the cut list once and uses logarithmic cut lookups.

Sources, documentation, schema fields and public command spellings were read
and compared. This is limited source evidence, not compiler, formatter,
JSON-schema or runtime qualification.

## Authored coverage

- Nine Rust integration cases for fractional clocks, cuts, one-frame endpoint,
  Mac 4K refusal, preset/headroom policy, disabled behavior, stale/profile
  refusals, exact identities, imported evidence and read-only machine CLI.
- Twelve provider cases for exact argv, PNG structure/profile/CRC, path/special
  nodes, staging, complete synthetic output, cut bypass, changed pins, missing
  output, timeout, host/storage admission and unexpected/capture-limited output.
- Five Python schema cases plus existing public command-spelling coverage.

All cases are authored **and unrun**. No compiler/build, lint/format, schema,
smoke/package, native/model or hosted qualification was executed. Maintainer
deferral remains #64; commits retain `[skip ci]`.

## Limits and next checkpoint

This provider produces indexed frames, not a final MP4. It does not independently
inspect the source video, decode PNG image data for fidelity, discover cuts,
probe current RAM/GPU state, resize inputs, mux audio or qualify color treatment.
The source frame/color profile and host observations remain attributed imports.
Actual disk free space and live child file growth are not monitored as hard
quotas. Tool snapshots do not authenticate transitive backend libraries.

The existing provider runtime owns overall timeout/cancellation and accepted
stage checkpointing; finer per-pair durable recovery remains parent work.
Parent #110 stays open for derived-clock-aware final assembly, source/import
ergonomics, broader rates, fresh host observations and native resource evidence.
The next bounded slice should connect accepted RIFE output evidence to final
video assembly without reusing unchanged-cardinality timing acceptance.

No source media, model weights, installed tool, key, publication or release was
created or processed. Parallel draft PR #105 remains untouched. Rollback is
reverting this feature's commits; existing pipeline schemas/checkpoints and
unselected workflows retain their behavior.
