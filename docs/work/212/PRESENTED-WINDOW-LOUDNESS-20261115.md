# Explicit presented-window loudness

The complete AAC decoder signal and the presented audio interval remain different subjects. This checkpoint adds an opt-in analysis view of the exactly proved presented sample interval. It does not edit delivery media, discard the retained decoder tail, change gain, resample, or infer that the excluded samples are silent.

Add this field to an existing final-audio-QC request:

```json
"presented_window": "exact_presented_audio"
```

The same selection is available on `VideoAudioSignalRequest`, a source recipe's `recipe.audio_signal` option, and a publishing request's `audio_qc` option. Omitting it preserves the complete-decoded default, old canonical request/receipt shapes and the existing guard against treating a padded-tail meter as presented loudness. The selected mode supports only the exact positive, zero-origin presented sample interval of an already admitted 48 kHz mono/stereo source. It does not expose arbitrary trim points.

Existing CLI owners remain authoritative:

```sh
aniflow --output json delivery audio-qc plan --request request.json
aniflow --output json delivery audio-qc run --request request.json --output-directory /private/new-qc
aniflow --output json delivery audio-qc status --directory /private/new-qc --receipt-sha256 EXTERNAL_QC_DIGEST
```

For resumable QC, retain the existing wrapper `{ "schema": "aniflow.resumable-final-video-audio-qc-request/v1", "qc": REQUEST }` and use `delivery audio-qc resumable run --request FILE --run-directory NEW`, followed by `status` or `resume` with that same run directory. A completed resume preserves the exact report; it does not replace the request.

The full WAV, accepted native-v3 child and original full meter stay intact. The explicit selection additionally writes `presented-window/selected-audio.wav` within the private signal workspace. Its canonical float32 header and every selected sample byte are checked against the retained full WAV **before and after** the separate existing meter runs, then rechecked on current readback. The receipt binds the source file, temporal digest, full PCM, exact sample interval, view WAV/PCM and separate meter. Source/final/resumable owners share this proof. Request and receipt selections must agree; legacy `None` cannot adopt an injected view.

The view is analysis evidence, not a new accepted media export. Both meters retain `runtime_qualified: false`. Policy and saved-policy validation select the separately bound presented meter only when its request, scope and current evidence agree. Numeric comparison can match a supplied goal or report it outside target; it grants no platform compliance or musical approval. A destination requiring loudness can now use the proved presented subject. Missing/unavailable evidence remains missing and is never zero. Existing padded-tail gain-video sample-count restrictions remain unchanged; this checkpoint performs no normalization or gain qualification.

Storage projections add a checked finite view envelope only for the opt-in request. The resumable owner admits its private view directory only alongside the committed meter marker. A partial view without that marker is refused and retained for reconciliation; it is not silently adopted.

## Observed evidence

The [portable receipt](evidence/20261115/presented-window.json) records the real draft-source head `e1ce9b37a4b820dc7e469b5a0176229058bf741d`, identical-tree local compile lineage, executable/contract/tool pins, raw observation hashes and exact failure classifications. The optimized dev Rust 1.85 build took 205.768 seconds. Repeating the same Cargo command on the actual published head in the same checkout took 0.315 seconds, with both artifacts marked fresh and identical bytes. This is explicit cache reuse, with zero independent rebuilds claimed.

The selected ordinary source has 96000 presented frames and 96256 complete decoded frames, including 256 tail frames containing 512 nonzero channel values. The full PCM remains `b70f04f7cd3c64fbb6c80784e69f5d10ef29c5df3643695dfe04be47d0c2a4ee`; its exact presented prefix is `1fc7c404a80fc34828d4f33515b3b9f28c5f11164eb50157316f0937c2565e9f`. The separate WAV is 768056 bytes. Source, direct final QC and resumable QC prove those same two PCM subjects. The full meter is exactly equal to the retained previous observation; the AAC packet/configuration/timing identities and original video/accepted-output bytes remain unchanged.

The main pilot passes 14 actual CLI calls in 21.169 seconds, including historical omitted-None byte preservation, completed resume, stale external-pin refusal, a supplied numeric goal that matches and one outside target, and a synthetic destination comparison. Eleven fault controls pass within 34 additional CLI calls in 14.584 seconds. Each uses a fresh native scratch subject that passes current status before mutation, and all subjects pass current status after exact restoration. The controls reject wrong scope/source/interval, missing or injected receipt selections, sample changes, coherently repinned wrong samples, changed complete-tail bytes, a missing WAV and a partial view without its meter marker. The external-digest-only check remains a separate case. Main accepted captures are untouched.

Four focused Rust source-helper tests, eleven saved-file observer controls and twenty-four schema controls pass. The full Rust test suite, Clippy, supported-host/native meter qualification, legacy alignment and descendant cancellation remain outside this checkpoint. Human words/timing/appearance/creative review, signing and publication remain unperformed.

To reobserve saved files without any media process:

```sh
task publishing:presented-window:observe VIDEO=/absolute/final.mp4 COMPLETE_WAV=/private/signal/selected-audio.wav WINDOW_WAV=/private/signal/presented-window/selected-audio.wav SIGNAL=/private/signal/video-audio-signal.json QC=/private/qc.json
task publishing:presented-window:test CAPTURE=/absolute/saved-pilot-directory
```

That observer proves saved canonical bytes, separate meter subject pins and optional QC request/subject joins. It does not itself reobserve AAC packets, authenticate receipt pins, run the meter or replace native current-owner acceptance. The captured native owner calls provide those separate readback facts. All requirements/goals in this synthetic demonstration are operator-supplied fixture choices, not researched platform standards.
