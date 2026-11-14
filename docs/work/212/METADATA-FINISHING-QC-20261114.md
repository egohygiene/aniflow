# Metadata finishing and fresh final QC — November 14, 2026

This [#212](https://github.com/egohygiene/aniflow/issues/212) continuation uses
the existing metadata-only finishing, accepted completion, exact-final audio-QC
and destination-preflight owners. It preserves the original master, delivery,
supplied lyrics and prior QC. Captions are unselected; no word/timing/preview/
creative/final review declaration is generated.

## Actual repairs and retained failures

The first native finishing plan correctly refused a generated Pipeline v3
document containing an absent audio selection as `null`. Metadata remux now
reuses assembly's small crate-internal authoring serializer for source/artifact
selections. Shared temporal serialization, canonical provenance, selection
meaning and the global null refusal remain unchanged.

The exact existing public remux argument builder exposed another producer fault:
FFmpeg's default 1000-tick movie edit list shortened the already-admitted
128000-sample extent to 127968 samples. The exact observer refused that result.
The native producer now explicitly represents the observed video time-base
denominator and its checked LCM with the selected AAC sample rate. For this
fixture those scales are 24 and 48000. Unsupported/overflowing representations
still refuse. No trim, padding, timestamp reset, resampling, gain or reencoding
is added. Exact packet/configuration/priming/clock validators remain mandatory.

A corrected argument-builder diagnostic preserves all 64 video and 126 AAC
packets, rational PTS/DTS/durations, codec configuration and priming. Both failed
receipts remain failures. Two standalone tests execute the actual clock helpers,
including overflow refusal; the newly authored full Rust unit regression remains
unrun with the rest of the full Rust test modules.

## Actual fixed-CLI continuation

The locked/offline Rust 1.85 diagnostic build passed in **3m28s**, using two jobs,
opt-level 1, no debug information, no LTO and 16 codegen units. Its executable is
38955624 bytes, SHA-256
`317432fb334d5fcc0859b731a5b19e197e038e29c7a75e3c4e769190add88021`.
The earlier canonical and output-file-repair executables remain byte-identical.

The refreshed initial QC/preflight pilot passes in **3.770 seconds**. The final
metadata continuation passes in **21.970 seconds**:

- Metadata-only finishing writes the explicit synthetic title into a distinct
  107301-byte `metadata_delivery` candidate. All 64 video and 126 AAC packet
  identities/configuration/rational clocks survive; full decode passes.
- Picture and stereo audio retain their exact 8/3-second/128000-sample extent.
  Fresh final-file QC binds the new bytes and unchanged decoded PCM, observing
  −21.8 LUFS and −20.7 dBTP without normalization or meter qualification.
- Fresh matching QC satisfies the explicitly synthetic dated preflight. Reusing
  original QC under the new producer/artifact refuses with the exact structured
  State message and **exit 8**, despite unchanged elementary audio.
- Original QC remains valid for its original owner. Readback and completed
  finishing resume preserve both QC inventories and the new candidate; the
  complete original pilot inventory and executables remain unchanged.
- Completion remains unapproved/unasserted. Next action is
  `review_final_candidate`; no caption-review file or posting package is created.

The QC transport now also requires native State exit 8 alongside its exact JSON
command/category/message. A valid-looking error paired with panic exit 101 cannot
pass. Seven QC observer cases and four metadata observer cases pass. The original
first-checkpoint source/run receipt remains its historical observation; this
continuation records the stronger helper and refreshed run separately.

```sh
task publishing:qc-pilot PILOT="/absolute/passed-synthetic-pilot" \
  ANIFLOW="/absolute/current-aniflow" OUTPUT="/absolute/fresh-qc"
task publishing:metadata-qc-pilot QC="/absolute/passed-qc-continuation" \
  ANIFLOW="/absolute/current-aniflow" OUTPUT="/absolute/fresh-metadata-qc"
```

Without Task, invoke `python3 -B scripts/publishing-metadata-qc-pilot.py` with
`--qc-pilot-root`, `--aniflow` and `--output`. Output must be fresh and disjoint
from both input inventories; nested output refuses before creating anything.
Private failures/logs are retained. The shared bounded inventory, CLI deadline
and native process-group limitations from the [QC guide](FINAL-VIDEO-QC-20261114.md)
still apply.

The [sanitized observation](METADATA-FINISHING-QC-OBSERVATION-20261114.json) binds
actual source/build/harness/tool/output/receipt hashes and keeps failures distinct.
Synthetic metadata/requirements establish no identity or live platform policy.
Human approval, captions, native-host/meter qualification, ordinary cropped or
padded-source admission, descendant shutdown, release and publication remain
separate. Flow #88 remains unchanged and last.
