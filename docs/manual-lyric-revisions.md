# Create a fresh revision of manually supplied lyrics

This #108 source checkpoint closes the edit/import loop without overwriting a
prior workflow or transferring approval. Manually author the new SRT (or the
same explicitly selected interval format), then obtain a current private cue
worksheet from the existing import:

```bash
aniflow --output json video lyrics worksheet \
  --request "/private/lyrics request.json" --work-directory "/private/old lyrics"
```

The worksheet contains private words and review state. Keep its exact
`worksheet_sha256` and create an `aniflow.manual-lyrics-revision-request/v1`
document with `lyrics_request` (the same complete existing manual request),
`previous_run_directory`, `expected_worksheet_sha256`, `new_input`,
`expected_input_sha256`, `operator` and `declared_origin`. The operator/origin
are bounded declarations, not authenticated identity or creation history.

```bash
aniflow --output json video lyrics revise-plan --request "/private/revision.json"
aniflow --output json video lyrics revise-create \
  --request "/private/revision.json" --output-directory "/private/new revision"
```

The old worksheet/review state and supplied bytes must match their pins. Empty,
unchanged, malformed or out-of-extent input refuses. Change counts compare
native cue IDs only; relabeling/reordering is not interpreted as an alignment.
Timing-only and wire-format-only edits also start both words and timing reviews
fresh. New source audio, clock/offset or configuration requires a separately
initialized request; this revision operation preserves the exact existing one.

A fresh private root retains `revision-intent.json`, `previous-worksheet.json`,
the exact revised wire bytes in `input.bin`, and a new ordinary workflow under
`lyrics/`. The completion receipt is written last. The ordinary manual owner
tracks this retained input snapshot, so later edits to the external authoring
file do not silently alter an earlier revision. Previous workflow files and
review declarations remain unchanged; edits made outside Aniflow may already
have invalidated an old source locator and are recorded honestly in its worksheet.

Keep the returned `receipt_sha256` separately and inspect current new-owner
states with:

```bash
aniflow --output json video lyrics revision-status \
  --directory "/private/new revision" --receipt-sha256 "<known exact receipt SHA-256>"
```

Use the existing `review` and `export` commands with the original manual request
and `--work-directory /private/new revision/lyrics`. Both exact new decisions
must be supplied. A prior approving declaration cannot authorize changed input.
Later explicit decisions update current status without rewriting the revision
receipt. Rendering/appearance and final-movie approval remain separate.

Readback can use the retained parent snapshot after the old workspace is gone;
it does not freshly reobserve that old workspace. Current bound source audio
and the retained input/new workflow must remain intact under existing owner
rules. Extra members, stale receipt/intent/content and partial roots refuse.
Failed/cancelled creation preserves partial evidence; choose a fresh root after
inspection. Revision requests are capped at 128 KiB, new raw input at the
existing 1 MiB limit, and revision metadata/parent snapshots at 2 MiB; oversized
snapshots refuse without truncating private lyrics.

Eight library/refusal/revision cases plus one parser case are authored,
uncompiled/unrun. No recognition/alignment, model, native media, real human
review, tests/builds/CLI/SDK/CI execution, source disposal or publication occurred.
ADR-0014 stays proposed; full #108 acceptance remains open. Flow #88 stays last
with its original ledger and known unresolved alignment failure unchanged.
