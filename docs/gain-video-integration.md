# QC and video delivery for a selected gain candidate

First remeasure the exact accepted gain WAV with the existing final-lossless-QC
owner. Its `subject` can now be:

```json
{"kind":"accepted_gain_candidate","run_directory":"/private/EXACT_GAIN_RUN"}
```

Both existing versioned final-lossless-QC requests and recognized resumable QC
can use this source. The native gain owner must currently accept the file;
a loose WAV plus an invented receipt is insufficient. Source-shape projection
uses the candidate's actual byte digest, float encoding, rate, channels and full
sample count without inventing measurements. Existing signal/meter owners then
measure the candidate. A 44.1 kHz candidate uses the explicit v2 QC request.

For video, `delivery_audio::gain_video` requires a gain candidate derived from
the exact original video's QC. Its fresh WAV QC must match the original selected
policy before another encode is planned. Supply `aniflow.gain-video-request/v1`:

| Field | Selection |
| --- | --- |
| `gain_run_directory` | Current native gain run |
| `gain_qc_directory`, `gain_qc_receipt_sha256` | Current exact gain-WAV QC and separately retained receipt digest |
| `settings` | Existing assembly/tool/resource settings with an explicitly selected metadata policy |
| `audio_bitrate_kbps` | Explicit AAC bitrate, 64..320 in steps of 16; no universal platform bitrate is inferred |
| `provider_executable` | Exact separately pinned Aniflow native provider |

```bash
aniflow --output json delivery gain-video plan --request video-gain.json
aniflow --output json delivery gain-video run --request video-gain.json --output-directory /private/new-video
aniflow --output json delivery gain-video status --run-directory /private/new-video/EXACT_RUN
aniflow --output json delivery gain-video resume --request video-gain.json --run-directory /private/new-video/EXACT_RUN
```

The initial video subset is a current plain/burned-caption H.264/yuv420p BT.709
SDR MP4 with one 48 kHz mono/stereo AAC-LC track. Soft subtitles, extra tracks,
chapters/programs, audio-only-derived candidates and other clocks/layouts refuse.
This does not silently discard them or resample a 44.1 kHz music master.

The one ordinary native v3 stage snapshots both selected media files and exact
components. It stream-copies picture and AAC-encodes the selected gain WAV;
no volume filter runs a second time. Exact original-rate sample ticks map the
audio origin through `asetpts`, preserving offsets without decimal-second
rounding. A nonintegral sample offset or unrepresentable MP4 timescale refuses.
There is no shortest/trim/pad/stretch/downmix behavior.

Existing media observers compare exact ordered picture packet hashes and
PTS/DTS/duration/flags, decoded frame presentation intervals, geometry/color,
audio rate/channels and the entire original audio start/end/tail. The fixed
metadata policy, bounded MP4 atom inspection, complete decode and faststart
checks remain separate observations. Any failure preserves unaccepted work.
V3 owns process/resource/cancellation/locks/checkpoints and artifact/delivery
acceptance; this adapter adds no state engine or review database.

The resulting ordinary `final-video-handoff/v1` is a new candidate, with
`requires_fresh_encoded_file_qc` in its private report. New AAC can change true
peak and loudness. Run existing final-video QC and then the selected policy check
on **that exact MP4**, after any further caption mux/metadata edit. A passing
WAV check never qualifies a later video. Native integrity/temporal observations,
numeric goal matching, meter/platform qualification and human musical review
remain distinct. No approval or upload is inferred.

Four useful library/parser boundary cases are authored, uncompiled/unrun. The
source schemas now include the explicit accepted-gain source and reuse the
canonical media-observation shape for policy assessments. All tests/compiler/
build/package/dependencies/lint/format/schema/docs/drift/product validators,
CLI/SDK/native/media/provider/model/browser/container/CI, acquisition, actual
source replacement/disposal, signing/timestamps and publication remain deferred.
Real AAC priming/padding, clocks, resource/cancellation, codec/platform and
musical quality must be qualified later. Flow #88 remains last and unchanged;
the known alignment cancellation/start-marker failure is unresolved, with cause
unproven. #212 remains open, #35 closed and ADR-0024 proposed.
