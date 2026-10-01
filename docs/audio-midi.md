# Audio-to-MIDI candidates

`audio midi extract` estimates notes from a supported audio source using one
optional local Basic Pitch profile. It retains source-relative note timing,
model/tool/configuration identities and explicit uncertainty. The result is a
probabilistic candidate for review, not an authored score, instrument
identification or proof that every note was detected. Source audio and authored
MIDI remain unchanged; this operation does not accept an authored MIDI file as
an overwrite target.

This is [#50](https://github.com/egohygiene/aniflow/issues/50), checkpoint nine
of the open [#13 audio mini roadmap](https://github.com/egohygiene/aniflow/issues/13),
implemented in [PR #62](https://github.com/egohygiene/aniflow/pull/62).
It builds on [technical inspection](audio-inspection.md) and the
[normalized audio contract](audio-analysis.md). Extraction produces evidence;
Standard MIDI File creation is a separate explicit export.

## Selected optional profile

The adapter targets Basic Pitch 0.4.0 at upstream revision
`9991303bba609a3b93089d13ec80d1d495083596`. It uses an explicitly supplied local
`icassp_2022` ONNX model with ONNX Runtime's CPU execution provider. There is no
automatic installation, model download, cache lookup, remote service, or fallback
model runtime.

| Property | Supported profile |
| --- | --- |
| Source | Nonempty mono PCM16 RIFF WAV admitted by technical inspection |
| Clock | Exactly 22,050 Hz, source-relative origin zero |
| Audio bounds | At most 120 seconds and 8 MiB |
| Scope | Whole supported mix, or a full accepted stem through the lineage importer |
| Processing | Local CPU; no resampling or downmix |
| Notes | At most 8,192 events; MIDI pitch 21 through 108 |
| Timing | Native seconds converted to integer microseconds, then exact rational source-relative time |
| Strength | Native mean activation in the range zero through one, retained as activation |
| Confidence | Explicitly unavailable; activation is not a calibrated correctness probability |
| Instrument/channel | Not inferred from the recording |

A supported monophonic or polyphonic candidate can still contain missed,
spurious, overlapping or mistimed notes. Silent input or unsupported sample
rate, channel count or duration has an explicit unavailable result; malformed
or oversized files are refused. A successful model run with no notes has a
candidate containing an explicit empty note list. Empty detection does not
establish silence or the absence of pitched music. Neither outcome receives an
invented note.

The pinned
[inference implementation](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/basic_pitch/inference.py),
[note conversion](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/basic_pitch/note_creation.py)
and [constants](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/basic_pitch/constants.py)
define the admitted upstream behavior. The adapter uses fixed onset/frame
thresholds of 0.5/0.3, onset inference, an 11-frame minimum note length and the
upstream Melodia postprocessing option. Pitch-bend generation is disabled.
Source/profile checks and independent result validation remain aniflow's job;
zero process exit alone cannot establish accepted output.

## Configure explicit local dependencies

Author a document using the
[settings schema](../providers/audio-midi/configuration.schema.json) and
[configuration example](../providers/audio-midi/configuration.example.json).
The facade constructs the separate source/tool-bound
[provider wrapper](../providers/audio-midi/provider-configuration.schema.json).
Use exact absolute local locations and byte identities for the selected Python,
repository adapter and ONNX model. Model identity and upstream revision are
supplied declarations bound to bytes, not independent publisher authentication.

| Settings field | Meaning |
| --- | --- |
| `schema` | `aniflow.audio-midi.configuration/v1` |
| `python` | Absolute `executable`, exact Python `version` and `sha256` |
| `adapter` | Absolute `path` and exact `sha256` of the repository adapter |
| `runtime` | Installed-distribution inventory `sha256`, `file_count` and `byte_count` |
| `model` | Absolute `path`, `sha256`, `byte_size`, model ID and supplied upstream revision |
| `tool_timeout_milliseconds` | Explicit deadline from 1 through 120,000 ms |
| `maximum_tool_output_bytes` | Explicit capture bound from 1,024 through 2,097,152 bytes |

The model ID is `basic-pitch-onnx-icassp-2022` and the admitted revision is the
pinned upstream commit above. The admitted model is exactly 230,444 bytes; the
adapter also checks its upstream Git blob identity
`c30e5f9438e798604b7177aa26be1fe64482f767` in addition to the caller's SHA-256
pin. Settings are bounded to 64 KiB, native observations to 2 MiB and reports
to 8 MiB. Runtime inventory is bounded to 50,000 files and 2 GiB. These are validation limits, not a promise
that every admitted environment can complete inference before its deadline.
The suggested outer Pipeline v3 limit is 384 MiB and eight artifact files;
`MidiRequest::new` preserves explicitly supplied inspection limits.

The admitted Python profile is 3.10 or 3.11 with these exact key packages:

| Distribution | Version |
| --- | --- |
| `basic-pitch` | `0.4.0` |
| `onnxruntime` | `1.20.1` |
| `numpy` | `1.26.4` |
| `librosa` | `0.10.2.post1` |
| `scipy` | `1.13.1` |
| `pretty_midi` | `0.2.10` |

Preflight observes the selected environment's installed distribution inventory,
including metadata identities, and compares it with the configured evidence.
The profile requires the ONNX backend; additional installed model runtimes are
refused rather than silently selected. Package metadata and Python executable
pins do not authenticate Python's standard library, every imported native
library or a complete operating-system dependency closure. Keep the prepared environment trusted and
stable. This requires a separately prepared sole-ONNX environment. Basic Pitch
0.4.0's default package resolver requests TFLite on Linux/Python 3.10 or
TensorFlow on Python 3.11, so an ordinary default `pip install` does not produce
this admitted profile. No dependency-resolver-complete runtime or actual model
qualification is claimed here; installation is an explicit separate operator
step, outside the ordinary inspect/plan/run workflow.

The adapter is invoked with direct arguments through the selected Python:

```text
<python> -I -B <adapter.py> --probe
<python> -I -B <private-adapter.py> --input <private-source.wav> --model <private-model.onnx>
```

Python's isolated mode suppresses ambient Python path settings, and `-B`
prevents bytecode cache creation. The interpreter remains at its explicit local
location so its environment can resolve the prepared packages. The model,
adapter and selected source are privately staged and their byte identities
rechecked. Deadlines, cancellation and output limits constrain subprocess work.
These controls and declared offline effects are not an operating-system sandbox
or a guarantee that arbitrary untrusted native code cannot access the network.

## Plan, extract and resume

Use the existing explicit
[technical-tool configuration](audio-inspection.md#configure-exact-local-tools)
and separate MIDI settings:

```bash
aniflow --output json audio plan --analysis midi-candidates \
  --input "/absolute/synthetic/notes.wav" \
  --configuration "/absolute/config/tools.json" \
  --midi-configuration "/absolute/config/midi.json"

aniflow --output json audio midi extract \
  --input "/absolute/synthetic/notes.wav" \
  --configuration "/absolute/config/tools.json" \
  --midi-configuration "/absolute/config/midi.json" \
  --output-directory "/absolute/generated-runs"

aniflow --output json status-v3 "/absolute/generated-runs/RUN_ID"

aniflow --output json audio resume "/absolute/generated-runs/RUN_ID" \
  --analysis midi-candidates \
  --input "/absolute/synthetic/notes.wav" \
  --configuration "/absolute/config/tools.json" \
  --midi-configuration "/absolute/config/midi.json"
```

Planning performs dependency preflight and binds the source without creating a
run workspace or executing note inference. A ready dependency probe does not
prove source acceptance or model accuracy. `audio plan` and `audio resume`
retain their technical default; select `midi-candidates` explicitly. The
`audio midi extract` command selects this analysis family.

The [stem-selection flags](audio-stem-lineage.md) also apply. Keep `--input`
bound to the original mix and supply the accepted stem run, stage and artifact
ID. Note timing belongs to the selected stem's clock. Retained lineage does not
prove onset or phase alignment with the mix and does not rerun separation.

Task wrappers are `audio:midi:plan`, `audio:midi:extract` and
`audio:midi:resume`. Supply `INPUT`, `CONFIGURATION` and `MIDI_CONFIGURATION`;
execution also takes `OUTPUT`, while resume takes `RUN`. `ANIFLOW_BIN` selects
an explicitly built executable:

```bash
task audio:midi:extract \
  ANIFLOW_BIN="/absolute/aniflow/target/debug/aniflow" \
  INPUT="/absolute/synthetic/notes.wav" \
  CONFIGURATION="/absolute/config/tools.json" \
  MIDI_CONFIGURATION="/absolute/config/midi.json" \
  OUTPUT="/absolute/generated-runs"
```

Task values are passed as quoted arguments. Status is read-only; compatible
run-local resume reobserves source, Python, adapter, model and dependency-probe
identity before accepting reusable checkpoints. Changed dependencies do not
reuse an older candidate. This feature does not add a cross-run cache.

## Normalized evidence and API

Technical inspection and optional stem lineage precede the `analyze_midi`
stage under the existing Pipeline v3 lifecycle. Extraction produces the MIDI
companion report and normalized analysis. It does not create a placeholder MIDI
file for an unavailable result. An empty note candidate and an unavailable
result both leave normalized analysis partial, with the MIDI capability marked
unavailable and an explanatory diagnostic. A nonempty valid candidate means
structural completion, not a verified score.

| Export | Run-relative path |
| --- | --- |
| `midi` | `artifacts/audio-midi/midi.json` |
| `analysis` | `artifacts/audio-midi/analysis.json`, artifact ID `midi_analysis` |

The [MIDI report](contracts/audio-midi-v1.schema.json) binds the exact source,
provider, tool/model/configuration identities and probabilistic note evidence.
The [preflight report](contracts/audio-midi-preflight-v1.schema.json) separates
dependency readiness/refusal from source acceptance and inference. Rust parsing
adds semantic relationships beyond JSON Schema shape, including source bounds,
note ordering and the derivation of exported timing and velocity.

The public `aniflow::audio_midi` module exposes
`MidiRequest::new(inspection, configuration)` with an `AudioInspectionRequest`
and validated `MidiConfiguration`, plus `preflight`, `plan`, `run` and `resume`.
Failures retain typed dependency, inspection or planning evidence. Existing
cancellation/progress callbacks and Pipeline v3 run results remain authoritative
for execution status. Provider `org.egohygiene.aniflow.audio-midi` and capability
`aniflow/audio-midi-extraction` use version `1.0.0`. The adapter's strict
[observation](contracts/audio-midi-observation-v1.schema.json) and
[probe](contracts/audio-midi-probe-v1.schema.json) contracts remain separate from
accepted normalized evidence. Candidate evidence is an ordinary artifact; it is not
#33's general completion gate or #34's cross-run reuse mechanism.

## Explicit Standard MIDI File export

```bash
aniflow --output json audio midi export \
  --candidate "/absolute/generated-runs/RUN_ID/artifacts/audio-midi/midi.json" \
  --output-directory "/absolute/generated-evidence/midi-candidate"
```

`audio:midi:export` takes `CANDIDATE` and `OUTPUT`; `ANIFLOW_BIN` selects the
executable. The public `export_midi_file` function exports validated candidate
evidence into a new output package. The destination must be new, and authored
MIDI is never an overwrite target. Keep the normalized evidence with the MIDI:
a standalone MIDI file does not establish model identity, confidence or score
truth.

| Package member | Meaning |
| --- | --- |
| `candidate.mid` | Bounded Standard MIDI File candidate |
| `candidate-report.json` | Exact original report bytes and source/provider evidence |
| `notes.json` | Canonical native and normalized note observations |
| `export-report.json` | Source/report/member digests, mappings and independent read-back |

`export-report.json` is published last as the completion marker, after the
source report is rechecked. An interrupted directory without that marker is
incomplete and must not be consumed as a finished export. An unavailable result
cannot be exported. An explicit empty candidate can be exported with no note
events and `empty_candidate: true`; it still does not establish silence.

The declared subset is Standard MIDI File type 0 with one track, PPQ 960 and a
constant 500,000 microseconds per quarter note (120 BPM). This tempo is a fixed
serialization clock, not an estimate of musical tempo. Note on/off events share
zero-based channel 0 and program 0 (commonly displayed as channel 1 and the
first General MIDI program). These are declared placeholders and do not identify
the instrument in the audio. Other tracks, tempo changes, pitch bends, controllers,
SysEx, authored notation and broader MIDI feature preservation are outside this
export subset. The file is bounded to 256 KiB and writes explicit event statuses;
running status and SMPTE clocks are not accepted. At a shared tick, note-off
events precede note-on events.

Two distinct conversions are recorded:

- Native timing becomes integer microseconds with nearest rounding and ties
  upward, introducing at most half a microsecond at that conversion boundary.
  This numerical tolerance says nothing about acoustic model timing accuracy.
- Source-relative rational timing becomes MIDI ticks using nearest rounding
  and ties upward. At the fixed clock, one tick is 1/1,920 second; a boundary
  moves by at most half a tick, approximately 260.417 microseconds. A note that
  collapses to zero ticks is refused instead of lengthened silently. Overlapping
  occurrences of the same pitch, including overlap caused by quantization, are
  refused because this single-channel subset cannot pair them unambiguously.

Native mean activation becomes integer velocity by rounding
`127 × activation` with ties to even. Zero velocity is refused; velocity is a
serialization of model strength, not measured loudness or calibrated
confidence. Evidence retains the original activation and the declared losses.
The independent MIDI read-back checks event ordering, note on/off pairing,
pitch/velocity ranges, count/size bounds and agreement with the normalized
candidate before export is accepted. Process success and a `.mid` extension are
insufficient.

## Local evidence and remaining gates

Focused checks use only synthetic protocol, note and audio fixtures:

```bash
task audio:midi:corpus
task audio:midi:schema
```

The [checkpoint record](validation/aniflow-50-checkpoint.md) and
[local receipt](validation/aniflow-50-local.json) record pushed savepoints,
364 Rust tests, 36 focused Rust 1.85.1 tests, strict Clippy/docs/package checks,
the full synthetic smoke run, captured-document validation and external MIDI
read-back. Synthetic adapter behavior,
SMF read-back, refusal and resume tests can establish contract conformance.
Actual Basic Pitch/model inference and musical accuracy, native macOS/other
platforms, broader input/model profiles, complete native dependency closure,
hosted CI and release qualification require separate evidence. No real media,
model weights, paid APIs or implicit installation is required for these tests.

Basic Pitch's [Apache-2.0 license](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/LICENSE)
and [notice](https://github.com/spotify/basic-pitch/blob/9991303bba609a3b93089d13ec80d1d495083596/NOTICE)
apply to the pinned upstream repository, which includes its model asset. No
separate model license was found at that revision. ONNX Runtime has its own
[MIT license](https://github.com/microsoft/onnxruntime/blob/v1.20.1/LICENSE), and
the remaining installed packages retain their own notices. These are recorded
upstream declarations, not authentication of arbitrary caller-supplied model
bytes or a legal qualification. aniflow vendors no model weights or ML runtime.
