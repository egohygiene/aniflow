# Native audio implementation checkpoint — #80

Authored 2026-10-03T15:05:37Z; checkpoints #93–#95. Start at merged main `cb19cdba8b1f8be1dcc2192cab12947aba3995d7`.
Implementation checkpoint `bc16caed3153f10925079e3604225f40b1667374`, tree `5da1acf2efd6aee40813fff4bff7dc5668d81846`. The final draft
adds this handoff, receipt and root continuity; GitHub records its final head/tree.

## Delivered boundary

Technical inspection v2/provider2 independently parses classic RIFF32 PCM16,
packed PCM24 or finite IEEE float32. It keeps native data bytes, file and sample
SHA-256 identities, exact sample count/duration, interleaved channel order and
pinned tool/configuration evidence. FFprobe must agree with the parsed format;
decode uses the same native encoding and must produce the original payload hash.
Finite values above ±1, negative zero and subnormals are not clamped or quantized.
Nonfinite samples, unsupported layouts, malformed frames/fact metadata and
incompatible decoder observations fail before successful evidence publication.

Explicit signal settings v2 select native companion v3/provider3 through the
existing `--analysis signal --signal-configuration` CLI or
`SignalAnalysisRequest::new_native`. Direct source samples yield peak, RMS,
crest factor and exact silence/threshold frame regions. The default threshold
ratio 1.0 is nominal full scale: signed integer positive maxima are below it,
while the negative endpoint reaches it. Regions are threshold observations,
not proof of audible distortion. All native loudness/true-peak quantities are
explicitly unavailable in this profile. The final normalized analysis is partial
and retains the companion's reason, never a fabricated measurement or zero.

Legacy PCM16 signal settings/report v1/v2 preserve their meanings and algorithms;
technical v1 remains readable. New execution uses inspectionv2 identities, so old
plans/checkpoints cannot silently become compatible. Musical, transcription,
alignment, MIDI and stem selection remain their bounded PCM16 profiles. The
inspection extension does not widen model support. Independently selected
originals and finishing candidates each receive their own source-bound run;
finishing transformations and approval remain #79/#83.

## Qualification and source review

All tests, compiler/build, formatting/lint, schema/drift, native, smoke/package,
platform and hosted qualification are **unrun under #64**. No models, private
media, downloads, releases or publication to a media platform occurred.
Synthetic Rust and schema cases cover native precision, tails, stereo, float
headroom/nonfinite refusal, malformed padding/fact metadata, version selection,
source mutation and resume. Corpus locators/digests were authored directly;
no corpus generator or checker was run.

Read-only review resolved missing RIFF padding in report byte bounds and the
float `fact` ordering bypass. It also inspected CLI selection, snapshot/digest
binding and legacy consumer refusal before sample interpretation/inference.
These observations are not passing execution evidence.

The known alignment cancellation failure remains:
`cancellation_keeps_technical_checkpoint_and_resume_reexecutes_alignment`,
`tests/audio_alignment.rs:547`, `marker.exists()`, joining-thread failure at
line561. Its cause is unproven and it was not rerun or fixed here.

## Resume and next work

Review the draft before merge. #64 must later establish actual native host/tool
behavior and execute the authored checks before native support is advertised as
qualified. Native loudness/true-peak needs a separately versioned compatible
method extension if selected; current unavailability must remain explicit.
Next suggested implementation is #38, then #36/#37/#35, #83 and #82.
[Flow #78](https://github.com/egohygiene/flow/issues/78) captures the reusable AMV
preset without implementing it. #10 still needs Egolint #29, qualification and
actual immutable publication before Flow #51. No release version/tag was chosen.

See [receipt](aniflow-80-implementation.json), [inspection guide](../audio-inspection.md),
[native signal guide](../audio-signal-analysis.md), and [support matrix](../audio-support-matrix.md).
