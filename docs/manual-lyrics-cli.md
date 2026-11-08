# Manual timed lyrics from the CLI

`video lyrics` delivers the existing [manual workflow](validation/aniflow-108-manual-lyrics.md)
and [recipe attachment](recipe-lyrics-delivery.md). It imports explicitly authored
intervals, retains separate words/timing decisions and exports selected codecs.
No recognition, automatic timing, model discovery, muxing or burn-in is invoked.
All request/workflow/attachment JSON is private evidence with local bindings;
keep it out of public bundles. Inputs remain available for later rechecks.

For a recipe with completed optional original-audio observation, prepare a
request using explicit `RecipeLyricsOptions` JSON:

```json
{
  "input_format": "srt",
  "language": "en",
  "overlap_policy": "reject",
  "targets": ["srt", "webvtt", "json"],
  "conversion": { "allow_losses": [] }
}
```

Check the conversion-options wire shape in [the request schema](contracts/manual-lyrics-request-v1.schema.json)
when selecting permitted losses. The CLI reads each request/options/declaration
as closed JSON of at most 64 KiB. Path flags resolve against the current working
directory; embedded audio paths retain their exact supplied binding and must
meet the SDK's canonical-path rules.

```bash
aniflow video lyrics prepare-request \
  --recipe-run-directory "/absolute/accepted recipe" --options options.json
```

Human output is the request JSON, which the operator can save privately as
`request.json`. With global `--output json`, save the envelope's `result` object
as the request, rather than the entire envelope. Preparation writes no request
file or review. A standalone caller may instead supply its own exact
`aniflow.manual-lyrics-request/v1` with original PCM/clock/extent bindings.

```bash
aniflow --output json video lyrics initialize \
  --request request.json --work-directory "/absolute/new manual work"
aniflow --output json video lyrics import \
  --request request.json --work-directory "/absolute/new manual work" \
  --input "/absolute/artist lyrics.srt"
aniflow --output json video lyrics status \
  --request request.json --work-directory "/absolute/new manual work"
```

Initialize can also accept `--input`. Without it, author the SRT externally and
import later. SRT, supported WebVTT and TTML require positive explicit intervals;
no cue end, offset, trim or padding is inferred. Export target selection and
permitted losses come only from the saved request. Edits require a fresh workflow.

Status returns distinct `words` and `timing` fingerprints. After reviewing each
scope yourself, provide its exact `aniflow.manual-review/v1` JSON containing
`schema`, `context_sha256`, `subject_sha256`, `decision` (`approve` or `reject`),
`reviewer` and `notes`. Use that scope's current hashes. No field is filled in by
the CLI and no default approval exists. Each declaration is retained once:

```bash
aniflow --output json video lyrics review \
  --request request.json --work-directory "/absolute/new manual work" \
  --scope words --declaration operator-words-decision.json
aniflow --output json video lyrics review \
  --request request.json --work-directory "/absolute/new manual work" \
  --scope timing --declaration operator-timing-decision.json
aniflow --output json video lyrics export \
  --request request.json --work-directory "/absolute/new manual work" \
  --output-directory "/absolute/new sidecars"
```

Both exact scopes must be approved before export. Rejection is preserved;
repeating a decision, crossing scopes or changing original audio/request/manual
bytes is refused. For an accepted final recipe assembly, use `video lyrics
attach` with the same `--request`, `--work-directory` and explicit
`--recipe-run-directory`. Attachment exports directly into the recipe's fixed
private sidecar slot; a prior standalone export is optional. It neither replaces
video nor approves subtitle appearance, publication or creative correctness.
Renderflow can consume the exported SRT separately.

Success/failure uses the existing v1 machine envelope with distinct
`video_lyrics_prepare_request`, `video_lyrics_initialize`, `video_lyrics_import`,
`video_lyrics_status`, `video_lyrics_review`, `video_lyrics_export` and
`video_lyrics_attach` names. Initialize/import use the ordinary CLI cancellation
token. Interrupted work remains for explicit reconciliation; no loose partial
files become accepted. Read status before retrying an operation that may already
have committed a decision or receipt.

Two parser cases and two scoped-review/stale-binding regressions are authored,
uncompiled/unrun. All tests/builds/format/lint/schema/docs/native/provider/media/
CLI/SDK/CI execution remains deferred under #64. Its known alignment cancellation/
start-marker failure remains unresolved, cause unproven. #108 stays open for its
remaining preview/render/mux/burn-in and qualification work. No actual review,
private-media trial, release or publication occurred.
