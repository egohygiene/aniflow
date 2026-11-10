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

## Explicit recognized resume

`delivery_audio::final_qc::resumable` adds a separate
`aniflow.resumable-final-video-audio-qc-request/v1` wrapper with one `qc` field
containing the unchanged final-audio-QC v1 request. The existing fresh-only API
and saved legacy roots keep their meanings. Use a new private root:

```bash
aniflow --output json delivery audio-qc resumable plan --request resumable-video-qc.json
aniflow --output json delivery audio-qc resumable run --request resumable-video-qc.json --run-directory /private/video-qc
aniflow --output json delivery audio-qc resumable status --run-directory /private/video-qc
aniflow --output json delivery audio-qc resumable resume --run-directory /private/video-qc
```

The immutable intent binds exact current accepted final video, selected AAC
payload/clock/extent, explicit components/settings and conservative whole-work
admission. `capture.json` commits private final-file/component snapshots.
`decoded.json` commits the actual bounded temporal observation, complete 48 kHz
float PCM and actual two-stage native signal plan. Rational source audio offsets
remain separate from the zero-origin decoded sample clock. There is no trim,
pad, rate/channel conversion or normalization. The existing decoder, extent
parser and readback checks are shared with source-signal composition.

Ordinary native acceptance/checkpoints stay in `signal/children/signal`.
`meter.json` commits the separately unqualified meter observation and exact native
manifest. `signal/video-audio-signal.json` then `qc.json` are published last. A
restart can reuse committed capture/decode/native/meter boundaries, and finish
recognized interrupted receipt publication without repeating accepted work.
The final existing-shaped QC report retains its external receipt digest and
feeds unchanged destination preflight through exact current readback.

Status reconstructs current native plans from identities without probes or
media execution. Execution retains native dependency preflight. Unknown roots,
unmarked capture/decode payloads, malformed/half-written markers, source/tool/
settings/plan changes, symbolic members and stale writer locks are preserved
and refused; no overwrite or inferred adoption is offered. Documents are bounded
to 2 MiB, including the retained temporal observation; larger observations refuse.
Working estimates are admission bounds, not OS quotas or reservations.

Six SDK cases and one CLI case are authored, uncompiled/unrun. All standing
execution/CI deferrals remain. No actual private video/audio, native meter,
creative review, release or posting occurred. Flow #88 remains last with its
original 45-ticket ledger; the known alignment cancellation/start-marker failure
remains unresolved, cause unproven. ADR-0019 remains proposed.
