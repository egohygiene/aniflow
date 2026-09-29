# aniflow #49 checkpoint

Status: implemented in draft PR #61; final local validation and evidence pending.

Issue: <https://github.com/egohygiene/aniflow/issues/49>
Parent: <https://github.com/egohygiene/aniflow/issues/13> (remains open).
PR: <https://github.com/egohygiene/aniflow/pull/61>

## Starting evidence

Fresh main: `01130266cd2d6f52fca72e357e01a5a854ad540b`, merged PR #60.
Live prerequisites #42, #43 and #47 are completed; #48 is also merged but is not
a requirement for aligning supplied lyrics. No competing aniflow PR was open.
flow #11, its holistic graph, all four suite issue/PR/release collections,
repository guidance and the existing provider/timed-text contracts were read.
Older roadmap execution snapshots remain historical.

## Bounded implementation plan

- Accept an explicitly reviewed timed-text JSON document. Retain its original
  bytes/digest, cue text, supplied review provenance and revision identity.
  A successful provider run does not verify the review assertion.
- Use one optional pinned local PocketSphinx 5.1.1 English CPU forced-alignment
  profile, upstream revision `511126b492dcb267cf30d49d631946d7b61a9530`.
  Exact executable and explicitly supplied acoustic/dictionary resource bytes
  are caller pins. No automatic discovery, installation or model download.
- Admit mono 16 kHz PCM16 WAV through existing technical inspection and optional
  stem lineage. Keep candidate timing, uncertainty and unsupported profiles
  explicit; alignment does not prove that authored words occur in the audio.
- Preserve authored words and punctuation while recording a bounded explicit
  tokenization mapping. Reject malformed, contradictory and out-of-range native
  evidence. Missing/ambiguous matches never become a complete alignment.
- Reuse Pipeline v3 planning/run/resume, bounded direct invocation, cancellation,
  dependency rechecks and the shared loss-aware timed-text export boundary.
- Ship public Rust/API/CLI/task seams, strict schemas, synthetic fixtures,
  focused refusal/integration/smoke checks and reproducible local evidence.

## Recovery and review gates

Push implementation and validation checkpoints to this same draft PR.
Run meaningful local checks without hosted-CI polling. Keep actual forced
alignment execution/quality, native macOS, wider profiles and release
qualification separate from synthetic contract evidence.

Synthetic fixtures only. No real-media access or mutation, model downloads,
paid APIs, merges, tags or releases. The maintainer owns review and merge.
#50 is a separate ready issue; #51 reconciles #13 after #49 and #50 land.

## Implementation savepoint

The library, strict schemas, CLI/tasks, source-bound export and synthetic
fixtures are implemented. Candidate timing is explicitly unreviewed; the
original reviewed document remains the semantic authority. Missing words and
ambiguous repeated occurrences retain untimed cues and partial analysis.

Focused local checks passed: initial 15 alignment unit tests, eight integration
tests, two CLI tests, twelve independent schema tests, 70 contract documents,
eleven synthetic CLI smoke scenarios and five actual Task literal-argument
checks. New published-report tests and stronger integration assertions will be
covered by the final full suite. The smoke captured twelve documents.

The initial integration run found an unsorted component inventory; fixing its
required canonical order made the focused run pass. Native-source review also
found a compiled default-LM probe; the fixed invocation now passes a nonexistent
private -lm sentinel, which align clears before decoder initialization.

Remaining: final formatting/naming/Clippy, all Rust targets, focused MSRV,
serial repository smoke, package/docs checks, captured-document validation,
final receipt and handoff. Actual native inference/accuracy remains unverified.
