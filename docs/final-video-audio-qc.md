# Observe audio from the exact final video

Run QC after every byte-changing encode, caption mux and metadata operation.
Select the current native final-video run and artifact ID; original PCM or an
earlier video does not establish loudness for a different final file.

`delivery_audio::final_qc::{plan,run,observe}` and the thin commands below compose
the existing complete AAC decode, accepted native signal analysis and separately
versioned loudness meter. The initial lane is native plain/burned-caption video
with 48 kHz mono/stereo AAC-LC. Soft-subtitle input, missing audio, other rates or
channels are refused explicitly. No gain, normalization, resampling, downmix,
trim or automatic repair is performed. Overs and typed unavailability remain
observations; there is no universal target or musical quality verdict.

Supply `aniflow.final-audio-qc-request/v1`: `source_run_directory`, `artifact_id`,
`configuration` (existing finite assembly/tool configuration),
`provider_executable`, and `maximum_pcm_bytes`. A metadata policy on the source
configuration is excluded only from the decoder request; source bytes and the
original configuration are retained. The complete decoded float32 WAV must fit
the explicit PCM/working bounds; no truncated result is accepted.

```bash
aniflow --output json delivery audio-qc plan --request final-qc.json
aniflow --output json delivery audio-qc run --request final-qc.json --output-directory /private/final-qc
aniflow --output json delivery audio-qc status --directory /private/final-qc --receipt-sha256 KNOWN_QC_JSON_SHA256
```

The fresh private root contains `intent.json`, a native `signal/` child and last
`qc.json` completion marker. Retain the raw `qc.json` digest separately. Partial
roots are preserved and require explicit reconciliation or a new root; there is
no automatic adoption/resume of incomplete measurement. Status rechecks current
native video and signal acceptance, retained complete PCM/tool identities and
the pinned meter receipt without executing a tool. This is fixed composition,
not another acceptance database. Original sources remain immutable.

Preflight can select `accepted_video_with_qc` with the native `run_directory`,
`artifact_id`, `qc_directory` and `qc_receipt_sha256`. Both integrated loudness
and true peak must be finite measured quantities when requirements request
loudness evidence. Missing, silent, short or gated unavailable quantities remain
missing evidence. A stale QC from an earlier mux or metadata edit is refused.
Old `accepted_video` remains available without implying loudness evidence.

The native signal child supplies accepted PCM statistics. The separate pinned
loudness observation remains `runtime_qualified: false`; source inclusion,
numeric availability and preflight matching do not establish meter qualification,
creative approval, identity authentication, platform acceptance or posting.
Private receipts/paths stay out of the public posting manifest.

Five SDK comparison/refusal cases and one CLI parser case are authored,
uncompiled/unrun. All execution/validation remains deferred to
[flow #88](https://github.com/egohygiene/flow/issues/88), scheduled last. The known
alignment cancellation/start-marker failure remains unresolved; cause unproven.
