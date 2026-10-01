# Exact timing and explicit streams

The #32 implementation records source timing as reduced signed rational values
and input-global stream indices. Inspection and processing support are separate
facts. `24000/1001` remains an exact rate; floating-point summary fields never
select segment boundaries, reconstruct frames or establish synchronization.

This implementation pass has **not been compiled or tested**. Test/check execution
was deferred by the maintainer to [#64](https://github.com/egohygiene/aniflow/issues/64).
The statements below describe the authored behavior and contracts, not observed
native/platform qualification.

## Inspect before selecting a processing profile

```bash
aniflow --output json temporal inspect "source.mp4"

aniflow --output json temporal inspect "source.mp4" \
  --video-stream 2 --audio-stream 5 --discard-stream 0,1,3,4
```

The result is `aniflow.temporal-inspection/v1`. A well-formed source can produce a
successful inspection whose `processing.supported` is false. Consumers must
check that field and its typed diagnostics. Malformed or excessive observations
produce a typed error instead. `error.temporal.code` retains the reason without
requiring human-message parsing.

Stream selection uses input-global indices, including subtitle, data,
attachment and attached-picture streams. An attached picture is cover art, not a
moving-video candidate. Automatic selection requires a unique moving-video and
a unique optional audio stream; neither inventory order nor default disposition
breaks a tie. Every unselected stream needs explicit `--discard-stream`
acknowledgement before processing. `--no-audio` selects no audio and still
requires acknowledging omitted audio indices. Conflicting, absent, selected-and-
discarded or duplicate indices are refused.

The same flags apply to `inspect`, `plan`, `run`, `segment plan` and `segment run`.
Resume uses the frozen selection, source digest and complete temporal record;
it cannot silently adopt a different stream selection.

## Supported and refused temporal profiles

| Input condition | Authored behavior |
| --- | --- |
| Contiguous CFR decoded video, including fractional rates | Reconstruct on the observed exact frame grid; verify every frame interval and count |
| Reordered packet PTS with monotonic DTS | Retain separate decode/presentation observations; fixed-grid transcode can proceed |
| VFR presentation or gaps/overlaps | Inspect when observations are well formed; refuse fixed-rate processing rather than duplicate, drop or retime frames |
| Nonzero or negative presentation origin | Preserve the exact offset in inspection; refuse this zero-origin processing profile |
| Multiple moving-video/audio streams | Require explicit selection and acknowledgement of every discard |
| Subtitle, data, attachment, cover-art or unknown extra stream | Report its identity; require explicit discard acknowledgement |
| Missing PTS, duration, packet PTS/DTS or time base | Refuse; do not substitute best-effort timestamps or an average-rate estimate |
| Declared duration or selected A/V clock contradiction | Refuse using exact, recorded tolerances |
| Excessive probe output, duplicate identities or arithmetic overflow | Fail with a stable temporal diagnostic |

Each FFprobe invocation is bounded to 120 seconds and 64 MiB of captured output.
Each selected frame/packet array is limited to one million observations and the
inventory to 1,024 streams. Inventory, decoded frame PTS/duration and demuxed
packet PTS/DTS are separate observations. The probe reads a private copy and
source digests are checked around inspection. This is bounded local execution;
it does not provide OS isolation or establish FFprobe authenticity.

The processing profile requires exact zero-origin presentation and contiguous
decoded frame intervals. Video declaration-versus-observation duration tolerance
is one source time-base tick. Audio declaration tolerance is one observed decode
block. A/V origins must match exactly; end tolerance is one observed video frame
plus one observed audio decode block. Reconstruction must preserve every video
frame interval exactly and uses the same explicitly recorded A/V end tolerance.
These are physical timing checks, not evidence that transformed content is true
to the creator's intent.

## Segment and reconstruct

```bash
aniflow --output json segment plan \
  --input "source.mp4" --output-directory ".aniflow/segments/demo" \
  --segment-duration-ms 10000 --mode transcode-h264-aac
```

Segment boundaries follow exact source frame intervals. Millisecond fields are
ceil-rounded compatibility summaries; `source_time` is authoritative. Audio
trim boundaries round upward to the selected source sample clock, with an error
below one sample, and are bounded by its actual interval. FFmpeg seek text is an
explicit microsecond projection; actual output must still satisfy the exact grid.
Each segment remains below 30 seconds.

Stream-copy additionally requires selected packet PTS/DTS equality, nonnegative
packet times, keyframe boundaries and aligned audio decode boundaries. Inputs
outside that narrower profile are refused with a recommendation to use explicit
transcoding. Reconstruction verifies segment digests, temporal records and source
windows, concatenates the verified video ordinals on the original CFR grid and
restores the original selected audio once as AAC. Segmented audio padding does
not become the reconstruction clock. The original source must remain available
and unchanged; reconstruction exclusively publishes to a new output path.

New durable records use `aniflow.segment-plan/v2`, `aniflow.segment-manifest/v2`
and `aniflow.reconstruction-report/v2`. Published v1 schemas remain historical.
V1 segment state and old Pipeline v2 runs without exact temporal evidence cannot
be resumed or reconstructed under the new policy; start a new run. Pipeline v3
provider manifests and the audio-family sample-clock contracts retain their
existing versions. This checkpoint does not silently retime generic Pipeline v3
provider outputs or replace their capability-specific contracts.

## Derived-media source clocks

Pipeline v2 frame, audio, video and master artifacts are indexed in
`metadata/temporal-artifacts.json` using `aniflow.temporal-artifact-index/v1`.
Each binding carries a workspace-relative path, artifact digest, original source
digest, inspection digest and exact selected source stream intervals. Extracted
and processed frames retain their original ordinal and interval. Segment records
carry corresponding source windows and independently observed output clocks.

Frame cardinality and filenames, intermediate audio/video clocks, final video
timing and source digests are checked before delivery. A cached support flag does
not override a recomputed refusal. Source checks detect net drift; they are not
an atomic snapshot against arbitrary concurrent external writers. Path/digest
bindings establish recorded lineage, not content authenticity. General layered
delivery evidence and completion gates remain [#33](https://github.com/egohygiene/aniflow/issues/33).

## Authored coverage and later qualification

- `scripts/generate-temporal-fixtures.py` authors 30 generator-first FFprobe
  protocol fixtures: CFR/fractional clocks, VFR, offsets, packet reordering,
  malformed evidence, stream ambiguity, explicit choices and mixed-stream refusal.
- `tests/temporal_contract.rs` covers exact arithmetic, typed refusals, immutable
  selection, source windows, reconstruction intervals and workspace confinement.
- `tests/temporal_native.rs` authors native FFmpeg fractional segmentation/resume/
  reconstruction and multi-video selection checks using tiny synthetic inputs.
- The published JSON schemas describe wire shape; Rust enforces reduced rational
  values and cross-field identity, order, interval and tolerance invariants.

No tests, compiler/check runs, schema qualification, full smoke, package checks
or hosted CI polling were executed for this pass. Real media, broader containers,
native-platform/MSRV breadth and releases remain unqualified. Later validation
must execute the authored coverage and record failures honestly under #64.
