# aniflow #48 recovery checkpoint

Status: scope checkpoint; implementation and validation are pending.

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
