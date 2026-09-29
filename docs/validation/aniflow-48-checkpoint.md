# aniflow #48 recovery checkpoint

Status: implementation checkpoint; broader validation and final review are pending.

Draft PR: <https://github.com/egohygiene/aniflow/pull/60>

The library, Pipeline v3 adapter, CLI/tasks, four schemas, examples, synthetic
fixtures, export bridge and documentation are implemented. The admitted profile
is whisper.cpp 1.8.7 with explicitly pinned local tiny.en bytes, English, CPU,
mono 16 kHz PCM16, exact segment timing and unavailable word timing/confidence.
Private tool/model/source staging and clean child environment reduce ambient
discovery; compiled backend paths and linked libraries remain trusted installation
dependencies rather than a verified sandbox.

Initial all-target compilation passed. The first focused pass passed ten unit
tests and six of seven integration tests. One initial tool launch was refused
without publishing a transcript; that test passed when isolated. Its underlying
OS cause was not established, so launch diagnostics now include error kind/code
and the final full suite must be checked for recurrence. Additional authority and
identity tests have since been added. Ten independent schema checks, all 66
published contract documents, product naming and naming fixtures pass.

Resume from the final source tree in this draft: run the full local Rust/Clippy,
formatting, synthetic smoke, schema/docs/package gates, focused MSRV checks and
independent review. Record exact final evidence in `aniflow-48-local.json`, update
the handoffs and request maintainer review. No native transcription or model
accuracy has been tested. Do not treat this checkpoint as ready for merge yet.

Issue: <https://github.com/egohygiene/aniflow/issues/48>

Parent mini roadmap: <https://github.com/egohygiene/aniflow/issues/13>

Base main: `0fa06979fe717505f0dc922bf34bc71125b285b4` (merged PR #59).

## Scope

Add one optional, replaceable offline transcription provider through the existing
Pipeline v3 lifecycle. Evaluate the pinned whisper.cpp CLI using explicitly
configured local executable and model identities. No model download belongs in
preflight or execution. Record tool/model/configuration identities and licensing
sources without implying independent authenticity or legal qualification.

Normalize observed transcript segments, language and available timing evidence.
Word timing and confidence must be explicitly unavailable when the selected
upstream output does not provide them. Never promote observed transcription to
reviewed lyrics. Reuse timed-text conversion for exports and preserve supplied
source/stem scope.

Keep execution bounded, cancellable and source-preserving. Refuse missing or
changed dependencies, malformed/oversized output and out-of-range timestamps.
Reobserve mutable dependencies before run-local checkpoint reuse. Expose public
library, canonical CLI/tasks, strict schemas, synthetic fixtures and docs together.

## Recovery sequence

1. Confirm the pinned upstream output/argument contract and bounded support profile.
2. Implement contracts, adapter and Pipeline v3 integration with synthetic providers.
3. Push the implementation checkpoint; pause to assess scope and review.
4. Run meaningful local checks and independent review; publish an exact receipt,
   update roadmap handoffs and push the final review checkpoint.
5. Stop for the maintainer's review and merge. Do not close parent #13.

## Evidence boundaries

Synthetic fixtures only. No real-media access or mutation, implicit model
downloads, paid APIs, hosted CI polling, merge, tag or release. Real inference and
transcription quality may only be claimed if an already available model runs on
generated speech; otherwise record both as unverified. Native platforms and
release qualification remain separate gates.

The live suite refresh found #42/#43/#47 merged, #48 open, and #49/#50 independently
ready. Other release gates remain in their owning issues. Historical roadmap
snapshots do not override a fresh live query.
