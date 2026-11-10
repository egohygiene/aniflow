# Measure the exact final WAV or FLAC

`delivery_audio::final_lossless_qc` composes current accepted native PCM or a
current native lossless export with fresh complete final-file measurements.
It keeps the final container/file digest separate from original PCM identity.
It grants no musical review, platform acceptance or publication authority.

Supply `aniflow.final-lossless-audio-qc-request/v1` with an explicit `subject`:
`accepted_pcm` (`file`, `signal_run_directory`) or `accepted_audio_export`
(`run_directory`). Add exact pinned 8.0.1 `configuration` using the existing
native audio-inspection settings, `provider_executable`, `maximum_pcm_bytes`
(1024..268435456), `maximum_working_bytes` and `minimum_free_bytes`. Inputs retain
the existing ten-minute bounded native mono/stereo profile. No loose FLAC or
WAV file becomes accepted by supplying a receipt-shaped document.

```bash
aniflow --output json delivery lossless-qc plan --request final-audio-qc.json
aniflow --output json delivery lossless-qc run --request final-audio-qc.json --output-directory /private/final-audio-qc
aniflow --output json delivery lossless-qc status --directory /private/final-audio-qc --receipt-sha256 EXTERNALLY_RETAINED_QC_DIGEST
```

Plan reads current accepted source and explicit component identities only. Run
reserves a fresh private root, snapshots exact final bytes and components, then
copies WAV exactly or asks the existing lossless native adapter to fully decode
FLAC to its original PCM16/PCM24 bytes. The existing minimal WAV writer wraps
those bytes without codec-default extensible headers. The independent parser
requires exact original PCM hash, rate, channels, precision and complete frame
count. The ordinary native signal pipeline then owns sample/RMS/peak/silence/
clipping measurements and current v3 acceptance in `signal-runs/`. The report
points to that actual accepted child and its separately pinned signal summary.

For 48 kHz native mono/stereo, the existing two-pass ebur128 owner can consume
the exact integer or float WAV through its new `aniflow.native-pcm-loudness/v2`
view. It does not quantize to PCM16. Original meter v1 and native signal v3
contracts keep their meanings. Other native rates receive typed
`unsupported_native_signal_profile` loudness/true-peak values, while their
original-rate sample statistics remain available in the native child. Silence,
insufficient duration and below-gate cases retain existing typed reasons.
Numeric meter observations remain `runtime_qualified: false`.

`qc.json` is written last. Retain its actual file digest separately. Readback
executes nothing: it checks the exact producer, final original/snapshot bytes,
components, native child acceptance, retained decoded PCM, report pin and meter
scope. Any final file byte change, including metadata-only edits, invalidates
QC even if samples are unchanged. Failures preserve private partial work; there
is no partial-output adoption or implicit overwrite/retry. The storage estimate
and native byte/time/capture limits are admission bounds, not an OS quota or
reservation.

Destination preflight accepts explicit `accepted_audio_with_qc` (`subject`,
`qc_directory`, `qc_receipt_sha256`). Final quantities drive that preflight;
unavailable final readings cannot borrow an earlier original-master meter.
Old plain accepted PCM/export inputs retain their existing behavior. Manual
posting uses the actual source WAV/FLAC filename, with separate exact-file and
public-metadata review. There is no upload or release action.

The inert setup selection `final_lossless_audio_qc` names explicit FFmpeg/probe
roles and closed `format`, `tool_timeout_milliseconds`,
`maximum_tool_output_bytes`, `maximum_pcm_bytes`, `maximum_working_bytes` and
`minimum_free_bytes` options. Its source-free setup document supplies native
configuration/bounds only; actual subject/provider are added to the QC request.
It participates in whole selected publishing readiness without media reads,
probe/decoder/meter launch, downloads or configuration writes.

Nine SDK/CLI source cases cover native precision/clock identity, changed samples,
typed unavailable quantities, absent acceptance, closed options, literal native
decode arguments, receipt version/qualification, dependency readiness and CLI
requirements. They are authored, uncompiled/unrun. Tests/compiler/build/
dependencies/lint/format/schema/docs/corpus, product CLI/SDK/native/provider/
media/model and CI execution remain deferred to flow #88, scheduled last with
its original 45-ticket ledger. No private Akashic assets/review were used.
Known alignment cancellation/start-marker failure remains unresolved, cause
unproven. ADR-0019 remains proposed.

## Explicit native 44.1 kHz QC

Select `aniflow.final-lossless-audio-qc-request/v2` with the same closed fields
to add original 44.1 kHz mono/stereo PCM16/PCM24/float32 metering. The intent
and final report use v2; native 44.1 kHz readings use the separately pinned
`aniflow.native-pcm-loudness/v3` view of the existing two-pass ebur128 owner.
Its window thresholds and command evidence use 44100, with independent native
sample-peak agreement. Final file samples, complete count, rate, channels and
precision stay intact. Existing 48 kHz native metering keeps its v2 meter receipt.

The same plan/run/status CLI accepts the explicitly versioned request. Current
final-QC preflight can consume either report version. Old final-QC request/report
v1 stays 48 kHz only, and saved v1 44.1 kHz unavailable readings remain valid;
new code does not upgrade them. Other unsupported rates remain unavailable.
Silence, insufficient duration and below-gate readings keep typed reasons;
finite overs remain observations, and no numerical quantity qualifies the
algorithm or grants musical approval. The current audio publishing v1 request
keeps its legacy final-QC selection; a later explicit recipe version can select
this new lane. Inert dependency readiness uses the same actual native meter
features, without implying rate/platform qualification.

Five focused original-rate/version/precision/refusal cases are authored,
uncompiled/unrun. All standing execution/CI deferrals remain in force; flow
#88 remains last. ADR-0019 remains proposed. No actual audio was processed.
